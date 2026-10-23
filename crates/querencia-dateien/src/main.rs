//! Querencia Dateien – provisional test window of the views worker (B2):
//! one folder in the icon or list view with a View menu, until B1's frame
//! (tag `dateien/geruest-1`) replaces this file. The operations of the
//! context menus only log their action here.
//!
//! Usage: querencia-dateien [FOLDER]

mod ansichtskern;
mod ansichtsregeln;
mod i18n;
mod kontextmenue;
mod listenansicht;
mod symbolansicht;
mod tippsuche;
mod umbenennen;
mod vorlauf_modell;

use std::rc::Rc;
use std::time::Instant;

use gtk4::prelude::*;
use gtk4::{gdk, gio, glib};

use querencia_dateiarbeit::namen;
use querencia_oberflaeche::{classic_menu, farbschema};

use ansichtskern::{Kern, Kriterium, Sortierung, protokoll};
use ansichtsregeln::{Spalte, Zusatz};
use i18n::gettext;
use listenansicht::Listenansicht;
use symbolansicht::Symbolansicht;
use vorlauf_modell::{ATTRIBUTE, Eintrag, Modell};

const APP_ID: &str = "org.querencia.Dateien";

const MENU_CSS: &str = "\
popover.querencia-menu { padding: 0; }
popover.querencia-menu > contents { padding: 2px 0; border: 1px solid #c7c2be; border-radius: 0; box-shadow: none; background-color: #f5f4f2; color: #3c3c3c; }
popover.querencia-menu .qmenu { min-width: 150px; }
popover.querencia-menu .qmenu-item { min-height: 20px; padding: 1px 10px 1px 4px; border: 1px solid transparent; outline: none; }
popover.querencia-menu .qmenu-item .qmenu-arrow { color: #686463; }
popover.querencia-menu .qmenu-item.qmenu-active { background-color: #f07746; background-image: linear-gradient(to bottom, #f8875d, #ea703d); border-color: #b36c4e; box-shadow: inset 0 1px #fd9975; color: #ffffff; }
popover.querencia-menu .qmenu-item.qmenu-active .qmenu-arrow { color: #ffffff; }
popover.querencia-menu .qmenu-item:disabled { color: @insensitive_fg_color; }
popover.querencia-menu separator { margin: 3px 1px; min-height: 1px; background-color: #dcd8d3; }
@media (prefers-color-scheme: dark) {
  popover.querencia-menu > contents { border-color: #2b2a27; background-color: #353430; color: #dfdbd2; }
  popover.querencia-menu .qmenu-item .qmenu-arrow { color: #a5a09a; }
  popover.querencia-menu separator { background-color: #2b2a27; }
}
.menueleiste menubutton > button { padding: 2px 6px; }
";

fn main() -> glib::ExitCode {
    i18n::init();
    let app = gtk4::Application::new(Some(APP_ID), gio::ApplicationFlags::HANDLES_OPEN);
    app.connect_startup(|_| {
        let provider = gtk4::CssProvider::new();
        farbschema::folgen(&provider, ansichtskern::PROGRAMM);
        provider.load_from_data(&format!("{MENU_CSS}{}", ansichtskern::STIL));
        if let Some(display) = gdk::Display::default() {
            gtk4::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
        farbschema::startfehler_beheben();
    });
    app.connect_activate(|app| Probe::fenster(app, gio::File::for_path(glib::home_dir())));
    app.connect_open(|app, dateien, _| {
        if let Some(ordner) = dateien.first() {
            Probe::fenster(app, ordner.clone());
        }
    });
    let tasten: &[(&str, &[&str])] = &[
        ("win.ansicht::symbole", &["<Control>1"]),
        ("win.ansicht::liste", &["<Control>2"]),
        (
            "win.vergroessern",
            &["<Control>plus", "<Control>equal", "<Control>KP_Add"],
        ),
        (
            "win.verkleinern",
            &["<Control>minus", "<Control>KP_Subtract"],
        ),
        ("win.normale-groesse", &["<Control>0", "<Control>KP_0"]),
        ("win.verborgene", &["<Control>h"]),
        ("win.neu-laden", &["<Control>r", "F5"]),
        ("win.umbenennen", &["F2"]),
        ("win.neuer-ordner", &["<Shift><Control>n"]),
        ("win.alles-auswaehlen", &["<Control>a"]),
        ("win.papierkorb", &["Delete"]),
        ("win.schliessen", &["<Control>w", "<Control>q"]),
    ];
    for (aktion, kuerzel) in tasten {
        app.set_accels_for_action(aktion, kuerzel);
    }
    app.run()
}

struct Probe {
    fenster: gtk4::ApplicationWindow,
    modell: Rc<Modell>,
    symbole: Rc<Symbolansicht>,
    liste: Rc<Listenansicht>,
    stapel: gtk4::Stack,
}

impl Probe {
    fn fenster(app: &gtk4::Application, ordner: gio::File) {
        let fenster = gtk4::ApplicationWindow::new(app);
        fenster.set_default_size(800, 550);
        let modell = Modell::new();
        let symbole = Symbolansicht::new(&modell.auswahl);
        let liste = Listenansicht::new(&modell.auswahl);
        let stapel = gtk4::Stack::new();
        stapel.add_named(&symbole.widget(), Some("symbole"));
        stapel.add_named(&liste.widget(), Some("liste"));
        let leiste = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        leiste.add_css_class("menueleiste");
        let knopf = gtk4::MenuButton::new();
        knopf.set_label(&gettext("_View").replace('_', ""));
        knopf.set_has_frame(false);
        classic_menu::attach(&knopf, Self::menue().upcast_ref());
        leiste.append(&knopf);
        let inhalt = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        inhalt.append(&leiste);
        inhalt.append(&stapel);
        fenster.set_child(Some(&inhalt));
        let probe = Rc::new(Self {
            fenster,
            modell,
            symbole,
            liste,
            stapel,
        });
        probe.verbinden();
        probe.aktionen();
        probe.einstellungen_aus_umgebung();
        if std::env::var("QUERENCIA_DATEIEN_ANSICHT").as_deref() == Ok("liste") {
            probe.ansicht_waehlen("liste");
        }
        probe.oeffne(ordner, None);
        probe.fenster.present();
    }

    /// The settings B1's dialog will write (`dateien.ini`), from the
    /// environment of the isolated checks.
    fn einstellungen_aus_umgebung(self: &Rc<Self>) {
        let wert = |n: &str| std::env::var(n).ok().filter(|v| !v.is_empty());
        if let Some(z) = wert("QUERENCIA_DATEIEN_ZUSATZ") {
            let z: Vec<Zusatz> = z.split(',').filter_map(Zusatz::aus_kennung).collect();
            if let [a, b] = z[..] {
                self.symbole.zusatz_setzen([a, b]);
            }
        }
        if let Some(s) = wert("QUERENCIA_DATEIEN_SPALTEN") {
            let s: Vec<Spalte> = s.split(',').filter_map(Spalte::aus_kennung).collect();
            self.liste.spalten_setzen(&s);
        }
        if wert("QUERENCIA_DATEIEN_DATUM").as_deref() == Some("iso") {
            self.symbole.datumsformat_setzen(true);
            self.liste.datumsformat_setzen(true);
        }
        if let Some(i) = wert("QUERENCIA_DATEIEN_ZOOM")
            .and_then(|z| z.parse().ok())
            .and_then(ansichtsregeln::zoom_index)
        {
            self.symbole.zoom_setzen(i);
        }
    }

    fn menue() -> gio::Menu {
        let m = gio::Menu::new();
        let a = gio::Menu::new();
        a.append(Some(&gettext("Zoom In")), Some("win.vergroessern"));
        a.append(Some(&gettext("Zoom Out")), Some("win.verkleinern"));
        a.append(Some(&gettext("Normal Size")), Some("win.normale-groesse"));
        m.append_section(None, &a);
        let b = gio::Menu::new();
        b.append(Some(&gettext("Icons")), Some("win.ansicht::symbole"));
        b.append(Some(&gettext("List")), Some("win.ansicht::liste"));
        m.append_section(None, &b);
        m
    }

    fn kern(&self) -> Rc<Kern> {
        if self.stapel.visible_child_name().as_deref() == Some("liste") {
            self.liste.kern.clone()
        } else {
            self.symbole.kern.clone()
        }
    }

    fn verbinden(self: &Rc<Self>) {
        for kern in [self.symbole.kern.clone(), self.liste.kern.clone()] {
            let weak = Rc::downgrade(self);
            *kern.rueckrufe.aktiviert.borrow_mut() = Some(Rc::new(move |liste: Vec<Eintrag>| {
                let Some(p) = weak.upgrade() else { return };
                let namen: Vec<&str> = liste.iter().map(|e| e.daten().name.as_str()).collect();
                protokoll(&format!("aktiviert: {}", namen.join(", ")));
                if let [e] = liste.as_slice()
                    && e.daten().ist_ordner
                {
                    p.oeffne(e.daten().datei.clone(), None);
                }
            }));
            let weak = Rc::downgrade(self);
            *kern.rueckrufe.nach_oben.borrow_mut() = Some(Rc::new(move |()| {
                let Some(p) = weak.upgrade() else { return };
                let Some(ordner) = p.modell.ordner() else {
                    return;
                };
                if let Some(eltern) = ordner.parent() {
                    let name = ordner.basename().map(|n| n.to_string_lossy().into_owned());
                    p.oeffne(eltern, name);
                }
            }));
            *kern.rueckrufe.ortsfeld.borrow_mut() = Some(Rc::new(|c: char| {
                protokoll(&format!("Ortsfeld mit „{c}“"));
            }));
            let weak = Rc::downgrade(self);
            *kern.rueckrufe.auswahl.borrow_mut() = Some(Rc::new(move |()| {
                if let Some(p) = weak.upgrade() {
                    let k = p.kern();
                    protokoll(&format!(
                        "Auswahl: {}",
                        k.gewaehlte()
                            .iter()
                            .map(|e| e.daten().name.clone())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
            }));
            let weak = Rc::downgrade(self);
            *kern.rueckrufe.umbenannt.borrow_mut() =
                Some(Rc::new(move |(alt, datei): (Eintrag, gio::File)| {
                    if let Some(p) = weak.upgrade() {
                        p.neu_einlesen(Some(&alt), &datei);
                    }
                }));
            let weak = Rc::downgrade(self);
            *kern.rueckrufe.umgebung.borrow_mut() = Some(Rc::new(move || {
                weak.upgrade().map_or((None, Sortierung::default()), |p| {
                    (p.modell.ordner(), p.modell.sortierung())
                })
            }));
        }
        let modell = Rc::downgrade(&self.modell);
        self.liste.bei_sortierklick(move |s| {
            if let Some(m) = modell.upgrade() {
                m.sortierung_setzen(s);
            }
        });
        let weak = Rc::downgrade(self);
        self.modell.bei_sortierung(move |s| {
            if let Some(p) = weak.upgrade() {
                p.liste.sortierung_zeigen(s);
                if let Some(a) = p.fenster.lookup_action("anordnen") {
                    a.change_state(&s.kriterium.kennung().to_variant());
                }
            }
        });
    }

    /// Reads the entry of `datei` again (after a rename or a new folder).
    fn neu_einlesen(self: &Rc<Self>, alt: Option<&Eintrag>, datei: &gio::File) {
        let Some(ordner) = self.modell.ordner() else {
            return;
        };
        let Ok(info) = datei.query_info(
            ATTRIBUTE,
            gio::FileQueryInfoFlags::NONE,
            gio::Cancellable::NONE,
        ) else {
            return;
        };
        let neu = Eintrag::aus_info(&ordner, &info);
        match alt {
            Some(alt) => self.modell.ersetzen(alt, neu),
            None => self.modell.hinzufuegen(neu),
        }
    }

    fn oeffne(self: &Rc<Self>, ordner: gio::File, fokus: Option<String>) {
        let t0 = Instant::now();
        self.symbole.kern.ordner_gewechselt();
        self.liste.kern.ordner_gewechselt();
        let titel = ordner
            .basename()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.fenster.set_title(Some(&titel));
        let pfad = ordner.parse_name().to_string();
        let weak = Rc::downgrade(self);
        let pfad_erst = pfad.clone();
        let erst = Rc::new(std::cell::Cell::new(false));
        let erst_k = erst.clone();
        let fenster = self.fenster.clone();
        let handler = self
            .modell
            .sortiert
            .connect_items_changed(move |_, _, _, neu| {
                if neu > 0 && !erst_k.replace(true) {
                    let pfad = pfad_erst.clone();
                    fenster.add_tick_callback(move |_, _| {
                        eprintln!(
                            "querencia-dateien: Messung {pfad}: erstes Bild nach {} ms",
                            t0.elapsed().as_millis()
                        );
                        glib::ControlFlow::Break
                    });
                }
            });
        let handler = Rc::new(std::cell::RefCell::new(Some(handler)));
        self.modell.oeffne(ordner, move || {
            let Some(p) = weak.upgrade() else { return };
            if let Some(h) = handler.borrow_mut().take() {
                p.modell.sortiert.disconnect(h);
            }
            p.kern().geladen(fokus.as_deref());
            let pfad = pfad.clone();
            p.fenster.add_tick_callback(move |_, _| {
                eprintln!(
                    "querencia-dateien: Messung {pfad}: Ordner vollständig gezeigt nach {} ms",
                    t0.elapsed().as_millis()
                );
                glib::ControlFlow::Break
            });
        });
    }

    fn ansicht_waehlen(&self, name: &str) {
        let vorher = self.kern();
        let cursor = vorher.cursor();
        self.stapel.set_visible_child_name(name);
        let nachher = self.kern();
        protokoll(&format!("Ansicht: {}", nachher.art.name()));
        if let Some(c) = cursor
            && let Some(pos) = nachher.position_von(&c)
        {
            nachher.fokus_auf(pos, false);
        } else {
            nachher.fokus();
        }
    }

    fn zoom(&self, schritt: Option<i32>) {
        let liste = self.stapel.visible_child_name().as_deref() == Some("liste");
        let alt = self.kern().zoom();
        let neu = match schritt {
            Some(s) => ansichtsregeln::zoom_schritt(alt, s),
            None if liste => ansichtsregeln::ZOOM_LISTE,
            None => ansichtsregeln::ZOOM_SYMBOLE,
        };
        if liste {
            self.liste.zoom_setzen(neu);
        } else {
            self.symbole.zoom_setzen(neu);
        }
        protokoll(&format!("Zoomanzeige {}", ansichtsregeln::zoom_text(neu)));
    }

    fn aktionen(self: &Rc<Self>) {
        let f = &self.fenster;
        let einfach = |name: &str, tun: Box<dyn Fn()>| {
            let a = gio::SimpleAction::new(name, None);
            a.connect_activate(move |_, _| tun());
            f.add_action(&a);
        };
        let p = Rc::downgrade(self);
        let mit = move |tun: fn(&Rc<Probe>)| -> Box<dyn Fn()> {
            let p = p.clone();
            Box::new(move || {
                if let Some(p) = p.upgrade() {
                    tun(&p)
                }
            })
        };
        einfach("vergroessern", mit(|p| p.zoom(Some(1))));
        einfach("verkleinern", mit(|p| p.zoom(Some(-1))));
        einfach("normale-groesse", mit(|p| p.zoom(None)));
        einfach("umbenennen", mit(|p| p.kern().umbenennen_starten()));
        einfach("alles-auswaehlen", mit(|p| p.kern().alles_auswaehlen()));
        einfach(
            "neu-laden",
            mit(|p| {
                if let Some(o) = p.modell.ordner() {
                    let k = p.kern();
                    let namen: Vec<String> = k
                        .gewaehlte()
                        .iter()
                        .map(|e| e.daten().name.clone())
                        .collect();
                    p.oeffne(o, k.cursor());
                    k.waehlen(&namen);
                }
            }),
        );
        einfach("schliessen", mit(|p| p.fenster.close()));
        einfach(
            "neuer-ordner",
            mit(|p| {
                let Some(ordner) = p.modell.ordner() else {
                    return;
                };
                let name = namen::unique_name(&gettext("New Folder"), |n| {
                    ordner.child(n).query_exists(gio::Cancellable::NONE)
                });
                let neu = ordner.child(&name);
                if neu.make_directory(gio::Cancellable::NONE).is_ok() {
                    protokoll(&format!("Ordner angelegt: {name}"));
                    p.neu_einlesen(None, &neu);
                    p.kern().umbenennen_wenn_da(&name);
                }
            }),
        );
        let ansicht = gio::SimpleAction::new_stateful(
            "ansicht",
            Some(glib::VariantTy::STRING),
            &"symbole".to_variant(),
        );
        let p = Rc::downgrade(self);
        ansicht.connect_activate(move |a, ziel| {
            if let (Some(p), Some(ziel)) = (p.upgrade(), ziel.and_then(|z| z.get::<String>())) {
                a.set_state(&ziel.to_variant());
                p.ansicht_waehlen(&ziel);
            }
        });
        f.add_action(&ansicht);
        let anordnen = gio::SimpleAction::new_stateful(
            "anordnen",
            Some(glib::VariantTy::STRING),
            &"name".to_variant(),
        );
        let p = Rc::downgrade(self);
        anordnen.connect_activate(move |_, ziel| {
            let (Some(p), Some(k)) = (
                p.upgrade(),
                ziel.and_then(|z| z.get::<String>())
                    .and_then(|z| Kriterium::aus_kennung(&z)),
            ) else {
                return;
            };
            let s = Sortierung {
                kriterium: k,
                ..p.modell.sortierung()
            };
            p.modell.sortierung_setzen(s);
        });
        f.add_action(&anordnen);
        let umgekehrt = gio::SimpleAction::new_stateful("umgekehrt", None, &false.to_variant());
        let p = Rc::downgrade(self);
        umgekehrt.connect_activate(move |a, _| {
            let Some(p) = p.upgrade() else { return };
            let s = p.modell.sortierung();
            let neu = Sortierung {
                umgekehrt: !s.umgekehrt,
                ..s
            };
            a.set_state(&neu.umgekehrt.to_variant());
            p.modell.sortierung_setzen(neu);
        });
        f.add_action(&umgekehrt);
        let verborgene = gio::SimpleAction::new_stateful("verborgene", None, &false.to_variant());
        let p = Rc::downgrade(self);
        verborgene.connect_activate(move |a, _| {
            let Some(p) = p.upgrade() else { return };
            let neu = !p.modell.verborgene();
            a.set_state(&neu.to_variant());
            p.modell.verborgene_setzen(neu);
            p.kern().nach_umordnen();
        });
        f.add_action(&verborgene);
        // B3's operations: only logged in this test window.
        let ohne = [
            "oeffnen",
            "oeffnen-neues-fenster",
            "oeffnen-mit-anderer",
            "ausschneiden",
            "kopieren",
            "einfuegen",
            "in-ordner-einfuegen",
            "papierkorb",
            "loeschen",
            "wiederherstellen",
            "eigenschaften",
        ];
        for name in ohne {
            let a = gio::SimpleAction::new(name, None);
            a.connect_activate(move |_, _| protokoll(&format!("Aktion win.{name}")));
            f.add_action(&a);
        }
        for name in ["oeffnen-mit", "neues-dokument"] {
            let a = gio::SimpleAction::new(name, Some(glib::VariantTy::STRING));
            a.connect_activate(move |_, ziel| {
                let ziel = ziel.and_then(|z| z.get::<String>()).unwrap_or_default();
                protokoll(&format!("Aktion win.{name} „{ziel}“"));
            });
            f.add_action(&a);
        }
    }
}
