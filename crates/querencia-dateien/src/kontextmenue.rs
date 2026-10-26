//! The context menus of the views after Nautilus 2.30
//! (`nautilus-directory-view-ui.xml`, B §1.7 level 0.1): one file, one
//! folder, several objects, an object in the trash, and the background.
//!
//! The menus only name window actions (`win.*`); what they do lies with
//! the window and the operations (B3, operationen.rs). An entry whose
//! action the window does not have, or has disabled, is shown
//! insensitive (classic_menu draws entries without action that way).
//! [`zeilen`] is pure and tested; [`als_menue`] turns its rows into the
//! `gio::Menu` the classic menu shows.

use std::cell::RefCell;

use gtk4::prelude::*;
use gtk4::{gdk, gio, glib};

use querencia_oberflaeche::classic_menu;

use crate::ansicht::Kontextziel;
use crate::ansichtskern::{Eintrag, Kriterium, Sortierung, protokoll};
use crate::app::Register;
use crate::i18n::{gettext, gettext_f};

/// What was clicked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ort {
    /// Free space of the view (the selection was cleared).
    Hintergrund,
    /// The selection, at least one object.
    Auswahl(Auswahlinfo),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Auswahlinfo {
    pub anzahl: usize,
    pub ordner: usize,
    /// The content type all selected files share (`None` when they
    /// differ or a folder is among them).
    pub gemeinsamer_typ: Option<String>,
}

/// A program for „Öffnen mit“.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Programm {
    pub id: String,
    pub name: String,
}

/// What the menu needs to know besides the click.
#[derive(Clone, Debug, Default)]
pub struct Umgebung {
    /// The folder shown is the trash (`trash:///`).
    pub im_papierkorb: bool,
    /// Default program first, then the recommended ones.
    pub programme: Vec<Programm>,
    /// Templates as (shown name, URI).
    pub vorlagen: Vec<(String, String)>,
    pub sortierung: Sortierung,
}

/// One row of a menu group.
#[derive(Clone, Debug, PartialEq)]
pub enum Zeile {
    Eintrag {
        text: String,
        aktion: Option<&'static str>,
        ziel: Option<String>,
        /// Shown with a check mark (current sort order).
        haken: bool,
    },
    Unter {
        text: String,
        gruppen: Vec<Vec<Zeile>>,
    },
}

/// A translated menu text without its access key mark (the classic menus
/// show none; the msgids are the menu bar's, B §3.3).
fn t(text: String) -> String {
    crate::menue::ohne_zugriffstaste(&text)
}

fn eintrag(text: String, aktion: &'static str) -> Zeile {
    Zeile::Eintrag {
        text,
        aktion: Some(aktion),
        ziel: None,
        haken: false,
    }
}

fn gesperrt(text: String) -> Zeile {
    Zeile::Eintrag {
        text,
        aktion: None,
        ziel: None,
        haken: false,
    }
}

fn mit_ziel(text: String, aktion: &'static str, ziel: &str) -> Zeile {
    Zeile::Eintrag {
        text,
        aktion: Some(aktion),
        ziel: Some(ziel.to_owned()),
        haken: false,
    }
}

/// The groups (separated by lines) of the menu for `ort`.
pub fn zeilen(ort: &Ort, u: &Umgebung) -> Vec<Vec<Zeile>> {
    match ort {
        Ort::Hintergrund => hintergrund(u),
        Ort::Auswahl(a) if u.im_papierkorb => papierkorb_objekt(a),
        Ort::Auswahl(a) => objekte(a, u),
    }
}

fn objekte(a: &Auswahlinfo, u: &Umgebung) -> Vec<Vec<Zeile>> {
    let nur_ordner = a.ordner == a.anzahl;
    let mut oeffnen = Vec::new();
    match (&a.gemeinsamer_typ, u.programme.first()) {
        (Some(_), Some(standard)) if a.ordner == 0 => {
            oeffnen.push(eintrag(
                gettext_f("Open with “{app}”", &[("app", &standard.name)]),
                "win.oeffnen",
            ));
        }
        _ => oeffnen.push(eintrag(t(gettext("_Open")), "win.oeffnen")),
    }
    if nur_ordner {
        oeffnen.push(eintrag(
            t(gettext("Open in New _Tab")),
            "win.oeffnen-neuer-reiter",
        ));
        oeffnen.push(eintrag(
            t(gettext("Open in New _Window")),
            "win.oeffnen-neues-fenster",
        ));
    }
    // „Öffnen mit“ only where one program list fits all (Nautilus: same
    // type), without the program „Öffnen“ already names; „Mit anderer
    // Anwendung öffnen …“ (the chooser) comes with the Beta (B21).
    if a.gemeinsamer_typ.is_some() {
        let ohne_standard = usize::from(a.ordner == 0);
        let andere: Vec<Zeile> = u
            .programme
            .iter()
            .skip(ohne_standard)
            .map(|p| mit_ziel(p.name.clone(), "win.oeffnen-mit", &p.id))
            .collect();
        if !andere.is_empty() {
            oeffnen.push(Zeile::Unter {
                text: t(gettext("Open Wit_h")),
                gruppen: vec![andere],
            });
        }
    }
    let mut ablage = vec![
        eintrag(t(gettext("Cu_t")), "win.ausschneiden"),
        eintrag(t(gettext("_Copy")), "win.kopieren"),
    ];
    if a.anzahl == 1 && a.ordner == 1 {
        ablage.push(eintrag(
            t(gettext("Paste Into Folder")),
            "win.in-ordner-einfuegen",
        ));
    }
    let umbenennen = if a.anzahl == 1 {
        eintrag(t(gettext("_Rename…")), "win.umbenennen")
    } else {
        gesperrt(t(gettext("_Rename…")))
    };
    vec![
        oeffnen,
        ablage,
        vec![umbenennen],
        vec![eintrag(t(gettext("Mo_ve to Trash")), "win.papierkorb")],
        vec![eintrag(t(gettext("_Properties")), "win.eigenschaften")],
    ]
}

fn papierkorb_objekt(_a: &Auswahlinfo) -> Vec<Vec<Zeile>> {
    vec![
        vec![eintrag(t(gettext("_Restore")), "win.wiederherstellen")],
        vec![eintrag(t(gettext("_Delete…")), "win.loeschen")],
        vec![eintrag(t(gettext("_Properties")), "win.eigenschaften")],
    ]
}

fn hintergrund(u: &Umgebung) -> Vec<Vec<Zeile>> {
    let mut gruppen = Vec::new();
    if !u.im_papierkorb {
        let mut dokumente = Vec::new();
        if u.vorlagen.is_empty() {
            dokumente.push(vec![gesperrt(t(gettext("No templates installed")))]);
        } else {
            dokumente.push(
                u.vorlagen
                    .iter()
                    .map(|(name, uri)| mit_ziel(name.clone(), "win.dokument-anlegen", uri))
                    .collect(),
            );
        }
        dokumente.push(vec![mit_ziel(
            t(gettext("Empty File")),
            "win.dokument-anlegen",
            "",
        )]);
        gruppen.push(vec![
            eintrag(t(gettext("Create _Folder")), "win.ordner-anlegen"),
            Zeile::Unter {
                text: t(gettext("Create _Document")),
                gruppen: dokumente,
            },
        ]);
    }
    let s = u.sortierung;
    let kriterien = Kriterium::ALLE
        .into_iter()
        .map(|k| Zeile::Eintrag {
            text: match k {
                Kriterium::Name => t(gettext("By _Name")),
                Kriterium::Groesse => t(gettext("By _Size")),
                Kriterium::Typ => t(gettext("By _Type")),
                Kriterium::Datum => t(gettext("By Modification _Date")),
            },
            aktion: Some("win.anordnen"),
            ziel: Some(k.kennung().to_owned()),
            haken: s.kriterium == k,
        })
        .collect();
    gruppen.push(vec![Zeile::Unter {
        text: t(gettext("Arran_ge Items")),
        gruppen: vec![
            kriterien,
            vec![Zeile::Eintrag {
                text: t(gettext("Re_versed Order")),
                aktion: Some("win.umgekehrt"),
                ziel: None,
                haken: s.umgekehrt,
            }],
        ],
    }]);
    if !u.im_papierkorb {
        gruppen.push(vec![eintrag(t(gettext("_Paste")), "win.einfuegen")]);
    }
    gruppen.push(vec![
        eintrag(t(gettext("Zoom _In")), "win.vergroessern"),
        eintrag(t(gettext("Zoom _Out")), "win.verkleinern"),
        eintrag(t(gettext("Normal Si_ze")), "win.normale-groesse"),
    ]);
    gruppen.push(vec![eintrag(
        t(gettext("_Properties")),
        "win.eigenschaften",
    )]);
    gruppen
}

thread_local! {
    /// The anchors of the views with the menu each one shows.
    static ANKER: RefCell<Vec<(glib::WeakRef<gtk4::MenuButton>, gio::Menu)>> =
        const { RefCell::new(Vec::new()) };
}

/// Puts the anchor of the context menus into a view's overlay: an empty,
/// untargetable menu button at its origin whose classic menu points at the
/// click (desktop pattern, `desktop_view.rs`).
pub fn anker_anlegen(overlay: &gtk4::Overlay) {
    let anker = gtk4::MenuButton::new();
    anker.set_child(Some(&gtk4::Box::new(gtk4::Orientation::Horizontal, 0)));
    anker.add_css_class("menue-anker");
    anker.set_can_target(false);
    // stays focusable: an unfocusable menu button makes GTK report a
    // critical each time its menu opens (desktop)
    anker.set_focus_on_click(false);
    anker.set_halign(gtk4::Align::Start);
    anker.set_valign(gtk4::Align::Start);
    overlay.add_overlay(&anker);
    // Esc or a chosen entry leaves the focus on the anchor; typing then
    // went nowhere (prototype §5 A10). Back to the list of the view.
    let fuer_ende = overlay.downgrade();
    anker.connect_active_notify(move |a| {
        if a.is_active() {
            return;
        }
        let Some(overlay) = fuer_ende.upgrade() else {
            return;
        };
        let liste = overlay
            .child()
            .and_downcast::<gtk4::ScrolledWindow>()
            .and_then(|s| s.child());
        let Some(liste) = liste else {
            return;
        };
        // after GTK has moved the focus at the end of the popup
        let anker = a.clone();
        glib::idle_add_local_once(move || {
            let fokus = anker.root().and_then(|r| r.focus());
            let verloren = fokus
                .as_ref()
                .is_none_or(|f| f == anker.upcast_ref::<gtk4::Widget>() || f.is_ancestor(&anker));
            if verloren {
                liste.grab_focus();
            }
        });
    });
    let menue = gio::Menu::new();
    classic_menu::attach_popup(&anker, menue.upcast_ref());
    ANKER.with(|a| {
        let mut a = a.borrow_mut();
        a.retain(|(k, _)| k.upgrade().is_some());
        a.push((anker.downgrade(), menue));
    });
}

/// The anchor next to `widget` (in its overlay) and its menu.
fn anker_bei(widget: &gtk4::Widget) -> Option<(gtk4::MenuButton, gio::Menu)> {
    let mut w = Some(widget.clone());
    while let Some(kandidat) = w {
        let mut kind = kandidat.first_child();
        while let Some(k) = kind {
            if k.has_css_class("menue-anker")
                && let Ok(knopf) = k.clone().downcast::<gtk4::MenuButton>()
            {
                let menue = ANKER.with(|a| {
                    a.borrow()
                        .iter()
                        .find(|(b, _)| b.upgrade().as_ref() == Some(&knopf))
                        .map(|(_, m)| m.clone())
                });
                return menue.map(|m| (knopf, m));
            }
            kind = k.next_sibling();
        }
        w = kandidat.parent();
    }
    None
}

/// What the menu shows for `gewaehlt` (empty: the background).
pub fn ort_fuer(gewaehlt: &[Eintrag]) -> Ort {
    let Some(erster) = gewaehlt.first() else {
        return Ort::Hintergrund;
    };
    let typ = erster.daten().typ.clone();
    let gleich = gewaehlt.iter().all(|e| e.daten().typ == typ);
    Ort::Auswahl(Auswahlinfo {
        anzahl: gewaehlt.len(),
        ordner: gewaehlt.iter().filter(|e| e.daten().ist_ordner).count(),
        gemeinsamer_typ: gleich.then_some(typ),
    })
}

/// Shows the context menu for `gewaehlt` (empty: the background of
/// `ordner`) at (x, y) in `widget`, a view's list widget.
pub fn zeigen(
    widget: &gtk4::Widget,
    x: f64,
    y: f64,
    gewaehlt: &[Eintrag],
    ordner: Option<&gio::File>,
    sortierung: Sortierung,
) {
    let Some((anker, menue)) = anker_bei(widget) else {
        protokoll("Kontextmenü: kein Anker neben der Ansicht");
        return;
    };
    let ort = ort_fuer(gewaehlt);
    let programme = match &ort {
        Ort::Auswahl(Auswahlinfo {
            gemeinsamer_typ: Some(t),
            ..
        }) => programme_fuer(t),
        _ => Vec::new(),
    };
    let vorlagen = if ort == Ort::Hintergrund {
        crate::menue::vorlagen_liste()
    } else {
        Vec::new()
    };
    let umgebung = Umgebung {
        im_papierkorb: ordner.is_some_and(|o| o.has_uri_scheme("trash")),
        programme,
        vorlagen,
        sortierung,
    };
    let gruppen = zeilen(&ort, &umgebung);
    let modell = als_menue(&gruppen, &|a| aktion_verfuegbar(widget, a));
    menue.remove_all();
    for i in 0..modell.n_items() {
        if let Some(abschnitt) = modell.item_link(i, "section") {
            menue.append_section(None, &abschnitt);
        }
    }
    protokoll(&format!("Kontextmenü: {}", protokoll_zeilen(&menue)));
    // the anchor sits at the overlay's origin; the popover points at the
    // click in the overlay's coordinates
    let basis = anker.parent().unwrap_or_else(|| widget.clone());
    let punkt = widget
        .compute_point(&basis, &gtk4::graphene::Point::new(x as f32, y as f32))
        .map_or((x, y), |p| (f64::from(p.x()), f64::from(p.y())));
    if let Some(popover) = anker.popover() {
        popover.set_pointing_to(Some(&gdk::Rectangle::new(
            punkt.0 as i32,
            punkt.1 as i32,
            1,
            1,
        )));
    }
    anker.popup();
}

/// The menu model of `gruppen`; `verfuegbar("win.x")` tells whether the
/// window can run the action now (otherwise the entry is insensitive).
pub fn als_menue(gruppen: &[Vec<Zeile>], verfuegbar: &dyn Fn(&str) -> bool) -> gio::Menu {
    let menue = gio::Menu::new();
    for gruppe in gruppen {
        let abschnitt = gio::Menu::new();
        for zeile in gruppe {
            match zeile {
                Zeile::Eintrag {
                    text,
                    aktion,
                    ziel,
                    haken,
                } => {
                    let item = gio::MenuItem::new(Some(text), None);
                    if let Some(aktion) = aktion.filter(|a| verfuegbar(a)) {
                        let ziel = ziel.as_ref().map(|z| z.to_variant());
                        item.set_action_and_target_value(Some(aktion), ziel.as_ref());
                    }
                    if *haken {
                        // the check mark of GNOME 2's radio items, as the
                        // desktop's „Am Raster ausrichten“
                        item.set_icon(&gio::ThemedIcon::from_names(&[
                            "object-select-symbolic",
                            "emblem-ok",
                        ]));
                    }
                    abschnitt.append_item(&item);
                }
                Zeile::Unter { text, gruppen } => {
                    abschnitt.append_submenu(Some(text), &als_menue(gruppen, verfuegbar));
                }
            }
        }
        menue.append_section(None, &abschnitt);
    }
    menue
}

/// Whether the window (or the application) of `widget` has the action
/// `name` (`win.x`/`app.x`) and it is enabled.
pub fn aktion_verfuegbar(widget: &impl IsA<gtk4::Widget>, name: &str) -> bool {
    let Some((gruppe, kurz)) = name.split_once('.') else {
        return false;
    };
    let fenster = widget
        .as_ref()
        .root()
        .and_downcast::<gtk4::ApplicationWindow>();
    match gruppe {
        "win" => fenster.is_some_and(|f| f.has_action(kurz) && f.is_action_enabled(kurz)),
        "app" => fenster
            .and_then(|f| f.application())
            .is_some_and(|a| a.has_action(kurz) && a.is_action_enabled(kurz)),
        _ => false,
    }
}

/// The programs for `typ`: the default first, then the recommended and
/// the other ones shown in menus (the frame's „Öffnen mit“ uses the same
/// list, `querencia_dateiarbeit::oeffnen::apps_for`).
pub fn programme_fuer(typ: &str) -> Vec<Programm> {
    querencia_dateiarbeit::oeffnen::apps_for(typ)
        .into_iter()
        .filter_map(|app| {
            Some(Programm {
                id: app.id()?.to_string(),
                name: app.display_name().to_string(),
            })
        })
        .collect()
}

/// Registers the context menus of the views with the frame (B1's hook:
/// the window passes the target, the list widget and the point).
pub fn registrieren(register: &mut Register) {
    register.kontextmenue(|fenster, ziel, widget, x, y| {
        let modell = fenster.modell();
        let gewaehlt = match ziel {
            Kontextziel::Auswahl => modell.gewaehlte(),
            Kontextziel::Hintergrund => Vec::new(),
        };
        zeigen(
            widget,
            x,
            y,
            &gewaehlt,
            fenster.ort().as_ref(),
            modell.sortierung(),
        );
    });
}

#[cfg(test)]
/// Texts of `gruppen` with "|" between groups, for the logs and tests:
/// insensitive entries in brackets, checked ones with "*", submenus with
/// "▸" and their content in braces.
pub fn beschreibung(gruppen: &[Vec<Zeile>]) -> String {
    gruppen
        .iter()
        .map(|g| {
            g.iter()
                .map(|z| match z {
                    Zeile::Eintrag {
                        text,
                        aktion,
                        haken,
                        ..
                    } => {
                        let t = if *haken {
                            format!("*{text}")
                        } else {
                            text.clone()
                        };
                        if aktion.is_some() {
                            t
                        } else {
                            format!("[{t}]")
                        }
                    }
                    Zeile::Unter { text, gruppen } => {
                        format!("{text} ▸ {{{}}}", beschreibung(gruppen))
                    }
                })
                .collect::<Vec<_>>()
                .join(", ")
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

/// The menu as shown: rows with their actions, for the debug log.
pub fn protokoll_zeilen(menue: &gio::Menu) -> String {
    fn zeilen_von(m: &gio::MenuModel, aus: &mut Vec<String>) {
        for i in 0..m.n_items() {
            if let Some(abschnitt) = m.item_link(i, "section") {
                zeilen_von(&abschnitt, aus);
                aus.push("|".into());
                continue;
            }
            let text = m
                .item_attribute_value(i, "label", Some(glib::VariantTy::STRING))
                .and_then(|v| v.get::<String>())
                .unwrap_or_default();
            let aktiv = m.item_attribute_value(i, "action", None).is_some();
            if m.item_link(i, "submenu").is_some() {
                aus.push(format!("{text} ▸"));
            } else if aktiv {
                aus.push(text);
            } else {
                aus.push(format!("[{text}]"));
            }
        }
    }
    let mut aus = Vec::new();
    zeilen_von(menue.upcast_ref(), &mut aus);
    while aus.last().is_some_and(|z| z == "|") {
        aus.pop();
    }
    aus.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn umgebung() -> Umgebung {
        Umgebung {
            programme: vec![
                Programm {
                    id: "org.querencia.Editor.desktop".into(),
                    name: "Editor".into(),
                },
                Programm {
                    id: "org.querencia.Dokumente.desktop".into(),
                    name: "Dokumente".into(),
                },
            ],
            vorlagen: vec![("Brief".into(), "file:///v/Brief.odt".into())],
            ..Umgebung::default()
        }
    }

    fn datei() -> Ort {
        Ort::Auswahl(Auswahlinfo {
            anzahl: 1,
            ordner: 0,
            gemeinsamer_typ: Some("text/plain".into()),
        })
    }

    #[test]
    fn file_menu_after_nautilus() {
        assert_eq!(
            beschreibung(&zeilen(&datei(), &umgebung())),
            "Open with “Editor”, Open With ▸ {Dokumente} \
             | Cut, Copy | Rename… | Move to Trash | Properties"
        );
    }

    #[test]
    fn folder_menu_has_new_window_and_paste_into() {
        let ordner = Ort::Auswahl(Auswahlinfo {
            anzahl: 1,
            ordner: 1,
            gemeinsamer_typ: Some("inode/directory".into()),
        });
        let mut u = umgebung();
        u.programme.truncate(1);
        assert_eq!(
            beschreibung(&zeilen(&ordner, &u)),
            "Open, Open in New Tab, Open in New Window, Open With ▸ {Editor} \
             | Cut, Copy, Paste Into Folder | Rename… | Move to Trash | Properties"
        );
    }

    #[test]
    fn several_objects_rename_is_insensitive_open_with_only_same_type() {
        let gemischt = Ort::Auswahl(Auswahlinfo {
            anzahl: 3,
            ordner: 1,
            gemeinsamer_typ: None,
        });
        assert_eq!(
            beschreibung(&zeilen(&gemischt, &umgebung())),
            "Open | Cut, Copy | [Rename…] | Move to Trash | Properties"
        );
        let gleich = Ort::Auswahl(Auswahlinfo {
            anzahl: 2,
            ordner: 0,
            gemeinsamer_typ: Some("text/plain".into()),
        });
        let text = beschreibung(&zeilen(&gleich, &umgebung()));
        assert!(
            text.starts_with("Open with “Editor”, Open With ▸"),
            "{text}"
        );
        assert!(text.contains("[Rename…]"));
        assert!(!text.contains("Paste Into Folder"));
    }

    #[test]
    fn file_without_other_programs_has_no_open_with() {
        let mut u = umgebung();
        u.programme.truncate(1);
        assert_eq!(
            beschreibung(&zeilen(&datei(), &u)),
            "Open with “Editor” | Cut, Copy | Rename… | Move to Trash | Properties"
        );
        u.programme.clear();
        assert!(beschreibung(&zeilen(&datei(), &u)).starts_with("Open | Cut"));
    }

    #[test]
    fn background_menu_with_templates_and_sort_marks() {
        let mut u = umgebung();
        u.sortierung = Sortierung {
            kriterium: Kriterium::Groesse,
            umgekehrt: true,
            ..Sortierung::default()
        };
        assert_eq!(
            beschreibung(&zeilen(&Ort::Hintergrund, &u)),
            "Create Folder, Create Document ▸ {Brief | Empty File} \
             | Arrange Items ▸ {By Name, *By Size, By Type, By Modification Date | *Reversed Order} \
             | Paste | Zoom In, Zoom Out, Normal Size | Properties"
        );
        u.vorlagen.clear();
        let text = beschreibung(&zeilen(&Ort::Hintergrund, &u));
        assert!(
            text.contains("Create Document ▸ {[No templates installed] | Empty File}"),
            "{text}"
        );
    }

    #[test]
    fn trash_menus() {
        let mut u = umgebung();
        u.im_papierkorb = true;
        assert_eq!(
            beschreibung(&zeilen(&datei(), &u)),
            "Restore | Delete… | Properties"
        );
        let text = beschreibung(&zeilen(&Ort::Hintergrund, &u));
        assert!(
            !text.contains("Create Folder") && !text.contains("Paste |"),
            "{text}"
        );
    }
}
