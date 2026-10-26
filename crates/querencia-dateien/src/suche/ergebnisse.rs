//! The view „Suchergebnisse“: a list of the hits with the columns Name,
//! Ort (the folder of the hit), Größe, Typ and Änderungsdatum, sortable by
//! a click on a column head. A double click or Enter opens the selected
//! hits, the context menu offers „Öffnen“ and „Im Ordner anzeigen“.
//! Thumbnails come through the Bildquelle je Eintrag as in the list view.
//!
//! The view sits in the window's view area while the search is shown; keys
//! the window binds there for the folder (Entf, Strg+C …) are kept from it
//! here, they would act on the hidden folder's selection.

use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{gdk, gio, glib, pango};
use querencia_oberflaeche::classic_menu;

use crate::ansichtskern;
use crate::ansichtsregeln::{datumsmuster, groesse_text};
use crate::eintrag::Eintrag;
use crate::i18n::gettext;
use crate::menue;
use crate::miniaturen::bildquelle;
use crate::regeln;

/// Icon size of the rows (the list view's standard zoom, 50 %).
const SYMBOL: i32 = 24;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Spalte {
    Name,
    Ort,
    Groesse,
    Typ,
    Datum,
}

impl Spalte {
    const ALLE: [Spalte; 5] = [
        Spalte::Name,
        Spalte::Ort,
        Spalte::Groesse,
        Spalte::Typ,
        Spalte::Datum,
    ];

    fn titel(self) -> String {
        match self {
            Spalte::Name => gettext("Name"),
            // TRANSLATORS: column of the search results: the folder of a hit
            Spalte::Ort => gettext("Location"),
            Spalte::Groesse => gettext("Size"),
            Spalte::Typ => gettext("Type"),
            Spalte::Datum => gettext("Date Modified"),
        }
    }
}

/// The folder of a hit as the Ort column shows it (`~` for the home
/// folder, as the location entry).
pub fn ort_text(datei: &gio::File) -> String {
    let Some(eltern) = datei.parent() else {
        return String::new();
    };
    let Some(pfad) = eltern.path() else {
        return eltern.parse_name().to_string();
    };
    let heim = glib::home_dir();
    match pfad.strip_prefix(&heim) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".to_owned(),
        Ok(rest) => format!("~/{}", rest.to_string_lossy()),
        Err(_) => pfad.to_string_lossy().into_owned(),
    }
}

fn text(spalte: Spalte, e: &Eintrag) -> String {
    let d = e.daten();
    match spalte {
        Spalte::Name => d.anzeige.clone(),
        Spalte::Ort => ort_text(&d.datei),
        Spalte::Groesse => groesse_text(d.ist_ordner, ansichtskern::groesse(e), regeln::groesse),
        Spalte::Typ => ansichtskern::typ_text(e),
        Spalte::Datum => glib::DateTime::from_unix_local(ansichtskern::zeit(e))
            .ok()
            .and_then(|t| t.format(datumsmuster(false)).ok())
            .map(|t| t.to_string())
            .unwrap_or_default(),
    }
}

fn vergleich(spalte: Spalte, a: &Eintrag, b: &Eintrag) -> std::cmp::Ordering {
    let (x, y) = (&a.daten().sort, &b.daten().sort);
    match spalte {
        Spalte::Name => x.name.cmp(&y.name),
        Spalte::Ort => ort_text(a.datei()).cmp(&ort_text(b.datei())),
        Spalte::Groesse => x.groesse.cmp(&y.groesse),
        Spalte::Typ => x.typ.cmp(&y.typ),
        Spalte::Datum => x.zeit.cmp(&y.zeit),
    }
    .then_with(|| x.name.cmp(&y.name))
}

/// What the results tell the search (window side).
pub trait Handlung {
    fn oeffnen(&self, eintraege: &[Eintrag]);
    fn im_ordner_zeigen(&self, eintrag: &Eintrag);
    /// Esc in the list.
    fn escape(&self);
}

pub struct Ergebnisse {
    wurzel: gtk4::Overlay,
    liste: gtk4::ColumnView,
    store: gio::ListStore,
    auswahl: gtk4::MultiSelection,
    anker: gtk4::MenuButton,
    menue: gio::Menu,
}

impl Ergebnisse {
    pub fn new(handlung: Rc<dyn Handlung>) -> Rc<Self> {
        let store = gio::ListStore::new::<Eintrag>();
        let liste = gtk4::ColumnView::new(None::<gtk4::MultiSelection>);
        liste.add_css_class("querencia-liste");
        liste.add_css_class("querencia-suchergebnisse");
        liste.set_enable_rubberband(true);
        liste.set_show_row_separators(false);
        liste.set_reorderable(false);
        let sortiert = gtk4::SortListModel::new(Some(store.clone()), liste.sorter());
        let auswahl = gtk4::MultiSelection::new(Some(sortiert));
        let mut name_spalte = None;
        for spalte in Spalte::ALLE {
            let factory = gtk4::SignalListItemFactory::new();
            fabrik(spalte, &factory);
            let col = gtk4::ColumnViewColumn::new(Some(&spalte.titel()), Some(factory));
            col.set_resizable(true);
            col.set_expand(matches!(spalte, Spalte::Name | Spalte::Ort));
            col.set_sorter(Some(&gtk4::CustomSorter::new(move |a, b| {
                match (a.downcast_ref::<Eintrag>(), b.downcast_ref::<Eintrag>()) {
                    (Some(a), Some(b)) => vergleich(spalte, a, b).into(),
                    _ => gtk4::Ordering::Equal,
                }
            })));
            liste.append_column(&col);
            if spalte == Spalte::Name {
                name_spalte = Some(col);
            }
        }
        liste.set_model(Some(&auswahl));
        liste.sort_by_column(name_spalte.as_ref(), gtk4::SortType::Ascending);
        let scroller = gtk4::ScrolledWindow::builder()
            .child(&liste)
            .hexpand(true)
            .vexpand(true)
            .build();
        let anker = gtk4::MenuButton::new();
        anker.set_can_focus(false);
        anker.set_has_frame(false);
        anker.set_opacity(0.0);
        anker.set_size_request(1, 1);
        anker.set_halign(gtk4::Align::Start);
        anker.set_valign(gtk4::Align::Start);
        let menue = gio::Menu::new();
        classic_menu::attach_popup(&anker, menue.upcast_ref());
        let wurzel = gtk4::Overlay::new();
        wurzel.set_child(Some(&scroller));
        wurzel.add_overlay(&anker);
        wurzel.set_hexpand(true);
        wurzel.set_vexpand(true);
        let e = Rc::new(Self {
            wurzel,
            liste,
            store,
            auswahl,
            anker,
            menue,
        });
        e.signale(handlung);
        e
    }

    pub fn widget(&self) -> &gtk4::Overlay {
        &self.wurzel
    }

    pub fn leeren(&self) {
        self.store.remove_all();
    }

    pub fn anhaengen(&self, eintraege: &[Eintrag]) {
        self.store.extend_from_slice(eintraege);
    }

    pub fn anzahl(&self) -> u32 {
        self.store.n_items()
    }

    pub fn fokus(&self) {
        self.liste.grab_focus();
    }

    /// `f` after every change of the selection of hits (also when hits
    /// go or come).
    pub fn bei_auswahl(&self, f: impl Fn() + 'static) {
        let f = Rc::new(f);
        let g = f.clone();
        self.auswahl.connect_selection_changed(move |_, _, _| g());
        self.auswahl.connect_items_changed(move |_, _, _, _| f());
    }

    /// The selected hits in shown order.
    pub fn gewaehlte(&self) -> Vec<Eintrag> {
        let menge = self.auswahl.selection();
        let mut out = Vec::new();
        for i in 0..menge.size() {
            let pos = menge.nth(i as u32);
            if let Some(e) = self.auswahl.item(pos).and_downcast::<Eintrag>() {
                out.push(e);
            }
        }
        out
    }

    /// The hit under `x`, `y` of the list (to select it before a context
    /// menu), found through the name cell's label.
    fn eintrag_bei(&self, x: f64, y: f64) -> Option<(u32, Eintrag)> {
        let mut w = self.liste.pick(x, y, gtk4::PickFlags::DEFAULT)?;
        loop {
            if let Some(pos) = zeile_von(&w) {
                return self
                    .auswahl
                    .item(pos)
                    .and_downcast::<Eintrag>()
                    .map(|e| (pos, e));
            }
            w = w.parent()?;
        }
    }

    fn signale(self: &Rc<Self>, handlung: Rc<dyn Handlung>) {
        let weak = Rc::downgrade(self);
        let h = handlung.clone();
        self.liste.connect_activate(move |_, pos| {
            let Some(e) = weak.upgrade() else { return };
            let mut auswahl = e.gewaehlte();
            if auswahl.is_empty()
                && let Some(x) = e.auswahl.item(pos).and_downcast::<Eintrag>()
            {
                auswahl.push(x);
            }
            h.oeffnen(&auswahl);
        });
        // context menu
        let gruppe = gio::SimpleActionGroup::new();
        let oeffnen = gio::SimpleAction::new("oeffnen", None);
        let weak = Rc::downgrade(self);
        let h = handlung.clone();
        oeffnen.connect_activate(move |_, _| {
            if let Some(e) = weak.upgrade() {
                h.oeffnen(&e.gewaehlte());
            }
        });
        gruppe.add_action(&oeffnen);
        let zeigen = gio::SimpleAction::new("im-ordner-zeigen", None);
        let weak = Rc::downgrade(self);
        let h = handlung.clone();
        zeigen.connect_activate(move |_, _| {
            if let Some(e) = weak.upgrade()
                && let Some(x) = e.gewaehlte().first()
            {
                h.im_ordner_zeigen(x);
            }
        });
        gruppe.add_action(&zeigen);
        self.wurzel.insert_action_group("suche", Some(&gruppe));
        let klick = gtk4::GestureClick::new();
        klick.set_button(gdk::BUTTON_SECONDARY);
        let weak = Rc::downgrade(self);
        klick.connect_pressed(move |g, _, x, y| {
            let Some(e) = weak.upgrade() else { return };
            match e.eintrag_bei(x, y) {
                Some((pos, _)) => {
                    if !e.auswahl.is_selected(pos) {
                        e.auswahl.select_item(pos, true);
                    }
                    e.kontextmenue(x, y);
                }
                None => {
                    e.auswahl.unselect_all();
                }
            }
            g.set_state(gtk4::EventSequenceState::Claimed);
        });
        self.liste.add_controller(klick);
        // keys: the window's file keys must not reach the hidden folder
        let tasten = gtk4::EventControllerKey::new();
        let weak = Rc::downgrade(self);
        tasten.connect_key_pressed(move |_, taste, _, zustand| {
            let Some(e) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            let strg = zustand.contains(gdk::ModifierType::CONTROL_MASK);
            match taste {
                gdk::Key::Escape => {
                    handlung.escape();
                    glib::Propagation::Stop
                }
                gdk::Key::a | gdk::Key::A if strg => {
                    e.auswahl.select_all();
                    glib::Propagation::Stop
                }
                gdk::Key::Menu => {
                    e.kontextmenue(24.0, 24.0);
                    glib::Propagation::Stop
                }
                k if gesperrt(k, strg) => glib::Propagation::Stop,
                _ => glib::Propagation::Proceed,
            }
        });
        self.wurzel.add_controller(tasten);
    }

    fn kontextmenue(&self, x: f64, y: f64) {
        let auswahl = self.gewaehlte();
        if auswahl.is_empty() {
            return;
        }
        self.menue.remove_all();
        let teil = gio::Menu::new();
        teil.append_item(&menue::eintrag(
            &gettext("_Open"),
            Some("suche.oeffnen"),
            None,
            None,
            None,
        ));
        if auswahl.len() == 1 {
            teil.append_item(&menue::eintrag(
                // TRANSLATORS: context menu of a search result
                &gettext("Show in _Folder"),
                Some("suche.im-ordner-zeigen"),
                None,
                None,
                None,
            ));
        }
        self.menue.append_section(None, &teil);
        let (x, y) = self
            .liste
            .compute_point(
                &self.wurzel,
                &gtk4::graphene::Point::new(x as f32, y as f32),
            )
            .map_or((x, y), |p| (f64::from(p.x()), f64::from(p.y())));
        if let Some(pop) = self.anker.popover() {
            pop.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
        }
        self.anker.popup();
    }
}

/// Keys the window binds on the view area for files (`menue.rs`,
/// `KUERZEL_IM_BEREICH`) other than Esc and Strg+A.
fn gesperrt(k: gdk::Key, strg: bool) -> bool {
    matches!(
        k,
        gdk::Key::Delete | gdk::Key::KP_Delete | gdk::Key::F2 | gdk::Key::BackSpace
    ) || (strg
        && matches!(
            k,
            gdk::Key::x | gdk::Key::X | gdk::Key::c | gdk::Key::C | gdk::Key::v | gdk::Key::V
        ))
}

thread_local! {
    /// The list item of a cell (set at bind, for `eintrag_bei`; the
    /// position is asked when needed, sorting moves rows without binding).
    static ZEILEN: std::cell::RefCell<std::collections::HashMap<usize, glib::WeakRef<gtk4::ListItem>>> =
        std::cell::RefCell::default();
}

fn zeile_von(w: &gtk4::Widget) -> Option<u32> {
    ZEILEN.with(|z| {
        z.borrow()
            .get(&(w.as_ptr() as usize))
            .and_then(|li| li.upgrade())
            .map(|li| li.position())
            .filter(|p| *p != gtk4::INVALID_LIST_POSITION)
    })
}

fn fabrik(spalte: Spalte, factory: &gtk4::SignalListItemFactory) {
    factory.connect_setup(move |_, obj| {
        let Some(li) = obj.downcast_ref::<gtk4::ListItem>() else {
            return;
        };
        let zelle = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        if spalte == Spalte::Name {
            let bild = gtk4::Image::new();
            bild.set_pixel_size(SYMBOL);
            zelle.append(&bild);
        }
        let text = gtk4::Label::new(None);
        text.set_xalign(if spalte == Spalte::Groesse { 1.0 } else { 0.0 });
        text.set_hexpand(true);
        text.set_ellipsize(if spalte == Spalte::Ort {
            pango::EllipsizeMode::Start
        } else {
            pango::EllipsizeMode::End
        });
        zelle.append(&text);
        li.set_child(Some(&zelle));
    });
    factory.connect_bind(move |_, obj| {
        let Some(li) = obj.downcast_ref::<gtk4::ListItem>() else {
            return;
        };
        let (Some(zelle), Some(e)) = (
            li.child().and_downcast::<gtk4::Box>(),
            li.item().and_downcast::<Eintrag>(),
        ) else {
            return;
        };
        ZEILEN.with(|z| {
            z.borrow_mut()
                .insert(zelle.as_ptr() as usize, li.downgrade())
        });
        if let Some(bild) = zelle.first_child().and_downcast::<gtk4::Image>() {
            bildquelle::binden(&bild, &e, bildquelle::Groesse::liste(SYMBOL));
        }
        if let Some(t) = zelle.last_child().and_downcast::<gtk4::Label>() {
            let s = text(spalte, &e);
            if spalte == Spalte::Ort {
                t.set_tooltip_text(Some(&s));
            }
            t.set_text(&s);
        }
    });
    factory.connect_unbind(move |_, obj| {
        let Some(li) = obj.downcast_ref::<gtk4::ListItem>() else {
            return;
        };
        if let Some(zelle) = li.child().and_downcast::<gtk4::Box>() {
            ZEILEN.with(|z| z.borrow_mut().remove(&(zelle.as_ptr() as usize)));
            if let Some(bild) = zelle.first_child().and_downcast::<gtk4::Image>() {
                bildquelle::loesen(&bild);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_location_column_shortens_the_home_folder() {
        let heim = glib::home_dir();
        assert_eq!(ort_text(&gio::File::for_path(heim.join("a.txt"))), "~");
        assert_eq!(
            ort_text(&gio::File::for_path(
                heim.join("Dokumente/Rechnungen/a.pdf")
            )),
            "~/Dokumente/Rechnungen"
        );
        assert_eq!(ort_text(&gio::File::for_path("/etc/fstab")), "/etc");
    }

    #[test]
    fn file_keys_are_kept_from_the_folder() {
        assert!(gesperrt(gdk::Key::Delete, false));
        assert!(gesperrt(gdk::Key::c, true));
        assert!(!gesperrt(gdk::Key::c, false));
        assert!(!gesperrt(gdk::Key::Down, false));
    }
}
