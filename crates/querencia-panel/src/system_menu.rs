//! The "System" menu: the settings from the application catalog, split the
//! GNOME 2 way into "Einstellungen" and "Systemverwaltung" (see
//! [`AppEntry::system_section`]). Entries launch through GIO by desktop ID
//! and the menu follows changed applications, both exactly like the
//! applications menu, whose code it shares. Session actions such as lock or
//! log out are not part of it yet.

use gtk4::MenuButton;
use gtk4::gio;
use gtk4::prelude::*;

use crate::app_menu;
use crate::apps::{AppEntry, Catalog, SystemSection};

const ACTION_GROUP: &str = "system";
const NAME: &str = "Systemmenü";
const NO_SETTINGS: &str = "Keine Einstellungen";

/// Menu button with the installed settings, kept up to date.
pub fn button() -> MenuButton {
    let menu = gio::Menu::new();
    rebuild(&menu);

    let button = MenuButton::builder()
        .label("System")
        .menu_model(&menu)
        .has_frame(false)
        .build();
    app_menu::add_launch_action(&button, ACTION_GROUP);
    app_menu::follow_changes(&button, &menu, NAME, rebuild);
    button
}

/// Reads the catalog again and refills `menu` from it.
fn rebuild(menu: &gio::Menu) {
    let catalog = Catalog::load();
    let sections = catalog.system_sections();
    let summary: Vec<_> = sections
        .iter()
        .map(|(section, entries)| format!("{} {}", section.label(), entries.len()))
        .collect();
    let summary = if summary.is_empty() {
        NO_SETTINGS.to_string()
    } else {
        summary.join(", ")
    };
    eprintln!("querencia-panel: {NAME}: {summary}");
    fill(menu, &sections);
}

/// Replaces the contents of `menu` in place: one submenu per section, as
/// [`Catalog::system_sections`] lists them, so empty sections never show.
/// Without any setting the menu holds one item without action instead of
/// an empty popover. The menu object itself, and with it the button's
/// popover and actions, stays the same.
fn fill(menu: &gio::Menu, sections: &[(SystemSection, Vec<&AppEntry>)]) {
    menu.remove_all();
    for (section, entries) in sections {
        let submenu = gio::Menu::new();
        for entry in entries {
            submenu.append_item(&app_menu::launch_item(entry, ACTION_GROUP));
        }
        menu.append_submenu(Some(section.label()), &submenu);
    }
    if menu.n_items() == 0 {
        menu.append(Some(NO_SETTINGS), None);
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use gtk4::glib;

    use super::*;
    use crate::app_menu::tests::{entry, group, label, structure};

    fn catalog(entries: Vec<AppEntry>) -> Catalog {
        Catalog {
            entries,
            excluded: Vec::new(),
        }
    }

    fn model(catalog: &Catalog) -> gio::Menu {
        let menu = gio::Menu::new();
        fill(&menu, &catalog.system_sections());
        menu
    }

    #[test]
    fn settings_go_to_preferences_or_administration() {
        let section = |categories: &[&str]| entry("x.desktop", "X", categories).system_section();
        assert_eq!(
            section(&["Settings", "DesktopSettings"]),
            Some(SystemSection::Preferences)
        );
        assert_eq!(section(&["Settings"]), Some(SystemSection::Preferences));
        assert_eq!(
            section(&["GTK", "Settings", "HardwareSettings"]),
            Some(SystemSection::Preferences)
        );
        assert_eq!(
            section(&["Settings", "System", "HardwareSettings"]),
            Some(SystemSection::Administration)
        );
        // Category order does not matter.
        assert_eq!(
            section(&["System", "Settings"]),
            Some(SystemSection::Administration)
        );
        assert_eq!(section(&["System", "Monitor"]), None);
        assert_eq!(section(&["Utility"]), None);
        assert_eq!(section(&[]), None);
        // Categories are case-sensitive in the desktop entry specification.
        assert_eq!(section(&["settings", "System"]), None);
        assert!(SystemSection::Preferences < SystemSection::Administration);
    }

    #[test]
    fn sections_become_submenus_sorted_by_name() {
        // In catalog order (by menu group), which is not the menu order here.
        let catalog = catalog(vec![
            entry("u.desktop", "Uhr", &["Utility"]),
            entry("w.desktop", "Wlan", &["Settings", "Network"]),
            entry("m.desktop", "Monitor", &["System", "Monitor"]),
            entry("p.desktop", "Partitionen", &["Settings", "System"]),
            entry("d.desktop", "Drucker", &["Settings", "System", "Printing"]),
            entry("a.desktop", "Aussehen", &["Settings", "DesktopSettings"]),
            entry("b.desktop", "Aussehen", &["Settings"]),
        ]);
        assert_eq!(
            structure(&model(&catalog)),
            [
                group(
                    "Einstellungen",
                    &[
                        // Equal names fall back to the desktop ID.
                        "Aussehen -> system.launch(a.desktop)",
                        "Aussehen -> system.launch(b.desktop)",
                        "Wlan -> system.launch(w.desktop)",
                    ]
                ),
                group(
                    "Systemverwaltung",
                    &[
                        "Drucker -> system.launch(d.desktop)",
                        "Partitionen -> system.launch(p.desktop)",
                    ]
                ),
            ]
        );
    }

    #[test]
    fn empty_sections_are_left_out() {
        let only_administration = catalog(vec![entry(
            "p.desktop",
            "Partitionen",
            &["Settings", "System"],
        )]);
        assert_eq!(
            structure(&model(&only_administration)),
            [group(
                "Systemverwaltung",
                &["Partitionen -> system.launch(p.desktop)"]
            )]
        );
        let only_preferences = catalog(vec![entry("w.desktop", "Wlan", &["Settings"])]);
        assert_eq!(
            structure(&model(&only_preferences)),
            [group(
                "Einstellungen",
                &["Wlan -> system.launch(w.desktop)"]
            )]
        );
    }

    #[test]
    fn no_settings_give_a_placeholder() {
        let no_settings = catalog(vec![
            entry("u.desktop", "Uhr", &["Utility"]),
            entry("m.desktop", "Monitor", &["System", "Monitor"]),
        ]);
        for catalog in [Catalog::default(), no_settings] {
            let menu = model(&catalog);
            assert_eq!(menu.n_items(), 1);
            assert_eq!(label(menu.upcast_ref(), 0), NO_SETTINGS);
            assert!(menu.item_attribute_value(0, "action", None).is_none());
            assert!(menu.item_link(0, "submenu").is_none());
        }
    }

    #[test]
    fn refill_replaces_the_contents_of_the_same_menu() {
        let before = catalog(vec![
            entry("w.desktop", "Wlan", &["Settings"]),
            entry("p.desktop", "Partitionen", &["Settings", "System"]),
        ]);
        // Wlan gone, Partitionen now a preference, Drucker added.
        let after = catalog(vec![
            entry("p.desktop", "Partitionen", &["Settings"]),
            entry("d.desktop", "Drucker", &["Settings", "System"]),
        ]);
        let menu = model(&before);
        let changes = Rc::new(Cell::new(0));
        menu.connect_items_changed(glib::clone!(
            #[strong]
            changes,
            move |_, _, _, _| changes.set(changes.get() + 1)
        ));

        fill(&menu, &after.system_sections());
        assert!(changes.get() > 0);
        assert_eq!(
            structure(&menu),
            [
                group(
                    "Einstellungen",
                    &["Partitionen -> system.launch(p.desktop)"]
                ),
                group("Systemverwaltung", &["Drucker -> system.launch(d.desktop)"]),
            ]
        );

        fill(&menu, &[]);
        assert_eq!(menu.n_items(), 1);
        assert_eq!(label(menu.upcast_ref(), 0), NO_SETTINGS);
        fill(&menu, &before.system_sections());
        assert_eq!(structure(&menu), structure(&model(&before)));
    }
}
