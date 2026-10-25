//! Classic GNOME 2 menus (HMP-03): the three main panel menus, the desktop
//! menu and every menu bar or context menu of a Querencia program.
//!
//! GTK 4's `PopoverMenu` shows no icons next to text and either slides
//! submenus in place or opens them beside the menu with an 80 ms timer
//! that loses diagonal pointer movement (`docs/m1-1b1d-…`, `m1-1b1e-…`).
//! This module draws the menus itself from the same `gio::Menu` models:
//!
//! - each row: 16 px icon, label, ▸ for a submenu; sections become
//!   separator lines; an item's `accel` attribute (GTK notation, e.g.
//!   `<Control><Alt>l`) is shown on the right as GTK 2 did, in German
//!   ([`accel_label`]), only shown: the compositor binds the key;
//! - submenus open beside their row, in their own popover (`position`
//!   right, `valign` start, no arrow: as GTK's own nested menus), after
//!   [`SUBMENU_DELAY_MS`] on hover or at once on click or keyboard;
//! - while the pointer moves from a row towards its open submenu, other
//!   rows it crosses do not take over ([`aiming`], the submenu navigation
//!   triangle of GTK 2/3 menus), for at most [`AIM_TIMEOUT_MS`];
//! - one highlighted row per menu, shared by pointer and keyboard: Up,
//!   Down, Home, End, Tab; Right, Enter or Space open a submenu; Left or
//!   Escape close it; Enter or Space activate an entry;
//! - activating an entry closes all menus and activates the item's action
//!   with its target on the panel button, as `PopoverMenu` would;
//! - the main menus of a panel act as one menu bar (HMP-04): while one is
//!   open, moving the pointer onto another main menu button opens that one
//!   instead, and Right on an entry without submenu or Left in the top
//!   level switch to the next or previous main menu, wrapping around; the
//!   pointer counts only over a button, not below the bar (QD-176), and
//!   context menus ([`attach_popup`]) are no part of the menu bar;
//! - only the top level grabs (HMP-05), as GTK 2's menu shell: a grabbing
//!   submenu got every pointer event over the menus above it from GTK
//!   (`gtk_grab_add`), so the menu above stuck on its old row. Keys go
//!   through the top level to the innermost menu with a highlighted row;
//!   Escape closes the innermost menu. `QUERENCIA_MENU_DEBUG=1` logs every
//!   state change.
//!
//! Text colour (HMP-04): GTK gives every popover the CSS class
//! `background`, through which the theme sets the text colour again
//! (`.background { color: … }`). Without it a popover inherits the colour
//! of its CSS parent, and a submenu's parent is the highlighted row with
//! white text: white on white. The popovers therefore keep their GTK
//! classes ([`menu_popover`]; never `css_classes`, which replaces them),
//! and the menu style of the program also sets the text colour itself.
//!
//! Icons of the rows are loaded without GTK's preloading in the thread pool
//! ([`crate::symbole`], QD-106) unless a program chooses GTK's own way
//! ([`set_icon_loading`]).
//!
//! The pure parts (rows, keyboard steps, hit test, aiming) are tested
//! without a display; the widgets are thin on top of them.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use gtk4::prelude::*;
use gtk4::{
    AccessibleRole, Align, Box as GtkBox, EventControllerKey, EventControllerMotion, GestureClick,
    Image, Label, MenuButton, Orientation, Popover, PositionType, PropagationPhase, Separator,
    Widget, gdk, gio, glib, graphene,
};

/// One line of a menu.
#[derive(Debug, Clone)]
pub enum Row {
    Separator,
    Entry {
        label: String,
        icon: Option<gio::Icon>,
        /// Detailed action name; `None` shows the entry insensitive.
        action: Option<String>,
        target: Option<glib::Variant>,
        /// Shortcut shown next to the label ([`accel_label`]).
        accel: Option<String>,
    },
    Submenu {
        label: String,
        icon: Option<gio::Icon>,
        model: gio::MenuModel,
    },
}

impl Row {
    /// Whether keyboard and pointer can highlight the row.
    pub fn selectable(&self) -> bool {
        match self {
            Row::Separator => false,
            Row::Entry { action, .. } => action.is_some(),
            Row::Submenu { model, .. } => model.n_items() > 0,
        }
    }
}

/// The rows of `model`: plain items and submenu links as they come, each
/// section as a group, with one separator between non-empty groups and none
/// at the start or end, as GTK's menus draw sections.
pub fn rows(model: &gio::MenuModel) -> Vec<Row> {
    let mut groups: Vec<Vec<Row>> = Vec::new();
    let mut plain: Vec<Row> = Vec::new();
    for i in 0..model.n_items() {
        if let Some(section) = model.item_link(i, "section") {
            if !plain.is_empty() {
                groups.push(std::mem::take(&mut plain));
            }
            let section = rows(&section);
            if !section.is_empty() {
                groups.push(section);
            }
            continue;
        }
        let label = string_attribute(model, i, "label").unwrap_or_default();
        let icon = model
            .item_attribute_value(i, "icon", None)
            .and_then(|v| gio::Icon::deserialize(&v));
        if let Some(submenu) = model.item_link(i, "submenu") {
            plain.push(Row::Submenu {
                label,
                icon,
                model: submenu,
            });
        } else {
            plain.push(Row::Entry {
                label,
                icon,
                action: string_attribute(model, i, "action"),
                target: model.item_attribute_value(i, "target", None),
                accel: string_attribute(model, i, "accel")
                    .as_deref()
                    .and_then(accel_label),
            });
        }
    }
    if !plain.is_empty() {
        groups.push(plain);
    }
    let mut rows = Vec::new();
    for (n, group) in groups.into_iter().enumerate() {
        if n > 0 {
            rows.push(Row::Separator);
        }
        rows.extend(group);
    }
    rows
}

/// A GTK accelerator (`<Control><Alt>l`) as GTK 2's menus showed it
/// (`Ctrl+Alt+L`, German `Strg+Alt+L` from the catalog); `None` for an empty
/// or unreadable one. Parsed here, not by GTK, so the text does not depend
/// on GTK's translations; the key names come from the text domain of this
/// library (`crate::i18n`). Only the panel shows shortcuts so far; the
/// desktop never sets `accel`.
pub fn accel_label(accel: &str) -> Option<String> {
    use crate::i18n::pgettext;
    let mut parts = Vec::new();
    let mut rest = accel.trim();
    while let Some(after) = rest.strip_prefix('<') {
        let (modifier, tail) = after.split_once('>')?;
        parts.push(match modifier.to_ascii_lowercase().as_str() {
            "control" | "ctrl" | "ctl" | "primary" => pgettext("keyboard key", "Ctrl"),
            "alt" | "mod1" => "Alt".to_owned(),
            "shift" => pgettext("keyboard key", "Shift"),
            "super" | "mod4" | "meta" => "Super".to_owned(),
            _ => return None,
        });
        rest = tail;
    }
    if rest.is_empty() || rest.contains(['<', '>', ' ']) {
        return None;
    }
    let key = match rest {
        "Delete" => pgettext("keyboard key", "Del"),
        "Escape" => "Esc".to_owned(),
        "Return" => pgettext("keyboard key", "Enter"),
        "Print" => pgettext("keyboard key", "Print"),
        key if key.chars().count() == 1 => key.to_uppercase(),
        key => key.to_owned(),
    };
    let mut text = parts.join("+");
    if !text.is_empty() {
        text.push('+');
    }
    text.push_str(&key);
    Some(text)
}

fn string_attribute(model: &gio::MenuModel, index: i32, name: &str) -> Option<String> {
    model
        .item_attribute_value(index, name, Some(glib::VariantTy::STRING))
        .and_then(|v| v.get::<String>())
}

/// A keyboard step in a menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Move {
    Next,
    Previous,
    First,
    Last,
}

/// The row a keyboard step from `current` leads to, among the rows marked
/// in `selectable`. Next and previous wrap around, as GTK 2 menus did; from
/// no row, next is the first and previous the last.
pub fn step(selectable: &[bool], current: Option<usize>, how: Move) -> Option<usize> {
    let candidates: Vec<usize> = (0..selectable.len()).filter(|&i| selectable[i]).collect();
    let first = *candidates.first()?;
    let last = *candidates.last()?;
    let current = current.filter(|&i| i < selectable.len());
    match (how, current) {
        (Move::First, _) | (Move::Next, None) => Some(first),
        (Move::Last, _) | (Move::Previous, None) => Some(last),
        (Move::Next, Some(i)) => Some(candidates.iter().copied().find(|&c| c > i).unwrap_or(first)),
        (Move::Previous, Some(i)) => Some(
            candidates
                .iter()
                .rev()
                .copied()
                .find(|&c| c < i)
                .unwrap_or(last),
        ),
    }
}

/// Index of the row whose `(top, height)` band holds `y`, if any.
pub fn row_at(rows: &[(f64, f64)], y: f64) -> Option<usize> {
    rows.iter()
        .position(|&(top, height)| y >= top && y < top + height)
}

/// Position of the main menu next to the one at `index` among `count`
/// main menus of a panel, to the right (`forward`) or left, wrapping around
/// like the GNOME 2 menu bar. `None` without another menu.
pub fn neighbour(count: usize, index: usize, forward: bool) -> Option<usize> {
    if count < 2 || index >= count {
        return None;
    }
    Some(if forward {
        (index + 1) % count
    } else {
        (index + count - 1) % count
    })
}

/// A rectangle in surface coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Delay before a hovered submenu opens: `gtk-menu-popup-delay` of GTK 2.
pub const SUBMENU_DELAY_MS: u64 = 225;
/// How long a move towards an open submenu keeps it open at most, before
/// the row under the pointer takes over after all.
pub const AIM_TIMEOUT_MS: u64 = 300;
/// Pointer events within this distance (px) of where the pointer was when
/// a key was pressed are no movement (see `Level::key_hold`).
pub const HOLD_TOLERANCE: f64 = 3.0;

/// A key hold (see `Level::key_hold`) after a pointer event at `(x, y)`:
/// whether the event is a real movement, and the hold that remains.
pub fn hold_step(
    hold: Option<Option<(f64, f64)>>,
    x: f64,
    y: f64,
) -> (bool, Option<Option<(f64, f64)>>) {
    match hold {
        None => (true, None),
        Some(None) => (false, Some(Some((x, y)))),
        Some(Some((hx, hy)))
            if (x - hx).abs() < HOLD_TOLERANCE && (y - hy).abs() < HOLD_TOLERANCE =>
        {
            (false, hold)
        }
        Some(Some(_)) => (true, None),
    }
}

/// Whether the pointer, moving from `apex` (where it last was on the row
/// of the open submenu) to `pointer`, heads for `submenu`: it lies in the
/// triangle between `apex` and the near edge of the submenu. The submenu
/// may sit on either side; an apex level with or inside it, or no sideways
/// movement towards it (standing still included), is no aiming.
pub fn aiming(apex: (f64, f64), pointer: (f64, f64), submenu: Rect) -> bool {
    if submenu.width <= 0.0 || submenu.height <= 0.0 {
        return false;
    }
    let edge = if apex.0 < submenu.x {
        submenu.x
    } else if apex.0 > submenu.x + submenu.width {
        submenu.x + submenu.width
    } else {
        return false;
    };
    let span = edge - apex.0;
    let progress = (pointer.0 - apex.0) / span;
    if !(progress > 0.0 && progress <= 1.0) {
        return false;
    }
    let top = apex.1 + progress * (submenu.y - apex.1);
    let bottom = apex.1 + progress * (submenu.y + submenu.height - apex.1);
    pointer.1 >= top && pointer.1 <= bottom
}

// ---- Widgets ---------------------------------------------------------

/// CSS class of every classic menu popover (style in the program).
pub const MENU_CLASS: &str = "querencia-menu";
/// CSS class of a menu row; [`ACTIVE_CLASS`] marks the highlighted one.
pub const ITEM_CLASS: &str = "qmenu-item";
pub const ACTIVE_CLASS: &str = "qmenu-active";
const ARROW_CLASS: &str = "qmenu-arrow";
/// Style class of a row's shortcut text ([`accel_label`]).
pub const ACCEL_CLASS: &str = "qmenu-accel";
const ICON_SIZE: i32 = 16;
/// Icon of an entry whose icon the theme does not have.
const MISSING_ICON: &str = "application-x-executable";
/// Padding plus border above the first row (CSS of the program): a submenu is
/// moved up by it, so its first row lines up with the row that opened it.
const MENU_TOP_INSET: i32 = 3;

/// Gives `button` a classic menu for `model` instead of GTK's
/// `PopoverMenu`, as a main menu of its window's menu bar (or panel): while
/// it is open, the pointer over another main menu button or Left/Right
/// switch over. The rows are built afresh each time the menu opens, so
/// they always show the model as it is then; the model is only changed
/// while the menu is closed (`app_menu::follow_changes`).
pub fn attach(button: &MenuButton, model: &gio::MenuModel) {
    attach_menu(button, model, true);
}

/// Gives `button` a classic menu for `model` that is not part of a menu
/// bar: a context menu on a hidden anchor. It never switches to a main
/// menu of its window, neither by pointer nor by Left/Right (QD-176).
pub fn attach_popup(button: &MenuButton, model: &gio::MenuModel) {
    attach_menu(button, model, false);
}

fn attach_menu(button: &MenuButton, model: &gio::MenuModel, main_menu: bool) {
    let popover = menu_popover();
    popover.set_halign(Align::Start);
    button.set_popover(Some(&popover));

    let current: Current = Rc::default();
    popover.connect_show(glib::clone!(
        #[strong]
        current,
        #[strong]
        model,
        move |popover| {
            current.replace(Some(Level::new(popover, &model, None)));
        }
    ));
    popover.connect_closed(glib::clone!(
        #[strong]
        current,
        move |_| {
            if let Some(level) = current.take() {
                level.close_child();
            }
        }
    ));

    // The keyboard (HMP-05): this top level is the only grabbing popup, so
    // it has the keyboard focus, and keys arrive at this popover, also
    // when no row has the focus yet (menu just opened by click). One
    // controller on the long-lived popover, capture phase so it comes
    // before GTK's own popover keys; it hands the key to the open level.
    let keys = EventControllerKey::new();
    keys.set_propagation_phase(PropagationPhase::Capture);
    keys.connect_key_pressed(glib::clone!(
        #[strong]
        current,
        move |_, key, _, _| {
            let level = current.borrow().clone();
            match level {
                Some(level) => level.on_key(key),
                None => glib::Propagation::Proceed,
            }
        }
    ));
    popover.add_controller(keys);

    if !main_menu {
        return;
    }
    // Menu bar behaviour: moving onto another main menu button of the same
    // panel while this menu is open switches over, as in GNOME 2. The open
    // top level grabs, so GTK hands the pointer motion over the panel to
    // its popover (with coordinates of the wrong surface); the position is
    // taken from the event itself, in panel surface coordinates.
    let panel_motion = EventControllerMotion::new();
    panel_motion.connect_motion(glib::clone!(
        #[weak]
        button,
        move |controller, _, _| {
            if let Some(event) = controller.current_event() {
                hover_panel(&button, &event);
            }
        }
    ));
    popover.add_controller(panel_motion);
    // Without a grab (no menu open, or a grab-free test) the button sees
    // the pointer itself.
    let motion = EventControllerMotion::new();
    motion.connect_enter(glib::clone!(
        #[weak]
        button,
        move |_, _, _| switch_on_hover(&button)
    ));
    button.add_controller(motion);

    MAIN_MENUS.with(|menus| {
        let mut menus = menus.borrow_mut();
        menus.retain(|(button, _)| button.upgrade().is_some());
        menus.push((button.downgrade(), Rc::downgrade(&current)));
    });
}

/// The open top level of a main menu, if any.
type Current = Rc<RefCell<Option<Rc<Level>>>>;
/// A main menu button and its open top level, both held weakly.
type MainMenu = (glib::WeakRef<MenuButton>, Weak<RefCell<Option<Rc<Level>>>>);

thread_local! {
    /// Every main menu button with its open top level; GTK runs on one
    /// thread, so this is the panel's whole set of main menus.
    static MAIN_MENUS: RefCell<Vec<MainMenu>> = const { RefCell::new(Vec::new()) };
}

/// A popover for a classic menu: GTK's own CSS classes (`background`, which
/// resets the text colour) plus [`MENU_CLASS`], no arrow.
fn menu_popover() -> Popover {
    let popover = Popover::builder().has_arrow(false).build();
    popover.add_css_class(MENU_CLASS);
    popover
}

/// The main menu buttons in the same panel window as `button`, left to
/// right (widget order), with their open top levels.
fn main_menus_beside(button: &MenuButton) -> Vec<(MenuButton, Current)> {
    let Some(root) = button.root() else {
        return Vec::new();
    };
    let root: Widget = root.upcast();
    let mut known: Vec<(MenuButton, Current)> = MAIN_MENUS.with(|menus| {
        menus
            .borrow()
            .iter()
            .filter_map(|(button, current)| Some((button.upgrade()?, current.upgrade()?)))
            .filter(|(other, _)| other.root().is_some_and(|r| r.upcast::<Widget>() == root))
            .collect()
    });
    let order = descendants(&root);
    known.sort_by_key(|(other, _)| {
        order
            .iter()
            .position(|w| w == other.upcast_ref::<Widget>())
            .unwrap_or(usize::MAX)
    });
    known
}

/// `widget` and everything below it, depth first (left to right in a
/// horizontal panel).
fn descendants(widget: &Widget) -> Vec<Widget> {
    let mut all = vec![widget.clone()];
    let mut child = widget.first_child();
    while let Some(current) = child {
        all.extend(descendants(&current));
        child = current.next_sibling();
    }
    all
}

/// The area of `button` in its window's surface coordinates.
fn surface_rect(button: &MenuButton) -> Option<Rect> {
    let native = button.native()?;
    let bounds = button.compute_bounds(&native)?;
    let (dx, dy) = native.surface_transform();
    Some(Rect {
        x: f64::from(bounds.x()) + dx,
        y: f64::from(bounds.y()) + dy,
        width: f64::from(bounds.width()),
        height: f64::from(bounds.height()),
    })
}

/// The index of the button area that holds the pointer at (`x`, `y`). Both
/// coordinates count: in a window with a menu bar the pointer below the bar
/// is over the content, not over a menu (QD-176).
pub fn button_at(buttons: &[Rect], x: f64, y: f64) -> Option<usize> {
    buttons
        .iter()
        .position(|b| x >= b.x && x < b.x + b.width && y >= b.y && y < b.y + b.height)
}

/// Pointer motion handed to the open top level of `open`: if it happened
/// over the panel window, above another main menu button, switch to it.
fn hover_panel(open: &MenuButton, event: &gdk::Event) {
    let panel_surface = open.native().and_then(|n| n.surface());
    if event.surface().is_none() || event.surface() != panel_surface || !is_open(open) {
        return;
    }
    let Some((x, y)) = event.position() else {
        return;
    };
    let menus = main_menus_beside(open);
    let areas: Vec<Rect> = menus
        .iter()
        .map(|(button, _)| surface_rect(button).unwrap_or_default())
        .collect();
    if let Some(index) = button_at(&areas, x, y) {
        let target = &menus[index].0;
        if target != open {
            debug(&format!(
                "Hauptmenü per Zeiger: {} -> {}",
                label_of(open),
                label_of(target)
            ));
            open.popdown();
            target.popup();
        }
    }
}

/// The label of a main menu button, for the debug log.
fn label_of(button: &MenuButton) -> String {
    descendants(button.upcast_ref())
        .into_iter()
        .find_map(|w| w.downcast::<Label>().ok().map(|l| l.label().to_string()))
        .unwrap_or_default()
}

/// Whether `QUERENCIA_MENU_DEBUG=1`: state changes of the menus go to the
/// log (stderr), for the isolated tests and the owner's GUI check.
fn debugging() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("QUERENCIA_MENU_DEBUG").is_ok_and(|v| v == "1"))
}

fn debug(message: &str) {
    if debugging() {
        let time = glib::DateTime::now_local()
            .and_then(|t| t.format("%H:%M:%S.%f"))
            .map(|t| t.chars().take(12).collect::<String>())
            .unwrap_or_default();
        eprintln!("querencia-panel: menü {time}: {message}");
    }
}

/// Whether the main menu of `button` is open.
fn is_open(button: &MenuButton) -> bool {
    button.popover().is_some_and(|p| p.is_visible())
}

/// Pointer entered `button`: if another main menu of its panel is open,
/// close that and open this one.
fn switch_on_hover(button: &MenuButton) {
    if is_open(button) {
        return;
    }
    let menus = main_menus_beside(button);
    let Some((open, _)) = menus
        .iter()
        .find(|(other, _)| other != button && is_open(other))
    else {
        return;
    };
    debug(&format!(
        "Hauptmenü per Zeiger: {} -> {}",
        label_of(open),
        label_of(button)
    ));
    open.popdown();
    button.popup();
}

/// Switches from the open main menu of `button` to its neighbour (see
/// [`neighbour`]); by keyboard the neighbour's first entry is highlighted.
fn switch_main_menu(button: &MenuButton, forward: bool) {
    let menus = main_menus_beside(button);
    let Some(index) = menus.iter().position(|(other, _)| other == button) else {
        return;
    };
    let Some(next) = neighbour(menus.len(), index, forward) else {
        return;
    };
    let (target, current) = &menus[next];
    debug(&format!(
        "Hauptmenü per Taste: {} -> {}",
        label_of(button),
        label_of(target)
    ));
    button.popdown();
    target.popup();
    if let Some(level) = current.borrow().as_ref() {
        level.select(step(&level.selectable(), None, Move::First), true);
    }
}

/// One open menu: the root menu of a panel button or a submenu.
struct Level {
    popover: glib::WeakRef<Popover>,
    rows: Vec<Row>,
    widgets: Vec<Widget>,
    content: GtkBox,
    parent: Option<Weak<Level>>,
    /// The highlighted row.
    active: Cell<Option<usize>>,
    /// The open submenu: row, its popover and its level.
    child: RefCell<Option<(usize, Popover, Rc<Level>)>>,
    /// Pending opening of the hovered row's submenu.
    open_timer: RefCell<Option<glib::SourceId>>,
    /// Pending switch away from a row whose submenu the pointer aims at.
    aim_timer: RefCell<Option<glib::SourceId>>,
    /// Last pointer position on the active row, surface coordinates.
    apex: Cell<Option<(f64, f64)>>,
    /// Last pointer position in `content`.
    pointer: Cell<Option<(f64, f64)>>,
    /// After a key: pointer events at the position the pointer had then are
    /// not a movement (the compositor repeats the position when a submenu
    /// surface goes away) and must not undo what the key did, e.g. reopen
    /// a submenu Escape closed. `Some(None)`: position not yet known.
    key_hold: Cell<Option<Option<(f64, f64)>>>,
}

impl Level {
    fn new(popover: &Popover, model: &gio::MenuModel, parent: Option<Weak<Level>>) -> Rc<Level> {
        let rows = rows(model);
        let content = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .css_classes(["qmenu"])
            .build();
        let widgets: Vec<Widget> = rows.iter().map(row_widget).collect();
        for widget in &widgets {
            content.append(widget);
        }
        popover.set_child(Some(&content));

        let level = Rc::new(Level {
            popover: popover.downgrade(),
            rows,
            widgets,
            content,
            parent,
            active: Cell::new(None),
            child: RefCell::new(None),
            open_timer: RefCell::new(None),
            aim_timer: RefCell::new(None),
            apex: Cell::new(None),
            pointer: Cell::new(None),
            key_hold: Cell::new(None),
        });
        level.connect_input();
        level
    }

    fn connect_input(self: &Rc<Self>) {
        // Events of a submenu bubble up through its row into this menu
        // too; each level only handles those of its own surface.
        let motion = EventControllerMotion::new();
        let weak = Rc::downgrade(self);
        motion.connect_motion(move |controller, x, y| {
            if let Some(level) = weak.upgrade()
                && level.owns(controller)
            {
                level.on_motion(x, y);
            }
        });
        let weak = Rc::downgrade(self);
        motion.connect_leave(move |_| {
            if let Some(level) = weak.upgrade() {
                level.on_leave();
            }
        });
        self.content.add_controller(motion);

        let click = GestureClick::new();
        click.set_button(0);
        let weak = Rc::downgrade(self);
        click.connect_released(move |gesture, _, _, y| {
            if let Some(level) = weak.upgrade()
                && level.owns(gesture)
                && let Some(index) = level.row_index_at(y)
            {
                level.activate(index, false);
            }
        });
        self.content.add_controller(click);

        // Keys: one controller on the top level's popover (`attach`).
    }

    /// Whether the event `controller` is handling happened on this menu's
    /// own surface.
    fn owns(&self, controller: &impl IsA<gtk4::EventController>) -> bool {
        let surface = controller.current_event().and_then(|event| event.surface());
        let own = self.popover.upgrade().and_then(|p| p.surface());
        surface.is_some() && surface == own
    }

    // -- highlight ------------------------------------------------------

    fn select(&self, index: Option<usize>, focus: bool) {
        if let Some(old) = self.active.get()
            && let Some(widget) = self.widgets.get(old)
        {
            widget.remove_css_class(ACTIVE_CLASS);
        }
        self.active.set(index);
        if let Some(widget) = index.and_then(|i| self.widgets.get(i)) {
            widget.add_css_class(ACTIVE_CLASS);
            // GTK keyboard focus only ever in the top level (HMP-05): a row
            // of a submenu with the focus would draw the keys into that
            // submenu's popover. A submenu's highlight is the CSS class.
            if focus && self.parent.is_none() {
                widget.grab_focus();
            }
        }
    }

    fn selectable(&self) -> Vec<bool> {
        self.rows.iter().map(Row::selectable).collect()
    }

    fn row_index_at(&self, y: f64) -> Option<usize> {
        let bands: Vec<(f64, f64)> = self
            .widgets
            .iter()
            .map(|widget| {
                widget
                    .compute_bounds(&self.content)
                    .map(|b| (f64::from(b.y()), f64::from(b.height())))
                    .unwrap_or((0.0, 0.0))
            })
            .collect();
        row_at(&bands, y).filter(|&i| self.rows[i].selectable())
    }

    // -- pointer --------------------------------------------------------

    fn on_motion(self: &Rc<Self>, x: f64, y: f64) {
        // The pointer arrived in this submenu: the menu above keeps the row
        // that opened it, whatever its pending timers would decide.
        if let Some(parent) = self.parent.as_ref().and_then(Weak::upgrade) {
            cancel(&parent.aim_timer);
            cancel(&parent.open_timer);
            parent.pointer.set(None);
        }
        if !self.pointer_moved(x, y) {
            return;
        }
        self.pointer.set(Some((x, y)));
        let here = self.to_surface(x, y);
        let Some(index) = self.row_index_at(y) else {
            return;
        };
        if Some(index) == self.active.get() {
            self.apex.set(here);
            cancel(&self.aim_timer);
            if self.child_row().is_none() && self.open_timer.borrow().is_none() {
                self.schedule_open(index);
            }
            return;
        }
        // Moving from the row of the open submenu towards that submenu:
        // the rows on the way do not take over (for a while).
        if let (Some(apex), Some(here), Some(rect)) = (self.apex.get(), here, self.child_rect())
            && self.child_row() == self.active.get()
            && aiming(apex, here, rect)
        {
            if self.aim_timer.borrow().is_none() {
                self.log_state(&format!(
                    "Zielbewegung zum Untermenü über {}, Wechsel höchstens {AIM_TIMEOUT_MS} ms gesperrt",
                    self.label(Some(index))
                ));
                let weak = Rc::downgrade(self);
                let id = glib::timeout_add_local_once(
                    std::time::Duration::from_millis(AIM_TIMEOUT_MS),
                    move || {
                        if let Some(level) = weak.upgrade() {
                            level.aim_timer.take();
                            level.log_state("Zielsperre abgelaufen");
                            level.follow_pointer();
                        }
                    },
                );
                self.aim_timer.replace(Some(id));
            }
            return;
        }
        self.switch_to(index);
    }

    /// Whether `(x, y)` is a real pointer movement, given a key hold (see
    /// [`Level::key_hold`]); ends the hold once the pointer really moves.
    fn pointer_moved(&self, x: f64, y: f64) -> bool {
        let held = self.key_hold.get().is_some();
        let (moved, hold) = hold_step(self.key_hold.get(), x, y);
        self.key_hold.set(hold);
        if held && moved {
            self.log_state("Zeiger bewegt, Tastatursperre aufgehoben");
        }
        moved
    }

    /// Holds this level and every open submenu below it against pointer
    /// events at the current pointer position (see [`Level::key_hold`]).
    fn hold_for_keyboard(&self) {
        self.key_hold.set(Some(self.pointer.get()));
        cancel(&self.open_timer);
        cancel(&self.aim_timer);
        let child = self.child.borrow().as_ref().map(|(_, _, c)| c.clone());
        if let Some(child) = child {
            child.hold_for_keyboard();
        }
    }

    /// Highlights the row under the last pointer position, if it is
    /// another one.
    fn follow_pointer(self: &Rc<Self>) {
        if let Some((_, y)) = self.pointer.get()
            && let Some(index) = self.row_index_at(y)
            && Some(index) != self.active.get()
        {
            self.switch_to(index);
        }
    }

    fn switch_to(self: &Rc<Self>, index: usize) {
        cancel(&self.aim_timer);
        cancel(&self.open_timer);
        if self.child_row().is_some_and(|row| row != index) {
            self.close_child();
        }
        self.select(Some(index), true);
        self.apex.set(None);
        if self.child_row().is_none() {
            self.schedule_open(index);
        }
        self.log_state(&format!("Zeiger auf {}", self.label(Some(index))));
    }

    fn on_leave(&self) {
        cancel(&self.aim_timer);
        cancel(&self.open_timer);
        self.pointer.set(None);
        // GTK 2 keeps the row of an open submenu highlighted.
        if self.child_row().is_none() {
            self.select(None, false);
        }
        self.log_state("Zeiger verlässt das Menü");
    }

    /// `(x, y)` in `content` as coordinates of this menu's surface.
    fn to_surface(&self, x: f64, y: f64) -> Option<(f64, f64)> {
        let popover = self.popover.upgrade()?;
        let point = self
            .content
            .compute_point(&popover, &graphene::Point::new(x as f32, y as f32))?;
        let (dx, dy) = popover.surface_transform();
        Some((f64::from(point.x()) + dx, f64::from(point.y()) + dy))
    }

    // -- submenus -------------------------------------------------------

    fn child_row(&self) -> Option<usize> {
        self.child.borrow().as_ref().map(|(row, _, _)| *row)
    }

    /// The open submenu's surface relative to this menu's surface.
    fn child_rect(&self) -> Option<Rect> {
        let child = self.child.borrow();
        let (_, popover, _) = child.as_ref()?;
        let surface = popover.surface()?;
        let popup = surface.downcast_ref::<gdk::Popup>()?;
        Some(Rect {
            x: f64::from(popup.position_x()),
            y: f64::from(popup.position_y()),
            width: f64::from(surface.width()),
            height: f64::from(surface.height()),
        })
    }

    fn schedule_open(self: &Rc<Self>, index: usize) {
        if !matches!(self.rows.get(index), Some(Row::Submenu { .. })) {
            return;
        }
        let weak = Rc::downgrade(self);
        let id = glib::timeout_add_local_once(
            std::time::Duration::from_millis(SUBMENU_DELAY_MS),
            move || {
                if let Some(level) = weak.upgrade() {
                    level.open_timer.take();
                    // Cancelled on every change of row; checked again anyway.
                    if level.active.get() == Some(index) {
                        level.open_child(index, false);
                    } else {
                        level.log_state("veraltete Öffnung verworfen");
                    }
                }
            },
        );
        cancel(&self.open_timer);
        self.open_timer.replace(Some(id));
    }

    fn open_child(self: &Rc<Self>, index: usize, keyboard: bool) {
        cancel(&self.open_timer);
        if self.child_row() == Some(index) {
            if keyboard && let Some((_, _, level)) = self.child.borrow().as_ref() {
                level.select(step(&level.selectable(), None, Move::First), true);
            }
            return;
        }
        self.close_child();
        let Some(Row::Submenu { model, .. }) = self.rows.get(index) else {
            return;
        };
        let popover = menu_popover();
        popover.set_position(PositionType::Right);
        popover.set_valign(Align::Start);
        // Submenus never grab (HMP-05). Only the top level of a main menu
        // grabs, like the menu shell of GTK 2: a grabbing GtkPopover calls
        // gtk_grab_add(), and GTK then hands every pointer event outside it
        // to it, with coordinates of the wrong surface. A grabbing submenu
        // therefore swallowed all pointer motion over the menu above: the
        // row that opened it stayed highlighted and its submenu open.
        popover.set_autohide(false);
        popover.set_cascade_popdown(false);
        popover.set_offset(0, -MENU_TOP_INSET);
        popover.set_parent(&self.widgets[index]);
        let level = Level::new(&popover, model, Some(Rc::downgrade(self)));
        // Should a key ever reach a submenu popover, it goes to the top level
        // like all keys (see `attach`).
        let keys = EventControllerKey::new();
        keys.set_propagation_phase(PropagationPhase::Capture);
        let root = Rc::downgrade(&self.root());
        keys.connect_key_pressed(move |_, key, _, _| match root.upgrade() {
            Some(root) => root.on_key(key),
            None => glib::Propagation::Proceed,
        });
        popover.add_controller(keys);
        let weak = Rc::downgrade(self);
        popover.connect_closed(move |popover| {
            if let Some(parent) = weak.upgrade() {
                let mine = parent
                    .child
                    .borrow()
                    .as_ref()
                    .is_some_and(|(_, open, _)| open == popover);
                if mine && let Some((_, _, level)) = parent.child.take() {
                    level.close_child();
                }
            }
            // Not from within the popover's own signal.
            let popover = popover.clone();
            glib::idle_add_local_once(move || {
                if popover.parent().is_some() {
                    popover.unparent();
                }
            });
        });
        self.child
            .replace(Some((index, popover.clone(), level.clone())));
        self.select(Some(index), false);
        popover.popup();
        self.log_state(&format!(
            "Untermenü {} geöffnet ({})",
            self.label(Some(index)),
            if keyboard { "Tastatur" } else { "Zeiger" }
        ));
        if keyboard {
            level.select(step(&level.selectable(), None, Move::First), true);
        }
    }

    fn close_child(&self) {
        let child = self.child.take();
        if let Some((_, popover, level)) = child {
            level.close_child();
            popover.popdown();
        }
    }

    fn root(self: &Rc<Self>) -> Rc<Level> {
        let mut level = self.clone();
        while let Some(parent) = level.parent.as_ref().and_then(Weak::upgrade) {
            level = parent;
        }
        level
    }

    // -- activation and keyboard ----------------------------------------

    fn activate(self: &Rc<Self>, index: usize, keyboard: bool) {
        match &self.rows[index] {
            Row::Submenu { .. } => {
                self.select(Some(index), true);
                self.open_child(index, keyboard);
            }
            Row::Entry {
                action: Some(action),
                target,
                ..
            } => {
                let widget = self.widgets[index].clone();
                let root = self.root();
                root.close_child();
                if let Some(popover) = root.popover.upgrade() {
                    popover.popdown();
                }
                // The row is still in the widget tree below the panel
                // button, which holds the menu's action groups.
                if let Err(err) = widget.activate_action(action, target.as_ref()) {
                    eprintln!("querencia-panel: Menüaktion {action} nicht ausführbar: {err}");
                }
            }
            _ => {}
        }
    }

    fn on_key(self: &Rc<Self>, key: gdk::Key) -> glib::Propagation {
        // Only the top level grabs and has the keyboard (HMP-05), so every
        // key arrives here; it belongs to the innermost menu the keyboard
        // works in: the deepest open submenu with a highlighted row.
        let target = self.keyboard_level();
        self.hold_for_keyboard();
        debug(&format!(
            "Taste {} -> Menü {}",
            key.name().unwrap_or_default(),
            target.describe()
        ));
        target.handle_key(key)
    }

    /// The innermost level below (or) this one whose open submenu has a
    /// highlighted row, following open submenus.
    fn keyboard_level(self: &Rc<Self>) -> Rc<Level> {
        let mut level = self.clone();
        loop {
            let child = level.child.borrow().as_ref().map(|(_, _, c)| c.clone());
            match child {
                Some(child) if child.active.get().is_some() => level = child,
                _ => return level,
            }
        }
    }

    fn handle_key(self: &Rc<Self>, key: gdk::Key) -> glib::Propagation {
        if key == gdk::Key::Escape {
            // GNOME 2: Escape closes the innermost open menu only.
            if self.child_row().is_some() {
                self.close_child();
                self.log_state("Escape schließt Untermenü");
                return glib::Propagation::Stop;
            }
            if self.parent.is_some() {
                if let Some(popover) = self.popover.upgrade() {
                    popover.popdown();
                }
                return glib::Propagation::Stop;
            }
            debug("Escape schließt das Hauptmenü");
            if let Some(popover) = self.popover.upgrade() {
                popover.popdown();
            }
            return glib::Propagation::Stop;
        }
        let how = match key {
            gdk::Key::Down | gdk::Key::KP_Down | gdk::Key::Tab => Some(Move::Next),
            gdk::Key::Up | gdk::Key::KP_Up | gdk::Key::ISO_Left_Tab => Some(Move::Previous),
            gdk::Key::Home | gdk::Key::KP_Home => Some(Move::First),
            gdk::Key::End | gdk::Key::KP_End => Some(Move::Last),
            _ => None,
        };
        if let Some(how) = how {
            cancel(&self.open_timer);
            if self.child_row().is_some() {
                self.close_child();
            }
            let next = step(&self.selectable(), self.active.get(), how);
            self.select(next, true);
            return glib::Propagation::Stop;
        }
        match key {
            gdk::Key::Right | gdk::Key::KP_Right => {
                match self.active.get() {
                    Some(index) if matches!(self.rows[index], Row::Submenu { .. }) => {
                        self.open_child(index, true);
                    }
                    // GNOME 2: Right on anything else goes to the next
                    // menu of the menu bar.
                    _ => self.root().switch_main_menu(true),
                }
                glib::Propagation::Stop
            }
            gdk::Key::Left | gdk::Key::KP_Left if self.parent.is_some() => {
                if let Some(popover) = self.popover.upgrade() {
                    popover.popdown();
                }
                glib::Propagation::Stop
            }
            gdk::Key::Left | gdk::Key::KP_Left => {
                self.switch_main_menu(false);
                glib::Propagation::Stop
            }
            gdk::Key::Return | gdk::Key::KP_Enter | gdk::Key::ISO_Enter | gdk::Key::space => {
                if let Some(index) = self.active.get() {
                    self.activate(index, true);
                }
                glib::Propagation::Stop
            }
            _ => glib::Propagation::Proceed,
        }
    }

    /// Switches from this top level's main menu to the neighbouring one.
    fn switch_main_menu(&self, forward: bool) {
        let button = self
            .popover
            .upgrade()
            .and_then(|popover| popover.parent())
            .and_then(|parent| parent.downcast::<MenuButton>().ok());
        if let Some(button) = button {
            switch_main_menu(&button, forward);
        }
    }

    // -- debug log ------------------------------------------------------

    fn label(&self, index: Option<usize>) -> String {
        match index.and_then(|i| self.rows.get(i)) {
            Some(Row::Entry { label, .. } | Row::Submenu { label, .. }) => label.clone(),
            _ => "-".into(),
        }
    }

    /// This level's highlighted row and open submenu.
    fn describe(&self) -> String {
        format!(
            "[aktiv={} offen={}]",
            self.label(self.active.get()),
            self.label(self.child_row())
        )
    }

    /// Logs `what` with the state of this level and the submenus below it.
    fn log_state(&self, what: &str) {
        if !debugging() {
            return;
        }
        let mut state = self.describe();
        let mut child = self.child.borrow().as_ref().map(|(_, _, c)| c.clone());
        while let Some(level) = child {
            state.push_str(&format!(" > {}", level.describe()));
            child = level.child.borrow().as_ref().map(|(_, _, c)| c.clone());
        }
        let mut depth = 0;
        let mut parent = self.parent.as_ref().and_then(Weak::upgrade);
        while let Some(level) = parent {
            depth += 1;
            parent = level.parent.as_ref().and_then(Weak::upgrade);
        }
        debug(&format!("Ebene {depth}: {what}: {state}"));
    }
}

/// Removes the pending source in `timer`, if any.
fn cancel(timer: &RefCell<Option<glib::SourceId>>) {
    if let Some(id) = timer.take() {
        id.remove();
    }
}

/// The widget for one row: icon column, label, ▸ for a submenu; a line for
/// a separator. Rows that cannot be chosen are insensitive.
fn row_widget(row: &Row) -> Widget {
    let (label, icon, submenu, accel) = match row {
        Row::Separator => {
            return Separator::new(Orientation::Horizontal).upcast();
        }
        Row::Entry {
            label, icon, accel, ..
        } => (label, icon, false, accel.as_deref()),
        Row::Submenu { label, icon, .. } => (label, icon, true, None),
    };
    let line = GtkBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(6)
        .css_classes([ITEM_CLASS])
        .focusable(true)
        .accessible_role(AccessibleRole::MenuItem)
        .sensitive(row.selectable())
        .build();
    let image = Image::new();
    image.set_pixel_size(ICON_SIZE);
    if let Some(icon) = icon.as_ref().map(usable_icon) {
        show_icon(&image, &icon);
    }
    line.append(&image);
    line.append(
        &Label::builder()
            .label(label)
            .xalign(0.0)
            .hexpand(true)
            .build(),
    );
    if let Some(accel) = accel {
        let shortcut = Label::builder().label(accel).xalign(1.0).build();
        shortcut.add_css_class(ACCEL_CLASS);
        // GTK 2 kept the shortcut column apart from the label
        shortcut.set_margin_start(18);
        line.append(&shortcut);
    }
    if submenu {
        let arrow = Image::new();
        arrow.set_pixel_size(ICON_SIZE);
        show_icon(
            &arrow,
            gio::ThemedIcon::new("pan-end-symbolic").upcast_ref(),
        );
        arrow.add_css_class(ARROW_CLASS);
        line.append(&arrow);
    }
    line.upcast()
}

/// How the rows of the classic menus show their icons ([`set_icon_loading`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IconLoading {
    /// Without GTK's preloading in the thread pool ([`crate::symbole`],
    /// QD-106): the way for every program, and a must for one that shows
    /// many icons.
    #[default]
    WithoutPreload,
    /// GTK's own way (`Image::set_from_gicon`), which the desktop kept up
    /// to 0.2.7.
    Gtk,
}

thread_local! {
    /// The way chosen with [`set_icon_loading`]; GTK runs on one thread.
    static ICON_LOADING: Cell<IconLoading> = const { Cell::new(IconLoading::WithoutPreload) };
}

/// Chooses how menu rows show their icons, for menus built from now on;
/// call once at start, before the first menu opens.
pub fn set_icon_loading(how: IconLoading) {
    ICON_LOADING.with(|loading| loading.set(how));
}

/// Shows `icon` on `image` the way chosen with [`set_icon_loading`].
fn show_icon(image: &Image, icon: &gio::Icon) {
    match ICON_LOADING.with(Cell::get) {
        IconLoading::WithoutPreload => crate::symbole::set(image, icon),
        IconLoading::Gtk => image.set_from_gicon(icon),
    }
}

/// `icon`, or the generic one if the theme has none of a themed icon's
/// names (instead of GTK's broken-image picture). Icons from files are
/// taken as they are.
fn usable_icon(icon: &gio::Icon) -> gio::Icon {
    let Some(display) = gdk::Display::default() else {
        return icon.clone();
    };
    if icon.is::<gio::ThemedIcon>() && !gtk4::IconTheme::for_display(&display).has_gicon(icon) {
        return gio::ThemedIcon::new(MISSING_ICON).upcast();
    }
    icon.clone()
}

// The isolated GUI test of the menus; a program's test enables the feature
// and hands over its own menu buttons (the panel: `menu_gui_test.rs`).
#[cfg(feature = "gui-test")]
pub mod gui_test;

#[cfg(test)]
mod tests {
    use super::*;

    /// First name of a themed icon.
    fn icon_name(icon: &Option<gio::Icon>) -> String {
        icon.as_ref()
            .and_then(|i| i.downcast_ref::<gio::ThemedIcon>().map(|t| t.names()))
            .and_then(|names| names.first().map(|n| n.to_string()))
            .unwrap_or_default()
    }

    fn summary(rows: &[Row]) -> Vec<String> {
        rows.iter()
            .map(|row| match row {
                Row::Separator => "---".to_string(),
                Row::Entry {
                    label,
                    icon,
                    action,
                    target,
                    accel,
                } => format!(
                    "{label} [{}] -> {}{}{}",
                    icon_name(icon),
                    action.as_deref().unwrap_or("-"),
                    target
                        .as_ref()
                        .and_then(|t| t.str().map(|s| format!("({s})")))
                        .unwrap_or_default(),
                    accel
                        .as_ref()
                        .map(|a| format!(" {{{a}}}"))
                        .unwrap_or_default()
                ),
                Row::Submenu { label, icon, model } => format!(
                    "{label} [{}] > {} Einträge",
                    icon_name(icon),
                    model.n_items()
                ),
            })
            .collect()
    }

    fn entry(label: &str, icon: &str, action: Option<&str>, target: Option<&str>) -> gio::MenuItem {
        let item = gio::MenuItem::new(Some(label), None);
        if let Some(action) = action {
            item.set_action_and_target_value(Some(action), target.map(|t| t.to_variant()).as_ref());
        }
        if !icon.is_empty() {
            item.set_icon(&gio::ThemedIcon::new(icon));
        }
        item
    }

    #[test]
    fn entries_submenus_and_icons_become_rows() {
        let sub = gio::Menu::new();
        sub.append(Some("Firefox"), Some("menu.launch"));
        let menu = gio::Menu::new();
        let submenu = gio::MenuItem::new_submenu(Some("Internet"), &sub);
        submenu.set_icon(&gio::ThemedIcon::new("applications-internet"));
        menu.append_item(&submenu);
        menu.append_item(&entry(
            "Vim",
            "gvim",
            Some("menu.launch"),
            Some("vim.desktop"),
        ));
        menu.append_item(&entry("Platzhalter", "", None, None));
        assert_eq!(
            summary(&rows(menu.upcast_ref())),
            [
                "Internet [applications-internet] > 1 Einträge",
                "Vim [gvim] -> menu.launch(vim.desktop)",
                "Platzhalter [] -> -",
            ]
        );
    }

    /// G2-AK: "Bildschirm sperren" shows Strg+Alt+L as GTK 2 menus did.
    #[test]
    fn shortcuts_are_shown_in_german() {
        let menu = gio::Menu::new();
        let lock = entry(
            "Bildschirm sperren",
            "system-lock-screen",
            Some("x.lock"),
            None,
        );
        lock.set_attribute_value("accel", Some(&"<Control><Alt>l".to_variant()));
        menu.append_item(&lock);
        let broken = entry("Kaputt", "", Some("x.b"), None);
        broken.set_attribute_value("accel", Some(&"<Hyper>".to_variant()));
        menu.append_item(&broken);
        assert_eq!(
            summary(&rows(menu.upcast_ref())),
            [
                "Bildschirm sperren [system-lock-screen] -> x.lock {Ctrl+Alt+L}",
                "Kaputt [] -> x.b",
            ]
        );
        for (accel, text) in [
            // without a catalog in the test: the English names
            ("<Control><Alt>l", Some("Ctrl+Alt+L")),
            ("<Primary><Shift>Delete", Some("Ctrl+Shift+Del")),
            ("<Super>f", Some("Super+F")),
            ("<Alt>F11", Some("Alt+F11")),
            ("F11", Some("F11")),
            ("<Alt>Print", Some("Alt+Print")),
            ("", None),
            ("<Control>", None),
            ("<Hyper>x", None),
            ("<Control", None),
            ("a b", None),
        ] {
            assert_eq!(accel_label(accel).as_deref(), text, "{accel:?}");
        }
    }

    #[test]
    fn sections_are_separated_by_lines_only_between_them() {
        let first = gio::Menu::new();
        first.append(Some("A"), Some("x.a"));
        let empty = gio::Menu::new();
        let second = gio::Menu::new();
        second.append(Some("B"), Some("x.b"));
        second.append(Some("C"), Some("x.c"));
        let menu = gio::Menu::new();
        menu.append_section(None, &empty);
        menu.append_section(None, &first);
        menu.append_section(None, &empty);
        menu.append_section(None, &second);
        menu.append_section(None, &empty);
        assert_eq!(
            summary(&rows(menu.upcast_ref())),
            ["A [] -> x.a", "---", "B [] -> x.b", "C [] -> x.c"]
        );
    }

    #[test]
    fn plain_items_next_to_sections_are_separated_too() {
        let section = gio::Menu::new();
        section.append(Some("B"), Some("x.b"));
        let menu = gio::Menu::new();
        menu.append(Some("A"), Some("x.a"));
        menu.append_section(None, &section);
        menu.append(Some("C"), Some("x.c"));
        assert_eq!(
            summary(&rows(menu.upcast_ref())),
            ["A [] -> x.a", "---", "B [] -> x.b", "---", "C [] -> x.c"]
        );
        assert!(rows(gio::Menu::new().upcast_ref()).is_empty());
    }

    #[test]
    fn only_entries_with_an_action_and_filled_submenus_are_selectable() {
        let filled = gio::Menu::new();
        filled.append(Some("A"), Some("x.a"));
        let menu = gio::Menu::new();
        menu.append(Some("Mit Aktion"), Some("x.go"));
        menu.append(Some("Ohne Aktion"), None);
        menu.append_submenu(Some("Voll"), &filled);
        menu.append_submenu(Some("Leer"), &gio::Menu::new());
        let section = gio::Menu::new();
        section.append(Some("B"), Some("x.b"));
        menu.append_section(None, &section);
        let selectable: Vec<bool> = rows(menu.upcast_ref())
            .iter()
            .map(Row::selectable)
            .collect();
        assert_eq!(selectable, [true, false, true, false, false, true]);
    }

    #[test]
    fn keyboard_moves_over_selectable_rows_and_wraps() {
        // separator at 1, insensitive entry at 3
        let selectable = [true, false, true, false, true];
        assert_eq!(step(&selectable, None, Move::Next), Some(0));
        assert_eq!(step(&selectable, None, Move::Previous), Some(4));
        assert_eq!(step(&selectable, Some(0), Move::Next), Some(2));
        assert_eq!(step(&selectable, Some(2), Move::Next), Some(4));
        assert_eq!(step(&selectable, Some(4), Move::Next), Some(0), "wraps");
        assert_eq!(step(&selectable, Some(0), Move::Previous), Some(4), "wraps");
        assert_eq!(step(&selectable, Some(4), Move::Previous), Some(2));
        assert_eq!(step(&selectable, Some(2), Move::First), Some(0));
        assert_eq!(step(&selectable, Some(0), Move::Last), Some(4));
        assert_eq!(step(&[false, false], None, Move::Next), None);
        assert_eq!(step(&[], Some(0), Move::Next), None);
        // a current index past the end is treated as none
        assert_eq!(step(&selectable, Some(9), Move::Next), Some(0));
    }

    #[test]
    fn row_under_the_pointer() {
        // (top, height) of three rows with a separator gap
        let rows = [(2.0, 22.0), (24.0, 7.0), (31.0, 22.0)];
        assert_eq!(row_at(&rows, 2.0), Some(0));
        assert_eq!(row_at(&rows, 23.9), Some(0));
        assert_eq!(row_at(&rows, 24.0), Some(1));
        assert_eq!(row_at(&rows, 52.9), Some(2));
        assert_eq!(row_at(&rows, 53.0), None);
        assert_eq!(row_at(&rows, 0.5), None, "padding above the first row");
        assert_eq!(row_at(&[], 5.0), None);
    }

    #[test]
    fn aiming_at_a_submenu_to_the_right() {
        // pointer left the row at (100, 30); submenu from x 150, y 20..220
        let submenu = Rect {
            x: 150.0,
            y: 20.0,
            width: 180.0,
            height: 200.0,
        };
        let apex = (100.0, 30.0);
        // diagonally down towards the lower part of the submenu
        assert!(aiming(apex, (120.0, 60.0), submenu));
        assert!(aiming(apex, (140.0, 150.0), submenu));
        // straight down: not towards the submenu
        assert!(!aiming(apex, (100.0, 60.0), submenu));
        // down too steeply for the remaining distance
        assert!(!aiming(apex, (110.0, 120.0), submenu));
        // up too steeply: above the submenu's top corner
        assert!(!aiming(apex, (110.0, 10.0), submenu));
        // up towards its top part is fine
        assert!(aiming(apex, (120.0, 26.0), submenu));
        // away from it
        assert!(!aiming(apex, (90.0, 40.0), submenu));
        // standing still is no movement towards it
        assert!(!aiming(apex, apex, submenu));
    }

    #[test]
    fn aiming_at_a_submenu_flipped_to_the_left() {
        let submenu = Rect {
            x: 0.0,
            y: 0.0,
            width: 80.0,
            height: 300.0,
        };
        let apex = (120.0, 50.0);
        assert!(aiming(apex, (100.0, 90.0), submenu));
        assert!(!aiming(apex, (140.0, 90.0), submenu));
    }

    #[test]
    fn no_aiming_without_a_usable_submenu() {
        let empty = Rect {
            x: 150.0,
            y: 20.0,
            width: 0.0,
            height: 0.0,
        };
        assert!(!aiming((100.0, 30.0), (120.0, 40.0), empty));
        // pointer already level with or past the submenu's edge
        let submenu = Rect {
            x: 150.0,
            y: 20.0,
            width: 180.0,
            height: 200.0,
        };
        assert!(!aiming((160.0, 30.0), (170.0, 40.0), submenu));
    }

    #[test]
    fn neighbouring_main_menu_wraps_like_the_gnome2_menu_bar() {
        // Anwendungen | Orte | System
        assert_eq!(neighbour(3, 0, true), Some(1));
        assert_eq!(neighbour(3, 1, true), Some(2));
        assert_eq!(neighbour(3, 2, true), Some(0), "wraps to the first");
        assert_eq!(neighbour(3, 0, false), Some(2), "wraps to the last");
        assert_eq!(neighbour(3, 2, false), Some(1));
        // a single main menu has no neighbour; nothing to switch to
        assert_eq!(neighbour(1, 0, true), None);
        assert_eq!(neighbour(0, 0, true), None);
        assert_eq!(neighbour(3, 3, true), None, "unknown position");
    }

    fn area(x: f64, y: f64, width: f64, height: f64) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn main_menu_under_the_pointer_on_the_panel() {
        // Anwendungen 10..146, Orte 156..211, System 221..286 (gaps between),
        // each over the whole height of a 30 px panel
        let buttons = [
            area(10.0, 0.0, 136.0, 30.0),
            area(156.0, 0.0, 55.0, 30.0),
            area(221.0, 0.0, 65.0, 30.0),
        ];
        assert_eq!(button_at(&buttons, 10.0, 15.0), Some(0));
        assert_eq!(button_at(&buttons, 145.9, 15.0), Some(0));
        assert_eq!(button_at(&buttons, 146.0, 15.0), None, "gap");
        assert_eq!(button_at(&buttons, 183.0, 15.0), Some(1));
        assert_eq!(button_at(&buttons, 253.0, 15.0), Some(2));
        assert_eq!(button_at(&buttons, 900.0, 15.0), None, "clock, window list");
        // the screen edge and the bottom row of the panel still count
        assert_eq!(button_at(&buttons, 183.0, 0.0), Some(1), "top edge");
        assert_eq!(button_at(&buttons, 183.0, 29.9), Some(1), "bottom row");
        assert_eq!(button_at(&[], 50.0, 15.0), None);
    }

    #[test]
    fn pointer_below_a_menu_bar_is_over_no_main_menu() {
        // QD-176, the window of Querencia Dateien at 1024 x 768: title bar
        // above, menu bar at y 28..52 (Datei, Bearbeiten, Ansicht, Gehe zu),
        // a context menu open at 215,270 below. The pointer under the menu
        // at x 250 (the column of Ansicht) must not open Ansicht.
        let bar = [
            area(3.0, 28.0, 81.0, 24.0),
            area(84.0, 28.0, 124.0, 24.0),
            area(208.0, 28.0, 80.0, 24.0),
            area(288.0, 28.0, 112.0, 24.0),
        ];
        assert_eq!(button_at(&bar, 250.0, 500.0), None, "below the bar");
        assert_eq!(button_at(&bar, 250.0, 271.0), None, "at the click");
        assert_eq!(button_at(&bar, 250.0, 27.9), None, "title bar");
        assert_eq!(button_at(&bar, 250.0, 52.0), None, "tool bar");
        assert_eq!(button_at(&bar, 250.0, 40.0), Some(2), "on Ansicht");
        assert_eq!(button_at(&bar, 40.0, 28.0), Some(0), "on Datei");
    }

    #[test]
    fn a_key_holds_until_the_pointer_really_moves() {
        // no hold: every event is a movement
        assert_eq!(hold_step(None, 10.0, 10.0), (true, None));
        // hold without a known position: the first event only records it
        assert_eq!(
            hold_step(Some(None), 50.0, 46.0),
            (false, Some(Some((50.0, 46.0))))
        );
        // the compositor repeating that position is no movement, however
        // often and however long after the key
        let hold = Some(Some((50.0, 46.0)));
        for _ in 0..5 {
            assert_eq!(hold_step(hold, 50.0, 46.0), (false, hold));
        }
        assert_eq!(
            hold_step(hold, 52.0, 44.5),
            (false, hold),
            "within tolerance"
        );
        // a real movement ends the hold
        assert_eq!(hold_step(hold, 50.0, 70.0), (true, None));
        assert_eq!(hold_step(hold, 54.0, 46.0), (true, None));
    }

    #[test]
    fn gtk2_timings() {
        assert_eq!(SUBMENU_DELAY_MS, 225, "gtk-menu-popup-delay of GTK 2");
        assert!((200..=500).contains(&AIM_TIMEOUT_MS));
    }
}
