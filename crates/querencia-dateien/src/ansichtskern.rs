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
use std::time::{Duration, Instant};

use gtk4::prelude::*;
use gtk4::{gdk, gio, glib};

use querencia_dateiarbeit::namen;

use crate::ansicht::{Ansicht, Kontextziel};
pub use crate::eintrag::Eintrag;
use crate::i18n::gettext;
use crate::konfig::Werte;
use crate::kontextmenue;
use crate::modell::{Ordnermodell, Zustand as Ladezustand};
use crate::regeln::{Ansichtsart, Zoom};
pub use crate::regeln::{Kriterium, Sortierung};
use crate::tippsuche::Tippsuche;
use crate::umbenennen::{self, Feld, Mietvertrag};

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

thread_local! {
    /// Type descriptions per content type (few types, many files).
    static TYPEN: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
}

/// Size in bytes (the sort key the entry keeps).
pub fn groesse(e: &Eintrag) -> u64 {
    e.daten().sort.groesse
}

/// Modification time, seconds since 1970.
pub fn zeit(e: &Eintrag) -> i64 {
    e.daten().sort.zeit
}

/// The description of the entry's type („Einfaches Textdokument“).
pub fn typ_text(e: &Eintrag) -> String {
    let typ = &e.daten().typ;
    TYPEN.with(|t| {
        t.borrow_mut()
            .entry(typ.clone())
            .or_insert_with(|| e.beschreibung())
            .clone()
    })
}

/// `st_mode` of the entry, if the backend reports it.
pub fn modus(e: &Eintrag) -> Option<u32> {
    let info = &e.daten().info;
    info.has_attribute("unix::mode")
        .then(|| info.attribute_uint32("unix::mode"))
}

/// Owner and group names (`owner::user`, `owner::group`; empty when the
/// folder read did not ask for them).
pub fn eigentuemer(e: &Eintrag) -> String {
    e.daten()
        .info
        .attribute_string("owner::user")
        .map(|s| s.to_string())
        .unwrap_or_default()
}

pub fn gruppe(e: &Eintrag) -> String {
    e.daten()
        .info
        .attribute_string("owner::group")
        .map(|s| s.to_string())
        .unwrap_or_default()
}

/// Runs `f` once after the next frame of `widget` is painted (GDK frame
/// clock `after-paint`): the Messung lines include layout and drawing.
fn nach_dem_zeichnen(widget: &gtk4::Widget, f: impl FnOnce() + 'static) {
    let Some(uhr) = widget.frame_clock() else {
        return;
    };
    let f = RefCell::new(Some(f));
    let id: Rc<RefCell<Option<glib::SignalHandlerId>>> = Rc::new(RefCell::new(None));
    let fuer_ende = id.clone();
    let handler = uhr.connect_after_paint(move |uhr| {
        if let Some(f) = f.borrow_mut().take() {
            f();
        }
        if let Some(h) = fuer_ende.borrow_mut().take() {
            uhr.disconnect(h);
        }
    });
    *id.borrow_mut() = Some(handler);
    widget.queue_draw();
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
    /// Double click or Enter: the objects to open; `true` for a new window
    /// (Umschalt+Doppelklick, middle button).
    pub aktiviert: Rueckruf<(Vec<Eintrag>, bool)>,
    /// Backspace: the parent folder.
    pub nach_oben: Rueckruf<()>,
    /// „/“ or „~“ typed: the location entry, starting with the character.
    pub ortsfeld: Rueckruf<char>,
    /// The selection changed (status bar).
    pub auswahl: Rueckruf<()>,
    /// Ctrl+wheel: one level larger (+1) or smaller (-1).
    pub zoom: Rueckruf<i32>,
    /// Right click or menu key: for the selection (`true`) or the
    /// background, at (x, y) in the list widget. Without it the view shows
    /// the menu of [`crate::kontextmenue`] itself.
    pub kontextmenue: Rueckruf<(bool, f64, f64)>,
    /// A checked new name for an object: the window renames it (job).
    pub umbenennen: Rueckruf<(Eintrag, String)>,
    /// A message for the status bar.
    pub melden: Rueckruf<String>,
    /// A click on a column header asks for this order (list view).
    pub sortieren: Rueckruf<Sortierung>,
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
    /// Zoom level in percent.
    zoom: u32,
    /// The next GTK activate opens in a new window (Umschalt+Doppelklick).
    neues_fenster: bool,
    /// Slow second click on a selected name: the pending rename.
    langsam: Option<glib::SourceId>,
    gedrueckt: Option<(f64, f64, bool)>,
    /// Umschalt+Gummiband: start point in list coordinates.
    gummiband: Option<(f64, f64)>,
    umordnen_geplant: bool,
    /// Start of a reorder (sort, filter), set before the list rebuilds.
    umordnen_start: Option<Instant>,
    /// Reading a folder: start and path, for the Messung lines.
    laden: Option<(Instant, String)>,
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
        kontextmenue::anker_anlegen(&wurzel);
        let kern = Rc::new(Self {
            art,
            auswahl,
            liste,
            behaelter: RefCell::new(None),
            wurzel,
            tipp,
            z: RefCell::new(Zustand::default()),
            zellen: RefCell::new(HashMap::new()),
            rueckrufe: Rueckrufe::default(),
            feldbreite: Cell::new(-1),
        });
        // Connected before the list gets the model, so it runs before the
        // list rebuilds its cells: the start of a reorder for the Messung.
        let weak = Rc::downgrade(&kern);
        kern.auswahl.connect_items_changed(move |_, _, weg, neu| {
            if let Some(k) = weak.upgrade()
                && weg > 1
                && weg == neu
            {
                k.z.borrow_mut().umordnen_start = Some(Instant::now());
            }
        });
        let _ = scroller;
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

    /// The zoom level in percent.
    pub fn zoom(&self) -> u32 {
        self.z.borrow().zoom
    }

    pub fn zoom_merken(self: &Rc<Self>, prozent: u32) {
        self.umbenennen_beenden(true);
        self.z.borrow_mut().zoom = prozent;
    }

    /// Nautilus' „Einfacher Klick zum Öffnen“ (Reiter „Verhalten“).
    pub fn einfacher_klick(&self, an: bool) {
        if let Some(g) = self.liste.downcast_ref::<gtk4::GridView>() {
            g.set_single_click_activate(an);
        } else if let Some(c) = self.liste.downcast_ref::<gtk4::ColumnView>() {
            c.set_single_click_activate(an);
        }
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

    /// After the list got the model: the kern's own handlers (they run
    /// after the list's).
    pub fn anschliessen(self: &Rc<Self>, scroller: &gtk4::ScrolledWindow) {
        let weak = Rc::downgrade(self);
        self.auswahl.connect_selection_changed(move |_, _, _| {
            if let Some(k) = weak.upgrade() {
                k.langsam_abbrechen();
                if k.liste.is_mapped() {
                    let g = k.gewaehlte();
                    let anfang: Vec<&str> =
                        g.iter().take(6).map(|e| e.daten().name.as_str()).collect();
                    protokoll(&format!(
                        "Auswahl {} {}: {}",
                        k.art.name(),
                        g.len(),
                        anfang.join(", ")
                    ));
                }
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

        // Umschalt+Gummiband: GTK 4.22 starts no rubberband while Shift is
        // held (measured); Nautilus 2.30 added the objects. Own band on free
        // space, only with Shift (GTK's own band stays for the rest).
        let ziehen = gtk4::GestureDrag::new();
        ziehen.set_propagation_phase(gtk4::PropagationPhase::Capture);
        let weak = Rc::downgrade(self);
        ziehen.connect_drag_begin(move |g, x, y| {
            if let Some(k) = weak.upgrade() {
                k.gummiband_beginn(g, x, y);
            }
        });
        let weak = Rc::downgrade(self);
        ziehen.connect_drag_update(move |g, dx, dy| {
            if let Some(k) = weak.upgrade() {
                k.gummiband_zeigen(g, dx, dy);
            }
        });
        let weak = Rc::downgrade(self);
        ziehen.connect_drag_end(move |g, dx, dy| {
            if let Some(k) = weak.upgrade() {
                k.gummiband_ende(g, dx, dy);
            }
        });
        self.liste.add_controller(ziehen);

        // Ctrl+wheel zooms through the window's actions.
        let rad = gtk4::EventControllerScroll::new(
            gtk4::EventControllerScrollFlags::VERTICAL | gtk4::EventControllerScrollFlags::DISCRETE,
        );
        rad.set_propagation_phase(gtk4::PropagationPhase::Capture);
        let weak = Rc::downgrade(self);
        rad.connect_scroll(move |c, _, dy| {
            if !c
                .current_event_state()
                .contains(gdk::ModifierType::CONTROL_MASK)
                || dy == 0.0
            {
                return glib::Propagation::Proceed;
            }
            let schritt = if dy < 0.0 { 1 } else { -1 };
            if let Some(f) = weak
                .upgrade()
                .and_then(|k| k.rueckrufe.zoom.borrow().clone())
            {
                f(schritt);
            }
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
        let start = self.z.borrow_mut().umordnen_start.take();
        if let Some(t0) = start
            && self.liste.is_mapped()
        {
            eprintln!(
                "{PROGRAMM}: Messung Umordnen {}: {n} Objekte, Zellen neu nach {} ms",
                self.art.name(),
                t0.elapsed().as_millis()
            );
            let art = self.art.name();
            nach_dem_zeichnen(&self.liste, move || {
                eprintln!(
                    "{PROGRAMM}: Messung Umordnen {art}: Bild nach {} ms",
                    t0.elapsed().as_millis()
                );
            });
        }
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
                        k.umbenennen_starten(false);
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
            gdk::BUTTON_MIDDLE => {
                // the middle button opens a folder in a new window
                if let Some((pos, _)) = treffer {
                    g.set_state(gtk4::EventSequenceState::Claimed);
                    if let Some(e) = self.eintrag(pos)
                        && let Some(f) = self.rueckrufe.aktiviert.borrow().clone()
                    {
                        f((vec![e], true));
                    }
                }
            }
            gdk::BUTTON_PRIMARY => {
                // Umschalt+Doppelklick: GTK's activate follows this press
                self.z.borrow_mut().neues_fenster =
                    n == 2 && zustand.contains(gdk::ModifierType::SHIFT_MASK);
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
                k.umbenennen_starten(false);
            }
        });
        self.z.borrow_mut().langsam = Some(quelle);
    }

    fn gummiband_beginn(&self, g: &gtk4::GestureDrag, x: f64, y: f64) {
        let umschalt = g
            .current_event_state()
            .contains(gdk::ModifierType::SHIFT_MASK)
            && !g
                .current_event_state()
                .contains(gdk::ModifierType::CONTROL_MASK);
        let (treffer, drin) = self.position_bei(x, y);
        if !umschalt || !drin || treffer.is_some() || g.current_button() != gdk::BUTTON_PRIMARY {
            g.set_state(gtk4::EventSequenceState::Denied);
            return;
        }
        g.set_state(gtk4::EventSequenceState::Claimed);
        self.z.borrow_mut().gummiband = Some((x, y));
    }

    /// The band as (x, y, width, height) in list coordinates.
    fn gummiband_rechteck(&self, dx: f64, dy: f64) -> Option<(f64, f64, f64, f64)> {
        let (x, y) = self.z.borrow().gummiband?;
        Some((x.min(x + dx), y.min(y + dy), dx.abs(), dy.abs()))
    }

    fn gummiband_zeigen(&self, _g: &gtk4::GestureDrag, dx: f64, dy: f64) {
        let Some((x, y, b, h)) = self.gummiband_rechteck(dx, dy) else {
            return;
        };
        let band = self.gummiband_widget();
        let p = self
            .liste
            .compute_point(
                &self.wurzel,
                &gtk4::graphene::Point::new(x as f32, y as f32),
            )
            .map_or((x, y), |p| (f64::from(p.x()), f64::from(p.y())));
        band.set_margin_start(p.0.max(0.0) as i32);
        band.set_margin_top(p.1.max(0.0) as i32);
        band.set_size_request(b as i32, h as i32);
        band.set_visible(true);
    }

    fn gummiband_widget(&self) -> gtk4::Widget {
        let mut kind = self.wurzel.first_child();
        while let Some(k) = kind {
            if k.has_css_class("gummiband") {
                return k;
            }
            kind = k.next_sibling();
        }
        let band = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        band.add_css_class("gummiband");
        band.set_can_target(false);
        band.set_halign(gtk4::Align::Start);
        band.set_valign(gtk4::Align::Start);
        self.wurzel.add_overlay(&band);
        band.upcast()
    }

    /// Adds the objects whose cells the band touches to the selection.
    fn gummiband_ende(&self, _g: &gtk4::GestureDrag, dx: f64, dy: f64) {
        let rechteck = self.gummiband_rechteck(dx, dy);
        self.z.borrow_mut().gummiband = None;
        self.gummiband_widget().set_visible(false);
        let Some((x, y, b, h)) = rechteck else {
            return;
        };
        let behaelter = self.behaelter();
        let neu = gtk4::Bitset::new_empty();
        for (zelle, li) in self.lebende_zellen() {
            // the item widget (grid child, list row) is the hit area
            let mut w: Option<gtk4::Widget> = Some(zelle.clone().upcast());
            while let Some(k) = w.clone() {
                if k.parent().as_ref() == Some(&behaelter) {
                    break;
                }
                w = k.parent();
            }
            let Some(r) = w.and_then(|w| w.compute_bounds(&self.liste)) else {
                continue;
            };
            let (rx, ry, rb, rh) = (
                f64::from(r.x()),
                f64::from(r.y()),
                f64::from(r.width()),
                f64::from(r.height()),
            );
            if rb > 0.0 && rx < x + b && rx + rb > x && ry < y + h && ry + rh > y {
                neu.add(li.position());
            }
        }
        protokoll(&format!(
            "Umschalt+Gummiband {}: {} Objekte dazu",
            self.art.name(),
            neu.size()
        ));
        if neu.size() > 0 {
            self.auswahl.set_selection(&neu, &neu);
        }
    }

    fn langsam_abbrechen(&self) {
        if let Some(q) = self.z.borrow_mut().langsam.take() {
            q.remove();
        }
    }

    // --- focus, selection --------------------------------------------

    /// Keyboard focus into the list: on the cursor object, else on the
    /// first selected one (after switching views the selection is shared,
    /// the cursor is the other view's), else the list itself.
    pub fn fokus(&self) {
        let erste_gewaehlte = || {
            let a = self.auswahl.selection();
            (a.size() > 0).then(|| a.nth(0))
        };
        match self.cursor_position().or_else(erste_gewaehlte) {
            Some(pos) => self.fokus_auf(pos, false),
            None => {
                self.liste.grab_focus();
            }
        }
    }

    /// The object whose icon or name is at (x, y) in the list widget (not
    /// the free space of a cell), for drag and drop.
    pub fn eintrag_bei_punkt(&self, x: f64, y: f64) -> Option<Eintrag> {
        let w = self.liste.pick(x, y, gtk4::PickFlags::DEFAULT)?;
        if !(w.is::<gtk4::Image>() || w.is::<gtk4::Label>()) {
            return None;
        }
        let (Some((e, zelle)), _) = self.eintrag_bei(Some(w.clone())) else {
            return None;
        };
        (w.parent().as_ref() == Some(&zelle)).then_some(e)
    }

    // called by the frame through `Ansicht::umbenennen_laeuft`
    #[allow(dead_code)]
    pub fn umbenennen_laeuft(&self) -> bool {
        self.z.borrow().umbenennen.is_some()
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

    pub fn gewaehlte(&self) -> Vec<Eintrag> {
        let auswahl = self.auswahl.selection();
        (0..auswahl.size())
            .filter_map(|i| self.eintrag(auswahl.nth(i as u32)))
            .collect()
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

    /// A new folder `pfad` is read: forget cursor, rename and type-ahead.
    pub fn ordner_gewechselt(self: &Rc<Self>, pfad: &str) {
        self.umbenennen_ende(false);
        self.tipp.schliessen();
        self.langsam_abbrechen();
        let mut z = self.z.borrow_mut();
        z.cursor = None;
        z.warten_auf = None;
        z.laden = Some((Instant::now(), pfad.to_owned()));
    }

    /// The first block of the folder is in the model: the time to the first
    /// picture (Messung).
    pub fn erster_block(&self) {
        let Some((t0, pfad)) = self.z.borrow().laden.clone() else {
            return;
        };
        if !self.liste.is_mapped() {
            return;
        }
        nach_dem_zeichnen(&self.liste, move || {
            eprintln!(
                "{PROGRAMM}: Messung {pfad}: erstes Bild nach {} ms",
                t0.elapsed().as_millis()
            );
        });
    }

    /// The folder is read while this view is hidden: no Messung, no focus.
    pub fn laden_vergessen(&self) {
        self.z.borrow_mut().laden = None;
    }

    /// The folder is read: focus `name` (the folder one came up from) or
    /// the first object, without selecting it.
    pub fn geladen(&self, name: Option<&str>) {
        if let Some((t0, pfad)) = self.z.borrow_mut().laden.take()
            && self.liste.is_mapped()
        {
            let n = self.auswahl.n_items();
            eprintln!(
                "{PROGRAMM}: Messung {pfad}: {n} Objekte im Modell nach {} ms",
                t0.elapsed().as_millis()
            );
            nach_dem_zeichnen(&self.liste, move || {
                eprintln!(
                    "{PROGRAMM}: Messung {pfad}: Ordner vollständig gezeigt nach {} ms",
                    t0.elapsed().as_millis()
                );
            });
        }
        self.liste.grab_focus();
        // the folder one came from (selected), else what the window chose
        // meanwhile (cursor, selection), else the first object
        let erste_gewaehlte = || {
            let a = self.auswahl.selection();
            (a.size() > 0).then(|| (a.nth(0), false))
        };
        let ziel = name
            .and_then(|n| self.position_von(n))
            .map(|p| (p, true))
            .or_else(|| self.cursor_position().map(|p| (p, false)))
            .or_else(erste_gewaehlte);
        match ziel {
            Some((pos, waehlen)) => self.fokus_auf(pos, waehlen),
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
            f((gewaehlt, false));
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
        let neu = std::mem::take(&mut self.z.borrow_mut().neues_fenster);
        if let Some(f) = self.rueckrufe.aktiviert.borrow().clone() {
            f((liste, neu));
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
    /// coordinates: through the window if it has a menu hook, else here.
    fn menue_zeigen(&self, objekte: bool, x: f64, y: f64) {
        if let Some(f) = self.rueckrufe.kontextmenue.borrow().clone() {
            f((objekte, x, y));
            return;
        }
        let (ordner, sortierung) = self
            .rueckrufe
            .umgebung
            .borrow()
            .as_ref()
            .map_or((None, Sortierung::default()), |f| f());
        let gewaehlt = if objekte {
            self.gewaehlte()
        } else {
            Vec::new()
        };
        kontextmenue::zeigen(&self.liste, x, y, &gewaehlt, ordner.as_ref(), sortierung);
    }

    // --- rename ------------------------------------------------------

    /// F2: the one selected object, else the focused one; `ganz` selects
    /// the whole name (Umschalt+F2) instead of the name without extension.
    pub fn umbenennen_starten(self: &Rc<Self>, ganz: bool) {
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
                if let Some(f) = self.rueckrufe.melden.borrow().clone() {
                    f(gettext("Only one item can be renamed at a time."));
                }
                return;
            }
        };
        let Some((pos, e)) = pos.and_then(|p| Some((p, self.eintrag(p)?))) else {
            return;
        };
        let vertrag = Mietvertrag::neu(&e, ganz);
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
        // A GtkTextView reports its height only after its idle validation;
        // in a GridView cell it stayed 0 px high (measured). Start with the
        // height the label had for the same name (one line at least).
        if self.art == Art::Symbole {
            let hoehe = beschriftung
                .height()
                .max(beschriftung.measure(gtk4::Orientation::Vertical, -1).1);
            feld.widget()
                .set_size_request(self.feldbreite.get(), hoehe.max(16) + 4);
        }
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
            protokoll(&format!(
                "Umbenennen: Feld {} × {} px",
                w.width(),
                w.height()
            ));
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
        protokoll(&format!("Umbenennen: {} → {neu}", d.name));
        // The window renames (a job of the queue, with its error dialogs)
        // and selects the renamed object; the cursor follows the name.
        self.z.borrow_mut().cursor = Some(neu.clone());
        self.z.borrow_mut().warten_auf = Some((neu.clone(), false));
        if let Some(f) = self.rueckrufe.umbenennen.borrow().clone() {
            f((eintrag, neu));
        }
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
.gummiband {
  border: 1px solid @theme_selected_bg_color;
  background-color: alpha(@theme_selected_bg_color, 0.25);
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

// --- the views as `Ansicht` of the frame -------------------------------

/// What [`Modul`] needs of a view (icon view, list view).
pub trait Darstellung: 'static {
    fn neu(auswahl: &gtk4::MultiSelection) -> Rc<Self>;
    fn kern(&self) -> &Rc<Kern>;
    fn widget(&self) -> gtk4::Widget;
    fn zoom_setzen(self: &Rc<Self>, prozent: u32);
    fn einstellungen(self: &Rc<Self>, werte: &Werte);
    /// The model's sort order changed (menu, settings, column head).
    fn sortierung_geaendert(&self, _s: Sortierung) {}
}

type RueckrufZelle = Rc<RefCell<Option<Rc<dyn crate::ansicht::Rueckruf>>>>;

/// A view of kind `art` as the frame's trait [`Ansicht`] (B1,
/// `ansicht.rs`): the view is built in `set_modell` on the model's
/// selection; its kern talks to the window through the [`Rueckruf`].
pub struct Modul<D: Darstellung> {
    art: Ansichtsart,
    halter: gtk4::Box,
    innen: RefCell<Option<Rc<D>>>,
    modell: RefCell<Option<Ordnermodell>>,
    rueckruf: RueckrufZelle,
    zoom: Cell<u32>,
    werte: RefCell<Option<Werte>>,
}

impl<D: Darstellung> Modul<D> {
    pub fn neu(art: Ansichtsart) -> Rc<Self> {
        let halter = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        halter.set_hexpand(true);
        halter.set_vexpand(true);
        Rc::new(Self {
            art,
            halter,
            innen: RefCell::new(None),
            modell: RefCell::new(None),
            rueckruf: Rc::new(RefCell::new(None)),
            zoom: Cell::new(match art {
                Ansichtsart::Symbole => crate::ansichtsregeln::ZOOM_SYMBOLE,
                _ => crate::ansichtsregeln::ZOOM_LISTE,
            }),
            werte: RefCell::new(None),
        })
    }

    fn innen(&self) -> Option<Rc<D>> {
        self.innen.borrow().clone()
    }

    fn kern(&self) -> Option<Rc<Kern>> {
        self.innen().map(|d| d.kern().clone())
    }

    /// The window's callbacks of the kern.
    fn verdrahten(&self, d: &Rc<D>, modell: &Ordnermodell) {
        let kern = d.kern();
        let r = self.rueckruf.clone();
        let mit = move |f: &dyn Fn(&Rc<dyn crate::ansicht::Rueckruf>)| {
            if let Some(r) = r.borrow().clone() {
                f(&r);
            }
        };
        let m = mit.clone();
        *kern.rueckrufe.aktiviert.borrow_mut() =
            Some(Rc::new(move |(liste, neu): (Vec<Eintrag>, bool)| {
                let namen: Vec<&str> = liste.iter().map(|e| e.daten().name.as_str()).collect();
                protokoll(&format!(
                    "aktiviert{}: {}",
                    if neu { " (neues Fenster)" } else { "" },
                    namen.join(", ")
                ));
                m(&|r| r.aktivieren(&liste, neu));
            }));
        let m = mit.clone();
        *kern.rueckrufe.nach_oben.borrow_mut() = Some(Rc::new(move |()| m(&|r| r.nach_oben())));
        let m = mit.clone();
        *kern.rueckrufe.ortsfeld.borrow_mut() = Some(Rc::new(move |c: char| {
            protokoll(&format!("Ortsfeld mit „{c}“"));
            m(&|r| r.adresse_tippen(&c.to_string()));
        }));
        let m = mit.clone();
        let weak = Rc::downgrade(d);
        *kern.rueckrufe.zoom.borrow_mut() = Some(Rc::new(move |schritt: i32| {
            let Some(d) = weak.upgrade() else { return };
            let neu = crate::ansichtsregeln::zoom_schritt(d.kern().zoom(), schritt);
            d.zoom_setzen(neu);
            m(&|r| r.zoom_geaendert(Zoom(neu)));
        }));
        let m = mit.clone();
        let liste = kern.liste.downgrade();
        *kern.rueckrufe.kontextmenue.borrow_mut() =
            Some(Rc::new(move |(objekte, x, y): (bool, f64, f64)| {
                let Some(liste) = liste.upgrade() else { return };
                let ziel = if objekte {
                    Kontextziel::Auswahl
                } else {
                    Kontextziel::Hintergrund
                };
                m(&|r| r.kontextmenue(ziel, &liste, x, y));
            }));
        let m = mit.clone();
        *kern.rueckrufe.umbenennen.borrow_mut() =
            Some(Rc::new(move |(e, neu): (Eintrag, String)| {
                m(&|r| r.umbenennen(e.datei(), &neu));
            }));
        let m = mit;
        *kern.rueckrufe.melden.borrow_mut() = Some(Rc::new(move |text: String| {
            protokoll(&format!("Meldung: {text}"));
            m(&|r| r.melden(&text));
        }));
        // the window's actions keep the menu state and dateien.ini in step;
        // without them the model alone
        let fuer_sortieren = modell.clone();
        let liste = kern.liste.downgrade();
        *kern.rueckrufe.sortieren.borrow_mut() = Some(Rc::new(move |s: Sortierung| {
            let m = &fuer_sortieren;
            let liste = liste.upgrade();
            let aktion = |name: &str, ziel: Option<glib::Variant>| {
                liste.as_ref().is_some_and(|l| {
                    kontextmenue::aktion_verfuegbar(l, name)
                        && l.activate_action(name, ziel.as_ref()).is_ok()
                })
            };
            if s.kriterium != m.sortierung().kriterium
                && !aktion("win.anordnen", Some(s.kriterium.kennung().to_variant()))
            {
                m.sortierung_setzen(Sortierung {
                    kriterium: s.kriterium,
                    ..m.sortierung()
                });
            }
            if s.umgekehrt != m.sortierung().umgekehrt && !aktion("win.umgekehrt", None) {
                m.sortierung_setzen(Sortierung {
                    umgekehrt: s.umgekehrt,
                    ..m.sortierung()
                });
            }
        }));
        let fuer_umgebung = modell.clone();
        *kern.rueckrufe.umgebung.borrow_mut() = Some(Rc::new(move || {
            (fuer_umgebung.ordner(), fuer_umgebung.sortierung())
        }));
        let weak = Rc::downgrade(d);
        modell.bei_sortierung(move |s| {
            if let Some(d) = weak.upgrade() {
                d.sortierung_geaendert(s);
            }
        });
        // reading a folder: forget cursor and rename, Messung lines,
        // keyboard focus when it is read
        let weak = Rc::downgrade(kern);
        modell.bei_zustand(move |z| {
            let Some(k) = weak.upgrade() else { return };
            protokoll(&format!("Modell {}: {z:?}", k.art.name()));
            match z {
                Ladezustand::Laedt(ordner) => k.ordner_gewechselt(&ordner.parse_name()),
                Ladezustand::ErsterBlock => k.erster_block(),
                Ladezustand::Geladen(_) | Ladezustand::Gestoppt if k.liste.is_mapped() => {
                    k.geladen(None)
                }
                // a hidden view only forgets the start of the read
                Ladezustand::Geladen(_) | Ladezustand::Gestoppt => k.laden_vergessen(),
                _ => {}
            }
        });
    }
}

impl<D: Darstellung> Ansicht for Modul<D> {
    fn art(&self) -> Ansichtsart {
        self.art
    }

    fn widget(&self) -> gtk4::Widget {
        self.halter.clone().upcast()
    }

    fn set_modell(&self, modell: &Ordnermodell) {
        if self.innen.borrow().is_some() {
            return;
        }
        protokoll(&format!("Ansicht {:?} am Modell", self.art));
        let d = D::neu(modell.auswahl());
        let w = d.widget();
        w.set_hexpand(true);
        w.set_vexpand(true);
        self.halter.append(&w);
        self.verdrahten(&d, modell);
        // a view made later (list after a sort in the icon view) starts
        // with the model's order in its header
        d.sortierung_geaendert(modell.sortierung());
        d.zoom_setzen(self.zoom.get());
        if let Some(werte) = self.werte.borrow().as_ref() {
            d.kern().einfacher_klick(werte.einfacher_klick);
            d.einstellungen(werte);
        }
        *self.modell.borrow_mut() = Some(modell.clone());
        *self.innen.borrow_mut() = Some(d);
    }

    fn modell(&self) -> Option<Ordnermodell> {
        self.modell.borrow().clone()
    }

    fn verbinden(&self, rueckruf: Rc<dyn crate::ansicht::Rueckruf>) {
        *self.rueckruf.borrow_mut() = Some(rueckruf);
    }

    fn zoom(&self) -> Zoom {
        Zoom(self.kern().map_or(self.zoom.get(), |k| k.zoom()))
    }

    fn zoom_setzen(&self, zoom: Zoom) {
        let prozent = crate::ansichtsregeln::zoom_stufe(zoom.0);
        self.zoom.set(prozent);
        if let Some(d) = self.innen() {
            d.zoom_setzen(prozent);
        }
    }

    fn zoomstufen(&self) -> &'static [u32] {
        &crate::ansichtsregeln::STUFEN
    }

    fn fokus(&self) {
        if let Some(k) = self.kern() {
            k.fokus();
        }
    }

    fn fokus_auf(&self, datei: &gio::File, waehlen: bool) {
        let (Some(k), Some(m)) = (self.kern(), self.modell()) else {
            return;
        };
        match m.position_von(datei) {
            Some(pos) => {
                if waehlen {
                    m.waehlen(std::slice::from_ref(datei));
                }
                k.fokus_auf(pos, false);
            }
            None => {
                if let Some(name) = datei.basename() {
                    k.waehlen(&[name.to_string_lossy().into_owned()]);
                }
            }
        }
    }

    fn umbenennen_anstossen(&self, datei: &gio::File, ganzer_name: bool) {
        let (Some(k), Some(m)) = (self.kern(), self.modell()) else {
            return;
        };
        let Some(pos) = m.position_von(datei) else {
            return;
        };
        if !k.auswahl.is_selected(pos) || k.auswahl.selection().size() != 1 {
            m.waehlen(std::slice::from_ref(datei));
        }
        k.fokus_auf(pos, false);
        k.umbenennen_starten(ganzer_name);
    }

    fn umbenennen_laeuft(&self) -> bool {
        self.kern().is_some_and(|k| k.umbenennen_laeuft())
    }

    fn eintrag_bei(&self, x: f64, y: f64) -> Option<Eintrag> {
        let k = self.kern()?;
        let p = self
            .halter
            .compute_point(&k.liste, &gtk4::graphene::Point::new(x as f32, y as f32))?;
        k.eintrag_bei_punkt(f64::from(p.x()), f64::from(p.y()))
    }

    fn einstellungen_anwenden(&self, werte: &Werte) {
        *self.werte.borrow_mut() = Some(werte.clone());
        if let Some(d) = self.innen() {
            d.kern().einfacher_klick(werte.einfacher_klick);
            d.einstellungen(werte);
        }
    }
}

/// The view rules of [`STIL`] for the whole program (once, at startup).
pub fn stil_laden() {
    thread_local! {
        static GELADEN: Cell<bool> = const { Cell::new(false) };
    }
    if GELADEN.with(|g| g.replace(true)) {
        return;
    }
    let css = gtk4::CssProvider::new();
    querencia_oberflaeche::farbschema::folgen(&css, PROGRAMM);
    css.load_from_data(STIL);
    if let Some(display) = gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &css,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}
