//! The "Anwendungen" menu: one submenu per catalog group, each entry
//! launching its application through GIO by desktop ID. Details and test
//! results: `docs/m1-1b1b-anwendungsmenue-test.md`.

use std::cell::Cell;
use std::rc::Rc;

use gio_unix::DesktopAppInfo;
use gtk4::prelude::*;
use gtk4::{MenuButton, gdk, gio, glib};

use crate::apps::Catalog;

const ACTION_GROUP: &str = "menu";
const LAUNCH: &str = "launch";

/// Menu button with the applications installed when the panel starts.
pub fn button() -> MenuButton {
    let catalog = Catalog::load();
    let held_back = catalog.entries.iter().filter(|e| e.is_settings()).count();
    eprintln!(
        "classic2-panel: Anwendungsmenü mit {} Anwendungen, {held_back} Einstellungen \
         zurückgehalten, {} ausgeschlossen",
        catalog.entries.len() - held_back,
        catalog.excluded.len()
    );

    let button = MenuButton::builder()
        .label("Anwendungen")
        .menu_model(&model(&catalog))
        .has_frame(false)
        .build();

    let launch = gio::SimpleAction::new(LAUNCH, Some(glib::VariantTy::STRING));
    launch.connect_activate(|_, target| {
        if let Some(id) = target.and_then(|t| t.str()) {
            launch_app(id);
        }
    });
    let actions = gio::SimpleActionGroup::new();
    actions.add_action(&launch);
    button.insert_action_group(ACTION_GROUP, Some(&actions));
    button
}

/// One submenu per non-empty group, in menu order. Settings stay in the
/// catalog but are held back for the later "System" menu.
fn model(catalog: &Catalog) -> gio::Menu {
    let model = gio::Menu::new();
    for (group, entries) in catalog.groups() {
        let submenu = gio::Menu::new();
        for entry in entries.iter().filter(|e| !e.is_settings()) {
            let item = gio::MenuItem::new(Some(&entry.name), None);
            item.set_action_and_target_value(
                Some(&format!("{ACTION_GROUP}.{LAUNCH}")),
                Some(&entry.id.to_variant()),
            );
            submenu.append_item(&item);
        }
        if submenu.n_items() > 0 {
            model.append_submenu(Some(group.label()), &submenu);
        }
    }
    model
}

/// Loads the application again by desktop ID, since its desktop file may
/// have changed or vanished since the menu was built, and hands it to GIO.
/// GIO resolves `Exec`, `Terminal=true` and D-Bus activation itself. A
/// successful call only means GIO started a process or sent the activation;
/// whether the application then runs is not known here. Failures are
/// logged and never end the panel.
fn launch_app(id: &str) {
    let Some(app) = DesktopAppInfo::new(id) else {
        eprintln!("classic2-panel: {id} ist nicht mehr vorhanden oder nicht startbar");
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
            Some(pid) => eprintln!("classic2-panel: {id} an GIO übergeben, PID {pid}"),
            None => eprintln!("classic2-panel: {id} an GIO übergeben, ohne PID"),
        },
        Err(err) => eprintln!("classic2-panel: Start von {id} fehlgeschlagen: {err}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apps::{AppEntry, menu_group};

    fn entry(id: &str, name: &str, categories: &[&str]) -> AppEntry {
        AppEntry {
            id: id.into(),
            name: name.into(),
            categories: categories.iter().map(|c| c.to_string()).collect(),
            icon: None,
            group: menu_group(categories),
        }
    }

    fn label(menu: &gio::MenuModel, index: i32) -> String {
        menu.item_attribute_value(index, "label", Some(glib::VariantTy::STRING))
            .and_then(|v| v.get())
            .expect("label")
    }

    /// Each group label with its entries as "label -> action(desktop ID)".
    fn structure(menu: &gio::Menu) -> Vec<(String, Vec<String>)> {
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

    #[test]
    fn groups_become_submenus_and_settings_are_held_back() {
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
        let group = |label: &str, items: &[&str]| {
            (
                label.to_string(),
                items.iter().map(|i| i.to_string()).collect::<Vec<_>>(),
            )
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
                group("Sonstiges", &["Zeug -> menu.launch(z.desktop)"]),
            ]
        );
    }

    #[test]
    fn empty_catalog_gives_empty_menu() {
        assert_eq!(model(&Catalog::default()).n_items(), 0);
    }
}
