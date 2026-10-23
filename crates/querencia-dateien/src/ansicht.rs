//! The views of a window (interface of Welle B, `docs/dateien.md` §3):
//! the trait [`Ansicht`] every view implements (symbol view, list view,
//! later the compact view), the [`Rueckruf`] through which a view tells
//! its window what the user did, and [`Ansichten`], the switch that holds
//! one instance per kind and shows the chosen one.
//!
//! The symbol view is deliberately an exchangeable module (it is being
//! judged at the screen, `dateien/prototyp-2`): any type that implements
//! [`Ansicht`] and is registered for a kind (`app::Register::ansicht`)
//! takes its place. Without a registered view the window uses
//! [`Ersatzansicht`], a plain list with icon and name.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use gtk4::prelude::*;
use gtk4::{gdk, gio, glib};
use querencia_oberflaeche::symbole;

use crate::eintrag::Eintrag;
use crate::i18n::gettext;
use crate::konfig::Werte;
use crate::modell::Ordnermodell;
use crate::regeln::{Ansichtsart, ZOOMSTUFEN, Zoom};

/// Where a context menu was asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kontextziel {
    /// On an entry: the menu is for the selection (the view selects the
    /// entry under the pointer first if it was not selected).
    Auswahl,
    /// On free space: the menu is for the shown folder.
    Hintergrund,
}

/// What a view reports to its window. The window implements it
/// (`fenster.rs`); a view gets it through [`Ansicht::verbinden`].
pub trait Rueckruf {
    /// Double click or Enter (single click with the setting "Einfacher
    /// Klick") on entries; `neues_fenster` for Umschalt+Doppelklick / the
    /// middle button (opens folders in a new window).
    fn aktivieren(&self, eintraege: &[Eintrag], neues_fenster: bool);
    /// Right click or the menu key at `x`, `y` in `widget`'s coordinates.
    fn kontextmenue(&self, ziel: Kontextziel, widget: &gtk4::Widget, x: f64, y: f64);
    /// "/" or "~" typed in the view: the location entry opens with it (B §1.3).
    fn adresse_tippen(&self, anfang: &str);
    /// The view changed its zoom itself (Strg+Mausrad).
    fn zoom_geaendert(&self, zoom: Zoom);
    /// Rücktaste in the view: go to the parent folder.
    fn nach_oben(&self);
    /// Rename finished in the cell: the window starts the job
    /// (`Op::Rename` through the queue; errors and conflicts show the
    /// operation dialogs) and selects the renamed entry afterwards.
    fn umbenennen(&self, datei: &gio::File, neuer_name: &str);
    /// A short message for the status bar (e.g. why a name is invalid).
    fn melden(&self, text: &str);
}

/// A view of the folder model. Implementations: `symbolansicht.rs`,
/// `listenansicht.rs` (Welle B2), [`Ersatzansicht`].
///
/// Rules every view keeps (`docs/dateien.md` §3.3):
/// - it shows `modell.auswahl()` (a `gtk::MultiSelection`) and never keeps
///   its own selection or order; selection and sort order belong to the
///   model, so switching views keeps them;
/// - theme icons only through `querencia_oberflaeche::symbole` (QD-106);
/// - keys that act on files (Entf, Strg+C, F2 …) are bound by the window
///   on the view area, not by the view; the view handles navigation keys,
///   type-ahead, Rücktaste, "/" and "~", Strg+Mausrad and the menu key;
/// - opening, context menus and renaming go through the [`Rueckruf`].
pub trait Ansicht {
    /// The kind this view stands for.
    fn art(&self) -> Ansichtsart;

    /// The top widget (usually a `ScrolledWindow` or an `Overlay` around it).
    fn widget(&self) -> gtk4::Widget;

    /// Shows `modell` (called once per window, before the view is shown).
    fn set_modell(&self, modell: &Ordnermodell);

    /// The model given with [`Self::set_modell`].
    fn modell(&self) -> Option<Ordnermodell>;

    /// The window's callbacks (called once, before the view is shown).
    fn verbinden(&self, rueckruf: Rc<dyn Rueckruf>);

    /// The current zoom level.
    fn zoom(&self) -> Zoom;

    /// Sets the zoom level (one of [`Self::zoomstufen`]).
    fn zoom_setzen(&self, zoom: Zoom);

    /// The zoom levels this view offers, ascending.
    fn zoomstufen(&self) -> &'static [u32] {
        &ZOOMSTUFEN
    }

    /// Gives the view the keyboard focus (cursor on the focused entry, else
    /// on the first one without selecting it).
    fn fokus(&self);

    /// Moves the cursor to `datei` and scrolls it into view; with `waehlen`
    /// it becomes the only selected entry.
    fn fokus_auf(&self, datei: &gio::File, waehlen: bool);

    /// Starts renaming `datei` in its cell (F2: the name without its
    /// extension is selected; Umschalt+F2 / `ganzer_name`: all of it). The
    /// result goes to [`Rueckruf::umbenennen`].
    fn umbenennen_anstossen(&self, datei: &gio::File, ganzer_name: bool);

    /// Whether a rename field is open (the window then leaves keys alone).
    fn umbenennen_laeuft(&self) -> bool {
        false
    }

    /// The entry at `x`, `y` in [`Self::widget`]'s coordinates (only on the
    /// icon or label, not on the free space of a cell), for drag and drop.
    fn eintrag_bei(&self, x: f64, y: f64) -> Option<Eintrag>;

    /// Marks `eintrag` as drop target while something is dragged over it
    /// (`None`: no mark).
    fn ablage_markieren(&self, _eintrag: Option<&Eintrag>) {}

    /// Settings changed (and once at the start): click policy, label
    /// lines, list columns … (`konfig::Werte`).
    fn einstellungen_anwenden(&self, _werte: &Werte) {}
}

/// Makes a view of one kind for a window.
pub type Fabrik = Rc<dyn Fn() -> Rc<dyn Ansicht>>;
/// Called for every view instance a window makes (drag and drop, …).
pub type Neu = Rc<dyn Fn(&Rc<dyn Ansicht>)>;

/// The view switch of a window: one instance per kind, made on first use
/// from the registered factories, inside a `gtk::Stack`.
pub struct Ansichten {
    stapel: gtk4::Stack,
    modell: Ordnermodell,
    rueckruf: Rc<dyn Rueckruf>,
    fabriken: HashMap<Ansichtsart, Fabrik>,
    bei_neu: Vec<Neu>,
    vorhanden: RefCell<HashMap<Ansichtsart, Rc<dyn Ansicht>>>,
    aktuell: Cell<Option<Ansichtsart>>,
    werte: RefCell<Werte>,
}

impl Ansichten {
    pub fn new(
        modell: &Ordnermodell,
        rueckruf: Rc<dyn Rueckruf>,
        fabriken: HashMap<Ansichtsart, Fabrik>,
        bei_neu: Vec<Neu>,
        werte: &Werte,
    ) -> Self {
        let stapel = gtk4::Stack::new();
        stapel.set_hexpand(true);
        stapel.set_vexpand(true);
        Self {
            stapel,
            modell: modell.clone(),
            rueckruf,
            fabriken,
            bei_neu,
            vorhanden: RefCell::default(),
            aktuell: Cell::new(None),
            werte: RefCell::new(werte.clone()),
        }
    }

    /// The widget the window places (holds all views).
    pub fn widget(&self) -> &gtk4::Stack {
        &self.stapel
    }

    /// The kind shown.
    pub fn art(&self) -> Option<Ansichtsart> {
        self.aktuell.get()
    }

    /// The view shown (made on first use).
    pub fn aktuell(&self) -> Rc<dyn Ansicht> {
        let art = self.aktuell.get().unwrap_or(Ansichtsart::Symbole);
        self.holen(art)
    }

    /// Every view made so far.
    pub fn alle(&self) -> Vec<Rc<dyn Ansicht>> {
        self.vorhanden.borrow().values().cloned().collect()
    }

    fn holen(&self, art: Ansichtsart) -> Rc<dyn Ansicht> {
        if let Some(a) = self.vorhanden.borrow().get(&art) {
            return a.clone();
        }
        let ansicht: Rc<dyn Ansicht> = match self.fabriken.get(&art) {
            Some(fabrik) => fabrik(),
            None => Ersatzansicht::new(art),
        };
        ansicht.set_modell(&self.modell);
        ansicht.verbinden(self.rueckruf.clone());
        ansicht.einstellungen_anwenden(&self.werte.borrow());
        ansicht.zoom_setzen(self.werte.borrow().zoom_fuer(art));
        self.stapel
            .add_named(&ansicht.widget(), Some(art.kennung()));
        self.vorhanden.borrow_mut().insert(art, ansicht.clone());
        for f in &self.bei_neu {
            f(&ansicht);
        }
        ansicht
    }

    /// Shows the view of kind `art`; keeps the focus in the view area if it
    /// was there.
    pub fn umschalten(&self, art: Ansichtsart) -> Rc<dyn Ansicht> {
        let hatte_fokus = self.aktuell.get().is_some_and(|alt| {
            self.vorhanden
                .borrow()
                .get(&alt)
                .is_some_and(|a| a.widget().has_focus() || hat_fokus_innen(&a.widget()))
        });
        let ansicht = self.holen(art);
        self.stapel.set_visible_child(&ansicht.widget());
        self.aktuell.set(Some(art));
        if hatte_fokus {
            ansicht.fokus();
        }
        ansicht
    }

    /// New settings for every view made so far (and the ones to come).
    pub fn einstellungen_anwenden(&self, werte: &Werte) {
        self.werte.replace(werte.clone());
        for a in self.alle() {
            a.einstellungen_anwenden(werte);
        }
    }
}

fn hat_fokus_innen(widget: &gtk4::Widget) -> bool {
    widget
        .root()
        .and_then(|r| r.focus())
        .is_some_and(|f| f.is_ancestor(widget))
}

// ------------------------------------------------------------- fallback

/// A plain list with icon and name: the view of a kind no module is
/// registered for (keeps the program usable while the real views are
/// built in parallel, Welle B2). Double click, Enter, Rücktaste, context
/// menu and zoom work; no type-ahead, no rename in place.
pub struct Ersatzansicht {
    art: Ansichtsart,
    liste: gtk4::ListView,
    scroller: gtk4::ScrolledWindow,
    modell: RefCell<Option<Ordnermodell>>,
    rueckruf: RefCell<Option<Rc<dyn Rueckruf>>>,
    zoom: Cell<Zoom>,
    einfacher_klick: Cell<bool>,
    zellen: RefCell<Vec<glib::WeakRef<gtk4::Image>>>,
}

impl Ersatzansicht {
    pub fn new(art: Ansichtsart) -> Rc<Self> {
        let factory = gtk4::SignalListItemFactory::new();
        let liste = gtk4::ListView::new(None::<gtk4::MultiSelection>, Some(factory.clone()));
        liste.add_css_class("querencia-ersatzansicht");
        let scroller = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .child(&liste)
            .hexpand(true)
            .vexpand(true)
            .build();
        let ansicht = Rc::new(Self {
            art,
            liste,
            scroller,
            modell: RefCell::default(),
            rueckruf: RefCell::default(),
            zoom: Cell::new(art.vorgabe_zoom()),
            einfacher_klick: Cell::new(false),
            zellen: RefCell::default(),
        });
        let weak = Rc::downgrade(&ansicht);
        factory.connect_setup(move |_, item| {
            let Some(item) = item.downcast_ref::<gtk4::ListItem>() else {
                return;
            };
            let zeile = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
            let bild = gtk4::Image::new();
            let text = gtk4::Label::new(None);
            text.set_xalign(0.0);
            text.set_ellipsize(gtk4::pango::EllipsizeMode::Middle);
            zeile.append(&bild);
            zeile.append(&text);
            item.set_child(Some(&zeile));
            if let Some(a) = weak.upgrade() {
                bild.set_pixel_size(a.zoom.get().symbolgroesse().min(48));
                a.zellen.borrow_mut().push(bild.downgrade());
            }
        });
        factory.connect_bind(|_, item| {
            let Some(item) = item.downcast_ref::<gtk4::ListItem>() else {
                return;
            };
            let (Some(e), Some(zeile)) = (
                item.item().and_downcast::<Eintrag>(),
                item.child().and_downcast::<gtk4::Box>(),
            ) else {
                return;
            };
            if let Some(bild) = zeile.first_child().and_downcast::<gtk4::Image>() {
                symbole::set(&bild, &e.daten().symbol);
            }
            if let Some(text) = zeile.last_child().and_downcast::<gtk4::Label>() {
                text.set_text(&e.daten().anzeige);
            }
        });
        ansicht.signale();
        ansicht
    }

    fn signale(self: &Rc<Self>) {
        let weak: Weak<Self> = Rc::downgrade(self);
        self.liste.connect_activate(move |_, pos| {
            let Some(a) = weak.upgrade() else { return };
            let (Some(m), Some(r)) = (a.modell.borrow().clone(), a.rueckruf.borrow().clone())
            else {
                return;
            };
            let mut auswahl = m.gewaehlte();
            if auswahl.is_empty()
                && let Some(e) = m.eintrag(pos)
            {
                auswahl.push(e);
            }
            r.aktivieren(&auswahl, false);
        });
        let klick = gtk4::GestureClick::new();
        klick.set_button(0);
        let weak: Weak<Self> = Rc::downgrade(self);
        klick.connect_pressed(move |g, n, x, y| {
            let Some(a) = weak.upgrade() else { return };
            let knopf = g.current_button();
            if knopf == gdk::BUTTON_SECONDARY {
                a.kontext(x, y);
                g.set_state(gtk4::EventSequenceState::Claimed);
            } else if knopf == gdk::BUTTON_PRIMARY
                && n == 1
                && a.einfacher_klick.get()
                && let (Some(e), Some(r)) = (a.eintrag_bei(x, y), a.rueckruf.borrow().clone())
            {
                r.aktivieren(&[e], false);
            }
        });
        self.liste.add_controller(klick);
        let tasten = gtk4::EventControllerKey::new();
        let weak: Weak<Self> = Rc::downgrade(self);
        tasten.connect_key_pressed(move |_, taste, _, zustand| {
            let Some(a) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            let Some(r) = a.rueckruf.borrow().clone() else {
                return glib::Propagation::Proceed;
            };
            let ohne = zustand & (gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::ALT_MASK)
                == gdk::ModifierType::empty();
            match taste {
                gdk::Key::BackSpace if ohne => {
                    r.nach_oben();
                    glib::Propagation::Stop
                }
                gdk::Key::slash | gdk::Key::KP_Divide if ohne => {
                    r.adresse_tippen("/");
                    glib::Propagation::Stop
                }
                gdk::Key::asciitilde | gdk::Key::dead_tilde if ohne => {
                    r.adresse_tippen("~");
                    glib::Propagation::Stop
                }
                gdk::Key::Menu => {
                    a.kontext_tastatur();
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        });
        self.liste.add_controller(tasten);
        let rad = gtk4::EventControllerScroll::new(gtk4::EventControllerScrollFlags::VERTICAL);
        let weak: Weak<Self> = Rc::downgrade(self);
        rad.connect_scroll(move |c, _, dy| {
            let Some(a) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            if !c
                .current_event_state()
                .contains(gdk::ModifierType::CONTROL_MASK)
            {
                return glib::Propagation::Proceed;
            }
            let neu = a
                .zoom
                .get()
                .schritt(if dy < 0.0 { 1 } else { -1 }, a.zoomstufen());
            a.zoom_setzen(neu);
            if let Some(r) = a.rueckruf.borrow().clone() {
                r.zoom_geaendert(neu);
            }
            glib::Propagation::Stop
        });
        self.liste.add_controller(rad);
    }

    fn kontext(&self, x: f64, y: f64) {
        let (Some(m), Some(r)) = (self.modell.borrow().clone(), self.rueckruf.borrow().clone())
        else {
            return;
        };
        let ziel = match self.eintrag_bei(x, y) {
            Some(e) => {
                if !m.gewaehlte().iter().any(|g| g == &e) {
                    m.waehlen(&[e.datei().clone()]);
                }
                Kontextziel::Auswahl
            }
            None => {
                m.auswahl().unselect_all();
                Kontextziel::Hintergrund
            }
        };
        r.kontextmenue(ziel, self.liste.upcast_ref(), x, y);
    }

    fn kontext_tastatur(&self) {
        let (Some(m), Some(r)) = (self.modell.borrow().clone(), self.rueckruf.borrow().clone())
        else {
            return;
        };
        let ziel = if m.gewaehlte().is_empty() {
            Kontextziel::Hintergrund
        } else {
            Kontextziel::Auswahl
        };
        r.kontextmenue(ziel, self.liste.upcast_ref(), 24.0, 24.0);
    }

    /// The row widget under `y` (ListView rows are as wide as the view).
    fn zeile_bei(&self, x: f64, y: f64) -> Option<gtk4::Widget> {
        let mut w = self.liste.pick(x, y, gtk4::PickFlags::DEFAULT)?;
        loop {
            let eltern = w.parent()?;
            if eltern == *self.liste.upcast_ref::<gtk4::Widget>() {
                return Some(w);
            }
            w = eltern;
        }
    }
}

impl Ansicht for Ersatzansicht {
    fn art(&self) -> Ansichtsart {
        self.art
    }

    fn widget(&self) -> gtk4::Widget {
        self.scroller.clone().upcast()
    }

    fn set_modell(&self, modell: &Ordnermodell) {
        self.liste.set_model(Some(modell.auswahl()));
        self.modell.replace(Some(modell.clone()));
    }

    fn modell(&self) -> Option<Ordnermodell> {
        self.modell.borrow().clone()
    }

    fn verbinden(&self, rueckruf: Rc<dyn Rueckruf>) {
        self.rueckruf.replace(Some(rueckruf));
    }

    fn zoom(&self) -> Zoom {
        self.zoom.get()
    }

    fn zoom_setzen(&self, zoom: Zoom) {
        self.zoom.set(zoom);
        let groesse = zoom.symbolgroesse().min(48);
        self.zellen.borrow_mut().retain(|w| match w.upgrade() {
            Some(bild) => {
                bild.set_pixel_size(groesse);
                true
            }
            None => false,
        });
    }

    fn fokus(&self) {
        self.liste.grab_focus();
    }

    fn fokus_auf(&self, datei: &gio::File, waehlen: bool) {
        let Some(m) = self.modell() else { return };
        let Some(pos) = m.position_von(datei) else {
            return;
        };
        if waehlen {
            m.waehlen(std::slice::from_ref(datei));
        }
        // GTK 4.10: the list's own action (scroll_to needs GTK 4.12)
        let _ =
            WidgetExt::activate_action(&self.liste, "list.scroll-to-item", Some(&pos.to_variant()));
    }

    fn umbenennen_anstossen(&self, _datei: &gio::File, _ganzer_name: bool) {
        if let Some(r) = self.rueckruf.borrow().clone() {
            r.melden(&gettext("Renaming is not possible in this view."));
        }
    }

    fn eintrag_bei(&self, x: f64, y: f64) -> Option<Eintrag> {
        let zeile = self.zeile_bei(x, y)?;
        // The ListItem is not reachable from its row widget; compare the
        // label text with the entries of the visible rows.
        let text = zeile
            .first_child()
            .and_then(|c| c.last_child())
            .and_downcast::<gtk4::Label>()?
            .text();
        let m = self.modell()?;
        (0..m.anzahl())
            .filter_map(|i| m.eintrag(i))
            .find(|e| e.daten().anzeige == text.as_str())
    }

    fn einstellungen_anwenden(&self, werte: &Werte) {
        self.einfacher_klick.set(werte.einfacher_klick);
        self.liste.set_single_click_activate(false);
    }
}
