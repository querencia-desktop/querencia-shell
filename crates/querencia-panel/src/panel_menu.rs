//! The panel's own context menu, as in the GNOME 2 panel: a right click on
//! free panel space opens it at the pointer. A right click on an applet is
//! left to the applet; none of them uses it yet, so it does nothing there,
//! as before. Left and middle clicks never reach this code.
//!
//! The menu is a `PopoverMenu` like the menus of the applets, so it takes
//! the keyboard the same way: the panel keeps `KeyboardMode::None`, labwc
//! gives the popover's grab the keyboard while it is open (Escape closes it)
//! and returns it to the previously focused window afterwards.

use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, CenterBox, EventSequenceState, GestureClick, PickFlags,
    PopoverMenu, PositionType, Window, gdk, gio, glib,
};

use crate::add_dialog;
use crate::layout::{Edge, PanelSpec};

/// CSS class of the top widget of every applet; marks where free panel
/// space ends.
pub const APPLET_CLASS: &str = "querencia-applet";

const ACTION_GROUP: &str = "panel";
const ADD: &str = "add";

/// The menu entries. More follow with the functions behind them (M2.2c and
/// later); until then the menu shows nothing that would not work.
pub fn model() -> gio::Menu {
    let menu = gio::Menu::new();
    menu.append(
        Some("Zum Panel hinzufügen …"),
        Some(&format!("{ACTION_GROUP}.{ADD}")),
    );
    menu
}

/// Gives the panel `window` with the bar `bar` its context menu.
pub fn attach(app: &Application, window: &ApplicationWindow, bar: &CenterBox, spec: &PanelSpec) {
    // One "Zum Panel hinzufügen" dialog per panel, as in GNOME 2; opening it
    // again raises the open one.
    let dialog = glib::WeakRef::<Window>::new();
    let add = gio::SimpleAction::new(ADD, None);
    let panel_id = spec.id.clone();
    add.connect_activate(glib::clone!(
        #[weak]
        app,
        move |_, _| add_dialog::present(&app, &dialog, &panel_id)
    ));
    let actions = gio::SimpleActionGroup::new();
    actions.add_action(&add);
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
    bar.connect_destroy(glib::clone!(
        #[strong]
        popover,
        move |_| popover.unparent()
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
        move |gesture, presses, x, y| {
            // Pointer events inside a popover, e.g. an open applet menu,
            // bubble up to the panel too; only clicks on the panel surface
            // itself count. Their coordinates are the panel's own.
            let on_panel =
                gesture.current_event().and_then(|event| event.surface()) == window.surface();
            if presses != 1 || !on_panel || popover.is_visible() || on_applet(&window, x, y) {
                return;
            }
            let Some((x, y)) = window.translate_coordinates(&bar, x, y) else {
                return;
            };
            gesture.set_state(EventSequenceState::Claimed);
            popover.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
            popover.popup();
            eprintln!("querencia-panel: Kontextmenü von Panel {panel_id} geöffnet");
        }
    ));
    window.add_controller(click);
}

/// Whether the point (window coordinates) lies on an applet rather than on
/// free panel space.
fn on_applet(window: &ApplicationWindow, x: f64, y: f64) -> bool {
    let mut hit = window.pick(x, y, PickFlags::DEFAULT);
    while let Some(widget) = hit {
        if widget.has_css_class(APPLET_CLASS) {
            return true;
        }
        hit = widget.parent();
    }
    false
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
    fn action_group_does_not_clash_with_the_applets() {
        // app_menu, system_menu and places_menu use "menu", "system" and
        // "places"; one group of the same name would hide the other.
        assert!(!["menu", "system", "places"].contains(&ACTION_GROUP));
    }
}
