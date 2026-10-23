//! The menu bar (B §3.3, stage 0.1) over `querencia_oberflaeche::classic_menu`
//! and the shortcuts. Texts are the B §3.3 msgids (English, German in
//! `po/de.po`) with their access keys; the classic menus show labels as
//! they are, so the underscore is taken out of entry labels (the menu bar
//! buttons keep it: GTK makes Alt+D, Alt+B … from it).
//!
//! The classic menus have no check or radio marks and no insensitive
//! state of their own: an entry without `action` is drawn insensitive.
//! Each menu is therefore rebuilt just before it opens
//! (`MenuButton::set_create_popup_func`): an entry gets its action only
//! while the action exists and is enabled in the window, toggles show a
//! check icon and the chosen view or sort order a radio icon.
//!
//! Shortcuts: keys that act on files are bound on the view area only
//! ([`KUERZEL_IM_BEREICH`], `fenster.rs`); the others are application
//! accels ([`kuerzel_setzen`]). The `accel` attribute of an entry is only
//! the text shown; for some keys it comes from the catalog (Alt+Pos1).

use std::rc::{Rc, Weak};

use gtk4::prelude::*;
use gtk4::{gio, glib};
use querencia_oberflaeche::classic_menu;

use crate::eintrag::Eintrag;
use crate::fenster::Fenster;
use crate::i18n::{gettext, pgettext};

/// Keys bound on the view area of a window (action without `win.`).
pub const KUERZEL_IM_BEREICH: &[(&str, &[&str])] = &[
    ("ausschneiden", &["<Control>x"]),
    ("kopieren", &["<Control>c"]),
    ("einfuegen", &["<Control>v"]),
    ("alles-auswaehlen", &["<Control>a"]),
    ("umbenennen", &["F2"]),
    ("umbenennen-ganz", &["<Shift>F2"]),
    ("papierkorb", &["Delete", "KP_Delete"]),
    ("loeschen", &["<Shift>Delete", "<Shift>KP_Delete"]),
    ("stopp", &["Escape"]),
];

/// Application accels (detailed action names).
pub const KUERZEL: &[(&str, &[&str])] = &[
    ("app.neues-fenster", &["<Control>n"]),
    ("app.alle-schliessen", &["<Shift><Control>w"]),
    ("win.schliessen", &["<Control>w"]),
    ("win.ordner-anlegen", &["<Shift><Control>n"]),
    ("win.oeffnen", &["<Control>o"]),
    ("win.eigenschaften", &["<Alt>Return", "<Control>i"]),
    ("win.aktualisieren", &["<Control>r", "F5"]),
    ("win.seitenleiste", &["F9"]),
    ("win.verborgene", &["<Control>h"]),
    (
        "win.vergroessern",
        &["<Control>plus", "<Control>equal", "<Control>KP_Add"],
    ),
    (
        "win.verkleinern",
        &["<Control>minus", "<Control>KP_Subtract"],
    ),
    ("win.normale-groesse", &["<Control>0", "<Control>KP_0"]),
    ("win.ansicht::symbole", &["<Control>1"]),
    ("win.ansicht::liste", &["<Control>2"]),
    ("win.ansicht::kompakt", &["<Control>3"]),
    ("win.nach-oben", &["<Alt>Up"]),
    ("win.zurueck", &["<Alt>Left"]),
    ("win.vor", &["<Alt>Right"]),
    ("win.persoenlich", &["<Alt>Home"]),
    ("win.ort", &["<Control>l"]),
    ("win.lesezeichen-hinzufuegen", &["<Control>d"]),
    ("win.hilfe", &["F1"]),
    // Reiter (Welle C3, `reiter/`)
    ("win.neuer-reiter", &["<Control>t"]),
    ("win.oeffnen-neuer-reiter", &["<Shift><Control>o"]),
    ("win.reiter-vorheriger", &["<Control>Page_Up"]),
    ("win.reiter-naechster", &["<Control>Page_Down"]),
    ("win.reiter-links", &["<Shift><Control>Page_Up"]),
    ("win.reiter-rechts", &["<Shift><Control>Page_Down"]),
    ("win.reiter::1", &["<Alt>1"]),
    ("win.reiter::2", &["<Alt>2"]),
    ("win.reiter::3", &["<Alt>3"]),
    ("win.reiter::4", &["<Alt>4"]),
    ("win.reiter::5", &["<Alt>5"]),
    ("win.reiter::6", &["<Alt>6"]),
    ("win.reiter::7", &["<Alt>7"]),
    ("win.reiter::8", &["<Alt>8"]),
    ("win.reiter::9", &["<Alt>9"]),
    ("win.reiter::0", &["<Alt>0"]),
];

/// Sets the application accels (startup).
pub fn kuerzel_setzen(app: &gtk4::Application) {
    for (aktion, tasten) in KUERZEL {
        app.set_accels_for_action(aktion, tasten);
    }
}

/// How an entry shows its state.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Art {
    Normal,
    /// Boolean state: check icon while on.
    Schalter,
    /// String state: radio icon on the entry whose target is the state.
    Wahl,
}

/// One entry of a menu table.
struct Punkt {
    /// msgid with access key (B §3.3).
    text: &'static str,
    /// Detailed action without target, `None` for a submenu.
    aktion: &'static str,
    ziel: Option<&'static str>,
    /// Shown shortcut (GTK notation, only display).
    kuerzel: Option<&'static str>,
    icon: Option<&'static str>,
    art: Art,
}

const fn p(
    text: &'static str,
    aktion: &'static str,
    kuerzel: Option<&'static str>,
    icon: Option<&'static str>,
) -> Punkt {
    Punkt {
        text,
        aktion,
        ziel: None,
        kuerzel,
        icon,
        art: Art::Normal,
    }
}

const fn s(text: &'static str, aktion: &'static str, kuerzel: Option<&'static str>) -> Punkt {
    Punkt {
        text,
        aktion,
        ziel: None,
        kuerzel,
        icon: None,
        art: Art::Schalter,
    }
}

const fn w(
    text: &'static str,
    aktion: &'static str,
    ziel: &'static str,
    kuerzel: Option<&'static str>,
) -> Punkt {
    Punkt {
        text,
        aktion,
        ziel: Some(ziel),
        kuerzel,
        icon: None,
        art: Art::Wahl,
    }
}

/// Takes the access key marks out of a label ("S_chließen" → "Schließen").
pub fn ohne_zugriffstaste(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut zeichen = text.chars().peekable();
    while let Some(c) = zeichen.next() {
        if c == '_' {
            if zeichen.peek() == Some(&'_') {
                out.push('_');
                zeichen.next();
            }
            continue;
        }
        out.push(c);
    }
    out
}

/// A menu item from translated text, action, target, shortcut and icon.
pub fn eintrag(
    text: &str,
    aktion: Option<&str>,
    ziel: Option<&str>,
    kuerzel: Option<&str>,
    icon: Option<&gio::Icon>,
) -> gio::MenuItem {
    let item = gio::MenuItem::new(Some(&ohne_zugriffstaste(text)), None);
    if let Some(a) = aktion {
        item.set_action_and_target_value(Some(a), ziel.map(|z| z.to_variant()).as_ref());
    }
    if let Some(k) = kuerzel {
        item.set_attribute_value("accel", Some(&k.to_variant()));
    }
    if let Some(i) = icon {
        item.set_icon(i);
    }
    item
}

/// The text shown for a shortcut whose key has a German name the classic
/// menus do not know (Pos1); others pass through.
fn kuerzel_text(k: &str) -> String {
    match k {
        // TRANSLATORS: shortcut shown in the Go menu, deutsch „<Alt>Pos1“
        "<Alt>Home" => pgettext("accel", "<Alt>Home"),
        k => k.to_owned(),
    }
}

/// Text of a table entry (the msgids must appear literally for the
/// catalog tool, see [`texte`]).
fn text(msgid: &str) -> String {
    gettext(msgid)
}

/// Marks a msgid in the tables for the catalog tool (`werkzeug/i18n.py`
/// knows `N_` as gettext's "no-op" keyword); translated when shown.
#[allow(non_snake_case)]
const fn N_(msgid: &'static str) -> &'static str {
    msgid
}

fn datei() -> Vec<Vec<Punkt>> {
    vec![
        vec![
            p(
                N_("New _Tab"),
                "win.neuer-reiter",
                Some("<Control>t"),
                Some("tab-new"),
            ),
            p(
                N_("New _Window"),
                "app.neues-fenster",
                Some("<Control>n"),
                Some("window-new"),
            ),
        ],
        vec![
            p(
                N_("Create _Folder"),
                "win.ordner-anlegen",
                Some("<Shift><Control>n"),
                Some("folder-new"),
            ),
            // submenu: filled from ~/Vorlagen (dokument_anlegen_menue)
            p(N_("Create _Document"), "", None, Some("document-new")),
        ],
        vec![
            p(
                N_("_Open"),
                "win.oeffnen",
                Some("<Control>o"),
                Some("document-open"),
            ),
            p(
                N_("Open in New _Tab"),
                "win.oeffnen-neuer-reiter",
                Some("<Shift><Control>o"),
                None,
            ),
            p(
                N_("Open in New _Window"),
                "win.oeffnen-neues-fenster",
                None,
                None,
            ),
            p(N_("Open Wit_h"), "", None, None),
        ],
        vec![p(
            N_("_Properties"),
            "win.eigenschaften",
            Some("<Alt>Return"),
            Some("document-properties"),
        )],
        vec![p(
            N_("_Empty Trash…"),
            "win.papierkorb-leeren",
            None,
            Some("user-trash"),
        )],
        vec![
            p(
                N_("Close _All Windows"),
                "app.alle-schliessen",
                Some("<Shift><Control>w"),
                None,
            ),
            p(
                N_("_Close"),
                "win.schliessen",
                Some("<Control>w"),
                Some("window-close"),
            ),
        ],
    ]
}

fn bearbeiten() -> Vec<Vec<Punkt>> {
    vec![
        vec![
            p(
                N_("Cu_t"),
                "win.ausschneiden",
                Some("<Control>x"),
                Some("edit-cut"),
            ),
            p(
                N_("_Copy"),
                "win.kopieren",
                Some("<Control>c"),
                Some("edit-copy"),
            ),
            p(
                N_("_Paste"),
                "win.einfuegen",
                Some("<Control>v"),
                Some("edit-paste"),
            ),
        ],
        vec![
            p(
                N_("Select _All"),
                "win.alles-auswaehlen",
                Some("<Control>a"),
                Some("edit-select-all"),
            ),
            p(
                N_("_Invert Selection"),
                "win.auswahl-umkehren",
                Some("<Shift><Control>i"),
                None,
            ),
        ],
        vec![p(N_("_Rename…"), "win.umbenennen", Some("F2"), None)],
        vec![
            p(
                N_("Mo_ve to Trash"),
                "win.papierkorb",
                Some("Delete"),
                Some("user-trash"),
            ),
            p(
                N_("_Delete…"),
                "win.loeschen",
                Some("<Shift>Delete"),
                Some("edit-delete"),
            ),
            p(N_("_Restore"), "win.wiederherstellen", None, None),
        ],
        vec![p(
            N_("Prefere_nces"),
            "win.einstellungen",
            None,
            Some("preferences-desktop"),
        )],
    ]
}

fn ansicht() -> Vec<Vec<Punkt>> {
    vec![
        vec![
            p(
                N_("_Stop"),
                "win.stopp",
                Some("Escape"),
                Some("process-stop"),
            ),
            p(
                N_("_Reload"),
                "win.aktualisieren",
                Some("<Control>r"),
                Some("view-refresh"),
            ),
        ],
        vec![
            s(N_("_Toolbar"), "win.werkzeugleiste", None),
            s(N_("Side _Pane"), "win.seitenleiste", Some("F9")),
            s(N_("Location _Bar"), "win.adressleiste", None),
            s(N_("St_atusbar"), "win.statusleiste", None),
        ],
        vec![
            s(
                N_("Show _Hidden Files"),
                "win.verborgene",
                Some("<Control>h"),
            ),
            p(N_("Arran_ge Items"), "", None, None),
        ],
        vec![
            p(
                N_("Zoom _In"),
                "win.vergroessern",
                Some("<Control>+"),
                Some("zoom-in"),
            ),
            p(
                N_("Zoom _Out"),
                "win.verkleinern",
                Some("<Control>-"),
                Some("zoom-out"),
            ),
            p(
                N_("Normal Si_ze"),
                "win.normale-groesse",
                Some("<Control>0"),
                Some("zoom-original"),
            ),
        ],
        vec![
            w(N_("_Icons"), "win.ansicht", "symbole", Some("<Control>1")),
            w(N_("_List"), "win.ansicht", "liste", Some("<Control>2")),
            w(N_("_Compact"), "win.ansicht", "kompakt", Some("<Control>3")),
        ],
    ]
}

fn anordnen() -> Vec<Vec<Punkt>> {
    vec![
        vec![
            w(N_("By _Name"), "win.anordnen", "name", None),
            w(N_("By _Size"), "win.anordnen", "groesse", None),
            w(N_("By _Type"), "win.anordnen", "typ", None),
            w(N_("By Modification _Date"), "win.anordnen", "datum", None),
        ],
        vec![s(N_("Re_versed Order"), "win.umgekehrt", None)],
    ]
}

fn gehe_zu() -> Vec<Vec<Punkt>> {
    vec![
        vec![
            p(
                N_("Open _Parent"),
                "win.nach-oben",
                Some("<Alt>↑"),
                Some("go-up"),
            ),
            p(
                N_("_Back"),
                "win.zurueck",
                Some("<Alt>←"),
                Some("go-previous"),
            ),
            p(N_("_Forward"), "win.vor", Some("<Alt>→"), Some("go-next")),
        ],
        vec![
            p(
                N_("_Home Folder"),
                "win.persoenlich",
                Some("<Alt>Home"),
                Some("user-home"),
            ),
            p(N_("_Computer"), "win.rechner", None, Some("computer")),
            p(
                N_("_Trash"),
                "win.papierkorb-zeigen",
                None,
                Some("user-trash"),
            ),
        ],
        vec![p(N_("_Location…"), "win.ort", Some("<Control>l"), None)],
    ]
}

fn hilfe() -> Vec<Vec<Punkt>> {
    vec![
        vec![
            p(
                N_("Online _Help"),
                "win.hilfe",
                Some("F1"),
                Some("help-browser"),
            ),
            p(N_("_Report a Bug"), "win.fehler-melden", None, None),
        ],
        vec![p(N_("_About"), "win.info", None, Some("help-about"))],
    ]
}

/// Whether `aktion` (detailed, `win.` or `app.`) exists and is enabled for
/// the window.
fn aktiv(fenster: &Fenster, aktion: &str) -> bool {
    let (gruppe, name) = aktion.split_once('.').unwrap_or(("win", aktion));
    match gruppe {
        "win" => fenster
            .window()
            .lookup_action(name)
            .is_some_and(|a| a.is_enabled()),
        _ => fenster
            .app()
            .and_then(|a| a.gtk().lookup_action(name))
            .is_some_and(|a| a.is_enabled()),
    }
}

fn zustand(fenster: &Fenster, aktion: &str) -> Option<glib::Variant> {
    let name = aktion.strip_prefix("win.")?;
    fenster.window().lookup_action(name)?.state()
}

/// Fills `menue` from `tabelle` for the window's current state.
fn fuellen(menue: &gio::Menu, tabelle: Vec<Vec<Punkt>>, fenster: &Rc<Fenster>) {
    menue.remove_all();
    for gruppe in tabelle {
        let abschnitt = gio::Menu::new();
        for punkt in gruppe {
            if punkt.aktion.is_empty() {
                let unter = match punkt.text {
                    "Create _Document" => dokument_anlegen_menue(fenster),
                    "Open Wit_h" => oeffnen_mit_menue(&fenster.modell().gewaehlte()),
                    _ => {
                        let m = gio::Menu::new();
                        fuellen(&m, anordnen(), fenster);
                        m
                    }
                };
                let item = gio::MenuItem::new_submenu(
                    Some(&ohne_zugriffstaste(&text(punkt.text))),
                    &unter,
                );
                if let Some(i) = punkt.icon {
                    item.set_icon(&gio::ThemedIcon::new(i));
                }
                abschnitt.append_item(&item);
                continue;
            }
            // entries that only show where they apply (B §3.3)
            let ort = fenster.ort();
            // trash:/// with GVfs, the home trash's files folder without
            let im_papierkorb = ort.as_ref().is_some_and(crate::ziehen::im_papierkorb);
            let werte = fenster
                .app()
                .map(|a| a.konfig().werte())
                .unwrap_or_default();
            match punkt.aktion {
                "win.papierkorb-leeren" | "win.wiederherstellen" if !im_papierkorb => continue,
                "win.loeschen" if !werte.loeschen_anbieten => continue,
                _ => {}
            }
            let an = aktiv(fenster, punkt.aktion);
            let icon: Option<gio::Icon> = match punkt.art {
                Art::Normal => punkt.icon.map(|i| gio::ThemedIcon::new(i).upcast()),
                Art::Schalter => zustand(fenster, punkt.aktion)
                    .and_then(|v| v.get::<bool>())
                    .filter(|b| *b)
                    .map(|_| gio::ThemedIcon::new("object-select-symbolic").upcast()),
                Art::Wahl => zustand(fenster, punkt.aktion)
                    .and_then(|v| v.get::<String>())
                    .filter(|v| Some(v.as_str()) == punkt.ziel)
                    .map(|_| gio::ThemedIcon::new("media-record-symbolic").upcast()),
            };
            let kuerzel = punkt.kuerzel.map(kuerzel_text);
            abschnitt.append_item(&eintrag(
                &text(punkt.text),
                an.then_some(punkt.aktion),
                punkt.ziel,
                kuerzel.as_deref(),
                icon.as_ref(),
            ));
        }
        if abschnitt.n_items() > 0 {
            menue.append_section(None, &abschnitt);
        }
    }
}

/// Bookmarks menu: "Lesezeichen hinzufügen" and the list.
fn lesezeichen_fuellen(menue: &gio::Menu, fenster: &Rc<Fenster>) {
    menue.remove_all();
    let oben = gio::Menu::new();
    oben.append_item(&eintrag(
        &gettext("_Add Bookmark"),
        aktiv(fenster, "win.lesezeichen-hinzufuegen").then_some("win.lesezeichen-hinzufuegen"),
        None,
        Some("<Control>d"),
        Some(&gio::ThemedIcon::new("bookmark-new").upcast()),
    ));
    menue.append_section(None, &oben);
    let liste = gio::Menu::new();
    if let Some(app) = fenster.app() {
        for l in app.lesezeichen().liste() {
            let datei = l.datei();
            let icon: gio::Icon = if datei.is_native() {
                gio::ThemedIcon::new(crate::eintrag::ordnersymbol(&datei)).upcast()
            } else {
                gio::ThemedIcon::new("folder-remote").upcast()
            };
            liste.append_item(&eintrag(
                &l.anzeige().replace('_', "__"),
                Some("win.gehe-zu"),
                Some(&l.uri),
                None,
                Some(&icon),
            ));
        }
    }
    if liste.n_items() > 0 {
        menue.append_section(None, &liste);
    }
}

/// "Dokument anlegen ▸": the templates in `~/Vorlagen` (XDG templates
/// folder, files only, sorted by name), then "Leere Datei"; target of
/// `win.dokument-anlegen` is the template's URI, "" for an empty file.
/// Also for the background context menu (Welle B2).
pub fn dokument_anlegen_menue(fenster: &Fenster) -> gio::Menu {
    let menue = gio::Menu::new();
    let erlaubt = aktiv(fenster, "win.dokument-anlegen");
    let aktion = erlaubt.then_some("win.dokument-anlegen");
    let vorlagen = gio::Menu::new();
    for (name, uri) in vorlagen_liste() {
        let typ = gio::content_type_guess(Some(name.as_str()), None).0;
        let icon = gio::content_type_get_icon(&typ);
        vorlagen.append_item(&eintrag(
            &name.replace('_', "__"),
            aktion,
            Some(&uri),
            None,
            Some(&icon),
        ));
    }
    if vorlagen.n_items() == 0 {
        vorlagen.append_item(&eintrag(
            &gettext("No templates installed"),
            None,
            None,
            None,
            None,
        ));
    }
    menue.append_section(None, &vorlagen);
    let leer = gio::Menu::new();
    leer.append_item(&eintrag(
        &gettext("Empty File"),
        aktion,
        Some(""),
        None,
        Some(&gio::ThemedIcon::new("text-x-generic").upcast()),
    ));
    menue.append_section(None, &leer);
    menue
}

/// Templates as (name without extension, URI), sorted.
pub fn vorlagen_liste() -> Vec<(String, String)> {
    let Some(dir) = glib::user_special_dir(glib::UserDirectory::Templates) else {
        return Vec::new();
    };
    if dir == glib::home_dir() {
        return Vec::new();
    }
    let Ok(lesen) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut liste: Vec<(String, String)> = lesen
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || name.ends_with('~') {
                return None;
            }
            let stamm = querencia_dateiarbeit::namen::split_extension(&name)
                .0
                .to_owned();
            Some((stamm, gio::File::for_path(e.path()).uri().to_string()))
        })
        .collect();
    liste.sort_by(|a, b| {
        glib::FilenameCollationKey::from(a.0.as_str())
            .cmp(&glib::FilenameCollationKey::from(b.0.as_str()))
    });
    liste
}

/// "Öffnen mit ▸": the programs for the type of the selection (only when
/// all selected files share it, B §1.7), target of `win.oeffnen-mit` is the
/// program's ID. Also for the context menus (Welle B2).
pub fn oeffnen_mit_menue(eintraege: &[Eintrag]) -> gio::Menu {
    let menue = gio::Menu::new();
    let Some(erster) = eintraege.first() else {
        return menue;
    };
    let typ = &erster.daten().typ;
    if eintraege.iter().any(|e| &e.daten().typ != typ) {
        return menue;
    }
    for app in querencia_dateiarbeit::oeffnen::apps_for(typ) {
        let Some(id) = app.id() else { continue };
        let icon = app.icon();
        menue.append_item(&eintrag(
            &app.display_name().replace('_', "__"),
            Some("win.oeffnen-mit"),
            Some(&id),
            None,
            icon.as_ref(),
        ));
    }
    menue
}

/// The menu bar of a window.
pub struct Menueleiste {
    leiste: gtk4::Box,
    /// „Reiter“: only while the window has two tabs or more.
    reiter: gtk4::MenuButton,
}

impl Menueleiste {
    pub fn new(fenster: &Rc<Fenster>) -> Self {
        let leiste = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        leiste.add_css_class("querencia-menuleiste");
        type Fueller = fn(&gio::Menu, &Rc<Fenster>);
        let menues: [(&str, Fueller); 7] = [
            (N_("_File"), |m, f| fuellen(m, datei(), f)),
            (N_("_Edit"), |m, f| fuellen(m, bearbeiten(), f)),
            (N_("_View"), |m, f| fuellen(m, ansicht(), f)),
            (N_("_Go"), |m, f| fuellen(m, gehe_zu(), f)),
            (N_("_Bookmarks"), lesezeichen_fuellen),
            (N_("_Tabs"), crate::reiter::menue_fuellen),
            (N_("_Help"), |m, f| fuellen(m, hilfe(), f)),
        ];
        let mut knoepfe = Vec::new();
        for (titel, fueller) in menues {
            let knopf = gtk4::MenuButton::new();
            knopf.set_label(&text(titel));
            knopf.set_use_underline(true);
            knopf.set_always_show_arrow(false);
            knopf.set_has_frame(false);
            knopf.set_can_focus(false);
            knopf.add_css_class("querencia-menuknopf");
            let modell = gio::Menu::new();
            fueller(&modell, fenster);
            classic_menu::attach(&knopf, modell.upcast_ref());
            let weak: Weak<Fenster> = Rc::downgrade(fenster);
            let m = modell.clone();
            knopf.set_create_popup_func(move |_| {
                if let Some(f) = weak.upgrade() {
                    fueller(&m, &f);
                }
            });
            leiste.append(&knopf);
            knoepfe.push(knopf);
        }
        let reiter = knoepfe.swap_remove(5);
        reiter.set_visible(false);
        Self { leiste, reiter }
    }

    /// Shows or hides the menu „Reiter“ (from two tabs on, Nautilus 2.30);
    /// true when that changed something.
    pub fn reiter_zeigen(&self, an: bool) -> bool {
        let anders = self.reiter.is_visible() != an;
        self.reiter.set_visible(an);
        anders
    }

    pub fn widget(&self) -> &gtk4::Box {
        &self.leiste
    }

    /// The menus are rebuilt when they open; nothing to do while closed.
    pub fn zustand_geaendert(&self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn access_keys_are_taken_out_of_entry_labels() {
        assert_eq!(ohne_zugriffstaste("S_chließen"), "Schließen");
        assert_eq!(ohne_zugriffstaste("Ei_genschaften"), "Eigenschaften");
        assert_eq!(ohne_zugriffstaste("a__b"), "a_b");
        assert_eq!(
            ohne_zugriffstaste("Papierkorb _leeren …"),
            "Papierkorb leeren …"
        );
    }

    #[test]
    fn tables_name_every_0_1_entry_with_shortcuts_of_b_3_3() {
        let alle: Vec<Punkt> = [
            datei(),
            bearbeiten(),
            ansicht(),
            anordnen(),
            gehe_zu(),
            hilfe(),
        ]
        .into_iter()
        .flatten()
        .flatten()
        .collect();
        let finde = |t: &str| {
            alle.iter()
                .find(|p| p.text == t)
                .unwrap_or_else(|| panic!("{t} fehlt"))
        };
        assert_eq!(finde("Create _Folder").kuerzel, Some("<Shift><Control>n"));
        assert_eq!(finde("_Properties").kuerzel, Some("<Alt>Return"));
        assert_eq!(finde("_Delete…").kuerzel, Some("<Shift>Delete"));
        assert_eq!(finde("Side _Pane").kuerzel, Some("F9"));
        assert_eq!(finde("_List").kuerzel, Some("<Control>2"));
        assert_eq!(finde("_Location…").kuerzel, Some("<Control>l"));
        assert_eq!(finde("Online _Help").kuerzel, Some("F1"));
        // Welle C3: tabs and the compact view (B §3.3)
        assert_eq!(finde("New _Tab").kuerzel, Some("<Control>t"));
        assert_eq!(finde("Open in New _Tab").kuerzel, Some("<Shift><Control>o"));
        assert_eq!(finde("_Compact").kuerzel, Some("<Control>3"));
        assert_eq!(finde("_Compact").ziel, Some("kompakt"));
        // no other B-stage entries yet (B §3.3)
        for b in ["Connect to _Server…", "_Undo", "D_uplicate"] {
            assert!(!alle.iter().any(|p| p.text == b), "{b} ist Beta");
        }
    }

    #[test]
    fn shortcuts_of_the_view_area_and_the_application_do_not_overlap() {
        // GTK notation without GTK (no display in tests): modifiers, then a key name
        let lesbar = |t: &str| {
            let mut rest = t;
            while let Some(r) = ["<Control>", "<Shift>", "<Alt>"]
                .iter()
                .find_map(|m| rest.strip_prefix(m))
            {
                rest = r;
            }
            !rest.is_empty() && rest.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        };
        for (aktion, tasten) in KUERZEL_IM_BEREICH {
            for t in *tasten {
                assert!(
                    !KUERZEL.iter().any(|(_, k)| k.contains(t)),
                    "{t} ({aktion}) doppelt"
                );
                assert!(lesbar(t), "{t} unlesbar");
            }
        }
        for (_, tasten) in KUERZEL {
            for t in *tasten {
                assert!(lesbar(t), "{t} unlesbar");
            }
        }
    }
}
