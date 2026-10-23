//! The toolbar (B §3.4): Zurück · Vor · Nach oben · Stopp · Aktualisieren |
//! Persönlicher Ordner · Rechner | Suchen (`suche.rs`) — right: zoom
//! (−, percentage, +), the view drop-down and the busy indicator. 16 px
//! icons, text only beside Zurück and Vor (GNOME 2 "important buttons
//! labelled"), tooltips everywhere, `can_focus=false` as in Dokumente.

use std::rc::{Rc, Weak};

use gtk4::prelude::*;
use querencia_oberflaeche::symbole;

use crate::fenster::Fenster;
use crate::i18n::gettext;
use crate::regeln::{Ansichtsart, Zoom};

pub struct Werkzeugleiste {
    leiste: gtk4::Box,
    zoom_text: gtk4::Label,
    kleiner: gtk4::Button,
    groesser: gtk4::Button,
    ansicht: gtk4::DropDown,
    arten: Vec<Ansichtsart>,
    drehen: gtk4::Spinner,
}

fn knopf(icon: &str, text: Option<&str>, tipp: &str, aktion: &str) -> gtk4::Button {
    let k = gtk4::Button::new();
    let inhalt = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
    inhalt.append(&symbole::image_named(icon, 16));
    if let Some(t) = text {
        inhalt.append(&gtk4::Label::new(Some(t)));
    }
    k.set_child(Some(&inhalt));
    k.set_tooltip_text(Some(tipp));
    k.set_action_name(Some(aktion));
    k.set_has_frame(false);
    k.set_can_focus(false);
    k.update_property(&[gtk4::accessible::Property::Label(tipp)]);
    k
}

fn trenner() -> gtk4::Separator {
    let t = gtk4::Separator::new(gtk4::Orientation::Vertical);
    t.set_margin_start(3);
    t.set_margin_end(3);
    t
}

impl Werkzeugleiste {
    pub fn new(fenster: &Rc<Fenster>) -> Self {
        let leiste = gtk4::Box::new(gtk4::Orientation::Horizontal, 2);
        leiste.add_css_class("querencia-werkzeugleiste");
        leiste.append(&knopf(
            "go-previous",
            Some(&gettext("Back")),
            &gettext("Go to the previous visited location"),
            "win.zurueck",
        ));
        leiste.append(&knopf(
            "go-next",
            Some(&gettext("Forward")),
            &gettext("Go to the next visited location"),
            "win.vor",
        ));
        leiste.append(&knopf(
            "go-up",
            None,
            &gettext("Go to the parent folder"),
            "win.nach-oben",
        ));
        leiste.append(&knopf(
            "process-stop",
            None,
            &gettext("Stop loading the current location"),
            "win.stopp",
        ));
        leiste.append(&knopf(
            "view-refresh",
            None,
            &gettext("Reload the current location"),
            "win.aktualisieren",
        ));
        leiste.append(&trenner());
        leiste.append(&knopf(
            "user-home",
            None,
            &gettext("Go to the home folder"),
            "win.persoenlich",
        ));
        leiste.append(&knopf(
            "computer",
            None,
            &gettext("Browse all local and remote disks and folders accessible from this computer"),
            "win.rechner",
        ));
        // Welle C2 (dateien-miniaturen-suche): Suchen
        leiste.append(&trenner());
        leiste.append(&crate::suche::werkzeugknopf());
        let luecke = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        luecke.set_hexpand(true);
        leiste.append(&luecke);

        let kleiner = knopf(
            "zoom-out",
            None,
            &gettext("Decrease the view size"),
            "win.verkleinern",
        );
        let zoom_text = gtk4::Label::new(Some(&Zoom::NORMAL.text()));
        zoom_text.set_width_chars(5);
        zoom_text.add_css_class("querencia-zoom");
        let groesser = knopf(
            "zoom-in",
            None,
            &gettext("Increase the view size"),
            "win.vergroessern",
        );
        // a click on the percentage is "Normale Größe" (Nautilus 2.30)
        let normal = gtk4::Button::new();
        normal.set_child(Some(&zoom_text));
        normal.set_has_frame(false);
        normal.set_can_focus(false);
        normal.set_action_name(Some("win.normale-groesse"));
        normal.set_tooltip_text(Some(&gettext("Use the normal view size")));
        leiste.append(&kleiner);
        leiste.append(&normal);
        leiste.append(&groesser);

        // 0.1: icon and list view (compact view: Beta)
        let arten = vec![Ansichtsart::Symbole, Ansichtsart::Liste];
        let namen: Vec<String> = arten.iter().map(|a| a.name()).collect();
        let namen_ref: Vec<&str> = namen.iter().map(String::as_str).collect();
        let ansicht = gtk4::DropDown::from_strings(&namen_ref);
        ansicht.set_can_focus(false);
        ansicht.set_tooltip_text(Some(&gettext("Choose how the items are shown")));
        ansicht.set_margin_start(6);
        let weak: Weak<Fenster> = Rc::downgrade(fenster);
        let arten2 = arten.clone();
        ansicht.connect_selected_notify(move |d| {
            let Some(art) = arten2.get(d.selected() as usize).copied() else {
                return;
            };
            if let Some(f) = weak.upgrade()
                && f.ansichten().art() != Some(art)
            {
                let _ = WidgetExt::activate_action(
                    f.window(),
                    "win.ansicht",
                    Some(&art.kennung().to_variant()),
                );
            }
        });
        leiste.append(&ansicht);
        let drehen = gtk4::Spinner::new();
        drehen.set_margin_start(6);
        drehen.set_margin_end(6);
        leiste.append(&drehen);
        Self {
            leiste,
            zoom_text,
            kleiner,
            groesser,
            ansicht,
            arten,
            drehen,
        }
    }

    pub fn widget(&self) -> &gtk4::Box {
        &self.leiste
    }

    pub fn zoom_anzeigen(&self, zoom: Zoom, kleiner: bool, groesser: bool) {
        self.zoom_text.set_text(&zoom.text());
        self.kleiner.set_sensitive(kleiner);
        self.groesser.set_sensitive(groesser);
    }

    pub fn ansicht_anzeigen(&self, art: Ansichtsart) {
        if let Some(i) = self.arten.iter().position(|a| *a == art) {
            self.ansicht.set_selected(i as u32);
        }
    }

    pub fn laedt(&self, an: bool) {
        self.drehen.set_spinning(an);
        self.drehen.set_visible(an);
    }

    /// Nothing to do: the buttons follow their actions.
    pub fn aktualisieren(&self, _fenster: &Fenster) {}
}
