//! The read-only backend without NetworkManager: the kernel's data
//! (`kernel.rs`), and on top what systemd-networkd says about each link
//! (`AdministrativeState`: `configured`, `configuring`, `unmanaged`, …) and
//! the name servers systemd-resolved uses per link, both over D-Bus with
//! public, typed properties.
//!
//! Changes are heard, not polled: a netlink socket (`NETLINK_ROUTE`, groups
//! link, IPv4/IPv6 address and route) only wakes the backend, which then
//! reads everything again after [`DEBOUNCE`]; `PropertiesChanged` from
//! systemd-networkd and systemd-resolved do the same. A test root
//! (`QUERENCIA_NETZWERK_WURZEL`) is watched with a GIO file monitor on
//! `<root>/ereignis` instead, and systemd-networkd's states then come from
//! `<root>/networkd` (`2 configured` per line; the file present means
//! systemd-networkd runs).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::net::IpAddr;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::rc::{Rc, Weak};
use std::time::Duration;

use gtk4::gio::{self, BusType, DBusCallFlags, DBusConnection, DBusSignalFlags, prelude::*};
use gtk4::glib::{self, Variant};

use crate::kernel::{self, Sources};
use crate::model::{Backend, State};

/// Wait this long after a wake-up before reading.
pub const DEBOUNCE: Duration = Duration::from_millis(200);
const NETWORKD: &str = "org.freedesktop.network1";
const NETWORKD_PATH: &str = "/org/freedesktop/network1";
const RESOLVED: &str = "org.freedesktop.resolve1";
const RESOLVED_PATH: &str = "/org/freedesktop/resolve1";

/// A netlink socket that becomes readable whenever links, addresses or
/// routes change. The messages are only drained, never parsed.
fn netlink_socket() -> std::io::Result<OwnedFd> {
    // SAFETY: plain socket calls; the descriptor is owned by OwnedFd at
    // once and the address struct is fully initialised.
    unsafe {
        let fd = libc::socket(
            libc::AF_NETLINK,
            libc::SOCK_RAW | libc::SOCK_CLOEXEC | libc::SOCK_NONBLOCK,
            libc::NETLINK_ROUTE,
        );
        if fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        let owned = OwnedFd::from_raw_fd(fd);
        let mut address: libc::sockaddr_nl = std::mem::zeroed();
        address.nl_family = libc::AF_NETLINK as libc::sa_family_t;
        address.nl_groups = (libc::RTMGRP_LINK
            | libc::RTMGRP_IPV4_IFADDR
            | libc::RTMGRP_IPV6_IFADDR
            | libc::RTMGRP_IPV4_ROUTE
            | libc::RTMGRP_IPV6_ROUTE) as u32;
        let result = libc::bind(
            fd,
            &address as *const libc::sockaddr_nl as *const libc::sockaddr,
            std::mem::size_of::<libc::sockaddr_nl>() as libc::socklen_t,
        );
        if result < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(owned)
    }
}

/// Reads everything waiting on the netlink socket; true if there was
/// something (or the buffer overflowed, which also means changes).
fn drain(fd: i32) -> bool {
    let mut buffer = [0u8; 8192];
    let mut any = false;
    loop {
        // SAFETY: recv into a local buffer of the given length.
        let n = unsafe { libc::recv(fd, buffer.as_mut_ptr().cast(), buffer.len(), 0) };
        if n > 0 {
            any = true;
            continue;
        }
        if n < 0 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ENOBUFS) {
            any = true;
            continue;
        }
        return any;
    }
}

/// systemd-networkd's administrative states by link index, from the test
/// root's file.
pub fn parse_networkd_file(text: &str) -> HashMap<u32, String> {
    text.lines()
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            Some((words.next()?.parse().ok()?, words.next()?.to_string()))
        })
        .collect()
}

/// systemd-resolved's `DNS` property `a(iiay)` (link index, family,
/// address) by link index.
pub fn resolved_dns(value: &Variant) -> HashMap<u32, Vec<IpAddr>> {
    let mut map: HashMap<u32, Vec<IpAddr>> = HashMap::new();
    if !value.is_container() {
        return map;
    }
    for entry in value.iter() {
        if entry.n_children() != 3 {
            continue;
        }
        let index = entry.child_value(0).get::<i32>().unwrap_or(-1);
        let bytes = crate::props::bytes(&entry.child_value(2));
        let ip = match bytes.len() {
            4 => Some(IpAddr::from(<[u8; 4]>::try_from(bytes.as_slice()).unwrap())),
            16 => Some(IpAddr::from(
                <[u8; 16]>::try_from(bytes.as_slice()).unwrap(),
            )),
            _ => None,
        };
        if let (Ok(index), Some(ip)) = (u32::try_from(index), ip) {
            map.entry(index).or_default().push(ip);
        }
    }
    map
}

type StateFn = Rc<dyn Fn(State)>;

/// The running fallback backend.
pub struct Fallback {
    sources: Sources,
    system: RefCell<Option<DBusConnection>>,
    netlink: RefCell<Option<(OwnedFd, glib::SourceId)>>,
    monitor: RefCell<Option<gio::FileMonitor>>,
    signals: RefCell<Vec<gio::SignalSubscription>>,
    scheduled: Cell<bool>,
    reading: Cell<bool>,
    again: Cell<bool>,
    stopped: Cell<bool>,
    on_state: StateFn,
    log: String,
}

impl Fallback {
    /// Starts the fallback; `on_state` gets every new state.
    pub fn start(log: &str, on_state: impl Fn(State) + 'static) -> Rc<Fallback> {
        let fallback = Rc::new(Fallback {
            sources: Sources::from_environment(),
            system: RefCell::new(None),
            netlink: RefCell::new(None),
            monitor: RefCell::new(None),
            signals: RefCell::new(Vec::new()),
            scheduled: Cell::new(false),
            reading: Cell::new(false),
            again: Cell::new(false),
            stopped: Cell::new(false),
            on_state: Rc::new(on_state),
            log: log.to_string(),
        });
        fallback.watch();
        fallback.read_now();
        fallback
    }

    /// Stops watching; no more states are reported.
    pub fn stop(&self) {
        self.stopped.set(true);
        if let Some((_, source)) = self.netlink.borrow_mut().take() {
            source.remove();
        }
        self.monitor.replace(None);
        self.signals.borrow_mut().clear();
    }

    fn watch(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        if self.sources.is_test_root() {
            let file = gio::File::for_path(
                self.sources
                    .ipv4_file
                    .as_ref()
                    .and_then(|p| p.parent())
                    .unwrap_or(std::path::Path::new("/"))
                    .join("ereignis"),
            );
            match file.monitor_file(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE) {
                Ok(monitor) => {
                    monitor.connect_changed(move |_, _, _, _| {
                        if let Some(f) = weak.upgrade() {
                            f.schedule();
                        }
                    });
                    self.monitor.replace(Some(monitor));
                }
                Err(err) => {
                    crate::say!("{}: Netzwerk: Testwurzel nicht überwacht: {err}", self.log)
                }
            }
            return;
        }
        match netlink_socket() {
            Ok(fd) => {
                let raw = fd.as_raw_fd();
                let source =
                    glib_unix::unix_fd_add_local(raw, glib::IOCondition::IN, move |fd, _| {
                        if drain(fd)
                            && let Some(f) = weak.upgrade()
                        {
                            f.schedule();
                        }
                        glib::ControlFlow::Continue
                    });
                self.netlink.replace(Some((fd, source)));
            }
            Err(err) => crate::say!(
                "{}: Netzwerk: kein Netlink ({err}); Änderungen werden nicht gemeldet",
                self.log
            ),
        }
    }

    fn schedule(self: &Rc<Self>) {
        if self.stopped.get() || self.scheduled.replace(true) {
            return;
        }
        let weak = Rc::downgrade(self);
        glib::timeout_add_local_once(DEBOUNCE, move || {
            if let Some(f) = weak.upgrade() {
                f.scheduled.set(false);
                f.read_now();
            }
        });
    }

    fn read_now(self: &Rc<Self>) {
        if self.reading.replace(true) {
            self.again.set(true);
            return;
        }
        let fallback = self.clone();
        glib::MainContext::default().spawn_local(async move {
            loop {
                fallback.again.set(false);
                let state = fallback.read().await;
                if fallback.stopped.get() {
                    break;
                }
                (fallback.on_state)(state);
                if !fallback.again.get() {
                    break;
                }
            }
            fallback.reading.set(false);
        });
    }

    /// The system bus, connected on first use, with the signals of
    /// systemd-networkd and systemd-resolved subscribed.
    async fn system_bus(self: &Rc<Self>) -> Option<DBusConnection> {
        if let Some(bus) = self.system.borrow().as_ref() {
            return Some(bus.clone());
        }
        let bus = gio::bus_get_future(BusType::System).await.ok()?;
        let mut signals = Vec::new();
        for sender in [NETWORKD, RESOLVED] {
            let weak: Weak<Fallback> = Rc::downgrade(self);
            signals.push(bus.subscribe_to_signal(
                Some(sender),
                Some("org.freedesktop.DBus.Properties"),
                Some("PropertiesChanged"),
                None,
                None,
                DBusSignalFlags::NONE,
                move |_| {
                    if let Some(f) = weak.upgrade() {
                        f.schedule();
                    }
                },
            ));
        }
        self.signals.replace(signals);
        self.system.replace(Some(bus.clone()));
        Some(bus)
    }

    async fn read(self: &Rc<Self>) -> State {
        let (networkd, running, dns) = if self.sources.is_test_root() {
            let file = self
                .sources
                .ipv4_file
                .as_ref()
                .and_then(|p| p.parent())
                .map(|root| root.join("networkd"));
            let text = file.and_then(|f| std::fs::read_to_string(f).ok());
            let running = text.is_some();
            (
                parse_networkd_file(&text.unwrap_or_default()),
                running,
                None,
            )
        } else {
            self.from_systemd().await
        };
        match kernel::read_links(&self.sources, &networkd, dns.as_ref()) {
            Ok(links) => State {
                backend: Backend::Fallback { networkd: running },
                devices: links.into_iter().map(|l| l.device).collect(),
                ..State::starting()
            },
            Err(reason) => State::unavailable(&reason),
        }
    }

    /// systemd-networkd's states by link index (and whether it runs) and
    /// systemd-resolved's name servers by link index (`None` without it).
    async fn from_systemd(
        self: &Rc<Self>,
    ) -> (
        HashMap<u32, String>,
        bool,
        Option<HashMap<u32, Vec<IpAddr>>>,
    ) {
        let Some(bus) = self.system_bus().await else {
            return (HashMap::new(), false, None);
        };
        let call = |name: &'static str,
                    path: String,
                    iface: &'static str,
                    method: &'static str,
                    args: Option<Variant>| {
            bus.call_future(
                Some(name),
                &path,
                iface,
                method,
                args.as_ref(),
                None,
                DBusCallFlags::NO_AUTO_START,
                3000,
            )
        };
        let mut states = HashMap::new();
        let mut running = false;
        let indexes: Vec<u32> = std::fs::read_dir(&self.sources.sys_class_net)
            .map(|dir| {
                dir.flatten()
                    .filter_map(|e| {
                        std::fs::read_to_string(e.path().join("ifindex"))
                            .ok()?
                            .trim()
                            .parse()
                            .ok()
                    })
                    .collect()
            })
            .unwrap_or_default();
        for index in indexes {
            let Ok(index_i32) = i32::try_from(index) else {
                continue;
            };
            let link = call(
                NETWORKD,
                NETWORKD_PATH.to_string(),
                "org.freedesktop.network1.Manager",
                "GetLinkByIndex",
                Some((index_i32,).to_variant()),
            )
            .await;
            let Ok(link) = link else {
                continue;
            };
            running = true;
            let Some(path) = link.child_value(1).str().map(str::to_string) else {
                continue;
            };
            let state = call(
                NETWORKD,
                path,
                "org.freedesktop.DBus.Properties",
                "Get",
                Some(("org.freedesktop.network1.Link", "AdministrativeState").to_variant()),
            )
            .await;
            if let Ok(reply) = state
                && let Some(text) = crate::props::unbox(reply.child_value(0))
                    .str()
                    .map(str::to_string)
            {
                states.insert(index, text);
            }
        }
        let dns = call(
            RESOLVED,
            RESOLVED_PATH.to_string(),
            "org.freedesktop.DBus.Properties",
            "Get",
            Some(("org.freedesktop.resolve1.Manager", "DNS").to_variant()),
        )
        .await
        .ok()
        .map(|reply| resolved_dns(&crate::props::unbox(reply.child_value(0))));
        (states, running, dns)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn networkd_file_of_the_test_root() {
        let map = parse_networkd_file("2 configured\nkaputt\n3 configuring\n");
        assert_eq!(map.get(&2).map(String::as_str), Some("configured"));
        assert_eq!(map.get(&3).map(String::as_str), Some("configuring"));
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn resolved_dns_by_link() {
        let list: Vec<(i32, i32, Vec<u8>)> = vec![
            (2, libc::AF_INET, vec![192, 0, 2, 1]),
            (0, libc::AF_INET, vec![9, 9, 9, 9]),
            (
                2,
                libc::AF_INET6,
                vec![0xfe, 0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            ),
            (3, libc::AF_INET, vec![1, 2, 3]),
        ];
        let map = resolved_dns(&list.to_variant());
        assert_eq!(
            map.get(&2).unwrap(),
            &vec![
                "192.0.2.1".parse::<IpAddr>().unwrap(),
                "fe80::1".parse::<IpAddr>().unwrap()
            ]
        );
        assert_eq!(map.get(&0).unwrap().len(), 1, "global");
        assert!(!map.contains_key(&3), "kaputte Adresse");
    }

    #[test]
    fn netlink_socket_opens_without_privileges() {
        let fd = netlink_socket().expect("Netlink");
        // Nothing to read is fine; it must not block.
        let _ = drain(fd.as_raw_fd());
    }
}
