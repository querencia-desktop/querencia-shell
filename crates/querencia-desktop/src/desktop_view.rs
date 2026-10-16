//! The icon layer of the desktop (HMP-09).
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
//! - The menus are the panel's classic GNOME 2 menus (`classic_menu.rs`,
//!   shared source) on an invisible menu button; the menu points at the click.
//! - Only entries that work are shown: "Ordner anlegen", "Dokument anlegen"
//!   ("Leere Datei" and the real files of the XDG templates directory),
//!   "Nach Namen aufräumen" and "Hintergrund der Arbeitsfläche ändern …"
//!   (writes `desktop.ini`, HMP-08 applies it). Not yet: "Starter
//!   anlegen", "Anordnung fixieren" (needs drag and drop), "Einfügen".
//! - New folders and documents are created with GIO calls that fail on an
//!   existing name, after [`icons::unique_name`] picked a free one: nothing
//!   is ever overwritten. The desktop directory itself is created only by
//!   such a user action.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use gio_unix::DesktopAppInfo;
use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, Fixed, GestureClick, Image, Label, MenuButton,
    Orientation, gdk, gio, glib,
};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

use crate::classic_menu;
use crate::config;
use crate::icons::{self, Item, Kind};

pub const FILES_APP: &str = "io.github.lgse.Strata.desktop";
pub const TERMINAL_APP: &str = "org.example.Querencia.Terminal.desktop";
// Grid cell: wide enough for "Persönlicher" on one line (GNOME 2 broke
// labels between words).
const CELL: (i32, i32) = (100, 88);
const MARGIN: i32 = 8;
const ICON_PX: i32 = 48;

/// What a `desktop.*` menu action does.
type ActionFn = Box<dyn Fn(&Rc<Icons>, Option<&glib::Variant>)>;

pub struct Icons {
    pub window: ApplicationWindow,
    fixed: Fixed,
    anchor: MenuButton,
    menu: gio::Menu,
    items: RefCell<Vec<Item>>,
    widgets: RefCell<Vec<gtk4::Box>>,
    selected: Cell<Option<usize>>,
    rows: Cell<usize>,
    watches: RefCell<Vec<gio::FileMonitor>>,
    /// Applies a new desktop.ini at once (main.rs `load`).
    background_changed: Rc<dyn Fn()>,
}

fn desktop_dir() -> Option<PathBuf> {
    icons::desktop_dir(
        glib::user_special_dir(glib::UserDirectory::Desktop),
        &glib::home_dir(),
    )
}

/// `$XDG_STATE_HOME/querencia/desktop-order` (default `~/.local/state`).
fn order_path() -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|dir| dir.is_absolute())
        .unwrap_or_else(|| glib::home_dir().join(".local/state"))
        .join("querencia")
        .join("desktop-order")
}

fn log(text: &str) {
    eprintln!("querencia-desktop: {text}");
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

        let icons = Rc::new(Self {
            window,
            fixed,
            anchor,
            menu,
            items: RefCell::default(),
            widgets: RefCell::default(),
            selected: Cell::new(None),
            rows: Cell::new(1),
            watches: RefCell::default(),
            background_changed,
        });
        icons.install_actions();
        icons.install_clicks();
        let weak = Rc::downgrade(&icons);
        icons.fixed.add_tick_callback(move |fixed, _| {
            if let Some(icons) = weak.upgrade() {
                let rows = icons::rows_for(fixed.height(), MARGIN, CELL.1);
                if fixed.height() > 0 && rows != icons.rows.get() {
                    icons.rows.set(rows);
                    icons.layout();
                }
            }
            glib::ControlFlow::Continue
        });
        icons.watch();
        icons.reload();
        icons.window.present();
        icons
    }

    /// Re-reads which icons there are and shows them in the saved order.
    pub fn reload(self: &Rc<Self>) {
        let saved = std::fs::read_to_string(order_path()).unwrap_or_default();
        let items = icons::ordered(&icons::parse_order(&saved), load_items());
        self.set_items(items);
    }

    fn set_items(self: &Rc<Self>, items: Vec<Item>) {
        let selected_key = self
            .selected
            .get()
            .and_then(|i| self.items.borrow().get(i).map(|item| item.key.clone()));
        for widget in self.widgets.borrow_mut().drain(..) {
            self.fixed.remove(&widget);
        }
        let widgets: Vec<gtk4::Box> = items.iter().map(icon_widget).collect();
        for widget in &widgets {
            self.fixed.put(widget, 0.0, 0.0);
        }
        self.selected
            .set(selected_key.and_then(|key| items.iter().position(|item| item.key == key)));
        let order: String = items.iter().map(|item| format!("{}\n", item.key)).collect();
        save_order(&order);
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

    fn layout(&self) {
        let rows = self.rows.get();
        for (index, widget) in self.widgets.borrow().iter().enumerate() {
            let (col, row) = icons::cell(index, rows);
            self.fixed.move_(
                widget,
                f64::from(MARGIN) + col as f64 * f64::from(CELL.0),
                f64::from(MARGIN) + row as f64 * f64::from(CELL.1),
            );
        }
    }

    fn show_selection(&self) {
        for (index, widget) in self.widgets.borrow().iter().enumerate() {
            if Some(index) == self.selected.get() {
                widget.add_css_class("selected");
            } else {
                widget.remove_css_class("selected");
            }
        }
    }

    fn hit(&self, x: f64, y: f64) -> Option<usize> {
        icons::hit(
            self.items.borrow().len(),
            self.rows.get(),
            (f64::from(MARGIN), f64::from(MARGIN)),
            (f64::from(CELL.0), f64::from(CELL.1)),
            (x, y),
        )
    }

    fn install_clicks(self: &Rc<Self>) {
        let click = GestureClick::new();
        click.set_button(0);
        let weak = Rc::downgrade(self);
        click.connect_pressed(move |gesture, n_press, x, y| {
            let Some(icons) = weak.upgrade() else { return };
            let index = icons.hit(x, y);
            icons.selected.set(index);
            icons.show_selection();
            match gesture.current_button() {
                gdk::BUTTON_PRIMARY if n_press == 2 => {
                    if let Some(item) = index.and_then(|i| icons.items.borrow().get(i).cloned()) {
                        open(&item, &icons.window);
                    }
                }
                gdk::BUTTON_SECONDARY => icons.popup_menu(index.is_some(), x, y),
                _ => {}
            }
        });
        self.fixed.add_controller(click);
    }

    fn popup_menu(&self, on_icon: bool, x: f64, y: f64) {
        self.menu.remove_all();
        if on_icon {
            self.menu.append(Some("Öffnen"), Some("desktop.open"));
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
            Box::new(|icons, _| {
                let items = icons::by_name(icons.items.borrow().clone());
                icons.set_items(items);
            }),
        );
        add(
            "change-background",
            None,
            Box::new(|icons, _| choose_background(icons.background_changed.clone())),
        );
        self.window.insert_action_group("desktop", Some(&group));
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
            match target.monitor_directory(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE) {
                Ok(monitor) => {
                    let weak = Rc::downgrade(self);
                    monitor.connect_changed(move |_, _, _, event| {
                        if matches!(
                            event,
                            gio::FileMonitorEvent::Created
                                | gio::FileMonitorEvent::Deleted
                                | gio::FileMonitorEvent::MovedIn
                                | gio::FileMonitorEvent::MovedOut
                                | gio::FileMonitorEvent::Renamed
                        ) && let Some(icons) = weak.upgrade()
                        {
                            icons.reload();
                        }
                    });
                    watches.push(monitor);
                }
                Err(err) => log(&format!("keine Überwachung von {}: {err}", target.uri())),
            }
        }
        *self.watches.borrow_mut() = watches;
    }
}

fn save_order(text: &str) {
    let path = order_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if std::fs::read_to_string(&path).ok().as_deref() != Some(text)
        && let Err(err) = std::fs::write(&path, text)
    {
        log(&format!("Reihenfolge nicht gespeichert: {err}"));
    }
}

/// Which icons there are right now (see `icons.rs`).
fn load_items() -> Vec<Item> {
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
    if let Some(entries) = desktop_dir().and_then(|dir| std::fs::read_dir(dir).ok()) {
        for entry in entries.filter_map(Result::ok) {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.starts_with('.') {
                items.push(Item::new(Kind::File(entry.path()), &name));
            }
        }
    }
    items
}

fn icon_for(item: &Item) -> gio::Icon {
    let themed = |name: &str| gio::ThemedIcon::new(name).upcast::<gio::Icon>();
    match &item.kind {
        Kind::Computer => themed("computer"),
        Kind::Home => themed("user-home"),
        Kind::Trash => {
            let full = gio::File::for_uri("trash:///")
                .query_info(
                    "trash::item-count",
                    gio::FileQueryInfoFlags::NONE,
                    gio::Cancellable::NONE,
                )
                .is_ok_and(|info| info.attribute_uint32("trash::item-count") > 0);
            themed(if full {
                "user-trash-full"
            } else {
                "user-trash"
            })
        }
        Kind::App(id) => DesktopAppInfo::new(id)
            .and_then(|app| app.icon())
            .unwrap_or_else(|| themed("application-x-executable")),
        Kind::File(path) => gio::File::for_path(path)
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

fn icon_widget(item: &Item) -> gtk4::Box {
    let tile = gtk4::Box::new(Orientation::Vertical, 2);
    tile.add_css_class("desktop-icon");
    tile.set_size_request(CELL.0 - 6, CELL.1 - 6);
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
        Kind::File(path) => {
            gio::AppInfo::launch_default_for_uri(&gio::File::for_path(path).uri(), Some(&context))
        }
    };
    match result {
        Ok(()) => log(&format!("geöffnet: {}", item.name)),
        Err(err) => log(&format!("{} nicht geöffnet: {err}", item.name)),
    }
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
