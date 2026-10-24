//! Help menu (QD-120): "Online-Hilfe" opens the documentation in the
//! language of the interface, "Fehler melden" the address from
//! `/usr/share/querencia/release` (`BUG_REPORT_URL`, else the project's
//! report page), "Info" the about dialog in the form of Querencia
//! Dokumente: icon, name and version, one sentence, the release of
//! Querencia Desktop (`/usr/share/querencia/release`, else
//! `/usr/share/querencia/version`), alpha notice, link, copyright,
//! licence and the MIT notice for the Strata modules (E9). Every dialog
//! is built afresh when opened and destroyed when closed (QD-076).

use std::path::Path;

use gtk4::prelude::*;
use gtk4::{gdk, gio, glib};

use crate::i18n::{gettext, gettext_f};

pub const RELEASE_FILE: &str = "/usr/share/querencia/release";
pub const VERSION_FILE: &str = "/usr/share/querencia/version";
const WEBSITE: &str = "https://querencialinux.org";
const REPORT_BUG: &str = "https://github.com/querencia-desktop/querencia/issues/new/choose";

/// The `KEY=value` lines of an os-release(5) file (values unquoted).
pub fn os_release(text: &str) -> Vec<(String, String)> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| line.split_once('='))
        .filter(|(key, _)| {
            !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        })
        .map(|(key, value)| {
            let v = value.trim();
            let v = v
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .or_else(|| v.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
                .unwrap_or(v);
            (
                key.to_string(),
                v.replace("\\\"", "\"").replace("\\\\", "\\"),
            )
        })
        .collect()
}

/// What the about dialog and "Fehler melden" take from the package files
/// below `root` (`/` on a running system): (release, bug report address).
pub fn release_von(root: &Path) -> (Option<String>, String) {
    let read = |file: &str| std::fs::read_to_string(root.join(file.trim_start_matches('/'))).ok();
    let felder = read(RELEASE_FILE).map(|t| os_release(&t));
    let feld = |key: &str| {
        felder
            .as_ref()
            .and_then(|f| f.iter().find(|(k, _)| k == key))
            .map(|(_, v)| v.trim().to_string())
            .filter(|v| !v.is_empty())
    };
    let release = match &felder {
        Some(_) => feld("VERSION"),
        None => read(VERSION_FILE).and_then(|t| {
            t.lines()
                .next()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(str::to_string)
        }),
    };
    // only a web address; anything else would be handed to the browser
    let bug = feld("BUG_REPORT_URL")
        .filter(|u| u.starts_with("https://") && !u.contains(char::is_whitespace))
        .unwrap_or_else(|| REPORT_BUG.to_string());
    (release, bug)
}

/// Address of the online help in the language of the interface (the
/// address itself is in the catalog).
pub fn hilfe_adresse() -> String {
    // TRANSLATORS: address of the online help; only change the language
    // code (German …/de/docs/).
    gettext("https://querencialinux.org/en/docs/")
}

/// Opens `uri` with the session's default program, through GIO (not the
/// portal: Dateien is not sandboxed).
pub fn uri_oeffnen(parent: &gtk4::Window, uri: &str) {
    let context = WidgetExt::display(parent).app_launch_context();
    querencia_oberflaeche::portal::for_children(&context);
    if let Err(err) = gio::AppInfo::launch_default_for_uri(uri, Some(&context)) {
        eprintln!("querencia-dateien: {uri} nicht geöffnet: {err}");
    }
}

pub fn online_hilfe(parent: &gtk4::Window) {
    uri_oeffnen(parent, &hilfe_adresse());
}

pub fn fehler_melden(parent: &gtk4::Window) {
    let (_, bug) = release_von(Path::new("/"));
    uri_oeffnen(parent, &bug);
}

/// A transient window that is destroyed when closed; Escape closes it.
pub fn dialogfenster(parent: &gtk4::Window, titel: &str, modal: bool) -> gtk4::Window {
    let w = gtk4::Window::builder()
        .transient_for(parent)
        .modal(modal)
        .destroy_with_parent(true)
        .title(titel)
        .build();
    if let Some(app) = parent.application() {
        w.set_application(Some(&app));
    }
    w.add_css_class("querencia-dateien-dialog");
    let tasten = gtk4::EventControllerKey::new();
    let ww = w.downgrade();
    tasten.connect_key_pressed(move |_, taste, _, _| {
        if taste == gdk::Key::Escape {
            if let Some(w) = ww.upgrade() {
                glib::idle_add_local_once(move || w.close());
            }
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    w.add_controller(tasten);
    w
}

/// Hilfe → Info.
pub fn zeigen(parent: &gtk4::Window) {
    // TRANSLATORS: title of the about dialog
    let window = dialogfenster(parent, &gettext("About Files"), false);
    window.set_resizable(false);
    let body = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    body.set_margin_top(18);
    body.set_margin_bottom(12);
    body.set_margin_start(18);
    body.set_margin_end(18);
    let bild = querencia_oberflaeche::symbole::image_named(crate::APP_ID, 64);
    body.append(&bild);
    let name = gtk4::Label::new(None);
    // TRANSLATORS: program name and version in the about dialog; German
    // „Querencia Dateien {version}“.
    let titel = gettext_f("Files {version}", &[("version", env!("CARGO_PKG_VERSION"))]);
    name.set_markup(&format!(
        "<b><big>{}</big></b>",
        glib::markup_escape_text(&titel)
    ));
    body.append(&name);
    let satz = gtk4::Label::new(Some(&gettext(
        "Classic file manager for Querencia Desktop\nmodelled on Nautilus 2.30.",
    )));
    satz.set_justify(gtk4::Justification::Center);
    body.append(&satz);
    let (release, _) = release_von(Path::new("/"));
    if let Some(r) = release {
        let l = gtk4::Label::new(Some(&gettext_f(
            "Querencia Desktop {release}",
            &[("release", &r)],
        )));
        body.append(&l);
    }
    body.append(&gtk4::Label::new(Some(&gettext(
        "Alpha – not released for everyday use.",
    ))));
    let link = gtk4::Label::new(None);
    link.set_markup(&format!("<a href=\"{WEBSITE}\">querencialinux.org</a>"));
    let ww = window.downgrade();
    link.connect_activate_link(move |_, uri| {
        if let Some(w) = ww.upgrade() {
            uri_oeffnen(&w, uri);
        }
        glib::Propagation::Stop
    });
    body.append(&link);
    for t in [
        "© 2026 Querencia Desktop contributors".to_owned(),
        gettext("License: GPL-3.0-or-later"),
        gettext("Contains code from Strata, MIT"),
    ] {
        let l = gtk4::Label::new(Some(&t));
        l.add_css_class("dim-label");
        body.append(&l);
    }
    let schliessen = gtk4::Button::with_mnemonic(&gettext("_Close"));
    let zeile = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    zeile.set_halign(gtk4::Align::End);
    zeile.set_margin_top(12);
    zeile.append(&schliessen);
    body.append(&zeile);
    let ww = window.downgrade();
    schliessen.connect_clicked(move |_| {
        if let Some(w) = ww.upgrade() {
            glib::idle_add_local_once(move || w.close());
        }
    });
    window.set_child(Some(&body));
    window.set_default_widget(Some(&schliessen));
    window.present();
    schliessen.grab_focus();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_and_bug_address_from_the_package_files() {
        let root = std::env::temp_dir().join(format!("qdat-info-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(release_von(&root), (None, REPORT_BUG.to_string()));
        std::fs::create_dir_all(root.join("usr/share/querencia")).unwrap();
        std::fs::write(root.join("usr/share/querencia/version"), "Alpha 2.2\n").unwrap();
        assert_eq!(release_von(&root).0.as_deref(), Some("Alpha 2.2"));
        std::fs::write(
            root.join("usr/share/querencia/release"),
            "NAME=\"Querencia Desktop\"\nVERSION=\"Alpha 2.3\"\nBUG_REPORT_URL=\"https://example.org/bugs\"\n",
        )
        .unwrap();
        assert_eq!(
            release_von(&root),
            (Some("Alpha 2.3".into()), "https://example.org/bugs".into())
        );
        std::fs::write(
            root.join("usr/share/querencia/release"),
            "VERSION=Alpha\nBUG_REPORT_URL=\"javascript:x\"\n",
        )
        .unwrap();
        assert_eq!(release_von(&root).1, REPORT_BUG);
        let _ = std::fs::remove_dir_all(&root);
    }
}
