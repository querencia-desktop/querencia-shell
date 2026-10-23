//! Bearbeiten → Einstellungen (B §3.6): five tabs with Nautilus' names –
//! Ansichten, Verhalten, Anzeige, Listenspalten, Vorschau – at most six
//! controls each. Not modal, built afresh each time (QD-076), only
//! "Hilfe" and "Schließen"; every change is written to `dateien.ini` at
//! once and reaches all windows (`konfig.rs`).

use std::rc::Rc;

use gtk4::prelude::*;

use crate::fenster::Fenster;
use crate::i18n::gettext;
use crate::konfig::{AusfuehrbareTexte, Werte, Wo, Zusatz};
use crate::regeln::{Ansichtsart, Kriterium, Zoom};

type Setzer<T> = fn(&mut Werte, T);

fn konfig_aendern(f: impl FnOnce(&mut Werte)) {
    if let Some(app) = crate::app::app_wenn() {
        app.konfig().aendern(f);
    }
}

fn schalter(text: &str, an: bool, setzen: Setzer<bool>) -> gtk4::CheckButton {
    let k = gtk4::CheckButton::with_mnemonic(text);
    k.set_active(an);
    k.connect_toggled(move |k| {
        let an = k.is_active();
        konfig_aendern(|w| setzen(w, an));
    });
    k
}

/// A label and a drop-down of `wahl` (shown names), `jetzt` chosen.
fn auswahl<T: Copy + PartialEq + 'static>(
    text: &str,
    wahl: Vec<(T, String)>,
    jetzt: T,
    setzen: Setzer<T>,
) -> gtk4::Box {
    let zeile = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    let l = gtk4::Label::with_mnemonic(text);
    l.set_xalign(0.0);
    l.set_hexpand(true);
    let namen: Vec<&str> = wahl.iter().map(|(_, n)| n.as_str()).collect();
    let d = gtk4::DropDown::from_strings(&namen);
    if let Some(i) = wahl.iter().position(|(w, _)| *w == jetzt) {
        d.set_selected(i as u32);
    }
    l.set_mnemonic_widget(Some(&d));
    let werte: Vec<T> = wahl.iter().map(|(w, _)| *w).collect();
    d.connect_selected_notify(move |d| {
        if let Some(w) = werte.get(d.selected() as usize).copied() {
            konfig_aendern(|k| setzen(k, w));
        }
    });
    zeile.append(&l);
    zeile.append(&d);
    zeile
}

fn rahmen(titel: &str, kinder: &[gtk4::Widget]) -> gtk4::Box {
    let b = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    let t = gtk4::Label::new(None);
    t.set_markup(&format!("<b>{}</b>", gtk4::glib::markup_escape_text(titel)));
    t.set_xalign(0.0);
    b.append(&t);
    let innen = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    innen.set_margin_start(12);
    for k in kinder {
        innen.append(k);
    }
    b.append(&innen);
    b
}

fn seite(teile: &[gtk4::Box]) -> gtk4::Box {
    let b = gtk4::Box::new(gtk4::Orientation::Vertical, 18);
    b.set_margin_top(12);
    b.set_margin_bottom(12);
    b.set_margin_start(12);
    b.set_margin_end(12);
    for t in teile {
        b.append(t);
    }
    b
}

fn zoomwahl() -> Vec<(Zoom, String)> {
    crate::regeln::ZOOMSTUFEN
        .iter()
        .map(|p| (Zoom(*p), Zoom(*p).text()))
        .collect()
}

fn ansichten(w: &Werte) -> gtk4::Box {
    let standard = rahmen(
        &gettext("Default View"),
        &[
            auswahl(
                &gettext("_View new folders using:"),
                vec![
                    (Ansichtsart::Symbole, Ansichtsart::Symbole.name()),
                    (Ansichtsart::Liste, Ansichtsart::Liste.name()),
                ],
                w.ansicht,
                |k, v| k.ansicht = v,
            )
            .upcast(),
            auswahl(
                &gettext("_Arrange items:"),
                vec![
                    (Kriterium::Name, gettext("By Name")),
                    (Kriterium::Groesse, gettext("By Size")),
                    (Kriterium::Typ, gettext("By Type")),
                    (Kriterium::Datum, gettext("By Modification Date")),
                ],
                w.anordnen,
                |k, v| k.anordnen = v,
            )
            .upcast(),
            schalter(
                &gettext("Sort _folders before files"),
                w.ordner_zuerst,
                |k, v| k.ordner_zuerst = v,
            )
            .upcast(),
            schalter(&gettext("Show _hidden files"), w.verborgene, |k, v| {
                k.verborgene = v
            })
            .upcast(),
        ],
    );
    let zoom = rahmen(
        &gettext("Default Zoom Levels"),
        &[
            auswahl(
                &gettext("Icon _view:"),
                zoomwahl(),
                w.zoom_symbole,
                |k, v| k.zoom_symbole = v,
            )
            .upcast(),
            auswahl(&gettext("_List view:"), zoomwahl(), w.zoom_liste, |k, v| {
                k.zoom_liste = v
            })
            .upcast(),
        ],
    );
    seite(&[standard, zoom])
}

fn radio(
    text: &str,
    gruppe: Option<&gtk4::CheckButton>,
    an: bool,
    setzen: impl Fn() + 'static,
) -> gtk4::CheckButton {
    let r = gtk4::CheckButton::with_mnemonic(text);
    if let Some(g) = gruppe {
        r.set_group(Some(g));
    }
    r.set_active(an);
    r.connect_toggled(move |r| {
        if r.is_active() {
            setzen();
        }
    });
    r
}

fn verhalten(w: &Werte) -> gtk4::Box {
    let doppel = radio(
        &gettext("_Double click to open items"),
        None,
        !w.einfacher_klick,
        || konfig_aendern(|k| k.einfacher_klick = false),
    );
    let einfach = radio(
        &gettext("_Single click to open items"),
        Some(&doppel),
        w.einfacher_klick,
        || konfig_aendern(|k| k.einfacher_klick = true),
    );
    let oeffnen = rahmen(&gettext("Behavior"), &[doppel.upcast(), einfach.upcast()]);
    let texte = rahmen(
        &gettext("Executable Text Files"),
        &[auswahl(
            &gettext("When _opened:"),
            vec![
                (AusfuehrbareTexte::Nachfragen, gettext("Ask each time")),
                (AusfuehrbareTexte::Anzeigen, gettext("View them")),
                (AusfuehrbareTexte::Ausfuehren, gettext("Run them")),
            ],
            w.ausfuehrbare_texte,
            |k, v| k.ausfuehrbare_texte = v,
        )
        .upcast()],
    );
    let papierkorb = rahmen(
        &gettext("Trash"),
        &[
            schalter(
                &gettext("Ask before _emptying the trash or deleting files"),
                w.nachfragen,
                |k, v| k.nachfragen = v,
            )
            .upcast(),
            schalter(
                &gettext("Include a Dele_te command that bypasses the trash"),
                w.loeschen_anbieten,
                |k, v| k.loeschen_anbieten = v,
            )
            .upcast(),
        ],
    );
    let adresse = rahmen(
        &gettext("Location"),
        &[schalter(
            &gettext("Always use the location _entry instead of the path buttons"),
            w.adresse_als_text,
            |k, v| k.adresse_als_text = v,
        )
        .upcast()],
    );
    seite(&[oeffnen, texte, papierkorb, adresse])
}

fn zusatzwahl() -> Vec<(Zusatz, String)> {
    vec![
        (Zusatz::Nichts, gettext("None")),
        (Zusatz::Groesse, gettext("Size")),
        (Zusatz::Datum, gettext("Date Modified")),
        (Zusatz::Typ, gettext("Type")),
    ]
}

fn anzeige(w: &Werte) -> gtk4::Box {
    let beschriftung = rahmen(
        &gettext("Icon Captions"),
        &[
            auswahl(
                &gettext("_Below the name:"),
                zusatzwahl(),
                w.zusatz,
                |k, v| k.zusatz = v,
            )
            .upcast(),
            auswahl(
                &gettext("From 150 % _also:"),
                zusatzwahl(),
                w.zusatz_gross,
                |k, v| k.zusatz_gross = v,
            )
            .upcast(),
        ],
    );
    let datum = rahmen(
        &gettext("Date"),
        &[auswahl(
            &gettext("_Format:"),
            vec![
                (false, gettext("As the region")),
                (true, gettext("ISO 8601")),
            ],
            w.datum_iso,
            |k, v| k.datum_iso = v,
        )
        .upcast()],
    );
    // Nautilus 2.30: „Vorgaben für kompakte Ansicht“ (Welle C3)
    let kompakt = rahmen(
        &gettext("Compact View"),
        &[schalter(
            &gettext("A_ll columns have the same width"),
            w.kompakt_gleich_breit,
            |k, v| k.kompakt_gleich_breit = v,
        )
        .upcast()],
    );
    seite(&[beschriftung, datum, kompakt])
}

fn spalten(w: &Werte) -> gtk4::Box {
    let teil = rahmen(
        &gettext("List Columns"),
        &[
            schalter(&gettext("_Size"), w.spalte_groesse, |k, v| {
                k.spalte_groesse = v
            })
            .upcast(),
            schalter(&gettext("_Type"), w.spalte_typ, |k, v| k.spalte_typ = v).upcast(),
            schalter(&gettext("_Date Modified"), w.spalte_datum, |k, v| {
                k.spalte_datum = v
            })
            .upcast(),
            schalter(&gettext("_Permissions"), w.spalte_rechte, |k, v| {
                k.spalte_rechte = v
            })
            .upcast(),
            schalter(&gettext("_Owner"), w.spalte_eigentuemer, |k, v| {
                k.spalte_eigentuemer = v
            })
            .upcast(),
            schalter(&gettext("_Group"), w.spalte_gruppe, |k, v| {
                k.spalte_gruppe = v
            })
            .upcast(),
        ],
    );
    let hinweis = gtk4::Label::new(Some(&gettext("The name is always shown.")));
    hinweis.add_css_class("dim-label");
    hinweis.set_xalign(0.0);
    let b = seite(&[teil]);
    b.append(&hinweis);
    b
}

fn wowahl() -> Vec<(Wo, String)> {
    vec![
        (Wo::Immer, gettext("Always")),
        (Wo::NurLokal, gettext("Local files only")),
        (Wo::Nie, gettext("Never")),
    ]
}

fn vorschau(w: &Werte) -> gtk4::Box {
    let bilder = rahmen(
        &gettext("Thumbnails"),
        &[
            auswahl(
                &gettext("_Show thumbnails:"),
                wowahl(),
                w.vorschaubilder,
                |k, v| k.vorschaubilder = v,
            )
            .upcast(),
            auswahl(
                &gettext("_Only for files smaller than:"),
                [1u32, 3, 10, 30, 100]
                    .into_iter()
                    .map(|n| (n, format!("{n} MB")))
                    .collect(),
                w.vorschau_max_mb,
                |k, v| k.vorschau_max_mb = v,
            )
            .upcast(),
        ],
    );
    let zaehlen = rahmen(
        &gettext("Folders"),
        &[auswahl(
            &gettext("_Count number of items:"),
            wowahl(),
            w.objekte_zaehlen,
            |k, v| k.objekte_zaehlen = v,
        )
        .upcast()],
    );
    seite(&[bilder, zaehlen])
}

/// Opens the dialog for `fenster`.
pub fn zeigen(fenster: &Rc<Fenster>) {
    let parent: gtk4::Window = fenster.window().clone().upcast();
    // an open dialog of the process comes to the front instead
    if let Some(app) = fenster.app() {
        for w in app.gtk().windows() {
            if w.widget_name() == "querencia-dateien-einstellungen" {
                w.present();
                return;
            }
        }
    }
    let window =
        crate::info::dialogfenster(&parent, &gettext("File Management Preferences"), false);
    window.set_widget_name("querencia-dateien-einstellungen");
    window.set_transient_for(None::<&gtk4::Window>);
    let w = fenster
        .app()
        .map(|a| a.konfig().werte())
        .unwrap_or_default();
    let reiter = gtk4::Notebook::new();
    for (titel, inhalt) in [
        (gettext("Views"), ansichten(&w)),
        (gettext("Behavior"), verhalten(&w)),
        (gettext("Display"), anzeige(&w)),
        (gettext("List Columns"), spalten(&w)),
        (gettext("Preview"), vorschau(&w)),
    ] {
        reiter.append_page(&inhalt, Some(&gtk4::Label::new(Some(&titel))));
    }
    let hilfe = gtk4::Button::with_mnemonic(&gettext("_Help"));
    let zu = gtk4::Button::with_mnemonic(&gettext("_Close"));
    let knoepfe = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    knoepfe.append(&hilfe);
    let luecke = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    luecke.set_hexpand(true);
    knoepfe.append(&luecke);
    knoepfe.append(&zu);
    let aussen = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    aussen.set_margin_top(12);
    aussen.set_margin_bottom(12);
    aussen.set_margin_start(12);
    aussen.set_margin_end(12);
    aussen.append(&reiter);
    aussen.append(&knoepfe);
    window.set_child(Some(&aussen));
    window.set_default_widget(Some(&zu));
    let ww = window.downgrade();
    zu.connect_clicked(move |_| {
        if let Some(w) = ww.upgrade() {
            gtk4::glib::idle_add_local_once(move || w.close());
        }
    });
    let ww = window.downgrade();
    hilfe.connect_clicked(move |_| {
        if let Some(w) = ww.upgrade() {
            crate::info::online_hilfe(&w);
        }
    });
    window.present();
    zu.grab_focus();
}
