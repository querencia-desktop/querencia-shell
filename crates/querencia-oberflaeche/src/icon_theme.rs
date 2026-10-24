//! The icon theme of the Querencia programs (G2-AD, panel and desktop
//! first): the user's choice, else GNOME Human.
//!
//! The session sets no icon theme, so GTK would use its default Adwaita.
//! For the GNOME 2 look without a session-wide change, the panel (HMP-02)
//! and the desktop (HMP-09) switched their own process to GNOME Human
//! whenever GTK reported a built-in default ("", "Adwaita", "hicolor"). GTK
//! only knows the resulting value, though, so a user who chose Adwaita got
//! GNOME Human, and once set by the program, GTK no longer applied changes
//! from the settings portal: a new theme needed a restart.
//!
//! The user chooses with `org.gnome.desktop.interface icon-theme`
//! (GSettings in dconf; settings dialogs write it, the GTK settings portal
//! passes it on). Here the key's user value decides: set, it is used (if the
//! theme is installed); unset, GNOME Human gives the GNOME 2 look. Without
//! the schema, GTK's value counts and only its built-in defaults are
//! replaced, as before. The key is watched, so a new choice or a reset
//! applies at once. The theme is set on this process's `GtkSettings` only;
//! no file and no other application changes.

use std::cell::RefCell;
use std::path::PathBuf;

use gtk4::prelude::*;
use gtk4::{gdk, gio, glib};

/// Icon theme of the GNOME 2 look, used while the user has chosen none.
pub const CLASSIC_ICON_THEME: &str = "gnome-human";
const SCHEMA: &str = "org.gnome.desktop.interface";
const KEY: &str = "icon-theme";

/// What the user chose, as far as it can be known.
#[derive(Clone, Debug, PartialEq)]
pub enum UserChoice {
    /// The key has a user value.
    Theme(String),
    /// The key is at its default (reset or never set).
    Default,
    /// No schema: GTK's current theme is all there is.
    Unknown { gtk: String },
}

/// The theme to use for `choice`, or `None` to leave GTK's own value.
/// `installed` tells whether a theme is installed.
pub fn choose(choice: &UserChoice, installed: impl Fn(&str) -> bool) -> Option<String> {
    let classic = || installed(CLASSIC_ICON_THEME).then(|| CLASSIC_ICON_THEME.to_string());
    match choice {
        UserChoice::Theme(name) if !name.is_empty() && installed(name) => Some(name.clone()),
        // Chosen but not installed: GTK would fall back to hicolor and lose
        // most icons; GNOME Human is the better fallback.
        UserChoice::Theme(_) | UserChoice::Default => classic(),
        UserChoice::Unknown { gtk } => {
            let built_in_default = matches!(gtk.as_str(), "" | "Adwaita" | "hicolor");
            if built_in_default { classic() } else { None }
        }
    }
}

thread_local! {
    /// The watched settings object; it must stay alive for its signal.
    static WATCHED: RefCell<Option<gio::Settings>> = const { RefCell::new(None) };
}

/// Applies [`choose`] to this process and follows changes of the user's
/// choice. `program` prefixes the log lines.
pub fn follow(program: &'static str) {
    let schema = gio::SettingsSchemaSource::default()
        .and_then(|source| source.lookup(SCHEMA, true))
        .filter(|schema| schema.has_key(KEY));
    if schema.is_none() {
        apply(program, None);
        return;
    }
    let settings = gio::Settings::new(SCHEMA);
    apply(program, Some(&settings));
    settings.connect_changed(Some(KEY), move |settings, _| apply(program, Some(settings)));
    WATCHED.with(|watched| watched.replace(Some(settings)));
}

fn apply(program: &str, settings: Option<&gio::Settings>) {
    let Some(gtk_settings) = gtk4::Settings::default() else {
        return;
    };
    let choice = match settings {
        Some(settings) => match settings.user_value(KEY).and_then(|v| v.get::<String>()) {
            Some(name) => UserChoice::Theme(name),
            None => UserChoice::Default,
        },
        None => UserChoice::Unknown {
            gtk: gtk_settings
                .gtk_icon_theme_name()
                .map(|name| name.to_string())
                .unwrap_or_default(),
        },
    };
    let dirs = search_path();
    let installed = |name: &str| theme_installed(name, &dirs);
    let source = match &choice {
        UserChoice::Theme(name) => format!("Benutzereinstellung „{name}“"),
        UserChoice::Default => "keine Benutzereinstellung".to_string(),
        UserChoice::Unknown { gtk } => format!("ohne {SCHEMA}, GTK „{gtk}“"),
    };
    match choose(&choice, installed) {
        Some(theme) => {
            gtk_settings.set_gtk_icon_theme_name(Some(&theme));
            eprintln!("{program}: Symbolthema {theme} ({source})");
        }
        None => {
            // Back to GTK's own value, in case an earlier choice set one.
            gtk_settings.reset_property("gtk-icon-theme-name");
            eprintln!("{program}: Symbolthema von GTK ({source})");
        }
    }
}

/// Whether one of the icon folders `dirs` has `<name>/index.theme`.
fn theme_installed(name: &str, dirs: &[PathBuf]) -> bool {
    dirs.iter()
        .any(|dir| dir.join(name).join("index.theme").is_file())
}

/// The folders GTK looks for icon themes in.
fn search_path() -> Vec<PathBuf> {
    match gdk::Display::default() {
        Some(display) => gtk4::IconTheme::for_display(&display).search_path(),
        None => {
            let mut dirs = vec![glib::user_data_dir().join("icons")];
            dirs.extend(
                glib::system_data_dirs()
                    .into_iter()
                    .map(|d| d.join("icons")),
            );
            dirs
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn installed(name: &str) -> bool {
        matches!(name, "gnome-human" | "Adwaita" | "Querencia")
    }

    fn chosen(choice: UserChoice) -> Option<String> {
        choose(&choice, installed)
    }

    #[test]
    fn the_users_choice_counts_even_adwaita() {
        for name in ["Querencia", "Adwaita", "gnome-human"] {
            assert_eq!(
                chosen(UserChoice::Theme(name.into())).as_deref(),
                Some(name)
            );
        }
    }

    #[test]
    fn without_a_choice_gnome_human() {
        assert_eq!(chosen(UserChoice::Default).as_deref(), Some("gnome-human"));
        // A theme that is not installed, or an empty name, is no usable
        // choice either.
        assert_eq!(
            chosen(UserChoice::Theme("Papirus".into())).as_deref(),
            Some("gnome-human")
        );
        assert_eq!(
            chosen(UserChoice::Theme(String::new())).as_deref(),
            Some("gnome-human")
        );
    }

    #[test]
    fn without_gnome_human_gtk_decides() {
        let only_adwaita = |name: &str| name == "Adwaita";
        assert_eq!(choose(&UserChoice::Default, only_adwaita), None);
        assert_eq!(
            choose(&UserChoice::Theme("Papirus".into()), only_adwaita),
            None
        );
        assert_eq!(
            choose(&UserChoice::Theme("Adwaita".into()), only_adwaita).as_deref(),
            Some("Adwaita")
        );
    }

    #[test]
    fn a_theme_is_installed_if_an_icon_folder_has_its_index() {
        let root = std::env::temp_dir().join(format!("querencia-icons-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let with = root.join("mit");
        let without = root.join("ohne");
        std::fs::create_dir_all(with.join("gnome-human")).unwrap();
        std::fs::write(with.join("gnome-human/index.theme"), "[Icon Theme]\n").unwrap();
        std::fs::create_dir_all(without.join("gnome-human")).unwrap();
        assert!(theme_installed(
            "gnome-human",
            &[without.clone(), with.clone()]
        ));
        assert!(!theme_installed(
            "gnome-human",
            std::slice::from_ref(&without)
        ));
        assert!(!theme_installed("humanity", &[with]));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn without_the_schema_only_gtks_defaults_are_replaced() {
        let gtk = |name: &str| UserChoice::Unknown { gtk: name.into() };
        for default in ["", "Adwaita", "hicolor"] {
            assert_eq!(chosen(gtk(default)).as_deref(), Some("gnome-human"));
        }
        assert_eq!(chosen(gtk("Querencia")), None);
        assert_eq!(chosen(gtk("gnome-human")), None);
        assert_eq!(choose(&gtk("Adwaita"), |_| false), None);
    }
}
