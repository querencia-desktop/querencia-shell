//! The compact view (Nautilus 2.30's „Kompakte Ansicht“, the icon
//! container in top-to-bottom layout with labels beside the icons) on a
//! horizontal `gtk4::GridView`: the entries run down a column and on into
//! the next one, the view scrolls sideways (the mouse wheel too), small
//! icon with the name on its right. Keyboard, focus, type-ahead, rename,
//! selection and context menus are the other views' ([`crate::ansichtskern`]).
//!
//! Column widths as `nautilus-icon-container.c` (lay_down_icons_vertical):
//! with „Alle Spalten haben dieselbe Breite“ (default) every column is as
//! wide as the widest name of the folder, without it each column is as
//! wide as its own widest name, nothing is cut. Nautilus 2.30 capped equal
//! columns at 150 px × zoom (13 characters at 50 %) and shrank its font
//! with the zoom; the session's font here keeps its size, so that cap cut
//! ordinary names (QD-173 P-2). The cap is [`ZEICHEN_GRENZE`] characters
//! of the session's font in every zoom, at most the visible width of the
//! view ([`grenze_px`]); longer names are cut with „…“ and show whole in
//! the tooltip. The GridView makes each column as wide as its widest
//! cell, so the cells carry the width (`breite_fuer`); the name label is
//! only as wide as its name, so the highlight stays on the name.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{gdk, glib, pango};

use crate::ansicht::Ansicht;
use crate::ansichtskern::{self, Art, Darstellung, Eintrag, Kern, Modul, protokoll, teile};
use crate::ansichtsregeln::{self, symbolgroesse};
use crate::app::Register;
use crate::konfig::Werte;
use crate::miniaturen::bildquelle;
use crate::regeln::Ansichtsart;

/// The cap of equal columns in characters, in every zoom: Nautilus 2.30's
/// 150 px × zoom at 200 % (42 characters of its font).
pub const ZEICHEN_GRENZE: i32 = 42;
/// However narrow the view, a column keeps room for this many characters
/// (Nautilus' cap at 33 %).
pub const ZEICHEN_MINDESTENS: i32 = 8;
/// Gap between icon and name (Nautilus' hbox spacing in the beside layout).
pub const ABSTAND: i32 = 4;
/// The name label's padding left and right ([`STIL`]: `padding: 0 3px`):
/// GTK counts it into the size request.
const RAND: i32 = 6;
/// The cell's padding left and right ([`STIL`]: `padding: 1px 6px 1px 3px`).
const ZELLRAND: i32 = 9;
/// How many of the longest names are measured for the folder's widest
/// name (the width follows the length closely; measuring 20 000 names on
/// every change would hold up loading, B35).
pub const GEMESSEN: usize = 48;

/// The text limit of equal columns in px: [`ZEICHEN_GRENZE`] characters
/// of `zeichen` px × 1024 (Pango units), within the `sichtbar` px of the
/// view beside an icon of `symbol` px (no cap while the view has no
/// width), never below [`ZEICHEN_MINDESTENS`] characters.
pub fn grenze_px(zeichen: i32, sichtbar: i32, symbol: i32) -> i32 {
    let text = ZEICHEN_GRENZE * zeichen / pango::SCALE;
    let mindestens = ZEICHEN_MINDESTENS * zeichen / pango::SCALE;
    let platz = if sichtbar > 0 {
        sichtbar - symbol - ABSTAND - RAND - ZELLRAND
    } else {
        text
    };
    text.min(platz).max(mindestens).max(1)
}

/// The width a name label gets: with equal columns the folder's widest
/// name (`breiteste`) within `grenze` px, else its own width.
pub fn breite_fuer(eigene: i32, breiteste: i32, gleich: bool, grenze: i32) -> i32 {
    if gleich {
        breiteste.min(grenze).max(1)
    } else {
        eigene.max(1)
    }
}

/// Rows of a column that fit into `hoehe` px with icons of `symbol` px and
/// text lines of `zeile` px (at least one).
pub fn zeilen(hoehe: i32, symbol: i32, zeile: i32) -> u32 {
    (hoehe / (symbol.max(zeile) + 2).max(1)).max(1) as u32
}

/// The `n` longest names (by characters) of `namen`: the candidates for
/// the widest one.
pub fn laengste<'a>(namen: impl Iterator<Item = &'a str>, n: usize) -> Vec<&'a str> {
    if n == 0 {
        return Vec::new();
    }
    let mut alle: Vec<(usize, &str)> = namen.map(|t| (t.chars().count(), t)).collect();
    if alle.len() > n {
        alle.select_nth_unstable_by(n - 1, |a, b| b.0.cmp(&a.0));
        alle.truncate(n);
    }
    alle.into_iter().map(|(_, t)| t).collect()
}

pub struct Kompaktansicht {
    pub kern: Rc<Kern>,
    pub grid: gtk4::GridView,
    scroller: gtk4::ScrolledWindow,
    gleich: Cell<bool>,
    /// Pixel width of the folder's widest name (equal columns).
    breiteste: Cell<i32>,
    /// Measured name widths (font of the view), for the own width.
    breiten: RefCell<HashMap<String, i32>>,
    messen_geplant: Cell<bool>,
    /// The text limit the cells were bound with ([`grenze_px`]).
    grenze: Cell<i32>,
    breite_geplant: Cell<bool>,
}

impl Kompaktansicht {
    pub fn new(auswahl: &gtk4::MultiSelection) -> Rc<Self> {
        let factory = gtk4::SignalListItemFactory::new();
        let grid = gtk4::GridView::new(None::<gtk4::MultiSelection>, Some(factory.clone()));
        grid.set_orientation(gtk4::Orientation::Horizontal);
        grid.set_enable_rubberband(true);
        // in this orientation „columns“ are the rows of a column; GTK keeps
        // max_columns × about 32 cells alive (measured: 8,192 cells with 256,
        // 3 s for 10,000 entries), so `zeilen_anpassen` sets what fits
        grid.set_min_columns(1);
        grid.set_max_columns(16);
        grid.add_css_class("querencia-kompakt");
        let scroller = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Automatic)
            .vscrollbar_policy(gtk4::PolicyType::Never)
            .child(&grid)
            .vexpand(true)
            .hexpand(true)
            .build();
        let kern = Kern::new(
            Art::Kompakt,
            auswahl.clone(),
            grid.clone().upcast(),
            &scroller,
        );
        kern.zoom_merken(ansichtsregeln::ZOOM_LISTE);
        let ansicht = Rc::new(Self {
            kern,
            grid,
            scroller,
            gleich: Cell::new(true),
            breiteste: Cell::new(1),
            breiten: RefCell::default(),
            messen_geplant: Cell::new(false),
            grenze: Cell::new(0),
            breite_geplant: Cell::new(false),
        });
        ansicht.fabrik(&factory);
        ansicht.grid.set_model(Some(auswahl));
        ansicht.kern.anschliessen(&ansicht.scroller);
        let weak = Rc::downgrade(&ansicht);
        ansicht.grid.connect_activate(move |_, pos| {
            if let Some(a) = weak.upgrade() {
                a.kern.aktiviert(pos);
            }
        });
        // a new folder or new names: measure the widest name again
        let weak = Rc::downgrade(&ansicht);
        auswahl.connect_items_changed(move |_, _, _, _| {
            if let Some(a) = weak.upgrade() {
                a.messen_planen();
            }
        });
        ansicht.rad_waagrecht();
        let weak = Rc::downgrade(&ansicht);
        ansicht
            .scroller
            .vadjustment()
            .connect_page_size_notify(move |_| {
                if let Some(a) = weak.upgrade() {
                    a.zeilen_anpassen();
                }
            });
        // a wider or narrower view moves the limit of equal columns
        let weak = Rc::downgrade(&ansicht);
        ansicht
            .scroller
            .hadjustment()
            .connect_page_size_notify(move |_| {
                if let Some(a) = weak.upgrade() {
                    a.breite_planen();
                }
            });
        // the model may be read already (the view is made on first use)
        ansicht.messen_planen();
        ansicht
    }

    pub fn widget(&self) -> gtk4::Widget {
        self.kern.wurzel.clone().upcast()
    }

    /// The vertical wheel scrolls sideways (Nautilus 2.30, fm-icon-view.c
    /// scroll_event); Strg+wheel stays the kern's zoom.
    fn rad_waagrecht(self: &Rc<Self>) {
        let rad = gtk4::EventControllerScroll::new(gtk4::EventControllerScrollFlags::VERTICAL);
        let weak = Rc::downgrade(self);
        rad.connect_scroll(move |c, _, dy| {
            let Some(a) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            if c.current_event_state()
                .contains(gdk::ModifierType::CONTROL_MASK)
                || dy == 0.0
            {
                return glib::Propagation::Proceed;
            }
            let adj = a.scroller.hadjustment();
            let schritt = match c.unit() {
                gdk::ScrollUnit::Wheel => (adj.page_size() * adj.page_size()).powf(1.0 / 3.0),
                _ => 1.0,
            };
            let ziel =
                (adj.value() + dy * schritt).clamp(adj.lower(), adj.upper() - adj.page_size());
            adj.set_value(ziel);
            protokoll(&format!("Kompaktansicht waagrecht: {:.0}", adj.value()));
            glib::Propagation::Stop
        });
        self.grid.add_controller(rad);
    }

    fn symbolgroesse(&self) -> i32 {
        symbolgroesse(self.kern.zoom())
    }

    /// As many rows as cells of the current zoom fit into the height of the
    /// view (cell: the icon or one line of text, whichever is higher, and
    /// the child's padding of 1 px above and below, `STIL`).
    fn zeilen_anpassen(&self) {
        let hoehe = self.kern.wurzel.height();
        if hoehe <= 0 {
            return;
        }
        let m = self.grid.pango_context().metrics(None, None);
        let zeile = (m.ascent() + m.descent()) / pango::SCALE;
        let zeilen = zeilen(hoehe, self.symbolgroesse(), zeile);
        if zeilen != self.grid.max_columns() {
            protokoll(&format!("Kompaktansicht: {zeilen} Zeilen (Höhe {hoehe})"));
            self.grid.set_max_columns(zeilen);
        }
    }

    /// The text limit of equal columns in px ([`grenze_px`]): the view's
    /// font and visible width.
    fn grenze(&self) -> i32 {
        let zeichen = self
            .grid
            .pango_context()
            .metrics(None, None)
            .approximate_char_width();
        let sichtbar = self.scroller.hadjustment().page_size() as i32;
        grenze_px(zeichen, sichtbar, self.symbolgroesse())
    }

    /// After the view's width changed (idle, once, not inside the
    /// allocation): bind the cells again when the limit moved.
    fn breite_planen(self: &Rc<Self>) {
        if !self.gleich.get() || self.breite_geplant.replace(true) {
            return;
        }
        let weak = Rc::downgrade(self);
        glib::idle_add_local_once(move || {
            if let Some(a) = weak.upgrade() {
                a.breite_geplant.set(false);
                let grenze = a.grenze();
                if grenze != a.grenze.get() {
                    a.neu_binden();
                }
            }
        });
    }

    /// Pixel width of `text` in the view's font.
    fn messen(&self, text: &str) -> i32 {
        if let Some(b) = self.breiten.borrow().get(text) {
            return *b;
        }
        let layout = self.grid.create_pango_layout(Some(text));
        let b = layout.pixel_size().0 + 1;
        self.breiten.borrow_mut().insert(text.to_owned(), b);
        b
    }

    /// After the model changed (idle, once): the widest name of the folder.
    fn messen_planen(self: &Rc<Self>) {
        if !self.gleich.get() || self.messen_geplant.replace(true) {
            return;
        }
        let weak = Rc::downgrade(self);
        glib::idle_add_local_once(move || {
            if let Some(a) = weak.upgrade() {
                a.messen_geplant.set(false);
                a.breiteste_messen();
            }
        });
    }

    fn breiteste_messen(&self) {
        let auswahl = &self.kern.auswahl;
        let eintraege: Vec<Eintrag> = (0..auswahl.n_items())
            .filter_map(|i| auswahl.item(i).and_downcast::<Eintrag>())
            .collect();
        let namen = eintraege.iter().map(|e| e.daten().anzeige.as_str());
        let breiteste = laengste(namen, GEMESSEN)
            .into_iter()
            .map(|t| self.messen(t))
            .max()
            .unwrap_or(1);
        if breiteste != self.breiteste.get() {
            protokoll(&format!(
                "Kompaktansicht: breitester Name {breiteste} px ({} Einträge)",
                eintraege.len()
            ));
            self.breiteste.set(breiteste);
            self.neu_binden();
        }
    }

    fn fabrik(self: &Rc<Self>, factory: &gtk4::SignalListItemFactory) {
        let weak = Rc::downgrade(self);
        factory.connect_setup(move |_, obj| {
            let (Some(a), Some(li)) = (weak.upgrade(), obj.downcast_ref::<gtk4::ListItem>()) else {
                return;
            };
            let zelle = gtk4::Box::new(gtk4::Orientation::Horizontal, ABSTAND);
            zelle.add_css_class("zelle");
            zelle.set_halign(gtk4::Align::Start);
            zelle.set_valign(gtk4::Align::Center);
            let bild = gtk4::Image::new();
            bild.set_pixel_size(a.symbolgroesse());
            let text = gtk4::Label::new(None);
            text.add_css_class("beschriftung");
            text.set_xalign(0.0);
            text.set_single_line_mode(true);
            // the widths come from set_size_request (`binden`); one
            // character keeps the label's own natural width below it
            text.set_ellipsize(pango::EllipsizeMode::End);
            text.set_max_width_chars(1);
            zelle.append(&bild);
            zelle.append(&text);
            li.set_child(Some(&zelle));
            a.kern.zelle_anmelden(&zelle, li);
        });
        let weak = Rc::downgrade(self);
        factory.connect_bind(move |_, obj| {
            let (Some(a), Some(li)) = (weak.upgrade(), obj.downcast_ref::<gtk4::ListItem>()) else {
                return;
            };
            let (Some(zelle), Some(eintrag)) = (
                li.child().and_downcast::<gtk4::Box>(),
                li.item().and_downcast::<Eintrag>(),
            ) else {
                return;
            };
            a.binden(&zelle, &eintrag);
        });
        let weak = Rc::downgrade(self);
        factory.connect_unbind(move |_, obj| {
            let (Some(a), Some(li)) = (weak.upgrade(), obj.downcast_ref::<gtk4::ListItem>()) else {
                return;
            };
            if let Some(zelle) = li.child().and_downcast::<gtk4::Box>() {
                a.kern.geloest(&zelle);
                bildquelle::loesen(&teile(&zelle).0);
            }
        });
    }

    fn binden(&self, zelle: &gtk4::Box, eintrag: &Eintrag) {
        let (bild, text) = teile(zelle);
        // A cell never shows a rename field of another object.
        self.kern.gebunden(zelle, eintrag);
        let groesse = self.symbolgroesse();
        if bild.pixel_size() != groesse {
            bild.set_pixel_size(groesse);
        }
        let d = eintrag.daten();
        // QD-106: theme icons only through querencia-oberflaeche; the
        // thumbnail through the Bildquelle je Eintrag (Welle C2)
        bildquelle::binden(&bild, eintrag, bildquelle::Groesse::liste(groesse));
        text.set_text(&d.anzeige);
        let eigene = self.messen(&d.anzeige);
        let grenze = self.grenze();
        self.grenze.set(grenze);
        let breite = breite_fuer(eigene, self.breiteste.get(), self.gleich.get(), grenze);
        // the cell carries the column's width, the label only its name's:
        // the highlight sits on the name as in Nautilus, not on the column
        text.set_size_request(eigene.min(breite) + RAND, -1);
        zelle.set_size_request(groesse + ABSTAND + breite + RAND, -1);
        // a cut name shows whole in the tooltip
        zelle.set_tooltip_text((eigene > breite).then_some(d.anzeige.as_str()));
    }

    fn neu_binden(&self) {
        let grenze = self.grenze();
        protokoll(&format!(
            "Kompaktansicht: Spaltenbreite {} px (breitester Name {} px, Grenze {grenze} px, Zoom {} %, {})",
            breite_fuer(1, self.breiteste.get(), true, grenze),
            self.breiteste.get(),
            self.kern.zoom(),
            if self.gleich.get() {
                "gleich breit"
            } else {
                "je nach Inhalt"
            }
        ));
        for (zelle, li) in self.kern.lebende_zellen() {
            if let Some(e) = li.item().and_downcast::<Eintrag>() {
                self.binden(&zelle, &e);
            }
        }
        self.grid.queue_resize();
    }

    pub fn zoom_setzen(self: &Rc<Self>, prozent: u32) {
        let prozent = ansichtsregeln::zoom_stufe(prozent);
        if prozent == self.kern.zoom() {
            return;
        }
        self.kern.zoom_merken(prozent);
        self.zeilen_anpassen();
        self.neu_binden();
        protokoll(&format!(
            "Zoom Kompaktansicht {prozent} % ({} px)",
            self.symbolgroesse()
        ));
        let kern = self.kern.clone();
        glib::idle_add_local_once(move || {
            if let Some(pos) = kern.cursor_position() {
                if kern.liste.focus_child().is_some() {
                    kern.fokus_auf(pos, false);
                } else {
                    kern.scrollen_zu(pos);
                }
            }
        });
    }

    /// „Alle Spalten haben dieselbe Breite“ (Reiter „Anzeige“).
    pub fn gleich_breit_setzen(self: &Rc<Self>, gleich: bool) {
        if gleich == self.gleich.get() {
            return;
        }
        self.gleich.set(gleich);
        protokoll(&format!(
            "Kompaktansicht: Spalten {}",
            if gleich {
                "gleich breit"
            } else {
                "je nach Inhalt"
            }
        ));
        if gleich {
            self.breiteste_messen();
        }
        self.neu_binden();
    }
}

impl Darstellung for Kompaktansicht {
    fn neu(auswahl: &gtk4::MultiSelection) -> Rc<Self> {
        Self::new(auswahl)
    }

    fn kern(&self) -> &Rc<Kern> {
        &self.kern
    }

    fn widget(&self) -> gtk4::Widget {
        Kompaktansicht::widget(self)
    }

    fn zoom_setzen(self: &Rc<Self>, prozent: u32) {
        Kompaktansicht::zoom_setzen(self, prozent);
    }

    fn einstellungen(self: &Rc<Self>, werte: &Werte) {
        self.gleich_breit_setzen(werte.kompakt_gleich_breit);
    }
}

/// The styles of the compact view: the name is highlighted as in the icon
/// view (Nautilus' canvas), the cell itself stays plain.
const STIL: &str = "
gridview.querencia-kompakt > child,
gridview.querencia-kompakt > child:selected,
gridview.querencia-kompakt > child:hover {
  background: none;
  color: inherit;
  padding: 1px 6px 1px 3px;
  outline-color: transparent;
}
gridview.querencia-kompakt > child box.zelle {
  margin: 0;
  border-spacing: 4px;
}
gridview.querencia-kompakt > child:focus:focus-visible .beschriftung {
  outline: 1px dashed alpha(@theme_text_color, 0.6);
  outline-offset: -1px;
}
gridview.querencia-kompakt .beschriftung {
  padding: 0 3px;
  border-radius: 3px;
  color: @theme_text_color;
}
gridview.querencia-kompakt > child:selected .beschriftung {
  background-color: @theme_selected_bg_color;
  color: @theme_selected_fg_color;
}
gridview.querencia-kompakt:not(:focus-within) > child:selected .beschriftung {
  background-color: @theme_unfocused_selected_bg_color;
  color: @theme_unfocused_selected_fg_color;
}
gridview.querencia-kompakt > child:selected image {
  -gtk-icon-filter: brightness(0.78);
}
gridview.querencia-kompakt > child:hover image {
  -gtk-icon-filter: brightness(1.12);
}
gridview.querencia-kompakt text.umbenennen {
  border: 1px solid @theme_text_color;
  padding: 0 2px;
  background-color: @theme_base_color;
  color: @theme_text_color;
}
";

fn stil_laden() {
    let css = gtk4::CssProvider::new();
    querencia_oberflaeche::farbschema::folgen(&css, ansichtskern::PROGRAMM);
    css.load_from_data(STIL);
    if let Some(display) = gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &css,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

/// Registers the compact view (kind „Kompakt“, Strg+3) with the frame
/// (`main.rs`, `erweiterungen`).
pub fn registrieren(register: &mut Register) {
    register.ansicht(Ansichtsart::Kompakt, || {
        Modul::<Kompaktansicht>::neu(Ansichtsart::Kompakt) as Rc<dyn Ansicht>
    });
    register.anwendung(|_| stil_laden());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_limit_does_not_shrink_with_the_zoom() {
        // QD-173 P-2: 7 px per character (Pango units), no width yet:
        // 42 characters whatever the icon size
        let z = 7 * pango::SCALE;
        for symbol in [16, 24, 48, 96] {
            assert_eq!(grenze_px(z, 0, symbol), 294);
        }
        // a wide view does not widen it
        assert_eq!(grenze_px(z, 1500, 96), 294);
        // a narrow view: its width beside icon, gap, padding
        assert_eq!(grenze_px(z, 300, 24), 300 - 24 - ABSTAND - RAND - ZELLRAND);
        assert_eq!(grenze_px(z, 300, 96), 300 - 96 - ABSTAND - RAND - ZELLRAND);
        // never below 8 characters
        assert_eq!(grenze_px(z, 60, 96), 56);
        assert_eq!(grenze_px(0, 0, 24), 1);
        // the names of the finding (L1-05) fit in every zoom
        let querencia_tiefblau = 22 * 7;
        assert!(querencia_tiefblau <= grenze_px(z, 650, 24));
        assert!(querencia_tiefblau <= grenze_px(z, 650, 96));
    }

    #[test]
    fn width_of_a_name_label() {
        // equal columns: the widest name, at most the limit
        assert_eq!(breite_fuer(40, 120, true, 150), 120);
        assert_eq!(breite_fuer(40, 400, true, 150), 150);
        assert_eq!(breite_fuer(40, 400, true, 75), 75);
        // own width: nothing is cut, however long
        assert_eq!(breite_fuer(40, 400, false, 150), 40);
        assert_eq!(breite_fuer(900, 400, false, 75), 900);
        assert_eq!(breite_fuer(0, 0, true, 150), 1);
    }

    #[test]
    fn rows_that_fit_the_height() {
        // 50 %: 24 px icons, a 19 px line: 26 px per row (measured in K02)
        assert_eq!(zeilen(426, 24, 19), 16);
        assert_eq!(zeilen(426, 96, 19), 4);
        assert_eq!(zeilen(426, 16, 19), 20);
        assert_eq!(zeilen(10, 48, 19), 1);
    }

    #[test]
    fn the_longest_names_are_the_candidates() {
        let namen = ["a", "bbbb", "cc", "dddddd", "ä€ß", "eeeee"];
        let mut l = laengste(namen.iter().copied(), 2);
        l.sort();
        assert_eq!(l, vec!["dddddd", "eeeee"]);
        // characters, not bytes
        let l = laengste(["ä€ßü", "abc"].into_iter(), 1);
        assert_eq!(l, vec!["ä€ßü"]);
        assert_eq!(laengste(namen.iter().copied(), 10).len(), 6);
        assert!(laengste(std::iter::empty(), 3).is_empty());
    }
}
