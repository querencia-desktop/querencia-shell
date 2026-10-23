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
//!   "Nach Namen aufräumen", "Einfügen" and "Hintergrund der Arbeitsfläche
//!   ändern …" (writes `desktop.ini`, HMP-08 applies it). Not yet:
//!   "Starter anlegen".
//! - Desktop 0.2.3 (QD-108): files and folders have GNOME 2's menu (Öffnen,
//!   Öffnen mit, Ausschneiden, Kopieren, Einfügen in den Ordner,
//!   Umbenennen, In den Papierkorb verschieben, Eigenschaften), Strg+X/C/V,
//!   Entf and Umschalt+Entf, and go to and come from other programs by
//!   clipboard and drag and drop (`transfer.rs`). Dragging icons is a real
//!   drag now; inside the desktop it still only moves the icons.
//! - New folders and documents are jobs of `querencia_dateiarbeit` that
//!   pick a free name and never replace: nothing is ever overwritten. The
//!   desktop directory itself is created only by such a user action.
//! - QE-03: mounted removable volumes (`volumes.rs`) appear below the trash
//!   while they are mounted (GIO's volume monitor), open in Querencia Files
//!   and offer "Auswerfen" or "Aushängen". They can be moved like any icon,
//!   but their places are kept in memory only, never in `desktop-symbole`
//!   (`QUERENCIA_DESKTOP_VOLUMES=0` leaves them out).
//! - 0.3.0 (E3): removable media that are not mounted appear too; a double
//!   click or "Einhängen" mounts them (passwords through GTK's mount
//!   operation) and opens them. Media inserted while the desktop runs are
//!   mounted at once when GIO says so (`should_automount`), as GNOME 2 did.
//! - 0.3.0: copies, moves, the trash and new objects are jobs with the
//!   window "Dateioperationen" and the question for taken names (E2,
//!   `transfer.rs`, `fortschritt.rs`, `frage.rs`); the trash icon follows
//!   `papierkorb::Papierkorb`, the folder `ueberwachung::FolderWatch`.
//! - QE-04 (QD-042): `desktop.ini` `[Icons]` hides Rechner, Persönlicher
//!   Ordner or Papierkorb and sets the icon size (32/48/72/96, the cell
//!   grows with it); `[Volumes]` chooses the volume types (removable,
//!   optical, network shares, internal). Changes apply at once.

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

use querencia_dateiarbeit::auftraege::{JobId, Op, Queue};
use querencia_dateiarbeit::papierkorb::Papierkorb;
use querencia_dateiarbeit::ueberwachung::{self, Aenderung, FolderWatch};
use querencia_dateiarbeit::{laufwerke, namen, oeffnen, typ};
use querencia_oberflaeche::symbole;

use crate::classic_menu;
use crate::config;
use crate::i18n::{gettext, gettext_f};
use crate::icons::{self, Item, Kind};
use crate::layout::{self, Geometry, Point};
use crate::store::{self, Store};
use crate::volumes::{self, Removal, Volume};

mod fortschritt;
mod frage;
mod transfer;

/// The program for Rechner, Papierkorb and volumes: Querencia Dateien, then
/// Querencia Files under its new and its old desktop ID
/// (`oeffnen::FILE_MANAGERS`, since 0.3.0 shared with the file manager).
pub fn files_app() -> Option<DesktopAppInfo> {
    oeffnen::file_manager()
}
pub const TERMINAL_APP: &str = "org.example.Querencia.Terminal.desktop";
// Grid cell at the reference font (11 pt, as HMP-10b measured): wide
// enough for "Persönlicher" on one line (GNOME 2 broke labels between
// words). One icon with its label (at most two lines) takes one cell;
// cells never overlap.
const CELL: (i32, i32) = (100, 88);
const MARGIN: i32 = 8;
/// What a tile draws around its label text: the tile's padding (1 px each
/// side) and the label's (3 px left and right, 1 px top and bottom), see
/// `DESKTOP_CSS`; the spacing between icon and label (2 px); and the free
/// gap below a tile before the next one.
const TILE_SIDES: i32 = 2 + 6;
const TILE_ROWS: i32 = 2 + 2 + 2;
const TILE_GAP: i32 = 6;
/// Characters on one label line, as many as `max_width_chars` of the label
/// allows: room for "Persönlicher" and for host names of network shares
/// ("nas-test.local").
const LINE_CHARS: i32 = 14;

/// QD-091: the label font in pixels, measured from the icon layer's Pango
/// context (the font of the session, its size and scaling).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LabelFont {
    /// Pango's approximate character width.
    pub char_width: f64,
    /// The height of two label lines.
    pub two_lines: i32,
}

impl LabelFont {
    /// DejaVu Sans 11 at 96 dpi, the font HMP-10b sized the cells for.
    pub const REFERENCE: LabelFont = LabelFont {
        char_width: 7.66,
        two_lines: 36,
    };

    fn measure(widget: &impl IsA<gtk4::Widget>) -> LabelFont {
        let context = widget.pango_context();
        let metrics = context.metrics(None, None);
        let char_width =
            f64::from(metrics.approximate_char_width()) / f64::from(gtk4::pango::SCALE);
        let layout = widget.create_pango_layout(Some("Ag\nAg"));
        let (_, two_lines) = layout.pixel_size();
        if char_width > 0.0 && two_lines > 0 {
            LabelFont {
                char_width,
                two_lines,
            }
        } else {
            LabelFont::REFERENCE
        }
    }
}

/// QE-04: the grid cell for an icon size of `desktop.ini` `[Icons] Size`.
/// QD-091: never smaller than the drawn tile in the session's font:
/// `LINE_CHARS` average characters per line, the icon, two label lines and
/// a gap below, so that neighbouring tiles never touch.
fn cell_for(icon_px: i32, font: LabelFont) -> (i32, i32) {
    let base = match icon_px {
        32 => (CELL.0, 72),
        72 => (110, 112),
        96 => (130, 136),
        _ => CELL,
    };
    let text = (f64::from(LINE_CHARS) * font.char_width).ceil() as i32;
    (
        base.0.max(text + TILE_SIDES),
        base.1.max(icon_px + font.two_lines + TILE_ROWS + TILE_GAP),
    )
}
// The rename box is a little wider than a cell, as Nautilus' was.
const RENAME_WIDTH: i32 = 150;

/// What a `desktop.*` menu action does.
type ActionFn = Box<dyn Fn(&Rc<Icons>, Option<&glib::Variant>)>;

/// A press with the left button that may become a rubber band (HMP-10a).
/// A press on an icon becomes a drag of `transfer.rs` instead (0.2.3).
struct Drag {
    /// The icon under the pointer at the press; `None`: the free desktop
    /// (a rubber band).
    index: Option<usize>,
    start: (f64, f64),
    /// Past the threshold on the free desktop: the rectangle and the
    /// selection it adds to (Strg/Umschalt kept it).
    band: Option<(gtk4::Box, BTreeSet<usize>)>,
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
    /// 0.3.0: the desktop folder (`ueberwachung::FolderWatch`, waits for a
    /// missing one, QD-112).
    folder_watch: RefCell<Option<FolderWatch>>,
    /// Applies a new desktop.ini at once (main.rs `load`).
    background_changed: Rc<dyn Fn()>,
    drag: RefCell<Option<Drag>>,
    /// 0.2.3: the icons being dragged (`transfer.rs`).
    own_drag: RefCell<Option<transfer::OwnDrag>>,
    /// 0.2.3: the folder or trash icon a drop would go into.
    drop_over: Cell<Option<usize>>,
    /// 0.2.3: the clipboard while the desktop owns it.
    clip: RefCell<Option<transfer::Clip>>,
    /// 0.2.3: where the last menu opened (a paste puts the files there).
    menu_point: Cell<(f64, f64)>,
    /// Counts drags; a double click must not span one.
    drags: Cell<u64>,
    drags_at_first_press: Cell<u64>,
    drags_at_press: Cell<u64>,
    /// This press only ended a rename: no selection, drag or open.
    press_swallowed: Cell<bool>,
    editing: RefCell<Option<Editing>>,
    /// A refresh that waits for the end of a drag or rename.
    reload_waiting: Cell<bool>,
    /// QE-02: what the trash icon shows (`Some(true)`: full); `None` without
    /// a trash icon.
    trash_full: Cell<Option<bool>>,
    /// 0.3.0: the trash count (a monitor, counted off the main thread).
    papierkorb: Papierkorb,
    /// 0.3.0: the jobs (copy, move, trash, …), one thread each.
    queue: Queue,
    running: RefCell<std::collections::HashMap<JobId, transfer::Running>>,
    /// 0.3.0: the window "Dateioperationen" and the open questions.
    fortschritt: Rc<fortschritt::Fortschritt>,
    fragen: Rc<frage::Fragen>,
    /// QE-03: `None` when volume icons are switched off.
    volume_monitor: Option<gio::VolumeMonitor>,
    /// QE-03: places of volume icons, for this session only.
    volume_places: RefCell<volumes::Places>,
    volume_timer: RefCell<Option<glib::SourceId>>,
    /// QE-04: `desktop.ini` `[Icons]`/`[Volumes]` (main.rs `load` passes
    /// changes on, see `apply_settings`).
    settings: Cell<config::IconSettings>,
    /// QD-091: the label font the cells are sized for (`None` before the
    /// first measurement).
    font: Cell<Option<LabelFont>>,
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
            .title(gettext("Desktop – Icons"))
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
        let settings = config::parse_icons(
            &std::fs::read_to_string(config::config_path()).unwrap_or_default(),
        )
        .0;
        log(&settings_text(&settings));
        let icons = Rc::new_cyclic(|weak_icons: &std::rc::Weak<Self>| Self {
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
                tile: cell_for(settings.size, LabelFont::REFERENCE),
                margin: MARGIN,
                // "Am Raster ausrichten": on unless switched off
                snap: store.snap.unwrap_or(true),
            }),
            store: RefCell::new(store),
            migrated_order,
            backed_up: Cell::new(false),
            folder_watch: RefCell::default(),
            background_changed,
            drag: RefCell::default(),
            own_drag: RefCell::default(),
            drop_over: Cell::new(None),
            clip: RefCell::default(),
            menu_point: Cell::new((0.0, 0.0)),
            drags: Cell::new(0),
            drags_at_first_press: Cell::new(0),
            drags_at_press: Cell::new(0),
            press_swallowed: Cell::new(false),
            editing: RefCell::default(),
            reload_waiting: Cell::new(false),
            trash_full: Cell::new(None),
            papierkorb: Papierkorb::new(),
            queue: Queue::new(),
            running: RefCell::default(),
            fortschritt: fortschritt::Fortschritt::new({
                let weak = weak_icons.clone();
                move |id| {
                    if let Some(icons) = weak.upgrade() {
                        icons.cancel_job(id);
                    }
                }
            }),
            fragen: Rc::default(),
            volume_monitor: crate::icons_enabled(
                std::env::var("QUERENCIA_DESKTOP_VOLUMES").ok().as_deref(),
            )
            .then(gio::VolumeMonitor::get),
            volume_places: RefCell::default(),
            volume_timer: RefCell::default(),
            settings: Cell::new(settings),
            font: Cell::new(None),
            last_logged: RefCell::default(),
            last_selection: RefCell::default(),
            check_tiles: Cell::new(false),
        });
        let weak = Rc::downgrade(&icons);
        icons.queue.subscribe(move |event| {
            if let Some(icons) = weak.upgrade() {
                icons.job_event(event);
            }
        });
        // QE-02: the trash icon follows the count, without polling
        let weak = Rc::downgrade(&icons);
        icons.papierkorb.connect_changed(move |count| {
            if debugging() {
                log(&format!("Papierkorb gezählt: {count:?}"));
            }
            if let Some(icons) = weak.upgrade() {
                icons.show_trash_state(count);
            }
        });
        icons.install_actions();
        icons.install_clicks();
        icons.install_drag();
        icons.install_dnd();
        icons.install_keys();
        icons.watch_clipboard();
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
        log(&format!(
            "Dateiverwaltung: {}",
            files_app()
                .and_then(|app| app.id())
                .map_or("keine".to_owned(), |id| id.to_string())
        ));
        icons.update_font();
        icons.watch_font();
        icons.watch();
        icons.watch_volumes();
        icons.reload();
        for item in icons.items.borrow().iter() {
            log_volume(item);
        }
        icons.window.present();
        icons
    }

    /// QD-091: measures the label font; when it changed, the cells follow
    /// (`cell_for`). Returns whether it changed.
    fn update_font(&self) -> bool {
        let font = LabelFont::measure(&self.fixed);
        if Some(font) == self.font.get() {
            return false;
        }
        self.font.set(Some(font));
        let mut geometry = self.geometry.get();
        geometry.tile = cell_for(self.settings.get().size, font);
        self.geometry.set(geometry);
        log(&format!(
            "Schrift der Beschriftung: Zeichen {:.2} px, zwei Zeilen {} px, Zelle {}x{}",
            font.char_width, font.two_lines, geometry.tile.0, geometry.tile.1
        ));
        true
    }

    /// QD-091: another font (name, size, scaling) resizes the cells and
    /// rebuilds the icons; places are kept, the grid snaps them. Measured
    /// once the widgets took the new font (idle after the notification).
    fn watch_font(self: &Rc<Self>) {
        let Some(settings) = gtk4::Settings::default() else {
            return;
        };
        for property in ["gtk-font-name", "gtk-xft-dpi"] {
            let weak = Rc::downgrade(self);
            settings.connect_notify_local(Some(property), move |_, _| {
                let weak = weak.clone();
                glib::idle_add_local_once(move || {
                    if let Some(icons) = weak.upgrade()
                        && icons.update_font()
                    {
                        icons.reload();
                    }
                });
            });
        }
    }

    /// Re-reads which icons there are; places come from the state file.
    pub fn reload(self: &Rc<Self>) {
        if self.drag.borrow().is_some()
            || self.own_drag.borrow().is_some()
            || self.editing.borrow().is_some()
        {
            self.reload_waiting.set(true);
            return;
        }
        self.reload_waiting.set(false);
        let settings = self.settings.get();
        let (mut items, listed) = load_items(&settings);
        if let Some(monitor) = &self.volume_monitor {
            items.extend(volume_items(monitor, &settings));
        }
        let present: Vec<&str> = items
            .iter()
            .filter(|item| item.is_transient())
            .map(|item| item.key.as_str())
            .collect();
        self.volume_places.borrow_mut().keep_only(&present);
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

    /// QE-04: new `[Icons]`/`[Volumes]` from `desktop.ini`: the icons and
    /// their size change at once, without a restart. Places are kept (the
    /// grid of the new size snaps them, `layout.rs`).
    pub fn apply_settings(self: &Rc<Self>, settings: config::IconSettings) {
        if settings == self.settings.get() {
            return;
        }
        log(&format!(
            "Einstellungen geändert: {}",
            settings_text(&settings)
        ));
        self.settings.set(settings);
        let mut geometry = self.geometry.get();
        geometry.tile = cell_for(
            settings.size,
            self.font.get().unwrap_or(LabelFont::REFERENCE),
        );
        self.geometry.set(geometry);
        self.reload();
        self.check_tiles.set(true);
    }

    /// QE-02: swaps the picture of the trash icon (and of nothing else)
    /// when the trash changed between empty and full.
    fn show_trash_state(&self, count: Option<u32>) {
        let full = count.unwrap_or(0) > 0;
        if self.trash_full.get().is_none_or(|shown| shown == full) {
            return;
        }
        self.trash_full.set(Some(full));
        let index = self
            .items
            .borrow()
            .iter()
            .position(|item| item.kind == Kind::Trash);
        let image = index
            .and_then(|i| self.widgets.borrow().get(i).cloned())
            .and_then(|tile| tile.first_child())
            .and_then(|child| child.downcast::<Image>().ok());
        if let Some(image) = image {
            symbole::set(&image, &typ::trash_icon(full));
        }
        log(&format!(
            "Papierkorb jetzt {}",
            if full { "voll" } else { "leer" }
        ));
        log_trash_icon(full, count);
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
        self.drop_over.set(None);
        for widget in self.widgets.borrow_mut().drain(..) {
            self.fixed.remove(&widget);
        }
        // QE-02: the trash as last counted (0.3.0: `Papierkorb`, never
        // counted on the main thread); its signal keeps the icon up to date.
        let trash_full = if items.iter().any(|item| item.kind == Kind::Trash) {
            let count = self.papierkorb.anzahl();
            let full = count.unwrap_or(0) > 0;
            log_trash_icon(full, count);
            Some(full)
        } else {
            None
        };
        self.trash_full.set(trash_full);
        let widgets: Vec<gtk4::Box> = items
            .iter()
            .map(|item| {
                icon_widget(
                    item,
                    trash_full.unwrap_or(false),
                    self.settings.get().size,
                    self.geometry.get().tile,
                )
            })
            .collect();
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
        self.show_cut();
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
        // first), then the icons without a place; volumes after all of them.
        let (mut order, transient): (Vec<usize>, Vec<usize>) =
            (0..items.len()).partition(|&i| !items[i].is_transient());
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
        for (&i, (wanted, at)) in transient
            .iter()
            .zip(self.place_volumes(&geometry, &items, &positions, &transient))
        {
            positions[i] = at;
            if wanted.is_none() {
                self.volume_places
                    .borrow_mut()
                    .set(&items[i].key, geometry.free_place(at));
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

    /// QE-03: where the volumes `transient` go, after every other icon took
    /// its place in `positions`: (place from this session, position).
    fn place_volumes(
        &self,
        geometry: &Geometry,
        items: &[Item],
        positions: &[Point],
        transient: &[usize],
    ) -> Vec<(Option<layout::Place>, Point)> {
        let taken: Vec<Point> = (0..items.len())
            .filter(|i| !transient.contains(i))
            .map(|i| positions[i])
            .collect();
        let anchor = items
            .iter()
            .position(|item| item.kind == Kind::Trash)
            .map(|i| positions[i]);
        let places = self.volume_places.borrow();
        let wanted: Vec<_> = transient
            .iter()
            .map(|&i| places.get(&items[i].key))
            .collect();
        let at = geometry.place_transient(&taken, &wanted, anchor);
        wanted.into_iter().zip(at).collect()
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
        let selection = self.selection.borrow();
        for (index, (item, widget)) in items.iter().zip(widgets.iter()).enumerate() {
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
            let tile = self.geometry.get().tile;
            // a selected icon shows its whole name on purpose (`show_selection`)
            if !selection.contains(&index) && (w > tile.0 || h > tile.1) {
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
        let mut raised = false;
        for (index, widget) in self.widgets.borrow().iter().enumerate() {
            let selected = selection.contains(&index);
            if selected {
                widget.add_css_class("selected");
            } else {
                widget.remove_css_class("selected");
            }
            // QD-091: a selected icon shows its whole name, as GNOME 2 did,
            // drawn above its neighbours (the last child is drawn last)
            if let Some(label) = widget.last_child().and_downcast::<Label>() {
                let lines = if selected { -1 } else { 2 };
                if label.lines() != lines {
                    label.set_lines(lines);
                    label.set_ellipsize(if selected {
                        gtk4::pango::EllipsizeMode::None
                    } else {
                        gtk4::pango::EllipsizeMode::End
                    });
                }
            }
            if selected && widget.next_sibling().is_some() {
                widget.insert_before(&self.fixed, None::<&gtk4::Widget>);
                raised = true;
            }
        }
        // the rename box stays above every icon
        if raised
            && let Ok(editing) = self.editing.try_borrow()
            && let Some(edit) = editing.as_ref()
        {
            edit.entry.insert_before(&self.fixed, None::<&gtk4::Widget>);
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
            *icons.drag.borrow_mut() = Some(Drag {
                index: icons.hit(x, y),
                start: (x, y),
                band: None,
            });
        });
        let weak = Rc::downgrade(self);
        drag.connect_drag_update(move |_, dx, dy| {
            let Some(icons) = weak.upgrade() else { return };
            let mut state = icons.drag.borrow_mut();
            let Some(drag) = state.as_mut() else { return };
            // a press on an icon becomes a real drag (`transfer.rs`)
            if drag.index.is_some() {
                return;
            }
            if drag.band.is_none() {
                if !layout::is_drag(dx, dy, drag_threshold(&icons.window)) {
                    return;
                }
                icons.drags.set(icons.drags.get() + 1);
                icons.collapse_on_release.set(None);
                let frame = gtk4::Box::new(Orientation::Horizontal, 0);
                frame.add_css_class("rubberband");
                frame.set_can_target(false);
                icons.fixed.put(&frame, drag.start.0, drag.start.1);
                drag.band = Some((frame, icons.selection.borrow().clone()));
            }
            let geometry = icons.geometry.get();
            let positions = icons.positions.borrow().clone();
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
        drag.connect_drag_end(move |_, _, _| {
            let Some(icons) = weak.upgrade() else { return };
            icons.press_swallowed.set(false);
            let Some(drag) = icons.drag.borrow_mut().take() else {
                return;
            };
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
        let mut front = 0;
        for &(index, at) in moved {
            let Some(item) = items.get(index) else {
                continue;
            };
            if shown.get(index) == Some(&at) {
                continue;
            }
            any = true;
            if item.is_transient() {
                self.volume_places
                    .borrow_mut()
                    .set(&item.key, geometry.free_place(at));
                continue;
            }
            store.set_place(item.art(), &item.ident(), geometry.free_place(at));
            if let Some(i) = store.entries.iter().position(|e| e.key() == item.key) {
                let entry = store.entries.remove(i);
                store.entries.insert(front, entry);
                front += 1;
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
            let ctrl = modifiers.contains(gdk::ModifierType::CONTROL_MASK);
            // 0.2.3: Strg+X/C/V, Entf, Umschalt+Entf
            if ctrl && matches!(key, gdk::Key::x | gdk::Key::X | gdk::Key::c | gdk::Key::C) {
                icons.put_on_clipboard(matches!(key, gdk::Key::x | gdk::Key::X));
                return glib::Propagation::Stop;
            }
            if ctrl && matches!(key, gdk::Key::v | gdk::Key::V) {
                icons.paste(None, None);
                return glib::Propagation::Stop;
            }
            if matches!(key, gdk::Key::Delete | gdk::Key::KP_Delete) {
                if modifiers.contains(gdk::ModifierType::SHIFT_MASK) {
                    icons.confirm_delete();
                } else {
                    icons.trash_selection();
                }
                return glib::Propagation::Stop;
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
        self.menu_point.set((x, y));
        let volume = index
            .and_then(|i| self.items.borrow().get(i).cloned())
            .and_then(|item| match item.kind {
                Kind::Volume(volume) => Some(volume),
                _ => None,
            });
        if let Some(volume) = volume {
            // QE-03: GNOME 2's volume menu, without what Querencia lacks
            self.menu
                .append(Some(&gettext("Open")), Some("desktop.open"));
            let section = gio::Menu::new();
            if volume.uri.is_none() {
                // E3: a medium that is not mounted yet
                section.append(Some(&gettext("Mount")), Some("desktop.mount"));
            } else if let Some(removal) = volume.removal {
                section.append(Some(&removal.menu_label()), Some(volumes::action(removal)));
            }
            if section.n_items() > 0 {
                self.menu.append_section(None, &section);
            }
            if debugging() {
                log(&format!(
                    "Menü Datenträger: Öffnen{}",
                    if volume.uri.is_none() {
                        ", Einhängen".to_owned()
                    } else {
                        volume
                            .removal
                            .map_or(String::new(), |r| format!(", {}", r.label()))
                    }
                ));
            }
        } else if let Some(index) = index {
            let is_trash = self
                .items
                .borrow()
                .get(index)
                .is_some_and(|item| item.kind == Kind::Trash);
            if is_trash {
                // HMP-10b: GNOME 2's trash menu
                self.menu
                    .append(Some(&gettext("Open Trash")), Some("desktop.open"));
                let empty = gio::Menu::new();
                // insensitive while the trash is empty (or unreachable), as
                // last counted
                let count = self.papierkorb.anzahl();
                let full = count.unwrap_or(0) > 0;
                empty.append(
                    Some(&gettext("Empty Trash…")),
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
                self.menu
                    .append(Some(&gettext("Open")), Some("desktop.open"));
            }
            let item = self.items.borrow().get(index).cloned();
            let file = item
                .as_ref()
                .filter(|item| transfer::item_path(item).is_some());
            let single = self.selection.borrow().len() <= 1;
            if let Some(item) = file
                && matches!(item.kind, Kind::File(_) | Kind::Folder(_))
                && let Some(programs) = self.open_with_menu(item)
            {
                self.menu
                    .append_submenu(Some(&gettext("Open With")), &programs);
            }
            if let Some(item) = file {
                // 0.2.3: GNOME 2's clipboard section
                let clip = gio::Menu::new();
                clip.append(Some(&gettext("Cut")), Some("desktop.cut"));
                clip.append(Some(&gettext("Copy")), Some("desktop.copy"));
                if let Kind::Folder(path) = &item.kind {
                    let paste = gio::MenuItem::new(Some(&gettext("Paste Into Folder")), None);
                    if single && self.clipboard_has_files() {
                        paste.set_action_and_target_value(
                            Some("desktop.paste"),
                            Some(&path.to_string_lossy().to_variant()),
                        );
                    }
                    clip.append_item(&paste);
                }
                self.menu.append_section(None, &clip);
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
            edit.append(
                Some(&gettext("Rename…")),
                can_rename.then_some("desktop.rename"),
            );
            self.menu.append_section(None, &edit);
            if file.is_some() {
                let trash = gio::Menu::new();
                trash.append(Some(&gettext("Move to Trash")), Some("desktop.trash"));
                self.menu.append_section(None, &trash);
                let properties = gio::Menu::new();
                properties.append(
                    Some(&gettext("Properties")),
                    single.then_some("desktop.properties"),
                );
                self.menu.append_section(None, &properties);
            }
            if debugging() {
                log(&format!(
                    "Menü Symbol: {}",
                    menu_labels(&self.menu).join(" | ")
                ));
            }
        } else {
            let can_create = desktop_dir().is_some();
            let create = gio::Menu::new();
            if can_create {
                create.append(Some(&gettext("Create Folder")), Some("desktop.new-folder"));
                let documents = gio::Menu::new();
                documents.append_item(&gio::MenuItem::new(
                    Some(&gettext("Empty File")),
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
                create.append_submenu(Some(&gettext("Create Document")), &documents);
            }
            if create.n_items() > 0 {
                self.menu.append_section(None, &create);
            }
            let arrange = gio::Menu::new();
            arrange.append(
                Some(&gettext("Clean Up by Name")),
                Some("desktop.arrange-by-name"),
            );
            // HMP-10a: put every icon on the grid once; switch the grid on
            // or off (a check mark in the icon column, as GNOME 2 drew it)
            arrange.append(
                Some(&gettext("Align Icons to Grid")),
                Some("desktop.align-to-grid"),
            );
            let toggle =
                gio::MenuItem::new(Some(&gettext("Keep Aligned")), Some("desktop.toggle-grid"));
            if self.geometry.get().snap {
                toggle.set_icon(&gio::ThemedIcon::from_names(&[
                    "object-select-symbolic",
                    "emblem-ok",
                    "emblem-OK",
                ]));
            }
            arrange.append_item(&toggle);
            self.menu.append_section(None, &arrange);
            if can_create {
                // 0.2.3: the clipboard's files onto the desktop, at the click
                let paste = gio::MenuItem::new(Some(&gettext("Paste")), None);
                if self.clipboard_has_files() {
                    paste
                        .set_action_and_target_value(Some("desktop.paste"), Some(&"".to_variant()));
                }
                let section = gio::Menu::new();
                section.append_item(&paste);
                self.menu.append_section(None, &section);
            }
            let background = gio::Menu::new();
            background.append(
                Some(&gettext("Change Desktop Background…")),
                Some("desktop.change-background"),
            );
            self.menu.append_section(None, &background);
            if debugging() {
                log(&format!(
                    "Menü Fläche: {}",
                    menu_labels(&self.menu).join(" | ")
                ));
            }
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
        // 0.2.3 (`transfer.rs`)
        add(
            "open-with",
            Some(glib::VariantTy::STRING),
            Box::new(|icons, value| {
                let app = value.and_then(|v| v.str()).unwrap_or_default().to_owned();
                if let Some(item) = icons
                    .selected
                    .get()
                    .and_then(|i| icons.items.borrow().get(i).cloned())
                {
                    icons.open_with(&item, &app);
                }
            }),
        );
        add(
            "cut",
            None,
            Box::new(|icons, _| icons.put_on_clipboard(true)),
        );
        add(
            "copy",
            None,
            Box::new(|icons, _| icons.put_on_clipboard(false)),
        );
        add(
            "paste",
            Some(glib::VariantTy::STRING),
            Box::new(|icons, value| {
                let into = value.and_then(|v| v.str()).unwrap_or_default();
                if into.is_empty() {
                    // at the menu's place, as GNOME 2 pasted onto the desktop
                    let (x, y) = icons.menu_point.get();
                    let at = ((x - 50.0).round() as i32, (y - 25.0).round() as i32);
                    icons.paste(None, Some(at));
                } else {
                    icons.paste(Some(PathBuf::from(into)), None);
                }
            }),
        );
        add("trash", None, Box::new(|icons, _| icons.trash_selection()));
        add(
            "properties",
            None,
            Box::new(|icons, _| {
                if let Some(item) = icons
                    .selected
                    .get()
                    .and_then(|i| icons.items.borrow().get(i).cloned())
                {
                    icons.show_properties(&item);
                }
            }),
        );
        add(
            "unmount",
            None,
            Box::new(|icons, _| icons.remove_volume(Removal::Unmount)),
        );
        add(
            "eject",
            None,
            Box::new(|icons, _| icons.remove_volume(Removal::Eject)),
        );
        add(
            "mount",
            None,
            Box::new(|icons, _| {
                if let Some(item) = icons
                    .selected
                    .get()
                    .and_then(|i| icons.items.borrow().get(i).cloned())
                    && let Kind::Volume(volume) = &item.kind
                {
                    mount_volume(&item.name, volume, &icons.window, false);
                }
            }),
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
                    // a free name ("Neuer Ordner 2"), never an existing one
                    icons.start_job(transfer::Job::new(
                        Op::NewFolder {
                            name: gettext("New Folder"),
                        },
                        Vec::new(),
                        Some(dir),
                    ));
                }
            }),
        );
        add(
            "new-document",
            Some(glib::VariantTy::STRING),
            Box::new(|icons, value| {
                let template = value.and_then(|v| v.str()).unwrap_or_default().to_owned();
                if let Some(dir) = icons.ensure_desktop_dir() {
                    icons.start_job(new_document(dir, &template));
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
            let mut n = 0;
            for (item, at) in sorted.iter().zip(&positions) {
                if item.is_transient() {
                    self.volume_places
                        .borrow_mut()
                        .set(&item.key, geometry.top_left_place(*at));
                    continue;
                }
                store.set_place(item.art(), &item.ident(), geometry.top_left_place(*at));
                let entry = store.entries.pop().expect("eben eingefügt");
                store.entries.insert(n, entry);
                n += 1;
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
        if self.papierkorb.anzahl().unwrap_or(0) == 0 {
            log("Papierkorb ist leer: nichts zu tun");
            return;
        }
        let dialog = gtk4::AlertDialog::builder()
            .message(gettext("Empty all items from the trash?"))
            .detail(gettext(
                "If you empty the trash, all items in it will be permanently lost.",
            ))
            .buttons([gettext("Cancel"), gettext("Empty Trash")])
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

    /// Empties the trash as a job (0.3.0): every top-level object of
    /// `trash:///` (every trash location GVfs knows) is deleted through GIO,
    /// never following links out of the trash; with progress and
    /// "Abbrechen", and every object that could not be deleted is reported.
    fn empty_trash(self: &Rc<Self>) {
        log("Papierkorb leeren");
        self.start_job(transfer::Job::new(Op::EmptyTrash, Vec::new(), None));
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
        let (mut order, transient): (Vec<usize>, Vec<usize>) =
            (0..items.len()).partition(|&i| !items[i].is_transient());
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
        let mut positions = vec![(0, 0); items.len()];
        for (n, &i) in order.iter().enumerate() {
            positions[i] = placed[n];
        }
        for (&i, (wanted, at)) in transient
            .iter()
            .zip(self.place_volumes(&geometry, &items, &positions, &transient))
        {
            let place = match wanted {
                Some(old) => geometry.place_like(old, at),
                None => geometry.free_place(at),
            };
            self.volume_places.borrow_mut().set(&items[i].key, place);
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
            // the folder watch notices it (it waits in the parent, QD-112)
            log(&format!("Arbeitsflächen-Ordner {} angelegt", dir.display()));
        }
        Some(dir)
    }

    /// Follows the desktop directory (0.3.0: `FolderWatch`): a directory
    /// made after the start (or removed) is awaited in its nearest existing
    /// ancestor (QD-112). The trash has its own monitor (`Papierkorb`).
    fn watch(self: &Rc<Self>) {
        let Some(dir) = desktop_dir() else {
            return;
        };
        let watch = FolderWatch::new(&gio::File::for_path(&dir), true);
        if !watch.watches_folder() {
            log_waiting(&dir);
        }
        let weak = Rc::downgrade(self);
        watch.connect(move |changes| {
            if let Some(icons) = weak.upgrade() {
                icons.folder_changed(changes);
            }
        });
        *self.folder_watch.borrow_mut() = Some(watch);
    }

    /// A bundle of changes of the desktop directory (already collected over
    /// a short time): refresh, and a file renamed elsewhere (a file manager)
    /// keeps its place.
    fn folder_changed(self: &Rc<Self>, changes: &[Aenderung]) {
        let dir = desktop_dir().unwrap_or_default();
        for change in changes {
            match change {
                Aenderung::Umbenannt(old, new) => {
                    let (Some(old), Some(new)) = (old.basename(), new.basename()) else {
                        continue;
                    };
                    let (old, new) = (old.to_string_lossy(), new.to_string_lossy());
                    if self
                        .store
                        .borrow()
                        .place(&store::key(store::Art::File, &old))
                        .is_some()
                    {
                        self.store.borrow_mut().rename(&old, &new);
                        self.save();
                        log(&format!("umbenannt (außerhalb): {old} → {new}"));
                    }
                }
                Aenderung::OrdnerDa => {
                    log(&format!("Arbeitsflächen-Ordner da: {}", dir.display()));
                }
                Aenderung::OrdnerWeg => {
                    log(&format!("Arbeitsflächen-Ordner weg: {}", dir.display()));
                    log_waiting(&dir);
                }
                Aenderung::Neu(_)
                | Aenderung::Weg(_)
                | Aenderung::Geaendert(_)
                | Aenderung::NeuLaden => {}
            }
        }
        self.reload();
    }

    /// QE-03: follows mounts coming and going (and changing); 0.3.0 (E3):
    /// volumes too, and mounts media inserted now that GIO wants mounted.
    fn watch_volumes(self: &Rc<Self>) {
        let Some(monitor) = &self.volume_monitor else {
            log("Datenträger-Symbole aus (QUERENCIA_DESKTOP_VOLUMES=0)");
            return;
        };
        let soon = |icons: &std::rc::Weak<Self>| {
            let weak = icons.clone();
            move |_: &gio::VolumeMonitor, _: &gio::Mount| {
                if let Some(icons) = weak.upgrade() {
                    icons.update_volumes_soon();
                }
            }
        };
        let weak = Rc::downgrade(self);
        monitor.connect_mount_added(soon(&weak));
        monitor.connect_mount_removed(soon(&weak));
        monitor.connect_mount_changed(soon(&weak));
        let soon = |icons: &std::rc::Weak<Self>| {
            let weak = icons.clone();
            move |_: &gio::VolumeMonitor, _: &gio::Volume| {
                if let Some(icons) = weak.upgrade() {
                    icons.update_volumes_soon();
                }
            }
        };
        monitor.connect_volume_removed(soon(&weak));
        monitor.connect_volume_changed(soon(&weak));
        let weak = Rc::downgrade(self);
        monitor.connect_volume_added(move |_, volume| {
            let Some(icons) = weak.upgrade() else { return };
            icons.update_volumes_soon();
            automount(volume, &icons.window);
        });
    }

    /// QE-03: a look at the volumes shortly after monitor events (they
    /// come in bursts while a volume is mounted or taken away).
    fn update_volumes_soon(self: &Rc<Self>) {
        if let Some(id) = self.volume_timer.take() {
            id.remove();
        }
        let weak = Rc::downgrade(self);
        let id = glib::timeout_add_local_once(Duration::from_millis(120), move || {
            if let Some(icons) = weak.upgrade() {
                icons.volume_timer.take();
                icons.update_volumes();
            }
        });
        *self.volume_timer.borrow_mut() = Some(id);
    }

    /// QE-03: rebuilds the icons only when the volume icons changed; other
    /// mounts (network shares, portals of other sessions) change nothing.
    fn update_volumes(self: &Rc<Self>) {
        let Some(monitor) = &self.volume_monitor else {
            return;
        };
        let now = volume_items(monitor, &self.settings.get());
        let shown: Vec<Item> = self
            .items
            .borrow()
            .iter()
            .filter(|item| item.is_transient())
            .cloned()
            .collect();
        let mut changed = false;
        for item in now.iter().filter(|item| !shown.contains(item)) {
            log_volume(item);
            changed = true;
        }
        for item in shown.iter().filter(|item| !now.contains(item)) {
            log(&format!("Datenträger weg: {}", item.name));
            changed = true;
        }
        if changed {
            self.reload();
        }
    }

    /// QE-03 "Aushängen" or "Auswerfen" through GIO with a GTK mount
    /// operation (programs still using the volume: `ask_when_busy`); the
    /// icon goes when the volume monitor reports the mount gone.
    fn remove_volume(self: &Rc<Self>, removal: Removal) {
        let Some(item) = self
            .selected
            .get()
            .and_then(|i| self.items.borrow().get(i).cloned())
        else {
            return;
        };
        let (Kind::Volume(volume), Some(monitor)) = (&item.kind, &self.volume_monitor) else {
            return;
        };
        let Some(mount) = volume
            .uri
            .as_deref()
            .and_then(|uri| laufwerke::find_mount(monitor, uri))
        else {
            log(&format!("{} ist nicht mehr eingehängt", item.name));
            return;
        };
        let operation = gtk4::MountOperation::new(Some(&self.window));
        let question = ask_when_busy(&operation, removal, &item.name, mount.root().path());
        let name = item.name.clone();
        log(&format!("{}: {name}", removal.label()));
        let done = move |result: Result<(), glib::Error>| match finish_question(&question, result) {
            Ok(()) => log(&format!(
                "{}: {name}",
                match removal {
                    Removal::Eject => "ausgeworfen",
                    Removal::Unmount => "ausgehängt",
                }
            )),
            Err(err) => match laufwerke::failure(
                err.kind::<gio::IOErrorEnum>(),
                removal,
                &name,
                err.message(),
            ) {
                None => log(&format!("{} abgebrochen: {name} ({err})", removal.label())),
                Some((title, detail)) => {
                    log(&format!(
                        "{} fehlgeschlagen: {name}: {err}",
                        removal.label()
                    ));
                    show_error(&title, &detail);
                }
            },
        };
        glib::MainContext::default().spawn_local(async move {
            let result =
                laufwerke::entfernen(&mount, removal, &Rueckfrage(operation.upcast())).await;
            done(result);
        });
    }

    /// Edits the label of icon `index` in place (F2, "Umbenennen …").
    fn start_rename(self: &Rc<Self>, index: usize) {
        if self.editing.borrow().is_some() || self.drag.borrow().is_some() {
            return;
        }
        let Some(item) = self.items.borrow().get(index).cloned() else {
            return;
        };
        if item.is_transient() {
            log(&format!(
                "Datenträger werden nicht umbenannt: {}",
                item.name
            ));
            return;
        }
        if let Err(reason) = renamable(&item) {
            log(&format!(
                "Umbenennen nicht möglich: {}: {reason}",
                item.name
            ));
            show_error(
                &gettext_f("“{name}” cannot be renamed.", &[("name", &item.name)]),
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
        let left = (x + (geometry.tile.0 - RENAME_WIDTH) / 2)
            .clamp(0, (geometry.width - RENAME_WIDTH).max(0));
        self.fixed.put(
            &entry,
            f64::from(left),
            f64::from(y + self.settings.get().size + 3),
        );
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
        let selected = namen::selected_chars(&item.name, matches!(item.kind, Kind::File(_)));
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
    /// Files and folders: checked here, renamed by a job that never replaces
    /// (0.3.0, `RENAME_NOREPLACE`); its failure is reported when it ends.
    fn apply_rename(self: &Rc<Self>, item: &Item, new: &str) -> Result<(), (String, String)> {
        let failed = |detail: String| {
            (
                gettext_f(
                    "“{name}” could not be renamed to “{new}”.",
                    &[("name", &item.name), ("new", new)],
                ),
                detail,
            )
        };
        if new == item.name {
            return Ok(());
        }
        match &item.kind {
            Kind::File(path) | Kind::Folder(path) => {
                namen::check_file_name(new).map_err(|p| failed(p.text(new)))?;
                let old = item.ident();
                if path.symlink_metadata().is_err() {
                    return Err(failed(gettext_f(
                        "“{name}” no longer exists. Perhaps it was deleted or moved in the meantime.",
                        &[("name", &old)],
                    )));
                }
                if path.with_file_name(new).symlink_metadata().is_ok() {
                    return Err(failed(gettext_f(
                        "The name “{new}” is already used in this folder. \
                         Please use a different name.",
                        &[("new", new)],
                    )));
                }
                self.start_job_renaming(
                    transfer::Job::new(
                        Op::Rename {
                            new: new.to_owned(),
                        },
                        vec![gio::File::for_path(path).uri().to_string()],
                        path.parent().map(Path::to_path_buf),
                    ),
                    Some((old, new.to_owned())),
                );
                Ok(())
            }
            Kind::Launcher(path) => {
                namen::check_label(new).map_err(|p| failed(p.text(new)))?;
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
                namen::check_label(new).map_err(|p| failed(p.text(new)))?;
                self.set_alias(item, new);
                Ok(())
            }
            Kind::Volume(_) => Err(failed(volume_name_note())),
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

/// The labels of a menu, sections and submenus included (one line in the
/// log for the isolated tests; "(aus)" marks insensitive entries).
fn menu_labels(menu: &gio::Menu) -> Vec<String> {
    let mut labels = Vec::new();
    for i in 0..menu.n_items() {
        if let Some(section) = menu.item_link(i, "section").and_downcast::<gio::Menu>() {
            labels.extend(menu_labels(&section));
            continue;
        }
        let label = menu
            .item_attribute_value(i, "label", Some(glib::VariantTy::STRING))
            .and_then(|v| v.str().map(str::to_owned))
            .unwrap_or_default();
        let action = menu.item_attribute_value(i, "action", Some(glib::VariantTy::STRING));
        let submenu = menu.item_link(i, "submenu").is_some();
        labels.push(if action.is_none() && !submenu {
            format!("{label} (aus)")
        } else {
            label
        });
    }
    labels
}

/// `QUERENCIA_DESKTOP_DEBUG=1`: extra log lines for the isolated tests.
fn debugging() -> bool {
    std::env::var("QUERENCIA_DESKTOP_DEBUG").is_ok_and(|v| v == "1")
}

/// `QUERENCIA_DESKTOP_DEBUG=1`: the state the trash icon shows (HMP-10b
/// tests read the last line).
fn log_trash_icon(full: bool, count: Option<u32>) {
    if debugging() {
        log(&format!(
            "Papierkorb-Symbol: {} ({count:?} Objekte)",
            if full { "voll" } else { "leer" }
        ));
    }
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
    let text =
        std::str::from_utf8(&bytes).map_err(|_| gettext("The launcher file is not valid text."))?;
    let keys = glib::KeyFile::new();
    keys.load_from_data(text, glib::KeyFileFlags::KEEP_TRANSLATIONS)
        .map_err(|err| {
            gettext_f(
                "The launcher file is not valid: {why}",
                &[("why", err.message())],
            )
        })?;
    let key = namen::name_key(
        keys.locale_for_key("Desktop Entry", "Name", None)
            .as_deref(),
    );
    let changed = namen::with_name(text, &key, new)
        .ok_or_else(|| gettext("The launcher file has no group [Desktop Entry]."))?;
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
            gettext("The launcher file was changed in the meantime. Please try again.")
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
                Ok(_) => Err(gettext_f(
                    "You do not have the permissions to rename “{name}”.",
                    &[("name", &item.name)],
                )),
                Err(_) => Err(gettext_f(
                    "“{name}” no longer exists.",
                    &[("name", &item.name)],
                )),
            }
        }
        // a name of the user's own, kept in the state file
        Kind::Launcher(_) | Kind::Computer | Kind::Home | Kind::Trash | Kind::App(_) => Ok(()),
        Kind::Volume(_) => Err(volume_name_note()),
    }
}

fn volume_name_note() -> String {
    gettext("Volumes keep their own names.")
}

fn show_error(title: &str, detail: &str) {
    let dialog = gtk4::AlertDialog::builder()
        .message(title)
        .detail(detail)
        .buttons([gettext("Close")])
        .cancel_button(0)
        .default_button(0)
        .modal(false)
        .build();
    // No parent: the desktop is a layer surface, not a toplevel window.
    dialog.show(None::<&gtk4::Window>);
}

/// Which icons there are right now (see `icons.rs`), and whether the
/// desktop directory could be read.
/// QE-04: Rechner, Persönlicher Ordner and Papierkorb only when
/// `desktop.ini` `[Icons]` keeps them (they keep their places when hidden).
fn load_items(settings: &config::IconSettings) -> (Vec<Item>, bool) {
    let mut items = Vec::new();
    let files = files_app();
    if files.is_some() && settings.computer {
        items.push(Item::new(Kind::Computer, &gettext("Computer")));
    }
    if settings.home && gio::AppInfo::default_for_type("inode/directory", false).is_some() {
        items.push(Item::new(Kind::Home, &gettext("Home Folder")));
    }
    if files.is_some() && settings.trash {
        items.push(Item::new(Kind::Trash, &gettext("Trash")));
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

fn icon_for(item: &Item, trash_full: bool) -> gio::Icon {
    let themed = |name: &str| gio::ThemedIcon::new(name).upcast::<gio::Icon>();
    match &item.kind {
        Kind::Computer => themed("computer"),
        Kind::Home => themed("user-home"),
        Kind::Trash => typ::trash_icon(trash_full),
        Kind::Volume(volume) => laufwerke::volume_icon(volume.picture),
        Kind::App(id) => DesktopAppInfo::new(id)
            .and_then(|app| app.icon())
            .unwrap_or_else(|| themed("application-x-executable")),
        Kind::Launcher(path) => DesktopAppInfo::from_filename(path)
            .and_then(|app| app.icon())
            .unwrap_or_else(|| themed("application-x-executable")),
        // An empty file ("Leere Datei") is a blank page, as on GNOME 2, not
        // the theme's grey "zero size" picture (`typ::icon_for`).
        Kind::File(path) | Kind::Folder(path) => {
            let file = gio::File::for_path(path);
            file.query_info(
                typ::ICON_ATTRIBUTES,
                gio::FileQueryInfoFlags::NONE,
                gio::Cancellable::NONE,
            )
            .map(|info| typ::icon_for(&file, &info))
            .unwrap_or_else(|_| themed("text-x-generic"))
        }
    }
}

/// QE-03: an open busy question closes when the operation ends (GVfs does
/// not always abort it when the programs end).
fn finish_question(
    question: &Rc<RefCell<Option<gio::Cancellable>>>,
    result: Result<(), glib::Error>,
) -> Result<(), glib::Error> {
    if let Some(cancellable) = question.borrow_mut().take() {
        cancellable.cancel();
    }
    result
}

/// QE-03: GVfs asks the mount operation what to do while programs still
/// use the volume ("show-processes", again every few seconds). GTK's own
/// dialog for it is English without GTK's translations, so the desktop asks
/// in German instead: "Abbrechen" aborts (GIO then reports it as handled),
/// "Trotzdem …" picks GVfs' first choice ("Unmount Anyway"). When the
/// programs end, GVfs takes the volume away by itself and the question
/// closes. Returns the open question's cancellable.
fn ask_when_busy(
    operation: &gtk4::MountOperation,
    removal: Removal,
    name: &str,
    root: Option<PathBuf>,
) -> Rc<RefCell<Option<gio::Cancellable>>> {
    let asking: Rc<RefCell<Option<gio::Cancellable>>> = Rc::default();
    let name = name.to_owned();
    let open = asking.clone();
    operation.connect_local("show-processes", false, move |values| {
        let operation = values.first()?.get::<gio::MountOperation>().ok()?;
        operation.stop_signal_emission_by_name("show-processes");
        if open.borrow().is_some() {
            return None;
        }
        let users = root
            .as_deref()
            .map(|root| laufwerke::users_of(Path::new("/proc"), root))
            .unwrap_or_default();
        log(&format!(
            "Datenträger {name} wird noch benutzt: {}",
            users
                .iter()
                .map(|(pid, program)| format!("{program} ({pid})"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
        let (message, detail, buttons) = laufwerke::busy_question(removal, &name, &users);
        let dialog = gtk4::AlertDialog::builder()
            .message(message)
            .detail(detail)
            .buttons(buttons)
            .cancel_button(0)
            .default_button(0)
            .modal(false)
            .build();
        let cancellable = gio::Cancellable::new();
        *open.borrow_mut() = Some(cancellable.clone());
        let (open, name) = (open.clone(), name.clone());
        dialog.choose(None::<&gtk4::Window>, Some(&cancellable), move |answer| {
            open.borrow_mut().take();
            match answer {
                Ok(1) => {
                    log(&format!("{}: trotzdem: {name}", removal.label()));
                    operation.set_choice(0);
                    operation.reply(gio::MountOperationResult::Handled);
                }
                Err(err)
                    if err.matches(gtk4::DialogError::Cancelled)
                        || err.matches(gio::IOErrorEnum::Cancelled) =>
                {
                    log(&format!("Datenträger {name} ist nicht mehr belegt"));
                }
                other => {
                    if debugging() {
                        log(&format!("Frage zu {name}: {other:?}"));
                    }
                    operation.reply(gio::MountOperationResult::Aborted);
                }
            }
        });
        None
    });
    let aborted = asking.clone();
    operation.connect_aborted(move |_| {
        if let Some(cancellable) = aborted.borrow_mut().take() {
            cancellable.cancel();
        }
    });
    asking
}

/// QE-04: the settings in one log line (also read by the isolated test).
fn settings_text(s: &config::IconSettings) -> String {
    let on = |b: bool| if b { "an" } else { "aus" };
    format!(
        "Symbole: Rechner {} Persönlicher Ordner {} Papierkorb {} Größe {} | Laufwerke: Wechsel {} Optisch {} Netz {} Eingebaut {}",
        on(s.computer),
        on(s.home),
        on(s.trash),
        s.size,
        on(s.removable),
        on(s.optical),
        on(s.network),
        on(s.internal)
    )
}

fn log_volume(item: &Item) {
    if let Kind::Volume(volume) = &item.kind {
        log(&format!(
            "Datenträger da: {} ({}, {}, {})",
            item.name,
            volume.picture.name(),
            match (&volume.uri, volume.removal) {
                (None, _) => "nicht eingehängt",
                (Some(_), Some(removal)) => removal.label(),
                (Some(_), None) => "nicht aushängbar",
            },
            volume.uri.as_deref().unwrap_or(&volume.slot)
        ));
    }
}

/// QD-112: where the desktop waits for its missing directory.
fn log_waiting(dir: &Path) {
    if let Some(point) = ueberwachung::waiting_point(dir, Path::is_dir) {
        log(&format!(
            "Arbeitsflächen-Ordner fehlt, warte in {}",
            point.display()
        ));
    }
}

/// QE-03: the icons of the volumes that are mounted now; QE-04: of the
/// types `desktop.ini` `[Volumes]` asks for; 0.3.0 (E3): and of removable
/// media that are not mounted.
fn volume_items(monitor: &gio::VolumeMonitor, settings: &config::IconSettings) -> Vec<Item> {
    volumes::listed(monitor, settings)
        .into_iter()
        .map(|(name, volume)| Item::new(Kind::Volume(volume), &name))
        .collect()
}

/// Questions while mounting or taking a volume away: GTK's mount operation
/// (passwords) with the desktop's own German "still in use" question.
struct Rueckfrage(gio::MountOperation);

impl laufwerke::Rueckfrage for Rueckfrage {
    fn mount_operation(&self) -> Option<gio::MountOperation> {
        Some(self.0.clone())
    }
}

/// E3: mounts the medium of icon `name`; `open`: then opens it in the file
/// manager (a double click), as GNOME 2 did.
fn mount_volume(name: &str, volume: &Volume, window: &ApplicationWindow, open: bool) {
    let Some(gio_volume) = volumes::find_volume(&gio::VolumeMonitor::get(), &volume.slot) else {
        log(&format!("{name}: Datenträger nicht mehr da"));
        return;
    };
    let operation = gtk4::MountOperation::new(Some(window));
    let context = WidgetExt::display(window).app_launch_context();
    let name = name.to_owned();
    log(&format!("Einhängen: {name}"));
    glib::MainContext::default().spawn_local(async move {
        match laufwerke::einhaengen(&gio_volume, &Rueckfrage(operation.upcast())).await {
            Ok(mount) => {
                let root = mount.root().uri().to_string();
                log(&format!("eingehängt: {name} ({root})"));
                if open {
                    crate::portal::for_children(&context);
                    open_location(&name, &root, &context);
                }
            }
            Err(err) => {
                match laufwerke::mount_failure(err.kind::<gio::IOErrorEnum>(), &name, err.message())
                {
                    None => log(&format!("Einhängen abgebrochen: {name} ({err})")),
                    Some((title, detail)) => {
                        log(&format!("Einhängen fehlgeschlagen: {name}: {err}"));
                        show_error(&title, &detail);
                    }
                }
            }
        }
    });
}

/// E3: a medium inserted while the desktop runs is mounted when GIO wants
/// it (`should_automount`), as GNOME 2's desktop did; nothing opens.
fn automount(volume: &gio::Volume, window: &ApplicationWindow) {
    let facts = laufwerke::volume_facts(volume);
    let medium = matches!(
        laufwerke::classify_unmounted(&facts),
        Some(volumes::VolumeType::Removable | volumes::VolumeType::Optical)
    );
    if !medium || !volume.should_automount() || !volume.can_mount() || volume.get_mount().is_some()
    {
        return;
    }
    let operation = gtk4::MountOperation::new(Some(window));
    let volume = volume.clone();
    let name = facts.name;
    log(&format!("Einhängen (automatisch): {name}"));
    glib::MainContext::default().spawn_local(async move {
        match laufwerke::einhaengen(&volume, &Rueckfrage(operation.upcast())).await {
            Ok(mount) => log(&format!("eingehängt: {name} ({})", mount.root().uri())),
            Err(err) => log(&format!("automatisch nicht eingehängt: {name}: {err}")),
        }
    });
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

fn icon_widget(item: &Item, trash_full: bool, icon_px: i32, cell: (i32, i32)) -> gtk4::Box {
    let tile = gtk4::Box::new(Orientation::Vertical, 2);
    tile.add_css_class("desktop-icon");
    // HMP-10b: the tile spans the whole cell width (cells never overlap,
    // every grid keeps at least a cell between columns). With 6 px less,
    // "Persönlicher" (89 px in DejaVu/Adwaita Sans 11) missed the 88 px
    // left for the text by one pixel and was split ("Persönliche" / "r").
    // QD-091: the cell follows the font (`cell_for`).
    tile.set_size_request(cell.0, cell.1 - TILE_GAP);
    // QD-106: without GTK's preloading in the thread pool (0.3.0)
    let image = symbole::image(&icon_for(item, trash_full), icon_px);
    image.set_halign(Align::Center);
    let label = Label::new(Some(&item.name));
    label.set_wrap(true);
    label.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
    // QD-091: the natural width is the unwrapped text, so a long label takes
    // the whole cell and wraps there. With `Word` GTK sized a wrapped label
    // to the narrowest width with the same number of lines; with an
    // ellipsized second line that split words ("Gemeinsam" / "e …") and cut
    // names that fit ("user auf" / "nas.exam…").
    label.set_natural_wrap_mode(gtk4::NaturalWrapMode::None);
    label.set_lines(2);
    label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    label.set_justify(gtk4::Justification::Center);
    // GNOME 2 broke desktop labels between words ("Persönlicher" /
    // "Ordner") and only split over-long words, never with a hyphen.
    label.set_max_width_chars(LINE_CHARS);
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
    crate::portal::for_children(&context);
    let with_files = |uri: &str| match files_app() {
        Some(app) => app.launch_uris(&[uri], Some(&context)),
        None => Err(glib::Error::new(
            gio::IOErrorEnum::NotFound,
            "kein Dateimanager",
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
            if oeffnen::is_trusted(path) {
                launch_starter(path, &context)
            } else {
                ask_trust(item, path.clone(), context.clone());
                return;
            }
        }
        Kind::File(path) => match open_file(path, &context) {
            Err(OpenFailure::NoProgram(content_type)) => {
                log(&format!(
                    "{} nicht geöffnet: kein Programm für {content_type}",
                    item.name
                ));
                no_program(item, path, &content_type, context.clone());
                return;
            }
            Err(OpenFailure::Gio(err)) => Err(err),
            Ok(()) => Ok(()),
        },
        Kind::Folder(path) => {
            gio::AppInfo::launch_default_for_uri(&gio::File::for_path(path).uri(), Some(&context))
        }
        // QE-03: in Querencia Files, as GNOME 2 opened volumes in Nautilus;
        // E3: a medium that is not mounted is mounted first
        Kind::Volume(volume) => match &volume.uri {
            Some(uri) => {
                open_location(&item.name, uri, &context);
                return;
            }
            None => {
                mount_volume(&item.name, volume, window, true);
                return;
            }
        },
    };
    match result {
        Ok(()) => log(&format!("geöffnet: {}", item.name)),
        Err(err) => log(&format!("{} nicht geöffnet: {err}", item.name)),
    }
}

/// A volume's root in the file manager (else the default program).
fn open_location(name: &str, uri: &str, context: &gdk::AppLaunchContext) {
    let result = match files_app() {
        Some(app) => app.launch_uris(&[uri], Some(context)),
        None => gio::AppInfo::launch_default_for_uri(uri, Some(context)),
    };
    match result {
        Ok(()) => log(&format!("geöffnet: {name}")),
        Err(err) => log(&format!("{name} nicht geöffnet: {err}")),
    }
}

/// Why `open_file` started nothing.
enum OpenFailure {
    /// QD-102: neither the file's type nor the type its name suggests has a
    /// program; the file's type.
    NoProgram(String),
    Gio(glib::Error),
}

/// Desktop 0.2.2: a file opens with the default program for its type, as
/// before. When that type has none (an empty file is
/// `application/x-zerosize`), it opens with the default program for the
/// type its name suggests, as Nautilus does. QD-102: without one nothing is
/// started and the caller shows a notice (`no_program`). 0.3.0: decided by
/// `oeffnen::plan_open`; an executable file opens like any other (the
/// question "Ausführen oder anzeigen?" of Nautilus 2.30 is not built yet).
fn open_file(path: &Path, context: &gdk::AppLaunchContext) -> Result<(), OpenFailure> {
    let file = gio::File::for_path(path);
    let uri = file.uri();
    let info = file
        .query_info(
            oeffnen::OPEN_ATTRIBUTES,
            gio::FileQueryInfoFlags::NONE,
            gio::Cancellable::NONE,
        )
        .map_err(OpenFailure::Gio)?;
    let content_type = info
        .content_type()
        .map(|t| t.to_string())
        .unwrap_or_else(|| "application/octet-stream".to_owned());
    let by_name = |app: &gio::AppInfo, t: &str| {
        log(&format!(
            "{}: {content_type} ohne Programm, öffne als {t} mit {}",
            path.display(),
            app.id().unwrap_or_default()
        ));
        app.launch_uris(&[uri.as_str()], Some(context))
            .map_err(OpenFailure::Gio)
    };
    match oeffnen::plan_open(&file, &info) {
        oeffnen::OpenPlan::ByName(t, app) => by_name(&app, &t),
        oeffnen::OpenPlan::NoProgram(t) => Err(OpenFailure::NoProgram(t)),
        oeffnen::OpenPlan::Executable { .. } => {
            match gio::AppInfo::default_for_type(&content_type, false) {
                Some(_) => gio::AppInfo::launch_default_for_uri(&uri, Some(context))
                    .map_err(OpenFailure::Gio),
                None => match path
                    .file_name()
                    .and_then(|n| typ::type_from_name(&n.to_string_lossy()))
                    .and_then(|t| gio::AppInfo::default_for_type(&t, false).map(|a| (t, a)))
                {
                    Some((t, app)) => by_name(&app, &t),
                    None => Err(OpenFailure::NoProgram(content_type.clone())),
                },
            }
        }
        _ => gio::AppInfo::launch_default_for_uri(&uri, Some(context)).map_err(OpenFailure::Gio),
    }
}

/// QD-102: no program opens `path`: says so with the file's name and type
/// (as Nautilus 2 did) and offers GTK's program choice.
fn no_program(item: &Item, path: &Path, content_type: &str, context: gdk::AppLaunchContext) {
    let description = gio::content_type_get_description(content_type);
    let dialog = gtk4::AlertDialog::builder()
        .message(gettext_f(
            "“{name}” cannot be opened.",
            &[("name", &item.name)],
        ))
        .detail(gettext_f(
            "There is no application installed for “{type}” files ({mime}).",
            &[("type", &description), ("mime", content_type)],
        ))
        .buttons([gettext("Close"), gettext("Open With Other Application…")])
        .cancel_button(0)
        .default_button(1)
        .modal(false)
        .build();
    log(&format!("Hinweis: kein Programm für {}", item.name));
    let item = item.clone();
    let path = path.to_path_buf();
    // No parent: the desktop is a layer surface, not a toplevel window.
    dialog.choose(
        None::<&gtk4::Window>,
        gio::Cancellable::NONE,
        move |answer| {
            if answer == Ok(1) {
                let uri = gio::File::for_path(&path).uri().to_string();
                transfer::choose_app(&item, &path, uri, context);
            } else {
                log(&format!("Hinweis ohne Programm geschlossen: {}", item.name));
            }
        },
    );
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

/// Nautilus 2.30's question for an untrusted starter.
fn ask_trust(item: &Item, path: PathBuf, context: gdk::AppLaunchContext) {
    let dialog = gtk4::AlertDialog::builder()
        .message(gettext("Untrusted application launcher"))
        .detail(gettext_f(
            "The application launcher “{name}” has not been marked as trusted. \
             If you do not know the source of this file, launching it may be unsafe.",
            &[("name", &item.name)],
        ))
        .buttons([
            gettext("Cancel"),
            gettext("Mark as Trusted"),
            gettext("Launch Anyway"),
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
                match oeffnen::mark_trusted(&path) {
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

/// "Leere Datei" (`template` empty) or a copy of a template in `dir`, as a
/// job that picks a free name ("Neues Dokument 2") and never replaces.
fn new_document(dir: PathBuf, template: &str) -> transfer::Job {
    let op = if template.is_empty() {
        Op::NewFile {
            name: gettext("New Document"),
            template: None,
        }
    } else {
        let source = gio::File::for_path(template);
        Op::NewFile {
            name: source
                .basename()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| gettext("Template")),
            template: Some(source.uri().to_string()),
        }
    };
    transfer::Job::new(op, Vec::new(), Some(dir))
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
    crate::portal::launcher_for_children(&launcher);
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
    filter.set_name(Some(&gettext("Images")));
    filter.add_mime_type("image/*");
    let filters = gio::ListStore::new::<gtk4::FileFilter>();
    filters.append(&filter);
    let dialog = gtk4::FileDialog::builder()
        .title(gettext("Change Desktop Background"))
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
                    // QE-04: only [Background]; [Icons]/[Volumes] stay
                    let new =
                        config::replace_group(&text, "Background", &config::render(&background));
                    std::fs::write(&temporary, new)?;
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

    #[test]
    fn cells_follow_the_label_font() {
        // DejaVu Sans 11 and 12 at 96 dpi (Pango's approximate character
        // width, height of two lines)
        let eleven = LabelFont::REFERENCE;
        let twelve = LabelFont {
            char_width: 8.35,
            two_lines: 38,
        };
        // 14 characters and the padding; icon, two lines, padding and gap
        assert_eq!(cell_for(48, eleven), (116, 96));
        assert_eq!(cell_for(48, twelve), (125, 98));
        // the drawn tile of two lines (2 + 48 + 2 + 38 + 2 = 92) fits with
        // a gap: neighbouring tiles never touch (the live finding: 92 in 88)
        assert!(cell_for(48, twelve).1 >= 92 + TILE_GAP);
        // larger icons keep their wider cells, smaller fonts never shrink
        // a cell below the QE-04 sizes
        assert_eq!(cell_for(96, eleven).0, 130);
        let tiny = LabelFont {
            char_width: 4.0,
            two_lines: 20,
        };
        assert_eq!(cell_for(32, tiny), (100, 72));
        assert_eq!(cell_for(72, tiny), (110, 112));
    }
}
