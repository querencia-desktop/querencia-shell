//! The StatusNotifierWatcher: the list of items and hosts on the session
//! bus that every program with a status icon looks for. The panel offers
//! it only while nobody else does: the name `org.kde.StatusNotifierWatcher`
//! is requested with queueing, so a watcher that is already there keeps
//! it, and the panel takes over when it ends. Programs then register again
//! on their own (they follow the owner of the name). Once the panel has
//! the KDE name it also asks for `org.freedesktop.StatusNotifierWatcher`
//! (a few programs use that one), the same object under both names.
//!
//! - `RegisterStatusNotifierItem(s)`: a bus name, or an object path of the
//!   calling connection (libappindicator); the ID is `<bus name><path>`
//!   as KDE's watcher forms it. The bus name must have an owner; an item
//!   goes when its bus name loses the owner.
//! - `RegisterStatusNotifierHost(s)`: the calling connection counts as a
//!   host until it ends.
//! - properties `RegisteredStatusNotifierItems`,
//!   `IsStatusNotifierHostRegistered`, `ProtocolVersion` (0); signals
//!   `StatusNotifierItemRegistered`, `…Unregistered`,
//!   `StatusNotifierHostRegistered`, `…Unregistered`.
//!
//! The panel's own host (`host.rs`) talks to whichever watcher owns the
//! name, also to this one, over the bus.

use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{gio, glib};

use super::eintrag::DEFAULT_PATH;

pub const BUS_NAME: &str = "org.kde.StatusNotifierWatcher";
pub const BUS_NAME_FREEDESKTOP: &str = "org.freedesktop.StatusNotifierWatcher";
pub const OBJECT_PATH: &str = "/StatusNotifierWatcher";
pub const INTERFACE: &str = "org.kde.StatusNotifierWatcher";
pub const INTERFACE_FREEDESKTOP: &str = "org.freedesktop.StatusNotifierWatcher";

const INTROSPECTION: &str = r#"<node>
  <interface name="org.kde.StatusNotifierWatcher">
    <method name="RegisterStatusNotifierItem"><arg name="service" type="s" direction="in"/></method>
    <method name="RegisterStatusNotifierHost"><arg name="service" type="s" direction="in"/></method>
    <property name="RegisteredStatusNotifierItems" type="as" access="read"/>
    <property name="IsStatusNotifierHostRegistered" type="b" access="read"/>
    <property name="ProtocolVersion" type="i" access="read"/>
    <signal name="StatusNotifierItemRegistered"><arg type="s"/></signal>
    <signal name="StatusNotifierItemUnregistered"><arg type="s"/></signal>
    <signal name="StatusNotifierHostRegistered"/>
    <signal name="StatusNotifierHostUnregistered"/>
  </interface>
</node>"#;

/// The items and hosts known, in the order they registered.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Register {
    /// (ID, bus name the item lives on).
    pub items: Vec<(String, String)>,
    /// Unique names of the host connections.
    pub hosts: Vec<String>,
}

/// What an item registration means: its ID and the bus name to watch;
/// `None` for an argument that is neither a bus name nor an object path.
pub fn item_id(service: &str, sender: &str) -> Option<(String, String)> {
    if service.starts_with('/') {
        if !glib::Variant::is_object_path(service) || !gio::dbus_is_unique_name(sender) {
            return None;
        }
        return Some((format!("{sender}{service}"), sender.to_owned()));
    }
    if !gio::dbus_is_name(service) {
        return None;
    }
    Some((format!("{service}{DEFAULT_PATH}"), service.to_owned()))
}

impl Register {
    /// Adds an item; `false` if it was known already.
    pub fn item_dazu(&mut self, id: &str, bus: &str) -> bool {
        if self.items.iter().any(|(known, _)| known == id) {
            return false;
        }
        self.items.push((id.to_owned(), bus.to_owned()));
        true
    }

    /// Adds a host; `false` if it was known already.
    pub fn host_dazu(&mut self, name: &str) -> bool {
        if self.hosts.iter().any(|h| h == name) {
            return false;
        }
        self.hosts.push(name.to_owned());
        true
    }

    /// The bus name `name` lost its owner: the IDs of the items that go,
    /// and whether a host went.
    pub fn weg(&mut self, name: &str) -> (Vec<String>, bool) {
        let mut gone = Vec::new();
        self.items.retain(|(id, bus)| {
            let keep = bus != name;
            if !keep {
                gone.push(id.clone());
            }
            keep
        });
        let hosts = self.hosts.len();
        self.hosts.retain(|h| h != name);
        (gone, self.hosts.len() != hosts)
    }

    pub fn ids(&self) -> Vec<String> {
        self.items.iter().map(|(id, _)| id.clone()).collect()
    }
}

/// The offered watcher; everything is given back when it is dropped.
pub struct Watcher {
    connection: gio::DBusConnection,
    register: Rc<RefCell<Register>>,
    registrations: RefCell<Vec<gio::RegistrationId>>,
    owners: RefCell<Vec<gio::OwnerId>>,
    _owner_changes: gio::SignalSubscription,
}

impl Watcher {
    /// Exports the watcher on `connection` and asks for both names.
    pub fn anbieten(connection: &gio::DBusConnection) -> Rc<Watcher> {
        let register = Rc::new(RefCell::new(Register::default()));
        // items and hosts go when their bus name loses its owner
        let weak_register = Rc::downgrade(&register);
        let conn = connection.clone();
        let owner_changes = connection.subscribe_to_signal(
            Some("org.freedesktop.DBus"),
            Some("org.freedesktop.DBus"),
            Some("NameOwnerChanged"),
            Some("/org/freedesktop/DBus"),
            None,
            gio::DBusSignalFlags::NONE,
            move |signal| {
                let Some(register) = weak_register.upgrade() else {
                    return;
                };
                let Some((name, _, neu)) = signal.parameters.get::<(String, String, String)>()
                else {
                    return;
                };
                if !neu.is_empty() {
                    return;
                }
                let (gone, host_gone) = register.borrow_mut().weg(&name);
                for id in gone {
                    eprintln!("querencia-panel: Infobereich: Watcher: {id} abgemeldet");
                    signal_senden(
                        &conn,
                        "StatusNotifierItemUnregistered",
                        Some((id,).to_variant()),
                    );
                }
                if host_gone {
                    eprintln!("querencia-panel: Infobereich: Watcher: Host {name} abgemeldet");
                    signal_senden(&conn, "StatusNotifierHostUnregistered", None);
                }
            },
        );
        let watcher = Rc::new(Watcher {
            connection: connection.clone(),
            register,
            registrations: RefCell::default(),
            owners: RefCell::default(),
            _owner_changes: owner_changes,
        });
        let node = gio::DBusNodeInfo::for_xml(INTROSPECTION).expect("Schnittstelle");
        let kde = node.lookup_interface(INTERFACE).expect("Schnittstelle");
        // the same interface under its freedesktop name
        let freedesktop =
            gio::DBusNodeInfo::for_xml(&INTROSPECTION.replace(INTERFACE, INTERFACE_FREEDESKTOP))
                .ok()
                .and_then(|n| n.lookup_interface(INTERFACE_FREEDESKTOP));
        for info in std::iter::once(kde).chain(freedesktop) {
            match watcher.exportieren(&info) {
                Ok(id) => watcher.registrations.borrow_mut().push(id),
                Err(err) => eprintln!(
                    "querencia-panel: Infobereich: Watcher {} nicht angeboten: {err}",
                    info.name()
                ),
            }
        }
        // The freedesktop alias only together with the KDE name: the host
        // follows the KDE watcher, so an item at a freedesktop alias of
        // another program than that watcher would never be shown.
        let weak = Rc::downgrade(&watcher);
        let id = gio::bus_own_name_on_connection(
            connection,
            BUS_NAME,
            gio::BusNameOwnerFlags::NONE,
            move |connection, name| {
                eprintln!("querencia-panel: Infobereich: Watcher {name} übernommen");
                let Some(watcher) = weak.upgrade() else {
                    return;
                };
                if watcher.owners.borrow().len() > 1 {
                    return;
                }
                let alias = gio::bus_own_name_on_connection(
                    &connection,
                    BUS_NAME_FREEDESKTOP,
                    gio::BusNameOwnerFlags::NONE,
                    |_, name| eprintln!("querencia-panel: Infobereich: Watcher {name} übernommen"),
                    |_, name| {
                        eprintln!(
                            "querencia-panel: Infobereich: {name} hat ein anderes Programm \
                             (nur der KDE-Name ist vom Panel)"
                        )
                    },
                );
                watcher.owners.borrow_mut().push(alias);
            },
            |_, name| {
                eprintln!(
                    "querencia-panel: Infobereich: {name} hat ein anderes Programm, \
                     das Panel wartet (nur Host)"
                )
            },
        );
        watcher.owners.borrow_mut().push(id);
        watcher
    }

    fn exportieren(
        self: &Rc<Self>,
        info: &gio::DBusInterfaceInfo,
    ) -> Result<gio::RegistrationId, glib::Error> {
        let methods = Rc::downgrade(self);
        let properties = Rc::downgrade(self);
        self.connection
            .register_object(OBJECT_PATH, info)
            .method_call(move |_, sender, _, _, method, params, invocation| {
                let Some(watcher) = methods.upgrade() else {
                    invocation.return_error(gio::DBusError::Failed, "Watcher beendet");
                    return;
                };
                let service = params.child_value(0).str().unwrap_or_default().to_owned();
                let sender = sender.unwrap_or_default().to_owned();
                match method {
                    "RegisterStatusNotifierItem" => watcher.item(&service, &sender, invocation),
                    "RegisterStatusNotifierHost" => watcher.host(&sender, invocation),
                    _ => invocation.return_error(
                        gio::DBusError::UnknownMethod,
                        &format!("{method} gibt es nicht"),
                    ),
                }
            })
            .property(move |_, _, _, _, name| {
                let Some(watcher) = properties.upgrade() else {
                    return false.to_variant();
                };
                let register = watcher.register.borrow();
                match name {
                    "RegisteredStatusNotifierItems" => register.ids().to_variant(),
                    "IsStatusNotifierHostRegistered" => (!register.hosts.is_empty()).to_variant(),
                    _ => 0i32.to_variant(),
                }
            })
            .build()
    }

    /// `RegisterStatusNotifierItem`: answered once the owner of the item's
    /// bus name is known.
    fn item(self: &Rc<Self>, service: &str, sender: &str, invocation: gio::DBusMethodInvocation) {
        let Some((id, bus)) = item_id(service, sender) else {
            invocation.return_error(
                gio::DBusError::InvalidArgs,
                &format!("kein Busname und kein Objektpfad: {service:?}"),
            );
            return;
        };
        let weak = Rc::downgrade(self);
        self.connection.call(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "GetNameOwner",
            Some(&(bus.as_str(),).to_variant()),
            Some(glib::VariantTy::new("(s)").expect("Signatur")),
            gio::DBusCallFlags::NONE,
            3000,
            gio::Cancellable::NONE,
            move |result| {
                let Some(watcher) = weak.upgrade() else {
                    invocation.return_error(gio::DBusError::Failed, "Watcher beendet");
                    return;
                };
                if let Err(err) = result {
                    eprintln!(
                        "querencia-panel: Infobereich: Watcher: {id} ohne Besitzer, abgelehnt"
                    );
                    invocation.return_error(
                        gio::DBusError::ServiceUnknown,
                        &format!("{bus} hat keinen Besitzer: {err}"),
                    );
                    return;
                }
                let neu = watcher.register.borrow_mut().item_dazu(&id, &bus);
                invocation.return_value(None);
                if neu {
                    eprintln!("querencia-panel: Infobereich: Watcher: {id} angemeldet");
                    signal_senden(
                        &watcher.connection,
                        "StatusNotifierItemRegistered",
                        Some((id,).to_variant()),
                    );
                }
            },
        );
    }

    fn host(&self, sender: &str, invocation: gio::DBusMethodInvocation) {
        let neu = self.register.borrow_mut().host_dazu(sender);
        invocation.return_value(None);
        if neu {
            eprintln!("querencia-panel: Infobereich: Watcher: Host {sender} angemeldet");
            signal_senden(&self.connection, "StatusNotifierHostRegistered", None);
        }
    }
}

/// Sends `signal` of the watcher under both interface names.
fn signal_senden(connection: &gio::DBusConnection, signal: &str, args: Option<glib::Variant>) {
    for interface in [INTERFACE, INTERFACE_FREEDESKTOP] {
        if let Err(err) =
            connection.emit_signal(None, OBJECT_PATH, interface, signal, args.as_ref())
        {
            eprintln!("querencia-panel: Infobereich: Signal {signal} nicht gesendet: {err}");
        }
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        for id in self.owners.take() {
            gio::bus_unown_name(id);
        }
        for id in self.registrations.take() {
            let _ = self.connection.unregister_object(id);
        }
        eprintln!("querencia-panel: Infobereich: Watcher beendet");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_ids_as_kde_forms_them() {
        assert_eq!(
            item_id("org.kde.StatusNotifierItem-4-1", ":1.9"),
            Some((
                "org.kde.StatusNotifierItem-4-1/StatusNotifierItem".into(),
                "org.kde.StatusNotifierItem-4-1".into()
            ))
        );
        assert_eq!(
            item_id("/org/ayatana/NotificationItem/x", ":1.9"),
            Some((":1.9/org/ayatana/NotificationItem/x".into(), ":1.9".into()))
        );
        assert_eq!(
            item_id(":1.12", ":1.12"),
            Some((":1.12/StatusNotifierItem".into(), ":1.12".into()))
        );
        assert_eq!(item_id("kein name", ":1.9"), None);
        assert_eq!(item_id("/a//b", ":1.9"), None);
        assert_eq!(item_id("/a", "org.kein.unique"), None);
        assert_eq!(item_id("", ":1.9"), None);
    }

    #[test]
    fn register_keeps_order_once_and_drops_by_bus_name() {
        let mut r = Register::default();
        assert!(r.item_dazu("a/StatusNotifierItem", "a"));
        assert!(r.item_dazu(":1.5/x", ":1.5"));
        assert!(r.item_dazu(":1.5/y", ":1.5"));
        assert!(!r.item_dazu("a/StatusNotifierItem", "a"));
        assert_eq!(r.ids(), ["a/StatusNotifierItem", ":1.5/x", ":1.5/y"]);
        assert!(r.host_dazu(":1.2"));
        assert!(!r.host_dazu(":1.2"));
        assert_eq!(
            r.weg(":1.5"),
            (vec![":1.5/x".to_owned(), ":1.5/y".to_owned()], false)
        );
        assert_eq!(r.weg(":1.2"), (vec![], true));
        assert_eq!(r.weg(":1.2"), (vec![], false));
        assert_eq!(r.ids(), ["a/StatusNotifierItem"]);
    }

    #[test]
    fn introspection_has_both_interfaces() {
        let kde = gio::DBusNodeInfo::for_xml(INTROSPECTION).unwrap();
        assert!(kde.lookup_interface(INTERFACE).is_some());
        let fd =
            gio::DBusNodeInfo::for_xml(&INTROSPECTION.replace(INTERFACE, INTERFACE_FREEDESKTOP))
                .unwrap();
        let fd = fd.lookup_interface(INTERFACE_FREEDESKTOP).unwrap();
        assert!(fd.lookup_method("RegisterStatusNotifierItem").is_some());
        assert!(fd.lookup_property("ProtocolVersion").is_some());
    }
}
