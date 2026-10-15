//! The "Orte" menu as in GNOME 2: the home directory and the XDG user
//! directories, then "Computer", "Netzwerk" and "Papierkorb", each opened
//! with the default application for directories through GIO.
//!
//! Only what works is shown. A user directory must exist; a location needs
//! its GVfs backend (`computer:`, `network:`, `trash:`), asked once per
//! build from GIO without touching the backend. Without any application for
//! `inode/directory` nothing could be opened, so the menu shows only a hint
//! instead. The menu is rebuilt when applications change (a file manager is
//! installed or removed), like the applications menu; `user-dirs.dirs` is
//! not watched.

use std::path::{Path, PathBuf};

use gtk4::prelude::*;
use gtk4::{MenuButton, gdk, gio, glib};

use crate::app_menu;

const ACTION_GROUP: &str = "places";
const OPEN: &str = "open";
const NAME: &str = "Ortemenü";
/// Shown instead of the places when no application can open a directory.
const NO_FILE_MANAGER: &str = "Kein Dateimanager installiert";

/// XDG user directories in GNOME 2 order (Ubuntu 10.04: desktop, then the
/// bookmarks Dokumente, Musik, Bilder, Videos, Downloads) with their German
/// GNOME 2 labels.
const SPECIAL: [(&str, glib::UserDirectory); 6] = [
    ("Arbeitsfläche", glib::UserDirectory::Desktop),
    ("Dokumente", glib::UserDirectory::Documents),
    ("Musik", glib::UserDirectory::Music),
    ("Bilder", glib::UserDirectory::Pictures),
    ("Videos", glib::UserDirectory::Videos),
    ("Downloads", glib::UserDirectory::Downloads),
];

/// Locations of the second section: label, GVfs URI scheme, URI.
const LOCATIONS: [(&str, &str, &str); 3] = [
    ("Computer", "computer", "computer:///"),
    ("Netzwerk", "network", "network:///"),
    ("Papierkorb", "trash", "trash:///"),
];

/// Menu button with the user's places, kept up to date.
pub fn button() -> MenuButton {
    let menu = gio::Menu::new();
    rebuild(&menu);

    let button = app_menu::menu_button("Orte", None, &menu);

    let open = gio::SimpleAction::new(OPEN, Some(glib::VariantTy::STRING));
    open.connect_activate(|_, target| {
        if let Some(uri) = target.and_then(|t| t.str()) {
            open_uri(uri);
        }
    });
    let actions = gio::SimpleActionGroup::new();
    actions.add_action(&open);
    button.insert_action_group(ACTION_GROUP, Some(&actions));
    app_menu::follow_changes(&button, &menu, NAME, rebuild);
    button
}

/// Reads the places again and refills `menu`.
fn rebuild(menu: &gio::Menu) {
    let personal = places(&glib::home_dir(), glib::user_special_dir);
    let schemes: Vec<String> = gio::Vfs::default()
        .supported_uri_schemes()
        .iter()
        .map(|s| s.to_string())
        .collect();
    let locations = locations(&schemes);
    let can_open = gio::AppInfo::default_for_type("inode/directory", false).is_some();
    let labels: Vec<_> = personal
        .iter()
        .map(|(label, _)| *label)
        .chain(locations.iter().map(|(label, _)| *label))
        .collect();
    if can_open {
        eprintln!("querencia-panel: {NAME}: {}", labels.join(", "));
    } else {
        eprintln!(
            "querencia-panel: {NAME}: keine Anwendung für Ordner (inode/directory), \
             nur Hinweis „{NO_FILE_MANAGER}“; vorhanden wären {}",
            labels.join(", ")
        );
    }
    fill(menu, &personal, &locations, can_open);
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
                "querencia-panel: {label} ausgelassen, {} existiert nicht",
                path.display()
            );
            continue;
        }
        places.push((label, path));
    }
    places
}

/// The locations whose URI scheme GIO supports (`schemes`, from
/// `g_vfs_get_supported_uri_schemes`), in menu order, with their URIs.
fn locations(schemes: &[String]) -> Vec<(&'static str, &'static str)> {
    LOCATIONS
        .iter()
        .filter(|(_, scheme, _)| schemes.iter().any(|s| s == scheme))
        .map(|(label, _, uri)| (*label, *uri))
        .collect()
}

/// Replaces the contents of `menu`: a section with the personal places and,
/// if there are any, one with the locations; GTK draws a separator between
/// them. If no application can open a directory (`can_open` false), only a
/// hint without action. The menu object stays the same.
fn fill(menu: &gio::Menu, places: &[(&str, PathBuf)], locations: &[(&str, &str)], can_open: bool) {
    menu.remove_all();
    if !can_open {
        menu.append(Some(NO_FILE_MANAGER), None);
        return;
    }
    let personal = gio::Menu::new();
    for (label, path) in places {
        personal.append_item(&open_item(label, &gio::File::for_path(path).uri()));
    }
    menu.append_section(None, &personal);
    if !locations.is_empty() {
        let section = gio::Menu::new();
        for (label, uri) in locations {
            section.append_item(&open_item(label, uri));
        }
        menu.append_section(None, &section);
    }
}

fn open_item(label: &str, uri: &str) -> gio::MenuItem {
    let item = gio::MenuItem::new(Some(label), None);
    item.set_action_and_target_value(
        Some(&format!("{ACTION_GROUP}.{OPEN}")),
        Some(&uri.to_variant()),
    );
    item
}

/// Hands the URI to GIO, which picks the default application: for `file:`
/// directories the one for `inode/directory`, for `computer:`, `network:` and
/// `trash:` a handler of the scheme or else again the one for directories.
/// Like `app_menu::launch_app`, a successful call only means the launch was
/// started; failures are logged and never end the panel.
fn open_uri(uri: &str) {
    // The display's context passes the Wayland activation token on.
    let context = gdk::Display::default().map(|display| display.app_launch_context());
    match gio::AppInfo::launch_default_for_uri(uri, context.as_ref()) {
        Ok(()) => eprintln!("querencia-panel: {uri} an GIO übergeben"),
        Err(err) => eprintln!("querencia-panel: Öffnen von {uri} fehlgeschlagen: {err}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(label: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("querencia-orte-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch directory");
        dir
    }

    fn attr(menu: &gio::MenuModel, index: i32, name: &str) -> Option<String> {
        menu.item_attribute_value(index, name, Some(glib::VariantTy::STRING))
            .and_then(|v| v.get::<String>())
    }

    /// Each section as "label -> action(target)" lines.
    fn sections(menu: &gio::Menu) -> Vec<Vec<String>> {
        (0..menu.n_items())
            .map(|s| {
                let section = menu.item_link(s, "section").expect("section link");
                (0..section.n_items())
                    .map(|i| {
                        format!(
                            "{} -> {}({})",
                            attr(&section, i, "label").expect("label"),
                            attr(&section, i, "action").expect("action"),
                            attr(&section, i, "target").expect("target")
                        )
                    })
                    .collect()
            })
            .collect()
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
    fn personal_places_in_gnome2_order_and_names() {
        let home = scratch("reihenfolge");
        let names = [
            ("Desktop", glib::UserDirectory::Desktop),
            ("Documents", glib::UserDirectory::Documents),
            ("Music", glib::UserDirectory::Music),
            ("Pictures", glib::UserDirectory::Pictures),
            ("Videos", glib::UserDirectory::Videos),
            ("Downloads", glib::UserDirectory::Downloads),
        ];
        for (name, _) in names {
            std::fs::create_dir(home.join(name)).unwrap();
        }
        let lookup = |dir| {
            names
                .iter()
                .find(|(_, d)| *d == dir)
                .map(|(name, _)| home.join(name))
        };
        let labels: Vec<_> = places(&home, lookup).into_iter().map(|(l, _)| l).collect();
        assert_eq!(
            labels,
            [
                "Persönlicher Ordner",
                "Arbeitsfläche",
                "Dokumente",
                "Musik",
                "Bilder",
                "Videos",
                "Downloads"
            ]
        );
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn locations_only_for_schemes_gvfs_offers() {
        let all = ["file", "computer", "network", "trash", "smb"].map(String::from);
        assert_eq!(
            locations(&all),
            [
                ("Computer", "computer:///"),
                ("Netzwerk", "network:///"),
                ("Papierkorb", "trash:///")
            ]
        );
        // without GVfs only file: is known
        assert!(locations(&["file".to_string()]).is_empty());
        assert_eq!(
            locations(&["trash".to_string(), "file".to_string()]),
            [("Papierkorb", "trash:///")]
        );
    }

    #[test]
    fn menu_has_a_personal_and_a_location_section() {
        let home = scratch("menu");
        std::fs::create_dir(home.join("Musik")).unwrap();
        let personal = places(&home, |dir| {
            (dir == glib::UserDirectory::Music).then(|| home.join("Musik"))
        });
        let menu = gio::Menu::new();
        let schemes = ["file", "network", "trash"].map(String::from);
        fill(&menu, &personal, &locations(&schemes), true);
        let home_uri = gio::File::for_path(&home).uri();
        let music_uri = gio::File::for_path(home.join("Musik")).uri();
        assert_eq!(
            sections(&menu),
            [
                vec![
                    format!("Persönlicher Ordner -> places.open({home_uri})"),
                    format!("Musik -> places.open({music_uri})"),
                ],
                vec![
                    "Netzwerk -> places.open(network:///)".to_string(),
                    "Papierkorb -> places.open(trash:///)".to_string(),
                ],
            ]
        );
        assert!(home_uri.starts_with("file:///"));
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn no_location_section_without_locations() {
        let home = scratch("ohne-orte");
        let menu = gio::Menu::new();
        fill(&menu, &places(&home, |_| None), &[], true);
        assert_eq!(sections(&menu).len(), 1);
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn without_a_file_manager_only_a_hint_is_shown() {
        let home = scratch("ohne-dateimanager");
        let menu = gio::Menu::new();
        let schemes = ["file", "trash"].map(String::from);
        fill(&menu, &places(&home, |_| None), &locations(&schemes), false);
        assert_eq!(menu.n_items(), 1);
        let model = menu.upcast_ref::<gio::MenuModel>();
        assert_eq!(attr(model, 0, "label").as_deref(), Some(NO_FILE_MANAGER));
        assert!(menu.item_attribute_value(0, "action", None).is_none());
        // filling again replaces the contents
        fill(&menu, &places(&home, |_| None), &[], true);
        assert_eq!(sections(&menu).len(), 1);
        let _ = std::fs::remove_dir_all(&home);
    }
}
