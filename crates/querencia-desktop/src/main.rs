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
#[path = "../../querencia-panel/src/farbschema.rs"]
mod farbschema;
mod fileops;
// The icon theme rule of the panel (G2-AD), shared source.
#[path = "../../querencia-panel/src/icon_theme.rs"]
mod icon_theme;
mod icons;
mod layout;
// The settings portal check of the panel (QD-062), shared source.
#[path = "../../querencia-panel/src/portal.rs"]
mod portal;
mod rename;
mod store;
mod volumes;
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
/// (background only, the HMP-08 behaviour); the same rule for
/// `QUERENCIA_DESKTOP_VOLUMES` (QE-03).
fn icons_enabled(value: Option<&str>) -> bool {
    value.map(str::trim) != Some("0")
}

type Shared = Rc<RefCell<Desktop>>;

/// QE-01: what this desktop can show beyond HMP-10a, for Querencia
/// Einstellungen (`querencia-desktop --faehigkeiten`); QE-04: it reads
/// `desktop.ini` `[Volumes]` (volume types), `[Icons]` (the three special
/// icons) and the icon size.
const CAPABILITIES: &str =
    "type-color mode-centered mode-tiled volumes-types icons-visible icon-size";

fn main() -> glib::ExitCode {
    // answered before GTK or GApplication start: no display needed, no
    // running instance is activated
    if std::env::args().skip(1).any(|a| a == "--faehigkeiten") {
        println!("{CAPABILITIES}");
        return glib::ExitCode::SUCCESS;
    }
    // QE-04: which mounts the icon layer would show with the current
    // desktop.ini, read only (GIO's volume monitor, no window, no GTK)
    if std::env::args()
        .skip(1)
        .any(|a| a == "--datentraeger-zeigen")
    {
        let text = std::fs::read_to_string(config::config_path()).unwrap_or_default();
        let (settings, problems) = config::parse_icons(&text);
        for problem in problems {
            println!("desktop.ini: {problem}");
        }
        println!(
            "Laufwerke: Wechsel={} Optisch={} Netz={} Eingebaut={}",
            settings.removable, settings.optical, settings.network, settings.internal
        );
        for mount in gio::VolumeMonitor::get().mounts() {
            let facts = volumes::facts(&mount);
            println!(
                "{}\t{}\t{}\t{}",
                facts.name,
                facts.root_uri,
                volumes::classify(&facts).map_or("nie", volumes::VolumeType::name),
                if volumes::shown(&facts, &settings) {
                    "Symbol"
                } else {
                    "kein Symbol"
                }
            );
        }
        return glib::ExitCode::SUCCESS;
    }
    // before GTK and any thread (QD-062): no long wait for a stale portal
    let portal_mode = portal::prepare("querencia-desktop");
    if let Err(err) = gtk4::init() {
        eprintln!("querencia-desktop: GTK konnte nicht initialisiert werden: {err}");
        return glib::ExitCode::FAILURE;
    }
    portal::follow_gsettings("querencia-desktop", portal_mode);
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
        icon_theme::follow("querencia-desktop");
        install_style();
        farbschema::startfehler_beheben();
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
    // QE-01: Type=color loads no image at all, not even the built-in one
    let texture = match settings.kind {
        config::Kind::Color => None,
        config::Kind::Image => settings
            .file
            .as_ref()
            .and_then(|file| read_texture(file))
            .or_else(built_in_texture),
    };
    if settings.kind == config::Kind::Color {
        let (r, g, b) = settings.color;
        eprintln!("querencia-desktop: Hintergrund nur Farbe #{r:02x}{g:02x}{b:02x} (Type=color)");
    } else {
        eprintln!(
            "querencia-desktop: Hintergrund {}, Modus {}{}",
            settings.file.as_ref().map_or_else(
                || "eingebautes Bild".to_owned(),
                |f| f.display().to_string()
            ),
            settings.mode.name(),
            if texture.is_none() {
                " – kein Bild lesbar, nur Farbe"
            } else {
                ""
            }
        );
    }
    // QE-04: [Icons]/[Volumes] go to the icon layer (applied only when changed)
    let (icon_settings, problems) = config::parse_icons(&text);
    for problem in problems {
        eprintln!("querencia-desktop: {}: {problem}", path.display());
    }
    let mut state = desktop.borrow_mut();
    state.settings = settings;
    state.texture = texture;
    for (_, _, wallpaper) in &state.windows {
        wallpaper.show(&state.settings, state.texture.as_ref());
    }
    let icons = state.icons.as_ref().map(|(_, icons)| icons.clone());
    drop(state);
    if let Some(icons) = icons {
        icons.apply_settings(icon_settings);
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
    let path = config::built_in_candidates(&data_dirs, &source_tree)
        .into_iter()
        .find(|path| path.is_file())?;
    eprintln!("querencia-desktop: eingebautes Bild {}", path.display());
    read_texture(&path)
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

/// Desktop 0.2.1: icon labels sit on a rounded dark plate (55 % black), so
/// white text keeps 4.5:1 on any picture, the light ones of Hintergründe
/// 0.2 included; a selected label has the orange plate instead.
const DESKTOP_CSS: &str = "\
window.querencia-desktop-icons, window.querencia-desktop-icons.background { background: none; background-color: transparent; }
window.querencia-desktop-icons menubutton.desktop-menu-anchor > button { min-width: 0; min-height: 0; padding: 0; margin: 0; border: none; background: none; box-shadow: none; }
window.querencia-desktop-icons box.desktop-icon { padding: 1px; }
window.querencia-desktop-icons box.desktop-icon > label { color: #ffffff; text-shadow: 1px 1px 2px #000000; background-color: alpha(#000000, 0.55); padding: 1px 3px; border-radius: 4px; }
window.querencia-desktop-icons box.desktop-icon.selected > label { background-color: #f07746; background-image: linear-gradient(to bottom, #f8875d, #ea703d); text-shadow: none; }
window.querencia-desktop-icons box.desktop-icon.selected > image { opacity: 0.7; }
window.querencia-desktop-icons box.desktop-icon.dragging { opacity: 0.6; }
window.querencia-desktop-icons box.desktop-icon.cut > image { opacity: 0.45; }
window.querencia-desktop-icons box.desktop-icon.drop-target > label { background-color: #f07746; background-image: linear-gradient(to bottom, #f8875d, #ea703d); text-shadow: none; }
window.querencia-desktop-icons box.desktop-icon.drop-target > image { opacity: 0.7; }
window.querencia-desktop-icons box.rubberband { background-color: alpha(#f07746, 0.18); border: 1px solid #ea703d; }
window.querencia-desktop-icons entry.desktop-rename { min-height: 0; padding: 1px 3px; border: 1px solid #000000; border-radius: 0; box-shadow: none; outline: none; background-color: #ffffff; background-image: none; color: #000000; }
window.querencia-desktop-icons entry.desktop-rename > text > selection { background-color: #f07746; color: #ffffff; }
";

/// QD-081 (CSS-Welle dunkel): the dark menu rules of the panel, line for
/// line (a test compares them), and a dark rename field (Thema 0.2 view
/// colours). They apply only while `farbschema::dunkel` holds; the icon
/// labels keep their dark plate (Desktop 0.2.1), the same light and dark.
const DUNKEL_CSS: &str = "\
@media (prefers-color-scheme: dark) {
  popover.querencia-menu > contents { border-color: #2b2a27; background-color: #353430; color: #dfdbd2; }
  popover.querencia-menu .qmenu-item .qmenu-arrow { color: #a5a09a; }
  popover.querencia-menu separator { background-color: #2b2a27; }
  window.querencia-desktop-icons entry.desktop-rename { background-color: #2b2a27; color: #f2f1f0; }
}
";

fn install_style() {
    let Some(display) = gdk::Display::default() else {
        return;
    };
    let provider = gtk4::CssProvider::new();
    // QD-081: before loading, so the dark rules apply from the start
    farbschema::folgen(&provider, "querencia-desktop");
    provider.load_from_data(&format!("{MENU_CSS}{DESKTOP_CSS}{DUNKEL_CSS}"));
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

    /// QD-081: the dark menu rules are the panel's, line for line; the
    /// dark block is the only `@media` one and scoped like the light rules.
    #[test]
    fn the_dark_desktop_menu_looks_exactly_like_the_dark_panel_menus() {
        let panel = include_str!("../../querencia-panel/src/main.rs");
        let panel_rules: Vec<&str> = panel
            .lines()
            .filter(|l| l.starts_with("  popover.querencia-menu"))
            .collect();
        let ours: Vec<&str> = DUNKEL_CSS
            .lines()
            .filter(|l| l.starts_with("  popover.querencia-menu"))
            .collect();
        assert_eq!(ours.len(), 3);
        assert_eq!(ours, panel_rules);
        assert!(DUNKEL_CSS.starts_with("@media (prefers-color-scheme: dark) {\n"));
        assert!(DUNKEL_CSS.ends_with("\n}\n"));
        assert!(!MENU_CSS.contains("@media") && !DESKTOP_CSS.contains("@media"));
        for rule in DUNKEL_CSS.lines().filter(|l| l.starts_with("  ")) {
            let rule = rule.trim_start();
            assert!(
                rule.starts_with("popover.querencia-menu")
                    || rule.starts_with("window.querencia-desktop-icons"),
                "{rule}"
            );
            assert!(
                !rule.contains("#f07746") && !rule.contains("#ffffff"),
                "{rule}"
            );
        }
    }

    #[test]
    fn desktop_rules_are_scoped_to_the_desktop() {
        for rule in DESKTOP_CSS.lines().filter(|l| !l.is_empty()) {
            assert!(rule.starts_with("window.querencia-desktop-icons"), "{rule}");
        }
    }

    /// Desktop 0.2.1: white label text on the dark plate stays at 4.5:1 or
    /// more even over pure white (the brightest any picture can be), and
    /// the plate looks the same light and dark (no label rule in the dark
    /// block); a selected label trades the plate for the orange one.
    #[test]
    fn the_label_plate_keeps_white_text_readable_on_any_picture() {
        let rule = DESKTOP_CSS
            .lines()
            .find(|l| l.contains("box.desktop-icon > label"))
            .expect("label rule");
        let alpha: f64 = rule
            .split("background-color: alpha(#000000, ")
            .nth(1)
            .and_then(|rest| rest.split(')').next())
            .and_then(|a| a.parse().ok())
            .expect("plate alpha");
        let luminance = |channel: f64| {
            let c = channel / 255.0;
            let linear = if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            };
            0.2126 * linear + 0.7152 * linear + 0.0722 * linear
        };
        // white picture under the plate, rounded up as an 8-bit pixel
        let under = (255.0 * (1.0 - alpha)).ceil();
        let contrast = 1.05 / (luminance(under) + 0.05);
        assert!(contrast >= 4.5, "{contrast:.2}:1 bei Deckkraft {alpha}");
        assert!(rule.contains("color: #ffffff;") && rule.contains("border-radius"));
        let selected = DESKTOP_CSS
            .lines()
            .find(|l| l.contains("box.desktop-icon.selected > label"))
            .expect("selected rule");
        assert!(selected.contains("background-color: #f07746;"));
        assert!(!DUNKEL_CSS.contains("box.desktop-icon"));
    }

    #[test]
    fn icons_can_be_switched_off() {
        assert!(icons_enabled(None));
        assert!(icons_enabled(Some("1")));
        assert!(!icons_enabled(Some("0")));
        assert!(!icons_enabled(Some(" 0 ")));
    }
}
