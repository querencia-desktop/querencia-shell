//! The dark rules of the panel's and the desktop's own style sheets
//! (QD-081, CSS-Welle dunkel, Thema 0.2; shared with querencia-desktop
//! through `#[path]`).
//!
//! Two facts of GTK 4.22, measured in isolation (Bilder 0.2.4, Dokumente
//! 0.1.2, Benachrichtigungen 0.2.1 and again for this module,
//! `nachweis/welle-dunkel/bindung.txt`):
//! - a style provider the program makes itself keeps `prefers-color-scheme`
//!   at "default"; GTK binds only the theme's provider to the setting
//!   `gtk-interface-color-scheme`, so an `@media (prefers-color-scheme:
//!   dark)` block in the program's provider never applies on its own;
//! - a program started while the setting is already "dark" shows the light
//!   theme until the setting is announced again ([`startfehler_beheben`]).
//!
//! The provider does not follow the setting alone but [`dunkel`]: a dark
//! colour scheme **and** a GTK theme with a dark variant
//! (`gtk-4.0/gtk-dark.css`), the criterion of Querencia Einstellungen 0.1.2.
//! Under a theme without one (Querencia-Klassisch 0.1) the theme stays
//! light, and so do the panel and the desktop: no dark pager or menu on a
//! light panel. Without the settings portal (`portal.rs`) the colour scheme
//! arrives as `gtk-application-prefer-dark-theme`, which counts as dark too.

use std::path::PathBuf;

use gtk4::glib;
use gtk4::prelude::*;

/// `GtkInterfaceColorScheme` (GTK ≥ 4.20) by number; the bindings used here
/// know GTK 4.10 only.
const STANDARD: i32 = 1;
const DUNKEL: i32 = 2;

/// GTK's built-in theme, which has a dark variant; also the name GTK uses
/// when none is set.
const EINGEBAUT: [&str; 2] = ["Default", "Adwaita"];

const SCHEMA: &str = "gtk-interface-color-scheme";
const PROVIDER_SCHEMA: &str = "prefers-color-scheme";

/// Folders GTK searches for a theme, in its order.
fn themenordner() -> Vec<PathBuf> {
    let mut ordner = vec![
        glib::user_data_dir().join("themes"),
        glib::home_dir().join(".themes"),
    ];
    ordner.extend(
        glib::system_data_dirs()
            .into_iter()
            .map(|d| d.join("themes")),
    );
    ordner
}

/// Has the GTK theme `name` a dark variant? As in GTK, the first folder in
/// which the theme has `gtk-4.0/gtk.css` counts.
pub fn thema_mit_dunkel(name: &str) -> bool {
    themenordner()
        .iter()
        .map(|d| d.join(name).join("gtk-4.0"))
        .find(|d| d.join("gtk.css").is_file())
        .map_or(EINGEBAUT.contains(&name), |d| {
            d.join("gtk-dark.css").is_file()
        })
}

fn zahl(object: &impl IsA<glib::Object>, property: &str) -> Option<i32> {
    object.find_property(property)?;
    object
        .property_value(property)
        .transform::<i32>()
        .ok()
        .and_then(|v| v.get::<i32>().ok())
}

/// Does GTK show the dark variant of the theme right now?
pub fn dunkel(settings: &gtk4::Settings) -> bool {
    let schema = zahl(settings, SCHEMA) == Some(DUNKEL);
    (schema || settings.is_gtk_application_prefer_dark_theme())
        && thema_mit_dunkel(settings.gtk_theme_name().as_deref().unwrap_or("Default"))
}

fn setzen(provider: &gtk4::CssProvider, dunkel: bool) -> bool {
    let wert = if dunkel { DUNKEL } else { STANDARD };
    if zahl(provider, PROVIDER_SCHEMA) == Some(wert) {
        return false;
    }
    let Some(pspec) = provider.find_property(PROVIDER_SCHEMA) else {
        return false;
    };
    let Some(value) = glib::EnumClass::with_type(pspec.value_type()).and_then(|k| k.to_value(wert))
    else {
        return false;
    };
    provider.set_property_from_value(PROVIDER_SCHEMA, &value);
    true
}

/// Lets the `@media (prefers-color-scheme: dark)` rules of `provider` apply
/// exactly while [`dunkel`] holds, from now on and after every change of the
/// colour scheme or the theme. Called before the CSS is loaded.
pub fn folgen(provider: &gtk4::CssProvider, programm: &'static str) {
    let Some(settings) = gtk4::Settings::default() else {
        return;
    };
    if provider.find_property(PROVIDER_SCHEMA).is_none() {
        eprintln!("{programm}: GTK ohne Farbschema (vor 4.20), Programm-CSS bleibt hell");
        return;
    }
    let weak = provider.downgrade();
    let anwenden = move |s: &gtk4::Settings, anlass: &str| {
        let Some(provider) = weak.upgrade() else {
            return;
        };
        let an = dunkel(s);
        if setzen(&provider, an) || anlass == "Start" {
            eprintln!(
                "{programm}: Farbschema {} ({anlass}; Einstellung {}, Thema {})",
                if an { "dunkel" } else { "hell" },
                zahl(s, SCHEMA).unwrap_or(0),
                s.gtk_theme_name().unwrap_or_default()
            );
        }
    };
    anwenden(&settings, "Start");
    for name in [
        SCHEMA,
        "gtk-application-prefer-dark-theme",
        "gtk-theme-name",
    ] {
        let anwenden = anwenden.clone();
        settings.connect_notify_local(Some(name), move |s, _| anwenden(s, "Wechsel"));
    }
}

/// GTK 4.22 shows the light theme in a program started while the colour
/// scheme is already dark, until the setting is announced again (Thema 0.2,
/// Übergabe §6); once after start.
pub fn startfehler_beheben() {
    if let Some(s) = gtk4::Settings::default()
        && s.find_property(SCHEMA).is_some()
    {
        s.notify(SCHEMA);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_theme_has_a_dark_variant() {
        assert!(thema_mit_dunkel("Default"));
        assert!(thema_mit_dunkel("Adwaita"));
        assert!(!thema_mit_dunkel("Querencia-gibt-es-nicht"));
    }
}
