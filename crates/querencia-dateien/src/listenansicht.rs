//! The list view (Nautilus 2.30's list view) on `gtk4::ColumnView`:
//! columns Name, Größe, Typ, Änderungsdatum, by setting also
//! Zugriffsrechte, Eigentümer and Gruppe; a click on a column header sorts
//! by it (again: the other way round) with the arrow in the header; zoom
//! sets the icon size and so the row height. Keyboard, focus, type-ahead,
//! rename and context menus are the icon view's ([`crate::ansichtskern`]).
//!
//! The ColumnView does not sort itself: its sorter only reports the
//! clicks, the order stays the folder model's (one order for both views).

use std::cell::Cell;
use std::rc::Rc;
use std::time::Instant;

use gtk4::prelude::*;
use gtk4::{glib, pango};

use querencia_oberflaeche::symbole;

use crate::ansicht::Ansicht;
use crate::ansichtskern::{
    self, Art, Darstellung, Eintrag, Kern, Modul, Sortierung, protokoll, teile,
};
use crate::ansichtsregeln::{
    self, Spalte, datumsmuster, groesse_text, rechte_text, sichtbare_spalten, sortierung_nach_klick,
};
use crate::app::Register;
use crate::konfig::Werte;
use crate::regeln;
use crate::regeln::Ansichtsart;

pub struct Listenansicht {
    pub kern: Rc<Kern>,
    pub ansicht: gtk4::ColumnView,
    spalten: Vec<(Spalte, gtk4::ColumnViewColumn)>,
    iso: Cell<bool>,
    /// Set while the view itself moves the header arrow.
    zeigt: Cell<bool>,
    sortierung: Cell<Sortierung>,
}

fn text_der_spalte(spalte: Spalte, e: &Eintrag, iso: bool) -> String {
    let d = e.daten();
    match spalte {
        Spalte::Name => d.anzeige.clone(),
        Spalte::Groesse => groesse_text(d.ist_ordner, ansichtskern::groesse(e), regeln::groesse),
        Spalte::Typ => ansichtskern::typ_text(e),
        Spalte::Datum => glib::DateTime::from_unix_local(ansichtskern::zeit(e))
            .ok()
            .and_then(|t| t.format(datumsmuster(iso)).ok())
            .map(|t| t.to_string())
            .unwrap_or_default(),
        Spalte::Rechte => ansichtskern::modus(e).map(rechte_text).unwrap_or_default(),
        Spalte::Eigentuemer => ansichtskern::eigentuemer(e),
        Spalte::Gruppe => ansichtskern::gruppe(e),
    }
}

impl Listenansicht {
    pub fn new(auswahl: &gtk4::MultiSelection) -> Rc<Self> {
        let ansicht = gtk4::ColumnView::new(None::<gtk4::MultiSelection>);
        ansicht.set_enable_rubberband(true);
        ansicht.set_reorderable(false);
        ansicht.set_show_row_separators(false);
        ansicht.set_show_column_separators(false);
        ansicht.add_css_class("querencia-liste");
        let scroller = gtk4::ScrolledWindow::builder()
            .child(&ansicht)
            .vexpand(true)
            .hexpand(true)
            .build();
        let kern = Kern::new(
            Art::Liste,
            auswahl.clone(),
            ansicht.clone().upcast(),
            &scroller,
        );
        kern.zoom_merken(ansichtsregeln::ZOOM_LISTE);
        let mut spalten = Vec::new();
        for spalte in Spalte::ALLE {
            let factory = gtk4::SignalListItemFactory::new();
            let col = gtk4::ColumnViewColumn::new(Some(&spalte.titel()), Some(factory.clone()));
            col.set_resizable(true);
            if spalte == Spalte::Name {
                col.set_expand(true);
            }
            if spalte.kriterium().is_some() {
                // only for the clickable header with its arrow; never asked
                // to compare (the model sorts)
                col.set_sorter(Some(&gtk4::CustomSorter::new(|_, _| gtk4::Ordering::Equal)));
            }
            col.set_visible(Spalte::VORGABE.contains(&spalte));
            ansicht.append_column(&col);
            spalten.push((spalte, col, factory));
        }
        let liste = Rc::new(Self {
            kern,
            ansicht,
            spalten: spalten.iter().map(|(s, c, _)| (*s, c.clone())).collect(),
            iso: Cell::new(false),
            zeigt: Cell::new(false),
            sortierung: Cell::new(Sortierung::default()),
        });
        for (spalte, _, factory) in &spalten {
            liste.fabrik(*spalte, factory);
        }
        liste.ansicht.set_model(Some(auswahl));
        liste.kern.anschliessen(&scroller);
        // a new order from the menu (Ansicht → Objekte anordnen): the
        // header arrow follows the model's order
        let weak = Rc::downgrade(&liste);
        auswahl.connect_items_changed(move |_, _, weg, neu| {
            if weg > 1
                && weg == neu
                && let Some(l) = weak.upgrade()
            {
                l.sortierung_abgleichen();
            }
        });
        let weak = Rc::downgrade(&liste);
        liste.ansicht.connect_activate(move |_, pos| {
            if let Some(l) = weak.upgrade() {
                l.kern.aktiviert(pos);
            }
        });
        if let Some(sorter) = liste.ansicht.sorter() {
            let weak = Rc::downgrade(&liste);
            sorter.connect_changed(move |_, _| {
                if let Some(l) = weak.upgrade() {
                    l.kopf_geklickt();
                }
            });
        }
        liste.sortierung_zeigen(Sortierung::default());
        // where the column headers are (for the isolated checks)
        let weak = Rc::downgrade(&liste);
        liste.ansicht.connect_map(move |_| {
            let weak = weak.clone();
            glib::idle_add_local_once(move || {
                if let Some(l) = weak.upgrade() {
                    l.kopf_protokoll();
                }
            });
        });
        liste
    }

    fn kopf_protokoll(&self) {
        let (Some(kopf), Some(fenster)) = (self.ansicht.first_child(), self.ansicht.root()) else {
            return;
        };
        let sichtbar: Vec<Spalte> = self
            .spalten
            .iter()
            .filter(|(_, c)| c.is_visible())
            .map(|(s, _)| *s)
            .collect();
        let mut kind = kopf.first_child();
        let mut i = 0;
        while let Some(k) = kind {
            if let (Some(s), Some(r)) = (sichtbar.get(i), k.compute_bounds(&fenster)) {
                protokoll(&format!(
                    "Spaltenkopf {} bei {:.0} {:.0} {:.0} {:.0}",
                    s.kennung(),
                    r.x(),
                    r.y(),
                    r.width(),
                    r.height()
                ));
            }
            i += 1;
            kind = k.next_sibling();
        }
    }

    pub fn widget(&self) -> gtk4::Widget {
        self.kern.wurzel.clone().upcast()
    }

    fn kopf_geklickt(self: &Rc<Self>) {
        if self.zeigt.get() {
            return;
        }
        let Some(sorter) = self
            .ansicht
            .sorter()
            .and_downcast::<gtk4::ColumnViewSorter>()
        else {
            return;
        };
        let Some(col) = sorter.primary_sort_column() else {
            // GTK took the arrow away (third click is not a thing in
            // Nautilus): show the current order again.
            self.sortierung_zeigen(self.sortierung.get());
            return;
        };
        let Some(spalte) = self
            .spalten
            .iter()
            .find(|(_, c)| *c == col)
            .map(|(s, _)| *s)
        else {
            return;
        };
        let absteigend = sorter.primary_sort_order() == gtk4::SortType::Descending;
        let Some(neu) = sortierung_nach_klick(self.sortierung.get(), spalte, absteigend) else {
            return;
        };
        protokoll(&format!(
            "Spaltenkopf {}: Sortierung {} umgekehrt {}",
            spalte.kennung(),
            neu.kriterium.kennung(),
            neu.umgekehrt
        ));
        self.sortierung.set(neu);
        if let Some(f) = self.kern.rueckrufe.sortieren.borrow().clone() {
            f(neu);
        }
        self.kern.nach_umordnen();
    }

    fn sortierung_abgleichen(&self) {
        let s = self
            .kern
            .rueckrufe
            .umgebung
            .borrow()
            .as_ref()
            .map(|f| f().1);
        if let Some(s) = s
            && s != self.sortierung.get()
        {
            self.sortierung_zeigen(s);
        }
    }

    /// Shows the arrow of `s` in the header (after a change from the menu).
    pub fn sortierung_zeigen(&self, s: Sortierung) {
        self.sortierung.set(s);
        let spalte = Spalte::fuer_kriterium(s.kriterium);
        let col = self
            .spalten
            .iter()
            .find(|(sp, _)| *sp == spalte)
            .map(|(_, c)| c.clone());
        self.zeigt.set(true);
        self.ansicht.sort_by_column(
            col.as_ref(),
            if s.umgekehrt {
                gtk4::SortType::Descending
            } else {
                gtk4::SortType::Ascending
            },
        );
        self.zeigt.set(false);
    }

    /// The columns to show (Name always; Reiter „Listenspalten“).
    pub fn spalten_setzen(&self, gewaehlt: &[Spalte]) {
        let sichtbar = sichtbare_spalten(gewaehlt);
        for (s, col) in &self.spalten {
            col.set_visible(sichtbar.contains(s));
        }
    }

    pub fn datumsformat_setzen(&self, iso: bool) {
        if iso != self.iso.get() {
            self.iso.set(iso);
            // the ColumnView binds its rows anew with a new model
            let modell = self.ansicht.model();
            self.ansicht.set_model(None::<&gtk4::SelectionModel>);
            self.ansicht.set_model(modell.as_ref());
        }
    }

    fn symbolgroesse(&self) -> i32 {
        ansichtsregeln::symbolgroesse(self.kern.zoom())
    }

    fn fabrik(self: &Rc<Self>, spalte: Spalte, factory: &gtk4::SignalListItemFactory) {
        let weak = Rc::downgrade(self);
        factory.connect_setup(move |_, obj| {
            let (Some(l), Some(li)) = (weak.upgrade(), obj.downcast_ref::<gtk4::ListItem>()) else {
                return;
            };
            if spalte == Spalte::Name {
                let zelle = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
                zelle.add_css_class("namenszelle");
                let bild = gtk4::Image::new();
                bild.set_pixel_size(l.symbolgroesse());
                let text = gtk4::Label::new(None);
                text.set_xalign(0.0);
                text.set_hexpand(true);
                text.set_ellipsize(pango::EllipsizeMode::End);
                zelle.append(&bild);
                zelle.append(&text);
                li.set_child(Some(&zelle));
                l.kern.zelle_anmelden(&zelle, li);
            } else {
                let text = gtk4::Label::new(None);
                text.set_xalign(if spalte == Spalte::Groesse { 1.0 } else { 0.0 });
                text.set_ellipsize(pango::EllipsizeMode::End);
                text.add_css_class("spalte");
                if spalte == Spalte::Rechte {
                    text.add_css_class("monospace");
                }
                li.set_child(Some(&text));
            }
        });
        let weak = Rc::downgrade(self);
        factory.connect_bind(move |_, obj| {
            let (Some(l), Some(li)) = (weak.upgrade(), obj.downcast_ref::<gtk4::ListItem>()) else {
                return;
            };
            let Some(e) = li.item().and_downcast::<Eintrag>() else {
                return;
            };
            if spalte == Spalte::Name {
                if let Some(zelle) = li.child().and_downcast::<gtk4::Box>() {
                    l.name_binden(&zelle, &e);
                }
            } else if let Some(text) = li.child().and_downcast::<gtk4::Label>() {
                text.set_text(&text_der_spalte(spalte, &e, l.iso.get()));
            }
        });
        if spalte == Spalte::Name {
            let weak = Rc::downgrade(self);
            factory.connect_unbind(move |_, obj| {
                let (Some(l), Some(li)) = (weak.upgrade(), obj.downcast_ref::<gtk4::ListItem>())
                else {
                    return;
                };
                if let Some(zelle) = li.child().and_downcast::<gtk4::Box>() {
                    l.kern.geloest(&zelle);
                }
            });
        }
    }

    fn name_binden(self: &Rc<Self>, zelle: &gtk4::Box, e: &Eintrag) {
        let (bild, text) = teile(zelle);
        self.kern.gebunden(zelle, e);
        let groesse = self.symbolgroesse();
        if bild.pixel_size() != groesse {
            bild.set_pixel_size(groesse);
        }
        // QD-106: theme icons only through querencia-oberflaeche
        symbole::set(&bild, &e.daten().symbol);
        text.set_text(&e.daten().anzeige);
    }

    pub fn zoom_setzen(self: &Rc<Self>, prozent: u32) {
        let prozent = ansichtsregeln::zoom_stufe(prozent);
        if prozent == self.kern.zoom() {
            return;
        }
        let t0 = Instant::now();
        self.kern.zoom_merken(prozent);
        for (zelle, li) in self.kern.lebende_zellen() {
            if let Some(e) = li.item().and_downcast::<Eintrag>() {
                self.name_binden(&zelle, &e);
            }
        }
        protokoll(&format!(
            "Zoom Listenansicht {prozent} % ({} px) in {} ms",
            ansichtsregeln::symbolgroesse(prozent),
            t0.elapsed().as_millis()
        ));
        let kern = self.kern.clone();
        glib::idle_add_local_once(move || {
            if let Some(pos) = kern.cursor_position() {
                kern.scrollen_zu(pos);
            }
        });
    }
}

impl Darstellung for Listenansicht {
    fn neu(auswahl: &gtk4::MultiSelection) -> Rc<Self> {
        Self::new(auswahl)
    }

    fn kern(&self) -> &Rc<Kern> {
        &self.kern
    }

    fn widget(&self) -> gtk4::Widget {
        Listenansicht::widget(self)
    }

    fn zoom_setzen(self: &Rc<Self>, prozent: u32) {
        Listenansicht::zoom_setzen(self, prozent);
    }

    fn einstellungen(self: &Rc<Self>, werte: &Werte) {
        let mut spalten = Vec::new();
        for (an, s) in [
            (werte.spalte_groesse, Spalte::Groesse),
            (werte.spalte_typ, Spalte::Typ),
            (werte.spalte_datum, Spalte::Datum),
            (werte.spalte_rechte, Spalte::Rechte),
            (werte.spalte_eigentuemer, Spalte::Eigentuemer),
            (werte.spalte_gruppe, Spalte::Gruppe),
        ] {
            if an {
                spalten.push(s);
            }
        }
        self.spalten_setzen(&spalten);
        self.datumsformat_setzen(werte.datum_iso);
    }
}

/// Registers the list view (kind „Liste“) and the view styles with the
/// frame (`main.rs`, `erweiterungen`).
pub fn registrieren(register: &mut Register) {
    register.ansicht(Ansichtsart::Liste, || {
        Modul::<Listenansicht>::neu(Ansichtsart::Liste) as Rc<dyn Ansicht>
    });
    register.anwendung(|_| ansichtskern::stil_laden());
}
