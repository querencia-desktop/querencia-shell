//! The panel's own context menu, as in the GNOME 2 panel: a right click on
//! free panel space opens it at the pointer. A right click on an applet is
//! left to the applet (launchers have a menu, the others none yet, so it
//! does nothing there). The window list fills the bottom panel; the space
//! between and after its buttons counts as free panel space
//! ([`FREE_SPACE_CLASS`], QREL-04d), so the bottom panel has its menu too.
//! Left and middle clicks never reach this code.
//!
//! The menu is a `PopoverMenu` like the menus of the applets, so it takes
//! the keyboard the same way: the panel keeps `KeyboardMode::None`, labwc
//! gives the popover's grab the keyboard while it is open (Escape closes it)
//! and returns it to the previously focused window afterwards.
//!
//! A right click on an application launcher (HMP-11) opens the launcher's
//! menu instead, as in GNOME 2: "Starten" and "Aus dem Panel entfernen";
//! on the clock (QREL-04d) a menu with "Aus dem Panel entfernen" only, so
//! the single clock can be removed and added again, e.g. to the other panel.
//! Removing goes through the shared [`Editor`] too: saved first, then taken
//! off the screen.
//!
//! "Zum Panel hinzufügen …" adds to the region of the free space that was
//! right-clicked (left, middle or right third of the panel, see
//! [`Region::at`]), next to that space, as GNOME 2 adds where the menu was
//! opened. The change goes through the shared [`Editor`], which saves it,
//! and then into the running panel ([`panel::insert_applet`]).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, CenterBox, EventSequenceState, GestureClick, PickFlags,
    PopoverMenu, PositionType, Window, gdk, gio, glib,
};

use crate::layout::{Edge, PanelSpec, Region};
use crate::layout_editor::Editor;
use crate::{add_dialog, app_menu, applets, panel};

/// CSS class of the top widget of every applet; marks where free panel
/// space ends.
pub const APPLET_CLASS: &str = "querencia-applet";
/// CSS class of an applet widget whose own background counts as free panel
/// space (QREL-04d): the window list fills the bottom panel, and a right
/// click between or after its buttons opens the panel menu, as on a GNOME 2
/// panel. Its buttons and all other applets keep their own clicks. No style
/// rule uses the class.
pub const FREE_SPACE_CLASS: &str = "querencia-free-space";

const ACTION_GROUP: &str = "panel";
const ADD: &str = "add";
const LAUNCH: &str = "launcher-start";
const REMOVE: &str = "applet-remove";

/// The menu entries. More follow with the functions behind them (removing,
/// moving, properties); until then the menu shows nothing that would not
/// work.
pub fn model() -> gio::Menu {
    let menu = gio::Menu::new();
    menu.append(
        Some("Zum Panel hinzufügen …"),
        Some(&format!("{ACTION_GROUP}.{ADD}")),
    );
    menu
}

/// The menu of a launcher (GNOME 2: "Starten", then the panel object
/// entries; only removing is offered, as moving and properties do not exist
/// yet).
pub fn launcher_model() -> gio::Menu {
    let menu = gio::Menu::new();
    menu.append(Some("Starten"), Some(&format!("{ACTION_GROUP}.{LAUNCH}")));
    let object = gio::Menu::new();
    object.append(
        Some("Aus dem Panel entfernen"),
        Some(&format!("{ACTION_GROUP}.{REMOVE}")),
    );
    menu.append_section(None, &object);
    menu
}

/// The menu of a removable applet without other entries (QREL-04d: the
/// clock): "Aus dem Panel entfernen".
pub fn applet_model() -> gio::Menu {
    let menu = gio::Menu::new();
    menu.append(
        Some("Aus dem Panel entfernen"),
        Some(&format!("{ACTION_GROUP}.{REMOVE}")),
    );
    menu
}

/// Gives the panel `window` with the bar `bar` its context menu.
pub fn attach(
    app: &Application,
    window: &ApplicationWindow,
    bar: &CenterBox,
    spec: &PanelSpec,
    editor: &Rc<RefCell<Editor>>,
) {
    // Region of the free space last right-clicked: where the dialog adds.
    let target = Rc::new(Cell::new(Region::Start));
    let panel_id = spec.id.clone();
    let hooks = Rc::new(add_dialog::Hooks {
        available: Box::new(glib::clone!(
            #[strong]
            editor,
            move |applet| editor.borrow().can_add(applet)
        )),
        add: Box::new(glib::clone!(
            #[strong]
            editor,
            #[strong]
            target,
            #[weak]
            bar,
            #[upgrade_or_else]
            || Err("Das Panel gibt es nicht mehr".to_string()),
            move |applet, app| add_applet(&editor, &bar, &panel_id, applet, app, target.get())
        )),
    });

    // One "Zum Panel hinzufügen" dialog per panel, as in GNOME 2; opening it
    // again raises the open one.
    let dialog = glib::WeakRef::<Window>::new();
    let add = gio::SimpleAction::new(ADD, None);
    let panel_id = spec.id.clone();
    add.connect_activate(glib::clone!(
        #[weak]
        app,
        move |_, _| add_dialog::present(&app, &dialog, &panel_id, &hooks)
    ));
    // The launcher last right-clicked (its instance ID): what "Starten" and
    // "Aus dem Panel entfernen" act on.
    let clicked = Rc::new(RefCell::new(String::new()));
    let launch = gio::SimpleAction::new(LAUNCH, None);
    launch.connect_activate(glib::clone!(
        #[strong]
        editor,
        #[strong]
        clicked,
        move |_, _| {
            let app = editor
                .borrow()
                .layout()
                .panels
                .iter()
                .flat_map(|p| p.instances())
                .find(|i| i.id == *clicked.borrow())
                .and_then(|i| i.app.clone());
            if let Some(app) = app {
                app_menu::launch_app(&app);
            }
        }
    ));
    let remove = gio::SimpleAction::new(REMOVE, None);
    remove.connect_activate(glib::clone!(
        #[strong]
        editor,
        #[strong]
        clicked,
        #[weak]
        bar,
        move |_, _| {
            let id = clicked.borrow().clone();
            if let Err(err) = remove_applet(&editor, &bar, &id) {
                eprintln!("querencia-panel: {id} nicht entfernt: {err}");
            }
        }
    ));
    let actions = gio::SimpleActionGroup::new();
    actions.add_action(&add);
    actions.add_action(&launch);
    actions.add_action(&remove);
    bar.insert_action_group(ACTION_GROUP, Some(&actions));

    // A single popover per panel: a second right click cannot stack menus.
    let popover = PopoverMenu::from_model(Some(&model()));
    popover.set_has_arrow(false);
    popover.set_halign(Align::Start);
    popover.set_position(match spec.edge {
        Edge::Top => PositionType::Bottom,
        Edge::Bottom => PositionType::Top,
    });
    popover.set_parent(bar);
    let launcher_popover = PopoverMenu::from_model(Some(&launcher_model()));
    launcher_popover.set_has_arrow(false);
    launcher_popover.set_halign(Align::Start);
    launcher_popover.set_position(popover.position());
    launcher_popover.set_parent(bar);
    let applet_popover = PopoverMenu::from_model(Some(&applet_model()));
    applet_popover.set_has_arrow(false);
    applet_popover.set_halign(Align::Start);
    applet_popover.set_position(popover.position());
    applet_popover.set_parent(bar);
    bar.connect_destroy(glib::clone!(
        #[strong]
        popover,
        #[strong]
        launcher_popover,
        #[strong]
        applet_popover,
        move |_| {
            popover.unparent();
            launcher_popover.unparent();
            applet_popover.unparent();
        }
    ));

    // On the window, not the bar, so that the bar's side margins count as
    // panel space too. Only the secondary button; the gesture sees the click
    // after the applet below the pointer (bubble phase).
    let click = GestureClick::new();
    click.set_button(gdk::BUTTON_SECONDARY);
    let panel_id = spec.id.clone();
    click.connect_pressed(glib::clone!(
        #[weak]
        window,
        #[weak]
        bar,
        #[weak]
        popover,
        #[weak]
        launcher_popover,
        #[weak]
        applet_popover,
        #[strong]
        target,
        #[strong]
        clicked,
        #[strong]
        editor,
        move |gesture, presses, x, y| {
            // Pointer events inside a popover, e.g. an open applet menu,
            // bubble up to the panel too; only clicks on the panel surface
            // itself count. Their coordinates are the panel's own.
            let on_panel =
                gesture.current_event().and_then(|event| event.surface()) == window.surface();
            let open = [&popover, &launcher_popover, &applet_popover]
                .iter()
                .any(|p| p.is_visible());
            if presses != 1 || !on_panel || open {
                return;
            }
            let applet = applet_at(&window, x, y);
            let Some((x, y)) = window.translate_coordinates(&bar, x, y) else {
                return;
            };
            let pointer = gdk::Rectangle::new(x as i32, y as i32, 1, 1);
            if let Some(applet) = applet {
                // The applet's type from the layout, by its instance ID.
                let id = applet.widget_name().to_string();
                let kind = editor
                    .borrow()
                    .layout()
                    .panels
                    .iter()
                    .flat_map(|p| p.instances())
                    .find(|i| i.id == id)
                    .and_then(|i| applets::find(&i.applet));
                let menu = match kind {
                    Some(t) if t.id == crate::layout::LAUNCHER => Some(&launcher_popover),
                    Some(t) if t.removable => Some(&applet_popover),
                    _ => None,
                };
                if let Some(menu) = menu {
                    *clicked.borrow_mut() = id;
                    gesture.set_state(EventSequenceState::Claimed);
                    menu.set_pointing_to(Some(&pointer));
                    menu.popup();
                    eprintln!(
                        "querencia-panel: Appletmenü von {} geöffnet",
                        clicked.borrow()
                    );
                }
                return;
            }
            target.set(Region::at(x, f64::from(bar.width())));
            gesture.set_state(EventSequenceState::Claimed);
            popover.set_pointing_to(Some(&pointer));
            popover.popup();
            eprintln!(
                "querencia-panel: Kontextmenü von Panel {panel_id} geöffnet (Bereich {})",
                target.get().label()
            );
        }
    ));
    window.add_controller(click);
}

/// Adds an `applet` instance (for a launcher: of the application with
/// desktop ID `app`) to `region` of the panel `panel_id` with the bar `bar`:
/// first in the layout and `panel.ini` through `editor`, then, once saved,
/// on screen.
fn add_applet(
    editor: &RefCell<Editor>,
    bar: &CenterBox,
    panel_id: &str,
    applet: &str,
    app: Option<&str>,
    region: Region,
) -> Result<(), String> {
    let (change, path, height) = {
        let mut editor = editor.borrow_mut();
        let height = editor
            .layout()
            .panels
            .iter()
            .find(|p| p.id == panel_id)
            .map_or(crate::layout::DEFAULT_HEIGHT, |p| p.height);
        let change = match app {
            Some(app) if applet == crate::layout::LAUNCHER => {
                editor.add_launcher(panel_id, app, region)?
            }
            Some(app) => return Err(format!("{applet} braucht keine Anwendung ({app})")),
            None => editor.add(panel_id, applet, region)?,
        };
        (change, editor.path().to_path_buf(), height)
    };
    if let Some(backup) = &change.backup {
        eprintln!(
            "querencia-panel: bisherige {} gesichert als {}",
            path.display(),
            backup.display()
        );
    }
    let added = &change.added;
    if !panel::insert_applet(bar, added.region, added.index, &added.instance, height) {
        return Err(format!(
            "gespeichert, aber nicht angezeigt: Applet-Typ {} unbekannt",
            added.instance.applet
        ));
    }
    eprintln!(
        "querencia-panel: {} ({}{}) zu Panel {panel_id} hinzugefügt, Bereich {}, Position {}; gespeichert in {}",
        added.instance.id,
        added.instance.applet,
        added
            .instance
            .app
            .as_deref()
            .map(|app| format!(" {app}"))
            .unwrap_or_default(),
        added.region.label(),
        added.index + 1,
        path.display()
    );
    Ok(())
}

/// Removes the applet instance `instance_id` from the panel bar `bar`:
/// first from the layout and `panel.ini` through `editor`, then, once
/// saved, from the screen.
fn remove_applet(
    editor: &RefCell<Editor>,
    bar: &CenterBox,
    instance_id: &str,
) -> Result<(), String> {
    let (removed, path) = {
        let mut editor = editor.borrow_mut();
        let removed = editor.remove(instance_id)?;
        (removed, editor.path().to_path_buf())
    };
    if let Some(backup) = &removed.backup {
        eprintln!(
            "querencia-panel: bisherige {} gesichert als {}",
            path.display(),
            backup.display()
        );
    }
    if !panel::remove_applet(bar, instance_id) {
        return Err("gespeichert, aber nicht in diesem Panel angezeigt".to_string());
    }
    eprintln!(
        "querencia-panel: {instance_id} ({}) aus dem Panel entfernt; gespeichert in {}",
        removed.instance.applet,
        path.display()
    );
    Ok(())
}

/// The top widget of the applet at the point (window coordinates), `None`
/// on free panel space: outside every applet, or directly on an applet's
/// own background marked with [`FREE_SPACE_CLASS`] (not on its children).
fn applet_at(window: &ApplicationWindow, x: f64, y: f64) -> Option<gtk4::Widget> {
    let mut hit = window.pick(x, y, PickFlags::DEFAULT);
    if hit
        .as_ref()
        .is_some_and(|w| w.has_css_class(FREE_SPACE_CLASS))
    {
        return None;
    }
    while let Some(widget) = hit {
        if widget.has_css_class(APPLET_CLASS) {
            return Some(widget);
        }
        hit = widget.parent();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_offers_only_what_works() {
        let menu = model();
        assert_eq!(menu.n_items(), 1);
        let label = menu.item_attribute_value(0, "label", Some(glib::VariantTy::STRING));
        assert_eq!(
            label.and_then(|v| v.get::<String>()).as_deref(),
            Some("Zum Panel hinzufügen …")
        );
        let action = menu.item_attribute_value(0, "action", Some(glib::VariantTy::STRING));
        assert_eq!(
            action.and_then(|v| v.get::<String>()).as_deref(),
            Some("panel.add")
        );
    }

    #[test]
    fn launcher_menu_starts_and_removes() {
        let menu = launcher_model();
        let label = |menu: &gio::Menu, i| {
            menu.item_attribute_value(i, "label", Some(glib::VariantTy::STRING))
                .and_then(|v| v.get::<String>())
        };
        let action = |menu: &gio::Menu, i| {
            menu.item_attribute_value(i, "action", Some(glib::VariantTy::STRING))
                .and_then(|v| v.get::<String>())
        };
        assert_eq!(label(&menu, 0).as_deref(), Some("Starten"));
        assert_eq!(action(&menu, 0).as_deref(), Some("panel.launcher-start"));
        let section = menu
            .item_link(1, "section")
            .and_downcast::<gio::Menu>()
            .expect("Abschnitt");
        assert_eq!(
            label(&section, 0).as_deref(),
            Some("Aus dem Panel entfernen")
        );
        assert_eq!(action(&section, 0).as_deref(), Some("panel.applet-remove"));
    }

    #[test]
    fn clock_menu_only_removes() {
        let menu = applet_model();
        assert_eq!(menu.n_items(), 1);
        let label = menu
            .item_attribute_value(0, "label", Some(glib::VariantTy::STRING))
            .and_then(|v| v.get::<String>());
        assert_eq!(label.as_deref(), Some("Aus dem Panel entfernen"));
        let action = menu
            .item_attribute_value(0, "action", Some(glib::VariantTy::STRING))
            .and_then(|v| v.get::<String>());
        assert_eq!(action.as_deref(), Some("panel.applet-remove"));
    }

    #[test]
    fn action_group_does_not_clash_with_the_applets() {
        // app_menu, system_menu and places_menu use "menu", "system",
        // "session" and "places"; one group of the same name would hide the
        // other.
        assert!(!["menu", "system", "session", "places"].contains(&ACTION_GROUP));
    }
}
