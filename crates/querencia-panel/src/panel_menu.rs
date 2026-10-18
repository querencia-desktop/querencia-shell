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
//! An applet type with a menu of its own ([`applets::AppletType::menu`],
//! G2-J: the volume applet) gets that menu instead. Its popover is parented
//! to the applet's widget, so the menu reaches the applet's own actions
//! (e.g. `volume.mute`) as well as the panel's (`panel.applet-remove`).
//!
//! Every applet's menu ends with the GNOME 2 „Verschieben“ entries
//! ([`move_section`], Teil 5): „Nach links“, „Nach rechts“ (one place,
//! across the regions), „An den linken Rand“, „An den rechten Rand“;
//! entries that would not move anything are greyed out. Applets without a
//! menu of their own (Anwendungen, Orte, System, Fensterliste,
//! Arbeitsflächenumschalter) get a menu with only these. Dragging with the
//! middle button (or Alt and the left button) is in `applet_move.rs`.
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

use crate::layout::{Edge, Move, PanelSpec, Region};
use crate::layout_editor::Editor;
use crate::{add_dialog, app_menu, applet_move, applets, panel};

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
/// Full name of the „Aus dem Panel entfernen“ action, for applet menus.
pub const REMOVE_ACTION: &str = "panel.applet-remove";
/// The moving actions (Teil 5), with their labels and moves.
const MOVES: [(&str, &str, Move); 4] = [
    ("applet-move-left", "Nach _links", Move::Left),
    ("applet-move-right", "Nach _rechts", Move::Right),
    ("applet-move-start", "An den linken _Rand", Move::StartEdge),
    ("applet-move-end", "An den rechten R_and", Move::EndEdge),
];

/// Name of the „Verschieben“ action (the applet follows the pointer).
const FOLLOW: &str = "applet-follow";

/// The „Verschieben“ entries every applet menu ends with.
pub fn move_section() -> gio::Menu {
    let menu = gio::Menu::new();
    menu.append(
        Some("_Verschieben"),
        Some(&format!("{ACTION_GROUP}.{FOLLOW}")),
    );
    for (name, label, _) in MOVES {
        menu.append(Some(label), Some(&format!("{ACTION_GROUP}.{name}")));
    }
    menu
}

/// The menu of an applet without entries of its own and that cannot be
/// removed (Anwendungen, Orte, System, Fensterliste,
/// Arbeitsflächenumschalter): only the „Verschieben“ entries.
pub fn fixed_model() -> gio::Menu {
    move_section()
}

/// `own` followed by the „Verschieben“ entries.
fn with_moves(own: &gio::Menu) -> gio::Menu {
    let menu = gio::Menu::new();
    menu.append_section(None, own);
    menu.append_section(None, &move_section());
    menu
}

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
    object.append(Some("Aus dem Panel entfernen"), Some(REMOVE_ACTION));
    menu.append_section(None, &object);
    menu.append_section(None, &move_section());
    menu
}

/// The menu of a removable applet without other entries (QREL-04d: the
/// clock): "Aus dem Panel entfernen".
pub fn applet_model() -> gio::Menu {
    let menu = gio::Menu::new();
    menu.append(Some("Aus dem Panel entfernen"), Some(REMOVE_ACTION));
    menu.append_section(None, &move_section());
    menu
}

/// Gives the panel `window` with the bar `bar` its context menu.
pub fn attach(
    app: &Application,
    window: &ApplicationWindow,
    bar: &CenterBox,
    spec: &PanelSpec,
    editor: &Rc<RefCell<Editor>>,
    mover: &Rc<applet_move::Mover>,
) {
    // Region of the free space last right-clicked: where the dialog adds.
    let target = Rc::new(Cell::new(Region::Start));
    let panel_id = spec.id.clone();
    let hooks = Rc::new(add_dialog::Hooks {
        region: target.clone(),
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
    // The menu of an applet with a menu of its own (G2-J), parented to the
    // applet's widget while it is open.
    let own_menu: Rc<RefCell<Option<PopoverMenu>>> = Rc::new(RefCell::new(None));
    let remove = gio::SimpleAction::new(REMOVE, None);
    remove.connect_activate(glib::clone!(
        #[strong]
        editor,
        #[strong]
        clicked,
        #[strong]
        own_menu,
        #[weak]
        bar,
        move |_, _| {
            let id = clicked.borrow().clone();
            // A menu parented to the applet must leave the widget before the
            // widget goes; the menu is still finishing its activation, so
            // both happen once it has returned.
            let menu = own_menu.borrow_mut().take();
            let editor = editor.clone();
            glib::idle_add_local_once(move || {
                if let Some(menu) = menu
                    && menu.parent().is_some()
                {
                    menu.unparent();
                }
                if let Err(err) = remove_applet(&editor, &bar, &id) {
                    eprintln!("querencia-panel: {id} nicht entfernt: {err}");
                }
            });
        }
    ));
    let actions = gio::SimpleActionGroup::new();
    actions.add_action(&add);
    actions.add_action(&launch);
    actions.add_action(&remove);
    // Teil 5: move the applet last right-clicked within its panel; saved
    // first, then moved on screen, after the menu has closed (it may be
    // parented to the applet).
    let mut moves = Vec::new();
    for (name, _, how) in MOVES {
        let action = gio::SimpleAction::new(name, None);
        action.connect_activate(glib::clone!(
            #[strong]
            editor,
            #[strong]
            clicked,
            #[weak]
            bar,
            move |_, _| {
                let id = clicked.borrow().clone();
                let editor = editor.clone();
                glib::idle_add_local_once(move || move_applet(&editor, &bar, &id, how));
            }
        ));
        actions.add_action(&action);
        moves.push((action, how));
    }
    let moves = Rc::new(moves);
    let follow = gio::SimpleAction::new(FOLLOW, None);
    follow.connect_activate(glib::clone!(
        #[strong]
        clicked,
        #[strong]
        mover,
        move |_, _| {
            let id = clicked.borrow().clone();
            let mover = mover.clone();
            glib::idle_add_local_once(move || mover.follow(&id));
        }
    ));
    actions.add_action(&follow);
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
    let fixed_popover = PopoverMenu::from_model(Some(&fixed_model()));
    fixed_popover.set_has_arrow(false);
    fixed_popover.set_halign(Align::Start);
    fixed_popover.set_position(popover.position());
    fixed_popover.set_parent(bar);
    bar.connect_destroy(glib::clone!(
        #[strong]
        popover,
        #[strong]
        launcher_popover,
        #[strong]
        applet_popover,
        #[strong]
        fixed_popover,
        move |_| {
            popover.unparent();
            launcher_popover.unparent();
            applet_popover.unparent();
            fixed_popover.unparent();
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
        #[weak]
        fixed_popover,
        #[strong]
        moves,
        #[strong]
        target,
        #[strong]
        clicked,
        #[strong]
        editor,
        #[strong]
        own_menu,
        #[strong]
        actions,
        move |gesture, presses, x, y| {
            // Pointer events inside a popover, e.g. an open applet menu,
            // bubble up to the panel too; only clicks on the panel surface
            // itself count. Their coordinates are the panel's own.
            let on_panel =
                gesture.current_event().and_then(|event| event.surface()) == window.surface();
            let open = [&popover, &launcher_popover, &applet_popover, &fixed_popover]
                .iter()
                .any(|p| p.is_visible())
                || own_menu.borrow().as_ref().is_some_and(|p| p.is_visible());
            if presses != 1 || !on_panel || open {
                return;
            }
            let applet = applet_at(&window, x, y);
            let (window_x, window_y) = (x, y);
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
                // Greys out the moves that would do nothing here.
                for (action, how) in moves.iter() {
                    action.set_enabled(editor.borrow().can_move(&id, *how));
                }
                if let Some(model) = kind.and_then(|t| t.menu) {
                    let Some((ax, ay)) = window.translate_coordinates(&applet, window_x, window_y)
                    else {
                        return;
                    };
                    *clicked.borrow_mut() = id;
                    gesture.set_state(EventSequenceState::Claimed);
                    if let Some(old) = own_menu.borrow_mut().take()
                        && old.parent().is_some()
                    {
                        old.unparent();
                    }
                    let menu = PopoverMenu::from_model(Some(&with_moves(&model())));
                    // The panel's actions on the menu itself: an applet
                    // built before the bar got its action group (an applet
                    // from panel.ini) does not find them through its
                    // parents; GTK does not relink an existing muxer.
                    menu.insert_action_group(ACTION_GROUP, Some(&actions));
                    menu.set_has_arrow(false);
                    menu.set_halign(Align::Start);
                    menu.set_position(popover.position());
                    menu.set_parent(&applet);
                    menu.set_pointing_to(Some(&gdk::Rectangle::new(ax as i32, ay as i32, 1, 1)));
                    menu.connect_closed(|menu| {
                        let menu = menu.clone();
                        glib::idle_add_local_once(move || {
                            if menu.parent().is_some() {
                                menu.unparent();
                            }
                        });
                    });
                    *own_menu.borrow_mut() = Some(menu.clone());
                    menu.popup();
                    eprintln!(
                        "querencia-panel: Appletmenü von {} geöffnet",
                        clicked.borrow()
                    );
                    return;
                }
                let menu = match kind {
                    Some(t) if t.id == crate::layout::LAUNCHER => Some(&launcher_popover),
                    Some(t) if t.removable => Some(&applet_popover),
                    Some(_) => Some(&fixed_popover),
                    None => None,
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

/// Moves the applet instance `instance_id` as `how` says: first in the
/// layout and `panel.ini` through `editor`, then, once saved, on screen.
pub(crate) fn move_applet(editor: &RefCell<Editor>, bar: &CenterBox, instance_id: &str, how: Move) {
    let result = editor.borrow_mut().move_applet(instance_id, how);
    match result {
        Ok(Some(moved)) => {
            if !panel::move_applet(bar, instance_id, moved.to.0, moved.to.1) {
                eprintln!(
                    "querencia-panel: {instance_id} gespeichert, aber nicht in diesem Panel angezeigt"
                );
            }
            eprintln!(
                "querencia-panel: {instance_id} verschoben ({how:?}): {} {} → {} {}; gespeichert in {}",
                moved.from.0.label(),
                moved.from.1 + 1,
                moved.to.0.label(),
                moved.to.1 + 1,
                editor.borrow().path().display()
            );
        }
        Ok(None) => eprintln!("querencia-panel: {instance_id} bleibt, wo es ist ({how:?})"),
        Err(err) => eprintln!("querencia-panel: {instance_id} nicht verschoben: {err}"),
    }
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
    fn every_applet_menu_ends_with_the_moves() {
        let labels = |menu: &gio::MenuModel| -> Vec<(String, String)> {
            (0..menu.n_items())
                .map(|i| {
                    let get = |key| {
                        menu.item_attribute_value(i, key, Some(glib::VariantTy::STRING))
                            .and_then(|v| v.get::<String>())
                            .unwrap_or_default()
                    };
                    (get("label"), get("action"))
                })
                .collect()
        };
        let moves = labels(move_section().upcast_ref());
        assert_eq!(
            moves,
            [
                (
                    "_Verschieben".to_string(),
                    "panel.applet-follow".to_string()
                ),
                (
                    "Nach _links".to_string(),
                    "panel.applet-move-left".to_string()
                ),
                ("Nach _rechts".into(), "panel.applet-move-right".into()),
                (
                    "An den linken _Rand".into(),
                    "panel.applet-move-start".into()
                ),
                (
                    "An den rechten R_and".into(),
                    "panel.applet-move-end".into()
                ),
            ]
        );
        for menu in [
            launcher_model(),
            applet_model(),
            with_moves(&gio::Menu::new()),
        ] {
            let last = menu
                .item_link(menu.n_items() - 1, "section")
                .expect("Abschnitt");
            assert_eq!(labels(&last), moves);
        }
        assert_eq!(labels(fixed_model().upcast_ref()), moves);
    }

    #[test]
    fn clock_menu_removes_then_moves() {
        let menu = applet_model();
        assert_eq!(menu.n_items(), 2);
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
    fn remove_action_is_the_panel_groups_remove() {
        assert_eq!(REMOVE_ACTION, format!("{ACTION_GROUP}.{REMOVE}"));
    }

    #[test]
    fn action_group_does_not_clash_with_the_applets() {
        // app_menu, system_menu and places_menu use "menu", "system",
        // "session" and "places"; one group of the same name would hide the
        // other.
        assert!(!["menu", "system", "session", "places"].contains(&ACTION_GROUP));
    }
}
