//! The "System" menu as in GNOME 2: the settings from the application
//! catalog, split into "Einstellungen" and "Systemverwaltung" (see
//! [`AppEntry::system_section`]); then "Informationen über Querencia"; then
//! "Abmelden …" if the session can be ended safely (`session.rs`). GTK draws
//! separators between these sections. Settings launch through GIO by
//! desktop ID, and the menu follows changed applications, both exactly like
//! the applications menu, whose code it shares.
//!
//! GNOME 2 also had "Hilfe und Support", "Bildschirm sperren" and
//! "Ausschalten …". They are left out until something real stands behind
//! them: no help browser and no screen locker are installed, and Querencia
//! has no shutdown mechanism yet.

use gtk4::prelude::*;
use gtk4::{MenuButton, Window, gio, glib};

use crate::apps::{AppEntry, Catalog, SystemSection};
use crate::{about, app_menu, session};

const ACTION_GROUP: &str = "system";
const SESSION_GROUP: &str = "session";
const ABOUT: &str = "about";
const LOGOUT: &str = "logout";
const NAME: &str = "Systemmenü";

/// Menu button with the installed settings and the session entries, kept
/// up to date.
pub fn button() -> MenuButton {
    let menu = gio::Menu::new();
    rebuild(&menu);

    let button = app_menu::menu_button("System", None, &menu);
    app_menu::add_launch_action(&button, ACTION_GROUP);

    // One window each, as in GNOME 2; choosing the entry again raises it.
    let about_window = glib::WeakRef::<Window>::new();
    let about = gio::SimpleAction::new(ABOUT, None);
    about.connect_activate(move |_, _| about::present(&about_window));
    let question = glib::WeakRef::<Window>::new();
    let logout = gio::SimpleAction::new(LOGOUT, None);
    logout.connect_activate(move |_, _| match session::logout_command() {
        Some(labwc) => session::confirm_logout(&question, labwc),
        None => eprintln!("querencia-panel: Abmelden nicht möglich: kein labwc (LABWC_PID)"),
    });
    let actions = gio::SimpleActionGroup::new();
    actions.add_action(&about);
    actions.add_action(&logout);
    button.insert_action_group(SESSION_GROUP, Some(&actions));

    app_menu::follow_changes(&button, &menu, NAME, rebuild);
    button
}

/// Reads the catalog again and refills `menu` from it.
fn rebuild(menu: &gio::Menu) {
    let catalog = Catalog::load();
    let sections = catalog.system_sections();
    let logout = session::logout_command();
    let mut summary: Vec<_> = sections
        .iter()
        .map(|(section, entries)| format!("{} {}", section.label(), entries.len()))
        .collect();
    if summary.is_empty() {
        summary.push("keine Einstellungen".to_string());
    }
    summary.push(match &logout {
        Some(labwc) => format!("Abmelden über {} --exit", labwc.display()),
        None => "kein Abmelden (kein labwc in LABWC_PID)".to_string(),
    });
    eprintln!("querencia-panel: {NAME}: {}", summary.join(", "));
    fill(menu, &sections, logout.is_some());
}

/// Replaces the contents of `menu` in place: a section with one submenu
/// per settings section, as [`Catalog::system_sections`] lists them (none
/// without settings), then "Informationen über Querencia", then
/// "Abmelden …" if `logout`. The menu object itself, and with it the
/// button's popover and actions, stays the same.
fn fill(menu: &gio::Menu, sections: &[(SystemSection, Vec<&AppEntry>)], logout: bool) {
    menu.remove_all();
    if !sections.is_empty() {
        let settings = gio::Menu::new();
        for (section, entries) in sections {
            let submenu = gio::Menu::new();
            for entry in entries {
                submenu.append_item(&app_menu::launch_item(entry, ACTION_GROUP));
            }
            let icon = match section {
                SystemSection::Preferences => "preferences-desktop",
                SystemSection::Administration => "preferences-system",
            };
            settings.append_item(&app_menu::submenu_item(section.label(), icon, &submenu));
        }
        menu.append_section(None, &settings);
    }
    let information = gio::Menu::new();
    information.append_item(&action_item(
        "Informationen über Querencia",
        ABOUT,
        "help-about",
    ));
    menu.append_section(None, &information);
    if logout {
        let session = gio::Menu::new();
        session.append_item(&action_item("Abmelden …", LOGOUT, "system-log-out"));
        menu.append_section(None, &session);
    }
}

/// An item `label` for the action `action` of the session group, with the
/// themed icon `icon`.
fn action_item(label: &str, action: &str, icon: &str) -> gio::MenuItem {
    let item = gio::MenuItem::new(Some(label), Some(&format!("{SESSION_GROUP}.{action}")));
    item.set_icon(&gio::ThemedIcon::new(icon));
    item
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

    fn section(menu: &gio::Menu, index: i32) -> gio::Menu {
        menu.item_link(index, "section")
            .expect("section link")
            .downcast::<gio::Menu>()
            .expect("gio::Menu")
    }

    /// The submenus of the settings section, the first one.
    fn model(catalog: &Catalog) -> gio::Menu {
        let menu = gio::Menu::new();
        fill(&menu, &catalog.system_sections(), true);
        section(&menu, 0)
    }

    /// Every item of a section as "label -> action".
    fn actions(section: &gio::Menu) -> Vec<String> {
        (0..section.n_items())
            .map(|i| {
                let action = section
                    .item_attribute_value(i, "action", Some(glib::VariantTy::STRING))
                    .and_then(|v| v.get::<String>())
                    .unwrap_or_default();
                format!("{} -> {action}", label(section.upcast_ref(), i))
            })
            .collect()
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
    fn gnome2_order_settings_then_information_then_session() {
        let catalog = catalog(vec![entry("w.desktop", "Wlan", &["Settings"])]);
        let menu = gio::Menu::new();
        fill(&menu, &catalog.system_sections(), true);
        assert_eq!(menu.n_items(), 3);
        assert_eq!(
            actions(&section(&menu, 1)),
            ["Informationen über Querencia -> session.about"]
        );
        assert_eq!(
            actions(&section(&menu, 2)),
            ["Abmelden … -> session.logout"]
        );
    }

    #[test]
    fn system_entries_carry_their_gnome2_icons() {
        use crate::app_menu::tests::icon;
        let catalog = catalog(vec![
            entry("w.desktop", "Wlan", &["Settings"]),
            entry("p.desktop", "Partitionen", &["Settings", "System"]),
        ]);
        let menu = gio::Menu::new();
        fill(&menu, &catalog.system_sections(), true);
        let settings = section(&menu, 0);
        assert_eq!(
            (
                icon(settings.upcast_ref(), 0),
                icon(settings.upcast_ref(), 1)
            ),
            (
                Some("preferences-desktop".to_string()),
                Some("preferences-system".to_string())
            )
        );
        assert_eq!(
            icon(section(&menu, 1).upcast_ref(), 0).as_deref(),
            Some("help-about")
        );
        assert_eq!(
            icon(section(&menu, 2).upcast_ref(), 0).as_deref(),
            Some("system-log-out")
        );
    }

    #[test]
    fn logout_only_with_a_session_to_end() {
        let menu = gio::Menu::new();
        fill(&menu, &[], false);
        assert_eq!(menu.n_items(), 1);
        assert_eq!(
            actions(&section(&menu, 0)),
            ["Informationen über Querencia -> session.about"]
        );
    }

    #[test]
    fn without_settings_only_information_and_session_remain() {
        // Replaces the placeholder "Keine Einstellungen" of M1.5b: the menu
        // is never empty now, so no placeholder is needed.
        let no_settings = catalog(vec![
            entry("u.desktop", "Uhr", &["Utility"]),
            entry("m.desktop", "Monitor", &["System", "Monitor"]),
        ]);
        for catalog in [Catalog::default(), no_settings] {
            let menu = gio::Menu::new();
            fill(&menu, &catalog.system_sections(), true);
            assert_eq!(menu.n_items(), 2);
            assert_eq!(
                actions(&section(&menu, 0)),
                ["Informationen über Querencia -> session.about"]
            );
            assert_eq!(
                actions(&section(&menu, 1)),
                ["Abmelden … -> session.logout"]
            );
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
        let menu = gio::Menu::new();
        fill(&menu, &before.system_sections(), true);
        let changes = Rc::new(Cell::new(0));
        menu.connect_items_changed(glib::clone!(
            #[strong]
            changes,
            move |_, _, _, _| changes.set(changes.get() + 1)
        ));

        fill(&menu, &after.system_sections(), true);
        assert!(changes.get() > 0);
        assert_eq!(
            structure(&section(&menu, 0)),
            [
                group(
                    "Einstellungen",
                    &["Partitionen -> system.launch(p.desktop)"]
                ),
                group("Systemverwaltung", &["Drucker -> system.launch(d.desktop)"]),
            ]
        );

        fill(&menu, &[], true);
        assert_eq!(menu.n_items(), 2);
        fill(&menu, &before.system_sections(), true);
        assert_eq!(menu.n_items(), 3);
        assert_eq!(structure(&section(&menu, 0)), structure(&model(&before)));
    }
}
