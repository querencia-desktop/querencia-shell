//! The applet types a panel can show. Each type wraps one of the existing
//! panel parts unchanged; its factory builds a fresh widget for one layout
//! instance (only the launcher reads the instance's settings) that releases
//! everything it holds (GIO monitor handler, Wayland connection, timer) when
//! the widget is destroyed, so a panel can later drop an applet by removing
//! its widget. The type IDs are stable: the panel layout and `panel.ini`
//! refer to them.

use gtk4::prelude::*;
use gtk4::{Label, Widget};

use crate::layout::{Instance, LAUNCHER};
use crate::{app_menu, clock, launcher, places_menu, system_menu, window_list, workspace_switcher};

/// One applet type.
pub struct AppletType {
    /// Stable ID used in layouts and the configuration file.
    pub id: &'static str,
    /// Name shown to the user, e.g. in the "Zum Panel hinzufügen" dialog.
    pub name: &'static str,
    pub description: &'static str,
    /// Icon name from the freedesktop icon naming specification.
    pub icon: &'static str,
    /// Icon name the GNOME 2 panel used for this applet, tried before
    /// `icon`. Classic themes such as GNOME Human have these, but not every
    /// freedesktop name (`preferences-system-time`, for example).
    pub classic_icon: Option<&'static str>,
    /// Whether a layout may hold more than one instance. Types with their
    /// own Wayland connection stay single for now: a second instance would
    /// open a second connection and show the same data twice.
    pub multiple: bool,
    /// Adding one first asks for an installed application (the launcher,
    /// HMP-11): the "Zum Panel hinzufügen" dialog then shows its second
    /// page.
    pub chooses_application: bool,
    factory: fn(&Instance) -> Widget,
}

impl AppletType {
    /// Builds the widget of the layout instance `instance` of this type.
    pub fn create(&self, instance: &Instance) -> Widget {
        (self.factory)(instance)
    }
}

/// All applet types. The "Zum Panel hinzufügen" dialog lists them sorted by
/// name.
pub const APPLETS: &[AppletType] = &[
    AppletType {
        id: "applications",
        name: "Anwendungen",
        description: "Menü der installierten Anwendungen",
        icon: "start-here",
        classic_icon: None,
        multiple: true,
        chooses_application: false,
        factory: |_| app_menu::button().upcast(),
    },
    AppletType {
        id: "places",
        name: "Orte",
        description: "Persönlicher Ordner und Benutzerordner",
        icon: "folder",
        classic_icon: None,
        multiple: true,
        chooses_application: false,
        factory: |_| places_menu::button().upcast(),
    },
    AppletType {
        id: "system",
        name: "System",
        description: "Einstellungen und Systemverwaltung",
        icon: "preferences-system",
        classic_icon: None,
        multiple: true,
        chooses_application: false,
        factory: |_| system_menu::button().upcast(),
    },
    AppletType {
        id: "window-list",
        name: "Fensterliste",
        description: "Knöpfe für alle offenen Fenster",
        icon: "preferences-system-windows",
        classic_icon: None,
        multiple: false,
        chooses_application: false,
        factory: |_| window_list::widget().upcast(),
    },
    AppletType {
        id: "workspace-switcher",
        name: "Arbeitsflächenumschalter",
        description: "Wechsel zwischen den Arbeitsflächen",
        icon: "preferences-desktop-workspaces",
        classic_icon: Some("workspace-switcher"),
        multiple: false,
        chooses_application: false,
        factory: |_| workspace_switcher::widget().upcast(),
    },
    AppletType {
        id: "clock",
        name: "Uhr",
        description: "Datum und Uhrzeit",
        icon: "preferences-system-time",
        classic_icon: Some("clock"),
        multiple: true,
        chooses_application: false,
        factory: |_| {
            let label = Label::new(None);
            clock::attach(&label);
            label.upcast()
        },
    },
    AppletType {
        id: LAUNCHER,
        name: "Anwendungsstarter …",
        description: "Einen Starter aus dem Anwendungsmenü kopieren",
        icon: "system-run",
        classic_icon: None,
        multiple: true,
        chooses_application: true,
        factory: |instance| launcher::button(instance.app.as_deref().unwrap_or_default()).upcast(),
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
                "clock",
                "launcher"
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
    fn classic_icons_only_where_the_theme_needs_them() {
        let classic: Vec<_> = APPLETS
            .iter()
            .filter_map(|a| a.classic_icon.map(|icon| (a.id, icon)))
            .collect();
        assert_eq!(
            classic,
            [
                ("workspace-switcher", "workspace-switcher"),
                ("clock", "clock")
            ]
        );
    }

    #[test]
    fn only_the_launcher_asks_for_an_application_and_it_may_repeat() {
        let choosing: Vec<_> = APPLETS
            .iter()
            .filter(|a| a.chooses_application)
            .map(|a| a.id)
            .collect();
        assert_eq!(choosing, [LAUNCHER]);
        assert!(find(LAUNCHER).unwrap().multiple);
    }

    #[test]
    fn wayland_applets_are_single() {
        assert!(!find("window-list").unwrap().multiple);
        assert!(!find("workspace-switcher").unwrap().multiple);
    }
}
