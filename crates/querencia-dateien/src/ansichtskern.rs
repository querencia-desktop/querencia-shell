//! What the icon view and the list view share: the behaviour of Nautilus
//! 2.30's views that GTK 4's GridView and ColumnView do not bring along.
//!
//! - type-ahead ([`crate::tippsuche`]), Backspace for the parent folder,
//!   Enter opens every selected object;
//! - the keyboard focus followed through the window (GTK has no getter)
//!   and kept on the same object when the order changes;
//! - a click on free space clears the selection, Ctrl+wheel zooms;
//! - rename in place with the lease of [`crate::umbenennen`];
//! - the context menus of [`crate::kontextmenue`].
//!
//! The views register their name cells (a `gtk4::Box` with the icon and
//! the name label) with [`Kern::zelle_anmelden`] and tell the kern about
//! bind and unbind; everything else happens here.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::Duration;

use gtk4::prelude::*;
use gtk4::{gdk, gio, glib};

use querencia_dateiarbeit::namen;
use querencia_oberflaeche::classic_menu;

use crate::kontextmenue::{self, Auswahlinfo, Ort, Umgebung};
use crate::tippsuche::Tippsuche;
use crate::umbenennen::{self, Feld, Mietvertrag};
pub use crate::vorlauf_modell::{Eintrag, Kriterium, Sortierung};

// GTK 4.12: gtk_grid_view_scroll_to and gtk_column_view_scroll_to. The
// workspace builds gtk4-rs with the feature v4_10 only; both functions
// are in the installed GTK 4.22 (prototype ansicht.rs).
unsafe extern "C" {
    fn gtk_grid_view_scroll_to(
        view: *mut gtk4::ffi::GtkGridView,
        pos: u32,
        flags: u32,
        scroll: *mut std::ffi::c_void,
    );
    fn gtk_column_view_scroll_to(
        view: *mut gtk4::ffi::GtkColumnView,
        pos: u32,
        column: *mut gtk4::ffi::GtkColumnViewColumn,
        flags: u32,
        scroll: *mut std::ffi::c_void,
    );
}
const SCROLL_NONE: u32 = 0;
const SCROLL_FOCUS: u32 = 1;
const SCROLL_SELECT: u32 = 2;

pub const PROGRAMM: &str = "querencia-dateien";

/// Diagnostics of the views on stderr (`QUERENCIA_DATEIEN_DEBUG=1`); the
/// isolated checks read these lines.
pub fn protokoll(text: &str) {
    thread_local! {
        static AN: bool = std::env::var_os("QUERENCIA_DATEIEN_DEBUG").is_some_and(|v| v == "1");
    }
    if AN.with(|a| *a) {
        eprintln!("{PROGRAMM}: {text}");
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Art {
    Symbole,
    Liste,
}

impl Art {
    pub fn name(self) -> &'static str {
        match self {
            Art::Symbole => "Symbolansicht",
            Art::Liste => "Listenansicht",
        }
    }
}

type Rueckruf<T> = RefCell<Option<Rc<dyn Fn(T)>>>;
type UmgebungFn = Rc<dyn Fn() -> (Option<gio::File>, Sortierung)>;

/// What the window learns from a view, and what the view asks of it.
#[derive(Default)]
pub struct Rueckrufe {
    /// Double click or Enter: the objects to open.
    pub aktiviert: Rueckruf<Vec<Eintrag>>,
    /// Backspace: the parent folder.
    pub nach_oben: Rueckruf<()>,
    /// „/“ or „~“ typed: the location entry, starting with the character.
    pub ortsfeld: Rueckruf<char>,
    /// The selection changed (status bar).
    pub auswahl: Rueckruf<()>,
    /// A rename succeeded: the old entry and the renamed file.
    pub umbenannt: Rueckruf<(Eintrag, gio::File)>,
    /// The folder shown and the sort order, for the context menus.
    pub umgebung: RefCell<Option<UmgebungFn>>,
}

#[derive(Default)]
struct Zustand {
    /// File name of the object that had the keyboard focus last.
    cursor: Option<String>,
    umbenennen: Option<Mietvertrag>,
    /// Select (and focus) this name once it is in the model; with `true`
    /// start renaming it (new folder).
    warten_auf: Option<(String, bool)>,
    zoom: usize,
    /// Slow second click on a selected name: the pending rename.
    langsam: Option<glib::SourceId>,
    gedrueckt: Option<(f64, f64, bool)>,
    umordnen_geplant: bool,
}

type ZelleUndItem = (glib::WeakRef<gtk4::Box>, glib::WeakRef<gtk4::ListItem>);

pub struct Kern {
    pub art: Art,
    pub auswahl: gtk4::MultiSelection,
    /// The GridView or the ColumnView.
    pub liste: gtk4::Widget,
    /// The widget whose children are the item widgets (the GridView, the
    /// ColumnView's inner list).
    behaelter: RefCell<Option<gtk4::Widget>>,
    pub wurzel: gtk4::Overlay,
    tipp: Rc<Tippsuche>,
    anker: gtk4::MenuButton,
    menue: gio::Menu,
    z: RefCell<Zustand>,
    zellen: RefCell<HashMap<usize, ZelleUndItem>>,
    pub rueckrufe: Rueckrufe,
    /// Breite of a rename field under icons (icon view), per zoom level.
    feldbreite: Cell<i32>,
}

fn adresse(w: &impl IsA<gtk4::Widget>) -> usize {
    w.as_ref().as_ptr() as usize
}

/// Icon and name label of a registered cell.
pub fn teile(zelle: &gtk4::Box) -> (gtk4::Image, gtk4::Label) {
    let bild = zelle
        .first_child()
        .and_downcast::<gtk4::Image>()
        .expect("Bild");
    let text = bild
        .next_sibling()
        .and_downcast::<gtk4::Label>()
        .expect("Beschriftung");
    (bild, text)
}

/// A rename field left in a cell (it carries the class `umbenennen`).
fn feld_rest(zelle: &gtk4::Box) -> Option<gtk4::Widget> {
    let mut kind = zelle.first_child();
    while let Some(k) = kind {
        if k.has_css_class("umbenennen") {
            return Some(k);
        }
        kind = k.next_sibling();
    }
    None
}

impl Kern {
    pub fn new(
        art: Art,
        auswahl: gtk4::MultiSelection,
        liste: gtk4::Widget,
        scroller: &gtk4::ScrolledWindow,
    ) -> Rc<Self> {
        let tipp = Tippsuche::new();
        let wurzel = gtk4::Overlay::new();
        wurzel.set_child(Some(scroller));
        wurzel.add_overlay(&tipp.rahmen);
        // The anchor of the context menus: an empty menu button at the
        // view's origin; the popover points at the click (desktop pattern).
        let anker = gtk4::MenuButton::new();
        anker.set_child(Some(&gtk4::Box::new(gtk4::Orientation::Horizontal, 0)));
        anker.add_css_class("menue-anker");
        anker.set_can_target(false);
        anker.set_focus_on_click(false);
        anker.set_halign(gtk4::Align::Start);
        anker.set_valign(gtk4::Align::Start);
        wurzel.add_overlay(&anker);
        let menue = gio::Menu::new();
        classic_menu::attach(&anker, menue.upcast_ref());
        let kern = Rc::new(Self {
            art,
            auswahl,
            liste,
            behaelter: RefCell::new(None),
            wurzel,
            tipp,
            anker,
            menue,
            z: RefCell::new(Zustand::default()),
            zellen: RefCell::new(HashMap::new()),
            rueckrufe: Rueckrufe::default(),
            feldbreite: Cell::new(-1),
        });
        kern.signale(scroller);
        kern
    }

    fn behaelter(&self) -> gtk4::Widget {
        if let Some(b) = self.behaelter.borrow().as_ref() {
            return b.clone();
        }
        let b = match self.art {
            Art::Symbole => self.liste.clone(),
            Art::Liste => {
                let mut kind = self.liste.first_child();
                while let Some(k) = kind.clone() {
                    if k.is::<gtk4::ListView>() {
                        break;
                    }
                    kind = k.next_sibling();
                }
                kind.unwrap_or_else(|| self.liste.clone())
            }
        };
        *self.behaelter.borrow_mut() = Some(b.clone());
        b
    }

    pub fn eintrag(&self, pos: u32) -> Option<Eintrag> {
        self.auswahl.item(pos).and_downcast::<Eintrag>()
    }

    pub fn zoom(&self) -> usize {
        self.z.borrow().zoom
    }

    pub fn zoom_merken(self: &Rc<Self>, index: usize) {
        self.umbenennen_beenden(true);
        self.z.borrow_mut().zoom = index;
    }

    pub fn feldbreite_setzen(&self, breite: i32) {
        self.feldbreite.set(breite);
    }

    // --- cells -------------------------------------------------------

    pub fn zelle_anmelden(self: &Rc<Self>, zelle: &gtk4::Box, li: &gtk4::ListItem) {
        let schluessel = adresse(zelle);
        self.zellen
            .borrow_mut()
            .insert(schluessel, (zelle.downgrade(), li.downgrade()));
        let weak = Rc::downgrade(self);
        zelle.connect_destroy(move |_| {
            if let Some(k) = weak.upgrade() {
                k.zellen.borrow_mut().remove(&schluessel);
            }
        });
    }

    pub fn lebende_zellen(&self) -> Vec<(gtk4::Box, gtk4::ListItem)> {
        self.zellen
            .borrow()
            .values()
            .filter_map(|(z, li)| Some((z.upgrade()?, li.upgrade()?)))
            .collect()
    }

    fn item_von_zelle(&self, zelle: &gtk4::Widget) -> Option<gtk4::ListItem> {
        self.zellen
            .borrow()
            .get(&adresse(zelle))
            .and_then(|(_, li)| li.upgrade())
    }

    fn zelle_von(&self, eintrag: &Eintrag) -> Option<gtk4::Box> {
        self.lebende_zellen()
            .into_iter()
            .find(|(_, li)| li.item().as_ref() == Some(eintrag.upcast_ref()))
            .map(|(z, _)| z)
    }

    /// The registered cell inside `w` (an item widget), searched a few
    /// levels down.
    fn zelle_in(&self, w: &gtk4::Widget, tiefe: u32) -> Option<gtk4::Widget> {
        if self.zellen.borrow().contains_key(&adresse(w)) {
            return Some(w.clone());
        }
        if tiefe == 0 {
            return None;
        }
        let mut kind = w.first_child();
        while let Some(k) = kind {
            if let Some(z) = self.zelle_in(&k, tiefe - 1) {
                return Some(z);
            }
            kind = k.next_sibling();
        }
        None
    }

    /// The object of the item widget that contains `w` (`None` for free
    /// space or a widget outside the list, such as a column header).
    /// Second value: whether `w` lies inside the list at all.
    fn eintrag_bei(&self, w: Option<gtk4::Widget>) -> (Option<(Eintrag, gtk4::Widget)>, bool) {
        let behaelter = self.behaelter();
        let mut w = w;
        while let Some(kind) = w {
            if kind == behaelter {
                return (None, true);
            }
            if kind.parent().as_ref() == Some(&behaelter) {
                let e = self.zelle_in(&kind, 4).and_then(|z| {
                    let li = self.item_von_zelle(&z)?;
                    Some((li.item().and_downcast::<Eintrag>()?, z))
                });
                return (e, true);
            }
            if kind == self.liste {
                return (None, false);
            }
            w = kind.parent();
        }
        (None, false)
    }

    fn position_bei(&self, x: f64, y: f64) -> (Option<(u32, gtk4::Widget)>, bool) {
        let (e, drin) = self.eintrag_bei(self.liste.pick(x, y, gtk4::PickFlags::DEFAULT));
        (e.and_then(|(e, w)| Some((self.position_des(&e)?, w))), drin)
    }

    /// From a view's bind: removes a field that is not this object's and
    /// puts the field back when this is the object being renamed.
    pub fn gebunden(self: &Rc<Self>, zelle: &gtk4::Box, eintrag: &Eintrag) {
        let (gehoert, ziel) = {
            let z = self.z.borrow();
            let u = z.umbenennen.as_ref();
            (
                u.and_then(|u| u.feld.as_ref())
                    .is_some_and(|(_, z)| z == zelle),
                u.is_some_and(|u| &u.eintrag == eintrag),
            )
        };
        if gehoert && !ziel {
            self.feld_parken();
        }
        if !gehoert && let Some(rest) = feld_rest(zelle) {
            zelle.remove(&rest);
            teile(zelle).1.set_visible(true);
        }
        let ohne_feld = self
            .z
            .borrow()
            .umbenennen
            .as_ref()
            .is_some_and(|u| u.feld.is_none());
        if ziel && ohne_feld {
            protokoll("Umbenennen: Feld wieder in der Zelle des Objekts");
            self.feld_anlegen(zelle, false);
        }
    }

    /// From a view's unbind.
    pub fn geloest(&self, zelle: &gtk4::Box) {
        let gehoert = self
            .z
            .borrow()
            .umbenennen
            .as_ref()
            .and_then(|u| u.feld.as_ref())
            .is_some_and(|(_, z)| z == zelle);
        if gehoert {
            self.feld_parken();
        }
    }

    // --- signals -----------------------------------------------------

    fn signale(self: &Rc<Self>, scroller: &gtk4::ScrolledWindow) {
        let weak = Rc::downgrade(self);
        self.auswahl.connect_selection_changed(move |_, _, _| {
            if let Some(k) = weak.upgrade() {
                k.langsam_abbrechen();
                if let Some(f) = k.rueckrufe.auswahl.borrow().clone() {
                    f(());
                }
            }
        });
        let weak = Rc::downgrade(self);
        self.auswahl.connect_items_changed(move |m, _, weg, neu| {
            if let Some(k) = weak.upgrade() {
                k.modell_geaendert(m.n_items(), weg, neu);
            }
        });

        // Keys before the list's own bindings.
        let tasten = gtk4::EventControllerKey::new();
        tasten.set_propagation_phase(gtk4::PropagationPhase::Capture);
        let weak = Rc::downgrade(self);
        tasten.connect_key_pressed(move |_, taste, _, zustand| {
            weak.upgrade()
                .map_or(glib::Propagation::Proceed, |k| k.taste(taste, zustand))
        });
        self.liste.add_controller(tasten);

        // Clicks: free space, context menu, slow second click.
        let klick = gtk4::GestureClick::new();
        klick.set_button(0);
        klick.set_propagation_phase(gtk4::PropagationPhase::Capture);
        let weak = Rc::downgrade(self);
        klick.connect_pressed(move |g, n, x, y| {
            if let Some(k) = weak.upgrade() {
                k.gedrueckt(g, n, x, y);
            }
        });
        let weak = Rc::downgrade(self);
        klick.connect_released(move |g, n, x, y| {
            if let Some(k) = weak.upgrade() {
                k.losgelassen(g, n, x, y);
            }
        });
        self.liste.add_controller(klick);

        // Ctrl+wheel zooms through the window's actions.
        let rad = gtk4::EventControllerScroll::new(
            gtk4::EventControllerScrollFlags::VERTICAL | gtk4::EventControllerScrollFlags::DISCRETE,
        );
        rad.set_propagation_phase(gtk4::PropagationPhase::Capture);
        let liste = self.liste.clone();
        rad.connect_scroll(move |c, _, dy| {
            if !c
                .current_event_state()
                .contains(gdk::ModifierType::CONTROL_MASK)
                || dy == 0.0
            {
                return glib::Propagation::Proceed;
            }
            let aktion = if dy < 0.0 {
                "win.vergroessern"
            } else {
                "win.verkleinern"
            };
            let _ = liste.activate_action(aktion, None);
            glib::Propagation::Stop
        });
        scroller.add_controller(rad);

        // Follow the keyboard focus.
        let weak = Rc::downgrade(self);
        self.liste.connect_realize(move |liste| {
            let Some(fenster) = liste.root().and_downcast::<gtk4::Window>() else {
                return;
            };
            let weak = weak.clone();
            fenster.connect_focus_widget_notify(move |f| {
                if let Some(k) = weak.upgrade() {
                    k.fokus_verfolgen(f);
                }
            });
        });
    }

    fn fokus_verfolgen(&self, f: &gtk4::Window) {
        let Some(fokus) = GtkWindowExt::focus(f) else {
            return;
        };
        if !fokus.is_ancestor(&self.liste) {
            return;
        }
        let (Some((e, zelle)), _) = self.eintrag_bei(Some(fokus.clone())) else {
            return;
        };
        let lage = zelle
            .compute_bounds(f)
            .map(|r| {
                format!(
                    "{:.0} {:.0} {:.0} {:.0}",
                    r.x(),
                    r.y(),
                    r.width(),
                    r.height()
                )
            })
            .unwrap_or_default();
        protokoll(&format!(
            "Fokus {}: {} | {lage}",
            self.art.name(),
            e.daten().name
        ));
        // While renaming, the object being renamed stays the cursor.
        if self.z.borrow().umbenennen.is_none() {
            self.z.borrow_mut().cursor = Some(e.daten().name.clone());
        }
    }

    fn modell_geaendert(self: &Rc<Self>, n: u32, weg: u32, neu: u32) {
        // A lease whose object left the model ends without a rename.
        let verloren = self
            .z
            .borrow()
            .umbenennen
            .as_ref()
            .is_some_and(|u| self.position_des(&u.eintrag).is_none());
        if verloren && weg > 0 && weg != neu {
            protokoll("Umbenennen beendet: Objekt nicht mehr im Ordner");
            self.umbenennen_ende(false);
        }
        let wartet = self.z.borrow().warten_auf.clone();
        if let Some((name, umbenennen)) = wartet
            && let Some(pos) = self.position_von(&name)
        {
            self.z.borrow_mut().warten_auf = None;
            self.z.borrow_mut().cursor = Some(name.clone());
            let weak = Rc::downgrade(self);
            glib::idle_add_local_once(move || {
                if let Some(k) = weak.upgrade() {
                    k.fokus_auf(pos, true);
                    if umbenennen {
                        k.umbenennen_starten();
                    }
                }
            });
            return;
        }
        // A new order (sorting, filter): GTK keeps the selection but not
        // the focus; give it back to the same object.
        if weg > 1 && weg == neu && weg <= n {
            self.nach_umordnen();
        }
    }

    /// Puts the keyboard focus back on the cursor object after the order
    /// changed (once per main loop turn).
    pub fn nach_umordnen(self: &Rc<Self>) {
        if self.z.borrow().umordnen_geplant {
            return;
        }
        self.z.borrow_mut().umordnen_geplant = true;
        let weak = Rc::downgrade(self);
        glib::idle_add_local_once(move || {
            let Some(k) = weak.upgrade() else {
                return;
            };
            k.z.borrow_mut().umordnen_geplant = false;
            let im_fokus = k.liste.has_focus() || k.liste.focus_child().is_some();
            if let Some(pos) = k.cursor_position() {
                if im_fokus {
                    k.fokus_auf(pos, false);
                } else {
                    k.scrollen_zu(pos);
                }
                protokoll(&format!(
                    "Fokus nach Umordnen {}: {} ({pos})",
                    k.art.name(),
                    k.z.borrow().cursor.as_deref().unwrap_or("")
                ));
            }
        });
    }

    fn taste(self: &Rc<Self>, taste: gdk::Key, zustand: gdk::ModifierType) -> glib::Propagation {
        if self.z.borrow().umbenennen.is_some() {
            return self.feld_taste(taste);
        }
        let suche = self.tipp.offen();
        let befehl = zustand.intersects(
            gdk::ModifierType::CONTROL_MASK
                | gdk::ModifierType::ALT_MASK
                | gdk::ModifierType::SUPER_MASK,
        );
        let umschalt = zustand.contains(gdk::ModifierType::SHIFT_MASK);
        match taste {
            gdk::Key::BackSpace if suche => {
                if self.tipp.zuruecknehmen().is_empty() {
                    self.tipp.schliessen();
                } else {
                    self.suchen(0, true);
                }
                glib::Propagation::Stop
            }
            gdk::Key::BackSpace if !befehl => {
                if let Some(f) = self.rueckrufe.nach_oben.borrow().clone() {
                    f(());
                }
                glib::Propagation::Stop
            }
            gdk::Key::Escape if suche => {
                self.tipp.schliessen();
                glib::Propagation::Stop
            }
            gdk::Key::Down | gdk::Key::Up if suche && !befehl => {
                let start = self.cursor_position().unwrap_or(0);
                let n = self.auswahl.n_items().max(1);
                if taste == gdk::Key::Down {
                    self.suchen((start + 1) % n, true);
                } else {
                    self.suchen((start + n - 1) % n, false);
                }
                glib::Propagation::Stop
            }
            gdk::Key::Return | gdk::Key::KP_Enter | gdk::Key::ISO_Enter if !befehl => {
                self.tipp.schliessen();
                self.aktivieren_auswahl();
                glib::Propagation::Stop
            }
            gdk::Key::Tab if suche => {
                self.tipp.schliessen();
                glib::Propagation::Proceed
            }
            gdk::Key::Menu => {
                self.menue_an_fokus();
                glib::Propagation::Stop
            }
            gdk::Key::F10 if umschalt && !befehl => {
                self.menue_an_fokus();
                glib::Propagation::Stop
            }
            _ if befehl => glib::Propagation::Proceed,
            _ => match taste.to_unicode() {
                Some(' ') if !suche => glib::Propagation::Proceed,
                Some(c) if (c == '/' || c == '~') && !suche => {
                    if let Some(f) = self.rueckrufe.ortsfeld.borrow().clone() {
                        f(c);
                        return glib::Propagation::Stop;
                    }
                    glib::Propagation::Proceed
                }
                Some(c) if !c.is_control() => {
                    self.tipp.anhaengen(c);
                    self.suchen(0, true);
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            },
        }
    }

    fn suchen(self: &Rc<Self>, start: u32, vorwaerts: bool) {
        let eingabe = self.tipp.eingabe();
        let model = &self.auswahl;
        let gefunden =
            crate::ansichtsregeln::treffer(model.n_items(), start, vorwaerts, &eingabe, |i| {
                model
                    .item(i)
                    .and_downcast::<Eintrag>()
                    .map(|e| crate::ansichtsregeln::suchform(&e.daten().anzeige))
                    .unwrap_or_default()
            });
        protokoll(&format!(
            "Tippsuche {}: „{eingabe}“ → {}",
            self.art.name(),
            gefunden.map_or("kein Treffer".to_owned(), |p| p.to_string())
        ));
        if let Some(pos) = gefunden {
            self.fokus_auf(pos, true);
        }
        self.tipp.zeigen(gefunden.is_some());
    }

    // --- clicks ------------------------------------------------------

    fn gedrueckt(self: &Rc<Self>, g: &gtk4::GestureClick, n: i32, x: f64, y: f64) {
        self.langsam_abbrechen();
        let knopf = g.current_button();
        let zustand = g.current_event_state();
        let (treffer, drin) = self.position_bei(x, y);
        if !drin {
            return;
        }
        let modifikator =
            zustand.intersects(gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK);
        match knopf {
            gdk::BUTTON_SECONDARY => {
                g.set_state(gtk4::EventSequenceState::Claimed);
                self.umbenennen_beenden(true);
                self.tipp.schliessen();
                match &treffer {
                    Some((pos, _)) if !self.auswahl.is_selected(*pos) => {
                        self.fokus_auf(*pos, true);
                    }
                    Some(_) => {}
                    None => {
                        self.auswahl.unselect_all();
                        self.liste.grab_focus();
                    }
                }
                self.menue_zeigen(treffer.is_some(), x, y);
            }
            gdk::BUTTON_PRIMARY => {
                if modifikator {
                    return;
                }
                match treffer {
                    None => {
                        self.umbenennen_beenden(true);
                        self.auswahl.unselect_all();
                        self.liste.grab_focus();
                    }
                    Some((pos, zelle)) => {
                        // A slow second click on the name of the one
                        // selected object renames it (GtkTreeView's
                        // „click again to edit“); decided on release.
                        let auf_name =
                            self.liste
                                .pick(x, y, gtk4::PickFlags::DEFAULT)
                                .is_some_and(|w| {
                                    w.is::<gtk4::Label>() && w.parent().as_ref() == Some(&zelle)
                                });
                        let einzeln = self.auswahl.selection().size() == 1
                            && self.auswahl.is_selected(pos)
                            && (self.liste.has_focus() || self.liste.focus_child().is_some());
                        self.z.borrow_mut().gedrueckt = Some((x, y, n == 1 && auf_name && einzeln));
                    }
                }
            }
            _ => {}
        }
    }

    fn losgelassen(self: &Rc<Self>, g: &gtk4::GestureClick, n: i32, x: f64, y: f64) {
        let gedrueckt = self.z.borrow_mut().gedrueckt.take();
        let Some((px, py, langsam)) = gedrueckt else {
            return;
        };
        if g.current_button() != gdk::BUTTON_PRIMARY || n != 1 || !langsam {
            return;
        }
        if (px - x).abs() > 4.0 || (py - y).abs() > 4.0 {
            return;
        }
        let warten = self.liste.settings().gtk_double_click_time().max(200) as u64;
        let weak = Rc::downgrade(self);
        let quelle = glib::timeout_add_local_once(Duration::from_millis(warten + 50), move || {
            if let Some(k) = weak.upgrade() {
                k.z.borrow_mut().langsam = None;
                protokoll("langsamer zweiter Klick: Umbenennen");
                k.umbenennen_starten();
            }
        });
        self.z.borrow_mut().langsam = Some(quelle);
    }

    fn langsam_abbrechen(&self) {
        if let Some(q) = self.z.borrow_mut().langsam.take() {
            q.remove();
        }
    }

    // --- focus, selection --------------------------------------------

    pub fn fokus(&self) {
        match self.cursor_position() {
            Some(pos) => self.fokus_auf(pos, false),
            None => {
                self.liste.grab_focus();
            }
        }
    }

    fn scrollen(&self, pos: u32, flags: u32) {
        if pos >= self.auswahl.n_items() {
            return;
        }
        // SAFETY: `liste` is the live GridView or ColumnView of this kern;
        // GTK accepts NULL for the column and the scroll info.
        unsafe {
            match self.art {
                Art::Symbole => gtk_grid_view_scroll_to(
                    self.liste.as_ptr() as *mut gtk4::ffi::GtkGridView,
                    pos,
                    flags,
                    std::ptr::null_mut(),
                ),
                Art::Liste => gtk_column_view_scroll_to(
                    self.liste.as_ptr() as *mut gtk4::ffi::GtkColumnView,
                    pos,
                    std::ptr::null_mut(),
                    flags,
                    std::ptr::null_mut(),
                ),
            }
        }
    }

    /// Scrolls to `pos` and gives it the keyboard focus, selecting it alone
    /// if `waehlen`.
    pub fn fokus_auf(&self, pos: u32, waehlen: bool) {
        if pos >= self.auswahl.n_items() {
            return;
        }
        // GTK moves the focus to the item only while the list has it.
        if !self.liste.has_focus() && self.liste.focus_child().is_none() {
            self.liste.grab_focus();
        }
        let flags = SCROLL_FOCUS | if waehlen { SCROLL_SELECT } else { SCROLL_NONE };
        self.scrollen(pos, flags);
        if let Some(e) = self.eintrag(pos) {
            self.z.borrow_mut().cursor = Some(e.daten().name.clone());
        }
    }

    pub fn scrollen_zu(&self, pos: u32) {
        self.scrollen(pos, SCROLL_NONE);
    }

    pub fn position_von(&self, name: &str) -> Option<u32> {
        (0..self.auswahl.n_items())
            .find(|&i| self.eintrag(i).is_some_and(|e| e.daten().name == name))
    }

    fn position_des(&self, eintrag: &Eintrag) -> Option<u32> {
        (0..self.auswahl.n_items()).find(|&i| self.eintrag(i).as_ref() == Some(eintrag))
    }

    pub fn cursor_position(&self) -> Option<u32> {
        let name = self.z.borrow().cursor.clone()?;
        self.position_von(&name)
    }

    pub fn cursor(&self) -> Option<String> {
        self.z.borrow().cursor.clone()
    }

    pub fn gewaehlte(&self) -> Vec<Eintrag> {
        let auswahl = self.auswahl.selection();
        (0..auswahl.size())
            .filter_map(|i| self.eintrag(auswahl.nth(i as u32)))
            .collect()
    }

    pub fn alles_auswaehlen(&self) {
        self.auswahl.select_all();
    }

    /// Selects the objects called `namen` (file names) and focuses the
    /// first; names not (yet) in the model wait for it.
    pub fn waehlen(self: &Rc<Self>, namen: &[String]) {
        let n = self.auswahl.n_items();
        let gesucht: HashSet<&str> = namen.iter().map(String::as_str).collect();
        let neu = gtk4::Bitset::new_empty();
        let mut erste = None;
        for i in 0..n {
            if let Some(e) = self.eintrag(i)
                && gesucht.contains(e.daten().name.as_str())
            {
                neu.add(i);
                erste.get_or_insert(i);
            }
        }
        match erste {
            Some(pos) => {
                self.auswahl
                    .set_selection(&neu, &gtk4::Bitset::new_range(0, n));
                self.z.borrow_mut().cursor = self.eintrag(pos).map(|e| e.daten().name.clone());
                self.fokus_auf(pos, false);
            }
            None => {
                if let Some(name) = namen.first() {
                    self.z.borrow_mut().warten_auf = Some((name.clone(), false));
                }
            }
        }
    }

    /// For a new folder or document: rename `name` as soon as it is in
    /// the model.
    pub fn umbenennen_wenn_da(self: &Rc<Self>, name: &str) {
        match self.position_von(name) {
            Some(pos) => {
                self.fokus_auf(pos, true);
                self.umbenennen_starten();
            }
            None => self.z.borrow_mut().warten_auf = Some((name.to_owned(), true)),
        }
    }

    /// A new folder is shown: forget cursor, rename and type-ahead.
    pub fn ordner_gewechselt(self: &Rc<Self>) {
        self.umbenennen_ende(false);
        self.tipp.schliessen();
        self.langsam_abbrechen();
        let mut z = self.z.borrow_mut();
        z.cursor = None;
        z.warten_auf = None;
    }

    /// The folder is read: focus `name` (the folder one came up from) or
    /// the first object, without selecting it.
    pub fn geladen(&self, name: Option<&str>) {
        self.liste.grab_focus();
        match name.and_then(|n| self.position_von(n)) {
            Some(pos) => self.fokus_auf(pos, true),
            None if self.auswahl.n_items() > 0 => self.fokus_auf(0, false),
            None => {}
        }
    }

    fn aktivieren_auswahl(&self) {
        let mut gewaehlt = self.gewaehlte();
        if gewaehlt.is_empty()
            && let Some(e) = self.cursor_position().and_then(|p| self.eintrag(p))
        {
            gewaehlt.push(e);
        }
        if !gewaehlt.is_empty()
            && let Some(f) = self.rueckrufe.aktiviert.borrow().clone()
        {
            f(gewaehlt);
        }
    }

    /// GTK's activate (double click): the selection if the object is in
    /// it, else the object alone.
    pub fn aktiviert(&self, pos: u32) {
        let Some(e) = self.eintrag(pos) else {
            return;
        };
        let liste = if self.auswahl.is_selected(pos) {
            self.gewaehlte()
        } else {
            vec![e]
        };
        if let Some(f) = self.rueckrufe.aktiviert.borrow().clone() {
            f(liste);
        }
    }

    // --- context menu ------------------------------------------------

    fn menue_an_fokus(self: &Rc<Self>) {
        let ort = self
            .cursor_position()
            .and_then(|p| self.eintrag(p))
            .and_then(|e| self.zelle_von(&e))
            .and_then(|z| z.compute_bounds(&self.liste))
            .map(|r| {
                (
                    f64::from(r.x() + r.width() / 2.0),
                    f64::from(r.y() + r.height()),
                )
            });
        let (x, y) = ort.unwrap_or((12.0, 12.0));
        let hat_auswahl = self.auswahl.selection().size() > 0;
        self.menue_zeigen(hat_auswahl, x, y);
    }

    /// The menu for the selection (or the background) at (x, y) in list
    /// coordinates.
    fn menue_zeigen(&self, objekte: bool, x: f64, y: f64) {
        let (ordner, sortierung) = self
            .rueckrufe
            .umgebung
            .borrow()
            .as_ref()
            .map_or((None, Sortierung::default()), |f| f());
        let im_papierkorb = ordner.as_ref().is_some_and(|o| o.has_uri_scheme("trash"));
        let gewaehlt = self.gewaehlte();
        let ort = if objekte && !gewaehlt.is_empty() {
            let ordner_zahl = gewaehlt.iter().filter(|e| e.daten().ist_ordner).count();
            let erster = gewaehlt[0].daten().typ.clone();
            let gleich = gewaehlt.iter().all(|e| e.daten().typ == erster);
            Ort::Auswahl(Auswahlinfo {
                anzahl: gewaehlt.len(),
                ordner: ordner_zahl,
                gemeinsamer_typ: gleich.then_some(erster),
            })
        } else {
            Ort::Hintergrund
        };
        let programme = match &ort {
            Ort::Auswahl(Auswahlinfo {
                gemeinsamer_typ: Some(t),
                ..
            }) => kontextmenue::programme_fuer(t),
            _ => Vec::new(),
        };
        let vorlagen = if ort == Ort::Hintergrund {
            kontextmenue::vorlagen(
                glib::user_special_dir(glib::UserDirectory::Templates).as_deref(),
                &glib::home_dir(),
            )
        } else {
            Vec::new()
        };
        let umgebung = Umgebung {
            im_papierkorb,
            programme,
            vorlagen,
            sortierung,
        };
        let gruppen = kontextmenue::zeilen(&ort, &umgebung);
        let liste = self.liste.clone();
        let modell =
            kontextmenue::als_menue(&gruppen, &|a| kontextmenue::aktion_verfuegbar(&liste, a));
        self.menue.remove_all();
        for i in 0..modell.n_items() {
            if let Some(abschnitt) = modell.item_link(i, "section") {
                self.menue.append_section(None, &abschnitt);
            }
        }
        protokoll(&format!(
            "Kontextmenü {}: {}",
            self.art.name(),
            kontextmenue::protokoll(&self.menue)
        ));
        let punkt = self
            .liste
            .compute_point(
                &self.wurzel,
                &gtk4::graphene::Point::new(x as f32, y as f32),
            )
            .map_or((x, y), |p| (f64::from(p.x()), f64::from(p.y())));
        if let Some(popover) = self.anker.popover() {
            popover.set_pointing_to(Some(&gdk::Rectangle::new(
                punkt.0 as i32,
                punkt.1 as i32,
                1,
                1,
            )));
        }
        self.anker.popup();
    }

    // --- rename ------------------------------------------------------

    /// F2: the one selected object, else the focused one.
    pub fn umbenennen_starten(self: &Rc<Self>) {
        if self.z.borrow().umbenennen.is_some() {
            return;
        }
        self.tipp.schliessen();
        let auswahl = self.auswahl.selection();
        let pos = match auswahl.size() {
            0 => self.cursor_position(),
            1 => Some(auswahl.nth(0)),
            _ => {
                protokoll("Umbenennen: mehrere Objekte gewählt, nichts getan");
                return;
            }
        };
        let Some((pos, e)) = pos.and_then(|p| Some((p, self.eintrag(p)?))) else {
            return;
        };
        let vertrag = Mietvertrag::neu(&e);
        protokoll(&format!(
            "Umbenennen begonnen {}: {} (markiert {:?})",
            self.art.name(),
            e.daten().name,
            vertrag.markierung
        ));
        self.z.borrow_mut().umbenennen = Some(vertrag);
        self.fokus_auf(pos, true);
        self.feld_holen(None);
    }

    fn feld_anlegen(self: &Rc<Self>, zelle: &gtk4::Box, fokus: bool) {
        let (text, markierung) = {
            let z = self.z.borrow();
            let Some(u) = z.umbenennen.as_ref() else {
                return;
            };
            (u.text.clone(), u.markierung)
        };
        let (_, beschriftung) = teile(zelle);
        let feld = match self.art {
            Art::Symbole => Feld::mehrzeilig(self.feldbreite.get()),
            Art::Liste => Feld::einzeilig(),
        };
        feld.setzen(&text, markierung);
        zelle.insert_child_after(&feld.widget(), Some(&beschriftung));
        beschriftung.set_visible(false);
        let tasten = gtk4::EventControllerKey::new();
        tasten.set_propagation_phase(gtk4::PropagationPhase::Capture);
        let weak = Rc::downgrade(self);
        tasten.connect_key_pressed(move |_, taste, _, _| {
            let Some(k) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            match taste {
                gdk::Key::Escape => k.umbenennen_ende(false),
                gdk::Key::Return | gdk::Key::KP_Enter | gdk::Key::ISO_Enter => {
                    k.umbenennen_ende(true)
                }
                _ => return glib::Propagation::Proceed,
            }
            glib::Propagation::Stop
        });
        feld.widget().add_controller(tasten);
        // Focus elsewhere ends the rename only while the field is on
        // screen (a click elsewhere); GTK also takes the focus away when
        // the cell scrolls out of view, then the field waits.
        let fokus_weg = gtk4::EventControllerFocus::new();
        let weak = Rc::downgrade(self);
        let fuer_weg = feld.widget().downgrade();
        fokus_weg.connect_leave(move |_| {
            let (weak, fuer_weg) = (weak.clone(), fuer_weg.clone());
            glib::idle_add_local_once(move || {
                let (Some(k), Some(feld)) = (weak.upgrade(), fuer_weg.upgrade()) else {
                    return;
                };
                let zelle = {
                    let z = k.z.borrow();
                    match z.umbenennen.as_ref().and_then(|u| u.feld.clone()) {
                        Some((f, zelle)) if f.widget() == feld => zelle,
                        _ => return,
                    }
                };
                if feld.has_focus() || feld.focus_child().is_some() {
                    return;
                }
                if k.sichtbar(&zelle) {
                    k.umbenennen_ende(true);
                } else {
                    protokoll("Umbenennen wartet: Feld außer Sicht");
                }
            });
        });
        feld.widget().add_controller(fokus_weg);
        if let Some(u) = self.z.borrow_mut().umbenennen.as_mut() {
            u.feld = Some((feld.clone(), zelle.clone()));
        }
        if fokus {
            feld.widget().grab_focus();
            // grab_focus of a GtkText selects everything; set the lease's
            // selection again
            feld.setzen(&text, markierung);
        }
    }

    fn sichtbar(&self, zelle: &gtk4::Box) -> bool {
        zelle.is_mapped()
            && zelle
                .compute_bounds(&self.wurzel)
                .is_some_and(|r| r.y() + r.height() > 0.0 && r.y() < self.wurzel.height() as f32)
    }

    fn feld_parken(&self) {
        let zelle = self
            .z
            .borrow_mut()
            .umbenennen
            .as_mut()
            .and_then(|u| u.parken());
        if let Some(zelle) = zelle {
            teile(&zelle).1.set_visible(true);
            protokoll("Umbenennen: Feld aus recycelter Zelle genommen, Text bleibt");
        }
    }

    /// A key while the field has no focus (it scrolled out of view): the
    /// field comes back and takes the key.
    fn feld_taste(self: &Rc<Self>, taste: gdk::Key) -> glib::Propagation {
        let feld = self
            .z
            .borrow()
            .umbenennen
            .as_ref()
            .and_then(|u| u.feld.as_ref().map(|(f, _)| f.widget()));
        if feld
            .as_ref()
            .is_some_and(|f| f.has_focus() || f.focus_child().is_some())
        {
            return glib::Propagation::Proceed;
        }
        match taste {
            gdk::Key::Escape => {
                self.umbenennen_ende(false);
                return glib::Propagation::Stop;
            }
            gdk::Key::Return | gdk::Key::KP_Enter => {
                self.umbenennen_ende(true);
                return glib::Propagation::Stop;
            }
            _ => {}
        }
        let zeichen = taste.to_unicode().filter(|c| !c.is_control());
        if zeichen.is_none() && taste != gdk::Key::BackSpace {
            return glib::Propagation::Proceed;
        }
        self.feld_holen(Some((taste == gdk::Key::BackSpace, zeichen)));
        glib::Propagation::Stop
    }

    /// Scrolls the object being renamed into view and gives its field the
    /// focus after the next layout, applying `eingabe` first.
    fn feld_holen(self: &Rc<Self>, eingabe: Option<(bool, Option<char>)>) {
        let Some(eintrag) = self
            .z
            .borrow()
            .umbenennen
            .as_ref()
            .map(|u| u.eintrag.clone())
        else {
            return;
        };
        if let Some(pos) = self.position_des(&eintrag) {
            self.scrollen_zu(pos);
        }
        let weak = Rc::downgrade(self);
        let mut versuche = 0;
        let mut eingabe = eingabe;
        glib::idle_add_local(move || {
            let Some(k) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            let feld =
                k.z.borrow()
                    .umbenennen
                    .as_ref()
                    .map(|u| u.feld.as_ref().map(|(f, _)| f.clone()));
            let feld = match feld {
                None => return glib::ControlFlow::Break,
                Some(None) => {
                    if let Some(zelle) = k.zelle_von(&eintrag) {
                        k.feld_anlegen(&zelle, true);
                    }
                    k.z.borrow()
                        .umbenennen
                        .as_ref()
                        .and_then(|u| u.feld.as_ref().map(|(f, _)| f.clone()))
                }
                Some(Some(f)) => Some(f),
            };
            let Some(feld) = feld else {
                versuche += 1;
                if versuche > 20 {
                    protokoll(&format!(
                        "Umbenennen: keine Zelle für {}",
                        eintrag.daten().name
                    ));
                    return glib::ControlFlow::Break;
                }
                return glib::ControlFlow::Continue;
            };
            let w = feld.widget();
            if !w.has_focus() && w.focus_child().is_none() {
                let (text, markierung) = (feld.text(), feld.markierung());
                w.grab_focus();
                feld.setzen(&text, markierung);
            }
            if let Some((rueck, zeichen)) = eingabe.take() {
                feld.eingeben(rueck, zeichen);
                protokoll(&format!(
                    "Umbenennen: Feld zurückgeholt, Text {}",
                    feld.text()
                ));
            }
            glib::ControlFlow::Break
        });
    }

    pub fn umbenennen_beenden(self: &Rc<Self>, uebernehmen: bool) {
        if self.z.borrow().umbenennen.is_some() {
            self.umbenennen_ende(uebernehmen);
        }
    }

    /// Ends the rename: with Enter (or a click elsewhere) the new name is
    /// checked and set, with Esc nothing changes.
    fn umbenennen_ende(self: &Rc<Self>, uebernehmen: bool) {
        let Some(u) = self.z.borrow_mut().umbenennen.take() else {
            return;
        };
        let neu = u.aktueller_text();
        if let Some((feld, zelle)) = &u.feld {
            zelle.remove(&feld.widget());
            teile(zelle).1.set_visible(true);
        }
        let eintrag = u.eintrag;
        let pos = self.position_des(&eintrag);
        if !uebernehmen {
            protokoll(&format!("Umbenennen abgebrochen: {}", eintrag.daten().name));
            if let Some(pos) = pos {
                self.fokus_auf(pos, true);
            }
            return;
        }
        let weak = Rc::downgrade(self);
        glib::idle_add_local_once(move || {
            if let Some(k) = weak.upgrade() {
                k.umbenennen_ausfuehren(eintrag, neu);
            }
        });
    }

    fn umbenennen_ausfuehren(self: &Rc<Self>, eintrag: Eintrag, neu: String) {
        let d = eintrag.daten();
        let fenster = self.liste.root().and_downcast::<gtk4::Window>();
        if d.starter {
            self.starter_umbenennen(eintrag.clone(), neu);
            return;
        }
        let eltern = d.datei.parent();
        let vorhanden = |name: &str| {
            eltern.as_ref().is_some_and(|p| {
                p.child(name).query_file_type(
                    gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
                    gio::Cancellable::NONE,
                ) != gio::FileType::Unknown
            })
        };
        let neu = match umbenennen::pruefen(&d.anzeige, &neu, vorhanden) {
            Ok(None) => {
                protokoll(&format!("Umbenennen ohne Änderung beendet: {}", d.name));
                if let Some(pos) = self.position_des(&eintrag) {
                    self.fokus_auf(pos, true);
                }
                return;
            }
            Ok(Some(neu)) => neu,
            Err(grund) => {
                let text = grund.text(&neu);
                protokoll(&format!("Umbenennen abgelehnt: {text}"));
                umbenennen::fehler_zeigen(fenster.as_ref(), &text);
                return;
            }
        };
        let weak = Rc::downgrade(self);
        let datei = d.datei.clone();
        glib::spawn_future_local(async move {
            let ergebnis = umbenennen::ausfuehren(datei, neu).await;
            let Some(k) = weak.upgrade() else {
                return;
            };
            match ergebnis {
                Ok(neue_datei) => {
                    let name = neue_datei
                        .basename()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    protokoll(&format!("umbenannt in {name}"));
                    k.z.borrow_mut().cursor = Some(name.clone());
                    k.z.borrow_mut().warten_auf = Some((name, false));
                    if let Some(f) = k.rueckrufe.umbenannt.borrow().clone() {
                        f((eintrag, neue_datei));
                    }
                }
                Err(fehler) => {
                    protokoll(&format!("Umbenennen fehlgeschlagen: {}", fehler.message()));
                    let fenster = k.liste.root().and_downcast::<gtk4::Window>();
                    umbenennen::fehler_zeigen(fenster.as_ref(), fehler.message());
                }
            }
        });
    }

    /// A starter keeps its file name; its shown name is the `Name` key in
    /// the language the desktop shows (as the desktop does it).
    fn starter_umbenennen(self: &Rc<Self>, eintrag: Eintrag, neu: String) {
        let d = eintrag.daten();
        let neu: String = neu.chars().filter(|c| *c != '\n' && *c != '\r').collect();
        if neu == d.anzeige {
            return;
        }
        let fenster = self.liste.root().and_downcast::<gtk4::Window>();
        if let Err(p) = namen::check_label(&neu) {
            umbenennen::fehler_zeigen(fenster.as_ref(), &p.text(&neu));
            return;
        }
        let weak = Rc::downgrade(self);
        let datei = d.datei.clone();
        glib::spawn_future_local(async move {
            let ergebnis: Result<(), String> = async {
                let (inhalt, _) = datei
                    .load_contents_future()
                    .await
                    .map_err(|e| e.message().to_owned())?;
                let text = String::from_utf8_lossy(&inhalt).into_owned();
                let keys = glib::KeyFile::new();
                keys.load_from_data(&text, glib::KeyFileFlags::KEEP_TRANSLATIONS)
                    .map_err(|e| e.message().to_owned())?;
                let schluessel = namen::name_key(
                    keys.locale_for_key("Desktop Entry", "Name", None)
                        .as_deref(),
                );
                let neu_text = namen::with_name(&text, &schluessel, &neu)
                    .ok_or_else(|| "Desktop Entry".to_owned())?;
                datei
                    .replace_contents_future(
                        neu_text.into_bytes(),
                        None,
                        false,
                        gio::FileCreateFlags::NONE,
                    )
                    .await
                    .map_err(|(_, e)| e.message().to_owned())?;
                Ok(())
            }
            .await;
            let Some(k) = weak.upgrade() else {
                return;
            };
            match ergebnis {
                Ok(()) => {
                    protokoll(&format!("Starter umbenannt: {neu}"));
                    k.z.borrow_mut().warten_auf = Some((eintrag.daten().name.clone(), false));
                    if let Some(f) = k.rueckrufe.umbenannt.borrow().clone() {
                        f((eintrag.clone(), eintrag.daten().datei.clone()));
                    }
                }
                Err(grund) => {
                    let fenster = k.liste.root().and_downcast::<gtk4::Window>();
                    umbenennen::fehler_zeigen(fenster.as_ref(), &grund);
                }
            }
        });
    }
}

/// The style rules of both views, only with the theme's named colours
/// (light and dark, QD-081). The window adds them to its own provider
/// (`farbschema::folgen`), after its menu rules.
pub const STIL: &str = "
gridview.querencia-symbole > child,
gridview.querencia-symbole > child:selected,
gridview.querencia-symbole > child:hover {
  background: none;
  color: inherit;
  padding: 4px 2px 6px 2px;
  outline-color: transparent;
}
gridview.querencia-symbole > child:focus:focus-visible {
  outline: 1px dashed alpha(@theme_text_color, 0.6);
  outline-offset: -1px;
}
gridview.querencia-symbole > child box.zelle {
  margin: 0;
  border-spacing: 2px;
}
gridview.querencia-symbole .beschriftung {
  padding: 0 3px;
  border-radius: 3px;
  color: @theme_text_color;
}
gridview.querencia-symbole > child:selected .beschriftung {
  background-color: @theme_selected_bg_color;
  color: @theme_selected_fg_color;
}
gridview.querencia-symbole:not(:focus-within) > child:selected .beschriftung {
  background-color: @theme_unfocused_selected_bg_color;
  color: @theme_unfocused_selected_fg_color;
}
gridview.querencia-symbole .zusatz {
  font-size: smaller;
}
gridview.querencia-symbole > child:selected image {
  -gtk-icon-filter: brightness(0.78);
}
gridview.querencia-symbole > child:hover image {
  -gtk-icon-filter: brightness(1.12);
}
gridview.querencia-symbole > child:selected:hover image {
  -gtk-icon-filter: brightness(0.86);
}
gridview.querencia-symbole textview.umbenennen,
columnview.querencia-liste text.umbenennen {
  border: 1px solid @theme_text_color;
  padding: 0 2px;
  background-color: @theme_base_color;
  color: @theme_text_color;
}
columnview.querencia-liste > listview > row {
  padding: 0;
}
columnview.querencia-liste > listview > row > cell {
  padding: 1px 6px;
}
columnview.querencia-liste > listview > row:selected {
  background-color: @theme_selected_bg_color;
  color: @theme_selected_fg_color;
}
columnview.querencia-liste:not(:focus-within) > listview > row:selected {
  background-color: @theme_unfocused_selected_bg_color;
  color: @theme_unfocused_selected_fg_color;
}
frame.suchfeld {
  background-color: @theme_base_color;
  border: 1px solid @borders;
  border-radius: 0;
  padding: 2px 8px;
  box-shadow: 0 1px 3px alpha(black, 0.3);
}
frame.suchfeld.ohne-treffer label {
  color: @error_color;
}
menubutton.menue-anker > button {
  min-width: 0;
  min-height: 0;
  padding: 0;
  margin: 0;
  border: none;
  background: none;
  box-shadow: none;
}
@media (prefers-color-scheme: dark) {
  gridview.querencia-symbole > child:selected image {
    -gtk-icon-filter: brightness(0.7);
  }
  frame.suchfeld {
    box-shadow: 0 1px 3px alpha(black, 0.6);
  }
}
";
