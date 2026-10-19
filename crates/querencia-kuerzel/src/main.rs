//! Querencia Tastenkürzel (G2-R): shows the active labwc shortcuts of the
//! running session, grouped as Fenster, Arbeitsflächen, Bildschirmfoto,
//! Sitzung (and Weitere), and the fixed shortcuts of the Querencia
//! programs.
//!
//!   querencia-kuerzel                 window
//!   querencia-kuerzel --liste         the same as text (no display needed)
//!   querencia-kuerzel --rc <datei>    another rc.xml (tests)
//!   querencia-kuerzel --seite programme   opens on the second page

mod labwc;
mod programme;
mod xml;

use std::path::PathBuf;

use gtk4::prelude::*;
use gtk4::{gdk, glib};

const APP_ID: &str = "org.querencia.Kuerzel";

/// What the window and `--liste` show.
struct Overview {
    source: labwc::Source,
    /// rc.xml could not be read: the reason (labwc's defaults are shown).
    problem: Option<String>,
    keybinds: Vec<labwc::Keybind>,
    programs: Vec<programme::Program>,
    programs_source: String,
}

impl Overview {
    fn load(rc_argument: Option<PathBuf>) -> Self {
        let source = match rc_argument {
            Some(path) => labwc::Source::Argument(path),
            None => labwc::find_rc(&labwc::Environment::current()),
        };
        let (rc, problem) = match source.path() {
            Some(path) => match labwc::read_rc(path) {
                Ok(rc) => (Some(rc), None),
                Err(err) => (None, Some(err)),
            },
            None => (None, None),
        };
        let (programs, programs_source) = programme::load();
        Self {
            keybinds: labwc::keybinds(rc.as_ref()),
            source,
            problem,
            programs,
            programs_source,
        }
    }

    fn group(&self, group: labwc::Group) -> Vec<&labwc::Keybind> {
        self.keybinds
            .iter()
            .filter(|b| b.group() == group)
            .collect()
    }

    fn text(&self) -> String {
        let mut out = String::new();
        out.push_str(&self.source.describe());
        out.push('\n');
        if let Some(problem) = &self.problem {
            out.push_str(&format!(
                "rc.xml nicht lesbar ({problem}); gezeigt wird die labwc-Standardbelegung\n"
            ));
        }
        for group in labwc::Group::ALL {
            let list = self.group(group);
            if list.is_empty() {
                continue;
            }
            out.push_str(&format!("\n{}\n", group.title()));
            for bind in list {
                out.push_str(&format!(
                    "  {:<28} {}{}\n",
                    bind.keys_text(),
                    bind.description(),
                    if bind.default {
                        "  [labwc-Standard]"
                    } else {
                        ""
                    }
                ));
            }
        }
        out.push_str(&format!("\n{}\n", self.programs_source));
        for program in &self.programs {
            out.push_str(&format!("\n{}\n", program.name));
            for (keys, what) in &program.entries {
                out.push_str(&format!("  {keys:<28} {what}\n"));
            }
        }
        out
    }
}

const CSS: &str = "
window.querencia-kuerzel .kuerzel-titel { font-weight: bold; margin-top: 6px; }
window.querencia-kuerzel label.tasten { border: 1px solid #c7c2be; border-radius: 3px; background-color: #fbfaf9; color: #3c3c3c; padding: 0 6px; }
window.querencia-kuerzel .kuerzel-quelle { color: #6e6e6e; font-size: smaller; }
window.querencia-kuerzel .kuerzel-warnung { color: #a0331a; }
";

fn heading(text: &str) -> gtk4::Label {
    let label = gtk4::Label::new(Some(text));
    label.add_css_class("kuerzel-titel");
    label.set_xalign(0.0);
    label
}

/// One section; `key_column` keeps the key column of all sections equally wide.
fn table(rows: &[(String, String, Option<String>)], key_column: &gtk4::SizeGroup) -> gtk4::Grid {
    let grid = gtk4::Grid::new();
    grid.set_column_spacing(16);
    grid.set_row_spacing(5);
    grid.set_margin_start(12);
    for (n, (keys, what, tooltip)) in rows.iter().enumerate() {
        let row = i32::try_from(n).unwrap_or(i32::MAX);
        let key = gtk4::Label::new(Some(keys));
        key.add_css_class("tasten");
        key.set_halign(gtk4::Align::Start);
        key.set_valign(gtk4::Align::Start);
        key.set_tooltip_text(tooltip.as_deref());
        // "Strg+Alt+Bild↑ / Strg+Alt+Bild↓" breaks after the slash
        key.set_wrap(true);
        key.set_max_width_chars(20);
        key.set_xalign(0.0);
        // the box around the keys stays as wide as its text; the cell
        // around it takes the common width
        let cell = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        cell.append(&key);
        key_column.add_widget(&cell);
        let text = gtk4::Label::new(Some(what));
        text.set_xalign(0.0);
        text.set_wrap(true);
        text.set_hexpand(true);
        text.set_valign(gtk4::Align::Start);
        grid.attach(&cell, 0, row, 1, 1);
        grid.attach(&text, 1, row, 1, 1);
    }
    grid
}

fn page(content: &gtk4::Box) -> gtk4::ScrolledWindow {
    content.set_margin_top(10);
    content.set_margin_bottom(12);
    content.set_margin_start(12);
    content.set_margin_end(12);
    let scrolled = gtk4::ScrolledWindow::new();
    scrolled.set_hscrollbar_policy(gtk4::PolicyType::Never);
    scrolled.set_vexpand(true);
    scrolled.set_child(Some(content));
    scrolled
}

fn build(app: &gtk4::Application, overview: &Overview, programs_first: bool) {
    let window = gtk4::ApplicationWindow::new(app);
    window.set_title(Some("Tastenkürzel"));
    window.set_default_size(620, 660);
    window.set_icon_name(Some(APP_ID));
    window.add_css_class("querencia-kuerzel");

    let desktop = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    let desktop_keys = gtk4::SizeGroup::new(gtk4::SizeGroupMode::Horizontal);
    if let Some(problem) = &overview.problem {
        let warn = gtk4::Label::new(Some(&format!(
            "Die rc.xml der Sitzung konnte nicht gelesen werden ({problem}). Gezeigt wird die Standardbelegung von labwc."
        )));
        warn.add_css_class("kuerzel-warnung");
        warn.set_wrap(true);
        warn.set_xalign(0.0);
        desktop.append(&warn);
    }
    for group in labwc::Group::ALL {
        let list = overview.group(group);
        if list.is_empty() {
            continue;
        }
        desktop.append(&heading(group.title()));
        let rows: Vec<_> = list
            .iter()
            .map(|b| {
                let origin = if b.default {
                    "labwc-Standard"
                } else {
                    "rc.xml"
                };
                (
                    b.keys_text(),
                    b.description(),
                    Some(format!("{origin}: {}", b.binding)),
                )
            })
            .collect();
        desktop.append(&table(&rows, &desktop_keys));
    }
    let programs = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    for program in &overview.programs {
        programs.append(&heading(&program.name));
        let rows: Vec<_> = program
            .entries
            .iter()
            .map(|(k, w)| (k.clone(), w.clone(), None))
            .collect();
        // each program its own key column: one long entry (Texteditor)
        // does not widen all the others
        let program_keys = gtk4::SizeGroup::new(gtk4::SizeGroupMode::Horizontal);
        programs.append(&table(&rows, &program_keys));
    }
    let notebook = gtk4::Notebook::new();
    notebook.append_page(
        &page(&desktop),
        Some(&gtk4::Label::new(Some("Arbeitsfläche"))),
    );
    notebook.append_page(&page(&programs), Some(&gtk4::Label::new(Some("Programme"))));
    if programs_first {
        notebook.set_current_page(Some(1));
    }

    let source = gtk4::Label::new(Some(&overview.source.describe()));
    source.add_css_class("kuerzel-quelle");
    source.set_wrap(true);
    source.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
    source.set_xalign(0.0);
    source.set_hexpand(true);
    source.set_selectable(true);
    let close = gtk4::Button::with_mnemonic("_Schließen");
    close.set_valign(gtk4::Align::End);
    let w = window.clone();
    close.connect_clicked(move |_| w.close());
    let bottom = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    bottom.append(&source);
    bottom.append(&close);

    let main = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    main.set_margin_top(12);
    main.set_margin_bottom(12);
    main.set_margin_start(12);
    main.set_margin_end(12);
    main.append(&notebook);
    main.append(&bottom);
    window.set_child(Some(&main));
    window.set_default_widget(Some(&close));

    // capture phase: before the focused button or the notebook sees the key
    let keys = gtk4::EventControllerKey::new();
    keys.set_propagation_phase(gtk4::PropagationPhase::Capture);
    let w = window.clone();
    keys.connect_key_pressed(move |_, key, _, state| {
        let ctrl = state.contains(gdk::ModifierType::CONTROL_MASK);
        if key == gdk::Key::Escape || (ctrl && matches!(key, gdk::Key::w | gdk::Key::W)) {
            eprintln!(
                "querencia-kuerzel: {} – Fenster geschlossen",
                if ctrl { "Strg+W" } else { "Esc" }
            );
            w.close();
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    window.add_controller(keys);
    window.present();
    close.grab_focus();
}

fn main() -> glib::ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let mut rc = None;
    let mut list = false;
    let mut programs_first = false;
    let mut it = args.iter().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--liste" => list = true,
            "--seite" => match it.next().map(String::as_str) {
                Some("programme") => programs_first = true,
                Some("arbeitsflaeche") => programs_first = false,
                _ => {
                    eprintln!("querencia-kuerzel: --seite programme|arbeitsflaeche");
                    return glib::ExitCode::from(2);
                }
            },
            "--rc" => match it.next() {
                Some(path) => rc = Some(PathBuf::from(path)),
                None => {
                    eprintln!("querencia-kuerzel: --rc braucht eine Datei");
                    return glib::ExitCode::from(2);
                }
            },
            "--version" => {
                println!("querencia-kuerzel {}", env!("CARGO_PKG_VERSION"));
                return glib::ExitCode::SUCCESS;
            }
            "--hilfe" | "--help" | "-h" => {
                println!(
                    "Aufruf: querencia-kuerzel [--liste] [--rc <rc.xml>] [--seite programme|arbeitsflaeche] [--version]"
                );
                return glib::ExitCode::SUCCESS;
            }
            other => {
                eprintln!("querencia-kuerzel: unbekannte Option {other}");
                return glib::ExitCode::from(2);
            }
        }
    }
    let overview = Overview::load(rc);
    if list {
        print!("{}", overview.text());
        return glib::ExitCode::SUCCESS;
    }
    eprintln!("querencia-kuerzel: {}", overview.source.describe());
    if let Some(problem) = &overview.problem {
        eprintln!("querencia-kuerzel: rc.xml nicht lesbar: {problem}");
    }
    let app = gtk4::Application::builder().application_id(APP_ID).build();
    app.connect_startup(|_| {
        let provider = gtk4::CssProvider::new();
        provider.load_from_data(CSS);
        if let Some(display) = gdk::Display::default() {
            gtk4::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    });
    let overview = std::rc::Rc::new(overview);
    app.connect_activate(move |app| {
        if let Some(window) = app.active_window() {
            window.present();
            return;
        }
        build(app, &overview, programs_first);
    });
    // the options above are ours; GApplication gets none
    app.run_with_args(&args[..1])
}
