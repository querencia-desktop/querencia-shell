//! What the network looks like, as plain data, and the rules the applet
//! follows: which icon, which tooltip, how addresses, speeds and signal
//! strengths are written. No GTK and no D-Bus here, so all of it is tested
//! without NetworkManager.
//!
//! Both backends fill the same [`State`]: NetworkManager (`nm.rs`, can
//! connect and disconnect) and the read-only fallback (`kernel.rs`, link
//! state, addresses and routes from the kernel, plus what systemd-networkd
//! and systemd-resolved say).

use crate::i18n::{gettext, gettext_f};
use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Where the state comes from.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Backend {
    /// Just started; nothing read yet.
    #[default]
    Starting,
    /// NetworkManager on the system bus: shows and changes connections.
    NetworkManager { version: String },
    /// No NetworkManager: read only, from the kernel. `networkd` tells
    /// whether systemd-networkd runs (it then configures the links).
    Fallback { networkd: bool },
    /// Neither: the kernel's network data cannot be read either.
    Unavailable(String),
}

impl Backend {
    /// Whether the user can change anything (connect, switch WLAN).
    pub fn can_control(&self) -> bool {
        matches!(self, Backend::NetworkManager { .. })
    }

    /// The line the menu and the tooltip show for a read-only backend.
    pub fn read_only_note(&self) -> Option<String> {
        match self {
            Backend::Fallback { networkd: true } => {
                Some(gettext("Managed by systemd-networkd, display only"))
            }
            Backend::Fallback { networkd: false } => {
                Some(gettext("No NetworkManager found, display only"))
            }
            _ => None,
        }
    }
}

/// Kind of a network device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Wired,
    Wireless,
}

/// State of a device, after NetworkManager's `NMDeviceState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceState {
    /// Not managed (NetworkManager leaves it alone).
    Unmanaged,
    /// Cannot be used now: no cable, WLAN switched off, …
    Unavailable,
    /// Usable, but no connection.
    Disconnected,
    /// Preparing, authenticating, getting addresses.
    Connecting,
    Connected,
    Disconnecting,
    Failed,
}

/// An address with its prefix length, written as `192.0.2.23/24`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Address {
    pub ip: IpAddr,
    pub prefix: u8,
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.ip, self.prefix)
    }
}

impl Address {
    pub fn v4(ip: Ipv4Addr, prefix: u8) -> Address {
        Address {
            ip: IpAddr::V4(ip),
            prefix,
        }
    }

    pub fn v6(ip: Ipv6Addr, prefix: u8) -> Address {
        Address {
            ip: IpAddr::V6(ip),
            prefix,
        }
    }

    /// Whether the address reaches beyond the link (not loopback, not
    /// IPv4 169.254/16, not IPv6 fe80::/10).
    pub fn is_global(&self) -> bool {
        match self.ip {
            IpAddr::V4(ip) => !ip.is_loopback() && !ip.is_link_local() && !ip.is_unspecified(),
            IpAddr::V6(ip) => {
                !ip.is_loopback() && !ip.is_unspecified() && (ip.segments()[0] & 0xffc0) != 0xfe80
            }
        }
    }

    /// The IPv4 netmask, e.g. `255.255.255.0` for /24.
    pub fn netmask(&self) -> Option<Ipv4Addr> {
        match self.ip {
            IpAddr::V4(_) if self.prefix <= 32 => Some(Ipv4Addr::from(
                u32::MAX
                    .checked_shl(32 - u32::from(self.prefix))
                    .unwrap_or(0),
            )),
            _ => None,
        }
    }
}

/// How a WLAN is secured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Security {
    Open,
    /// WEP: old, not offered for new connections.
    Wep,
    /// WPA/WPA2 Personal (pre-shared key).
    WpaPsk,
    /// WPA3 Personal (SAE).
    Sae,
    /// WPA Enterprise (802.1X): needs settings this applet does not have.
    Enterprise,
}

impl Security {
    /// Whether a password is needed (shown with a lock).
    pub fn secured(self) -> bool {
        self != Security::Open
    }

    /// Whether this applet can connect to a new network of this kind.
    pub fn supported(self) -> bool {
        matches!(self, Security::Open | Security::WpaPsk | Security::Sae)
    }

    /// NetworkManager's `802-11-wireless-security.key-mgmt`, if any.
    pub fn key_mgmt(self) -> Option<&'static str> {
        match self {
            Security::WpaPsk => Some("wpa-psk"),
            Security::Sae => Some("sae"),
            _ => None,
        }
    }

    pub fn label(self) -> String {
        match self {
            // TRANSLATORS: security of a wireless network
            Security::Open => gettext("none"),
            Security::Wep => "WEP".to_string(),
            Security::WpaPsk => gettext("WPA/WPA2 Personal"),
            Security::Sae => gettext("WPA3 Personal"),
            Security::Enterprise => gettext("WPA/WPA2 Enterprise"),
        }
    }

    /// From an access point's `Flags`, `WpaFlags` and `RsnFlags`
    /// (`NM80211ApFlags`, `NM80211ApSecurityFlags`).
    pub fn from_nm_flags(flags: u32, wpa: u32, rsn: u32) -> Security {
        const PRIVACY: u32 = 0x1;
        const KEY_MGMT_PSK: u32 = 0x100;
        const KEY_MGMT_802_1X: u32 = 0x200;
        const KEY_MGMT_SAE: u32 = 0x400;
        const KEY_MGMT_EAP_SUITE_B_192: u32 = 0x2000;
        let both = wpa | rsn;
        if both & (KEY_MGMT_802_1X | KEY_MGMT_EAP_SUITE_B_192) != 0 {
            Security::Enterprise
        } else if both & KEY_MGMT_PSK != 0 {
            // WPA2/WPA3 transition networks take both; WPA2 works for every
            // device.
            Security::WpaPsk
        } else if both & KEY_MGMT_SAE != 0 {
            Security::Sae
        } else if flags & PRIVACY != 0 {
            Security::Wep
        } else {
            Security::Open
        }
    }
}

/// One WLAN as an access point reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessPoint {
    /// Backend's key (NetworkManager: the object path).
    pub id: String,
    pub ssid: String,
    /// Signal strength in percent.
    pub strength: u8,
    pub security: Security,
    pub frequency_mhz: u32,
    /// Hardware address of the access point (BSSID).
    pub bssid: String,
    /// The device is connected (or connecting) through it.
    pub active: bool,
    /// There is a saved connection for this network (NetworkManager:
    /// object path of the profile).
    pub known: Option<String>,
}

/// One network device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    /// Backend's key (NetworkManager: the object path; fallback: the
    /// interface name).
    pub id: String,
    pub interface: String,
    pub kind: Kind,
    pub state: DeviceState,
    /// Hardware (MAC) address, `00:00:5e:00:53:df`.
    pub mac: String,
    pub driver: Option<String>,
    /// Cable plugged in (wired devices).
    pub carrier: Option<bool>,
    /// Link speed in Mbit/s (wired) or bit rate (WLAN).
    pub speed_mbit: Option<u32>,
    /// Name of the active connection (profile).
    pub connection: Option<String>,
    /// Backend's key of the active connection (to disconnect).
    pub active_connection: Option<String>,
    pub ipv4: Vec<Address>,
    pub ipv6: Vec<Address>,
    pub gateway4: Option<Ipv4Addr>,
    pub gateway6: Option<Ipv6Addr>,
    pub dns: Vec<IpAddr>,
    /// The device holds the default route (NetworkManager: the primary
    /// connection).
    pub primary: bool,
    /// WLAN: networks in reach, strongest first.
    pub access_points: Vec<AccessPoint>,
    /// WLAN: the connected network.
    pub ssid: Option<String>,
    /// WLAN: signal of the connected network in percent.
    pub signal: Option<u8>,
    /// Fallback: how systemd-networkd sees the link (`configured`,
    /// `configuring`, `unmanaged`, …).
    pub networkd_state: Option<String>,
}

impl Device {
    /// A device of `kind` named `interface` with nothing known yet.
    pub fn new(id: &str, interface: &str, kind: Kind) -> Device {
        Device {
            id: id.to_string(),
            interface: interface.to_string(),
            kind,
            state: DeviceState::Disconnected,
            mac: String::new(),
            driver: None,
            carrier: None,
            speed_mbit: None,
            connection: None,
            active_connection: None,
            ipv4: Vec::new(),
            ipv6: Vec::new(),
            gateway4: None,
            gateway6: None,
            dns: Vec::new(),
            primary: false,
            access_points: Vec::new(),
            ssid: None,
            signal: None,
            networkd_state: None,
        }
    }

    pub fn connected(&self) -> bool {
        self.state == DeviceState::Connected
    }

    /// The address shown first: the first global IPv4 address, else the
    /// first global IPv6 address.
    pub fn main_address(&self) -> Option<&Address> {
        self.ipv4
            .iter()
            .find(|a| a.is_global())
            .or_else(|| self.ipv6.iter().find(|a| a.is_global()))
    }

    /// What the device is called in the menu: „Kabelnetzwerk (eth0)“ or
    /// „WLAN (wlan0)“.
    pub fn title(&self) -> String {
        match self.kind {
            Kind::Wired => gettext_f(
                "Wired Network ({interface})",
                &[("interface", &self.interface)],
            ),
            Kind::Wireless => {
                gettext_f("Wireless ({interface})", &[("interface", &self.interface)])
            }
        }
    }

    /// A short state text: „verbunden“, „Kabel nicht angeschlossen“, …
    pub fn state_text(&self) -> String {
        match (self.state, self.kind) {
            (DeviceState::Connected, _) => gettext("connected"),
            (DeviceState::Connecting, _) => gettext("connecting…"),
            (DeviceState::Disconnecting, _) => gettext("disconnecting…"),
            (DeviceState::Failed, _) => gettext("Connection failed"),
            (DeviceState::Unmanaged, _) => gettext("not managed"),
            (DeviceState::Unavailable, Kind::Wired) if self.carrier != Some(true) => {
                gettext("Cable unplugged")
            }
            (DeviceState::Unavailable, _) => gettext("not available"),
            (DeviceState::Disconnected, _) => gettext("disconnected"),
        }
    }
}

/// The whole network as one backend reports it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct State {
    pub backend: Backend,
    /// Wired and WLAN devices; others are left out.
    pub devices: Vec<Device>,
    /// NetworkManager: networking switched on at all.
    pub networking_enabled: bool,
    /// WLAN switched on (in software); `None`: unknown or not switchable.
    pub wireless_enabled: Option<bool>,
    /// WLAN radio not blocked by a hardware switch.
    pub wireless_hardware_enabled: bool,
    /// Mobile broadband switched on; `None`: unknown.
    pub wwan_enabled: Option<bool>,
}

impl State {
    pub fn starting() -> State {
        State {
            networking_enabled: true,
            wireless_hardware_enabled: true,
            ..State::default()
        }
    }

    pub fn unavailable(reason: &str) -> State {
        State {
            backend: Backend::Unavailable(reason.to_string()),
            ..State::starting()
        }
    }

    pub fn has_wireless(&self) -> bool {
        self.devices.iter().any(|d| d.kind == Kind::Wireless)
    }

    pub fn has_wired(&self) -> bool {
        self.devices.iter().any(|d| d.kind == Kind::Wired)
    }

    /// Flugmodus (airplane mode): every radio off. NetworkManager has no
    /// such switch; it is WLAN and mobile broadband both switched off, as
    /// the applet sets it. `None` when it cannot be switched (fallback).
    pub fn airplane(&self) -> Option<bool> {
        if !self.backend.can_control() {
            return None;
        }
        Some(self.wireless_enabled == Some(false) && self.wwan_enabled != Some(true))
    }

    /// The device that carries the traffic: the primary one, else the first
    /// connected one.
    pub fn primary(&self) -> Option<&Device> {
        self.devices
            .iter()
            .find(|d| d.primary && d.connected())
            .or_else(|| self.devices.iter().find(|d| d.connected()))
    }
}

/// Signal stage of a WLAN (four stages, 0 = weak … 3 = excellent), as the
/// icons show it: up to 25 %, up to 50 %, up to 75 %, above.
pub fn signal_stage(strength: u8) -> u8 {
    match strength {
        0..=25 => 0,
        26..=50 => 1,
        51..=75 => 2,
        _ => 3,
    }
}

/// The icon of the applet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Icon {
    WiredConnected,
    WiredDisconnected,
    /// Connected through WLAN, with the signal stage 0–3.
    Wireless(u8),
    WirelessOff,
    Connecting,
    /// Nothing connected.
    NoNetwork,
    /// No network service and no data from the kernel.
    NoService,
}

impl Icon {
    pub const ALL: [Icon; 10] = [
        Icon::WiredConnected,
        Icon::WiredDisconnected,
        Icon::Wireless(0),
        Icon::Wireless(1),
        Icon::Wireless(2),
        Icon::Wireless(3),
        Icon::WirelessOff,
        Icon::Connecting,
        Icon::NoNetwork,
        Icon::NoService,
    ];
}

/// The icon for `state`:
///
/// 1. no data at all: [`Icon::NoService`];
/// 2. a device connecting: [`Icon::Connecting`];
/// 3. connected: by the primary device, cable or WLAN with its signal;
/// 4. nothing connected, WLAN on: [`Icon::NoNetwork`] (networks to choose
///    from in the menu);
/// 5. a cable device without cable: [`Icon::WiredDisconnected`];
/// 6. a WLAN device switched off (or Flugmodus): [`Icon::WirelessOff`];
/// 7. otherwise [`Icon::NoNetwork`].
pub fn icon(state: &State) -> Icon {
    match state.backend {
        Backend::Unavailable(_) => return Icon::NoService,
        Backend::Starting => return Icon::NoNetwork,
        _ => {}
    }
    if state
        .devices
        .iter()
        .any(|d| d.state == DeviceState::Connecting)
    {
        return Icon::Connecting;
    }
    if let Some(device) = state.primary() {
        return match device.kind {
            Kind::Wired => Icon::WiredConnected,
            Kind::Wireless => Icon::Wireless(signal_stage(device.signal.unwrap_or(0))),
        };
    }
    let wireless_on = state.wireless_enabled != Some(false) && state.wireless_hardware_enabled;
    if state.has_wireless() && wireless_on && state.networking_enabled {
        return Icon::NoNetwork;
    }
    if state
        .devices
        .iter()
        .any(|d| d.kind == Kind::Wired && d.carrier != Some(true))
    {
        return Icon::WiredDisconnected;
    }
    if state.has_wireless() {
        return Icon::WirelessOff;
    }
    Icon::NoNetwork
}

/// The tooltip of the applet: what is connected and its address, the
/// read-only note of the fallback.
pub fn tooltip(state: &State) -> String {
    let mut lines = Vec::new();
    match &state.backend {
        Backend::Unavailable(reason) => {
            lines.push(gettext("No network service"));
            if !reason.is_empty() {
                lines.push(reason.clone());
            }
            return lines.join("\n");
        }
        Backend::Starting => return gettext("Network: reading…"),
        _ => {}
    }
    let connected: Vec<&Device> = {
        let mut list: Vec<&Device> = state.devices.iter().filter(|d| d.connected()).collect();
        list.sort_by_key(|d| !d.primary);
        list
    };
    if let Some(device) = state
        .devices
        .iter()
        .find(|d| d.state == DeviceState::Connecting)
    {
        lines.push(match (&device.kind, &device.ssid) {
            (Kind::Wireless, Some(ssid)) => gettext_f(
                "Connecting to the wireless network “{ssid}”…",
                &[("ssid", ssid)],
            ),
            _ => gettext_f("{device}: connecting…", &[("device", &device.title())]),
        });
    }
    for device in &connected {
        lines.push(match device.kind {
            Kind::Wired => match &device.connection {
                Some(name) => gettext_f(
                    "Wired network “{name}” connected ({interface})",
                    &[("name", name), ("interface", &device.interface)],
                ),
                None => gettext_f(
                    "Wired network {interface} connected",
                    &[("interface", &device.interface)],
                ),
            },
            Kind::Wireless => {
                let name = device.ssid.as_deref().or(device.connection.as_deref());
                let signal = device.signal.map(|s| s.to_string());
                let interface = device.interface.as_str();
                match (name, signal.as_deref()) {
                    (Some(name), Some(signal)) => gettext_f(
                        "Wireless network “{name}” connected, signal {signal} %",
                        &[("name", name), ("signal", signal)],
                    ),
                    (None, Some(signal)) => gettext_f(
                        "Wireless network connected, signal {signal} %",
                        &[("signal", signal)],
                    ),
                    (Some(name), None) => gettext_f(
                        "Wireless network “{name}” connected ({interface})",
                        &[("name", name), ("interface", interface)],
                    ),
                    (None, None) => gettext_f(
                        "Wireless network connected ({interface})",
                        &[("interface", interface)],
                    ),
                }
            }
        });
        if let Some(address) = device.main_address() {
            lines.push(gettext_f(
                "Address: {address}",
                &[("address", &address.to_string())],
            ));
        }
    }
    if connected.is_empty() && lines.is_empty() {
        lines.push(match icon(state) {
            Icon::WiredDisconnected => gettext("No network cable plugged in"),
            Icon::WirelessOff if state.airplane() == Some(true) => gettext("Airplane mode is on"),
            Icon::WirelessOff => gettext("Wireless is disabled"),
            _ if !state.networking_enabled => gettext("Networking is disabled"),
            _ if state.devices.is_empty() => gettext("No network device"),
            _ => gettext("No network connection"),
        });
    }
    if let Some(note) = state.backend.read_only_note() {
        lines.push(note);
    }
    lines.join("\n")
}

/// A link speed in Mbit/s as the info dialog writes it: „1000 Mbit/s“,
/// „2,5 Gbit/s“ from 2500 on (as in GNOME 2, which wrote Mb/s).
pub fn speed_text(mbit: u32) -> String {
    if mbit >= 2500 && mbit.is_multiple_of(100) {
        let tenths = mbit / 100;
        if tenths.is_multiple_of(10) {
            gettext_f("{gbit} Gbit/s", &[("gbit", &(tenths / 10).to_string())])
        } else {
            // TRANSLATORS: a speed with one decimal; German writes a comma
            gettext_f(
                "{whole}.{tenth} Gbit/s",
                &[
                    ("whole", &(tenths / 10).to_string()),
                    ("tenth", &(tenths % 10).to_string()),
                ],
            )
        }
    } else {
        gettext_f("{mbit} Mbit/s", &[("mbit", &mbit.to_string())])
    }
}

/// A frequency in MHz as „2,4 GHz“ / „5 GHz“ / „6 GHz“ band.
pub fn band_text(mhz: u32) -> String {
    match mhz {
        // TRANSLATORS: a frequency band; German writes a decimal comma
        2400..=2500 => gettext("2.4 GHz"),
        4900..=5900 => "5 GHz".to_string(),
        5925..=7125 => "6 GHz".to_string(),
        _ => String::new(),
    }
}

/// A hardware address in the usual form: lower case, colons. Leaves
/// anything that is not six or more hex bytes as it is.
pub fn mac_text(mac: &str) -> String {
    let lower = mac.trim().to_ascii_lowercase();
    let parts: Vec<&str> = lower.split([':', '-']).collect();
    if parts.len() >= 6
        && parts
            .iter()
            .all(|p| p.len() == 2 && p.chars().all(|c| c.is_ascii_hexdigit()))
    {
        parts.join(":")
    } else {
        lower
    }
}

/// SSID bytes as text: UTF-8 if valid, else Latin-1, as nm-applet does.
pub fn ssid_text(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_string(),
        Err(_) => bytes.iter().map(|&b| char::from(b)).collect(),
    }
}

/// Whether `password` is usable as WPA key: 8 to 63 characters, or 64 hex
/// digits.
pub fn valid_wpa_key(password: &str) -> bool {
    let n = password.chars().count();
    (8..=63).contains(&n) || (n == 64 && password.chars().all(|c| c.is_ascii_hexdigit()))
}

/// The WLAN list of the menu: one row per network name (the strongest
/// access point), the connected one first, then by signal, hidden networks
/// (empty name) left out.
pub fn networks(device: &Device) -> Vec<AccessPoint> {
    let mut best: Vec<AccessPoint> = Vec::new();
    for ap in &device.access_points {
        if ap.ssid.is_empty() {
            continue;
        }
        match best.iter_mut().find(|b| b.ssid == ap.ssid) {
            Some(b) => {
                let active = b.active || ap.active;
                let known = b.known.clone().or_else(|| ap.known.clone());
                if ap.active || (!b.active && ap.strength > b.strength) {
                    *b = ap.clone();
                }
                b.active = active;
                b.known = known;
            }
            None => best.push(ap.clone()),
        }
    }
    best.sort_by(|a, b| {
        b.active
            .cmp(&a.active)
            .then(b.strength.cmp(&a.strength))
            .then_with(|| a.ssid.to_lowercase().cmp(&b.ssid.to_lowercase()))
    });
    best
}

/// One entry of the connection information: label and value.
pub type InfoRow = (String, String);

/// The connection information of `device` (dialog „Verbindungsinformationen“),
/// grouped: general, IPv4, IPv6.
pub fn info(device: &Device, backend: &Backend) -> Vec<(String, Vec<InfoRow>)> {
    let mut general: Vec<InfoRow> = vec![
        (
            gettext("Interface"),
            match device.kind {
                Kind::Wired => {
                    gettext_f("{interface} (wired)", &[("interface", &device.interface)])
                }
                Kind::Wireless => gettext_f(
                    "{interface} (wireless)",
                    &[("interface", &device.interface)],
                ),
            },
        ),
        (gettext("State"), device.state_text()),
    ];
    if let Some(name) = &device.connection {
        general.push((gettext("Connection"), name.clone()));
    }
    if !device.mac.is_empty() {
        general.push((gettext("Hardware Address"), mac_text(&device.mac)));
    }
    if let Some(driver) = &device.driver {
        general.push((gettext("Driver"), driver.clone()));
    }
    if let Some(speed) = device.speed_mbit.filter(|&s| s > 0) {
        general.push((gettext("Speed"), speed_text(speed)));
    }
    if device.kind == Kind::Wireless {
        if let Some(ssid) = &device.ssid {
            general.push((gettext("Network"), ssid.clone()));
        }
        if let Some(ap) = device.access_points.iter().find(|ap| ap.active) {
            general.push((gettext("Security"), ap.security.label()));
            let band = band_text(ap.frequency_mhz);
            if !band.is_empty() {
                general.push((gettext("Frequency Band"), band));
            }
        }
        if let Some(signal) = device.signal {
            general.push((gettext("Signal"), format!("{signal} %")));
        }
    }
    if let Some(state) = &device.networkd_state {
        general.push(("systemd-networkd".into(), state.clone()));
    }
    general.push((
        gettext("Managed by"),
        match backend {
            Backend::NetworkManager { .. } => "NetworkManager".to_string(),
            Backend::Fallback { networkd: true } => gettext("systemd-networkd (display only)"),
            _ => gettext("unknown (display only)"),
        },
    ));

    let none = || "–".to_string();
    let mut ipv4: Vec<InfoRow> = Vec::new();
    if device.ipv4.is_empty() {
        ipv4.push((gettext("Address"), none()));
    }
    for address in &device.ipv4 {
        ipv4.push((gettext("Address"), address.to_string()));
        if let Some(mask) = address.netmask() {
            ipv4.push((gettext("Netmask"), mask.to_string()));
        }
    }
    ipv4.push((
        gettext("Default Gateway"),
        device.gateway4.map(|g| g.to_string()).unwrap_or_else(none),
    ));
    let dns4: Vec<String> = device
        .dns
        .iter()
        .filter(|d| d.is_ipv4())
        .map(|d| d.to_string())
        .collect();
    ipv4.push((
        "DNS".into(),
        if dns4.is_empty() {
            none()
        } else {
            dns4.join(", ")
        },
    ));

    let mut ipv6: Vec<InfoRow> = Vec::new();
    if device.ipv6.is_empty() {
        ipv6.push((gettext("Address"), none()));
    }
    for address in &device.ipv6 {
        let shown = if address.is_global() {
            address.to_string()
        } else {
            gettext_f("{address} (local)", &[("address", &address.to_string())])
        };
        ipv6.push((gettext("Address"), shown));
    }
    ipv6.push((
        gettext("Default Gateway"),
        device.gateway6.map(|g| g.to_string()).unwrap_or_else(none),
    ));
    let dns6: Vec<String> = device
        .dns
        .iter()
        .filter(|d| d.is_ipv6())
        .map(|d| d.to_string())
        .collect();
    if !dns6.is_empty() {
        ipv6.push(("DNS".into(), dns6.join(", ")));
    }
    vec![
        (gettext("General"), general),
        ("IPv4".to_string(), ipv4),
        ("IPv6".to_string(), ipv6),
    ]
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn nm() -> Backend {
        Backend::NetworkManager {
            version: "1.54.0".into(),
        }
    }

    pub(crate) fn wired(state: DeviceState, carrier: bool) -> Device {
        let mut d = Device::new("/d/1", "eth0", Kind::Wired);
        d.state = state;
        d.carrier = Some(carrier);
        d.mac = "00:00:5E:00:53:DF".into();
        if state == DeviceState::Connected {
            d.ipv4 = vec![Address::v4(Ipv4Addr::new(192, 0, 2, 23), 24)];
            d.connection = Some("Kabelgebundene Verbindung 1".into());
        }
        d
    }

    pub(crate) fn wifi(state: DeviceState, signal: Option<u8>) -> Device {
        let mut d = Device::new("/d/2", "wlan0", Kind::Wireless);
        d.state = state;
        d.signal = signal;
        if state != DeviceState::Disconnected {
            d.ssid = Some("Heimnetz".into());
        }
        d
    }

    fn with(backend: Backend, devices: Vec<Device>) -> State {
        State {
            backend,
            devices,
            wireless_enabled: Some(true),
            wwan_enabled: Some(true),
            ..State::starting()
        }
    }

    #[test]
    fn signal_has_four_stages() {
        let stages: Vec<u8> = [0, 25, 26, 50, 51, 75, 76, 100]
            .into_iter()
            .map(signal_stage)
            .collect();
        assert_eq!(stages, [0, 0, 1, 1, 2, 2, 3, 3]);
    }

    #[test]
    fn icon_follows_the_state() {
        use DeviceState::*;
        assert_eq!(
            icon(&State::unavailable("kaputt")),
            Icon::NoService,
            "keine Daten"
        );
        assert_eq!(icon(&State::starting()), Icon::NoNetwork);
        // cable
        let s = with(nm(), vec![wired(Connected, true)]);
        assert_eq!(icon(&s), Icon::WiredConnected);
        let s = with(nm(), vec![wired(Unavailable, false)]);
        assert_eq!(icon(&s), Icon::WiredDisconnected);
        // fallback shows the same icons
        let s = with(
            Backend::Fallback { networkd: true },
            vec![wired(Connected, true)],
        );
        assert_eq!(icon(&s), Icon::WiredConnected);
        // WLAN with signal
        for (signal, stage) in [(10, 0), (40, 1), (60, 2), (90, 3)] {
            let s = with(
                nm(),
                vec![wired(Unavailable, false), wifi(Connected, Some(signal))],
            );
            assert_eq!(icon(&s), Icon::Wireless(stage), "{signal} %");
        }
        // cable wins when it is the primary connection
        let mut cable = wired(Connected, true);
        cable.primary = true;
        let s = with(nm(), vec![wifi(Connected, Some(90)), cable]);
        assert_eq!(icon(&s), Icon::WiredConnected);
        // connecting wins over everything connected
        let s = with(nm(), vec![wired(Connected, true), wifi(Connecting, None)]);
        assert_eq!(icon(&s), Icon::Connecting);
        // nothing connected, WLAN on: no network (choose one)
        let s = with(
            nm(),
            vec![wired(Unavailable, false), wifi(Disconnected, None)],
        );
        assert_eq!(icon(&s), Icon::NoNetwork);
        // WLAN off and no cable: the cable tells more
        let mut s = with(
            nm(),
            vec![wired(Unavailable, false), wifi(Unavailable, None)],
        );
        s.wireless_enabled = Some(false);
        assert_eq!(icon(&s), Icon::WiredDisconnected);
        // WLAN off, cable in but not connected: WLAN off
        s.devices[0] = wired(Disconnected, true);
        assert_eq!(icon(&s), Icon::WirelessOff);
        // hardware switch counts as off
        let mut s = with(nm(), vec![wifi(Unavailable, None)]);
        s.wireless_hardware_enabled = false;
        assert_eq!(icon(&s), Icon::WirelessOff);
        // no device at all
        assert_eq!(icon(&with(nm(), vec![])), Icon::NoNetwork);
    }

    #[test]
    fn every_icon_is_listed_once() {
        let mut seen = std::collections::HashSet::new();
        for icon in Icon::ALL {
            assert!(seen.insert(icon));
        }
        assert_eq!(seen.len(), 10);
    }

    #[test]
    fn airplane_is_wlan_and_wwan_off_and_only_with_networkmanager() {
        let mut s = with(nm(), vec![wifi(DeviceState::Unavailable, None)]);
        assert_eq!(s.airplane(), Some(false));
        s.wireless_enabled = Some(false);
        assert_eq!(s.airplane(), Some(false), "WWAN noch an");
        s.wwan_enabled = Some(false);
        assert_eq!(s.airplane(), Some(true));
        s.wwan_enabled = None;
        assert_eq!(s.airplane(), Some(true), "ohne WWAN-Angabe");
        s.backend = Backend::Fallback { networkd: true };
        assert_eq!(s.airplane(), None);
    }

    #[test]
    fn addresses_are_written_with_prefix_and_netmask() {
        let a = Address::v4(Ipv4Addr::new(192, 0, 2, 23), 24);
        assert_eq!(a.to_string(), "192.0.2.23/24");
        assert_eq!(a.netmask(), Some(Ipv4Addr::new(255, 255, 255, 0)));
        assert_eq!(
            Address::v4(Ipv4Addr::new(10, 0, 0, 1), 0).netmask(),
            Some(Ipv4Addr::UNSPECIFIED)
        );
        assert_eq!(
            Address::v4(Ipv4Addr::new(10, 0, 0, 1), 32).netmask(),
            Some(Ipv4Addr::BROADCAST)
        );
        let b = Address::v6("fe80::200:5eff:fe00:53df".parse().unwrap(), 64);
        assert_eq!(b.to_string(), "fe80::200:5eff:fe00:53df/64");
        assert!(!b.is_global());
        assert_eq!(b.netmask(), None);
        let c = Address::v6("2001:db8::1".parse().unwrap(), 64);
        assert!(c.is_global());
        assert!(!Address::v4(Ipv4Addr::new(169, 254, 3, 4), 16).is_global());
        assert!(!Address::v4(Ipv4Addr::LOCALHOST, 8).is_global());
    }

    #[test]
    fn main_address_prefers_global_ipv4() {
        let mut d = wired(DeviceState::Connected, true);
        d.ipv6 = vec![Address::v6("2001:db8::5".parse().unwrap(), 64)];
        assert_eq!(d.main_address().unwrap().to_string(), "192.0.2.23/24");
        d.ipv4 = vec![Address::v4(Ipv4Addr::new(169, 254, 1, 1), 16)];
        assert_eq!(d.main_address().unwrap().to_string(), "2001:db8::5/64");
        d.ipv6.clear();
        assert!(d.main_address().is_none());
    }

    #[test]
    fn speed_mac_band_and_ssid_texts() {
        assert_eq!(speed_text(100), "100 Mbit/s");
        assert_eq!(speed_text(1000), "1000 Mbit/s");
        assert_eq!(speed_text(2500), "2.5 Gbit/s");
        assert_eq!(speed_text(10000), "10 Gbit/s");
        assert_eq!(speed_text(2401), "2401 Mbit/s");
        assert_eq!(mac_text("00:00:5E:00:53:DF"), "00:00:5e:00:53:df");
        assert_eq!(mac_text("00-00-5e-00-53-df"), "00:00:5e:00:53:df");
        assert_eq!(mac_text("unbekannt"), "unbekannt");
        assert_eq!(band_text(2437), "2.4 GHz");
        assert_eq!(band_text(5180), "5 GHz");
        assert_eq!(band_text(6115), "6 GHz");
        assert_eq!(band_text(0), "");
        assert_eq!(ssid_text(b"Heimnetz"), "Heimnetz");
        assert_eq!(ssid_text("Café".as_bytes()), "Café");
        assert_eq!(ssid_text(&[0x43, 0x61, 0x66, 0xe9]), "Café", "Latin-1");
    }

    #[test]
    fn wpa_keys() {
        assert!(!valid_wpa_key("kurz"));
        assert!(valid_wpa_key("geheim123"));
        assert!(valid_wpa_key(&"x".repeat(63)));
        assert!(!valid_wpa_key(&"x".repeat(64)));
        assert!(valid_wpa_key(&"a1".repeat(32)));
    }

    #[test]
    fn security_from_access_point_flags() {
        assert_eq!(Security::from_nm_flags(0, 0, 0), Security::Open);
        assert_eq!(Security::from_nm_flags(1, 0, 0), Security::Wep);
        assert_eq!(Security::from_nm_flags(1, 0x188, 0x188), Security::WpaPsk);
        assert_eq!(Security::from_nm_flags(1, 0, 0x400), Security::Sae);
        assert_eq!(Security::from_nm_flags(1, 0, 0x500), Security::WpaPsk);
        assert_eq!(Security::from_nm_flags(1, 0, 0x200), Security::Enterprise);
        assert!(Security::WpaPsk.secured() && !Security::Open.secured());
        assert!(!Security::Enterprise.supported() && !Security::Wep.supported());
        assert_eq!(Security::Sae.key_mgmt(), Some("sae"));
    }

    fn ap(ssid: &str, strength: u8, active: bool) -> AccessPoint {
        AccessPoint {
            id: format!("/ap/{ssid}/{strength}"),
            ssid: ssid.into(),
            strength,
            security: Security::WpaPsk,
            frequency_mhz: 2437,
            bssid: String::new(),
            active,
            known: None,
        }
    }

    #[test]
    fn networks_one_row_per_name_connected_first_then_by_signal() {
        let mut d = wifi(DeviceState::Connected, Some(40));
        d.access_points = vec![
            ap("Nachbar", 80, false),
            ap("Heimnetz", 40, true),
            ap("Heimnetz", 70, false),
            ap("", 99, false),
            ap("Café", 55, false),
            ap("Nachbar", 30, false),
        ];
        d.access_points[2].known = Some("/s/1".into());
        let list = networks(&d);
        let names: Vec<(&str, u8, bool)> = list
            .iter()
            .map(|a| (a.ssid.as_str(), a.strength, a.active))
            .collect();
        assert_eq!(
            names,
            [
                ("Heimnetz", 40, true),
                ("Nachbar", 80, false),
                ("Café", 55, false)
            ]
        );
        assert_eq!(list[0].known.as_deref(), Some("/s/1"), "Profil bleibt");
    }

    #[test]
    fn tooltips() {
        use DeviceState::*;
        let s = with(nm(), vec![wired(Connected, true)]);
        assert_eq!(
            tooltip(&s),
            "Wired network “Kabelgebundene Verbindung 1” connected (eth0)\nAddress: 192.0.2.23/24"
        );
        let mut w = wifi(Connected, Some(72));
        w.ipv4 = vec![Address::v4(Ipv4Addr::new(192, 0, 2, 40), 24)];
        let s = with(nm(), vec![wired(Unavailable, false), w]);
        assert_eq!(
            tooltip(&s),
            "Wireless network “Heimnetz” connected, signal 72 %\nAddress: 192.0.2.40/24"
        );
        let mut d = wired(Connected, true);
        d.connection = None;
        let s = with(Backend::Fallback { networkd: true }, vec![d]);
        assert_eq!(
            tooltip(&s),
            "Wired network eth0 connected\nAddress: 192.0.2.23/24\nManaged by systemd-networkd, display only"
        );
        let s = with(nm(), vec![wired(Unavailable, false)]);
        assert_eq!(tooltip(&s), "No network cable plugged in");
        let mut s = with(nm(), vec![wifi(Unavailable, None)]);
        s.wireless_enabled = Some(false);
        assert_eq!(tooltip(&s), "Wireless is disabled");
        s.wwan_enabled = Some(false);
        assert_eq!(tooltip(&s), "Airplane mode is on");
        let s = with(nm(), vec![wifi(Connecting, None)]);
        assert_eq!(
            tooltip(&s),
            "Connecting to the wireless network “Heimnetz”…"
        );
        let s = with(nm(), vec![wifi(Disconnected, None)]);
        assert_eq!(tooltip(&s), "No network connection");
        assert_eq!(
            tooltip(&State::unavailable("/sys/class/net fehlt")),
            "No network service\n/sys/class/net fehlt"
        );
        let s = with(Backend::Fallback { networkd: false }, vec![]);
        assert_eq!(
            tooltip(&s),
            "No network device\nNo NetworkManager found, display only"
        );
    }

    #[test]
    fn info_lists_interface_mac_addresses_gateway_dns_speed() {
        let mut d = wired(DeviceState::Connected, true);
        d.speed_mbit = Some(1000);
        d.gateway4 = Some(Ipv4Addr::new(192, 0, 2, 1));
        d.dns = vec![IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1))];
        d.ipv6 = vec![Address::v6(
            "fe80::200:5eff:fe00:53df".parse().unwrap(),
            64,
        )];
        let groups = info(&d, &Backend::Fallback { networkd: true });
        let find = |group: usize, label: &str| {
            groups[group]
                .1
                .iter()
                .find(|(l, _)| l == label)
                .map(|(_, v)| v.clone())
        };
        assert_eq!(
            groups.iter().map(|g| g.0.clone()).collect::<Vec<_>>(),
            ["General", "IPv4", "IPv6"]
        );
        assert_eq!(find(0, "Interface").as_deref(), Some("eth0 (wired)"));
        assert_eq!(
            find(0, "Hardware Address").as_deref(),
            Some("00:00:5e:00:53:df")
        );
        assert_eq!(find(0, "Speed").as_deref(), Some("1000 Mbit/s"));
        assert_eq!(
            find(0, "Managed by").as_deref(),
            Some("systemd-networkd (display only)")
        );
        assert_eq!(find(1, "Address").as_deref(), Some("192.0.2.23/24"));
        assert_eq!(find(1, "Netmask").as_deref(), Some("255.255.255.0"));
        assert_eq!(find(1, "Default Gateway").as_deref(), Some("192.0.2.1"));
        assert_eq!(find(1, "DNS").as_deref(), Some("192.0.2.1"));
        assert_eq!(
            find(2, "Address").as_deref(),
            Some("fe80::200:5eff:fe00:53df/64 (local)")
        );
        assert_eq!(find(2, "Default Gateway").as_deref(), Some("–"));
    }

    #[test]
    fn read_only_note_only_for_the_fallback() {
        assert!(nm().read_only_note().is_none());
        assert!(nm().can_control());
        assert!(!Backend::Fallback { networkd: true }.can_control());
        assert!(
            Backend::Fallback { networkd: true }
                .read_only_note()
                .unwrap()
                .contains("systemd-networkd")
        );
    }

    #[test]
    fn device_titles_and_states() {
        let d = wired(DeviceState::Unavailable, false);
        assert_eq!(d.title(), "Wired Network (eth0)");
        assert_eq!(d.state_text(), "Cable unplugged");
        let d = wifi(DeviceState::Connecting, None);
        assert_eq!(d.title(), "Wireless (wlan0)");
        assert_eq!(d.state_text(), "connecting…");
    }
}
