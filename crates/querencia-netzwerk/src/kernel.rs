//! The network as the kernel reports it, read only: the fallback when no
//! NetworkManager runs (`fallback.rs` adds systemd-networkd and
//! systemd-resolved and watches for changes).
//!
//! - devices: `/sys/class/net/<name>/` (`operstate`, `carrier`, `address`,
//!   `speed`, `ifindex`, `device/driver`, `wireless` or `phy80211`); only
//!   hardware devices (with `device`) and WLAN, never `lo`, bridges or
//!   virtual links;
//! - IPv4 addresses: `getifaddrs` (the kernel lists them nowhere in
//!   `/proc`); IPv6 addresses: `/proc/net/if_inet6`;
//! - default gateways: `/proc/net/route`, `/proc/net/ipv6_route`;
//! - WLAN signal: `/proc/net/wireless` (link quality of 70);
//! - DNS without systemd-resolved: `/etc/resolv.conf`.
//!
//! For tests every path can sit below another root ([`Sources::below`],
//! environment `QUERENCIA_NETZWERK_WURZEL`); IPv4 addresses then come from
//! the file `<root>/ipv4` (`eth0 192.0.2.23/24` per line).

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::{Path, PathBuf};

use crate::model::{Address, Device, DeviceState, Kind};

/// Environment variable naming a test root instead of `/`.
pub const ROOT_VARIABLE: &str = "QUERENCIA_NETZWERK_WURZEL";

/// Where the kernel's data is read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sources {
    pub sys_class_net: PathBuf,
    pub proc_net: PathBuf,
    pub resolv_conf: PathBuf,
    /// Test root: IPv4 addresses from this file instead of `getifaddrs`.
    pub ipv4_file: Option<PathBuf>,
}

impl Sources {
    /// The real system.
    pub fn system() -> Sources {
        Sources {
            sys_class_net: "/sys/class/net".into(),
            proc_net: "/proc/net".into(),
            resolv_conf: "/etc/resolv.conf".into(),
            ipv4_file: None,
        }
    }

    /// Everything below `root` (tests).
    pub fn below(root: &Path) -> Sources {
        Sources {
            sys_class_net: root.join("sys/class/net"),
            proc_net: root.join("proc/net"),
            resolv_conf: root.join("etc/resolv.conf"),
            ipv4_file: Some(root.join("ipv4")),
        }
    }

    /// The test root from [`ROOT_VARIABLE`], else the real system.
    pub fn from_environment() -> Sources {
        match std::env::var_os(ROOT_VARIABLE) {
            Some(root) if !root.is_empty() => Sources::below(Path::new(&root)),
            _ => Sources::system(),
        }
    }

    pub fn is_test_root(&self) -> bool {
        self.ipv4_file.is_some()
    }
}

/// One device as read, with its kernel index (for systemd-networkd).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub index: u32,
    pub operstate: String,
    pub device: Device,
}

/// A default route: interface, gateway, metric.
pub type Route<A> = (String, A, u32);

fn read(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
}

/// Default IPv4 routes from `/proc/net/route` (destination 0, flags up and
/// gateway; addresses in the kernel's byte order, little-endian hex).
pub fn parse_route(text: &str) -> Vec<Route<Ipv4Addr>> {
    const RTF_UP: u32 = 0x1;
    const RTF_GATEWAY: u32 = 0x2;
    text.lines()
        .skip(1)
        .filter_map(|line| {
            let f: Vec<&str> = line.split_whitespace().collect();
            if f.len() < 8 {
                return None;
            }
            let destination = u32::from_str_radix(f[1], 16).ok()?;
            let gateway = u32::from_str_radix(f[2], 16).ok()?;
            let flags = u32::from_str_radix(f[3], 16).ok()?;
            let metric = f[6].parse().ok()?;
            let mask = u32::from_str_radix(f[7], 16).ok()?;
            (destination == 0
                && mask == 0
                && flags & (RTF_UP | RTF_GATEWAY) == RTF_UP | RTF_GATEWAY)
                .then(|| {
                    (
                        f[0].to_string(),
                        Ipv4Addr::from(gateway.to_le_bytes()),
                        metric,
                    )
                })
        })
        .collect()
}

fn hex_ipv6(hex: &str) -> Option<Ipv6Addr> {
    if hex.len() != 32 {
        return None;
    }
    let mut bytes = [0u8; 16];
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(Ipv6Addr::from(bytes))
}

/// Default IPv6 routes from `/proc/net/ipv6_route` (`::/0` with a next hop).
pub fn parse_ipv6_route(text: &str) -> Vec<Route<Ipv6Addr>> {
    const RTF_UP: u32 = 0x1;
    text.lines()
        .filter_map(|line| {
            let f: Vec<&str> = line.split_whitespace().collect();
            if f.len() < 10 {
                return None;
            }
            let destination = hex_ipv6(f[0])?;
            let length = u8::from_str_radix(f[1], 16).ok()?;
            let next_hop = hex_ipv6(f[4])?;
            let metric = u32::from_str_radix(f[5], 16).ok()?;
            let flags = u32::from_str_radix(f[8], 16).ok()?;
            (destination.is_unspecified()
                && length == 0
                && !next_hop.is_unspecified()
                && flags & RTF_UP != 0
                && f[9] != "lo")
                .then(|| (f[9].to_string(), next_hop, metric))
        })
        .collect()
}

/// IPv6 addresses from `/proc/net/if_inet6`: address, index, prefix,
/// scope, flags, name (hex fields).
pub fn parse_if_inet6(text: &str) -> Vec<(String, Address)> {
    text.lines()
        .filter_map(|line| {
            let f: Vec<&str> = line.split_whitespace().collect();
            if f.len() < 6 {
                return None;
            }
            let ip = hex_ipv6(f[0])?;
            let prefix = u8::from_str_radix(f[2], 16).ok()?;
            Some((f[5].to_string(), Address::v6(ip, prefix)))
        })
        .collect()
}

/// Name servers from a `resolv.conf`.
pub fn parse_resolv_conf(text: &str) -> Vec<IpAddr> {
    text.lines()
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            (words.next()? == "nameserver")
                .then(|| words.next()?.split('%').next()?.parse().ok())
                .flatten()
        })
        .collect()
}

/// WLAN link quality in percent from `/proc/net/wireless` (quality of 70,
/// as the kernel's wext reports it).
pub fn parse_proc_wireless(text: &str) -> Vec<(String, u8)> {
    text.lines()
        .skip(2)
        .filter_map(|line| {
            let (name, rest) = line.split_once(':')?;
            let quality: f64 = rest
                .split_whitespace()
                .nth(1)?
                .trim_end_matches('.')
                .parse()
                .ok()?;
            let percent = (quality * 100.0 / 70.0).round().clamp(0.0, 100.0) as u8;
            Some((name.trim().to_string(), percent))
        })
        .collect()
}

/// IPv4 addresses of the test root: `eth0 192.0.2.23/24` per line.
pub fn parse_ipv4_file(text: &str) -> Vec<(String, Address)> {
    text.lines()
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            let name = words.next()?;
            let (ip, prefix) = words.next()?.split_once('/')?;
            Some((
                name.to_string(),
                Address::v4(ip.parse().ok()?, prefix.parse().ok()?),
            ))
        })
        .collect()
}

/// IPv4 addresses of all interfaces from `getifaddrs`.
pub fn ipv4_addresses() -> Vec<(String, Address)> {
    let mut list = Vec::new();
    let mut head: *mut libc::ifaddrs = std::ptr::null_mut();
    // SAFETY: getifaddrs fills `head` with a list we only read and then
    // free with freeifaddrs; every pointer is checked before use.
    unsafe {
        if libc::getifaddrs(&mut head) != 0 {
            return list;
        }
        let mut entry = head;
        while !entry.is_null() {
            let ifa = &*entry;
            entry = ifa.ifa_next;
            if ifa.ifa_addr.is_null() || i32::from((*ifa.ifa_addr).sa_family) != libc::AF_INET {
                continue;
            }
            let name = std::ffi::CStr::from_ptr(ifa.ifa_name)
                .to_string_lossy()
                .into_owned();
            let addr = &*(ifa.ifa_addr as *const libc::sockaddr_in);
            let ip = Ipv4Addr::from(u32::from_be(addr.sin_addr.s_addr));
            let prefix = if ifa.ifa_netmask.is_null() {
                32
            } else {
                let mask = &*(ifa.ifa_netmask as *const libc::sockaddr_in);
                u32::from_be(mask.sin_addr.s_addr).count_ones() as u8
            };
            list.push((name, Address::v4(ip, prefix)));
        }
        libc::freeifaddrs(head);
    }
    list
}

/// The state of a link in the fallback: no cable or link down →
/// unavailable; WLAN authenticating (`dormant`) → connecting; up with a
/// global address → connected; up without one → connecting while
/// systemd-networkd still configures it, else disconnected.
pub fn link_state(
    operstate: &str,
    carrier: Option<bool>,
    has_global_address: bool,
    networkd_state: Option<&str>,
) -> DeviceState {
    match operstate {
        "down" | "lowerlayerdown" | "notpresent" => return DeviceState::Unavailable,
        "dormant" => return DeviceState::Connecting,
        _ => {}
    }
    if carrier == Some(false) {
        return DeviceState::Unavailable;
    }
    if has_global_address {
        DeviceState::Connected
    } else if matches!(
        networkd_state,
        Some("configuring" | "pending" | "initialized")
    ) {
        DeviceState::Connecting
    } else {
        DeviceState::Disconnected
    }
}

/// The links below `sources.sys_class_net` that the applet shows, sorted by
/// kernel index, with addresses, routes and DNS from the other sources.
/// `networkd` holds systemd-networkd's administrative state per index (may
/// be empty); `dns` the name servers per index from systemd-resolved
/// (`None`: no resolved, read `resolv.conf`).
pub fn read_links(
    sources: &Sources,
    networkd: &HashMap<u32, String>,
    dns: Option<&HashMap<u32, Vec<IpAddr>>>,
) -> Result<Vec<Link>, String> {
    let dir = std::fs::read_dir(&sources.sys_class_net)
        .map_err(|e| format!("{} nicht lesbar: {e}", sources.sys_class_net.display()))?;
    let ipv4 = match &sources.ipv4_file {
        Some(file) => parse_ipv4_file(&std::fs::read_to_string(file).unwrap_or_default()),
        None => ipv4_addresses(),
    };
    let proc =
        |name: &str| std::fs::read_to_string(sources.proc_net.join(name)).unwrap_or_default();
    let ipv6 = parse_if_inet6(&proc("if_inet6"));
    let routes4 = parse_route(&proc("route"));
    let routes6 = parse_ipv6_route(&proc("ipv6_route"));
    let signals = parse_proc_wireless(&proc("wireless"));
    let resolv =
        parse_resolv_conf(&std::fs::read_to_string(&sources.resolv_conf).unwrap_or_default());

    let mut links = Vec::new();
    for entry in dir.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        if name == "lo" {
            continue;
        }
        let wireless = path.join("wireless").exists() || path.join("phy80211").exists();
        // Only hardware: bridges, veth, tun, docker0 have no `device`.
        if !wireless && !path.join("device").exists() {
            continue;
        }
        // ARPHRD_ETHER only (WLAN reports it too).
        if read(&path.join("type")).is_some_and(|t| t != "1") {
            continue;
        }
        let index: u32 = read(&path.join("ifindex"))
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let kind = if wireless {
            Kind::Wireless
        } else {
            Kind::Wired
        };
        let mut device = Device::new(&name, &name, kind);
        device.mac = read(&path.join("address")).unwrap_or_default();
        device.carrier = read(&path.join("carrier")).map(|c| c == "1");
        device.speed_mbit = read(&path.join("speed"))
            .and_then(|s| s.parse::<i64>().ok())
            .filter(|&s| s > 0)
            .and_then(|s| u32::try_from(s).ok());
        device.driver = std::fs::read_link(path.join("device/driver"))
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()));
        device.ipv4 = ipv4
            .iter()
            .filter(|(n, _)| *n == name)
            .map(|(_, a)| *a)
            .collect();
        device.ipv6 = ipv6
            .iter()
            .filter(|(n, _)| *n == name)
            .map(|(_, a)| *a)
            .collect();
        device.gateway4 = routes4
            .iter()
            .filter(|(n, _, _)| *n == name)
            .min_by_key(|(_, _, m)| *m)
            .map(|(_, g, _)| *g);
        device.gateway6 = routes6
            .iter()
            .filter(|(n, _, _)| *n == name)
            .min_by_key(|(_, _, m)| *m)
            .map(|(_, g, _)| *g);
        device.dns = match dns {
            Some(per_link) => per_link.get(&index).cloned().unwrap_or_default(),
            None if device.gateway4.is_some() || device.gateway6.is_some() => resolv.clone(),
            None => Vec::new(),
        };
        device.networkd_state = networkd.get(&index).cloned();
        if wireless {
            device.signal = signals.iter().find(|(n, _)| *n == name).map(|(_, s)| *s);
        }
        let operstate = read(&path.join("operstate")).unwrap_or_default();
        let global = device
            .ipv4
            .iter()
            .chain(&device.ipv6)
            .any(|a| a.is_global());
        device.state = link_state(
            &operstate,
            device.carrier,
            global,
            device.networkd_state.as_deref(),
        );
        if !device.connected() {
            device.signal = None;
        }
        links.push(Link {
            index,
            operstate,
            device,
        });
    }
    links.sort_by_key(|l| (l.index, l.device.interface.clone()));
    // The primary device holds the default route with the lowest metric.
    let primary = routes4
        .iter()
        .map(|(n, _, m)| (n, *m))
        .chain(routes6.iter().map(|(n, _, m)| (n, *m)))
        .filter(|(n, _)| {
            links
                .iter()
                .any(|l| &l.device.interface == *n && l.device.connected())
        })
        .min_by_key(|(_, m)| *m)
        .map(|(n, _)| n.clone());
    for link in &mut links {
        link.device.primary = Some(&link.device.interface) == primary.as_ref();
    }
    Ok(links)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROUTE: &str =
        "Iface\tDestination\tGateway \tFlags\tRefCnt\tUse\tMetric\tMask\t\tMTU\tWindow\tIRTT
eth0\t00000000\t010200C0\t0003\t0\t0\t100\t00000000\t0\t0\t0
eth0\t000200C0\t00000000\t0001\t0\t0\t100\t00FFFFFF\t0\t0\t0
wlan0\t00000000\t0102A8C0\t0003\t0\t0\t600\t00000000\t0\t0\t0
eth1\t00000000\t00000000\t0001\t0\t0\t50\t00000000\t0\t0\t0
";
    const IPV6_ROUTE: &str = "fe800000000000000000000000000000 40 00000000000000000000000000000000 00 00000000000000000000000000000000 00000100 00000005 00000000 00000001     eth0
00000000000000000000000000000000 00 00000000000000000000000000000000 00 fe800000000000000000000000000001 00000400 00000001 00000000 00450003     eth0
00000000000000000000000000000000 00 00000000000000000000000000000000 00 00000000000000000000000000000000 ffffffff 00000001 00000000 00200200       lo
";
    const IF_INET6: &str = "00000000000000000000000000000001 01 80 10 80       lo
fe8000000000000002005efffe0053df 02 40 20 80     eth0
20010db8000000000000000000000005 02 40 00 00     eth0
";

    #[test]
    fn default_routes_ipv4() {
        let routes = parse_route(ROUTE);
        assert_eq!(
            routes,
            [
                ("eth0".to_string(), Ipv4Addr::new(192, 0, 2, 1), 100),
                ("wlan0".to_string(), Ipv4Addr::new(192, 168, 2, 1), 600)
            ]
        );
        assert!(parse_route("").is_empty());
        assert!(parse_route("Kopf\nkaputt").is_empty());
    }

    #[test]
    fn default_routes_ipv6() {
        assert_eq!(
            parse_ipv6_route(IPV6_ROUTE),
            [(
                "eth0".to_string(),
                "fe80::1".parse::<Ipv6Addr>().unwrap(),
                0x400
            )]
        );
    }

    #[test]
    fn ipv6_addresses() {
        let list = parse_if_inet6(IF_INET6);
        assert_eq!(list.len(), 3);
        assert_eq!(list[1].0, "eth0");
        assert_eq!(list[1].1.to_string(), "fe80::200:5eff:fe00:53df/64");
        assert_eq!(list[2].1.to_string(), "2001:db8::5/64");
        assert!(list[2].1.is_global() && !list[1].1.is_global());
    }

    #[test]
    fn resolv_conf_wireless_and_ipv4_file() {
        assert_eq!(
            parse_resolv_conf(
                "# x\nnameserver 127.0.0.53\noptions edns0\nnameserver fe80::1%eth0\nsearch lan\n"
            ),
            [
                "127.0.0.53".parse::<IpAddr>().unwrap(),
                "fe80::1".parse::<IpAddr>().unwrap()
            ]
        );
        let wireless =
            "Inter-| sta-|   Quality        |   Discarded packets               | Missed | WE
 face | tus | link level noise |  nwid  crypt   frag  retry   misc | beacon | 22
 wlan0: 0000   49.  -61.  -256        0      0      0      0     12        0
";
        assert_eq!(parse_proc_wireless(wireless), [("wlan0".to_string(), 70)]);
        assert_eq!(
            parse_ipv4_file("eth0 192.0.2.23/24\nkaputt\nwlan0 10.0.0.2/8\n"),
            [
                (
                    "eth0".to_string(),
                    Address::v4(Ipv4Addr::new(192, 0, 2, 23), 24)
                ),
                (
                    "wlan0".to_string(),
                    Address::v4(Ipv4Addr::new(10, 0, 0, 2), 8)
                )
            ]
        );
    }

    #[test]
    fn link_states() {
        use DeviceState::*;
        assert_eq!(link_state("up", Some(true), true, None), Connected);
        assert_eq!(link_state("up", Some(true), false, None), Disconnected);
        assert_eq!(
            link_state("up", Some(true), false, Some("configuring")),
            Connecting
        );
        assert_eq!(link_state("down", None, false, None), Unavailable);
        assert_eq!(
            link_state("lowerlayerdown", Some(false), true, None),
            Unavailable
        );
        assert_eq!(link_state("unknown", Some(false), true, None), Unavailable);
        assert_eq!(link_state("unknown", Some(true), true, None), Connected);
        assert_eq!(link_state("dormant", Some(true), false, None), Connecting);
    }

    /// A fake `/sys/class/net` with eth0 (cable, 1 Gbit/s), wlan0 and
    /// docker0 (virtual, left out).
    pub(crate) fn fixture(root: &Path, carrier: bool) {
        let net = root.join("sys/class/net");
        for (name, index, wireless, hardware) in [
            ("lo", 1, false, false),
            ("eth0", 2, false, true),
            ("wlan0", 3, true, true),
            ("docker0", 4, false, false),
        ] {
            let dir = net.join(name);
            std::fs::create_dir_all(&dir).unwrap();
            let w = |file: &str, text: &str| std::fs::write(dir.join(file), text).unwrap();
            w("ifindex", &format!("{index}\n"));
            w("type", if name == "lo" { "772\n" } else { "1\n" });
            w("address", &format!("00:00:5e:00:53:{index:02x}\n"));
            if hardware {
                std::fs::create_dir_all(dir.join("device")).unwrap();
            }
            if wireless {
                std::fs::create_dir_all(dir.join("wireless")).unwrap();
                w("operstate", "down\n");
            } else if name == "eth0" {
                w("operstate", if carrier { "up\n" } else { "down\n" });
                w("carrier", if carrier { "1\n" } else { "0\n" });
                w("speed", if carrier { "1000\n" } else { "-1\n" });
            } else {
                w("operstate", "up\n");
                w("carrier", "1\n");
            }
        }
        let proc = root.join("proc/net");
        std::fs::create_dir_all(&proc).unwrap();
        std::fs::write(proc.join("route"), if carrier { ROUTE } else { "Iface\n" }).unwrap();
        std::fs::write(proc.join("ipv6_route"), IPV6_ROUTE).unwrap();
        std::fs::write(proc.join("if_inet6"), IF_INET6).unwrap();
        std::fs::create_dir_all(root.join("etc")).unwrap();
        std::fs::write(root.join("etc/resolv.conf"), "nameserver 192.0.2.1\n").unwrap();
        std::fs::write(
            root.join("ipv4"),
            if carrier {
                "lo 127.0.0.1/8\neth0 192.0.2.23/24\ndocker0 172.17.0.1/16\n"
            } else {
                "lo 127.0.0.1/8\n"
            },
        )
        .unwrap();
    }

    fn temp(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("querencia-netzwerk-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn links_from_a_fixture_root() {
        let root = temp("wurzel");
        fixture(&root, true);
        let sources = Sources::below(&root);
        assert!(sources.is_test_root());
        let networkd = HashMap::from([(2, "configured".to_string())]);
        let links = read_links(&sources, &networkd, None).unwrap();
        let names: Vec<&str> = links.iter().map(|l| l.device.interface.as_str()).collect();
        assert_eq!(names, ["eth0", "wlan0"], "ohne lo und docker0");
        let eth = &links[0].device;
        assert_eq!(links[0].index, 2);
        assert_eq!(eth.kind, Kind::Wired);
        assert_eq!(eth.state, DeviceState::Connected);
        assert!(eth.primary);
        assert_eq!(eth.carrier, Some(true));
        assert_eq!(eth.speed_mbit, Some(1000));
        assert_eq!(eth.mac, "00:00:5e:00:53:02");
        assert_eq!(eth.ipv4[0].to_string(), "192.0.2.23/24");
        assert_eq!(eth.ipv6.len(), 2);
        assert_eq!(eth.gateway4, Some(Ipv4Addr::new(192, 0, 2, 1)));
        assert_eq!(eth.gateway6, Some("fe80::1".parse().unwrap()));
        assert_eq!(eth.dns, ["192.0.2.1".parse::<IpAddr>().unwrap()]);
        assert_eq!(eth.networkd_state.as_deref(), Some("configured"));
        let wlan = &links[1].device;
        assert_eq!(wlan.kind, Kind::Wireless);
        assert_eq!(wlan.state, DeviceState::Unavailable);
        // the fixture's route over the WLAN (metric 600) does not make it
        // the primary device: it is not connected
        assert!(!wlan.primary);
        assert_eq!(wlan.gateway4, Some(Ipv4Addr::new(192, 168, 2, 1)));

        // DNS from systemd-resolved instead of resolv.conf
        let dns = HashMap::from([(2, vec!["9.9.9.9".parse().unwrap()])]);
        let links = read_links(&sources, &networkd, Some(&dns)).unwrap();
        assert_eq!(links[0].device.dns, ["9.9.9.9".parse::<IpAddr>().unwrap()]);

        // cable pulled
        fixture(&root, false);
        let links = read_links(&sources, &HashMap::new(), None).unwrap();
        let eth = &links[0].device;
        assert_eq!(eth.state, DeviceState::Unavailable);
        assert_eq!(eth.state_text(), "Kabel nicht angeschlossen");
        assert_eq!(eth.speed_mbit, None);
        assert!(!eth.primary && eth.gateway4.is_none());
        std::fs::remove_dir_all(&root).unwrap();

        let missing = read_links(&Sources::below(&root), &HashMap::new(), None);
        assert!(missing.unwrap_err().contains("nicht lesbar"));
    }

    #[test]
    fn the_real_system_is_readable() {
        // Only reads; on the build machine eth0 or another link exists.
        let links =
            read_links(&Sources::system(), &HashMap::new(), None).expect("/sys/class/net lesbar");
        for link in links {
            assert!(link.index > 0);
            assert_ne!(link.device.interface, "lo");
        }
        let v4 = ipv4_addresses();
        assert!(
            v4.iter()
                .any(|(n, a)| n == "lo" && a.to_string() == "127.0.0.1/8")
        );
    }
}
