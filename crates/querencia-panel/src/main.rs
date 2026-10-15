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
// like the GNOME 2 menus of Ubuntu with its theme Ambiance. The colours are
// measured in the GNOME 2 reference screenshot (MATE, Ambiance; HMP-05):
// - menu: background #f5f4f2, text #3c3c3c (Ambiance text_color), 1 px
//   light grey frame #c7c2be, no shadow, square, 1 px separators;
// - highlighted row (pointer or keyboard): vertical gradient #f8875d to
//   #ea703d with a 1 px light top edge #fd9975 and a 1 px frame #b36c4e,
//   white text and arrow. Its middle is Ambiance's selected_bg_color
//   #f07746, which HMP-03/04 used flat and which looked too heavy;
// - every row has a 1 px transparent frame, so highlighting moves nothing;
// - submenu arrows grey #686463 (white when highlighted);
// - the three main menu buttons stay neutral (no orange): dark text, on
//   hover or while their menu is open only a slightly darker panel colour
//   #ebe7e3 with a 1 px frame #c4bdb7, as the open "Applications" button in
//   the reference.
// MENU_TOP_INSET in classic_menu.rs must match the padding plus border of
// `contents`.
//
// Text colours are set, never inherited (HMP-04): a submenu's popover sits
// below the highlighted row in the CSS tree; an inherited colour made its
// text white on white. Rows that cannot be chosen are the theme's
// insensitive grey.
const PANEL_CSS: &str = "
.querencia-panel menubutton > button,
.querencia-panel button.window-button,
.querencia-panel button.workspace-button { min-height: 0; padding-top: 0; padding-bottom: 0; }
.querencia-panel button.window-button.active,
.querencia-panel button.workspace-button.active { background-color: alpha(currentColor, 0.15); font-weight: bold; }
.querencia-panel menubutton.querencia-main-menu > button { border-radius: 0; padding-left: 6px; padding-right: 6px; border: 1px solid transparent; background-image: none; box-shadow: none; outline: none; color: #3c3c3c; }
.querencia-panel menubutton.querencia-main-menu > button:hover,
.querencia-panel menubutton.querencia-main-menu > button:active,
.querencia-panel menubutton.querencia-main-menu > button:checked { background-color: #ebe7e3; background-image: none; border-color: #c4bdb7; color: #3c3c3c; box-shadow: none; }
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
        // Ambiance text_color on the Ambiance menu background (HMP-05)
        assert!(contents.contains(" color: #3c3c3c;"), "{contents}");
        assert!(
            contents.contains("background-color: #f5f4f2;"),
            "{contents}"
        );
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
        // Ubuntu (Ambiance) highlight as measured in the GNOME 2 reference
        // (HMP-05): a light-to-dark orange gradient with a light top edge
        // and a thin darker frame, not a flat block
        let active = PANEL_CSS
            .lines()
            .find(|l| l.contains(".qmenu-item.qmenu-active"))
            .expect("highlight rule");
        assert!(
            active.contains("linear-gradient(to bottom, #f8875d, #ea703d)"),
            "{active}"
        );
        assert!(active.contains("border-color: #b36c4e"), "{active}");
        assert!(active.contains("inset 0 1px #fd9975"), "{active}");
        assert!(PANEL_CSS.contains("border-radius: 0"));
        assert!(PANEL_CSS.contains("min-height: 20px"));
    }

    /// HMP-05: Anwendungen, Orte and System stay neutral, open or not; the
    /// orange belongs to the menu rows only.
    #[test]
    fn main_menu_buttons_never_turn_orange() {
        let rules: Vec<&str> = PANEL_CSS
            .lines()
            .filter(|l| l.contains("querencia-main-menu"))
            .collect();
        // no blue GTK focus ring after keyboard use either
        assert!(
            rules
                .iter()
                .any(|r| r.ends_with("> button { border-radius: 0; padding-left: 6px; padding-right: 6px; border: 1px solid transparent; background-image: none; box-shadow: none; outline: none; color: #3c3c3c; }")),
            "Grundregel ohne outline: none"
        );
        for state in [":hover", ":checked", ":active"] {
            assert!(
                rules.iter().any(|r| r.contains(state)),
                "Zustand {state} der Hauptmenüknöpfe nicht festgelegt"
            );
        }
        for rule in &rules {
            for orange in [
                "#f07746", "#f8875d", "#ea703d", "#fd9975", "#b36c4e", "orange",
            ] {
                assert!(!rule.contains(orange), "{rule}");
            }
            assert!(!rule.contains("#ffffff"), "keine weiße Schrift: {rule}");
            if rule.contains("background") {
                assert!(rule.contains("color: #3c3c3c"), "dunkle Schrift: {rule}");
            }
        }
    }
}
