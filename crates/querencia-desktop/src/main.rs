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

// The panel's classic GNOME 2 menus (HMP-03/05), shared source: the desktop
// menu looks and behaves exactly like the panel menus.
#[allow(dead_code)]
#[path = "../../querencia-panel/src/classic_menu.rs"]
mod classic_menu;
mod config;
mod desktop_view;
mod icons;
mod layout;
mod rename;
mod store;
mod wallpaper;

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, gdk, gio, glib};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

use crate::config::Background;
use crate::desktop_view::Icons;
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
    /// The icon layer (HMP-09) on the first monitor.
    icons: Option<(gdk::Monitor, Rc<Icons>)>,
}

/// Desktop icons and menu are on unless `QUERENCIA_DESKTOP_ICONS=0`
/// (background only, the HMP-08 behaviour).
fn icons_enabled(value: Option<&str>) -> bool {
    value.map(str::trim) != Some("0")
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
        use_classic_icon_theme();
        install_style();
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
    // Icons on the first monitor, as GNOME 2 put them on the primary one.
    let first = state.windows.first().map(|(m, _, _)| m.clone());
    let current = state.icons.as_ref().map(|(m, _)| m.clone());
    if current != first || !icons_enabled(std::env::var("QUERENCIA_DESKTOP_ICONS").ok().as_deref())
    {
        if let Some((_, icons)) = state.icons.take() {
            icons.window.close();
        }
        if let Some(monitor) = first
            && icons_enabled(std::env::var("QUERENCIA_DESKTOP_ICONS").ok().as_deref())
        {
            eprintln!(
                "querencia-desktop: Symbole auf {}",
                monitor.connector().unwrap_or_default()
            );
            // "Hintergrund ändern" shows the new picture at once, without
            // waiting for the desktop.ini watch: GIO polls a watch whose
            // directory does not exist yet only every few seconds.
            let shared = desktop.clone();
            let reload: Rc<dyn Fn()> = Rc::new(move || load(&shared));
            let icons = Icons::create(app, &monitor, reload);
            state.icons = Some((monitor, icons));
        }
    }
}

/// GNOME 2's icon theme where the user kept GTK's built-in default, the
/// same rule as the panel (HMP-04) and Querencia Files (QF-0f).
const CLASSIC_ICON_THEME: &str = "gnome-human";

fn classic_icon_theme(current: &str, installed: impl Fn(&str) -> bool) -> Option<&'static str> {
    let built_in_default = matches!(current, "" | "Adwaita" | "hicolor");
    (built_in_default && installed(CLASSIC_ICON_THEME)).then_some(CLASSIC_ICON_THEME)
}

fn use_classic_icon_theme() {
    let Some(settings) = gtk4::Settings::default() else {
        return;
    };
    let current = settings
        .gtk_icon_theme_name()
        .map(|name| name.to_string())
        .unwrap_or_default();
    let mut dirs = vec![glib::user_data_dir()];
    dirs.extend(glib::system_data_dirs());
    let installed = |name: &str| {
        dirs.iter()
            .any(|dir| dir.join("icons").join(name).join("index.theme").is_file())
    };
    if let Some(theme) = classic_icon_theme(&current, installed) {
        settings.set_gtk_icon_theme_name(Some(theme));
    }
}

/// The panel's menu rules, line for line (a test compares them with the
/// panel's source), plus the icon layer's own rules, all scoped to the
/// desktop's windows and menus.
const MENU_CSS: &str = "\
popover.querencia-menu { padding: 0; }
popover.querencia-menu > contents { padding: 2px 0; border: 1px solid #c7c2be; border-radius: 0; box-shadow: none; background-color: #f5f4f2; color: #3c3c3c; }
popover.querencia-menu .qmenu { min-width: 150px; }
popover.querencia-menu .qmenu-item { min-height: 20px; padding: 1px 10px 1px 4px; border: 1px solid transparent; outline: none; }
popover.querencia-menu .qmenu-item .qmenu-arrow { color: #686463; }
popover.querencia-menu .qmenu-item.qmenu-active { background-color: #f07746; background-image: linear-gradient(to bottom, #f8875d, #ea703d); border-color: #b36c4e; box-shadow: inset 0 1px #fd9975; color: #ffffff; }
popover.querencia-menu .qmenu-item.qmenu-active .qmenu-arrow { color: #ffffff; }
popover.querencia-menu .qmenu-item:disabled { color: @insensitive_fg_color; }
popover.querencia-menu separator { margin: 3px 1px; min-height: 1px; background-color: #dcd8d3; }
";

const DESKTOP_CSS: &str = "\
window.querencia-desktop-icons, window.querencia-desktop-icons.background { background: none; background-color: transparent; }
window.querencia-desktop-icons menubutton.desktop-menu-anchor > button { min-width: 0; min-height: 0; padding: 0; margin: 0; border: none; background: none; box-shadow: none; }
window.querencia-desktop-icons box.desktop-icon { padding: 1px; }
window.querencia-desktop-icons box.desktop-icon > label { color: #ffffff; text-shadow: 1px 1px 2px alpha(#000000, 0.85); padding: 0 2px; border-radius: 3px; }
window.querencia-desktop-icons box.desktop-icon.selected > label { background-color: #f07746; background-image: linear-gradient(to bottom, #f8875d, #ea703d); text-shadow: none; }
window.querencia-desktop-icons box.desktop-icon.selected > image { opacity: 0.7; }
window.querencia-desktop-icons box.desktop-icon.dragging { opacity: 0.6; }
window.querencia-desktop-icons box.rubberband { background-color: alpha(#f07746, 0.18); border: 1px solid #ea703d; }
window.querencia-desktop-icons entry.desktop-rename { min-height: 0; padding: 1px 3px; border: 1px solid #000000; border-radius: 0; box-shadow: none; outline: none; background-color: #ffffff; background-image: none; color: #000000; }
window.querencia-desktop-icons entry.desktop-rename > text > selection { background-color: #f07746; color: #ffffff; }
";

fn install_style() {
    let Some(display) = gdk::Display::default() else {
        return;
    };
    let provider = gtk4::CssProvider::new();
    provider.load_from_data(&format!("{MENU_CSS}{DESKTOP_CSS}"));
    gtk4::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_desktop_menu_looks_exactly_like_the_panel_menus() {
        let panel = include_str!("../../querencia-panel/src/main.rs");
        let panel_rules: Vec<&str> = panel
            .lines()
            .filter(|l| l.starts_with("popover.querencia-menu"))
            .collect();
        let ours: Vec<&str> = MENU_CSS.lines().filter(|l| !l.is_empty()).collect();
        assert!(!panel_rules.is_empty());
        assert_eq!(ours, panel_rules);
    }

    #[test]
    fn desktop_rules_are_scoped_to_the_desktop() {
        for rule in DESKTOP_CSS.lines().filter(|l| !l.is_empty()) {
            assert!(rule.starts_with("window.querencia-desktop-icons"), "{rule}");
        }
    }

    #[test]
    fn icons_can_be_switched_off() {
        assert!(icons_enabled(None));
        assert!(icons_enabled(Some("1")));
        assert!(!icons_enabled(Some("0")));
        assert!(!icons_enabled(Some(" 0 ")));
    }

    #[test]
    fn the_classic_icon_theme_replaces_only_gtks_default() {
        let yes = |n: &str| n == CLASSIC_ICON_THEME;
        assert_eq!(classic_icon_theme("Adwaita", yes), Some("gnome-human"));
        assert_eq!(classic_icon_theme("", yes), Some("gnome-human"));
        assert_eq!(classic_icon_theme("Papirus", yes), None);
        assert_eq!(classic_icon_theme("Adwaita", |_| false), None);
    }
}
