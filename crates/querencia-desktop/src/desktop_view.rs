//! The icon layer of the desktop (HMP-09, HMP-10).
//!
//! A second layer-shell surface on the **bottom** layer: above the
//! background (HMP-08), below every window and below the panels (top
//! layer). Exclusive zone 0, so the compositor sizes it to the area between
//! the panels and icons never sit under a panel. It takes the pointer (the
//! desktop's clicks are now ours, as on GNOME 2) and the keyboard only on
//! demand (after a click on the desktop).
//!
//! - Left click selects an icon (or clears the selection), double click
//!   opens it; right click opens the desktop menu or, on an icon, its menu.
//! - HMP-10: pressing the left button on an icon and moving it further than
//!   GTK's drag threshold drags the icon: a translucent copy follows the
//!   pointer, the release puts the icon there (the nearest free spot if
//!   that is taken, always inside the area). Only the icon moves, never the
//!   file. A press that became a drag never counts towards a double click.
//! - HMP-10: every position and the user's own names are kept in the state
//!   file `desktop-symbole` (`store.rs`); refreshes by the file monitor,
//!   restarts and other screen sizes keep them (`layout.rs`).
//! - HMP-10: F2 or "Umbenennen …" in the icon menu edits the label in
//!   place, as Nautilus did: Enter keeps the new name, Escape keeps the old
//!   one, a click elsewhere keeps the new one. Files and folders are renamed
//!   with GIO and keep their place; starters change their visible name.
//! - The menus are the panel's classic GNOME 2 menus (`classic_menu.rs`,
//!   shared source) on an invisible menu button; the menu points at the click.
//! - Only entries that work are shown: "Ordner anlegen", "Dokument anlegen"
//!   ("Leere Datei" and the real files of the XDG templates directory),
//!   "Nach Namen aufräumen" and "Hintergrund der Arbeitsfläche ändern …"
//!   (writes `desktop.ini`, HMP-08 applies it). Not yet: "Starter
//!   anlegen", "Einfügen".
//! - New folders and documents are created with GIO calls that fail on an
//!   existing name, after [`icons::unique_name`] picked a free one: nothing
//!   is ever overwritten. The desktop directory itself is created only by
//!   such a user action.

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use gio_unix::DesktopAppInfo;
use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, EventControllerFocus, EventControllerKey, Fixed,
    GestureClick, GestureDrag, Image, Label, MenuButton, Orientation, PropagationPhase, gdk, gio,
    glib,
};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

use crate::classic_menu;
use crate::config;
use crate::icons::{self, Item, Kind};
use crate::layout::{self, Geometry, Point};
use crate::rename;
use crate::store::{self, Store};

pub const FILES_APP: &str = "io.github.lgse.Strata.desktop";
pub const TERMINAL_APP: &str = "org.example.Querencia.Terminal.desktop";
// Grid cell: wide enough for "Persönlicher" on one line (GNOME 2 broke
// labels between words). One icon with its label (at most two lines)
// takes one cell; cells never overlap.
const CELL: (i32, i32) = (100, 88);
const MARGIN: i32 = 8;
const ICON_PX: i32 = 48;
// The rename box is a little wider than a cell, as Nautilus' was.
const RENAME_WIDTH: i32 = 150;

/// What a `desktop.*` menu action does.
type ActionFn = Box<dyn Fn(&Rc<Icons>, Option<&glib::Variant>)>;

/// A press with the left button that may become a drag (HMP-10, HMP-10a).
struct Drag {
    /// The icon under the pointer at the press; `None`: the free desktop
    /// (a rubber band).
    index: Option<usize>,
    start: (f64, f64),
    /// Pointer position inside the icon under the pointer.
    grab: (f64, f64),
    /// Past the threshold on an icon: the icons moved together (the one
    /// under the pointer first) with their translucent copies.
    moving: Vec<(usize, gtk4::Box)>,
    /// Past the threshold on the free desktop: the rectangle and the
    /// selection it adds to (Strg/Umschalt kept it).
    band: Option<(gtk4::Box, BTreeSet<usize>)>,
}

impl Drag {
    fn started(&self) -> bool {
        !self.moving.is_empty() || self.band.is_some()
    }
}

/// The label being edited (HMP-10).
struct Editing {
    key: String,
    entry: gtk4::Entry,
}

pub struct Icons {
    pub window: ApplicationWindow,
    fixed: Fixed,
    anchor: MenuButton,
    menu: gio::Menu,
    items: RefCell<Vec<Item>>,
    widgets: RefCell<Vec<gtk4::Box>>,
    /// Where each icon is shown (top left corner of its cell).
    positions: RefCell<Vec<Point>>,
    /// The icon the last click was on (menu actions, F2, Umschalt+Klick
    /// counts from here); always one of `selection` or `None`.
    selected: Cell<Option<usize>>,
    /// HMP-10a: every selected icon.
    selection: RefCell<BTreeSet<usize>>,
    /// A plain press on an icon of a larger selection keeps the selection
    /// for a drag of all; without a drag its release selects that icon only.
    collapse_on_release: Cell<Option<usize>>,
    geometry: Cell<Geometry>,
    store: RefCell<Store>,
    /// HMP-09's saved order, for the first start without a state file.
    migrated_order: Vec<String>,
    /// HMP-10a: the HMP-10 state file was copied once before the first write.
    backed_up: Cell<bool>,
    watches: RefCell<Vec<gio::FileMonitor>>,
    /// Applies a new desktop.ini at once (main.rs `load`).
    background_changed: Rc<dyn Fn()>,
    drag: RefCell<Option<Drag>>,
    /// Counts drags; a double click must not span one.
    drags: Cell<u64>,
    drags_at_first_press: Cell<u64>,
    drags_at_press: Cell<u64>,
    /// This press only ended a rename: no selection, drag or open.
    press_swallowed: Cell<bool>,
    editing: RefCell<Option<Editing>>,
    /// A refresh that waits for the end of a drag or rename.
    reload_waiting: Cell<bool>,
    reload_timer: RefCell<Option<glib::SourceId>>,
    last_logged: RefCell<String>,
    last_selection: RefCell<String>,
    check_tiles: Cell<bool>,
}

fn desktop_dir() -> Option<PathBuf> {
    icons::desktop_dir(
        glib::user_special_dir(glib::UserDirectory::Desktop),
        &glib::home_dir(),
    )
}

/// `$XDG_STATE_HOME/querencia` (default `~/.local/state/querencia`).
fn state_dir() -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|dir| dir.is_absolute())
        .unwrap_or_else(|| glib::home_dir().join(".local/state"))
        .join("querencia")
}

/// HMP-09's order file; still written, so that a return to HMP-09 finds
/// the icons in about the same order.
fn order_path() -> PathBuf {
    state_dir().join("desktop-order")
}

fn store_path() -> PathBuf {
    state_dir().join(store::FILE_NAME)
}

fn log(text: &str) {
    eprintln!("querencia-desktop: {text}");
}

/// Writes `text` to `path` atomically: a temporary file in the same
/// directory, flushed to disk, then renamed over the old one.
fn write_atomically(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(".neu");
    let temporary = PathBuf::from(temporary);
    let mut file = std::fs::File::create(&temporary)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(&temporary, path)
}

impl Icons {
    pub fn create(
        app: &Application,
        monitor: &gdk::Monitor,
        background_changed: Rc<dyn Fn()>,
    ) -> Rc<Self> {
        let fixed = Fixed::new();
        let window = ApplicationWindow::builder()
            .application(app)
            .title("Arbeitsfläche – Symbole")
            .child(&fixed)
            .build();
        window.add_css_class("querencia-desktop-icons");
        window.init_layer_shell();
        window.set_namespace(Some("querencia-desktop-icons"));
        window.set_layer(Layer::Bottom);
        window.set_monitor(Some(monitor));
        window.set_keyboard_mode(KeyboardMode::OnDemand);
        window.set_exclusive_zone(0);
        for edge in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
            window.set_anchor(edge, true);
        }

        // The menus' anchor: an empty, untargetable menu button at the
        // origin; its menu is pointed at the click (see `popup_menu`).
        let anchor = MenuButton::new();
        anchor.set_child(Some(&gtk4::Box::new(Orientation::Horizontal, 0)));
        anchor.add_css_class("desktop-menu-anchor");
        anchor.set_can_target(false);
        // Stays focusable: an unfocusable menu button makes GTK report a
        // critical (gtk_widget_is_ancestor) each time its menu opens.
        anchor.set_focus_on_click(false);
        fixed.put(&anchor, 0.0, 0.0);
        let menu = gio::Menu::new();
        classic_menu::attach(&anchor, menu.upcast_ref());

        let (store, migrated_order) = load_store();
        let icons = Rc::new(Self {
            window,
            fixed,
            anchor,
            menu,
            items: RefCell::default(),
            widgets: RefCell::default(),
            positions: RefCell::default(),
            selected: Cell::new(None),
            selection: RefCell::default(),
            collapse_on_release: Cell::new(None),
            geometry: Cell::new(Geometry {
                width: 0,
                height: 0,
                tile: CELL,
                margin: MARGIN,
                // "Am Raster ausrichten": on unless switched off
                snap: store.snap.unwrap_or(true),
            }),
            store: RefCell::new(store),
            migrated_order,
            backed_up: Cell::new(false),
            watches: RefCell::default(),
            background_changed,
            drag: RefCell::default(),
            drags: Cell::new(0),
            drags_at_first_press: Cell::new(0),
            drags_at_press: Cell::new(0),
            press_swallowed: Cell::new(false),
            editing: RefCell::default(),
            reload_waiting: Cell::new(false),
            reload_timer: RefCell::default(),
            last_logged: RefCell::default(),
            last_selection: RefCell::default(),
            check_tiles: Cell::new(false),
        });
        icons.install_actions();
        icons.install_clicks();
        icons.install_drag();
        icons.install_keys();
        // The area between the panels: known after the first configure,
        // changes with the panels, the resolution and the scale factor.
        let weak = Rc::downgrade(&icons);
        icons.fixed.add_tick_callback(move |fixed, _| {
            if let Some(icons) = weak.upgrade() {
                let mut geometry = icons.geometry.get();
                let size = (fixed.width(), fixed.height());
                if size.0 > 0 && size.1 > 0 && size != (geometry.width, geometry.height) {
                    geometry.width = size.0;
                    geometry.height = size.1;
                    icons.geometry.set(geometry);
                    log(&format!("Symbolfläche {}x{}", size.0, size.1));
                    icons.layout();
                }
                if icons.check_tiles.get() && icons.log_oversized_tiles() {
                    icons.check_tiles.set(false);
                }
            }
            glib::ControlFlow::Continue
        });
        icons.watch();
        icons.reload();
        icons.window.present();
        icons
    }

    /// Re-reads which icons there are; places come from the state file.
    pub fn reload(self: &Rc<Self>) {
        if self.drag.borrow().is_some() || self.editing.borrow().is_some() {
            self.reload_waiting.set(true);
            return;
        }
        self.reload_waiting.set(false);
        let (mut items, listed) = load_items();
        {
            let mut store = self.store.borrow_mut();
            for item in &mut items {
                if let Some(alias) = store.alias(&item.key) {
                    item.name = alias.to_owned();
                }
            }
            // Gone files lose their place (a new file of the same name later
            // gets a free one), but only when the directory could be read.
            if listed {
                let present: Vec<String> = items.iter().map(Item::ident).collect();
                if store.forget_missing(|name| present.iter().any(|p| p == name)) {
                    drop(store);
                    self.save();
                }
            }
        }
        let mut saved: Vec<String> = self
            .store
            .borrow()
            .entries
            .iter()
            .map(|e| e.key())
            .collect();
        saved.extend(self.migrated_order.iter().cloned());
        let items = icons::ordered(&saved, items);
        self.set_items(items);
    }

    /// A refresh shortly after file monitor events (they come in bursts).
    fn reload_soon(self: &Rc<Self>) {
        if let Some(id) = self.reload_timer.take() {
            id.remove();
        }
        let weak = Rc::downgrade(self);
        let id = glib::timeout_add_local_once(Duration::from_millis(120), move || {
            if let Some(icons) = weak.upgrade() {
                icons.reload_timer.take();
                icons.reload();
            }
        });
        *self.reload_timer.borrow_mut() = Some(id);
    }

    fn set_items(self: &Rc<Self>, items: Vec<Item>) {
        let selected_key = self
            .selected
            .get()
            .and_then(|i| self.items.borrow().get(i).map(|item| item.key.clone()));
        let selection_keys: Vec<String> = self
            .selection
            .borrow()
            .iter()
            .filter_map(|&i| self.items.borrow().get(i).map(|item| item.key.clone()))
            .collect();
        *self.selection.borrow_mut() = items
            .iter()
            .enumerate()
            .filter(|(_, item)| selection_keys.contains(&item.key))
            .map(|(i, _)| i)
            .collect();
        self.collapse_on_release.set(None);
        for widget in self.widgets.borrow_mut().drain(..) {
            self.fixed.remove(&widget);
        }
        let widgets: Vec<gtk4::Box> = items.iter().map(icon_widget).collect();
        for widget in &widgets {
            // shown once placed (`layout`), never first at the top left
            widget.set_visible(false);
            self.fixed.put(widget, 0.0, 0.0);
        }
        self.selected.set(
            selected_key
                .and_then(|key| items.iter().position(|item| item.key == key))
                .filter(|i| self.selection.borrow().contains(i)),
        );
        log(&format!(
            "{} Symbole: {}",
            items.len(),
            items
                .iter()
                .map(|i| i.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
        *self.items.borrow_mut() = items;
        *self.widgets.borrow_mut() = widgets;
        self.layout();
        self.show_selection();
    }

    /// Shows every icon at its place; icons without one get a free spot,
    /// which is remembered. A smaller area moves icons in for display only.
    fn layout(&self) {
        let geometry = self.geometry.get();
        if !geometry.known() {
            return;
        }
        let items = self.items.borrow();
        // Priority: the order of the state file (the last moved icon is
        // first), then the icons without a place.
        let mut order: Vec<usize> = (0..items.len()).collect();
        {
            let store = self.store.borrow();
            let rank = |i: &usize| {
                store
                    .entries
                    .iter()
                    .position(|e| e.key() == items[*i].key)
                    .unwrap_or(usize::MAX)
            };
            order.sort_by_key(|i| (rank(i), *i));
        }
        let wanted: Vec<_> = order
            .iter()
            .map(|&i| self.store.borrow().place(&items[i].key))
            .collect();
        let placed = geometry.place_all(&wanted);
        let mut positions = vec![(0, 0); items.len()];
        let mut new_places = false;
        for (n, &i) in order.iter().enumerate() {
            positions[i] = placed[n];
            if wanted[n].is_none() {
                self.store.borrow_mut().set_place(
                    items[i].art(),
                    &items[i].ident(),
                    geometry.top_left_place(placed[n]),
                );
                new_places = true;
            }
        }
        for (widget, &(x, y)) in self.widgets.borrow().iter().zip(&positions) {
            self.fixed.move_(widget, f64::from(x), f64::from(y));
            widget.set_visible(true);
        }
        let text: Vec<String> = items
            .iter()
            .zip(&positions)
            .map(|(item, (x, y))| format!("{}={x},{y}", item.name))
            .collect();
        let text = text.join("; ");
        if *self.last_logged.borrow() != text {
            log(&format!("Lage: {text}"));
            *self.last_logged.borrow_mut() = text;
        }
        drop(items);
        *self.positions.borrow_mut() = positions;
        // HMP-10a: after the next allocation, check that no tile (icon and
        // label, however long) is larger than its cell
        self.check_tiles.set(true);
        if new_places {
            self.save();
        }
    }

    /// Writes the state file (and HMP-09's order file).
    fn save(&self) {
        let store = self.store.borrow();
        if let Some(version) = store.newer {
            log(&format!(
                "{} ist von Version {version}, wird nicht überschrieben",
                store_path().display()
            ));
            return;
        }
        let text = store.render();
        let path = store_path();
        // HMP-10a: before its first write, a complete copy of the file as
        // it was (written by HMP-10), once (`store::back_up`: temporary
        // files, checked, published without overwriting, completed by its
        // checksum mark). Without it nothing is written
        // (the places stay in memory and are saved at the next write once
        // the copy succeeds).
        if !self.backed_up.get() {
            let copy = state_dir().join(format!("{}.vor-hmp10a", store::FILE_NAME));
            match store::back_up(&path, &copy) {
                Ok(done) => {
                    if let store::Backup::Made { set_aside } = done {
                        for aside in set_aside {
                            log(&format!(
                                "unvollständige Sicherung beiseitegelegt: {}",
                                aside.display()
                            ));
                        }
                        log(&format!("Sicherung der Anordnung: {}", copy.display()));
                    }
                    self.backed_up.set(true);
                }
                Err(err) => {
                    log(&format!(
                        "Sicherung fehlgeschlagen ({err}): Anordnung nicht gespeichert"
                    ));
                    return;
                }
            }
        }
        if std::fs::read_to_string(&path).ok().as_deref() != Some(text.as_str())
            && let Err(err) = write_atomically(&path, &text)
        {
            log(&format!("Anordnung nicht gespeichert: {err}"));
        }
        // HMP-09 order: by column, then row.
        let items = self.items.borrow();
        let positions = self.positions.borrow();
        let mut by_place: Vec<(Point, String)> = items
            .iter()
            .zip(positions.iter())
            .filter_map(|(item, p)| store::hmp09_order_key(&item.key).map(|k| (*p, k)))
            .collect();
        by_place.sort();
        let order: String = by_place
            .into_iter()
            .map(|(_, k)| format!("{k}\n"))
            .collect();
        if !order.is_empty()
            && std::fs::read_to_string(order_path()).ok().as_deref() != Some(order.as_str())
            && let Err(err) = write_atomically(&order_path(), &order)
        {
            log(&format!("Reihenfolge nicht gespeichert: {err}"));
        }
    }

    /// Logs tiles whose allocation is larger than a cell (a label that
    /// would reach into the next cell); the isolated test looks for these.
    /// `false` while a tile has no size yet (not drawn): check again at the
    /// next frame.
    fn log_oversized_tiles(&self) -> bool {
        let items = self.items.borrow();
        let widgets = self.widgets.borrow();
        if widgets.iter().any(|w| w.is_visible() && w.width() == 0) {
            return false;
        }
        let lines = debugging();
        for (item, widget) in items.iter().zip(widgets.iter()) {
            // the whole tile as drawn (CSS padding included), not only its
            // content (`width()`)
            let Some(tile) = widget.compute_bounds(&self.fixed) else {
                continue;
            };
            let (w, h) = (tile.width().round() as i32, tile.height().round() as i32);
            if lines && let Some(label) = widget.last_child().and_downcast::<Label>() {
                let span = |child: Option<gtk4::Widget>| {
                    child
                        .and_then(|c| c.compute_bounds(widget))
                        .map_or((0, 0), |b| (b.x().round() as i32, b.width().round() as i32))
                };
                let (x, lw) = span(Some(label.clone().upcast()));
                let (ix, iw) = span(widget.first_child());
                log(&format!(
                    "Beschriftung: {} → {} [Label x {x} Breite {lw}, Kachel {w}x{h}, Symbol x {ix} Breite {iw}]",
                    item.name,
                    label_lines(&label).join(" | "),
                ));
            }
            if w > CELL.0 || h > CELL.1 {
                log(&format!(
                    "Kachel größer als die Zelle: {} {w}x{h}",
                    item.name
                ));
            }
        }
        true
    }

    fn show_selection(&self) {
        let selection = self.selection.borrow();
        for (index, widget) in self.widgets.borrow().iter().enumerate() {
            if selection.contains(&index) {
                widget.add_css_class("selected");
            } else {
                widget.remove_css_class("selected");
            }
        }
        // the selection in the log when it changes (for the isolated tests)
        let names: Vec<String> = selection
            .iter()
            .filter_map(|&i| self.items.borrow().get(i).map(|item| item.name.clone()))
            .collect();
        let text = names.join("; ");
        if *self.last_selection.borrow() != text {
            log(&format!("Auswahl: {text}"));
            *self.last_selection.borrow_mut() = text;
        }
    }

    fn hit(&self, x: f64, y: f64) -> Option<usize> {
        self.geometry.get().hit(&self.positions.borrow(), (x, y))
    }

    /// Whether `(x, y)` is on the rename box.
    fn on_rename_box(&self, x: f64, y: f64) -> bool {
        self.editing.borrow().as_ref().is_some_and(|edit| {
            edit.entry
                .compute_bounds(&self.fixed)
                .is_some_and(|b| b.contains_point(&gtk4::graphene::Point::new(x as f32, y as f32)))
        })
    }

    /// Only `index` selected (or nothing); it is also the anchor for
    /// Umschalt+Klick.
    fn select_only(&self, index: Option<usize>) {
        *self.selection.borrow_mut() = index.into_iter().collect();
        self.selected.set(index);
        self.show_selection();
    }

    /// The selected icons, sorted.
    fn selection_list(&self) -> Vec<usize> {
        self.selection.borrow().iter().copied().collect()
    }

    fn install_clicks(self: &Rc<Self>) {
        let click = GestureClick::new();
        click.set_button(0);
        let weak = Rc::downgrade(self);
        click.connect_pressed(move |gesture, n_press, x, y| {
            let Some(icons) = weak.upgrade() else { return };
            if icons.on_rename_box(x, y) {
                return;
            }
            if icons.editing.borrow().is_some() {
                // A click elsewhere ends the rename (and does nothing else).
                // Only a left press has a drag gesture that clears the flag
                // again at its release.
                if gesture.current_button() == gdk::BUTTON_PRIMARY {
                    icons.press_swallowed.set(true);
                }
                icons.finish_rename(true);
                return;
            }
            let index = icons.hit(x, y);
            icons.collapse_on_release.set(None);
            icons.drags_at_press.set(icons.drags.get());
            if gesture.current_button() == gdk::BUTTON_SECONDARY {
                // on a selected icon the selection stays (GNOME 2)
                match index {
                    Some(i) if icons.selection.borrow().contains(&i) => {
                        icons.selected.set(Some(i));
                    }
                    other => icons.select_only(other),
                }
                icons.popup_menu(index, x, y);
                return;
            }
            if gesture.current_button() != gdk::BUTTON_PRIMARY {
                return;
            }
            if n_press == 1 {
                icons.drags_at_first_press.set(icons.drags.get());
            }
            let modifiers = gesture.current_event_state();
            let ctrl = modifiers.contains(gdk::ModifierType::CONTROL_MASK);
            let shift = modifiers.contains(gdk::ModifierType::SHIFT_MASK);
            match index {
                // the free desktop: a plain press clears the selection
                // (Strg/Umschalt keep it for the rubber band)
                None => {
                    if !ctrl && !shift {
                        icons.select_only(None);
                    }
                }
                // Strg+Klick: in or out of the selection
                Some(i) if ctrl => {
                    let now_in = {
                        let mut selection = icons.selection.borrow_mut();
                        if !selection.remove(&i) {
                            selection.insert(i);
                        }
                        selection.contains(&i)
                    };
                    icons.selected.set(if now_in {
                        Some(i)
                    } else {
                        icons.selection.borrow().iter().next().copied()
                    });
                    icons.show_selection();
                }
                // Umschalt+Klick: from the last clicked icon to this one
                Some(i) if shift => {
                    let from = icons.selected.get().unwrap_or(i);
                    let range = layout::range_between(&icons.positions.borrow(), from, i);
                    *icons.selection.borrow_mut() = range.into_iter().collect();
                    // the anchor stays where the range started
                    icons.selected.set(Some(from));
                    icons.show_selection();
                }
                Some(i) if icons.selection.borrow().contains(&i) => {
                    icons.selected.set(Some(i));
                    if icons.selection.borrow().len() > 1 {
                        icons.collapse_on_release.set(Some(i));
                    }
                }
                Some(i) => icons.select_only(Some(i)),
            }
        });
        let weak = Rc::downgrade(self);
        click.connect_released(move |gesture, n_press, x, y| {
            let Some(icons) = weak.upgrade() else { return };
            if gesture.current_button() != gdk::BUTTON_PRIMARY
                || icons.press_swallowed.get()
                || icons.editing.borrow().is_some()
            {
                return;
            }
            let index = icons.hit(x, y);
            // a click (no drag) on one icon of a larger selection selects it alone
            if let Some(i) = icons.collapse_on_release.take()
                && icons.drags.get() == icons.drags_at_press.get()
                && index == Some(i)
            {
                icons.select_only(Some(i));
            }
            // Open on the release of the second click, and only if neither
            // press became a drag.
            if n_press != 2 || icons.drags.get() != icons.drags_at_first_press.get() {
                return;
            }
            if index.is_some() && index == icons.selected.get() {
                let item = index.and_then(|i| icons.items.borrow().get(i).cloned());
                if let Some(item) = item {
                    open(&item, &icons.window);
                }
            }
        });
        self.fixed.add_controller(click);
    }

    fn install_drag(self: &Rc<Self>) {
        let drag = GestureDrag::new();
        drag.set_button(gdk::BUTTON_PRIMARY);
        let weak = Rc::downgrade(self);
        drag.connect_drag_begin(move |_, x, y| {
            let Some(icons) = weak.upgrade() else { return };
            if icons.editing.borrow().is_some() || icons.press_swallowed.get() {
                return;
            }
            let index = icons.hit(x, y);
            let grab = index.map_or((0.0, 0.0), |i| {
                let (px, py) = icons.positions.borrow()[i];
                (x - f64::from(px), y - f64::from(py))
            });
            *icons.drag.borrow_mut() = Some(Drag {
                index,
                start: (x, y),
                grab,
                moving: Vec::new(),
                band: None,
            });
        });
        let weak = Rc::downgrade(self);
        drag.connect_drag_update(move |_, dx, dy| {
            let Some(icons) = weak.upgrade() else { return };
            let mut state = icons.drag.borrow_mut();
            let Some(drag) = state.as_mut() else { return };
            if !drag.started() {
                if !layout::is_drag(dx, dy, drag_threshold(&icons.window)) {
                    return;
                }
                icons.drags.set(icons.drags.get() + 1);
                icons.collapse_on_release.set(None);
                match drag.index {
                    Some(i) => {
                        // the whole selection moves if the icon is part of
                        // it, else this icon alone
                        if !icons.selection.borrow().contains(&i) {
                            icons.select_only(Some(i));
                        }
                        let mut members = vec![i];
                        members.extend(icons.selection_list().into_iter().filter(|&m| m != i));
                        for m in members {
                            let Some(item) = icons.items.borrow().get(m).cloned() else {
                                continue;
                            };
                            let ghost = icon_widget(&item);
                            ghost.add_css_class("dragging");
                            ghost.add_css_class("selected");
                            ghost.set_can_target(false);
                            icons.fixed.put(&ghost, 0.0, 0.0);
                            drag.moving.push((m, ghost));
                        }
                    }
                    None => {
                        let frame = gtk4::Box::new(Orientation::Horizontal, 0);
                        frame.add_css_class("rubberband");
                        frame.set_can_target(false);
                        icons.fixed.put(&frame, drag.start.0, drag.start.1);
                        drag.band = Some((frame, icons.selection.borrow().clone()));
                    }
                }
            }
            let geometry = icons.geometry.get();
            let positions = icons.positions.borrow().clone();
            if let Some((lead, _)) = drag.moving.first() {
                let lead_at = positions[*lead];
                let want = drop_point(drag, dx, dy);
                let (mx, my) = (want.0 - lead_at.0, want.1 - lead_at.1);
                for (m, ghost) in &drag.moving {
                    let at = geometry.clamp((positions[*m].0 + mx, positions[*m].1 + my));
                    icons.fixed.move_(ghost, f64::from(at.0), f64::from(at.1));
                }
            }
            if let Some((frame, base)) = &drag.band {
                let corner = (drag.start.0 + dx, drag.start.1 + dy);
                let (x0, y0) = (drag.start.0.min(corner.0), drag.start.1.min(corner.1));
                frame.set_size_request(
                    (dx.abs().round() as i32).max(1),
                    (dy.abs().round() as i32).max(1),
                );
                icons.fixed.move_(frame, x0, y0);
                let inside = geometry.in_rectangle(&positions, drag.start, corner);
                let mut selection: BTreeSet<usize> = base.clone();
                selection.extend(inside);
                *icons.selection.borrow_mut() = selection;
                icons.show_selection();
            }
        });
        let weak = Rc::downgrade(self);
        drag.connect_drag_end(move |_, dx, dy| {
            let Some(icons) = weak.upgrade() else { return };
            icons.press_swallowed.set(false);
            let Some(drag) = icons.drag.borrow_mut().take() else {
                return;
            };
            for (_, ghost) in &drag.moving {
                icons.fixed.remove(ghost);
            }
            match drag.moving.len() {
                0 => {}
                1 => icons.drop_icon(drag.moving[0].0, drop_point(&drag, dx, dy)),
                _ => {
                    let members: Vec<usize> = drag.moving.iter().map(|(m, _)| *m).collect();
                    icons.drop_group(&members, drop_point(&drag, dx, dy));
                }
            }
            if let Some((frame, _)) = &drag.band {
                icons.fixed.remove(frame);
                let selection = icons.selection_list();
                if !selection.contains(&icons.selected.get().unwrap_or(usize::MAX)) {
                    icons.selected.set(selection.first().copied());
                }
                let names: Vec<String> = selection
                    .iter()
                    .filter_map(|&i| icons.items.borrow().get(i).map(|item| item.name.clone()))
                    .collect();
                log(&format!("Auswahl (Rahmen): {}", names.join(", ")));
            }
            if icons.reload_waiting.get() {
                icons.reload();
            }
        });
        self.fixed.add_controller(drag);
    }

    /// Puts icon `index` at `want` or, if that is taken, at the nearest
    /// free spot (with the grid: the nearest free cell); remembers it. The
    /// file is not touched.
    fn drop_icon(&self, index: usize, want: Point) {
        let geometry = self.geometry.get();
        let others: Vec<Point> = self
            .positions
            .borrow()
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != index)
            .map(|(_, p)| *p)
            .collect();
        let Some(item) = self.items.borrow().get(index).cloned() else {
            return;
        };
        // no free spot at all (more icons than room): the icon stays
        let Some(at) = geometry.nearest_free(want, &others) else {
            log(&format!(
                "Verschieben abgelehnt (kein freier Platz): {}",
                item.name
            ));
            self.layout();
            return;
        };
        if !self.remember_moved(&[(index, at)]) {
            log(&format!("{} bleibt an seinem Platz", item.name));
            self.layout();
            return;
        }
        log(&format!(
            "verschoben: {} nach {},{} (gewünscht {},{})",
            item.name, at.0, at.1, want.0, want.1
        ));
        self.layout();
        self.save();
    }

    /// HMP-10a: moves the selected icons `members` (the one under the
    /// pointer first) together, that one dropped at `want`: same
    /// arrangement, on the grid, inside the area, never onto other icons
    /// (`Geometry::move_group`). If the arrangement fits nowhere, the icons
    /// go one by one to the nearest free spots; if not every one finds
    /// one, the move is refused and all stay where they were.
    fn drop_group(&self, members: &[usize], want: Point) {
        let geometry = self.geometry.get();
        let positions = self.positions.borrow().clone();
        let group: Vec<Point> = members.iter().map(|&m| positions[m]).collect();
        let others: Vec<Point> = positions
            .iter()
            .enumerate()
            .filter(|(i, _)| !members.contains(i))
            .map(|(_, p)| *p)
            .collect();
        // the whole group or nothing: no icon onto another (HMP-10a review)
        let Some(moved) = geometry.move_group(&group, 0, want, &others) else {
            log(&format!(
                "Verschieben abgelehnt (kein Platz für {} Symbole): alle bleiben, wo sie sind",
                members.len()
            ));
            self.layout();
            return;
        };
        let pairs: Vec<(usize, Point)> = members.iter().copied().zip(moved).collect();
        if !self.remember_moved(&pairs) {
            log(&format!(
                "{} Symbole bleiben an ihren Plätzen",
                members.len()
            ));
            self.layout();
            return;
        }
        let text: Vec<String> = pairs
            .iter()
            .filter_map(|(m, (x, y))| {
                self.items
                    .borrow()
                    .get(*m)
                    .map(|item| format!("{}={x},{y}", item.name))
            })
            .collect();
        log(&format!("gemeinsam verschoben: {}", text.join("; ")));
        self.layout();
        self.save();
    }

    /// Stores the places the user chose; the moved icons keep their spots
    /// first (front of the state file, in this order).
    /// An icon that ends up where it already was is not moved: its
    /// remembered place stays as it is (a drop on a full or smaller desktop
    /// must not replace a place chosen for a larger screen). `false` when
    /// nothing moved.
    fn remember_moved(&self, moved: &[(usize, Point)]) -> bool {
        let geometry = self.geometry.get();
        let items = self.items.borrow();
        let shown = self.positions.borrow().clone();
        let mut store = self.store.borrow_mut();
        let mut any = false;
        for (n, &(index, at)) in moved.iter().enumerate() {
            let Some(item) = items.get(index) else {
                continue;
            };
            if shown.get(index) == Some(&at) {
                continue;
            }
            any = true;
            store.set_place(item.art(), &item.ident(), geometry.free_place(at));
            if let Some(i) = store.entries.iter().position(|e| e.key() == item.key) {
                let entry = store.entries.remove(i);
                store.entries.insert(n, entry);
            }
        }
        any
    }

    fn install_keys(self: &Rc<Self>) {
        let keys = EventControllerKey::new();
        let weak = Rc::downgrade(self);
        keys.connect_key_pressed(move |_, key, _, modifiers| {
            let Some(icons) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            if icons.editing.borrow().is_some() {
                return glib::Propagation::Proceed;
            }
            if key == gdk::Key::F2 {
                // renaming is for one icon at a time
                if icons.selection.borrow().len() == 1
                    && let Some(index) = icons.selected.get()
                {
                    icons.start_rename(index);
                }
                return glib::Propagation::Stop;
            }
            if modifiers.contains(gdk::ModifierType::CONTROL_MASK)
                && matches!(key, gdk::Key::a | gdk::Key::A)
            {
                let count = icons.items.borrow().len();
                *icons.selection.borrow_mut() = (0..count).collect();
                if icons.selected.get().is_none() {
                    icons.selected.set((count > 0).then_some(0));
                }
                icons.show_selection();
                log(&format!("alle {count} Symbole ausgewählt"));
                return glib::Propagation::Stop;
            }
            if key == gdk::Key::Escape && !icons.selection.borrow().is_empty() {
                icons.select_only(None);
                log("Auswahl aufgehoben");
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        self.window.add_controller(keys);
    }

    fn popup_menu(&self, index: Option<usize>, x: f64, y: f64) {
        self.menu.remove_all();
        if let Some(index) = index {
            let is_trash = self
                .items
                .borrow()
                .get(index)
                .is_some_and(|item| item.kind == Kind::Trash);
            if is_trash {
                // HMP-10b: GNOME 2's trash menu
                self.menu
                    .append(Some("Papierkorb öffnen"), Some("desktop.open"));
                let empty = gio::Menu::new();
                // insensitive while the trash is empty (or unreachable)
                let full = trash_count().unwrap_or(0) > 0;
                empty.append(
                    Some("Papierkorb leeren…"),
                    full.then_some("desktop.empty-trash"),
                );
                self.menu.append_section(None, &empty);
                if debugging() {
                    log(&format!(
                        "Menü Papierkorb: „Papierkorb leeren…“ {}",
                        if full { "an" } else { "aus" }
                    ));
                }
            } else {
                self.menu.append(Some("Öffnen"), Some("desktop.open"));
            }
            let edit = gio::Menu::new();
            // one icon at a time (HMP-10a: not with several selected)
            let can_rename = self.selection.borrow().len() <= 1
                && self
                    .items
                    .borrow()
                    .get(index)
                    .is_some_and(|item| renamable(item).is_ok());
            // insensitive (no action) where renaming cannot work
            edit.append(Some("Umbenennen …"), can_rename.then_some("desktop.rename"));
            self.menu.append_section(None, &edit);
        } else {
            let can_create = desktop_dir().is_some();
            let create = gio::Menu::new();
            if can_create {
                create.append(Some("Ordner anlegen"), Some("desktop.new-folder"));
                let documents = gio::Menu::new();
                documents.append_item(&gio::MenuItem::new(
                    Some("Leere Datei"),
                    Some("desktop.new-document::"),
                ));
                for template in icons::templates(
                    glib::user_special_dir(glib::UserDirectory::Templates).as_deref(),
                    &glib::home_dir(),
                ) {
                    let label = template
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let item = gio::MenuItem::new(Some(&label), None);
                    item.set_action_and_target_value(
                        Some("desktop.new-document"),
                        Some(&template.to_string_lossy().to_variant()),
                    );
                    documents.append_item(&item);
                }
                create.append_submenu(Some("Dokument anlegen"), &documents);
            }
            if create.n_items() > 0 {
                self.menu.append_section(None, &create);
            }
            let arrange = gio::Menu::new();
            arrange.append(
                Some("Nach Namen aufräumen"),
                Some("desktop.arrange-by-name"),
            );
            // HMP-10a: put every icon on the grid once; switch the grid on
            // or off (a check mark in the icon column, as GNOME 2 drew it)
            arrange.append(
                Some("Symbole am Raster ausrichten"),
                Some("desktop.align-to-grid"),
            );
            let toggle =
                gio::MenuItem::new(Some("Am Raster ausrichten"), Some("desktop.toggle-grid"));
            if self.geometry.get().snap {
                toggle.set_icon(&gio::ThemedIcon::from_names(&[
                    "object-select-symbolic",
                    "emblem-ok",
                    "emblem-OK",
                ]));
            }
            arrange.append_item(&toggle);
            self.menu.append_section(None, &arrange);
            let background = gio::Menu::new();
            background.append(
                Some("Hintergrund der Arbeitsfläche ändern …"),
                Some("desktop.change-background"),
            );
            self.menu.append_section(None, &background);
        }
        // Top left corner of the menu at the pointer, as on GNOME 2. The
        // anchor stays at the surface origin and the popover points at the
        // click: moving the anchor first would only take effect at the next
        // layout, after the popover was already placed at the old spot.
        if let Some(popover) = self.anchor.popover() {
            popover.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
        }
        self.anchor.popup();
    }

    fn install_actions(self: &Rc<Self>) {
        let group = gio::SimpleActionGroup::new();
        let add = |name: &str, parameter: Option<&glib::VariantTy>, run: ActionFn| {
            let action = gio::SimpleAction::new(name, parameter);
            let weak = Rc::downgrade(self);
            action.connect_activate(move |_, value| {
                if let Some(icons) = weak.upgrade() {
                    run(&icons, value);
                }
            });
            group.add_action(&action);
        };
        add(
            "open",
            None,
            Box::new(|icons, _| {
                if let Some(item) = icons
                    .selected
                    .get()
                    .and_then(|i| icons.items.borrow().get(i).cloned())
                {
                    open(&item, &icons.window);
                }
            }),
        );
        add(
            "empty-trash",
            None,
            Box::new(|icons, _| icons.confirm_empty_trash()),
        );
        add(
            "rename",
            None,
            Box::new(|icons, _| {
                // after the menu has closed and given back the keyboard
                let weak = Rc::downgrade(icons);
                glib::idle_add_local_once(move || {
                    if let Some(icons) = weak.upgrade()
                        && let Some(index) = icons.selected.get()
                    {
                        icons.start_rename(index);
                    }
                });
            }),
        );
        add(
            "new-folder",
            None,
            Box::new(|icons, _| {
                if let Some(dir) = icons.ensure_desktop_dir() {
                    let name = icons::unique_name("Neuer Ordner", |n| dir.join(n).exists());
                    match gio::File::for_path(dir.join(&name))
                        .make_directory(gio::Cancellable::NONE)
                    {
                        Ok(()) => log(&format!("Ordner angelegt: {name}")),
                        Err(err) => log(&format!("Ordner {name} nicht angelegt: {err}")),
                    }
                    icons.reload();
                }
            }),
        );
        add(
            "new-document",
            Some(glib::VariantTy::STRING),
            Box::new(|icons, value| {
                let template = value.and_then(|v| v.str()).unwrap_or_default().to_owned();
                if let Some(dir) = icons.ensure_desktop_dir() {
                    new_document(&dir, &template);
                    icons.reload();
                }
            }),
        );
        add(
            "arrange-by-name",
            None,
            Box::new(|icons, _| icons.arrange_by_name()),
        );
        add(
            "align-to-grid",
            None,
            Box::new(|icons, _| icons.align_to_grid()),
        );
        add(
            "toggle-grid",
            None,
            Box::new(|icons, _| icons.toggle_grid()),
        );
        add(
            "change-background",
            None,
            Box::new(|icons, _| change_background(icons)),
        );
        self.window.insert_action_group("desktop", Some(&group));
    }

    /// "Nach Namen aufräumen": every icon gets a new place in grid order
    /// (the fixed icons first, then by name); no file is touched.
    fn arrange_by_name(self: &Rc<Self>) {
        let geometry = self.geometry.get();
        if !geometry.known() {
            return;
        }
        let sorted = icons::by_name(self.items.borrow().clone());
        let positions = geometry.arranged(sorted.len());
        {
            let mut store = self.store.borrow_mut();
            let keys: Vec<&str> = sorted.iter().map(|i| i.key.as_str()).collect();
            store.entries.retain(|e| !keys.contains(&e.key().as_str()));
            for (n, (item, at)) in sorted.iter().zip(&positions).enumerate() {
                store.set_place(item.art(), &item.ident(), geometry.top_left_place(*at));
                let entry = store.entries.pop().expect("eben eingefügt");
                store.entries.insert(n, entry);
            }
        }
        log("nach Namen aufgeräumt");
        self.set_items(sorted);
        self.save();
    }

    /// HMP-10b "Papierkorb leeren…": asks first (GNOME 2's question,
    /// "Abbrechen" is the default and Escape cancels); only "Papierkorb
    /// leeren" empties it.
    fn confirm_empty_trash(self: &Rc<Self>) {
        if trash_count().unwrap_or(0) == 0 {
            log("Papierkorb ist leer: nichts zu tun");
            return;
        }
        let dialog = gtk4::AlertDialog::builder()
            .message("Möchten Sie alle Dateien im Papierkorb endgültig löschen?")
            .detail(
                "Wenn Sie den Papierkorb leeren, werden alle darin befindlichen \
                 Objekte unwiderruflich gelöscht.",
            )
            .buttons(["Abbrechen", "Papierkorb leeren"])
            .cancel_button(0)
            .default_button(0)
            .modal(false)
            .build();
        let weak = Rc::downgrade(self);
        dialog.choose(
            None::<&gtk4::Window>,
            gio::Cancellable::NONE,
            move |answer| match answer {
                Ok(1) => {
                    if let Some(icons) = weak.upgrade() {
                        icons.empty_trash();
                    }
                }
                _ => log("Papierkorb leeren abgebrochen"),
            },
        );
    }

    /// Empties the trash through GVfs (`trash:///`, every trash location
    /// it knows): each top-level object is deleted with `g_file_delete`.
    /// GVfs takes it out of the trash and removes it there, never
    /// following links out of the trash; nothing here deletes paths itself.
    /// One object after the other, asynchronously; afterwards the icon is
    /// refreshed, and every object that could not be deleted is reported.
    fn empty_trash(self: &Rc<Self>) {
        let trash = gio::File::for_uri("trash:///");
        let listed = trash
            .enumerate_children(
                "standard::name,standard::display-name",
                gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
                gio::Cancellable::NONE,
            )
            .and_then(|objects| {
                let mut found = std::collections::VecDeque::new();
                while let Some(info) = objects.next_file(gio::Cancellable::NONE)? {
                    found.push_back((objects.child(&info), info.display_name().to_string()));
                }
                Ok(found)
            });
        let objects = match listed {
            Ok(objects) => objects,
            Err(err) => {
                log(&format!("Papierkorb nicht erreichbar: {err}"));
                show_error(
                    "Der Papierkorb konnte nicht geleert werden.",
                    &format!("Der Papierkorb ist nicht erreichbar: {}", err.message()),
                );
                return;
            }
        };
        log(&format!("Papierkorb leeren: {} Objekte", objects.len()));
        let run = Rc::new(RefCell::new(TrashRun {
            total: objects.len(),
            objects,
            failed: Vec::new(),
        }));
        delete_next(Rc::downgrade(self), run);
    }

    /// HMP-10a "Symbole am Raster ausrichten": every icon is put on the
    /// nearest free grid cell (in the order of the state file, as shown),
    /// and that is remembered; each keeps the edge it was measured from.
    /// No file is touched.
    fn align_to_grid(self: &Rc<Self>) {
        let mut geometry = self.geometry.get();
        if !geometry.known() {
            return;
        }
        geometry.snap = true;
        let items = self.items.borrow().clone();
        let mut order: Vec<usize> = (0..items.len()).collect();
        {
            let store = self.store.borrow();
            let rank = |i: &usize| {
                store
                    .entries
                    .iter()
                    .position(|e| e.key() == items[*i].key)
                    .unwrap_or(usize::MAX)
            };
            order.sort_by_key(|i| (rank(i), *i));
        }
        let wanted: Vec<_> = order
            .iter()
            .map(|&i| self.store.borrow().place(&items[i].key))
            .collect();
        let placed = geometry.place_all(&wanted);
        {
            let mut store = self.store.borrow_mut();
            for (n, &i) in order.iter().enumerate() {
                let place = match wanted[n] {
                    Some(old) => geometry.place_like(old, placed[n]),
                    None => geometry.top_left_place(placed[n]),
                };
                store.set_place(items[i].art(), &items[i].ident(), place);
            }
        }
        log("Symbole am Raster ausgerichtet");
        self.layout();
        self.save();
    }

    /// HMP-10a "Am Raster ausrichten" on or off (remembered). Off shows
    /// the remembered pixel places again (HMP-10); nothing is lost.
    fn toggle_grid(self: &Rc<Self>) {
        let mut geometry = self.geometry.get();
        geometry.snap = !geometry.snap;
        self.geometry.set(geometry);
        self.store.borrow_mut().snap = Some(geometry.snap);
        log(&format!(
            "Am Raster ausrichten: {}",
            if geometry.snap { "an" } else { "aus" }
        ));
        self.layout();
        self.save();
    }

    /// The desktop directory, created on this first user action if missing.
    fn ensure_desktop_dir(self: &Rc<Self>) -> Option<PathBuf> {
        let dir = desktop_dir()?;
        if !dir.is_dir() {
            if let Err(err) = std::fs::create_dir_all(&dir) {
                log(&format!(
                    "Arbeitsflächen-Ordner {} nicht angelegt: {err}",
                    dir.display()
                ));
                return None;
            }
            log(&format!("Arbeitsflächen-Ordner {} angelegt", dir.display()));
            self.watch();
        }
        Some(dir)
    }

    /// Follows the desktop directory and the trash.
    fn watch(self: &Rc<Self>) {
        let mut watches = Vec::new();
        let mut targets = vec![gio::File::for_uri("trash:///")];
        if let Some(dir) = desktop_dir().filter(|d| d.is_dir()) {
            targets.push(gio::File::for_path(dir));
        }
        for target in targets {
            // WATCH_MOVES: a rename inside the folder comes as one event with
            // the old and the new name (HMP-10), not as delete and create.
            match target
                .monitor_directory(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE)
            {
                Ok(monitor) => {
                    let weak = Rc::downgrade(self);
                    monitor.connect_changed(move |_, file, other, event| {
                        let Some(icons) = weak.upgrade() else { return };
                        // renamed elsewhere (a file manager): keep the place
                        if event == gio::FileMonitorEvent::Renamed
                            && let (Some(old), Some(new)) =
                                (file.basename(), other.and_then(|o| o.basename()))
                            && file.parent().is_some_and(|dir| {
                                Some(dir.uri())
                                    == desktop_dir().map(|d| gio::File::for_path(d).uri())
                            })
                        {
                            let (old, new) = (old.to_string_lossy(), new.to_string_lossy());
                            if icons
                                .store
                                .borrow()
                                .place(&store::key(store::Art::File, &old))
                                .is_some()
                            {
                                icons.store.borrow_mut().rename(&old, &new);
                                icons.save();
                                log(&format!("umbenannt (außerhalb): {old} → {new}"));
                            }
                        }
                        // ChangesDoneHint: a starter edited elsewhere
                        if matches!(
                            event,
                            gio::FileMonitorEvent::Created
                                | gio::FileMonitorEvent::Deleted
                                | gio::FileMonitorEvent::MovedIn
                                | gio::FileMonitorEvent::MovedOut
                                | gio::FileMonitorEvent::Renamed
                                | gio::FileMonitorEvent::ChangesDoneHint
                        ) {
                            icons.reload_soon();
                        }
                    });
                    watches.push(monitor);
                }
                Err(err) => log(&format!("keine Überwachung von {}: {err}", target.uri())),
            }
        }
        *self.watches.borrow_mut() = watches;
    }

    /// Edits the label of icon `index` in place (F2, "Umbenennen …").
    fn start_rename(self: &Rc<Self>, index: usize) {
        if self.editing.borrow().is_some() || self.drag.borrow().is_some() {
            return;
        }
        let Some(item) = self.items.borrow().get(index).cloned() else {
            return;
        };
        if let Err(reason) = renamable(&item) {
            log(&format!(
                "Umbenennen nicht möglich: {}: {reason}",
                item.name
            ));
            show_error(
                &format!("„{}“ kann nicht umbenannt werden.", item.name),
                &reason,
            );
            return;
        }
        let Some(&(x, y)) = self.positions.borrow().get(index) else {
            return;
        };
        let entry = gtk4::Entry::new();
        entry.add_css_class("desktop-rename");
        entry.set_text(&item.name);
        entry.set_size_request(RENAME_WIDTH, -1);
        EditableExt::set_alignment(&entry, 0.5);
        let geometry = self.geometry.get();
        let left =
            (x + (CELL.0 - RENAME_WIDTH) / 2).clamp(0, (geometry.width - RENAME_WIDTH).max(0));
        self.fixed
            .put(&entry, f64::from(left), f64::from(y + ICON_PX + 3));
        if let Some(label) = self.label_of(index) {
            label.set_opacity(0.0);
        }
        let weak = Rc::downgrade(self);
        entry.connect_activate(move |_| {
            if let Some(icons) = weak.upgrade() {
                icons.finish_rename(true);
            }
        });
        let escape = EventControllerKey::new();
        escape.set_propagation_phase(PropagationPhase::Capture);
        let weak = Rc::downgrade(self);
        escape.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Escape
                && let Some(icons) = weak.upgrade()
            {
                icons.finish_rename(false);
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        entry.add_controller(escape);
        // Leaving the box (another window took the keyboard) keeps the new
        // name, as in Nautilus; later, outside the focus change.
        let focus = EventControllerFocus::new();
        let weak = Rc::downgrade(self);
        focus.connect_leave(move |_| {
            let weak = weak.clone();
            glib::idle_add_local_once(move || {
                if let Some(icons) = weak.upgrade() {
                    icons.finish_rename(true);
                }
            });
        });
        entry.add_controller(focus);
        *self.editing.borrow_mut() = Some(Editing {
            key: item.key.clone(),
            entry: entry.clone(),
        });
        entry.grab_focus();
        let selected = rename::selected_chars(&item.name, matches!(item.kind, Kind::File(_)));
        entry.select_region(0, i32::try_from(selected).unwrap_or(-1));
        log(&format!("Umbenennen: {}", item.name));
    }

    fn label_of(&self, index: usize) -> Option<Label> {
        self.widgets
            .borrow()
            .get(index)
            .and_then(|tile| tile.last_child())
            .and_downcast::<Label>()
    }

    /// Ends the rename: `keep` applies the new name, else nothing changes.
    fn finish_rename(self: &Rc<Self>, keep: bool) {
        let Some(edit) = self.editing.borrow_mut().take() else {
            return;
        };
        let new = edit.entry.text().to_string();
        self.fixed.remove(&edit.entry);
        let item = self
            .items
            .borrow()
            .iter()
            .find(|i| i.key == edit.key)
            .cloned();
        if let Some(index) = self.items.borrow().iter().position(|i| i.key == edit.key)
            && let Some(label) = self.label_of(index)
        {
            label.set_opacity(1.0);
        }
        match (keep, item) {
            (false, Some(item)) => log(&format!("Umbenennen abgebrochen: {}", item.name)),
            (true, Some(item)) => {
                if let Err((title, detail)) = self.apply_rename(&item, &new) {
                    log(&format!(
                        "Umbenennen fehlgeschlagen: {} → {new}: {detail}",
                        item.name
                    ));
                    show_error(&title, &detail);
                }
            }
            (_, None) => {}
        }
        self.reload();
    }

    /// Gives `item` the name `new`. Err: (title, explanation) for the user.
    fn apply_rename(&self, item: &Item, new: &str) -> Result<(), (String, String)> {
        let failed = |detail: String| {
            (
                format!("„{}“ konnte nicht in „{new}“ umbenannt werden.", item.name),
                detail,
            )
        };
        if new == item.name {
            return Ok(());
        }
        match &item.kind {
            Kind::File(path) | Kind::Folder(path) => {
                rename::check_file_name(new).map_err(|p| failed(p.text(new)))?;
                let old = item.ident();
                let gone = || {
                    format!(
                        "„{old}“ gibt es nicht mehr. Vielleicht wurde es inzwischen gelöscht oder verschoben."
                    )
                };
                if path.symlink_metadata().is_err() {
                    return Err(failed(gone()));
                }
                let exists = || {
                    format!(
                        "Der Name „{new}“ wird in diesem Ordner bereits verwendet. \
                         Bitte verwenden Sie einen anderen Namen."
                    )
                };
                if path.with_file_name(new).symlink_metadata().is_ok() {
                    return Err(failed(exists()));
                }
                // GIO refuses an existing name (G_IO_ERROR_EXISTS) itself.
                let renamed = gio::File::for_path(path)
                    .set_display_name(new, gio::Cancellable::NONE)
                    .map_err(|err| {
                        failed(match err.kind::<gio::IOErrorEnum>() {
                            Some(gio::IOErrorEnum::Exists) => exists(),
                            Some(gio::IOErrorEnum::NotFound) => gone(),
                            Some(gio::IOErrorEnum::PermissionDenied) => {
                                format!("Sie haben keine Berechtigung, „{old}“ umzubenennen.")
                            }
                            Some(gio::IOErrorEnum::InvalidFilename) => {
                                rename::Problem::Dots.text(new)
                            }
                            _ => err.message().to_owned(),
                        })
                    })?;
                let new_name = renamed
                    .basename()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| new.to_owned());
                // the place goes with the name, before the file monitor
                // reports the "new" file
                self.store.borrow_mut().rename(&old, &new_name);
                self.save();
                log(&format!("umbenannt: {old} → {new_name}"));
                Ok(())
            }
            Kind::Launcher(path) => {
                rename::check_label(new).map_err(|p| failed(p.text(new)))?;
                if own_launcher(path) {
                    set_launcher_name(path, new).map_err(failed)?;
                    self.store
                        .borrow_mut()
                        .set_alias(item.art(), &item.ident(), None);
                    self.save();
                    log(&format!(
                        "Starter {} heißt jetzt {new}",
                        path.file_name().unwrap_or_default().to_string_lossy()
                    ));
                } else {
                    self.set_alias(item, new);
                }
                Ok(())
            }
            Kind::Computer | Kind::Home | Kind::Trash | Kind::App(_) => {
                rename::check_label(new).map_err(|p| failed(p.text(new)))?;
                self.set_alias(item, new);
                Ok(())
            }
        }
    }

    /// The user's own name for an icon whose name is not its own to change;
    /// the original name removes it again.
    fn set_alias(&self, item: &Item, new: &str) {
        let label = (new != item.default_name).then_some(new);
        self.store
            .borrow_mut()
            .set_alias(item.art(), &item.ident(), label);
        self.save();
        log(&format!(
            "eigener Name für {}: {}",
            item.key,
            label.unwrap_or("(keiner)")
        ));
    }
}

/// `QUERENCIA_DESKTOP_DEBUG=1`: extra log lines for the isolated tests.
fn debugging() -> bool {
    std::env::var("QUERENCIA_DESKTOP_DEBUG").is_ok_and(|v| v == "1")
}

/// Objects in the trash (GVfs `trash:///`); `None` when it is not
/// reachable.
fn trash_count() -> Option<u32> {
    gio::File::for_uri("trash:///")
        .query_info(
            "trash::item-count",
            gio::FileQueryInfoFlags::NONE,
            gio::Cancellable::NONE,
        )
        .ok()
        .map(|info| info.attribute_uint32("trash::item-count"))
}

/// One "Papierkorb leeren": what is left to delete and what failed.
struct TrashRun {
    total: usize,
    objects: std::collections::VecDeque<(gio::File, String)>,
    failed: Vec<(String, String)>,
}

/// Deletes the next object of `run`, then the one after it; at the end
/// refreshes the icons and reports failures (never a success that was not
/// one).
fn delete_next(icons: std::rc::Weak<Icons>, run: Rc<RefCell<TrashRun>>) {
    let next = run.borrow_mut().objects.pop_front();
    let Some((file, name)) = next else {
        let run = run.borrow();
        let deleted = run.total - run.failed.len();
        log(&format!(
            "Papierkorb geleert: {deleted} von {} gelöscht, {} nicht",
            run.total,
            run.failed.len()
        ));
        if let Some(icons) = icons.upgrade() {
            icons.reload();
        }
        if !run.failed.is_empty() {
            let (verb, rest) = if run.failed.len() == 1 {
                ("konnte", "ist")
            } else {
                ("konnten", "sind")
            };
            let mut detail = format!(
                "{} von {} Objekten {verb} nicht gelöscht werden und {rest} noch im Papierkorb:",
                run.failed.len(),
                run.total
            );
            for (name, why) in run.failed.iter().take(5) {
                detail.push_str(&format!("\n• {name}: {why}"));
            }
            if run.failed.len() > 5 {
                detail.push_str(&format!("\n… und {} weitere", run.failed.len() - 5));
            }
            show_error(
                "Der Papierkorb konnte nicht vollständig geleert werden.",
                &detail,
            );
        }
        return;
    };
    let file2 = file.clone();
    file.delete_async(
        glib::Priority::DEFAULT,
        gio::Cancellable::NONE,
        move |result| {
            if let Err(err) = result {
                log(&format!("nicht gelöscht: {name} ({}): {err}", file2.uri()));
                // in German first; GVfs' own text (often English) as detail
                let why = match err.kind::<gio::IOErrorEnum>() {
                    Some(gio::IOErrorEnum::PermissionDenied) => "keine Berechtigung".to_owned(),
                    Some(gio::IOErrorEnum::NotFound) => "nicht mehr vorhanden".to_owned(),
                    Some(
                        gio::IOErrorEnum::NotMounted
                        | gio::IOErrorEnum::HostNotFound
                        | gio::IOErrorEnum::HostUnreachable
                        | gio::IOErrorEnum::TimedOut,
                    ) => "Datenträger nicht erreichbar".to_owned(),
                    _ => format!("nicht löschbar (GVfs: {})", err.message()),
                };
                run.borrow_mut().failed.push((name, why));
            }
            delete_next(icons, run);
        },
    );
}

/// The drop position of a drag after a movement of `(dx, dy)`.
fn drop_point(drag: &Drag, dx: f64, dy: f64) -> Point {
    (
        (drag.start.0 + dx - drag.grab.0).round() as i32,
        (drag.start.1 + dy - drag.grab.1).round() as i32,
    )
}

fn drag_threshold(window: &ApplicationWindow) -> i32 {
    gtk4::Settings::for_display(&WidgetExt::display(window)).gtk_dnd_drag_threshold()
}

/// The state file, or (first start after HMP-09) HMP-09's order.
fn load_store() -> (Store, Vec<String>) {
    let path = store_path();
    match std::fs::read_to_string(&path) {
        Ok(text) => {
            let (store, problems) = Store::parse(&text);
            for problem in problems {
                log(&format!("{}: {problem}", path.display()));
            }
            (store, Vec::new())
        }
        Err(_) => {
            let old = std::fs::read_to_string(order_path()).unwrap_or_default();
            let order: Vec<String> = icons::parse_order(&old)
                .iter()
                .filter_map(|k| store::migrate_order_key(k))
                .collect();
            if !order.is_empty() {
                log("Anordnung aus der HMP-09-Reihenfolge übernommen");
            }
            (Store::default(), order)
        }
    }
}

/// Whether a desktop directory starter is the user's own to change: a
/// regular file (no link into the system), writable, in a writable folder.
fn own_launcher(path: &Path) -> bool {
    let regular = path
        .symlink_metadata()
        .is_ok_and(|m| m.file_type().is_file());
    let writable = |p: &Path| {
        gio::File::for_path(p)
            .query_info(
                "access::can-write",
                gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
                gio::Cancellable::NONE,
            )
            .is_ok_and(|info| info.boolean("access::can-write"))
    };
    regular && writable(path) && path.parent().is_some_and(writable)
}

/// Changes the visible name of a starter: only the `Name` line the desktop
/// shows; the file is replaced atomically and keeps its permissions.
fn set_launcher_name(path: &Path, new: &str) -> Result<(), String> {
    let file = gio::File::for_path(path);
    let (bytes, etag) = file
        .load_contents(gio::Cancellable::NONE)
        .map_err(|err| err.message().to_owned())?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| "Die Starter-Datei ist kein gültiger Text.".to_owned())?;
    let keys = glib::KeyFile::new();
    keys.load_from_data(text, glib::KeyFileFlags::KEEP_TRANSLATIONS)
        .map_err(|err| format!("Die Starter-Datei ist ungültig: {}", err.message()))?;
    let key = rename::name_key(
        keys.locale_for_key("Desktop Entry", "Name", None)
            .as_deref(),
    );
    let changed = rename::with_name(text, &key, new)
        .ok_or_else(|| "Die Starter-Datei hat keine Gruppe [Desktop Entry].".to_owned())?;
    let mode =
        std::fs::metadata(path).map(|m| std::os::unix::fs::PermissionsExt::mode(&m.permissions()));
    // The etag refuses to overwrite a file changed in the meantime.
    file.replace_contents(
        changed.as_bytes(),
        etag.as_deref(),
        false,
        gio::FileCreateFlags::NONE,
        gio::Cancellable::NONE,
    )
    .map_err(|err| match err.kind::<gio::IOErrorEnum>() {
        Some(gio::IOErrorEnum::WrongEtag) => {
            "Die Starter-Datei wurde inzwischen verändert. Bitte versuchen Sie es erneut."
                .to_owned()
        }
        _ => err.message().to_owned(),
    })?;
    if let Ok(mode) = mode {
        let _ = std::fs::set_permissions(
            path,
            std::os::unix::fs::PermissionsExt::from_mode(mode & 0o7777),
        );
    }
    Ok(())
}

/// Whether `item` can get a new name; Err: why not (for the user).
fn renamable(item: &Item) -> Result<(), String> {
    match &item.kind {
        Kind::File(path) | Kind::Folder(path) => {
            let info = gio::File::for_path(path).query_info(
                "access::can-rename",
                gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
                gio::Cancellable::NONE,
            );
            match info {
                Ok(info) if info.boolean("access::can-rename") => Ok(()),
                Ok(_) => Err(format!(
                    "Sie haben keine Berechtigung, „{}“ umzubenennen.",
                    item.name
                )),
                Err(_) => Err(format!("„{}“ gibt es nicht mehr.", item.name)),
            }
        }
        // a name of the user's own, kept in the state file
        Kind::Launcher(_) | Kind::Computer | Kind::Home | Kind::Trash | Kind::App(_) => Ok(()),
    }
}

fn show_error(title: &str, detail: &str) {
    let dialog = gtk4::AlertDialog::builder()
        .message(title)
        .detail(detail)
        .buttons(["Schließen"])
        .cancel_button(0)
        .default_button(0)
        .modal(false)
        .build();
    // No parent: the desktop is a layer surface, not a toplevel window.
    dialog.show(None::<&gtk4::Window>);
}

/// Which icons there are right now (see `icons.rs`), and whether the
/// desktop directory could be read.
fn load_items() -> (Vec<Item>, bool) {
    let mut items = Vec::new();
    let files = DesktopAppInfo::new(FILES_APP);
    if files.is_some() {
        items.push(Item::new(Kind::Computer, "Rechner"));
    }
    if gio::AppInfo::default_for_type("inode/directory", false).is_some() {
        items.push(Item::new(Kind::Home, "Persönlicher Ordner"));
    }
    if files.is_some() {
        items.push(Item::new(Kind::Trash, "Papierkorb"));
    }
    if let Some(terminal) = DesktopAppInfo::new(TERMINAL_APP) {
        items.push(Item::new(
            Kind::App(TERMINAL_APP.to_owned()),
            &terminal.display_name(),
        ));
    }
    let mut listed = false;
    if let Some(entries) = desktop_dir().and_then(|dir| std::fs::read_dir(dir).ok()) {
        listed = true;
        for entry in entries.filter_map(Result::ok) {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            let path = entry.path();
            if name.ends_with(".desktop")
                && path.is_file()
                && let Some(app) = DesktopAppInfo::from_filename(&path)
            {
                items.push(Item::new(Kind::Launcher(path), &app.display_name()));
            } else if path.is_dir() {
                items.push(Item::new(Kind::Folder(path), &name));
            } else {
                items.push(Item::new(Kind::File(path), &name));
            }
        }
    }
    (items, listed)
}

fn icon_for(item: &Item) -> gio::Icon {
    let themed = |name: &str| gio::ThemedIcon::new(name).upcast::<gio::Icon>();
    match &item.kind {
        Kind::Computer => themed("computer"),
        Kind::Home => themed("user-home"),
        Kind::Trash => {
            let count = trash_count();
            let full = count.unwrap_or(0) > 0;
            if debugging() {
                log(&format!(
                    "Papierkorb-Symbol: {} ({count:?} Objekte)",
                    if full { "voll" } else { "leer" }
                ));
            }
            themed(if full {
                "user-trash-full"
            } else {
                "user-trash"
            })
        }
        Kind::App(id) => DesktopAppInfo::new(id)
            .and_then(|app| app.icon())
            .unwrap_or_else(|| themed("application-x-executable")),
        Kind::Launcher(path) => DesktopAppInfo::from_filename(path)
            .and_then(|app| app.icon())
            .unwrap_or_else(|| themed("application-x-executable")),
        Kind::File(path) | Kind::Folder(path) => gio::File::for_path(path)
            .query_info(
                "standard::icon,standard::content-type",
                gio::FileQueryInfoFlags::NONE,
                gio::Cancellable::NONE,
            )
            .ok()
            .and_then(|info| {
                // An empty file ("Leere Datei") is a blank page, as on
                // GNOME 2, not the theme's grey "zero size" picture.
                if icons::is_empty_document(info.content_type().as_deref()) {
                    Some(themed("text-x-generic"))
                } else {
                    info.icon()
                }
            })
            .unwrap_or_else(|| themed("text-x-generic")),
    }
}

/// The lines a label shows right now (`QUERENCIA_DESKTOP_DEBUG=1`, for the
/// isolated test of HMP-10b).
fn label_lines(label: &Label) -> Vec<String> {
    let layout = label.layout();
    let text = layout.text().to_string();
    let mut lines = Vec::new();
    let mut iter = layout.iter();
    loop {
        if let Some(line) = iter.line_readonly() {
            let start = usize::try_from(line.start_index()).unwrap_or(0);
            let end = start + usize::try_from(line.length()).unwrap_or(0);
            lines.push(
                text.get(start..end)
                    .unwrap_or_default()
                    .trim_end()
                    .to_owned(),
            );
        }
        if !iter.next_line() {
            break;
        }
    }
    // An ellipsized last line holds the whole rest of the text; on screen
    // it is cut and ends with "…". Marked, not shown as drawn.
    if layout.is_ellipsized()
        && let Some(last) = lines.last_mut()
    {
        last.push_str(" [gekürzt]");
    }
    lines
}

fn icon_widget(item: &Item) -> gtk4::Box {
    let tile = gtk4::Box::new(Orientation::Vertical, 2);
    tile.add_css_class("desktop-icon");
    // HMP-10b: the tile spans the whole cell width (cells never overlap,
    // every grid keeps at least a cell between columns). With 6 px less,
    // "Persönlicher" (89 px in DejaVu/Adwaita Sans 11) missed the 88 px
    // left for the text by one pixel and was split ("Persönliche" / "r").
    tile.set_size_request(CELL.0, CELL.1 - 6);
    let image = Image::from_gicon(&icon_for(item));
    image.set_pixel_size(ICON_PX);
    image.set_halign(Align::Center);
    let label = Label::new(Some(&item.name));
    label.set_wrap(true);
    label.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
    // GTK sizes a wrapped label to the narrowest width with the same number
    // of lines, which may split a word: keep whole words for that width.
    label.set_natural_wrap_mode(gtk4::NaturalWrapMode::Word);
    label.set_lines(2);
    label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    label.set_justify(gtk4::Justification::Center);
    // GNOME 2 broke desktop labels between words ("Persönlicher" /
    // "Ordner") and only split over-long words, never with a hyphen.
    label.set_max_width_chars(14);
    let attributes = gtk4::pango::AttrList::new();
    attributes.insert(gtk4::pango::AttrInt::new_insert_hyphens(false));
    label.set_attributes(Some(&attributes));
    label.set_halign(Align::Center);
    tile.append(&image);
    tile.append(&label);
    tile
}

fn open(item: &Item, window: &ApplicationWindow) {
    let context = WidgetExt::display(window).app_launch_context();
    let with_files = |uri: &str| match DesktopAppInfo::new(FILES_APP) {
        Some(app) => app.launch_uris(&[uri], Some(&context)),
        None => Err(glib::Error::new(
            gio::IOErrorEnum::NotFound,
            "Querencia Files fehlt",
        )),
    };
    let result = match &item.kind {
        Kind::Computer => with_files("computer:///"),
        Kind::Trash => with_files("trash:///"),
        Kind::Home => gio::AppInfo::launch_default_for_uri(
            &gio::File::for_path(glib::home_dir()).uri(),
            Some(&context),
        ),
        Kind::App(id) => match DesktopAppInfo::new(id) {
            Some(app) => app.launch(&[], Some(&context)),
            None => Err(glib::Error::new(
                gio::IOErrorEnum::NotFound,
                "Anwendung fehlt",
            )),
        },
        Kind::Launcher(path) => {
            if is_trusted(path) {
                launch_starter(path, &context)
            } else {
                ask_trust(item, path.clone(), context.clone());
                return;
            }
        }
        Kind::File(path) | Kind::Folder(path) => {
            gio::AppInfo::launch_default_for_uri(&gio::File::for_path(path).uri(), Some(&context))
        }
    };
    match result {
        Ok(()) => log(&format!("geöffnet: {}", item.name)),
        Err(err) => log(&format!("{} nicht geöffnet: {err}", item.name)),
    }
}

fn launch_starter(path: &Path, context: &gdk::AppLaunchContext) -> Result<(), glib::Error> {
    match DesktopAppInfo::from_filename(path) {
        Some(app) => app.launch(&[], Some(context)),
        None => Err(glib::Error::new(
            gio::IOErrorEnum::NotFound,
            "Starter ungültig",
        )),
    }
}

/// A starter on the desktop runs only when it is executable for its owner,
/// as Nautilus 2.30 required ("vertrauenswürdig").
fn is_trusted(path: &Path) -> bool {
    let executable = std::fs::metadata(path)
        .is_ok_and(|m| std::os::unix::fs::PermissionsExt::mode(&m.permissions()) & 0o100 != 0);
    // A link to an installed starter (…/applications of the XDG data
    // directories) is trusted, as Nautilus 2.30 trusted system launchers.
    let installed = std::fs::canonicalize(path).is_ok_and(|real| {
        std::iter::once(glib::user_data_dir())
            .chain(glib::system_data_dirs())
            .filter_map(|dir| std::fs::canonicalize(dir.join("applications")).ok())
            .any(|apps| real.starts_with(apps))
    });
    executable || installed
}

/// Nautilus 2.30's question for an untrusted starter.
fn ask_trust(item: &Item, path: PathBuf, context: gdk::AppLaunchContext) {
    let dialog = gtk4::AlertDialog::builder()
        .message("Nicht vertrauenswürdiger Anwendungsstarter")
        .detail(format!(
            "Der Anwendungsstarter „{}“ wurde nicht als vertrauenswürdig markiert. \
             Wenn Sie die Herkunft dieser Datei nicht kennen, kann ihr Start unsicher sein.",
            item.name
        ))
        .buttons([
            "Abbrechen",
            "Als vertrauenswürdig markieren",
            "Trotzdem starten",
        ])
        .cancel_button(0)
        .default_button(0)
        .modal(false)
        .build();
    let name = item.name.clone();
    dialog.choose(
        None::<&gtk4::Window>,
        gio::Cancellable::NONE,
        move |answer| {
            let answer = answer.unwrap_or(0);
            if answer == 0 {
                log(&format!("Starter {name} nicht gestartet"));
                return;
            }
            if answer == 1 {
                let mode = std::fs::metadata(&path)
                    .map(|m| std::os::unix::fs::PermissionsExt::mode(&m.permissions()))
                    .unwrap_or(0o644);
                let marked = gio::File::for_path(&path).set_attribute_uint32(
                    "unix::mode",
                    (mode | 0o100) & 0o7777,
                    gio::FileQueryInfoFlags::NONE,
                    gio::Cancellable::NONE,
                );
                match marked {
                    Ok(()) => log(&format!("Starter {name} als vertrauenswürdig markiert")),
                    Err(err) => log(&format!("Starter {name} nicht markiert: {err}")),
                }
            }
            match launch_starter(&path, &context) {
                Ok(()) => log(&format!("geöffnet: {name}")),
                Err(err) => log(&format!("{name} nicht geöffnet: {err}")),
            }
        },
    );
}

/// "Leere Datei" (`template` empty) or a copy of a template, under a free
/// name; GIO's create/copy without overwrite flags fail on existing names.
fn new_document(dir: &std::path::Path, template: &str) {
    if template.is_empty() {
        let name = icons::unique_name("Neues Dokument", |n| dir.join(n).exists());
        match gio::File::for_path(dir.join(&name))
            .create(gio::FileCreateFlags::NONE, gio::Cancellable::NONE)
        {
            Ok(stream) => {
                let _ = stream.close(gio::Cancellable::NONE);
                log(&format!("Dokument angelegt: {name}"));
            }
            Err(err) => log(&format!("Dokument {name} nicht angelegt: {err}")),
        }
        return;
    }
    let source = gio::File::for_path(template);
    let base = source
        .basename()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Vorlage".to_owned());
    let name = icons::unique_name(&base, |n| dir.join(n).exists());
    match source.copy(
        &gio::File::for_path(dir.join(&name)),
        gio::FileCopyFlags::NONE,
        gio::Cancellable::NONE,
        None,
    ) {
        Ok(()) => log(&format!("Dokument aus Vorlage angelegt: {name}")),
        Err(err) => log(&format!("Vorlage {base} nicht kopiert: {err}")),
    }
}

/// QE-01 A3: `querencia-einstellungen`, first on `PATH`, then in
/// `~/opt/querencia/bin` (where the session installs Querencia programs).
pub fn settings_program(path_var: Option<&std::ffi::OsStr>, home: &Path) -> Option<PathBuf> {
    let executable = |p: &Path| {
        std::fs::metadata(p).is_ok_and(|m| {
            m.is_file() && std::os::unix::fs::PermissionsExt::mode(&m.permissions()) & 0o111 != 0
        })
    };
    path_var
        .into_iter()
        .flat_map(std::env::split_paths)
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join("querencia-einstellungen"))
        .chain(std::iter::once(
            home.join("opt/querencia/bin/querencia-einstellungen"),
        ))
        .find(|p| executable(p))
}

/// "Hintergrund der Arbeitsfläche ändern …" (QE-01 A3): Querencia
/// Einstellungen' background page when it is installed; it is then the only
/// writer of `desktop.ini` (the desktop follows the file). Otherwise, or
/// when it cannot be started, the file dialog of HMP-09.
fn change_background(icons: &Rc<Icons>) {
    if let Some(program) = settings_program(std::env::var_os("PATH").as_deref(), &glib::home_dir())
    {
        match start_settings(icons, &program) {
            Ok(()) => {
                log(&format!(
                    "Hintergrund-Einstellungen gestartet: {} --hintergrund",
                    program.display()
                ));
                return;
            }
            Err(err) => log(&format!(
                "{} nicht gestartet ({err}): Dateiauswahl",
                program.display()
            )),
        }
    }
    choose_background(icons.background_changed.clone());
}

/// Starts `<program> --hintergrund` in the home folder with an activation
/// token of the display, so the window may take the focus. Spawned directly
/// (not through `gio-launch-desktop`): a program that cannot be executed is
/// reported here, and the file dialog takes over. GIO reaps the child.
fn start_settings(icons: &Rc<Icons>, program: &Path) -> Result<(), glib::Error> {
    let launcher = gio::SubprocessLauncher::new(gio::SubprocessFlags::NONE);
    launcher.set_cwd(glib::home_dir());
    let context = WidgetExt::display(&icons.window).app_launch_context();
    if let Some(token) = context.startup_notify_id(None::<&gio::AppInfo>, &[]) {
        launcher.setenv("XDG_ACTIVATION_TOKEN", token.as_str(), true);
        launcher.setenv("DESKTOP_STARTUP_ID", token.as_str(), true);
    }
    launcher
        .spawn(&[program.as_os_str(), std::ffi::OsStr::new("--hintergrund")])
        .map(drop)
}

/// "Hintergrund der Arbeitsfläche ändern …": a file dialog for an image;
/// the choice goes into `desktop.ini` (HMP-08 shows it at once). Mode and
/// colour stay as they are.
fn choose_background(done: Rc<dyn Fn()>) {
    let filter = gtk4::FileFilter::new();
    filter.set_name(Some("Bilder"));
    filter.add_mime_type("image/*");
    let filters = gio::ListStore::new::<gtk4::FileFilter>();
    filters.append(&filter);
    let dialog = gtk4::FileDialog::builder()
        .title("Hintergrund der Arbeitsfläche ändern")
        .modal(false)
        .filters(&filters)
        .default_filter(&filter)
        .build();
    // No parent: the desktop is a layer surface, not a toplevel window.
    dialog.open(
        None::<&gtk4::Window>,
        gio::Cancellable::NONE,
        move |result| {
            let Ok(file) = result else { return };
            let Some(path) = file.path() else { return };
            let config_path = config::config_path();
            let text = std::fs::read_to_string(&config_path).unwrap_or_default();
            let (mut background, _) = config::parse(&text);
            background.file = Some(path.clone());
            // a chosen image is shown, also after the colour only (QE-01)
            background.kind = config::Kind::Image;
            let written = config_path
                .parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .and_then(|()| {
                    let temporary = config_path.with_extension("ini.neu");
                    std::fs::write(&temporary, config::render(&background))?;
                    std::fs::rename(&temporary, &config_path)
                });
            match written {
                Ok(()) => {
                    log(&format!("Hintergrund gewählt: {}", path.display()));
                    done();
                }
                Err(err) => log(&format!("desktop.ini nicht geschrieben: {err}")),
            }
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exe(path: &Path) {
        std::fs::write(path, "#!/bin/sh\n").expect("fixture");
        std::fs::set_permissions(path, std::os::unix::fs::PermissionsExt::from_mode(0o755))
            .expect("fixture");
    }

    /// QE-01 A3: PATH first, then ~/opt/querencia/bin; nothing else.
    #[test]
    fn the_settings_program_is_found_on_path_then_in_opt() {
        let base = std::env::temp_dir().join(format!("qe01-einstellungen-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (bin, home) = (base.join("bin"), base.join("home"));
        std::fs::create_dir_all(&bin).expect("fixture");
        std::fs::create_dir_all(home.join("opt/querencia/bin")).expect("fixture");
        let path = std::env::join_paths([bin.clone(), PathBuf::from("/nirgends")]).expect("pfad");
        // neither: the file dialog stays
        assert_eq!(settings_program(Some(&path), &home), None);
        assert_eq!(settings_program(None, &home), None);
        // only in ~/opt
        let opt = home.join("opt/querencia/bin/querencia-einstellungen");
        exe(&opt);
        assert_eq!(settings_program(Some(&path), &home), Some(opt.clone()));
        // PATH wins
        let on_path = bin.join("querencia-einstellungen");
        exe(&on_path);
        assert_eq!(settings_program(Some(&path), &home), Some(on_path.clone()));
        // not executable or relative PATH entries do not count
        std::fs::set_permissions(
            &on_path,
            std::os::unix::fs::PermissionsExt::from_mode(0o644),
        )
        .expect("fixture");
        assert_eq!(settings_program(Some(&path), &home), Some(opt));
        let relative = std::ffi::OsString::from("bin");
        std::fs::remove_file(home.join("opt/querencia/bin/querencia-einstellungen"))
            .expect("cleanup");
        assert_eq!(settings_program(Some(&relative), &home), None);
        std::fs::remove_dir_all(&base).expect("cleanup");
    }
}
