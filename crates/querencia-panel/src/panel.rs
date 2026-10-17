//! Turns a [`PanelSpec`] into a panel window: a layer-shell surface along one
//! screen edge with a `CenterBox` whose start, center and end hold the
//! applets of the matching regions, in order. The window setup is the one
//! the two fixed panels had before (M1.0–M1.5b). A right click on free panel
//! space opens the panel's context menu (`panel_menu.rs`). An applet added
//! later is put into the running panel by [`insert_applet`]; the applets
//! already shown stay as they are.

use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, Box as GtkBox, CenterBox, Orientation, Widget};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

use crate::layout::{self, Instance, PanelSpec, Region};
use crate::layout_editor::Editor;
use crate::{applets, launcher, panel_menu};

const NAMESPACE: &str = "querencia-panel";
/// Space between applets of one region (the former menu row).
const SPACING: i32 = 16;
/// Space between two launchers next to each other (HMP-11): as in the GNOME
/// 2 panel, launchers stand in a close row, not 16 px apart.
const LAUNCHER_SPACING: i32 = 2;

/// Builds the window for `spec`, one of the panels of `editor`'s layout.
/// The layout has been validated, so every applet type is known.
pub fn build(
    app: &Application,
    spec: &PanelSpec,
    editor: &Rc<RefCell<Editor>>,
) -> ApplicationWindow {
    let bar = CenterBox::builder()
        .css_classes(["querencia-panel"])
        .height_request(spec.height)
        .margin_start(10)
        .margin_end(10)
        .build();
    bar.set_start_widget(region(spec, Region::Start).as_ref());
    bar.set_center_widget(region(spec, Region::Center).as_ref());
    bar.set_end_widget(region(spec, Region::End).as_ref());

    let window = ApplicationWindow::builder()
        .application(app)
        .title("Querencia Panel")
        .default_height(spec.height)
        .child(&bar)
        .build();

    // Keyboard focus stays with the compositor: panels keep
    // KeyboardMode::None. From labwc 0.9.4 on, labwc gives a menu's grabbing
    // popup the keyboard and returns it to the previously focused window when
    // the popup closes. labwc 0.9.3 never focuses layer popups, so it is not
    // supported. Evidence: docs/m1-1b0-labwc-ab-test.md.
    window.init_layer_shell();
    window.set_namespace(Some(NAMESPACE));
    window.set_layer(Layer::Top);
    window.set_keyboard_mode(KeyboardMode::None);
    for anchor in [shell_edge(spec.edge), Edge::Left, Edge::Right] {
        window.set_anchor(anchor, true);
    }
    window.auto_exclusive_zone_enable();
    panel_menu::attach(app, &window, &bar, spec, editor);
    window
}

/// The applets of one region side by side. An empty region gets no widget,
/// as the center of both former panels had none: the `CenterBox` then lays
/// out start and end exactly as before.
fn region(spec: &PanelSpec, region: Region) -> Option<GtkBox> {
    if spec.region(region).is_empty() {
        return None;
    }
    let row = new_row();
    for instance in spec.region(region) {
        if let Some(widget) = applet_widget(instance) {
            row.append(&widget);
        }
    }
    space_out(&row);
    Some(row)
}

/// Puts a new widget for `instance` at `index` of `region` in the panel bar
/// `bar`, as [`crate::layout::Layout::add_applet`] placed it in the layout.
/// A region that had no applets gets its row now. Returns whether the
/// applet type was known.
pub fn insert_applet(bar: &CenterBox, region: Region, index: usize, instance: &Instance) -> bool {
    let Some(widget) = applet_widget(instance) else {
        return false;
    };
    let row = region_row(bar, region).unwrap_or_else(|| {
        let row = new_row();
        match region {
            Region::Start => bar.set_start_widget(Some(&row)),
            Region::Center => bar.set_center_widget(Some(&row)),
            Region::End => bar.set_end_widget(Some(&row)),
        }
        row
    });
    // The row holds one widget per instance of the region, in layout order.
    let mut previous: Option<Widget> = None;
    let mut next = row.first_child();
    for _ in 0..index {
        let Some(child) = next else { break };
        next = child.next_sibling();
        previous = Some(child);
    }
    row.insert_child_after(&widget, previous.as_ref());
    space_out(&row);
    true
}

/// Takes the widget of the applet instance `instance_id` out of the panel
/// bar `bar`. Returns whether there was one. A region left without applets
/// loses its row, as a region that never had any.
pub fn remove_applet(bar: &CenterBox, instance_id: &str) -> bool {
    for region in Region::ALL {
        let Some(row) = region_row(bar, region) else {
            continue;
        };
        let mut child = row.first_child();
        while let Some(widget) = child {
            if widget.widget_name() == instance_id {
                row.remove(&widget);
                if row.first_child().is_none() {
                    match region {
                        Region::Start => bar.set_start_widget(None::<&Widget>),
                        Region::Center => bar.set_center_widget(None::<&Widget>),
                        Region::End => bar.set_end_widget(None::<&Widget>),
                    }
                } else {
                    space_out(&row);
                }
                return true;
            }
            child = widget.next_sibling();
        }
    }
    false
}

fn region_row(bar: &CenterBox, region: Region) -> Option<GtkBox> {
    match region {
        Region::Start => bar.start_widget(),
        Region::Center => bar.center_widget(),
        Region::End => bar.end_widget(),
    }
    .and_then(|widget| widget.downcast::<GtkBox>().ok())
}

/// The row of one region. It has no spacing of its own: [`space_out`] puts
/// the gaps in as margins, so they can differ between neighbours.
fn new_row() -> GtkBox {
    GtkBox::new(Orientation::Horizontal, 0)
}

/// Gives every applet of `row` but the first its gap to the one before:
/// [`SPACING`], as the row spacing was before HMP-11, and
/// [`LAUNCHER_SPACING`] between two launchers. Run after every change of
/// the row.
fn space_out(row: &GtkBox) {
    let mut previous: Option<Widget> = None;
    let mut child = row.first_child();
    while let Some(widget) = child {
        let gap = previous.as_ref().map_or(0, |previous| {
            gap(
                previous.has_css_class(launcher::LAUNCHER_CLASS),
                widget.has_css_class(launcher::LAUNCHER_CLASS),
            )
        });
        widget.set_margin_start(gap);
        child = widget.next_sibling();
        previous = Some(widget);
    }
}

/// Gap between two neighbouring applets, given whether each is a launcher.
fn gap(left_is_launcher: bool, right_is_launcher: bool) -> i32 {
    if left_is_launcher && right_is_launcher {
        LAUNCHER_SPACING
    } else {
        SPACING
    }
}

/// A new widget for `instance`, or `None` (with a log line) for an unknown
/// applet type.
fn applet_widget(instance: &Instance) -> Option<Widget> {
    let Some(applet) = applets::find(&instance.applet) else {
        eprintln!(
            "querencia-panel: Applet-Typ {} unbekannt, {} ausgelassen",
            instance.applet, instance.id
        );
        return None;
    };
    let widget = applet.create(instance);
    // Names the widget after its instance, e.g. for the GTK inspector and
    // for finding it again when the layout changes.
    widget.set_widget_name(&instance.id);
    // Marks the applet's area: a right click there is not one on free
    // panel space. No style rule uses the class.
    widget.add_css_class(panel_menu::APPLET_CLASS);
    Some(widget)
}

fn shell_edge(edge: layout::Edge) -> Edge {
    match edge {
        layout::Edge::Top => Edge::Top,
        layout::Edge::Bottom => Edge::Bottom,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launchers_stand_close_and_other_applets_keep_their_gap() {
        assert_eq!(gap(false, false), 16);
        assert_eq!(gap(true, false), 16);
        assert_eq!(gap(false, true), 16);
        assert_eq!(gap(true, true), 2);
    }
}
