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
use crate::{applet_move, applets, launcher, panel_menu};

const NAMESPACE: &str = "querencia-panel";
/// Space between applets of one region (the former menu row).
const SPACING: i32 = 16;
/// Space between two launchers next to each other (HMP-11): as in the GNOME
/// 2 panel, launchers stand in a close row, not 16 px apart.
const LAUNCHER_SPACING: i32 = 2;
/// Space between two status icons next to each other (QD-045): as in the
/// GNOME 2 notification area. Status buttons have no padding, only their
/// 1 px hover border, so the icons themselves stand 4 px apart.
const STATUS_SPACING: i32 = 2;
/// Style class of the status applets (volume, network and later ones,
/// [`crate::applets::AppletType::status`]); `main.rs` gives them one inner
/// border and [`space_out`] the close gap.
pub const STATUS_CLASS: &str = "querencia-status";

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
    let mover = applet_move::attach(&window, &bar, editor);
    panel_menu::attach(app, &window, &bar, spec, editor, &mover);
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
        if let Some(widget) = applet_widget(instance, spec.height) {
            row.append(&widget);
        }
    }
    space_out(&row);
    Some(row)
}

/// Puts a new widget for `instance` at `index` of `region` in the panel bar
/// `bar`, as [`crate::layout::Layout::add_applet`] placed it in the layout.
/// A region that had no applets gets its row now. `panel_height` is the
/// height of that panel. Returns whether the applet type was known.
pub fn insert_applet(
    bar: &CenterBox,
    region: Region,
    index: usize,
    instance: &Instance,
    panel_height: i32,
) -> bool {
    let Some(widget) = applet_widget(instance, panel_height) else {
        return false;
    };
    place(bar, region, index, &widget);
    true
}

/// Puts `widget` at `index` of `region` in `bar`; a region that had no
/// applets gets its row now.
fn place(bar: &CenterBox, region: Region, index: usize, widget: &Widget) {
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
    row.insert_child_after(widget, previous.as_ref());
    space_out(&row);
}

/// Takes the widget of the applet instance `instance_id` out of the panel
/// bar `bar`. Returns whether there was one. A region left without applets
/// loses its row, as a region that never had any.
pub fn remove_applet(bar: &CenterBox, instance_id: &str) -> bool {
    take(bar, instance_id).is_some()
}

/// Moves the shown applet `instance_id` to `index` of `region` (counted
/// without it), keeping its widget and everything it holds (popups,
/// connections), as [`crate::layout::Layout::move_applet`] moved it.
pub fn move_applet(bar: &CenterBox, instance_id: &str, region: Region, index: usize) -> bool {
    let Some(widget) = take(bar, instance_id) else {
        return false;
    };
    place(bar, region, index, &widget);
    true
}

/// Where the applet `instance_id` stands in `bar`: region and index.
pub fn shown_position(bar: &CenterBox, instance_id: &str) -> Option<(Region, usize)> {
    Region::ALL.into_iter().find_map(|region| {
        let row = region_row(bar, region)?;
        let mut child = row.first_child();
        let mut index = 0;
        while let Some(widget) = child {
            if widget.widget_name() == instance_id {
                return Some((region, index));
            }
            index += 1;
            child = widget.next_sibling();
        }
        None
    })
}

/// The applet widgets of `region` in `bar`, left to right.
pub fn region_applets(bar: &CenterBox, region: Region) -> Vec<Widget> {
    let mut list = Vec::new();
    if let Some(row) = region_row(bar, region) {
        let mut child = row.first_child();
        while let Some(widget) = child {
            child = widget.next_sibling();
            list.push(widget);
        }
    }
    list
}

/// Takes the applet widget `instance_id` out of its row and returns it.
fn take(bar: &CenterBox, instance_id: &str) -> Option<Widget> {
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
                return Some(widget);
            }
            child = widget.next_sibling();
        }
    }
    None
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
/// [`SPACING`], as the row spacing was before HMP-11, [`LAUNCHER_SPACING`]
/// between two launchers and [`STATUS_SPACING`] between two status icons.
/// Run after every change of the row (adding, moving, removing).
fn space_out(row: &GtkBox) {
    let mut widgets = Vec::new();
    let mut child = row.first_child();
    while let Some(widget) = child {
        child = widget.next_sibling();
        widgets.push(widget);
    }
    let kinds: Vec<Kind> = widgets.iter().map(Kind::of).collect();
    for (widget, margin) in widgets.iter().zip(margins(&kinds)) {
        widget.set_margin_start(margin);
    }
}

/// The start margins of a row of applets of the given kinds, left to
/// right: none for the first, for every other one its gap to the left
/// neighbour.
fn margins(kinds: &[Kind]) -> Vec<i32> {
    let mut previous = None;
    kinds
        .iter()
        .map(|&kind| {
            let margin = previous.map_or(0, |previous| gap(previous, kind));
            previous = Some(kind);
            margin
        })
        .collect()
}

/// What decides the gap between two neighbouring applets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Launcher,
    Status,
    Other,
}

impl Kind {
    fn of(widget: &Widget) -> Kind {
        if widget.has_css_class(launcher::LAUNCHER_CLASS) {
            Kind::Launcher
        } else if widget.has_css_class(STATUS_CLASS) {
            Kind::Status
        } else {
            Kind::Other
        }
    }
}

/// Gap between two neighbouring applets: close within a row of launchers
/// and within a group of status icons, the normal one everywhere else (a
/// status icon next to the clock or a launcher, for example).
fn gap(left: Kind, right: Kind) -> i32 {
    match (left, right) {
        (Kind::Launcher, Kind::Launcher) => LAUNCHER_SPACING,
        (Kind::Status, Kind::Status) => STATUS_SPACING,
        _ => SPACING,
    }
}

/// A new widget for `instance` on a panel `panel_height` pixels high, or
/// `None` (with a log line) for an unknown applet type.
fn applet_widget(instance: &Instance, panel_height: i32) -> Option<Widget> {
    let Some(applet) = applets::find(&instance.applet) else {
        eprintln!(
            "querencia-panel: Applet-Typ {} unbekannt, {} ausgelassen",
            instance.applet, instance.id
        );
        return None;
    };
    let widget = applet.create(instance, panel_height);
    // Names the widget after its instance, e.g. for the GTK inspector and
    // for finding it again when the layout changes.
    widget.set_widget_name(&instance.id);
    // Marks the applet's area: a right click there is not one on free
    // panel space. No style rule uses the class.
    widget.add_css_class(panel_menu::APPLET_CLASS);
    // Before the widget gets into a row, so that space_out sees it.
    if applet.status {
        widget.add_css_class(STATUS_CLASS);
    }
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
        assert_eq!(gap(Kind::Other, Kind::Other), 16);
        assert_eq!(gap(Kind::Launcher, Kind::Other), 16);
        assert_eq!(gap(Kind::Other, Kind::Launcher), 16);
        assert_eq!(gap(Kind::Launcher, Kind::Launcher), 2);
    }

    /// QD-045: volume and network stand close as in the GNOME 2
    /// notification area; to the clock, launchers and menus the gap stays.
    #[test]
    fn status_icons_stand_close_and_keep_the_gap_to_everything_else() {
        assert_eq!(gap(Kind::Status, Kind::Status), 2);
        for other in [Kind::Other, Kind::Launcher] {
            assert_eq!(gap(Kind::Status, other), 16, "{other:?}");
            assert_eq!(gap(other, Kind::Status), 16, "{other:?}");
        }
        assert!((2..=4).contains(&STATUS_SPACING));
    }

    /// The margins space_out sets after adding, moving or removing an
    /// applet: it always works out the whole row again.
    #[test]
    fn every_row_gets_its_margins_from_its_order() {
        use Kind::{Launcher as L, Other as O, Status as S};
        assert_eq!(margins(&[]), Vec::<i32>::new());
        assert_eq!(margins(&[S]), [0]);
        // End=volume-1;network-1;clock-1
        assert_eq!(margins(&[S, S, O]), [0, 2, 16]);
        // the clock moved between the two status icons ("Nach links")
        assert_eq!(margins(&[S, O, S]), [0, 16, 16]);
        // and the network moved to the left edge
        assert_eq!(margins(&[S, S, O]), [0, 2, 16]);
        // the volume removed: the network is first and loses its gap
        assert_eq!(margins(&[S, O]), [0, 16]);
        // a later third status icon added before the clock
        assert_eq!(margins(&[S, S, S, O]), [0, 2, 2, 16]);
        // launchers next to status icons keep the normal gap
        assert_eq!(margins(&[L, L, S, S]), [0, 2, 16, 2]);
        // Start=applications-1;places-1;system-1
        assert_eq!(margins(&[O, O, O]), [0, 16, 16]);
    }
}
