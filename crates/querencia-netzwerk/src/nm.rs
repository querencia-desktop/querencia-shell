//! NetworkManager over D-Bus (system bus, GIO): the backend that can
//! connect, disconnect and switch WLAN.
//!
//! The whole state comes from one `GetManagedObjects` call on
//! `/org/freedesktop` (NetworkManager's object manager, as libnm reads it
//! since 1.22): manager, devices, access points, active connections, IP
//! configurations. Every signal NetworkManager sends (`PropertiesChanged`,
//! `InterfacesAdded`/`Removed`, `StateChanged`, …) schedules one more read
//! after [`DEBOUNCE`], so a burst of changes costs one call; nothing polls.
//! Saved WLAN profiles are read once each (`GetSettings`) and again after
//! their `Updated` signal.
//!
//! Every call that changes something allows interactive authorization: if
//! polkit asks, the session's polkit agent shows its dialog.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};
use std::time::Duration;

use gtk4::gio::{self, DBusCallFlags, DBusConnection, DBusSignalFlags};
use gtk4::glib::{self, Variant, prelude::*};

use crate::model::{AccessPoint, Address, Backend, Device, DeviceState, Kind, Security, State};
use crate::props::{self, Props};

pub const BUS_NAME: &str = "org.freedesktop.NetworkManager";
pub const PATH: &str = "/org/freedesktop/NetworkManager";
pub const OBJECT_MANAGER_PATH: &str = "/org/freedesktop";
pub const SETTINGS_PATH: &str = "/org/freedesktop/NetworkManager/Settings";
const IFACE: &str = "org.freedesktop.NetworkManager";
const DEVICE: &str = "org.freedesktop.NetworkManager.Device";
const WIRED: &str = "org.freedesktop.NetworkManager.Device.Wired";
const WIRELESS: &str = "org.freedesktop.NetworkManager.Device.Wireless";
const ACCESS_POINT: &str = "org.freedesktop.NetworkManager.AccessPoint";
const ACTIVE: &str = "org.freedesktop.NetworkManager.Connection.Active";
const IP4: &str = "org.freedesktop.NetworkManager.IP4Config";
const IP6: &str = "org.freedesktop.NetworkManager.IP6Config";
const SETTINGS_CONNECTION: &str = "org.freedesktop.NetworkManager.Settings.Connection";

/// Wait this long after a signal before reading the state again.
pub const DEBOUNCE: Duration = Duration::from_millis(150);
/// Calls that may wait for a polkit dialog get this long (ms).
const CHANGE_TIMEOUT_MS: i32 = 120_000;
const READ_TIMEOUT_MS: i32 = 10_000;

/// A saved WLAN profile: its object path, name and network name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub path: String,
    pub id: String,
    pub ssid: Vec<u8>,
}

/// `NMDeviceState` → [`DeviceState`].
pub fn device_state(value: u32) -> DeviceState {
    match value {
        20 => DeviceState::Unavailable,
        30 => DeviceState::Disconnected,
        40..=90 => DeviceState::Connecting,
        100 => DeviceState::Connected,
        110 => DeviceState::Disconnecting,
        120 => DeviceState::Failed,
        _ => DeviceState::Unmanaged,
    }
}

/// The profile of `settings` (`GetSettings` reply body `a{sa{sv}}`) if it is
/// a WLAN profile.
pub fn wifi_profile(path: &str, settings: &Variant) -> Option<Profile> {
    let connection = props::interface(settings, "connection");
    if connection.string("type").as_deref() != Some("802-11-wireless") {
        return None;
    }
    let wireless = props::interface(settings, "802-11-wireless");
    Some(Profile {
        path: path.to_string(),
        id: connection.string("id").unwrap_or_default(),
        ssid: wireless.bytes("ssid"),
    })
}

fn addresses(config: &Props, v6: bool) -> Vec<Address> {
    config
        .list("AddressData")
        .iter()
        .filter_map(|data| {
            let ip = props::ip(&data.string("address")?)?;
            let prefix = u8::try_from(data.u32("prefix")?).ok()?;
            match (ip, v6) {
                (std::net::IpAddr::V4(ip), false) => Some(Address::v4(ip, prefix)),
                (std::net::IpAddr::V6(ip), true) => Some(Address::v6(ip, prefix)),
                _ => None,
            }
        })
        .collect()
}

fn name_servers(ip4: &Props, ip6: &Props) -> Vec<std::net::IpAddr> {
    let mut list: Vec<std::net::IpAddr> = ip4
        .list("NameserverData")
        .iter()
        .filter_map(|d| props::ip(&d.string("address")?))
        .collect();
    if list.is_empty() {
        // NetworkManager before 1.14: `au` in network byte order.
        if let Some(v) = ip4.get("Nameservers")
            && v.is_container()
        {
            list.extend(
                v.iter()
                    .filter_map(|n| n.get::<u32>())
                    .map(|n| std::net::IpAddr::from(props::ipv4_from_u32(n))),
            );
        }
    }
    if let Some(v) = ip6.get("Nameservers")
        && v.is_container()
    {
        list.extend(
            v.iter()
                .filter_map(|n| props::ipv6_from_bytes(&props::bytes(&n)))
                .map(std::net::IpAddr::from),
        );
    }
    list
}

/// The [`State`] of a `GetManagedObjects` reply, with `profiles` marking the
/// networks that have a saved connection. Devices other than cable and
/// WLAN, and unmanaged ones, are left out.
pub fn state_from_objects(objects: &[(String, Variant)], profiles: &[Profile]) -> State {
    let by_path: HashMap<&str, &Variant> = objects.iter().map(|(p, v)| (p.as_str(), v)).collect();
    let get = |path: &str, iface: &str| match by_path.get(path) {
        Some(interfaces) => props::interface(interfaces, iface),
        None => Props(None),
    };
    let manager = get(PATH, IFACE);
    let primary_devices = manager
        .path("PrimaryConnection")
        .map(|p| get(&p, ACTIVE).paths("Devices"))
        .unwrap_or_default();

    let mut devices = Vec::new();
    for path in manager.paths("Devices") {
        let device = get(&path, DEVICE);
        let kind = match device.u32("DeviceType") {
            Some(1) => Kind::Wired,
            Some(2) => Kind::Wireless,
            _ => continue,
        };
        let state = device_state(device.u32("State").unwrap_or(0));
        if state == DeviceState::Unmanaged {
            continue;
        }
        let interface = device.string("Interface").unwrap_or_default();
        let mut d = Device::new(&path, &interface, kind);
        d.state = state;
        d.driver = device.string("Driver").filter(|s| !s.is_empty());
        d.primary = primary_devices.contains(&path);
        let specific = match kind {
            Kind::Wired => get(&path, WIRED),
            Kind::Wireless => get(&path, WIRELESS),
        };
        d.mac = device
            .string("HwAddress")
            .or_else(|| specific.string("HwAddress"))
            .unwrap_or_default();
        if let Some(active) = device.path("ActiveConnection") {
            let connection = get(&active, ACTIVE);
            d.connection = connection.string("Id");
            d.active_connection = Some(active);
        }
        let ip4 = device
            .path("Ip4Config")
            .map(|p| get(&p, IP4))
            .unwrap_or_default();
        let ip6 = device
            .path("Ip6Config")
            .map(|p| get(&p, IP6))
            .unwrap_or_default();
        d.ipv4 = addresses(&ip4, false);
        d.ipv6 = addresses(&ip6, true);
        d.gateway4 = ip4.string("Gateway").and_then(|g| g.parse().ok());
        d.gateway6 = ip6
            .string("Gateway")
            .and_then(|g| props::ip(&g))
            .and_then(|ip| match ip {
                std::net::IpAddr::V6(v6) => Some(v6),
                _ => None,
            });
        d.dns = name_servers(&ip4, &ip6);
        match kind {
            Kind::Wired => {
                d.carrier = specific.bool("Carrier");
                d.speed_mbit = specific.u32("Speed").filter(|&s| s > 0);
            }
            Kind::Wireless => {
                d.speed_mbit = specific.u32("Bitrate").map(|k| k / 1000).filter(|&s| s > 0);
                let active_ap = specific.path("ActiveAccessPoint");
                for ap_path in specific.paths("AccessPoints") {
                    let ap = get(&ap_path, ACCESS_POINT);
                    let ssid = ap.bytes("Ssid");
                    let known = profiles
                        .iter()
                        .find(|p| !ssid.is_empty() && p.ssid == ssid)
                        .map(|p| p.path.clone());
                    let entry = AccessPoint {
                        id: ap_path.clone(),
                        ssid: crate::model::ssid_text(&ssid),
                        strength: ap.u8("Strength").unwrap_or(0).min(100),
                        security: Security::from_nm_flags(
                            ap.u32("Flags").unwrap_or(0),
                            ap.u32("WpaFlags").unwrap_or(0),
                            ap.u32("RsnFlags").unwrap_or(0),
                        ),
                        frequency_mhz: ap.u32("Frequency").unwrap_or(0),
                        bssid: ap.string("HwAddress").unwrap_or_default(),
                        active: active_ap.as_deref() == Some(ap_path.as_str()),
                        known,
                    };
                    if entry.active {
                        // A hidden network's access point has no name; the
                        // profile's name stands in below.
                        d.ssid = (!entry.ssid.is_empty()).then(|| entry.ssid.clone());
                        d.signal = Some(entry.strength);
                    }
                    d.access_points.push(entry);
                }
                d.access_points.sort_by(|a, b| b.strength.cmp(&a.strength));
                if d.ssid.is_none() && d.active_connection.is_some() {
                    d.ssid = d.connection.clone();
                }
            }
        }
        devices.push(d);
    }
    State {
        backend: Backend::NetworkManager {
            version: manager.string("Version").unwrap_or_default(),
        },
        devices,
        networking_enabled: manager.bool("NetworkingEnabled").unwrap_or(true),
        wireless_enabled: manager.bool("WirelessEnabled"),
        wireless_hardware_enabled: manager.bool("WirelessHardwareEnabled").unwrap_or(true),
        wwan_enabled: manager.bool("WwanEnabled"),
    }
}

/// The settings for a new WLAN connection to `ssid` (`a{sa{sv}}` for
/// `AddAndActivateConnection`): infrastructure mode, the key for WPA, and
/// `hidden` for a network that does not announce its name. The key is
/// stored by NetworkManager for the system (no keyring needed), as a
/// connection made with `nmcli`.
pub fn new_wifi_settings(
    ssid: &str,
    security: Security,
    password: Option<&str>,
    hidden: bool,
) -> Variant {
    let mut connection: HashMap<String, Variant> = HashMap::new();
    connection.insert("id".into(), ssid.to_variant());
    connection.insert("type".into(), "802-11-wireless".to_variant());
    let mut wireless: HashMap<String, Variant> = HashMap::new();
    wireless.insert("ssid".into(), ssid.as_bytes().to_vec().to_variant());
    wireless.insert("mode".into(), "infrastructure".to_variant());
    if hidden {
        wireless.insert("hidden".into(), true.to_variant());
    }
    let mut all: HashMap<String, HashMap<String, Variant>> = HashMap::new();
    all.insert("connection".into(), connection);
    all.insert("802-11-wireless".into(), wireless);
    if let Some(key_mgmt) = security.key_mgmt() {
        let mut sec: HashMap<String, Variant> = HashMap::new();
        sec.insert("key-mgmt".into(), key_mgmt.to_variant());
        if let Some(password) = password {
            sec.insert("psk".into(), password.to_variant());
        }
        all.insert("802-11-wireless-security".into(), sec);
    }
    all.to_variant()
}

/// `path` as a D-Bus object path (type `o`); an invalid one becomes `/`.
pub fn object_path(path: &str) -> Variant {
    glib::variant::ObjectPath::try_from(path.to_string())
        .or_else(|_| glib::variant::ObjectPath::try_from("/".to_string()))
        .map(|p| p.to_variant())
        .expect("/ is a valid object path")
}

/// Arguments of `ActivateConnection` (`(ooo)`): profile, device, specific
/// object (access point), `/` for none.
pub fn activate_args(profile: &str, device: &str, specific: &str) -> Variant {
    Variant::tuple_from_iter([
        object_path(profile),
        object_path(device),
        object_path(specific),
    ])
}

/// Arguments of `AddAndActivateConnection` (`(a{sa{sv}}oo)`).
pub fn add_and_activate_args(settings: Variant, device: &str, specific: &str) -> Variant {
    Variant::tuple_from_iter([settings, object_path(device), object_path(specific)])
}

/// The reason text of a failed call, without GIO's `GDBus.Error:…` prefix.
pub fn error_text(error: &glib::Error) -> String {
    let message = error.message();
    match message.split_once(": ") {
        Some((prefix, rest)) if prefix.starts_with("GDBus.Error:") => rest.to_string(),
        _ => message.to_string(),
    }
}

type StateFn = Rc<dyn Fn(Result<State, String>)>;

/// A running connection to NetworkManager.
pub struct Nm {
    connection: DBusConnection,
    /// Unique bus name of NetworkManager (e.g. `:1.7`).
    owner: String,
    _signals: gio::SignalSubscription,
    profiles: RefCell<HashMap<String, Option<Profile>>>,
    scheduled: Cell<bool>,
    reading: Cell<bool>,
    again: Cell<bool>,
    stopped: Cell<bool>,
    on_state: StateFn,
}

impl Nm {
    /// Starts reading NetworkManager (owned by `owner` on `connection`);
    /// `on_state` gets every new state, or the error when reading fails.
    pub fn start(
        connection: &DBusConnection,
        owner: &str,
        on_state: impl Fn(Result<State, String>) + 'static,
    ) -> Rc<Nm> {
        Rc::new_cyclic(|weak: &Weak<Nm>| {
            let weak_signal = weak.clone();
            let signals = connection.subscribe_to_signal(
                Some(BUS_NAME),
                None,
                None,
                None,
                None,
                DBusSignalFlags::NONE,
                move |signal| {
                    if let Some(nm) = weak_signal.upgrade() {
                        if signal.interface_name == SETTINGS_CONNECTION
                            && matches!(signal.signal_name, "Updated" | "Removed")
                        {
                            nm.profiles.borrow_mut().remove(signal.object_path);
                        }
                        nm.schedule();
                    }
                },
            );
            let nm = Nm {
                connection: connection.clone(),
                owner: owner.to_string(),
                _signals: signals,
                profiles: RefCell::new(HashMap::new()),
                scheduled: Cell::new(false),
                reading: Cell::new(false),
                again: Cell::new(false),
                stopped: Cell::new(false),
                on_state: Rc::new(on_state),
            };
            let first = weak.clone();
            glib::idle_add_local_once(move || {
                if let Some(nm) = first.upgrade() {
                    nm.read_now();
                }
            });
            nm
        })
    }

    pub fn owner(&self) -> &str {
        &self.owner
    }

    pub fn connection(&self) -> &DBusConnection {
        &self.connection
    }

    /// No more states are reported (NetworkManager went away).
    pub fn stop(&self) {
        self.stopped.set(true);
    }

    /// Reads the state again after [`DEBOUNCE`].
    pub fn schedule(self: &Rc<Self>) {
        if self.scheduled.replace(true) {
            return;
        }
        let weak = Rc::downgrade(self);
        glib::timeout_add_local_once(DEBOUNCE, move || {
            if let Some(nm) = weak.upgrade() {
                nm.scheduled.set(false);
                nm.read_now();
            }
        });
    }

    fn read_now(self: &Rc<Self>) {
        if self.reading.replace(true) {
            self.again.set(true);
            return;
        }
        let nm = self.clone();
        glib::MainContext::default().spawn_local(async move {
            loop {
                nm.again.set(false);
                let result = nm.read().await;
                if nm.stopped.get() {
                    break;
                }
                (nm.on_state)(result);
                if !nm.again.get() {
                    break;
                }
            }
            nm.reading.set(false);
        });
    }

    async fn call(
        &self,
        path: &str,
        iface: &str,
        method: &str,
        args: Option<&Variant>,
        timeout: i32,
    ) -> Result<Variant, String> {
        self.connection
            .call_future(
                Some(BUS_NAME),
                path,
                iface,
                method,
                args,
                None,
                DBusCallFlags::ALLOW_INTERACTIVE_AUTHORIZATION,
                timeout,
            )
            .await
            .map_err(|e| error_text(&e))
    }

    async fn read(&self) -> Result<State, String> {
        let reply = self
            .call(
                OBJECT_MANAGER_PATH,
                "org.freedesktop.DBus.ObjectManager",
                "GetManagedObjects",
                None,
                READ_TIMEOUT_MS,
            )
            .await?;
        let objects = props::objects(&reply);
        if !objects.iter().any(|(p, _)| p == PATH) {
            return Err("NetworkManager meldet kein Managerobjekt".to_string());
        }
        // Saved profiles: read each new one once.
        let paths: Vec<String> = objects
            .iter()
            .filter(|(_, ifaces)| props::interface(ifaces, SETTINGS_CONNECTION).0.is_some())
            .map(|(p, _)| p.clone())
            .collect();
        self.profiles.borrow_mut().retain(|p, _| paths.contains(p));
        for path in &paths {
            if self.profiles.borrow().contains_key(path) {
                continue;
            }
            let profile = match self
                .call(
                    path,
                    SETTINGS_CONNECTION,
                    "GetSettings",
                    None,
                    READ_TIMEOUT_MS,
                )
                .await
            {
                Ok(reply) => wifi_profile(path, &reply.child_value(0)),
                Err(_) => None,
            };
            self.profiles.borrow_mut().insert(path.clone(), profile);
        }
        let profiles: Vec<Profile> = self.profiles.borrow().values().flatten().cloned().collect();
        Ok(state_from_objects(&objects, &profiles))
    }

    /// Connects `device` through access point `ap` (a WLAN with saved
    /// profile `known`, or a new one with `security` and `password`).
    pub async fn connect_wifi(
        &self,
        device: &str,
        ap: &AccessPoint,
        password: Option<&str>,
    ) -> Result<(), String> {
        if let Some(profile) = &ap.known {
            let args = activate_args(profile, device, &ap.id);
            self.call(
                PATH,
                IFACE,
                "ActivateConnection",
                Some(&args),
                CHANGE_TIMEOUT_MS,
            )
            .await
            .map(drop)
        } else {
            let settings = new_wifi_settings(&ap.ssid, ap.security, password, false);
            let args = add_and_activate_args(settings, device, &ap.id);
            self.call(
                PATH,
                IFACE,
                "AddAndActivateConnection",
                Some(&args),
                CHANGE_TIMEOUT_MS,
            )
            .await
            .map(drop)
        }
    }

    /// Connects `device` to the hidden network `ssid`.
    pub async fn connect_hidden(
        &self,
        device: &str,
        ssid: &str,
        security: Security,
        password: Option<&str>,
    ) -> Result<(), String> {
        let settings = new_wifi_settings(ssid, security, password, true);
        let args = add_and_activate_args(settings, device, "/");
        self.call(
            PATH,
            IFACE,
            "AddAndActivateConnection",
            Some(&args),
            CHANGE_TIMEOUT_MS,
        )
        .await
        .map(drop)
    }

    /// Activates the best profile NetworkManager has for `device` (cable).
    pub async fn connect_device(&self, device: &str) -> Result<(), String> {
        let args = activate_args("/", device, "/");
        self.call(
            PATH,
            IFACE,
            "ActivateConnection",
            Some(&args),
            CHANGE_TIMEOUT_MS,
        )
        .await
        .map(drop)
    }

    pub async fn disconnect(&self, device: &str) -> Result<(), String> {
        self.call(device, DEVICE, "Disconnect", None, CHANGE_TIMEOUT_MS)
            .await
            .map(drop)
    }

    async fn set_property(&self, name: &str, value: bool) -> Result<(), String> {
        let args = (IFACE, name, value.to_variant()).to_variant();
        self.call(
            PATH,
            "org.freedesktop.DBus.Properties",
            "Set",
            Some(&args),
            CHANGE_TIMEOUT_MS,
        )
        .await
        .map(drop)
    }

    pub async fn set_wireless(&self, enabled: bool) -> Result<(), String> {
        self.set_property("WirelessEnabled", enabled).await
    }

    /// Flugmodus: WLAN and mobile broadband off (or both on again).
    pub async fn set_airplane(&self, on: bool) -> Result<(), String> {
        self.set_property("WirelessEnabled", !on).await?;
        self.set_property("WwanEnabled", !on).await
    }

    /// Asks `device` (WLAN) to look for networks; NetworkManager reports
    /// new access points by signal. Errors (scan running, not allowed)
    /// are only logged by the caller.
    pub async fn request_scan(&self, device: &str) -> Result<(), String> {
        let options: HashMap<String, Variant> = HashMap::new();
        let args = (options,).to_variant();
        self.call(
            device,
            WIRELESS,
            "RequestScan",
            Some(&args),
            READ_TIMEOUT_MS,
        )
        .await
        .map(drop)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    type Ifaces = HashMap<String, HashMap<String, Variant>>;

    fn obj(path: &str, iface: &str, props: Vec<(&str, Variant)>) -> (String, Ifaces) {
        let mut map = HashMap::new();
        map.insert(
            iface.to_string(),
            props
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect::<HashMap<_, _>>(),
        );
        (path.to_string(), map)
    }

    fn merge(list: Vec<(String, Ifaces)>) -> Vec<(String, Variant)> {
        let mut all: HashMap<String, Ifaces> = HashMap::new();
        for (path, ifaces) in list {
            all.entry(path).or_default().extend(ifaces);
        }
        let dict = all.to_variant();
        props::objects(&dict)
    }

    fn paths(list: &[&str]) -> Variant {
        list.iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
            .to_variant()
    }

    fn address(ip: &str, prefix: u32) -> HashMap<String, Variant> {
        HashMap::from([
            ("address".to_string(), ip.to_variant()),
            ("prefix".to_string(), prefix.to_variant()),
        ])
    }

    /// Manager with a cable device (connected, primary) and a WLAN device
    /// (disconnected) seeing three networks, one of them saved.
    pub(crate) fn sample() -> Vec<(String, Variant)> {
        merge(vec![
            obj(
                PATH,
                IFACE,
                vec![
                    ("Devices", paths(&["/d/1", "/d/2", "/d/lo"])),
                    ("PrimaryConnection", "/a/1".to_variant()),
                    ("NetworkingEnabled", true.to_variant()),
                    ("WirelessEnabled", true.to_variant()),
                    ("WirelessHardwareEnabled", true.to_variant()),
                    ("WwanEnabled", true.to_variant()),
                    ("Version", "1.54.0".to_variant()),
                ],
            ),
            obj(
                "/d/1",
                DEVICE,
                vec![
                    ("DeviceType", 1u32.to_variant()),
                    ("Interface", "eth0".to_variant()),
                    ("State", 100u32.to_variant()),
                    ("Driver", "e1000e".to_variant()),
                    ("HwAddress", "52:54:00:12:34:56".to_variant()),
                    ("ActiveConnection", "/a/1".to_variant()),
                    ("Ip4Config", "/ip4/1".to_variant()),
                    ("Ip6Config", "/ip6/1".to_variant()),
                ],
            ),
            obj(
                "/d/1",
                WIRED,
                vec![
                    ("Carrier", true.to_variant()),
                    ("Speed", 1000u32.to_variant()),
                ],
            ),
            obj(
                "/d/2",
                DEVICE,
                vec![
                    ("DeviceType", 2u32.to_variant()),
                    ("Interface", "wlan0".to_variant()),
                    ("State", 30u32.to_variant()),
                    ("ActiveConnection", "/".to_variant()),
                    ("Ip4Config", "/".to_variant()),
                ],
            ),
            obj(
                "/d/2",
                WIRELESS,
                vec![
                    ("HwAddress", "52:54:00:ab:cd:ef".to_variant()),
                    ("AccessPoints", paths(&["/ap/1", "/ap/2", "/ap/3"])),
                    ("ActiveAccessPoint", "/".to_variant()),
                    ("Bitrate", 0u32.to_variant()),
                ],
            ),
            obj(
                "/d/lo",
                DEVICE,
                vec![
                    ("DeviceType", 32u32.to_variant()),
                    ("Interface", "lo".to_variant()),
                    ("State", 100u32.to_variant()),
                ],
            ),
            obj(
                "/a/1",
                ACTIVE,
                vec![
                    ("Id", "Kabelgebundene Verbindung 1".to_variant()),
                    ("Devices", paths(&["/d/1"])),
                    ("State", 2u32.to_variant()),
                ],
            ),
            obj(
                "/ip4/1",
                IP4,
                vec![
                    (
                        "AddressData",
                        vec![address("198.51.100.23", 24)].to_variant(),
                    ),
                    ("Gateway", "198.51.100.1".to_variant()),
                    (
                        "NameserverData",
                        vec![HashMap::from([(
                            "address".to_string(),
                            "198.51.100.1".to_variant(),
                        )])]
                        .to_variant(),
                    ),
                ],
            ),
            obj(
                "/ip6/1",
                IP6,
                vec![
                    (
                        "AddressData",
                        vec![address("fe80::5054:ff:fe12:3456", 64)].to_variant(),
                    ),
                    ("Gateway", "".to_variant()),
                ],
            ),
            obj(
                "/ap/1",
                ACCESS_POINT,
                vec![
                    ("Ssid", b"Heimnetz".to_vec().to_variant()),
                    ("Strength", 82u8.to_variant()),
                    ("Flags", 1u32.to_variant()),
                    ("RsnFlags", 0x188u32.to_variant()),
                    ("WpaFlags", 0u32.to_variant()),
                    ("Frequency", 2437u32.to_variant()),
                ],
            ),
            obj(
                "/ap/2",
                ACCESS_POINT,
                vec![
                    ("Ssid", b"Cafe offen".to_vec().to_variant()),
                    ("Strength", 45u8.to_variant()),
                    ("Flags", 0u32.to_variant()),
                    ("Frequency", 5180u32.to_variant()),
                ],
            ),
            obj(
                "/ap/3",
                ACCESS_POINT,
                vec![
                    ("Ssid", b"Nachbar".to_vec().to_variant()),
                    ("Strength", 20u8.to_variant()),
                    ("Flags", 1u32.to_variant()),
                    ("RsnFlags", 0x400u32.to_variant()),
                ],
            ),
        ])
    }

    #[test]
    fn device_states_from_networkmanager() {
        use DeviceState::*;
        let map: Vec<DeviceState> = [0, 10, 20, 30, 40, 60, 90, 100, 110, 120]
            .into_iter()
            .map(device_state)
            .collect();
        assert_eq!(
            map,
            [
                Unmanaged,
                Unmanaged,
                Unavailable,
                Disconnected,
                Connecting,
                Connecting,
                Connecting,
                Connected,
                Disconnecting,
                Failed
            ]
        );
    }

    #[test]
    fn state_from_managed_objects() {
        let profiles = [Profile {
            path: "/s/7".into(),
            id: "Heimnetz".into(),
            ssid: b"Heimnetz".to_vec(),
        }];
        let state = state_from_objects(&sample(), &profiles);
        assert_eq!(
            state.backend,
            Backend::NetworkManager {
                version: "1.54.0".into()
            }
        );
        assert_eq!(state.devices.len(), 2, "lo bleibt draußen");
        let eth = &state.devices[0];
        assert_eq!(eth.interface, "eth0");
        assert_eq!(eth.state, DeviceState::Connected);
        assert!(eth.primary);
        assert_eq!(eth.carrier, Some(true));
        assert_eq!(eth.speed_mbit, Some(1000));
        assert_eq!(eth.driver.as_deref(), Some("e1000e"));
        assert_eq!(eth.mac, "52:54:00:12:34:56");
        assert_eq!(
            eth.connection.as_deref(),
            Some("Kabelgebundene Verbindung 1")
        );
        assert_eq!(eth.active_connection.as_deref(), Some("/a/1"));
        assert_eq!(eth.ipv4[0].to_string(), "198.51.100.23/24");
        assert_eq!(eth.ipv6[0].to_string(), "fe80::5054:ff:fe12:3456/64");
        assert_eq!(eth.gateway4, Some("198.51.100.1".parse().unwrap()));
        assert_eq!(eth.gateway6, None);
        assert_eq!(
            eth.dns,
            ["198.51.100.1".parse::<std::net::IpAddr>().unwrap()]
        );
        let wlan = &state.devices[1];
        assert_eq!(wlan.kind, Kind::Wireless);
        assert_eq!(wlan.mac, "52:54:00:ab:cd:ef", "aus Device.Wireless");
        assert_eq!(wlan.state, DeviceState::Disconnected);
        assert!(wlan.ipv4.is_empty() && wlan.connection.is_none());
        let aps: Vec<(&str, u8, Security, bool)> = wlan
            .access_points
            .iter()
            .map(|a| (a.ssid.as_str(), a.strength, a.security, a.known.is_some()))
            .collect();
        assert_eq!(
            aps,
            [
                ("Heimnetz", 82, Security::WpaPsk, true),
                ("Cafe offen", 45, Security::Open, false),
                ("Nachbar", 20, Security::Sae, false)
            ]
        );
        assert_eq!(state.wireless_enabled, Some(true));
        assert_eq!(state.airplane(), Some(false));
        assert_eq!(
            crate::model::icon(&state),
            crate::model::Icon::WiredConnected
        );
    }

    #[test]
    fn empty_or_broken_replies_give_an_empty_state() {
        let state = state_from_objects(&[], &[]);
        assert!(state.devices.is_empty());
        assert!(state.networking_enabled);
        let broken = props::objects(&5u32.to_variant());
        assert!(broken.is_empty());
    }

    #[test]
    fn wifi_profiles_from_settings() {
        let mut connection: HashMap<String, Variant> = HashMap::new();
        connection.insert("id".into(), "Heim".to_variant());
        connection.insert("type".into(), "802-11-wireless".to_variant());
        let mut wireless: HashMap<String, Variant> = HashMap::new();
        wireless.insert("ssid".into(), b"Heimnetz".to_vec().to_variant());
        let settings: HashMap<String, HashMap<String, Variant>> = HashMap::from([
            ("connection".to_string(), connection.clone()),
            ("802-11-wireless".to_string(), wireless),
        ]);
        let profile = wifi_profile("/s/1", &settings.to_variant()).unwrap();
        assert_eq!(profile.id, "Heim");
        assert_eq!(profile.ssid, b"Heimnetz");
        connection.insert("type".into(), "802-3-ethernet".to_variant());
        let wired: HashMap<String, HashMap<String, Variant>> =
            HashMap::from([("connection".to_string(), connection)]);
        assert!(wifi_profile("/s/2", &wired.to_variant()).is_none());
    }

    #[test]
    fn new_wifi_settings_have_ssid_mode_and_key() {
        let v = new_wifi_settings("Heimnetz", Security::WpaPsk, Some("geheim123"), false);
        assert_eq!(v.type_().as_str(), "a{sa{sv}}");
        let wireless = props::interface(&v, "802-11-wireless");
        assert_eq!(wireless.bytes("ssid"), b"Heimnetz");
        assert_eq!(wireless.string("mode").as_deref(), Some("infrastructure"));
        assert_eq!(wireless.bool("hidden"), None);
        let sec = props::interface(&v, "802-11-wireless-security");
        assert_eq!(sec.string("key-mgmt").as_deref(), Some("wpa-psk"));
        assert_eq!(sec.string("psk").as_deref(), Some("geheim123"));
        let open = new_wifi_settings("Cafe", Security::Open, None, true);
        assert!(
            props::interface(&open, "802-11-wireless-security")
                .0
                .is_none()
        );
        assert_eq!(
            props::interface(&open, "802-11-wireless").bool("hidden"),
            Some(true)
        );
        let sae = new_wifi_settings("X", Security::Sae, None, false);
        assert_eq!(
            props::interface(&sae, "802-11-wireless-security")
                .string("key-mgmt")
                .as_deref(),
            Some("sae")
        );
    }

    #[test]
    fn call_arguments_have_the_signatures_networkmanager_expects() {
        assert_eq!(
            activate_args("/s/1", "/d/2", "/ap/3").type_().as_str(),
            "(ooo)"
        );
        assert_eq!(
            activate_args("/", "/d/2", "kaputt").child_value(2).str(),
            Some("/")
        );
        let settings = new_wifi_settings("X", Security::Open, None, false);
        assert_eq!(
            add_and_activate_args(settings, "/d/2", "/")
                .type_()
                .as_str(),
            "(a{sa{sv}}oo)"
        );
        // the property setter and the scan
        assert_eq!(
            (IFACE, "WirelessEnabled", true.to_variant())
                .to_variant()
                .type_()
                .as_str(),
            "(ssv)"
        );
        let options: HashMap<String, Variant> = HashMap::new();
        assert_eq!((options,).to_variant().type_().as_str(), "(a{sv})");
    }

    #[test]
    fn error_text_drops_the_gdbus_prefix() {
        let e = glib::Error::new(
            gio::IOErrorEnum::Failed,
            "GDBus.Error:org.freedesktop.NetworkManager.PermissionDenied: Nicht erlaubt",
        );
        assert_eq!(error_text(&e), "Nicht erlaubt");
        let e = glib::Error::new(gio::IOErrorEnum::Failed, "Zeitüberschreitung");
        assert_eq!(error_text(&e), "Zeitüberschreitung");
    }
}
