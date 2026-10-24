//! The application launcher of the GNOME 2 panel (HMP-11): a small button
//! with the icon of one installed application; a click starts it. The
//! launcher stores nothing but the application's desktop ID (`panel.ini`,
//! `DesktopId`); name, icon and command always come from the installed
//! desktop entry through GIO, exactly as for the "Anwendungen" menu
//! (`app_menu.rs`). So a launcher follows a renamed or re-iconed application
//! without a panel restart, and starting goes through the same
//! [`app_menu::launch_app`] (GIO, `DesktopAppInfo` by ID, no `Exec` path of
//! its own).
//!
//! If the desktop entry is missing (application removed, `Hidden=true`,
//! `TryExec` not found), the launcher stays in the panel with a generic,
//! dimmed icon and says so in its tooltip; a click only logs. It comes back
//! by itself when the application is installed again.

use std::cell::Cell;

use gio_unix::DesktopAppInfo;
use gtk4::prelude::*;
use gtk4::{Button, Image, gio, glib};

use crate::app_menu;
use crate::i18n::gettext_f;

/// CSS class of every launcher button (style in `main.rs`); also marks the
/// widget for the panel's context menu (`panel_menu.rs`).
pub const LAUNCHER_CLASS: &str = "querencia-launcher";
/// CSS class of a launcher whose application is not installed.
pub const MISSING_CLASS: &str = "missing";
/// Icon size on a panel of the standard height: as the "Anwendungen" logo.
const ICON_SIZE: i32 = 22;
/// Icon of an application without a usable `Icon=`, and of a missing one.
const GENERIC_ICON: &str = "application-x-executable";

/// What the launcher shows, as plain data.
#[derive(Debug, PartialEq, Eq)]
pub struct Look {
    /// Serialised `GIcon` of the application, `None` for the generic icon.
    pub icon: Option<String>,
    pub tooltip: String,
    /// The desktop entry is installed and can be started.
    pub available: bool,
}

/// The installed application as far as the launcher needs it.
#[derive(Debug, PartialEq, Eq)]
pub struct AppData {
    pub name: String,
    pub comment: Option<String>,
    pub icon: Option<String>,
}

/// The look of a launcher for `app` (its desktop ID), given what GIO knows
/// about it (`None`: not installed). The tooltip is the name and, below it,
/// the comment, as the GNOME 2 panel showed it.
pub fn look(app: &str, data: Option<&AppData>) -> Look {
    match data {
        Some(data) => Look {
            icon: data.icon.clone(),
            tooltip: match data.comment.as_deref().map(str::trim) {
                Some(comment) if !comment.is_empty() && comment != data.name => {
                    format!("{}\n{comment}", data.name)
                }
                _ => data.name.clone(),
            },
            available: true,
        },
        None => Look {
            icon: None,
            tooltip: gettext_f("Application not found: {app}", &[("app", app)]),
            available: false,
        },
    }
}

/// What GIO knows about the application with desktop ID `app`, read anew.
fn app_data(app: &str) -> Option<AppData> {
    let info = DesktopAppInfo::new(app)?;
    Some(AppData {
        name: info.name().to_string(),
        comment: info.description().map(|c| c.to_string()),
        icon: info
            .icon()
            .and_then(|icon| IconExt::to_string(&icon))
            .map(|icon| icon.to_string()),
    })
}

/// A launcher button for the application with desktop ID `app`.
pub fn button(app: &str) -> Button {
    let image = Image::new();
    image.set_pixel_size(ICON_SIZE);
    let button = Button::builder().has_frame(false).child(&image).build();
    // Added, not set: keeps GTK's own classes.
    button.add_css_class(LAUNCHER_CLASS);
    refresh(&button, &image, app);

    let id = app.to_string();
    button.connect_clicked(move |_| {
        eprintln!("querencia-panel: Starter {id} angeklickt");
        app_menu::launch_app(&id);
    });

    // Follows installed, removed and changed applications. The handler
    // holds the widgets weakly; the button drops it when destroyed.
    let monitor = gio::AppInfoMonitor::get();
    let id = app.to_string();
    let changed = monitor.connect_changed(glib::clone!(
        #[weak]
        button,
        #[weak]
        image,
        move |_| {
            refresh(&button, &image, &id);
            // GIO reports a change only once until the applications are
            // listed again; the menus do that too, but a panel may have
            // launchers without an "Anwendungen" menu.
            let _ = gio::AppInfo::all();
        }
    ));
    let changed = Cell::new(Some(changed));
    button.connect_destroy(move |_| {
        if let Some(id) = changed.take() {
            monitor.disconnect(id);
        }
    });
    button
}

/// Shows the current state of `app` on `button` and `image`.
fn refresh(button: &Button, image: &Image, app: &str) {
    let look = look(app, app_data(app).as_ref());
    let icon = look
        .icon
        .as_deref()
        .and_then(|icon| gio::Icon::for_string(icon).ok())
        .unwrap_or_else(|| gio::ThemedIcon::new(GENERIC_ICON).upcast());
    crate::symbole::set(image, &icon);
    button.set_tooltip_text(Some(&look.tooltip));
    if look.available {
        button.remove_css_class(MISSING_CLASS);
    } else {
        button.add_css_class(MISSING_CLASS);
    }
    eprintln!(
        "querencia-panel: Starter {app}: {}",
        if look.available {
            look.tooltip.lines().next().unwrap_or_default().to_string()
        } else {
            "nicht installiert".to_string()
        }
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data(name: &str, comment: Option<&str>, icon: Option<&str>) -> AppData {
        AppData {
            name: name.into(),
            comment: comment.map(str::to_string),
            icon: icon.map(str::to_string),
        }
    }

    #[test]
    fn tooltip_is_name_and_comment_like_gnome2() {
        let firefox = data("Firefox", Some("Im Internet surfen"), Some("firefox"));
        assert_eq!(
            look("firefox_firefox.desktop", Some(&firefox)),
            Look {
                icon: Some("firefox".into()),
                tooltip: "Firefox\nIm Internet surfen".into(),
                available: true,
            }
        );
        // No comment, an empty one or one equal to the name: name only.
        for comment in [None, Some(""), Some("  "), Some("Beispiel")] {
            let beispiel = data("Beispiel", comment, None);
            assert_eq!(
                look("beispiel.desktop", Some(&beispiel)).tooltip,
                "Beispiel"
            );
        }
    }

    #[test]
    fn a_missing_application_keeps_its_launcher_with_the_generic_icon() {
        let missing = look("weg.desktop", None);
        assert!(!missing.available);
        assert_eq!(missing.icon, None);
        assert!(
            missing.tooltip.contains("weg.desktop"),
            "{}",
            missing.tooltip
        );
    }
}
