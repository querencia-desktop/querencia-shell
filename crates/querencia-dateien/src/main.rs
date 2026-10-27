//! Querencia Dateien – klassischer Dateimanager für Querencia Desktop nach
//! dem Vorbild von Nautilus 2.30 (Browser-Modus): Menüleiste,
//! Werkzeugleiste, Knopfpfad, Seitenleiste „Orte“, Symbol- und
//! Listenansicht, Statusleiste. GTK 4 ohne libadwaita, labwc zeichnet die
//! Titelleiste. Dateioperationen über `querencia-dateiarbeit`, Menüs und
//! Symbole über `querencia-oberflaeche`. Aufbau und Schnittstellen der
//! Welle B: `docs/dateien.md`.

mod adressleiste;
mod ansicht;
mod app;
mod einstellungen;
mod eintrag;
mod fenster;
mod i18n;
mod info;
mod konfig;
mod lesezeichen;
mod menue;
mod modell;
mod oeffnen;
mod orte;
mod regeln;
mod seitenleiste;
mod statusleiste;
mod werkzeugleiste;

// Welle B2 (dateien-ansichten): mod-Zeilen unter dieser Zeile.
mod ansichtskern;
mod ansichtsregeln;
mod kontextmenue;
mod listenansicht;
mod symbolansicht;
mod tippsuche;
mod umbenennen;

// Welle B3 (dateien-operationen): mod-Zeilen unter dieser Zeile.
mod eigenschaften_dialog;
mod fortschritt;
mod konflikt;
mod operationen;
mod rueckgaengig;
// 0.3.0: „Hier entpacken“ über Querencia Archiv
mod entpacken;
mod ziehen;
mod zwischenablage;

// Welle C1 (dateien-netzwerk): mod-Zeilen unter dieser Zeile.
mod netzwerk;

// Welle C2 (dateien-miniaturen-suche): mod-Zeilen unter dieser Zeile.
mod miniaturen;
mod suche;

// Welle C3 (dateien-reiter-kompakt): mod-Zeilen unter dieser Zeile.
mod kompaktansicht;
mod reiter;

use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{gio, glib};
use querencia_oberflaeche::{farbschema, icon_theme, portal};

pub const APP_ID: &str = "org.querencia.Dateien";
const PROGRAMM: &str = "querencia-dateien";
const STIL: &str = include_str!("style.css");

/// The hooks of the modules of Welle B2 and B3 (`docs/dateien.md` §3.5).
/// Each module adds one line here.
fn erweiterungen(register: &mut app::Register) {
    // Welle B2: Aufrufe unter dieser Zeile.
    symbolansicht::registrieren(register);
    listenansicht::registrieren(register);
    kontextmenue::registrieren(register);

    // Welle B3: Aufrufe unter dieser Zeile.
    operationen::registrieren(register);
    entpacken::registrieren(register);

    // Welle C1 (dateien-netzwerk): Aufrufe unter dieser Zeile.
    netzwerk::registrieren(register);

    // Welle C2 (dateien-miniaturen-suche): Aufrufe unter dieser Zeile.
    miniaturen::registrieren(register);
    suche::registrieren(register);

    // Welle C3: Aufrufe unter dieser Zeile.
    kompaktansicht::registrieren(register);
}

/// One log line on stderr ("querencia-dateien: …"), as the other programs.
pub fn log(text: &str) {
    eprintln!("{PROGRAMM}: {text}");
}

fn stil_laden() {
    let css = gtk4::CssProvider::new();
    farbschema::folgen(&css, PROGRAMM);
    css.load_from_data(STIL);
    if let Some(display) = gtk4::gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &css,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
    farbschema::startfehler_beheben();
}

fn app_aktionen(app: &gtk4::Application) {
    let neu = gio::SimpleAction::new("neues-fenster", None);
    neu.connect_activate(|_, _| {
        if let Some(a) = app::app_wenn() {
            let ort = a
                .gtk()
                .active_window()
                .and_then(|w| a.fenster_von(&w))
                .and_then(|f| f.ort());
            a.neues_fenster(ort.as_ref());
        }
    });
    app.add_action(&neu);
    let alle = gio::SimpleAction::new("alle-schliessen", None);
    let a = app.downgrade();
    alle.connect_activate(move |_, _| {
        if let Some(app) = a.upgrade() {
            for w in app.windows() {
                w.close();
            }
        }
    });
    app.add_action(&alle);
}

fn main() -> glib::ExitCode {
    i18n::init();
    let modus = portal::prepare(PROGRAMM);
    glib::set_application_name(&i18n::gettext("Files"));
    let app = gtk4::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_OPEN)
        .build();
    let mut register = app::Register::default();
    erweiterungen(&mut register);
    let register = Rc::new(std::cell::RefCell::new(Some(register)));
    app.connect_startup(move |app| {
        portal::follow_gsettings(PROGRAMM, modus);
        gtk4::Window::set_default_icon_name(APP_ID);
        icon_theme::follow(PROGRAMM);
        stil_laden();
        if let Some(s) = gtk4::Settings::default() {
            s.notify("gtk-interface-color-scheme");
        }
        app_aktionen(app);
        menue::kuerzel_setzen(app);
        let r = register.borrow_mut().take().unwrap_or_default();
        app::App::starten(app, r);
    });
    app.connect_activate(|_| {
        if let Some(a) = app::app_wenn() {
            // the first window of a start without a folder: the tabs of
            // last time (0.3.0), else the home folder
            if a.alle_fenster().is_empty() && reiter::sitzung::wiederherstellen(&a).is_some() {
                return;
            }
            a.neues_fenster(None);
        }
    });
    app.connect_open(|_, dateien, _| {
        if let Some(a) = app::app_wenn() {
            for d in dateien {
                a.neues_fenster(Some(d));
            }
        }
    });
    let ende = app.run();
    app::freigeben();
    ende
}
