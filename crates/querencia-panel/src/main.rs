mod about;
mod add_dialog;
mod app_menu;
mod applets;
mod apps;
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
// The three main menus (popover class `querencia-menu`, app_menu.rs) look
// like GNOME 2 menus instead of GTK 4's: square corners, compact rows
// (22 px instead of 30 px, no rounding), little padding, narrow separators,
// and the active row, by pointer or keyboard (`:selected`), in the theme's
// selection colours like a GTK 2 menu item. Only sizes and colours change;
// the PopoverMenu itself, its keyboard handling and submenus stay GTK's.
const PANEL_CSS: &str = "
.querencia-panel menubutton > button,
.querencia-panel button.window-button,
.querencia-panel button.workspace-button { min-height: 0; padding-top: 0; padding-bottom: 0; }
.querencia-panel button.window-button.active,
.querencia-panel button.workspace-button.active { background-color: alpha(currentColor, 0.15); font-weight: bold; }
popover.menu.querencia-menu > contents { padding: 3px 0; border-radius: 0; }
popover.menu.querencia-menu modelbutton { min-height: 22px; padding: 0 10px; border-radius: 0; }
popover.menu.querencia-menu modelbutton:selected { background-color: @theme_selected_bg_color; color: @theme_selected_fg_color; }
popover.menu.querencia-menu separator { margin: 3px 0; }
popover.menu.querencia-menu label.title { padding: 2px 24px; }
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

fn build_panels(app: &Application) {
    // A second launch only re-activates the running instance; never stack panels.
    if !app.windows().is_empty() {
        return;
    }

    use_classic_icon_theme();
    let css = gtk4::CssProvider::new();
    css.load_from_data(PANEL_CSS);
    gtk4::style_context_add_provider_for_display(
        &gtk4::gdk::Display::default().expect("GTK is initialised, so a display exists"),
        &css,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

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

    #[test]
    fn menu_style_is_scoped_to_the_panel_menus() {
        // the classic menu look must not leak into the dialogs or other popovers
        for line in PANEL_CSS.lines().filter(|l| l.contains("modelbutton")) {
            assert!(line.contains("popover.menu.querencia-menu"), "{line}");
        }
        assert!(PANEL_CSS.contains("modelbutton:selected"));
        assert!(PANEL_CSS.contains("@theme_selected_bg_color"));
    }
}
