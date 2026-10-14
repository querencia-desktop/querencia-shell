//! The applet types a panel can show. Each type wraps one of the existing
//! panel parts unchanged; its factory builds a fresh widget that releases
//! everything it holds (GIO monitor handler, Wayland connection, timer) when
//! the widget is destroyed, so a panel can later drop an applet by removing
//! its widget. The type IDs are stable: the panel layout and `panel.ini`
//! refer to them.

use gtk4::prelude::*;
use gtk4::{Label, Widget};

use crate::{app_menu, clock, places_menu, system_menu, window_list, workspace_switcher};

/// One applet type.
pub struct AppletType {
    /// Stable ID used in layouts and the configuration file.
    pub id: &'static str,
    /// Name shown to the user, e.g. in the "Zum Panel hinzufügen" dialog.
    pub name: &'static str,
    pub description: &'static str,
    /// Icon name from the freedesktop icon naming specification.
    pub icon: &'static str,
    /// Whether a layout may hold more than one instance. Types with their
    /// own Wayland connection stay single for now: a second instance would
    /// open a second connection and show the same data twice.
    pub multiple: bool,
    factory: fn() -> Widget,
}

impl AppletType {
    /// Builds a new instance of this applet.
    pub fn create(&self) -> Widget {
        (self.factory)()
    }
}

/// All applet types, in the order a later "add to panel" list shows them.
pub const APPLETS: &[AppletType] = &[
    AppletType {
        id: "applications",
        name: "Anwendungen",
        description: "Menü der installierten Anwendungen",
        icon: "start-here",
        multiple: true,
        factory: || app_menu::button().upcast(),
    },
    AppletType {
        id: "places",
        name: "Orte",
        description: "Persönlicher Ordner und Benutzerordner",
        icon: "folder",
        multiple: true,
        factory: || places_menu::button().upcast(),
    },
    AppletType {
        id: "system",
        name: "System",
        description: "Einstellungen und Systemverwaltung",
        icon: "preferences-system",
        multiple: true,
        factory: || system_menu::button().upcast(),
    },
    AppletType {
        id: "window-list",
        name: "Fensterliste",
        description: "Knöpfe für alle offenen Fenster",
        icon: "preferences-system-windows",
        multiple: false,
        factory: || window_list::widget().upcast(),
    },
    AppletType {
        id: "workspace-switcher",
        name: "Arbeitsflächenumschalter",
        description: "Wechsel zwischen den Arbeitsflächen",
        icon: "preferences-desktop-workspaces",
        multiple: false,
        factory: || workspace_switcher::widget().upcast(),
    },
    AppletType {
        id: "clock",
        name: "Uhr",
        description: "Datum und Uhrzeit",
        icon: "preferences-system-time",
        multiple: true,
        factory: || {
            let label = Label::new(None);
            clock::attach(&label);
            label.upcast()
        },
    },
];

/// The applet type with `id`, if there is one.
pub fn find(id: &str) -> Option<&'static AppletType> {
    APPLETS.iter().find(|applet| applet.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn ids_are_unique_and_stable() {
        let ids: Vec<_> = APPLETS.iter().map(|a| a.id).collect();
        assert_eq!(
            ids,
            [
                "applications",
                "places",
                "system",
                "window-list",
                "workspace-switcher",
                "clock"
            ]
        );
        assert_eq!(ids.iter().collect::<HashSet<_>>().len(), ids.len());
        for id in ids {
            assert!(crate::layout::valid_id(id), "{id}");
        }
    }

    #[test]
    fn find_knows_every_type_and_nothing_else() {
        for applet in APPLETS {
            assert_eq!(find(applet.id).map(|a| a.id), Some(applet.id));
            assert!(!applet.name.is_empty() && !applet.description.is_empty());
            assert!(!applet.icon.is_empty());
        }
        assert!(find("unbekannt").is_none());
        assert!(find("").is_none());
    }

    #[test]
    fn wayland_applets_are_single() {
        assert!(!find("window-list").unwrap().multiple);
        assert!(!find("workspace-switcher").unwrap().multiple);
    }
}
