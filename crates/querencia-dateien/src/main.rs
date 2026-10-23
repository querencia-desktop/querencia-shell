//! Querencia Dateien – klassischer Dateimanager für Querencia Desktop nach
//! dem Vorbild von Nautilus 2.30 (Browser-Modus): Menüleiste,
//! Werkzeugleiste, Knopfpfad, Seitenleiste „Orte“, Symbol- und
//! Listenansicht, Statusleiste. GTK 4 ohne libadwaita, labwc zeichnet die
//! Titelleiste. Dateioperationen über `querencia-dateiarbeit`, Menüs und
//! Symbole über `querencia-oberflaeche`. Aufbau und Schnittstellen der
//! Welle B: `docs/dateien.md`.

#[allow(dead_code)] // Schnittstelle der Welle B (docs/dateien.md §3), bis zur Integration
mod adressleiste;
#[allow(dead_code)] // Schnittstelle der Welle B (docs/dateien.md §3), bis zur Integration
mod ansicht;
#[allow(dead_code)] // Schnittstelle der Welle B (docs/dateien.md §3), bis zur Integration
mod app;
mod eigenschaften;
mod einstellungen;
mod eintrag;
#[allow(dead_code)] // Schnittstelle der Welle B (docs/dateien.md §3), bis zur Integration
mod fenster;
mod i18n;
mod info;
mod konfig;
mod lesezeichen;
#[allow(dead_code)] // Schnittstelle der Welle B (docs/dateien.md §3), bis zur Integration
mod menue;
#[allow(dead_code)] // Schnittstelle der Welle B (docs/dateien.md §3), bis zur Integration
mod modell;
mod oeffnen;
mod orte;
#[allow(dead_code)] // Schnittstelle der Welle B (docs/dateien.md §3), bis zur Integration
mod regeln;
mod seitenleiste;
mod statusleiste;
mod werkzeugleiste;

// Welle B2 (dateien-ansichten): mod-Zeilen unter dieser Zeile.

// Welle B3 (dateien-operationen): mod-Zeilen unter dieser Zeile.

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
    let _ = register;
    // Welle B2: Aufrufe unter dieser Zeile.

    // Welle B3: Aufrufe unter dieser Zeile.
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
    app.run()
}
