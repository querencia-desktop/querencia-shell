//! The "Orte" menu: the home directory and the configured XDG user
//! directories, each opened with the default application for directories
//! through GIO. Built once at panel start; `user-dirs.dirs` is not watched.

use std::path::{Path, PathBuf};

use gtk4::prelude::*;
use gtk4::{MenuButton, gdk, gio, glib};

const ACTION_GROUP: &str = "places";
const OPEN: &str = "open";

/// XDG user directories in menu order with their labels.
const SPECIAL: [(&str, glib::UserDirectory); 6] = [
    ("Schreibtisch", glib::UserDirectory::Desktop),
    ("Dokumente", glib::UserDirectory::Documents),
    ("Downloads", glib::UserDirectory::Downloads),
    ("Bilder", glib::UserDirectory::Pictures),
    ("Musik", glib::UserDirectory::Music),
    ("Videos", glib::UserDirectory::Videos),
];

/// Menu button with the user's places.
pub fn button() -> MenuButton {
    let places = places(&glib::home_dir(), glib::user_special_dir);
    let labels: Vec<_> = places.iter().map(|(label, _)| *label).collect();
    eprintln!("classic2-panel: Ortemenü: {}", labels.join(", "));
    let menu = gio::Menu::new();
    fill(&menu, &places);

    let button = MenuButton::builder()
        .label("Orte")
        .menu_model(&menu)
        .has_frame(false)
        .build();

    let open = gio::SimpleAction::new(OPEN, Some(glib::VariantTy::STRING));
    open.connect_activate(|_, target| {
        if let Some(uri) = target.and_then(|t| t.str()) {
            open_uri(uri);
        }
    });
    let actions = gio::SimpleActionGroup::new();
    actions.add_action(&open);
    button.insert_action_group(ACTION_GROUP, Some(&actions));
    button
}

/// The home directory, then every XDG user directory that GLib reports,
/// that exists and is not the home directory itself (xdg-user-dirs marks a
/// disabled directory by pointing it at `$HOME`). Without `user-dirs.dirs`
/// GLib still reports `$HOME/Desktop` for the desktop. `special` is GLib's
/// lookup; the tests pass their own.
fn places(
    home: &Path,
    special: impl Fn(glib::UserDirectory) -> Option<PathBuf>,
) -> Vec<(&'static str, PathBuf)> {
    let mut places = vec![("Persönlicher Ordner", home.to_path_buf())];
    for (label, dir) in SPECIAL {
        let Some(path) = special(dir) else { continue };
        if path == home {
            continue;
        }
        if !path.is_dir() {
            eprintln!(
                "classic2-panel: {label} ausgelassen, {} existiert nicht",
                path.display()
            );
            continue;
        }
        places.push((label, path));
    }
    places
}

fn fill(menu: &gio::Menu, places: &[(&str, PathBuf)]) {
    for (label, path) in places {
        let item = gio::MenuItem::new(Some(label), None);
        item.set_action_and_target_value(
            Some(&format!("{ACTION_GROUP}.{OPEN}")),
            Some(&gio::File::for_path(path).uri().to_variant()),
        );
        menu.append_item(&item);
    }
}

/// Hands the directory to GIO, which picks the default application for
/// `inode/directory`. Like `app_menu::launch_app`, a successful call only
/// means the launch was started; failures are logged and never end the
/// panel. Without a default application GIO reports an error here.
fn open_uri(uri: &str) {
    // The display's context passes the Wayland activation token on.
    let context = gdk::Display::default().map(|display| display.app_launch_context());
    match gio::AppInfo::launch_default_for_uri(uri, context.as_ref()) {
        Ok(()) => eprintln!("classic2-panel: {uri} an GIO übergeben"),
        Err(err) => eprintln!("classic2-panel: Öffnen von {uri} fehlgeschlagen: {err}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(label: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("classic2-orte-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch directory");
        dir
    }

    #[test]
    fn home_first_then_configured_existing_directories() {
        let home = scratch("home");
        std::fs::create_dir(home.join("Dokumente")).unwrap();
        std::fs::create_dir(home.join("Downloads")).unwrap();
        let lookup = |dir| match dir {
            // Disabled the xdg-user-dirs way: points at home.
            glib::UserDirectory::Desktop => Some(home.clone()),
            glib::UserDirectory::Documents => Some(home.join("Dokumente")),
            // Configured but missing on disk.
            glib::UserDirectory::Pictures => Some(home.join("Bilder")),
            glib::UserDirectory::Downloads => Some(home.join("Downloads")),
            // Music and Videos are not configured.
            _ => None,
        };
        let places = places(&home, lookup);
        assert_eq!(
            places,
            [
                ("Persönlicher Ordner", home.clone()),
                ("Dokumente", home.join("Dokumente")),
                ("Downloads", home.join("Downloads")),
            ]
        );
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn menu_items_open_the_directory_uri() {
        let home = scratch("menu");
        let menu = gio::Menu::new();
        fill(&menu, &places(&home, |_| None));
        assert_eq!(menu.n_items(), 1);
        let attr = |name| {
            menu.item_attribute_value(0, name, Some(glib::VariantTy::STRING))
                .and_then(|v| v.get::<String>())
                .unwrap_or_else(|| panic!("{name} fehlt"))
        };
        assert_eq!(attr("label"), "Persönlicher Ordner");
        assert_eq!(attr("action"), "places.open");
        assert_eq!(attr("target"), gio::File::for_path(&home).uri().as_str());
        assert!(attr("target").starts_with("file:///"));
        let _ = std::fs::remove_dir_all(&home);
    }
}
