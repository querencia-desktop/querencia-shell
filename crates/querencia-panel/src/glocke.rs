//! The bell of the notification centre (G2-AL, QD-073 part 2): a status
//! icon next to volume and network that shows how many notifications are
//! unread in Querencia Benachrichtigungen (0.2.0 and later).
//!
//! - left click: opens the Mitteilungszentrale (`ZentraleZeigen` with the
//!   click's xdg-activation token), or closes it while it is open;
//! - right click (through `panel_menu.rs`, [`menu`]): „Nicht stören“ as a
//!   check item (property `NichtStoeren`), „Alle als gelesen markieren“
//!   (`AlsGelesen []`), „Aus dem Panel entfernen“;
//! - tooltip: the number of unread notifications, and „Nicht stören“.
//!
//! The service's bus name `org.querencia.Benachrichtigungen` is watched
//! (`NameOwnerChanged` through `g_bus_watch_name`, never auto-started);
//! the count is read once (`Properties.GetAll`) and then follows the
//! signals `PropertiesChanged` and `Zaehler`, no polling. Without an owner
//! (no service, or 0.1.x, which does not own the name) the bell is greyed
//! out without a count and the log says so once per change.
//!
//! Interface: `docs/dbus.md` of querencia-benachrichtigungen 0.2.0.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Button, Image, Label, Orientation, gdk, gio, glib};

use crate::logo;

/// CSS class of the applet's button; its look is the one of all status
/// applets (`panel::STATUS_CLASS`, style in `main.rs`).
pub const CLASS: &str = "querencia-glocke";
/// CSS class of the count next to the bell.
pub const ZAHL_CLASS: &str = "querencia-zahl";
/// CSS class while there is no notification centre.
pub const UNAVAILABLE_CLASS: &str = "unavailable";
/// CSS class while „Nicht stören“ is on.
pub const RUHE_CLASS: &str = "gedaempft";
/// Action group of the applet's menu.
pub const ACTION_GROUP: &str = "glocke";
const NICHT_STOEREN: &str = "nicht-stoeren";
const ALLE_GELESEN: &str = "alle-gelesen";

pub const BUS_NAME: &str = "org.querencia.Benachrichtigungen";
pub const OBJECT_PATH: &str = "/org/freedesktop/Notifications";
pub const INTERFACE: &str = "org.querencia.Benachrichtigungen";
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";
/// Time limit of a call to the service.
const TIMEOUT_MS: i32 = 3000;
/// Highest count shown in digits.
const HOECHSTENS: u32 = 99;

/// What the service reported.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Zustand {
    /// The bus name has an owner and its properties were read.
    pub da: bool,
    pub ungelesen: u32,
    pub anzahl: u32,
    pub nicht_stoeren: bool,
    pub zentrale_offen: bool,
}

/// Which of the bell's own icons is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Symbol {
    Glocke,
    Neu,
    Ruhe,
}

impl Symbol {
    fn svg(self) -> &'static [u8] {
        match self {
            Symbol::Glocke => include_bytes!("../data/symbole/glocke.svg"),
            Symbol::Neu => include_bytes!("../data/symbole/glocke-neu.svg"),
            Symbol::Ruhe => include_bytes!("../data/symbole/glocke-ruhe.svg"),
        }
    }
}

/// How the bell looks for a [`Zustand`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Aussehen {
    pub symbol: Symbol,
    /// The count next to the bell, `None`: no label.
    pub zahl: Option<String>,
    pub tooltip: String,
    pub ausgegraut: bool,
    pub gedaempft: bool,
}

pub fn aussehen(zustand: &Zustand) -> Aussehen {
    if !zustand.da {
        return Aussehen {
            symbol: Symbol::Glocke,
            zahl: None,
            tooltip: "Mitteilungszentrale nicht verfügbar".to_owned(),
            ausgegraut: true,
            gedaempft: false,
        };
    }
    let mut tooltip = match zustand.ungelesen {
        0 => "Keine ungelesenen Mitteilungen".to_owned(),
        1 => "1 ungelesene Mitteilung".to_owned(),
        n => format!("{n} ungelesene Mitteilungen"),
    };
    if zustand.nicht_stoeren {
        tooltip.push_str("\n„Nicht stören“ ist an");
    }
    let zahl = match zustand.ungelesen {
        0 => None,
        n if n > HOECHSTENS => Some(format!("{HOECHSTENS}+")),
        n => Some(n.to_string()),
    };
    let symbol = if zustand.nicht_stoeren {
        Symbol::Ruhe
    } else if zustand.ungelesen > 0 {
        Symbol::Neu
    } else {
        Symbol::Glocke
    };
    Aussehen {
        symbol,
        zahl,
        tooltip,
        ausgegraut: false,
        gedaempft: zustand.nicht_stoeren,
    }
}

/// Takes the properties of `werte` (`a{sv}`, from `GetAll` or
/// `PropertiesChanged`) into `zustand`; unknown names and wrong types are
/// left out. Returns whether anything changed.
pub fn eigenschaften(zustand: &mut Zustand, werte: &glib::Variant) -> bool {
    let vorher = zustand.clone();
    let Some(dict) = werte.get::<glib::VariantDict>() else {
        return false;
    };
    let zahl = |name: &str| dict.lookup::<u32>(name).ok().flatten();
    let schalter = |name: &str| dict.lookup::<bool>(name).ok().flatten();
    if let Some(n) = zahl("Ungelesen") {
        zustand.ungelesen = n;
    }
    if let Some(n) = zahl("Anzahl") {
        zustand.anzahl = n;
    }
    if let Some(an) = schalter("NichtStoeren") {
        zustand.nicht_stoeren = an;
    }
    if let Some(offen) = schalter("ZentraleOffen") {
        zustand.zentrale_offen = offen;
    }
    *zustand != vorher
}

/// The signal `Zaehler(u ungelesen, u anzahl)`. Returns whether anything
/// changed.
pub fn zaehler(zustand: &mut Zustand, werte: &glib::Variant) -> bool {
    let Some((ungelesen, anzahl)) = werte.get::<(u32, u32)>() else {
        return false;
    };
    let neu = Zustand {
        ungelesen,
        anzahl,
        ..zustand.clone()
    };
    let geaendert = neu != *zustand;
    *zustand = neu;
    geaendert
}

/// What a left click asks of the service.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Klick {
    Zeigen,
    Schliessen,
    Nichts,
}

pub fn klick(zustand: &Zustand) -> Klick {
    match (zustand.da, zustand.zentrale_offen) {
        (false, _) => Klick::Nichts,
        (true, true) => Klick::Schliessen,
        (true, false) => Klick::Zeigen,
    }
}

/// The right-click menu (see `panel_menu.rs`): „Nicht stören“ as a check
/// item, „Alle als gelesen markieren“, then the panel object entry.
pub fn menu() -> gio::Menu {
    let menu = gio::Menu::new();
    menu.append(
        Some("_Nicht stören"),
        Some(&format!("{ACTION_GROUP}.{NICHT_STOEREN}")),
    );
    menu.append(
        Some("Alle als _gelesen markieren"),
        Some(&format!("{ACTION_GROUP}.{ALLE_GELESEN}")),
    );
    let object = gio::Menu::new();
    object.append(
        Some("Aus dem Panel entfernen"),
        Some(crate::panel_menu::REMOVE_ACTION),
    );
    menu.append_section(None, &object);
    menu
}

/// The connection to the service while its name has an owner.
struct Verbunden {
    connection: gio::DBusConnection,
    owner: String,
    /// Unsubscribed when dropped.
    _signale: Vec<gio::SignalSubscription>,
}

struct Glocke {
    zustand: RefCell<Zustand>,
    verbunden: RefCell<Option<Verbunden>>,
    button: glib::WeakRef<Button>,
    image: glib::WeakRef<Image>,
    zahl: glib::WeakRef<Label>,
    size: i32,
    nicht_stoeren: gio::SimpleAction,
    alle_gelesen: gio::SimpleAction,
    /// Set while the action's state follows the service (no call back).
    folgt: Cell<bool>,
    /// What the log said last about the service (`Some(true)`: there).
    gemeldet: Cell<Option<bool>>,
    /// The state the log line of [`Glocke::zeigen`] described last.
    protokolliert: RefCell<Option<Zustand>>,
}

/// The applet for a panel `panel_height` pixels high.
pub fn widget(panel_height: i32) -> Button {
    let size = logo::size_for_panel(panel_height);
    let image = Image::new();
    image.set_pixel_size(size);
    let zahl = Label::new(None);
    zahl.add_css_class(ZAHL_CLASS);
    zahl.set_visible(false);
    let inner = GtkBox::new(Orientation::Horizontal, 1);
    inner.append(&image);
    inner.append(&zahl);
    let button = Button::builder().has_frame(false).child(&inner).build();
    button.add_css_class(CLASS);

    let nicht_stoeren = gio::SimpleAction::new_stateful(NICHT_STOEREN, None, &false.to_variant());
    let alle_gelesen = gio::SimpleAction::new(ALLE_GELESEN, None);
    let glocke = Rc::new(Glocke {
        zustand: RefCell::new(Zustand::default()),
        verbunden: RefCell::new(None),
        button: button.downgrade(),
        image: image.downgrade(),
        zahl: zahl.downgrade(),
        size,
        nicht_stoeren: nicht_stoeren.clone(),
        alle_gelesen: alle_gelesen.clone(),
        folgt: Cell::new(false),
        gemeldet: Cell::new(None),
        protokolliert: RefCell::new(None),
    });
    glocke.zeigen();

    let weak = Rc::downgrade(&glocke);
    nicht_stoeren.connect_change_state(move |action, value| {
        let (Some(glocke), Some(an)) = (weak.upgrade(), value.and_then(|v| v.get::<bool>())) else {
            return;
        };
        if glocke.folgt.get() {
            action.set_state(&an.to_variant());
            return;
        }
        eprintln!(
            "querencia-panel: Glocke: Menü → Nicht stören {}",
            if an { "an" } else { "aus" }
        );
        glocke.nicht_stoeren_setzen(an);
    });
    let weak = Rc::downgrade(&glocke);
    alle_gelesen.connect_activate(move |_, _| {
        if let Some(glocke) = weak.upgrade() {
            eprintln!("querencia-panel: Glocke: Menü → alle als gelesen");
            glocke.aufrufen("AlsGelesen", Some((Vec::<u32>::new(),).to_variant()));
        }
    });
    let actions = gio::SimpleActionGroup::new();
    actions.add_action(&nicht_stoeren);
    actions.add_action(&alle_gelesen);
    button.insert_action_group(ACTION_GROUP, Some(&actions));

    let weak = Rc::downgrade(&glocke);
    button.connect_clicked(move |button| {
        if let Some(glocke) = weak.upgrade() {
            glocke.geklickt(button);
        }
    });

    let appeared = Rc::downgrade(&glocke);
    let vanished = Rc::downgrade(&glocke);
    let watch = gio::bus_watch_name(
        gio::BusType::Session,
        BUS_NAME,
        gio::BusNameWatcherFlags::NONE,
        move |connection, _, owner| {
            if let Some(glocke) = appeared.upgrade() {
                Glocke::verbinden(&glocke, connection, owner);
            }
        },
        move |_, _| {
            if let Some(glocke) = vanished.upgrade() {
                glocke.getrennt();
            }
        },
    );
    // Ends with the applet: no watch, no subscription stays behind.
    let watch = Cell::new(Some(watch));
    button.connect_destroy(move |_| {
        if let Some(id) = watch.take() {
            gio::bus_unwatch_name(id);
        }
        glocke.verbunden.borrow_mut().take();
    });
    button
}

/// The bell at `size` pixels for the „Zum Panel hinzufügen“ dialog.
pub fn image_of_size(size: i32) -> Image {
    let image = Image::new();
    image.set_pixel_size(size);
    let draw = move |image: &Image| {
        if let Some(texture) = texture(Symbol::Glocke, size * image.scale_factor().max(1)) {
            image.set_paintable(Some(&texture));
        }
    };
    draw(&image);
    image.connect_scale_factor_notify(draw);
    image
}

fn texture(symbol: Symbol, pixels: i32) -> Option<gdk::Texture> {
    let stream = gio::MemoryInputStream::from_bytes(&glib::Bytes::from_static(symbol.svg()));
    match gtk4::gdk_pixbuf::Pixbuf::from_stream_at_scale(
        &stream,
        pixels,
        pixels,
        true,
        gio::Cancellable::NONE,
    ) {
        Ok(pixbuf) => Some(gdk::Texture::for_pixbuf(&pixbuf)),
        Err(err) => {
            eprintln!("querencia-panel: Glocke: Symbol nicht lesbar: {err}");
            None
        }
    }
}

impl Glocke {
    /// Shows the state: icon, count, tooltip, greyed out or dimmed, and
    /// the menu's actions.
    fn zeigen(&self) {
        let zustand = self.zustand.borrow().clone();
        let look = aussehen(&zustand);
        // numbers only, never what a notification says
        if zustand.da && self.protokolliert.borrow().as_ref() != Some(&zustand) {
            eprintln!(
                "querencia-panel: Glocke: {} ungelesen von {}, Nicht stören {}, Zentrale {}",
                zustand.ungelesen,
                zustand.anzahl,
                if zustand.nicht_stoeren { "an" } else { "aus" },
                if zustand.zentrale_offen {
                    "offen"
                } else {
                    "zu"
                }
            );
        }
        self.protokolliert.replace(Some(zustand.clone()));
        if let (Some(button), Some(image), Some(zahl)) = (
            self.button.upgrade(),
            self.image.upgrade(),
            self.zahl.upgrade(),
        ) {
            let pixels = self.size * image.scale_factor().max(1);
            match texture(look.symbol, pixels) {
                Some(texture) => image.set_paintable(Some(&texture)),
                None => image.set_icon_name(Some("preferences-desktop-notification")),
            }
            zahl.set_label(look.zahl.as_deref().unwrap_or(""));
            zahl.set_visible(look.zahl.is_some());
            button.set_tooltip_text(Some(&look.tooltip));
            for (class, an) in [
                (UNAVAILABLE_CLASS, look.ausgegraut),
                (RUHE_CLASS, look.gedaempft),
            ] {
                if an {
                    button.add_css_class(class);
                } else {
                    button.remove_css_class(class);
                }
            }
        }
        self.folgt.set(true);
        self.nicht_stoeren
            .change_state(&zustand.nicht_stoeren.to_variant());
        self.folgt.set(false);
        self.nicht_stoeren.set_enabled(zustand.da);
        self.alle_gelesen.set_enabled(zustand.da);
    }

    fn melden(&self, da: bool, text: &str) {
        if self.gemeldet.get() != Some(da) {
            self.gemeldet.set(Some(da));
            eprintln!("querencia-panel: Glocke: {text}");
        }
    }

    /// The name got an owner: follow its signals, then read its state.
    fn verbinden(glocke: &Rc<Glocke>, connection: gio::DBusConnection, owner: &str) {
        let weak = Rc::downgrade(glocke);
        let geaendert = connection.subscribe_to_signal(
            Some(owner),
            Some(PROPERTIES),
            Some("PropertiesChanged"),
            Some(OBJECT_PATH),
            Some(INTERFACE),
            gio::DBusSignalFlags::NONE,
            move |signal| {
                let Some(glocke) = weak.upgrade() else { return };
                let werte = signal.parameters.try_child_value(1);
                let mut zustand = glocke.zustand.borrow_mut();
                let neu = zustand.da && werte.is_some_and(|w| eigenschaften(&mut zustand, &w));
                drop(zustand);
                if neu {
                    glocke.zeigen();
                }
            },
        );
        let weak = Rc::downgrade(glocke);
        let zaehlt = connection.subscribe_to_signal(
            Some(owner),
            Some(INTERFACE),
            Some("Zaehler"),
            Some(OBJECT_PATH),
            None,
            gio::DBusSignalFlags::NONE,
            move |signal| {
                let Some(glocke) = weak.upgrade() else { return };
                let mut zustand = glocke.zustand.borrow_mut();
                let neu = zustand.da && zaehler(&mut zustand, signal.parameters);
                drop(zustand);
                if neu {
                    glocke.zeigen();
                }
            },
        );
        glocke.verbunden.replace(Some(Verbunden {
            connection: connection.clone(),
            owner: owner.to_owned(),
            _signale: vec![geaendert, zaehlt],
        }));
        let weak: Weak<Glocke> = Rc::downgrade(glocke);
        let owner = owner.to_owned();
        connection.call(
            Some(&owner.clone()),
            OBJECT_PATH,
            PROPERTIES,
            "GetAll",
            Some(&(INTERFACE,).to_variant()),
            Some(glib::VariantTy::new("(a{sv})").expect("Signatur")),
            gio::DBusCallFlags::NO_AUTO_START,
            TIMEOUT_MS,
            gio::Cancellable::NONE,
            move |result| {
                let Some(glocke) = weak.upgrade() else { return };
                // the owner may have gone or changed meanwhile
                let gleich = glocke
                    .verbunden
                    .borrow()
                    .as_ref()
                    .is_some_and(|v| v.owner == owner);
                if !gleich {
                    return;
                }
                match result {
                    Ok(antwort) => {
                        {
                            let mut zustand = glocke.zustand.borrow_mut();
                            zustand.da = true;
                            eigenschaften(&mut zustand, &antwort.child_value(0));
                        }
                        let zustand = glocke.zustand.borrow().clone();
                        glocke.melden(
                            true,
                            &format!(
                                "Mitteilungszentrale {BUS_NAME} ({owner}): {} ungelesen, \
                                 Nicht stören {}",
                                zustand.ungelesen,
                                if zustand.nicht_stoeren { "an" } else { "aus" }
                            ),
                        );
                    }
                    Err(err) => glocke.melden(
                        false,
                        &format!("{BUS_NAME} ({owner}) antwortet nicht ({err}), Glocke ausgegraut"),
                    ),
                }
                glocke.zeigen();
            },
        );
    }

    /// The name lost its owner (or never had one).
    fn getrennt(&self) {
        self.verbunden.borrow_mut().take();
        *self.zustand.borrow_mut() = Zustand::default();
        self.melden(
            false,
            &format!(
                "{BUS_NAME} ohne Besitzer (kein Dienst oder Benachrichtigungen vor 0.2.0), \
                 Glocke ausgegraut"
            ),
        );
        self.zeigen();
    }

    fn geklickt(&self, button: &Button) {
        match klick(&self.zustand.borrow()) {
            Klick::Nichts => {
                eprintln!("querencia-panel: Glocke: Klick, keine Mitteilungszentrale");
            }
            Klick::Schliessen => {
                eprintln!("querencia-panel: Glocke: Klick → ZentraleSchliessen");
                self.aufrufen("ZentraleSchliessen", None);
            }
            Klick::Zeigen => {
                let token = aktivierungs_token(button);
                eprintln!(
                    "querencia-panel: Glocke: Klick → ZentraleZeigen ({})",
                    if token.is_empty() {
                        "ohne Token"
                    } else {
                        "mit Token"
                    }
                );
                self.aufrufen("ZentraleZeigen", Some((token,).to_variant()));
            }
        }
    }

    fn nicht_stoeren_setzen(&self, an: bool) {
        let Some((connection, owner)) = self.verbindung() else {
            return;
        };
        let werte = (INTERFACE, "NichtStoeren", an.to_variant()).to_variant();
        connection.call(
            Some(&owner),
            OBJECT_PATH,
            PROPERTIES,
            "Set",
            Some(&werte),
            None,
            gio::DBusCallFlags::NO_AUTO_START,
            TIMEOUT_MS,
            gio::Cancellable::NONE,
            |result| {
                if let Err(err) = result {
                    eprintln!("querencia-panel: Glocke: Nicht stören nicht gesetzt: {err}");
                }
            },
        );
    }

    /// Calls `method` of the service's interface; errors go to the log.
    fn aufrufen(&self, method: &'static str, werte: Option<glib::Variant>) {
        let Some((connection, owner)) = self.verbindung() else {
            return;
        };
        connection.call(
            Some(&owner),
            OBJECT_PATH,
            INTERFACE,
            method,
            werte.as_ref(),
            None,
            gio::DBusCallFlags::NO_AUTO_START,
            TIMEOUT_MS,
            gio::Cancellable::NONE,
            move |result| {
                if let Err(err) = result {
                    eprintln!("querencia-panel: Glocke: {method} fehlgeschlagen: {err}");
                }
            },
        );
    }

    fn verbindung(&self) -> Option<(gio::DBusConnection, String)> {
        self.verbunden
            .borrow()
            .as_ref()
            .map(|v| (v.connection.clone(), v.owner.clone()))
    }
}

/// An xdg-activation token for the window the service shows, from the
/// panel's display (the click is the last input); empty if there is none.
fn aktivierungs_token(button: &Button) -> String {
    let context = button.display().app_launch_context();
    context
        .startup_notify_id(None::<&gio::AppInfo>, &[])
        .map(|id| id.to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dict(eintraege: &[(&str, glib::Variant)]) -> glib::Variant {
        let dict = glib::VariantDict::new(None);
        for (name, wert) in eintraege {
            dict.insert_value(name, wert);
        }
        dict.end()
    }

    fn da(ungelesen: u32) -> Zustand {
        Zustand {
            da: true,
            ungelesen,
            anzahl: ungelesen + 2,
            ..Zustand::default()
        }
    }

    #[test]
    fn without_the_service_the_bell_is_grey_without_a_count() {
        let look = aussehen(&Zustand::default());
        assert!(look.ausgegraut && !look.gedaempft);
        assert_eq!(look.zahl, None);
        assert_eq!(look.symbol, Symbol::Glocke);
        assert_eq!(look.tooltip, "Mitteilungszentrale nicht verfügbar");
        // what was counted before does not show without the service
        let alt = Zustand {
            ungelesen: 4,
            ..Zustand::default()
        };
        assert_eq!(aussehen(&alt).zahl, None);
    }

    #[test]
    fn the_count_shows_unread_notifications() {
        let null = aussehen(&da(0));
        assert_eq!((null.zahl, null.symbol), (None, Symbol::Glocke));
        assert_eq!(null.tooltip, "Keine ungelesenen Mitteilungen");
        assert!(!null.ausgegraut);
        let eins = aussehen(&da(1));
        assert_eq!(eins.zahl.as_deref(), Some("1"));
        assert_eq!(eins.symbol, Symbol::Neu);
        assert_eq!(eins.tooltip, "1 ungelesene Mitteilung");
        let drei = aussehen(&da(3));
        assert_eq!(drei.zahl.as_deref(), Some("3"));
        assert_eq!(drei.tooltip, "3 ungelesene Mitteilungen");
        assert_eq!(aussehen(&da(99)).zahl.as_deref(), Some("99"));
        assert_eq!(aussehen(&da(100)).zahl.as_deref(), Some("99+"));
    }

    #[test]
    fn do_not_disturb_dims_the_bell_and_keeps_the_count() {
        let zustand = Zustand {
            nicht_stoeren: true,
            ..da(3)
        };
        let look = aussehen(&zustand);
        assert!(look.gedaempft && !look.ausgegraut);
        assert_eq!(look.symbol, Symbol::Ruhe);
        assert_eq!(look.zahl.as_deref(), Some("3"));
        assert_eq!(
            look.tooltip,
            "3 ungelesene Mitteilungen\n„Nicht stören“ ist an"
        );
    }

    #[test]
    fn properties_are_taken_from_get_all_and_properties_changed() {
        let mut zustand = Zustand {
            da: true,
            ..Zustand::default()
        };
        let alle = dict(&[
            ("NichtStoeren", false.to_variant()),
            ("Ungelesen", 3u32.to_variant()),
            ("Anzahl", 5u32.to_variant()),
            ("ZentraleOffen", false.to_variant()),
        ]);
        assert!(eigenschaften(&mut zustand, &alle));
        assert_eq!((zustand.ungelesen, zustand.anzahl), (3, 5));
        // PropertiesChanged with one property: only that one changes
        assert!(eigenschaften(
            &mut zustand,
            &dict(&[("ZentraleOffen", true.to_variant())])
        ));
        assert!(zustand.zentrale_offen);
        assert_eq!(zustand.ungelesen, 3);
        // the same again: no change
        assert!(!eigenschaften(
            &mut zustand,
            &dict(&[("ZentraleOffen", true.to_variant())])
        ));
        // wrong types and unknown names are left out
        assert!(!eigenschaften(
            &mut zustand,
            &dict(&[
                ("Ungelesen", "drei".to_variant()),
                ("Unbekannt", 1u32.to_variant()),
            ])
        ));
        assert!(!eigenschaften(&mut zustand, &7u32.to_variant()));
        assert_eq!(zustand.ungelesen, 3);
    }

    #[test]
    fn the_counter_signal_sets_both_numbers() {
        let mut zustand = da(0);
        assert!(zaehler(&mut zustand, &(2u32, 7u32).to_variant()));
        assert_eq!((zustand.ungelesen, zustand.anzahl), (2, 7));
        assert!(!zaehler(&mut zustand, &(2u32, 7u32).to_variant()));
        assert!(!zaehler(&mut zustand, &(2u32,).to_variant()));
        assert_eq!(zustand.ungelesen, 2);
    }

    #[test]
    fn a_click_opens_or_closes_the_centre() {
        assert_eq!(klick(&Zustand::default()), Klick::Nichts);
        assert_eq!(klick(&da(2)), Klick::Zeigen);
        let offen = Zustand {
            zentrale_offen: true,
            ..da(2)
        };
        assert_eq!(klick(&offen), Klick::Schliessen);
    }

    fn label(menu: &gio::MenuModel, i: i32) -> Option<String> {
        menu.item_attribute_value(i, "label", Some(glib::VariantTy::STRING))
            .and_then(|v| v.get::<String>())
    }

    fn action(menu: &gio::MenuModel, i: i32) -> Option<String> {
        menu.item_attribute_value(i, "action", Some(glib::VariantTy::STRING))
            .and_then(|v| v.get::<String>())
    }

    #[test]
    fn menu_has_do_not_disturb_mark_all_read_and_remove() {
        let menu = menu().upcast::<gio::MenuModel>();
        assert_eq!(label(&menu, 0).as_deref(), Some("_Nicht stören"));
        assert_eq!(action(&menu, 0).as_deref(), Some("glocke.nicht-stoeren"));
        assert_eq!(
            label(&menu, 1).as_deref(),
            Some("Alle als _gelesen markieren")
        );
        assert_eq!(action(&menu, 1).as_deref(), Some("glocke.alle-gelesen"));
        let section = menu.item_link(2, "section").expect("Abschnitt");
        assert_eq!(action(&section, 0).as_deref(), Some("panel.applet-remove"));
    }

    #[test]
    fn own_icons_are_querencia_svgs() {
        for symbol in [Symbol::Glocke, Symbol::Neu, Symbol::Ruhe] {
            let text = std::str::from_utf8(symbol.svg()).unwrap();
            assert!(text.starts_with("<?xml"), "{symbol:?}");
            assert!(text.contains("#5a5a5a"), "grau: {symbol:?}");
        }
        assert!(
            std::str::from_utf8(Symbol::Neu.svg())
                .unwrap()
                .contains("#F07746")
        );
    }

    #[test]
    fn interface_names_match_the_service() {
        assert_eq!(BUS_NAME, "org.querencia.Benachrichtigungen");
        assert_eq!(INTERFACE, BUS_NAME);
        assert_eq!(OBJECT_PATH, "/org/freedesktop/Notifications");
        assert!(!["panel", "volume", "network", "menu"].contains(&ACTION_GROUP));
    }
}
