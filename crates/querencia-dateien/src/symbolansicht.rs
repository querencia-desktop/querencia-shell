//! The icon view (Nautilus 2.30's icon container) on `gtk4::GridView`,
//! ported from the prototype (`dateien/prototyp-2`, ansicht.rs): zoom
//! levels 50–200 % (24–96 px), names under the icon on up to three lines
//! (the whole name of the one selected object), extra lines from 100 % and
//! 150 % after the setting, `max_columns` from the width. Keyboard, focus,
//! type-ahead, rename and context menus come from [`crate::ansichtskern`].

use std::cell::Cell;
use std::rc::Rc;
use std::time::Instant;

use gtk4::prelude::*;
use gtk4::{glib, pango};

use querencia_oberflaeche::symbole;

use crate::ansichtskern::{Art, Eintrag, Kern, protokoll, teile};
use crate::ansichtsregeln::{
    self, ZEILEN, ZUSATZ_VORGABE, Zusatz, datumsmuster, symbolgroesse, zellbreite, zusatzzeilen,
};

pub struct Symbolansicht {
    pub kern: Rc<Kern>,
    pub grid: gtk4::GridView,
    zusatz: Cell<[Zusatz; 2]>,
    iso: Cell<bool>,
}

/// The label's width: its text on one line, at most the cell. A centred
/// wrapping label would otherwise get GTK's narrowest width for its height
/// and break „Rechnungen 2026“ into three short lines (prototype §5 A8).
fn beschriftung_breite(text: &gtk4::Label, groesse: i32) {
    text.set_size_request(-1, -1);
    let (_, eine_zeile, _, _) = text.measure(gtk4::Orientation::Horizontal, -1);
    text.set_size_request(eine_zeile.min(zellbreite(groesse)), -1);
}

fn zusatz_label(zelle: &gtk4::Box) -> Option<gtk4::Label> {
    zelle
        .last_child()
        .and_downcast::<gtk4::Label>()
        .filter(|l| l.has_css_class("zusatz"))
}

impl Symbolansicht {
    pub fn new(auswahl: &gtk4::MultiSelection) -> Rc<Self> {
        let factory = gtk4::SignalListItemFactory::new();
        let grid = gtk4::GridView::new(None::<gtk4::MultiSelection>, Some(factory.clone()));
        grid.set_enable_rubberband(true);
        // GTK keeps max_columns × about 30 rows of cells alive; spalten()
        // keeps it at what fits (prototype report §5 A1).
        grid.set_max_columns(7);
        grid.add_css_class("querencia-symbole");
        let scroller = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .child(&grid)
            .vexpand(true)
            .hexpand(true)
            .build();
        let kern = Kern::new(
            Art::Symbole,
            auswahl.clone(),
            grid.clone().upcast(),
            &scroller,
        );
        kern.zoom_merken(ansichtsregeln::ZOOM_SYMBOLE);
        kern.feldbreite_setzen(zellbreite(symbolgroesse(ansichtsregeln::ZOOM_SYMBOLE)));
        let ansicht = Rc::new(Self {
            kern,
            grid,
            zusatz: Cell::new(ZUSATZ_VORGABE),
            iso: Cell::new(false),
        });
        ansicht.fabrik(&factory);
        ansicht.grid.set_model(Some(auswahl));
        ansicht.kern.anschliessen(&scroller);
        let weak = Rc::downgrade(&ansicht);
        ansicht.grid.connect_activate(move |_, pos| {
            if let Some(a) = weak.upgrade() {
                a.kern.aktiviert(pos);
            }
        });
        let weak = Rc::downgrade(&ansicht);
        auswahl.connect_selection_changed(move |_, _, _| {
            if let Some(a) = weak.upgrade() {
                for (zelle, li) in a.kern.lebende_zellen() {
                    a.beschriftung_einstellen(&teile(&zelle).1, li.is_selected());
                }
            }
        });
        let weak = Rc::downgrade(&ansicht);
        scroller.hadjustment().connect_page_size_notify(move |_| {
            if let Some(a) = weak.upgrade() {
                a.spalten_anpassen();
            }
        });
        ansicht
    }

    pub fn widget(&self) -> gtk4::Widget {
        self.kern.wurzel.clone().upcast()
    }

    fn symbolgroesse(&self) -> i32 {
        symbolgroesse(self.kern.zoom())
    }

    /// As many columns as cells of the current zoom fit into the width of
    /// the view (not of the grid: that changes with the scroll bar and made
    /// the count swing between two values).
    fn spalten_anpassen(&self) {
        let breite = self.kern.wurzel.width();
        if breite <= 0 {
            return;
        }
        let spalten = ansichtsregeln::spalten(breite, self.symbolgroesse());
        if spalten != self.grid.max_columns() {
            protokoll(&format!("Spalten {spalten} (Breite {breite})"));
            self.grid.set_max_columns(spalten);
        }
    }

    fn fabrik(self: &Rc<Self>, factory: &gtk4::SignalListItemFactory) {
        let weak = Rc::downgrade(self);
        factory.connect_setup(move |_, obj| {
            let (Some(a), Some(li)) = (weak.upgrade(), obj.downcast_ref::<gtk4::ListItem>()) else {
                return;
            };
            let groesse = a.symbolgroesse();
            let zelle = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
            zelle.add_css_class("zelle");
            zelle.set_halign(gtk4::Align::Center);
            zelle.set_valign(gtk4::Align::Start);
            zelle.set_size_request(zellbreite(groesse), -1);
            let bild = gtk4::Image::new();
            bild.set_pixel_size(groesse);
            let text = gtk4::Label::new(None);
            text.add_css_class("beschriftung");
            text.set_wrap(true);
            text.set_wrap_mode(pango::WrapMode::WordChar);
            text.set_justify(gtk4::Justification::Center);
            text.set_halign(gtk4::Align::Center);
            text.set_lines(ZEILEN);
            text.set_ellipsize(pango::EllipsizeMode::End);
            // Pango ≥ 1.44 adds hyphens where it breaks inside a word;
            // Nautilus 2.30 (Pango 1.28) did not.
            let attribute = pango::AttrList::new();
            attribute.insert(pango::AttrInt::new_insert_hyphens(false));
            text.set_attributes(Some(&attribute));
            let zusatz = gtk4::Label::new(None);
            zusatz.add_css_class("zusatz");
            zusatz.add_css_class("dim-label");
            zusatz.set_justify(gtk4::Justification::Center);
            zusatz.set_ellipsize(pango::EllipsizeMode::End);
            zusatz.set_max_width_chars(1);
            zusatz.set_size_request(zellbreite(groesse) - 8, -1);
            zusatz.set_visible(false);
            zelle.append(&bild);
            zelle.append(&text);
            zelle.append(&zusatz);
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
            a.binden(&zelle, &eintrag, li.is_selected());
        });
        let weak = Rc::downgrade(self);
        factory.connect_unbind(move |_, obj| {
            let (Some(a), Some(li)) = (weak.upgrade(), obj.downcast_ref::<gtk4::ListItem>()) else {
                return;
            };
            if let Some(zelle) = li.child().and_downcast::<gtk4::Box>() {
                a.kern.geloest(&zelle);
            }
        });
    }

    fn binden(self: &Rc<Self>, zelle: &gtk4::Box, eintrag: &Eintrag, gewaehlt: bool) {
        let (bild, text) = teile(zelle);
        // A cell never shows a rename field of another object.
        self.kern.gebunden(zelle, eintrag);
        let groesse = self.symbolgroesse();
        if bild.pixel_size() != groesse {
            bild.set_pixel_size(groesse);
            zelle.set_size_request(zellbreite(groesse), -1);
        }
        let d = eintrag.daten();
        // QD-106: theme icons only through querencia-oberflaeche
        symbole::set(&bild, &d.symbol);
        text.set_text(&d.anzeige);
        beschriftung_breite(&text, groesse);
        self.beschriftung_einstellen(&text, gewaehlt);
        if let Some(z) = zusatz_label(zelle) {
            let zeilen = self.zusatz_text(eintrag);
            z.set_visible(!zeilen.is_empty());
            z.set_text(&zeilen);
            z.set_size_request(zellbreite(groesse) - 8, -1);
        }
    }

    /// The extra lines of `eintrag` at the current zoom.
    fn zusatz_text(&self, eintrag: &Eintrag) -> String {
        let d = eintrag.daten();
        zusatzzeilen(self.kern.zoom(), self.zusatz.get())
            .into_iter()
            .filter_map(|z| match z {
                Zusatz::Nichts => None,
                Zusatz::Groesse => {
                    (!d.ist_ordner).then(|| glib::format_size(d.groesse).to_string())
                }
                Zusatz::Datum => glib::DateTime::from_unix_local(d.zeit)
                    .ok()
                    .and_then(|t| t.format(datumsmuster(self.iso.get())).ok())
                    .map(|t| t.to_string()),
                Zusatz::Typ => Some(d.typ_text.clone()),
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Whole name for the one selected object, three lines otherwise.
    fn beschriftung_einstellen(&self, text: &gtk4::Label, gewaehlt: bool) {
        let ganz = gewaehlt && self.kern.auswahl.selection().size() == 1;
        if ganz {
            text.set_lines(-1);
            text.set_ellipsize(pango::EllipsizeMode::None);
        } else {
            text.set_lines(ZEILEN);
            text.set_ellipsize(pango::EllipsizeMode::End);
        }
    }

    pub fn zoom_setzen(self: &Rc<Self>, prozent: u32) {
        let prozent = ansichtsregeln::zoom_stufe(prozent);
        if prozent == self.kern.zoom() {
            return;
        }
        let t0 = Instant::now();
        self.kern.zoom_merken(prozent);
        let groesse = symbolgroesse(prozent);
        self.kern.feldbreite_setzen(zellbreite(groesse));
        for (zelle, li) in self.kern.lebende_zellen() {
            if let Some(e) = li.item().and_downcast::<Eintrag>() {
                self.binden(&zelle, &e, li.is_selected());
            } else {
                teile(&zelle).0.set_pixel_size(groesse);
                zelle.set_size_request(zellbreite(groesse), -1);
            }
        }
        self.spalten_anpassen();
        self.grid.queue_resize();
        protokoll(&format!(
            "Zoom Symbolansicht {prozent} % ({groesse} px) in {} ms",
            t0.elapsed().as_millis()
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

    /// The extra lines setting (Reiter „Anzeige“): from 100 % and from
    /// 150 %.
    pub fn zusatz_setzen(self: &Rc<Self>, zusatz: [Zusatz; 2]) {
        if zusatz == self.zusatz.get() {
            return;
        }
        self.zusatz.set(zusatz);
        self.neu_binden();
    }

    pub fn datumsformat_setzen(self: &Rc<Self>, iso: bool) {
        if iso != self.iso.get() {
            self.iso.set(iso);
            self.neu_binden();
        }
    }

    fn neu_binden(self: &Rc<Self>) {
        for (zelle, li) in self.kern.lebende_zellen() {
            if let Some(e) = li.item().and_downcast::<Eintrag>() {
                self.binden(&zelle, &e, li.is_selected());
            }
        }
    }
}
