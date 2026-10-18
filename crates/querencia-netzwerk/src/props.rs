//! Reading D-Bus property dictionaries (`a{sv}`) and object lists, as
//! GLib variants, without panicking on a missing or mistyped value.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use gtk4::glib::{self, Variant};

/// The properties of one interface of one object (`a{sv}`), possibly none.
#[derive(Debug, Clone, Default)]
pub struct Props(pub Option<Variant>);

impl Props {
    /// The value of `key`, unboxed from its `v`.
    pub fn get(&self, key: &str) -> Option<Variant> {
        lookup(self.0.as_ref()?, key)
    }

    /// A string or object path; `None` for `/` (NetworkManager's "none").
    pub fn path(&self, key: &str) -> Option<String> {
        self.string(key).filter(|p| p != "/" && !p.is_empty())
    }

    pub fn string(&self, key: &str) -> Option<String> {
        self.get(key)?.str().map(str::to_string)
    }

    pub fn u32(&self, key: &str) -> Option<u32> {
        self.get(key)?.get::<u32>()
    }

    pub fn u8(&self, key: &str) -> Option<u8> {
        self.get(key)?.get::<u8>()
    }

    pub fn bool(&self, key: &str) -> Option<bool> {
        self.get(key)?.get::<bool>()
    }

    /// An `ao` or `as`, in order, without `/`.
    pub fn paths(&self, key: &str) -> Vec<String> {
        strings(self.get(key).as_ref())
            .into_iter()
            .filter(|p| p != "/")
            .collect()
    }

    pub fn bytes(&self, key: &str) -> Vec<u8> {
        self.get(key).map(|v| bytes(&v)).unwrap_or_default()
    }

    /// An `aa{sv}`, one [`Props`] per element.
    pub fn list(&self, key: &str) -> Vec<Props> {
        match self.get(key) {
            Some(v) if v.is_container() => v.iter().map(|d| Props(Some(d))).collect(),
            _ => Vec::new(),
        }
    }
}

/// The strings of an `as` or `ao`.
pub fn strings(value: Option<&Variant>) -> Vec<String> {
    match value {
        Some(v) if v.is_container() => v
            .iter()
            .filter_map(|s| s.str().map(str::to_string))
            .collect(),
        _ => Vec::new(),
    }
}

/// The bytes of an `ay`.
pub fn bytes(value: &Variant) -> Vec<u8> {
    if value.type_() == glib::VariantTy::BYTE_STRING {
        value
            .fixed_array::<u8>()
            .map(<[u8]>::to_vec)
            .unwrap_or_default()
    } else {
        Vec::new()
    }
}

/// An IP address from text; also takes an address with a `%zone`.
pub fn ip(text: &str) -> Option<IpAddr> {
    text.split('%').next()?.parse().ok()
}

/// An IPv4 address NetworkManager sends as `u` in network byte order.
pub fn ipv4_from_u32(value: u32) -> Ipv4Addr {
    Ipv4Addr::from(value.to_le_bytes())
}

/// An IPv6 address from 16 bytes.
pub fn ipv6_from_bytes(bytes: &[u8]) -> Option<Ipv6Addr> {
    <[u8; 16]>::try_from(bytes).ok().map(Ipv6Addr::from)
}

/// The objects of a `GetManagedObjects` reply body `a{oa{sa{sv}}}` (or the
/// dictionary itself): path and its interfaces (`a{sa{sv}}`).
pub fn objects(reply: &Variant) -> Vec<(String, Variant)> {
    let dict = if reply.type_().is_tuple() && reply.n_children() == 1 {
        reply.child_value(0)
    } else {
        reply.clone()
    };
    if !dict.is_container() {
        return Vec::new();
    }
    dict.iter()
        .filter(|entry| entry.n_children() == 2)
        .filter_map(|entry| {
            let path = entry.child_value(0).str()?.to_string();
            Some((path, entry.child_value(1)))
        })
        .collect()
}

/// The value of `key` in a dictionary with string keys (`a{s*}`), unboxed
/// if it is a `v`.
pub fn lookup(dict: &Variant, key: &str) -> Option<Variant> {
    if !dict.is_container() || !dict.type_().is_array() {
        return None;
    }
    let entry = dict
        .iter()
        .find(|e| e.n_children() == 2 && e.child_value(0).str() == Some(key))?;
    Some(unbox(entry.child_value(1)))
}

/// The value inside a `v`, or `value` itself if it is none.
pub fn unbox(value: Variant) -> Variant {
    if value.type_() == glib::VariantTy::VARIANT {
        value.as_variant().unwrap_or(value)
    } else {
        value
    }
}

/// The properties of interface `name` in an interface dictionary.
pub fn interface(interfaces: &Variant, name: &str) -> Props {
    Props(lookup(interfaces, name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use glib::prelude::*;
    use std::collections::HashMap;

    #[test]
    fn props_read_typed_values_and_survive_wrong_types() {
        let mut map: HashMap<String, Variant> = HashMap::new();
        map.insert("Interface".into(), "eth0".to_variant());
        map.insert("State".into(), 100u32.to_variant());
        map.insert("Strength".into(), 73u8.to_variant());
        map.insert("Carrier".into(), true.to_variant());
        map.insert("Ssid".into(), b"Heim".to_vec().to_variant());
        map.insert(
            "Devices".into(),
            vec!["/a".to_string(), "/".to_string(), "/b".to_string()].to_variant(),
        );
        map.insert("Ip4Config".into(), "/".to_variant());
        let props = Props(Some(map.to_variant()));
        assert_eq!(props.string("Interface").as_deref(), Some("eth0"));
        assert_eq!(props.u32("State"), Some(100));
        assert_eq!(props.u8("Strength"), Some(73));
        assert_eq!(props.bool("Carrier"), Some(true));
        assert_eq!(props.bytes("Ssid"), b"Heim");
        assert_eq!(props.paths("Devices"), ["/a", "/b"]);
        assert_eq!(props.path("Ip4Config"), None, "/ heißt keins");
        assert_eq!(props.u32("Interface"), None, "falscher Typ");
        assert_eq!(props.string("Fehlt"), None);
        assert!(Props(None).paths("x").is_empty());
        assert_eq!(Props(Some(5u32.to_variant())).u32("x"), None);
    }

    #[test]
    fn addresses_from_networkmanager_forms() {
        assert_eq!(ipv4_from_u32(0x010200c0), Ipv4Addr::new(192, 0, 2, 1));
        assert_eq!(
            ipv6_from_bytes(&[0xfe, 0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]),
            Some("fe80::1".parse().unwrap())
        );
        assert_eq!(ipv6_from_bytes(&[1, 2, 3]), None);
        assert_eq!(ip("fe80::1%eth0"), Some("fe80::1".parse().unwrap()));
        assert_eq!(ip("kaputt"), None);
    }
}
