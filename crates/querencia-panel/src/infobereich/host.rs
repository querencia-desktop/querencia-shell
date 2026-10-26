//! The StatusNotifierHost of the panel: one per process, shared by the
//! notification area applets (there is one, but while `panel.ini` is read
//! again an old and a new one can live at the same time).
//!
//! - It offers the watcher (`watcher.rs`) while it lives, owns the name
//!   `org.kde.StatusNotifierHost-<pid>` and follows the owner of
//!   `org.kde.StatusNotifierWatcher` (its own watcher or another one): on
//!   every new owner it registers as host, subscribes to the watcher's
//!   signals and reads `RegisteredStatusNotifierItems`. When the owner
//!   goes, all items go; the programs register again with the next one.
//! - Per item it reads the properties (`GetAll`) and reads them again,
//!   debounced, after any of its signals (`NewIcon`, `NewStatus`,
//!   `NewToolTip`, `PropertiesChanged`, …).
//! - The applets show [`Host::items`] and are told about every change
//!   through the callbacks they registered ([`Host::beobachten`]).

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::time::Duration;

use gtk4::prelude::*;
use gtk4::{gio, glib};

use super::dbusmenu::Ziel;
use super::eintrag::{self, Eintrag};
use super::watcher::{self, Watcher};

/// Time limit of a call to an item or the watcher.
const TIMEOUT_MS: i32 = 3000;
/// Signals of an item within this time lead to one read of its properties
/// (animated icons send `NewIcon` many times a second).
const ENTPRELLEN_MS: u64 = 40;

/// One item on the bus.
pub struct Item {
    /// The ID in the watcher.
    pub key: String,
    pub bus: String,
    pub path: String,
    connection: gio::DBusConnection,
    /// What was read last; `None` until the first answer (not shown).
    eintrag: RefCell<Option<Eintrag>>,
    /// The item's interface (KDE name, or the freedesktop one).
    interface: Cell<&'static str>,
    pending: RefCell<Option<glib::SourceId>>,
    signale: RefCell<Option<gio::SignalSubscription>>,
    host: Weak<Host>,
}

/// The process's host.
pub struct Host {
    connection: gio::DBusConnection,
    _watcher: Rc<Watcher>,
    host_name: String,
    owner: RefCell<Option<gio::OwnerId>>,
    /// Ends the watch of the watcher's name.
    unwatch: RefCell<Option<Box<dyn FnOnce()>>>,
    /// Signals of the current watcher.
    watcher_signale: RefCell<Vec<gio::SignalSubscription>>,
    /// Unique name of the current watcher.
    watcher_owner: RefCell<Option<String>>,
    items: RefCell<Vec<Rc<Item>>>,
    beobachter: RefCell<Vec<Weak<dyn Fn()>>>,
}

thread_local! {
    static HOST: RefCell<Weak<Host>> = const { RefCell::new(Weak::new()) };
}

impl Host {
    /// The host of this process, started on first use; `None` without a
    /// session bus. It ends when the last applet drops it.
    pub fn holen() -> Option<Rc<Host>> {
        if let Some(host) = HOST.with(|h| h.borrow().upgrade()) {
            return Some(host);
        }
        let connection = match gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE) {
            Ok(connection) => connection,
            Err(err) => {
                eprintln!("querencia-panel: Infobereich: kein Sitzungsbus: {err}");
                return None;
            }
        };
        let host = Rc::new(Host {
            _watcher: Watcher::anbieten(&connection),
            host_name: format!("org.kde.StatusNotifierHost-{}", std::process::id()),
            connection,
            owner: RefCell::default(),
            unwatch: RefCell::default(),
            watcher_signale: RefCell::default(),
            watcher_owner: RefCell::default(),
            items: RefCell::default(),
            beobachter: RefCell::default(),
        });
        host.starten();
        HOST.with(|h| *h.borrow_mut() = Rc::downgrade(&host));
        Some(host)
    }

    fn starten(self: &Rc<Self>) {
        let owner = gio::bus_own_name_on_connection(
            &self.connection,
            &self.host_name,
            gio::BusNameOwnerFlags::DO_NOT_QUEUE,
            |_, _| {},
            |_, name| eprintln!("querencia-panel: Infobereich: Busname {name} nicht erhalten"),
        );
        self.owner.replace(Some(owner));
        let appeared = Rc::downgrade(self);
        let vanished = Rc::downgrade(self);
        let watch = gio::bus_watch_name_on_connection(
            &self.connection,
            watcher::BUS_NAME,
            gio::BusNameWatcherFlags::NONE,
            move |_, _, owner| {
                if let Some(host) = appeared.upgrade() {
                    host.watcher_da(owner);
                }
            },
            move |_, _| {
                if let Some(host) = vanished.upgrade() {
                    host.watcher_weg();
                }
            },
        );
        self.unwatch
            .replace(Some(Box::new(move || gio::bus_unwatch_name(watch))));
    }

    /// Registers `callback`, called after every change of the items; it is
    /// dropped with the `Rc` the caller keeps.
    pub fn beobachten(&self, callback: &Rc<dyn Fn()>) {
        let mut list = self.beobachter.borrow_mut();
        list.retain(|w| w.strong_count() > 0);
        list.push(Rc::downgrade(callback));
    }

    fn melden(&self) {
        let callbacks: Vec<_> = self
            .beobachter
            .borrow()
            .iter()
            .filter_map(Weak::upgrade)
            .collect();
        for callback in callbacks {
            callback();
        }
    }

    /// The items in the watcher's order, with what they said last.
    pub fn items(&self) -> Vec<(Rc<Item>, Eintrag)> {
        self.items
            .borrow()
            .iter()
            .filter_map(|item| Some((item.clone(), item.eintrag.borrow().clone()?)))
            .collect()
    }

    fn watcher_da(self: &Rc<Self>, owner: &str) {
        eprintln!(
            "querencia-panel: Infobereich: Watcher {} ist {owner}",
            watcher::BUS_NAME
        );
        self.watcher_weg_still();
        self.watcher_owner.replace(Some(owner.to_owned()));
        let mut signale = Vec::new();
        for (member, neu) in [
            ("StatusNotifierItemRegistered", true),
            ("StatusNotifierItemUnregistered", false),
        ] {
            let weak = Rc::downgrade(self);
            signale.push(self.connection.subscribe_to_signal(
                Some(owner),
                Some(watcher::INTERFACE),
                Some(member),
                Some(watcher::OBJECT_PATH),
                None,
                gio::DBusSignalFlags::NONE,
                move |signal| {
                    let Some(host) = weak.upgrade() else { return };
                    let Some(id) = signal.parameters.child_value(0).str().map(str::to_owned) else {
                        return;
                    };
                    if neu {
                        host.item_dazu(&id);
                    } else {
                        host.item_weg(&id);
                    }
                },
            ));
        }
        self.watcher_signale.replace(signale);
        let weak = Rc::downgrade(self);
        let owner_name = owner.to_owned();
        self.connection.call(
            Some(owner),
            watcher::OBJECT_PATH,
            watcher::INTERFACE,
            "RegisterStatusNotifierHost",
            Some(&(self.host_name.as_str(),).to_variant()),
            None,
            gio::DBusCallFlags::NO_AUTO_START,
            TIMEOUT_MS,
            gio::Cancellable::NONE,
            move |result| {
                let Some(host) = weak.upgrade() else { return };
                if let Err(err) = result {
                    eprintln!("querencia-panel: Infobereich: als Host nicht angemeldet: {err}");
                }
                host.items_lesen(&owner_name);
            },
        );
    }

    /// Reads the watcher's item list and takes it over.
    fn items_lesen(self: &Rc<Self>, owner: &str) {
        let weak = Rc::downgrade(self);
        let owner_name = owner.to_owned();
        self.connection.call(
            Some(owner),
            watcher::OBJECT_PATH,
            "org.freedesktop.DBus.Properties",
            "Get",
            Some(&(watcher::INTERFACE, "RegisteredStatusNotifierItems").to_variant()),
            Some(glib::VariantTy::new("(v)").expect("Signatur")),
            gio::DBusCallFlags::NO_AUTO_START,
            TIMEOUT_MS,
            gio::Cancellable::NONE,
            move |result| {
                let Some(host) = weak.upgrade() else { return };
                if host.watcher_owner.borrow().as_deref() != Some(owner_name.as_str()) {
                    return;
                }
                let ids = match result {
                    Ok(antwort) => antwort
                        .child_value(0)
                        .as_variant()
                        .and_then(|v| v.get::<Vec<String>>())
                        .unwrap_or_default(),
                    Err(err) => {
                        eprintln!("querencia-panel: Infobereich: Itemliste nicht lesbar: {err}");
                        Vec::new()
                    }
                };
                eprintln!(
                    "querencia-panel: Infobereich: {} Items beim Watcher",
                    ids.len()
                );
                for id in &ids {
                    host.item_dazu(id);
                }
            },
        );
    }

    fn watcher_weg_still(&self) {
        self.watcher_signale.borrow_mut().clear();
        self.watcher_owner.replace(None);
        let alt: Vec<_> = self.items.borrow_mut().drain(..).collect();
        for item in alt {
            item.ende();
        }
    }

    fn watcher_weg(&self) {
        eprintln!(
            "querencia-panel: Infobereich: kein Watcher {} (Symbole weg bis zum nächsten)",
            watcher::BUS_NAME
        );
        self.watcher_weg_still();
        self.melden();
    }

    fn item_dazu(self: &Rc<Self>, key: &str) {
        if self.items.borrow().iter().any(|i| i.key == key) {
            return;
        }
        let Some((bus, path)) = eintrag::adresse(key) else {
            eprintln!("querencia-panel: Infobereich: Item-ID {key:?} unlesbar, übergangen");
            return;
        };
        let item = Rc::new(Item {
            key: key.to_owned(),
            bus,
            path,
            connection: self.connection.clone(),
            eintrag: RefCell::default(),
            interface: Cell::new(eintrag::INTERFACE),
            pending: RefCell::default(),
            signale: RefCell::default(),
            host: Rc::downgrade(self),
        });
        let weak = Rc::downgrade(&item);
        let signale = self.connection.subscribe_to_signal(
            Some(&item.bus),
            None,
            None,
            Some(&item.path),
            None,
            gio::DBusSignalFlags::NONE,
            move |signal| {
                if let Some(item) = weak.upgrade()
                    && (signal.signal_name.starts_with("New")
                        || signal.signal_name == "PropertiesChanged")
                {
                    item.spaeter_lesen();
                }
            },
        );
        item.signale.replace(Some(signale));
        eprintln!("querencia-panel: Infobereich: Item {key}");
        self.items.borrow_mut().push(item.clone());
        item.lesen();
    }

    fn item_weg(&self, key: &str) {
        let gone: Vec<_> = {
            let mut items = self.items.borrow_mut();
            let (gone, keep) = items.drain(..).partition(|i| i.key == key);
            *items = keep;
            gone
        };
        if gone.is_empty() {
            return;
        }
        for item in gone {
            item.ende();
        }
        eprintln!("querencia-panel: Infobereich: Item {key} weg");
        self.melden();
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        if let Some(unwatch) = self.unwatch.take() {
            unwatch();
        }
        if let Some(id) = self.owner.take() {
            gio::bus_unown_name(id);
        }
        for item in self.items.take() {
            item.ende();
        }
        eprintln!("querencia-panel: Infobereich: Host beendet");
    }
}

impl Item {
    fn ende(&self) {
        self.signale.borrow_mut().take();
        if let Some(id) = self.pending.borrow_mut().take() {
            id.remove();
        }
    }

    fn spaeter_lesen(self: &Rc<Self>) {
        if self.pending.borrow().is_some() {
            return;
        }
        let weak = Rc::downgrade(self);
        let id = glib::timeout_add_local_once(Duration::from_millis(ENTPRELLEN_MS), move || {
            if let Some(item) = weak.upgrade() {
                item.pending.borrow_mut().take();
                item.lesen();
            }
        });
        self.pending.replace(Some(id));
    }

    /// Reads all properties; the first time with the KDE interface, on an
    /// error once more with the freedesktop name.
    fn lesen(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        let interface = self.interface.get();
        self.connection.call(
            Some(&self.bus),
            &self.path,
            "org.freedesktop.DBus.Properties",
            "GetAll",
            Some(&(interface,).to_variant()),
            Some(glib::VariantTy::new("(a{sv})").expect("Signatur")),
            gio::DBusCallFlags::NO_AUTO_START,
            TIMEOUT_MS,
            gio::Cancellable::NONE,
            move |result| {
                let Some(item) = weak.upgrade() else { return };
                let gelesen = result
                    .map_err(|e| e.to_string())
                    .and_then(|antwort| {
                        Eintrag::from_properties(&antwort.child_value(0))
                            .ok_or_else(|| "Eigenschaften unlesbar".to_owned())
                    })
                    .and_then(|e| {
                        // an empty answer: this interface is not the one
                        if e == Eintrag::default() {
                            Err("keine Eigenschaften".to_owned())
                        } else {
                            Ok(e)
                        }
                    });
                match gelesen {
                    Ok(neu) => {
                        let geaendert = item.eintrag.borrow().as_ref() != Some(&neu);
                        if geaendert {
                            eprintln!(
                                "querencia-panel: Infobereich: {} „{}“ {:?}, Symbol {}{}, Menü {}",
                                item.key,
                                neu.id,
                                neu.status,
                                if neu.icon_name.is_empty() {
                                    "-"
                                } else {
                                    &neu.icon_name
                                },
                                if neu.icon_pixmaps.is_empty() {
                                    ""
                                } else {
                                    " +Pixmap"
                                },
                                neu.menu.as_deref().unwrap_or("-")
                            );
                            item.eintrag.replace(Some(neu));
                            if let Some(host) = item.host.upgrade() {
                                host.melden();
                            }
                        }
                    }
                    Err(err) if interface == eintrag::INTERFACE => {
                        eprintln!(
                            "querencia-panel: Infobereich: {} ohne {} ({err}), versuche {}",
                            item.key,
                            eintrag::INTERFACE,
                            eintrag::INTERFACE_FREEDESKTOP
                        );
                        item.interface.set(eintrag::INTERFACE_FREEDESKTOP);
                        item.lesen();
                    }
                    Err(err) => {
                        eprintln!(
                            "querencia-panel: Infobereich: {} nicht lesbar ({err}), nicht gezeigt",
                            item.key
                        );
                        item.interface.set(eintrag::INTERFACE);
                    }
                }
            },
        );
    }

    /// Calls `method` of the item; `fehler` gets an error (else logged).
    pub fn aufrufen(
        &self,
        method: &'static str,
        args: glib::Variant,
        fehler: impl FnOnce(&glib::Error) + 'static,
    ) {
        let key = self.key.clone();
        self.connection.call(
            Some(&self.bus),
            &self.path,
            self.interface.get(),
            method,
            Some(&args),
            None,
            gio::DBusCallFlags::NO_AUTO_START,
            TIMEOUT_MS,
            gio::Cancellable::NONE,
            move |result| {
                if let Err(err) = result {
                    eprintln!(
                        "querencia-panel: Infobereich: {key}: {method} fehlgeschlagen: {err}"
                    );
                    fehler(&err);
                }
            },
        );
    }

    /// Hands an xdg-activation token to the item before an activation
    /// (`ProvideXdgActivationToken`, KDE; programs without it answer with
    /// an error, which is not logged).
    pub fn token_geben(&self, token: &str) {
        if token.is_empty() {
            return;
        }
        self.connection.call(
            Some(&self.bus),
            &self.path,
            self.interface.get(),
            "ProvideXdgActivationToken",
            Some(&(token,).to_variant()),
            None,
            gio::DBusCallFlags::NO_AUTO_START,
            TIMEOUT_MS,
            gio::Cancellable::NONE,
            |_| {},
        );
    }

    /// Where the item's menu lives, if it has one.
    pub fn menu_ziel(&self) -> Option<Ziel> {
        let path = self.eintrag.borrow().as_ref()?.menu.clone()?;
        Some(Ziel {
            connection: self.connection.clone(),
            bus: self.bus.clone(),
            path,
        })
    }
}
