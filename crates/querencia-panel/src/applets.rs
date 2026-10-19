//! The applet types a panel can show. Each type wraps one of the existing
//! panel parts unchanged; its factory builds a fresh widget for one layout
//! instance (only the launcher reads the instance's settings) that releases
//! everything it holds (GIO monitor handler, Wayland connection, timer) when
//! the widget is destroyed, so a panel can later drop an applet by removing
//! its widget. The type IDs are stable: the panel layout and `panel.ini`
//! refer to them.

use gtk4::prelude::*;
use gtk4::{Label, Widget, gio};

use crate::layout::{Instance, LAUNCHER};
use crate::{
    app_menu, clock, launcher, logo, network_applet, places_menu, system_menu, volume_applet,
    window_list, workspace_switcher,
};

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
    /// Offered only while no panel has one (QREL-04d: the clock, as the
    /// owner wants a single clock). Technically several work, and a stored
    /// layout that already has several keeps them; only adding another is
    /// refused (dialog greyed out, [`crate::layout_editor::Editor::add`]).
    pub once: bool,
    /// A right click on it offers "Aus dem Panel entfernen" (`panel_menu.rs`):
    /// launchers and the clock. The other applets have no menu yet.
    pub removable: bool,
    /// Adding one first asks for an installed application (the launcher,
    /// HMP-11): the "Zum Panel hinzufügen" dialog then shows its second
    /// page.
    pub chooses_application: bool,
    /// The applet's own right-click menu (G2-J: the volume applet). Its
    /// popover is parented to the applet's widget (`panel_menu.rs`), so the
    /// items reach the applet's own action group as well as the panel's
    /// `panel.applet-remove` ([`crate::panel_menu::REMOVE_ACTION`]). `None`:
    /// launchers and removable applets get the panel's menus, the others
    /// none.
    pub menu: Option<fn() -> gio::Menu>,
    /// Builds the widget for a layout instance on a panel of the given
    /// height (only "Anwendungen" uses the height, for its logo).
    factory: fn(&Instance, i32) -> Widget,
}

impl AppletType {
    /// Builds the widget of the layout instance `instance` of this type for
    /// a panel `panel_height` pixels high.
    pub fn create(&self, instance: &Instance, panel_height: i32) -> Widget {
        (self.factory)(instance, panel_height)
    }
}

/// All applet types. The "Zum Panel hinzufügen" dialog lists them sorted by
/// name.
pub const APPLETS: &[AppletType] = &[
    AppletType {
        id: "applications",
        name: "Anwendungen",
        description: "Menü der installierten Anwendungen",
        // The Querencia logo (QREL-04c), not the theme's `start-here`.
        icon: logo::ICON_NAME,
        classic_icon: None,
        multiple: true,
        once: false,
        removable: false,
        chooses_application: false,
        menu: None,
        factory: |_, height| app_menu::button(height).upcast(),
    },
    AppletType {
        id: "places",
        name: "Orte",
        description: "Persönlicher Ordner und Benutzerordner",
        icon: "folder",
        classic_icon: None,
        multiple: true,
        once: false,
        removable: false,
        chooses_application: false,
        menu: None,
        factory: |_, _| places_menu::button().upcast(),
    },
    AppletType {
        id: "system",
        name: "System",
        description: "Einstellungen und Systemverwaltung",
        icon: "preferences-system",
        classic_icon: None,
        multiple: true,
        once: false,
        removable: false,
        chooses_application: false,
        menu: None,
        factory: |_, _| system_menu::button().upcast(),
    },
    AppletType {
        id: "window-list",
        name: "Fensterliste",
        description: "Knöpfe für alle offenen Fenster",
        icon: "preferences-system-windows",
        classic_icon: None,
        multiple: false,
        once: false,
        removable: false,
        chooses_application: false,
        menu: None,
        factory: |_, _| window_list::widget().upcast(),
    },
    AppletType {
        id: "workspace-switcher",
        name: "Arbeitsflächenumschalter",
        description: "Wechsel zwischen den Arbeitsflächen",
        icon: "preferences-desktop-workspaces",
        classic_icon: Some("workspace-switcher"),
        multiple: false,
        once: false,
        removable: false,
        chooses_application: false,
        menu: None,
        factory: |_, _| workspace_switcher::widget().upcast(),
    },
    AppletType {
        id: "clock",
        name: "Uhr",
        description: "Datum und Uhrzeit",
        icon: "preferences-system-time",
        classic_icon: Some("clock"),
        multiple: true,
        once: true,
        removable: true,
        chooses_application: false,
        menu: None,
        factory: |_, _| {
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
        once: false,
        removable: true,
        chooses_application: true,
        menu: None,
        factory: |instance, _| {
            launcher::button(instance.app.as_deref().unwrap_or_default()).upcast()
        },
    },
    // G2-J: volume of the default output device (PulseAudio protocol, also
    // PipeWire). Single, like the applets with a connection of their own:
    // a second one would show the same device twice. Not in the standard
    // layout; `panel.ini` for the GNOME 2 place next to the clock:
    // `End=volume-1;clock-1` with `[Applet volume-1]` `Type=volume`.
    AppletType {
        id: VOLUME,
        name: "Lautstärke",
        description: "Lautstärke einstellen und stummschalten",
        // The dialog draws the applet's own icon (`add_dialog.rs`); the
        // theme name is only the fallback for other users of `icon`.
        icon: "audio-volume-high",
        classic_icon: None,
        multiple: false,
        once: false,
        removable: true,
        chooses_application: false,
        menu: Some(volume_applet::menu),
        factory: |_, height| volume_applet::widget(height).upcast(),
    },
    // G2-M: network connections (NetworkManager, else read only from the
    // kernel and systemd-networkd). Single: one connection to the network
    // service per panel. Not in the standard layout; `panel.ini` for the
    // GNOME 2 place right of the volume applet: `End=volume-1;network-1;clock-1`
    // with `[Applet network-1]` `Type=network`.
    AppletType {
        id: NETWORK,
        name: "Netzwerk",
        description: "Netzwerkverbindungen anzeigen und wählen",
        // The dialog draws the applet's own icon (`add_dialog.rs`).
        icon: "network-wired",
        classic_icon: None,
        multiple: false,
        once: false,
        removable: true,
        chooses_application: false,
        menu: Some(network_applet::menu),
        factory: |_, height| network_applet::widget(height).upcast(),
    },
];

/// Type ID of the volume applet (G2-J).
pub const VOLUME: &str = "volume";
/// Type ID of the network applet (G2-M).
pub const NETWORK: &str = "network";

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
                "launcher",
                "volume",
                "network"
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
    fn only_the_clock_is_offered_once_and_launchers_and_clock_are_removable() {
        let once: Vec<_> = APPLETS.iter().filter(|a| a.once).map(|a| a.id).collect();
        assert_eq!(once, ["clock"]);
        let removable: Vec<_> = APPLETS
            .iter()
            .filter(|a| a.removable)
            .map(|a| a.id)
            .collect();
        assert_eq!(removable, ["clock", LAUNCHER, VOLUME, NETWORK]);
    }

    #[test]
    fn only_volume_and_network_have_a_menu_of_their_own_and_are_single() {
        let with_menu: Vec<_> = APPLETS
            .iter()
            .filter(|a| a.menu.is_some())
            .map(|a| a.id)
            .collect();
        assert_eq!(with_menu, [VOLUME, NETWORK]);
        let network = find(NETWORK).unwrap();
        assert!(!network.multiple && network.removable && !network.once);
        assert!(!network.chooses_application);
        assert_eq!(network.name, "Netzwerk");
        let menu = (network.menu.unwrap())();
        let section = menu.item_link(3, "section").expect("Abschnitt");
        let action = section
            .item_attribute_value(0, "action", Some(gtk4::glib::VariantTy::STRING))
            .and_then(|v| v.get::<String>());
        assert_eq!(action.as_deref(), Some(crate::panel_menu::REMOVE_ACTION));
        let volume = find(VOLUME).unwrap();
        assert!(!volume.multiple && volume.removable && !volume.chooses_application);
        assert_eq!(volume.name, "Lautstärke");
        // its menu removes through the panel's action
        let menu = (volume.menu.unwrap())();
        let section = menu.item_link(2, "section").expect("Abschnitt");
        let action = section
            .item_attribute_value(0, "action", Some(gtk4::glib::VariantTy::STRING))
            .and_then(|v| v.get::<String>());
        assert_eq!(action.as_deref(), Some(crate::panel_menu::REMOVE_ACTION));
    }

    #[test]
    fn placement_neighbours_are_known_types() {
        for (kind, left) in crate::layout::RIGHT_OF {
            assert!(
                find(kind).is_some() && find(left).is_some(),
                "{kind} {left}"
            );
        }
    }

    #[test]
    fn wayland_applets_are_single() {
        assert!(!find("window-list").unwrap().multiple);
        assert!(!find("workspace-switcher").unwrap().multiple);
        assert!(!find(VOLUME).unwrap().multiple);
        assert!(!find(NETWORK).unwrap().multiple);
    }
}
