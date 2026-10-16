//! Querencia Desktop's desktop (HMP-08): the background of every monitor as
//! a layer-shell surface on the **background** layer, below all windows and
//! the panels (which are on the top layer). One surface per monitor,
//! following monitors as they come and go.
//!
//! - The image and how it fills the screen come from
//!   `~/.config/querencia/desktop.ini` (`config.rs`); a change of that file
//!   applies at once (file monitor), so a later "Hintergrund der
//!   Arbeitsfläche ändern" only writes the file.
//! - Without a file the built-in "Querencia Blau" is used (own image,
//!   `tools/hintergrund/erzeugen.py`).
//! - The surfaces take no keyboard and, in HMP-08, no pointer input either
//!   (empty input region): clicks on the desktop still reach labwc as before
//!   (its root menu). Desktop icons and the desktop menu (HMP-09) will take
//!   the pointer where they need it.
//! - Exclusive zone -1: the background ignores the panels' reserved space
//!   and covers the whole monitor.
//!
//! Only one instance per session (GApplication ID); a second start exits.

mod config;
mod wallpaper;

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, gdk, gio, glib};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

use crate::config::Background;
use crate::wallpaper::Wallpaper;

const APP_ID: &str = "org.querencia.Desktop";
const NAMESPACE: &str = "querencia-desktop";

/// What runs while the desktop is up.
#[derive(Default)]
struct Desktop {
    windows: Vec<(gdk::Monitor, ApplicationWindow, Wallpaper)>,
    settings: Background,
    texture: Option<gdk::Texture>,
    /// Keeps the config file watch alive.
    monitor: Option<gio::FileMonitor>,
}

type Shared = Rc<RefCell<Desktop>>;

fn main() -> glib::ExitCode {
    let app = Application::builder().application_id(APP_ID).build();
    let desktop: Shared = Rc::default();
    app.connect_activate(move |app| {
        if !desktop.borrow().windows.is_empty() {
            return;
        }
        if !gtk4_layer_shell::is_supported() {
            eprintln!("querencia-desktop: kein wlr-layer-shell, beendet");
            app.quit();
            return;
        }
        // keep running without visible windows while monitors change
        std::mem::forget(app.hold());
        load(&desktop);
        watch_config(&desktop);
        if let Some(display) = gdk::Display::default() {
            let monitors = display.monitors();
            let (app2, desktop2) = (app.clone(), desktop.clone());
            monitors.connect_items_changed(move |_, _, _, _| rebuild(&app2, &desktop2));
            rebuild(app, &desktop);
        }
    });
    app.run()
}

/// Reads the config and the image (the built-in one if none is set or it
/// cannot be read).
fn load(desktop: &Shared) {
    let path = config::config_path();
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let (settings, problems) = config::parse(&text);
    for problem in problems {
        eprintln!("querencia-desktop: {}: {problem}", path.display());
    }
    let texture = settings
        .file
        .as_ref()
        .and_then(|file| read_texture(file))
        .or_else(built_in_texture);
    eprintln!(
        "querencia-desktop: Hintergrund {}, Modus {}{}",
        settings
            .file
            .as_ref()
            .map_or_else(|| config::BUILT_IN.to_owned(), |f| f.display().to_string()),
        settings.mode.name(),
        if texture.is_none() {
            " – kein Bild lesbar, nur Farbe"
        } else {
            ""
        }
    );
    let mut desktop = desktop.borrow_mut();
    desktop.settings = settings;
    desktop.texture = texture;
    for (_, _, wallpaper) in &desktop.windows {
        wallpaper.show(&desktop.settings, desktop.texture.as_ref());
    }
}

fn read_texture(file: &std::path::Path) -> Option<gdk::Texture> {
    match gdk::Texture::from_filename(file) {
        Ok(texture) => Some(texture),
        Err(err) => {
            eprintln!(
                "querencia-desktop: Bild {} nicht lesbar: {err}",
                file.display()
            );
            None
        }
    }
}

fn built_in_texture() -> Option<gdk::Texture> {
    let mut data_dirs = vec![glib::user_data_dir()];
    data_dirs.extend(glib::system_data_dirs());
    let source_tree = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    config::built_in_candidates(&data_dirs, &source_tree)
        .into_iter()
        .find(|path| path.is_file())
        .and_then(|path| read_texture(&path))
}

/// Re-reads the config whenever `desktop.ini` changes.
fn watch_config(desktop: &Shared) {
    let file = gio::File::for_path(config::config_path());
    match file.monitor_file(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE) {
        Ok(monitor) => {
            let desktop2 = desktop.clone();
            monitor.connect_changed(move |_, _, _, event| {
                if matches!(
                    event,
                    gio::FileMonitorEvent::ChangesDoneHint
                        | gio::FileMonitorEvent::Created
                        | gio::FileMonitorEvent::Deleted
                ) {
                    load(&desktop2);
                }
            });
            desktop.borrow_mut().monitor = Some(monitor);
        }
        Err(err) => eprintln!("querencia-desktop: keine Überwachung von desktop.ini: {err}"),
    }
}

/// One background window per monitor; windows of vanished monitors close.
fn rebuild(app: &Application, desktop: &Shared) {
    let Some(display) = gdk::Display::default() else {
        return;
    };
    let monitors: Vec<gdk::Monitor> = display
        .monitors()
        .iter::<gdk::Monitor>()
        .filter_map(Result::ok)
        .collect();
    let mut state = desktop.borrow_mut();
    state.windows.retain(|(monitor, window, _)| {
        let keep = monitors.contains(monitor);
        if !keep {
            window.close();
        }
        keep
    });
    for monitor in monitors {
        if state.windows.iter().any(|(m, _, _)| *m == monitor) {
            continue;
        }
        let wallpaper = Wallpaper::default();
        wallpaper.show(&state.settings, state.texture.as_ref());
        let window = background_window(app, &monitor, &wallpaper);
        let geometry = monitor.geometry();
        eprintln!(
            "querencia-desktop: Arbeitsfläche auf {} ({}x{})",
            monitor.connector().unwrap_or_default(),
            geometry.width(),
            geometry.height()
        );
        state.windows.push((monitor, window, wallpaper));
    }
}

fn background_window(
    app: &Application,
    monitor: &gdk::Monitor,
    wallpaper: &Wallpaper,
) -> ApplicationWindow {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Arbeitsfläche")
        .child(wallpaper)
        .build();
    window.add_css_class("querencia-desktop");
    window.init_layer_shell();
    window.set_namespace(Some(NAMESPACE));
    window.set_layer(Layer::Background);
    window.set_monitor(Some(monitor));
    window.set_keyboard_mode(KeyboardMode::None);
    window.set_exclusive_zone(-1);
    for edge in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
        window.set_anchor(edge, true);
    }
    // No pointer input in HMP-08: the desktop stays labwc's (root menu).
    window.connect_realize(|window| {
        if let Some(surface) = window.surface() {
            surface.set_input_region(Some(&gtk4::cairo::Region::create()));
        }
    });
    window.present();
    window
}
