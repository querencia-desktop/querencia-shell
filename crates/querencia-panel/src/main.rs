mod about;
mod add_dialog;
mod app_menu;
mod applets;
mod apps;
mod classic_menu;
mod clock;
mod layout;
mod layout_editor;
mod layout_store;
mod panel;
mod panel_menu;
mod places_menu;
mod session;
mod system_menu;
mod window_list;
mod workspace_switcher;

use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Application, gio, glib};

use crate::layout_editor::Editor;

const APP_ID: &str = "org.example.Querencia.Panel";

// GTK's default button min-height plus padding is 34 px and would push a
// panel of layout::DEFAULT_HEIGHT past its height; panel buttons fill the
// bar height instead. A window or workspace button is highlighted while
// labwc reports it active.
//
// The classic menus of Anwendungen, Orte and System (classic_menu.rs) look
// like the GNOME 2 menus of Ubuntu 10.04: a square menu with a thin frame
// and no shadow, 22 px rows with a 16 px icon column, a thin line between
// sections, and the highlighted row, by pointer or keyboard, in the orange
// of Ubuntu 10.04's default theme Ambiance (selected_bg_color #f07746,
// white text). The button of the open menu is highlighted the same way, as
// a GNOME 2 menu bar item. MENU_TOP_INSET in classic_menu.rs must match
// the padding plus border of `contents`.
//
// Text colours are set, never inherited (HMP-04): normal rows in the
// theme's text colour on the menu background, the highlighted row white on
// orange, rows that cannot be chosen in the theme's insensitive grey. A
// submenu's popover sits below the highlighted row in the CSS tree; an
// inherited colour made its text white on white.
const PANEL_CSS: &str = "
.querencia-panel menubutton > button,
.querencia-panel button.window-button,
.querencia-panel button.workspace-button { min-height: 0; padding-top: 0; padding-bottom: 0; }
.querencia-panel button.window-button.active,
.querencia-panel button.workspace-button.active { background-color: alpha(currentColor, 0.15); font-weight: bold; }
.querencia-panel menubutton.querencia-main-menu > button { border-radius: 0; padding-left: 6px; padding-right: 6px; }
.querencia-panel menubutton.querencia-main-menu > button:checked { background-color: #f07746; background-image: none; color: #ffffff; box-shadow: none; }
popover.querencia-menu { padding: 0; }
popover.querencia-menu > contents { padding: 2px 0; border: 1px solid rgba(0, 0, 0, 0.35); border-radius: 0; box-shadow: none; background-color: @theme_bg_color; color: @theme_fg_color; }
popover.querencia-menu .qmenu { min-width: 150px; }
popover.querencia-menu .qmenu-item { min-height: 22px; padding: 1px 10px 1px 4px; outline: none; }
popover.querencia-menu .qmenu-item.qmenu-active { background-color: #f07746; color: #ffffff; }
popover.querencia-menu .qmenu-item:disabled { color: @insensitive_fg_color; }
popover.querencia-menu separator { margin: 3px 1px; min-height: 1px; background-color: rgba(0, 0, 0, 0.15); }
";

fn main() -> glib::ExitCode {
    if let Err(err) = gtk4::init() {
        eprintln!("querencia-panel: GTK konnte nicht initialisiert werden: {err}");
        return glib::ExitCode::FAILURE;
    }
    if !gtk4_layer_shell::is_supported() {
        eprintln!(
            "querencia-panel: Der Compositor bietet kein wlr-layer-shell an. \
             Ein normales Fenster wäre kein Panel, deshalb wird abgebrochen."
        );
        return glib::ExitCode::FAILURE;
    }

    let app = Application::builder().application_id(APP_ID).build();
    app.add_action_entries([gio::ActionEntry::builder("quit")
        .activate(|app: &Application, _, _| app.quit())
        .build()]);
    app.connect_activate(build_panels);
    app.run()
}

/// Icon theme of the GNOME 2 look, used by the panel process only.
const CLASSIC_ICON_THEME: &str = "gnome-human";

/// The icon theme the panel should switch to: GNOME Human if it is
/// installed and the current theme is only GTK's built-in default (no
/// theme chosen by the user). The setting lives in this process; no file
/// and no other application changes.
fn classic_icon_theme(current: &str, installed: impl Fn(&str) -> bool) -> Option<&'static str> {
    let built_in_default = matches!(current, "" | "Adwaita" | "hicolor");
    (built_in_default && installed(CLASSIC_ICON_THEME)).then_some(CLASSIC_ICON_THEME)
}

/// Whether one of `data_dirs` has `icons/<name>/index.theme`.
fn theme_installed(name: &str, data_dirs: &[std::path::PathBuf]) -> bool {
    data_dirs
        .iter()
        .any(|dir| dir.join("icons").join(name).join("index.theme").is_file())
}

/// Switches the panel to [`CLASSIC_ICON_THEME`] where [`classic_icon_theme`]
/// says so.
fn use_classic_icon_theme() {
    let Some(settings) = gtk4::Settings::default() else {
        return;
    };
    let current = settings
        .gtk_icon_theme_name()
        .map(|name| name.to_string())
        .unwrap_or_default();
    let mut data_dirs = vec![glib::user_data_dir()];
    data_dirs.extend(glib::system_data_dirs());
    match classic_icon_theme(&current, |name| theme_installed(name, &data_dirs)) {
        Some(theme) => {
            settings.set_gtk_icon_theme_name(Some(theme));
            eprintln!("querencia-panel: Symbolthema {theme} (statt „{current}“)");
        }
        None => eprintln!("querencia-panel: Symbolthema „{current}“ bleibt"),
    }
}

/// The panel's icon theme and style sheet for the whole display; also used
/// by the isolated GUI test (`classic_menu/gui_test.rs`).
fn install_panel_style() {
    use_classic_icon_theme();
    let css = gtk4::CssProvider::new();
    css.load_from_data(PANEL_CSS);
    gtk4::style_context_add_provider_for_display(
        &gtk4::gdk::Display::default().expect("GTK is initialised, so a display exists"),
        &css,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

fn build_panels(app: &Application) {
    // A second launch only re-activates the running instance; never stack panels.
    if !app.windows().is_empty() {
        return;
    }

    install_panel_style();

    // The panel never writes panel.ini on its own, only when the user
    // changes the layout ("Zum Panel hinzufügen"). Without the file, or if
    // it is unusable, the panels show the standard layout.
    let path = layout_store::config_path();
    let (editor, problems, source) = Editor::load(path.clone(), layout_editor::registry);
    for problem in &problems {
        eprintln!("querencia-panel: {}: {problem}", path.display());
    }
    let layout = editor.layout().clone();
    eprintln!(
        "querencia-panel: Layout {source}, {} Panel(s)",
        layout.panels.len()
    );
    let editor = Rc::new(RefCell::new(editor));
    for spec in &layout.panels {
        panel::build(app, spec, &editor).present();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gnome_human_replaces_only_the_built_in_default() {
        let installed = |name: &str| name == "gnome-human";
        for default in ["Adwaita", "hicolor", ""] {
            assert_eq!(
                classic_icon_theme(default, installed),
                Some("gnome-human"),
                "{default:?}"
            );
        }
        // a theme the user chose stays
        assert_eq!(classic_icon_theme("Humanity", installed), None);
        assert_eq!(classic_icon_theme("gnome-human", installed), None);
        // not installed: nothing changes
        assert_eq!(classic_icon_theme("Adwaita", |_| false), None);
    }

    #[test]
    fn icon_theme_is_installed_if_a_data_dir_has_its_index() {
        let root = std::env::temp_dir().join(format!("querencia-icons-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let with = root.join("mit");
        let without = root.join("ohne");
        std::fs::create_dir_all(with.join("icons/gnome-human")).unwrap();
        std::fs::write(with.join("icons/gnome-human/index.theme"), "[Icon Theme]\n").unwrap();
        std::fs::create_dir_all(without.join("icons/gnome-human")).unwrap();
        assert!(theme_installed(
            "gnome-human",
            &[without.clone(), with.clone()]
        ));
        assert!(!theme_installed(
            "gnome-human",
            std::slice::from_ref(&without)
        ));
        assert!(!theme_installed("humanity", &[with]));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// HMP-04: submenu text was white on white because the menu text colour
    /// was inherited from the highlighted parent row. Every menu rule that
    /// sets a background must also set the text colour.
    #[test]
    fn menu_text_colours_are_set_not_inherited() {
        let rule = |needle: &str| {
            PANEL_CSS
                .lines()
                .find(|l| l.starts_with(needle))
                .unwrap_or_else(|| panic!("Regel {needle} fehlt"))
        };
        let contents = rule("popover.querencia-menu > contents ");
        assert!(contents.contains(" color: @theme_fg_color;"), "{contents}");
        let disabled = rule("popover.querencia-menu .qmenu-item:disabled ");
        assert!(
            disabled.contains("color: @insensitive_fg_color;"),
            "{disabled}"
        );
        let active = rule("popover.querencia-menu .qmenu-item.qmenu-active ");
        assert!(active.contains("color: #ffffff;"), "{active}");
        for line in PANEL_CSS
            .lines()
            .filter(|l| l.starts_with("popover.querencia-menu"))
        {
            if line.contains("background-color") && !line.contains("separator") {
                assert!(
                    line.contains(" color: "),
                    "Hintergrund ohne Schriftfarbe: {line}"
                );
            }
            if line.contains("#ffffff") {
                assert!(
                    line.contains(".qmenu-active"),
                    "weiße Schrift außerhalb der Hervorhebung: {line}"
                );
            }
        }
    }

    #[test]
    fn menu_style_is_scoped_to_the_panel_and_its_menus() {
        // every rule belongs to the panel or to the classic menus; nothing
        // leaks into dialogs or other popovers
        for rule in PANEL_CSS.lines().filter(|l| l.contains('{')) {
            assert!(
                rule.starts_with(".querencia-panel") || rule.starts_with("popover.querencia-menu"),
                "{rule}"
            );
        }
        // Ubuntu 10.04 (Ambiance) highlight, square menus, compact rows
        let active = PANEL_CSS
            .lines()
            .find(|l| l.contains(".qmenu-item.qmenu-active"))
            .expect("highlight rule");
        // Ambiance selected_bg_color
        assert!(active.contains("#f07746"), "{active}");
        assert!(PANEL_CSS.contains("border-radius: 0"));
        assert!(PANEL_CSS.contains("min-height: 22px"));
        // the open main menu's button is highlighted like a menu bar item
        assert!(PANEL_CSS.contains("menubutton.querencia-main-menu > button:checked"));
    }
}
