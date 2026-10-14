//! Turns a [`PanelSpec`] into a panel window: a layer-shell surface along one
//! screen edge with a `CenterBox` whose start, center and end hold the
//! applets of the matching regions, in order. The window setup is the one
//! the two fixed panels had before (M1.0–M1.5b).

use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, Box as GtkBox, CenterBox, Orientation};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

use crate::applets;
use crate::layout::{self, PanelSpec, Region};

const NAMESPACE: &str = "querencia-panel";
/// Space between applets of one region (the former menu row).
const SPACING: i32 = 16;

/// Builds the window for `spec`. The layout has been validated, so every
/// applet type is known.
pub fn build(app: &Application, spec: &PanelSpec) -> ApplicationWindow {
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
    window
}

/// The applets of one region side by side. An empty region gets no widget,
/// as the center of both former panels had none: the `CenterBox` then lays
/// out start and end exactly as before.
fn region(spec: &PanelSpec, region: Region) -> Option<GtkBox> {
    if spec.region(region).is_empty() {
        return None;
    }
    let row = GtkBox::new(Orientation::Horizontal, SPACING);
    for instance in spec.region(region) {
        let Some(applet) = applets::find(&instance.applet) else {
            eprintln!(
                "querencia-panel: Applet-Typ {} unbekannt, {} ausgelassen",
                instance.applet, instance.id
            );
            continue;
        };
        let widget = applet.create();
        // Names the widget after its instance, e.g. for the GTK inspector and
        // for finding it again when the layout changes.
        widget.set_widget_name(&instance.id);
        row.append(&widget);
    }
    Some(row)
}

fn shell_edge(edge: layout::Edge) -> Edge {
    match edge {
        layout::Edge::Top => Edge::Top,
        layout::Edge::Bottom => Edge::Bottom,
    }
}
