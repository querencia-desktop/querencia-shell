//! The "Anwendungen" menu: one submenu per catalog group (GNOME 2 groups,
//! see `apps.rs`), each entry launching its application through GIO by
//! desktop ID. The menu follows installed, removed and changed applications
//! without a panel restart. [`menu_button`] gives all three main menus their
//! GNOME 2 button: label (and for "Anwendungen" the logo), no dropdown
//! arrow, menu left-aligned below the button.
//! Launching and following changes are shared with the "System" menu
//! (`system_menu.rs`). Details and test results:
//! `docs/m1-1b1b-anwendungsmenue-test.md`, `docs/m1-1b1c-appinfo-monitor-test.md`.

use std::cell::Cell;
use std::rc::Rc;

use gio_unix::DesktopAppInfo;
use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Image, Label, MenuButton, Orientation, Popover, gdk, gio, glib};

use crate::apps::{AppEntry, Catalog};

const ACTION_GROUP: &str = "menu";
const LAUNCH: &str = "launch";
const NAME: &str = "Anwendungsmenü";
const NO_APPLICATIONS: &str = "Keine Anwendungen";

/// Menu button with the installed applications, kept up to date.
pub fn button() -> MenuButton {
    let menu = gio::Menu::new();
    rebuild(&menu);

    let button = menu_button("Anwendungen", Some(&APPLICATIONS_ICON), &menu);
    add_launch_action(&button, ACTION_GROUP);
    follow_changes(&button, &menu, NAME, rebuild);
    button
}

/// Icon of the "Anwendungen" button, as the GNOME 2 panel showed it before
/// the label: the distribution or GNOME logo (`start-here`; GNOME Human
/// inherits it from `gnome`), else the generic names.
pub(crate) const APPLICATIONS_ICON: [&str; 3] =
    ["start-here", "distributor-logo", "start-here-symbolic"];
/// Size of that icon on a panel of the standard height.
const PANEL_ICON_SIZE: i32 = 22;
/// CSS class of the popovers of the three main menus (style in `main.rs`).
pub(crate) const MENU_CLASS: &str = "querencia-menu";

/// A panel menu button for `menu` in the GNOME 2 manner: the label, with
/// `icon` (names tried in order) before it if given, and no dropdown arrow;
/// the menu opens flush below or above the button, left-aligned, without
/// the popover's pointer, styled by [`MENU_CLASS`].
pub(crate) fn menu_button(label: &str, icon: Option<&[&str]>, menu: &gio::Menu) -> MenuButton {
    let content = GtkBox::new(Orientation::Horizontal, 4);
    if let Some(names) = icon {
        let image = Image::from_gicon(&gio::ThemedIcon::from_names(names));
        image.set_pixel_size(PANEL_ICON_SIZE);
        content.append(&image);
    }
    content.append(&Label::new(Some(label)));
    let button = MenuButton::builder()
        .menu_model(menu)
        .has_frame(false)
        .build();
    // A custom child shows no dropdown arrow (always-show-arrow stays off).
    button.set_child(Some(&content));
    let popover = button.popover().expect("menu button with a menu model");
    popover.set_has_arrow(false);
    popover.set_halign(Align::Start);
    popover.add_css_class(MENU_CLASS);
    button
}

/// Reads the catalog again and refills `menu` from it.
fn rebuild(menu: &gio::Menu) {
    let catalog = Catalog::load();
    let settings = catalog.entries.iter().filter(|e| e.is_settings()).count();
    eprintln!(
        "querencia-panel: {NAME} mit {} Anwendungen, {settings} Einstellungen \
         im Systemmenü, {} ausgeschlossen",
        catalog.entries.len() - settings,
        catalog.excluded.len()
    );
    fill(menu, &catalog);
}

/// Gives `button` its own action group `group` with the action `launch`,
/// which launches the desktop ID it gets as target through [`launch_app`].
/// Each menu has its own group, so its items name it unambiguously.
pub(crate) fn add_launch_action(button: &MenuButton, group: &str) {
    let launch = gio::SimpleAction::new(LAUNCH, Some(glib::VariantTy::STRING));
    launch.connect_activate(|_, target| {
        if let Some(id) = target.and_then(|t| t.str()) {
            launch_app(id);
        }
    });
    let actions = gio::SimpleActionGroup::new();
    actions.add_action(&launch);
    button.insert_action_group(group, Some(&actions));
}

/// Menu item named after `entry` that launches it through the `launch`
/// action of `group` (see [`add_launch_action`]).
pub(crate) fn launch_item(entry: &AppEntry, group: &str) -> gio::MenuItem {
    let item = gio::MenuItem::new(Some(&entry.name), None);
    item.set_action_and_target_value(
        Some(&format!("{group}.{LAUNCH}")),
        Some(&entry.id.to_variant()),
    );
    item
}

/// Calls `rebuild` for the menu when GIO reports changed applications. An
/// open menu is never rebuilt under the user's hands: the change is noted
/// and applied once the popover has closed. GIO fires `changed` only once
/// until the applications are read again, so any number of changes while
/// the menu is open ends in one rebuild, and the rebuild itself re-arms the
/// monitor.
///
/// Every menu that follows changes has its own handler and its own note, so
/// one menu is rebuilt while another one is open and waits; whichever reads
/// the catalog first re-arms the monitor for all of them. `name` is the
/// menu's name in the log.
///
/// The handlers hold the popover and the menu weakly, so the monitor, which
/// lives for the whole process, keeps no widget alive; the button drops its
/// monitor handler when it is destroyed.
pub(crate) fn follow_changes(
    button: &MenuButton,
    menu: &gio::Menu,
    name: &'static str,
    rebuild: fn(&gio::Menu),
) {
    // The button created its popover when it got the menu model; the model is
    // only ever changed in place, so this popover stays for the button's life.
    let popover = button.popover().expect("menu button with a menu model");
    let pending = Rc::new(Cell::new(false));

    let monitor = gio::AppInfoMonitor::get();
    let changed = monitor.connect_changed(glib::clone!(
        #[weak]
        popover,
        #[weak]
        menu,
        #[strong]
        pending,
        move |_| update(&popover, &menu, &pending, name, rebuild)
    ));
    let changed = Cell::new(Some(changed));
    button.connect_destroy(move |_| {
        if let Some(id) = changed.take() {
            monitor.disconnect(id);
        }
    });

    popover.connect_closed(glib::clone!(
        #[weak]
        menu,
        move |popover| {
            if pending.get() {
                // Not while the popover is still handling the click or key
                // that closed it.
                glib::idle_add_local_once(glib::clone!(
                    #[weak]
                    popover,
                    #[weak]
                    menu,
                    #[strong]
                    pending,
                    move || {
                        if pending.get() {
                            update(&popover, &menu, &pending, name, rebuild);
                        }
                    }
                ));
            }
        }
    ));
}

fn update(
    popover: &Popover,
    menu: &gio::Menu,
    pending: &Cell<bool>,
    name: &str,
    rebuild: fn(&gio::Menu),
) {
    if popover.is_visible() {
        if !pending.replace(true) {
            eprintln!(
                "querencia-panel: Anwendungen geändert, {name} ist offen, Aktualisierung vorgemerkt"
            );
        }
        return;
    }
    pending.set(false);
    eprintln!("querencia-panel: Anwendungen geändert, {name} wird neu aufgebaut");
    rebuild(menu);
}

/// Replaces the contents of `menu` in place: one submenu per non-empty
/// group, in menu order. Settings stay in the catalog but are left to the
/// "System" menu. Without any application the menu holds one item without
/// action instead of an empty popover; activating it does nothing. The menu
/// object itself, and with it the button's popover and actions, stays the
/// same.
fn fill(menu: &gio::Menu, catalog: &Catalog) {
    menu.remove_all();
    for (group, entries) in catalog.groups() {
        let submenu = gio::Menu::new();
        for entry in entries.iter().filter(|e| !e.is_settings()) {
            submenu.append_item(&launch_item(entry, ACTION_GROUP));
        }
        if submenu.n_items() > 0 {
            menu.append_submenu(Some(group.label()), &submenu);
        }
    }
    if menu.n_items() == 0 {
        menu.append(Some(NO_APPLICATIONS), None);
    }
}

/// Loads the application again by desktop ID, since its desktop file may
/// have changed or vanished since the menu was built, and hands it to GIO.
/// GIO resolves `Exec`, `Terminal=true` and D-Bus activation itself. A
/// successful call only means GIO started a process or sent the activation;
/// whether the application then runs is not known here. Failures are
/// logged and never end the panel.
pub(crate) fn launch_app(id: &str) {
    let Some(app) = DesktopAppInfo::new(id) else {
        eprintln!("querencia-panel: {id} ist nicht mehr vorhanden oder nicht startbar");
        return;
    };

    // The display's context passes the Wayland activation token on, so the
    // compositor may focus the new window.
    let context = gdk::Display::default().map(|display| display.app_launch_context());
    let pid = Rc::new(Cell::new(None));
    if let Some(context) = &context {
        let pid = pid.clone();
        context.connect_launched(move |_, _, platform_data| {
            let data = glib::VariantDict::new(Some(platform_data));
            pid.set(data.lookup::<i32>("pid").ok().flatten());
        });
    }

    match app.launch(&[], context.as_ref()) {
        Ok(()) => match pid.get() {
            Some(pid) => eprintln!("querencia-panel: {id} an GIO übergeben, PID {pid}"),
            None => eprintln!("querencia-panel: {id} an GIO übergeben, ohne PID"),
        },
        Err(err) => eprintln!("querencia-panel: Start von {id} fehlgeschlagen: {err}"),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::apps::menu_group;

    pub(crate) fn entry(id: &str, name: &str, categories: &[&str]) -> AppEntry {
        AppEntry {
            id: id.into(),
            name: name.into(),
            categories: categories.iter().map(|c| c.to_string()).collect(),
            icon: None,
            group: menu_group(categories),
        }
    }

    fn model(catalog: &Catalog) -> gio::Menu {
        let menu = gio::Menu::new();
        fill(&menu, catalog);
        menu
    }

    pub(crate) fn label(menu: &gio::MenuModel, index: i32) -> String {
        menu.item_attribute_value(index, "label", Some(glib::VariantTy::STRING))
            .and_then(|v| v.get())
            .expect("label")
    }

    /// Each submenu label with its entries as "label -> action(desktop ID)".
    pub(crate) fn structure(menu: &gio::Menu) -> Vec<(String, Vec<String>)> {
        (0..menu.n_items())
            .map(|g| {
                let submenu = menu.item_link(g, "submenu").expect("submenu link");
                let items = (0..submenu.n_items())
                    .map(|i| {
                        let attr = |name| {
                            submenu
                                .item_attribute_value(i, name, Some(glib::VariantTy::STRING))
                                .and_then(|v| v.get::<String>())
                                .unwrap_or_else(|| panic!("{name} fehlt"))
                        };
                        format!(
                            "{} -> {}({})",
                            attr("label"),
                            attr("action"),
                            attr("target")
                        )
                    })
                    .collect();
                (label(menu.upcast_ref(), g), items)
            })
            .collect()
    }

    pub(crate) fn group(label: &str, items: &[&str]) -> (String, Vec<String>) {
        (
            label.to_string(),
            items.iter().map(|i| i.to_string()).collect(),
        )
    }

    #[test]
    fn applications_button_shows_the_classic_logo_first() {
        assert_eq!(
            APPLICATIONS_ICON,
            ["start-here", "distributor-logo", "start-here-symbolic"]
        );
    }

    #[test]
    fn groups_become_submenus_and_settings_are_left_out() {
        // Entries as Catalog::load sorts them: by group, then by name.
        let catalog = Catalog {
            entries: vec![
                entry("b.desktop", "Bearbeiter", &["Utility"]),
                entry("o.desktop", "Ölwechsel", &["Utility"]),
                // A group holding only settings disappears from the menu.
                entry("g.desktop", "Farbprofil", &["Settings", "Graphics"]),
                entry("s.desktop", "Systemmonitor", &["System"]),
                entry("x.desktop", "Hardware", &["Settings", "System"]),
                entry("d.desktop", "Desktop", &["Settings", "DesktopSettings"]),
                entry("z.desktop", "Zeug", &[]),
            ],
            excluded: Vec::new(),
        };
        assert_eq!(
            structure(&model(&catalog)),
            [
                group(
                    "Zubehör",
                    &[
                        "Bearbeiter -> menu.launch(b.desktop)",
                        "Ölwechsel -> menu.launch(o.desktop)"
                    ]
                ),
                group(
                    "Systemwerkzeuge",
                    &["Systemmonitor -> menu.launch(s.desktop)"]
                ),
                group("Sonstige", &["Zeug -> menu.launch(z.desktop)"]),
            ]
        );
    }

    #[test]
    fn empty_catalog_gives_empty_menu() {
        // Empty means no application; the only item is a placeholder without action.
        let only_settings = Catalog {
            entries: vec![entry("x.desktop", "Hardware", &["Settings", "System"])],
            excluded: Vec::new(),
        };
        for catalog in [Catalog::default(), only_settings] {
            let menu = model(&catalog);
            assert_eq!(menu.n_items(), 1);
            assert_eq!(label(menu.upcast_ref(), 0), NO_APPLICATIONS);
            assert!(menu.item_attribute_value(0, "action", None).is_none());
            assert!(menu.item_link(0, "submenu").is_none());
        }
    }

    #[test]
    fn refill_replaces_the_contents_of_the_same_menu() {
        let before = Catalog {
            entries: vec![
                entry("a.desktop", "Abakus", &["Utility"]),
                entry("r.desktop", "Rechner", &["Utility"]),
                entry("w.desktop", "Weg", &["Utility"]),
                entry("t.desktop", "Text", &["Office"]),
            ],
            excluded: Vec::new(),
        };
        // Weg removed, Rechner renamed to Addierer, Text moved from Büro to
        // Entwicklung, Neu added; sorted as Catalog::load sorts.
        let after = Catalog {
            entries: vec![
                entry("a.desktop", "Abakus", &["Utility"]),
                entry("r.desktop", "Addierer", &["Utility"]),
                entry("t.desktop", "Text", &["Development"]),
                entry("n.desktop", "Neu", &[]),
            ],
            excluded: Vec::new(),
        };
        let menu = model(&before);
        let changes = Rc::new(Cell::new(0));
        menu.connect_items_changed(glib::clone!(
            #[strong]
            changes,
            move |_, _, _, _| changes.set(changes.get() + 1)
        ));

        fill(&menu, &after);
        // The popover follows the same menu object through items-changed.
        assert!(changes.get() > 0);
        assert_eq!(
            structure(&menu),
            [
                group(
                    "Zubehör",
                    &[
                        "Abakus -> menu.launch(a.desktop)",
                        "Addierer -> menu.launch(r.desktop)"
                    ]
                ),
                group("Entwicklung", &["Text -> menu.launch(t.desktop)"]),
                group("Sonstige", &["Neu -> menu.launch(n.desktop)"]),
            ]
        );

        fill(&menu, &Catalog::default());
        assert_eq!(label(menu.upcast_ref(), 0), NO_APPLICATIONS);
        assert_eq!(menu.n_items(), 1);
        fill(&menu, &before);
        assert_eq!(structure(&menu), structure(&model(&before)));
    }
}
