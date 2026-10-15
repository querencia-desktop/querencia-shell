//! Classic GNOME 2 menus for the three main panel menus (HMP-03).
//!
//! GTK 4's `PopoverMenu` shows no icons next to text and either slides
//! submenus in place or opens them beside the menu with an 80 ms timer
//! that loses diagonal pointer movement (`docs/m1-1b1d-…`, `m1-1b1e-…`).
//! This module draws the menus itself from the same `gio::Menu` models:
//!
//! - each row: 16 px icon, label, ▸ for a submenu; sections become
//!   separator lines;
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
//!   with its target on the panel button, as `PopoverMenu` would.
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

/// A rectangle in surface coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
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

/// CSS class of every classic menu popover (style in `main.rs`).
pub const MENU_CLASS: &str = "querencia-menu";
/// CSS class of a menu row; [`ACTIVE_CLASS`] marks the highlighted one.
pub const ITEM_CLASS: &str = "qmenu-item";
pub const ACTIVE_CLASS: &str = "qmenu-active";
const ARROW_CLASS: &str = "qmenu-arrow";
const ICON_SIZE: i32 = 16;
/// Icon of an entry whose icon the theme does not have.
const MISSING_ICON: &str = "application-x-executable";
/// Padding plus border above the first row (CSS in `main.rs`): a submenu is
/// moved up by it, so its first row lines up with the row that opened it.
const MENU_TOP_INSET: i32 = 3;

/// Gives `button` a classic menu for `model` instead of GTK's
/// `PopoverMenu`. The rows are built afresh each time the menu opens, so
/// they always show the model as it is then; the model is only changed
/// while the menu is closed (`app_menu::follow_changes`).
pub fn attach(button: &MenuButton, model: &gio::MenuModel) {
    let popover = Popover::builder()
        .has_arrow(false)
        .halign(Align::Start)
        .css_classes([MENU_CLASS])
        .build();
    button.set_popover(Some(&popover));

    let current: Rc<RefCell<Option<Rc<Level>>>> = Rc::default();
    popover.connect_show(glib::clone!(
        #[strong]
        current,
        #[strong]
        model,
        move |popover| {
            current.replace(Some(Level::new(popover, &model, None)));
        }
    ));
    popover.connect_closed(move |_| {
        if let Some(level) = current.take() {
            level.close_child();
        }
    });
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

        // On the content, not the popover: the root popover stays while
        // its content is rebuilt at every opening, and controllers on it
        // would pile up. The focus is always on a row of the content.
        let keys = EventControllerKey::new();
        keys.set_propagation_phase(PropagationPhase::Capture);
        let weak = Rc::downgrade(self);
        keys.connect_key_pressed(move |_, key, _, _| match weak.upgrade() {
            Some(level) => level.on_key(key),
            None => glib::Propagation::Proceed,
        });
        self.content.add_controller(keys);
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
            if focus {
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
                let weak = Rc::downgrade(self);
                let id = glib::timeout_add_local_once(
                    std::time::Duration::from_millis(AIM_TIMEOUT_MS),
                    move || {
                        if let Some(level) = weak.upgrade() {
                            level.aim_timer.take();
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
    }

    fn on_leave(&self) {
        cancel(&self.aim_timer);
        cancel(&self.open_timer);
        self.pointer.set(None);
        // GTK 2 keeps the row of an open submenu highlighted.
        if self.child_row().is_none() {
            self.select(None, false);
        }
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
                    if level.active.get() == Some(index) {
                        level.open_child(index, false);
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
        let popover = Popover::builder()
            .has_arrow(false)
            .position(PositionType::Right)
            .valign(Align::Start)
            .autohide(true)
            .cascade_popdown(false)
            .css_classes([MENU_CLASS])
            .build();
        popover.set_offset(0, -MENU_TOP_INSET);
        popover.set_parent(&self.widgets[index]);
        let level = Level::new(&popover, model, Some(Rc::downgrade(self)));
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
        // Only the innermost open menu reacts; keys for a submenu pass
        // through this one first.
        if self.child_row().is_some() && self.child_has_focus() {
            return glib::Propagation::Proceed;
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
                if let Some(index) = self.active.get()
                    && matches!(self.rows[index], Row::Submenu { .. })
                {
                    self.open_child(index, true);
                }
                glib::Propagation::Stop
            }
            gdk::Key::Left | gdk::Key::KP_Left if self.parent.is_some() => {
                if let Some(popover) = self.popover.upgrade() {
                    popover.popdown();
                }
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

    /// Whether the keyboard focus is in the open submenu.
    fn child_has_focus(&self) -> bool {
        let child = self.child.borrow();
        let Some((_, popover, _)) = child.as_ref() else {
            return false;
        };
        popover.is_visible()
            && popover
                .root()
                .and_then(|root| root.focus())
                .is_some_and(|focus| {
                    focus.is_ancestor(popover) || focus == *popover.upcast_ref::<Widget>()
                })
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
    let (label, icon, submenu) = match row {
        Row::Separator => {
            return Separator::new(Orientation::Horizontal).upcast();
        }
        Row::Entry { label, icon, .. } => (label, icon, false),
        Row::Submenu { label, icon, .. } => (label, icon, true),
    };
    let line = GtkBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(6)
        .css_classes([ITEM_CLASS])
        .focusable(true)
        .accessible_role(AccessibleRole::MenuItem)
        .sensitive(row.selectable())
        .build();
    let image = match icon.as_ref().map(usable_icon) {
        Some(icon) => Image::from_gicon(&icon),
        None => Image::new(),
    };
    image.set_pixel_size(ICON_SIZE);
    line.append(&image);
    line.append(
        &Label::builder()
            .label(label)
            .xalign(0.0)
            .hexpand(true)
            .build(),
    );
    if submenu {
        let arrow = Image::from_icon_name("pan-end-symbolic");
        arrow.set_pixel_size(ICON_SIZE);
        arrow.add_css_class(ARROW_CLASS);
        line.append(&arrow);
    }
    line.upcast()
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
                } => format!(
                    "{label} [{}] -> {}{}",
                    icon_name(icon),
                    action.as_deref().unwrap_or("-"),
                    target
                        .as_ref()
                        .and_then(|t| t.str().map(|s| format!("({s})")))
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
    fn gtk2_timings() {
        assert_eq!(SUBMENU_DELAY_MS, 225, "gtk-menu-popup-delay of GTK 2");
        assert!((200..=500).contains(&AIM_TIMEOUT_MS));
    }
}
