mod about;
mod add_dialog;
mod app_chooser;
mod app_menu;
mod applet_move;
mod applets;
mod apps;
mod classic_menu;
mod clock;
mod fensterliste;
mod icon_reload;
mod icon_theme;
mod launcher;
mod layout;
mod layout_editor;
mod layout_store;
mod logo;
mod network_applet;
mod network_dialogs;
mod neuladen;
mod panel;
mod panel_menu;
mod places_menu;
mod session;
mod sitzung_start;
mod system_menu;
mod volume_applet;
mod window_icons;
mod window_list;
mod workspace_switcher;

use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Application, gio, glib};

use crate::layout_editor::Editor;

pub(crate) const APP_ID: &str = "org.example.Querencia.Panel";

// GTK's default button min-height plus padding is 34 px and would push a
// panel of layout::DEFAULT_HEIGHT past its height; panel buttons fill the
// bar height instead.
//
// Workspace switcher (HMP-11b, replaces the HMP-07 taupe look): the classic
// GNOME 2 pager in the light greys of the window theme Querencia-Klassisch.
// The pager sits in a sunken 1 px frame (dark #9e9993 top and left, light
// #fdfcfb bottom and right) on #d8d4d0; every cell is a small raised
// rectangle: light gradient #f4f2f0..#e4e1de with a white top-left and a
// #c6c1bb bottom-right edge, cells separated by 1 px #aaa6a2 (the theme's
// inactive frame colour). The active cell uses the Querencia orange of the
// menu highlight (#f8875d..#ea703d, edges #fd9975 and #b36c4e), so it
// stands out clearly. Square corners, no transitions (GTK would fade the
// colour on every switch).
//
// Window list (HMP-06), measured in the GNOME 2 reference (Ambiance): framed
// buttons, 1 px border #c6c1bb on top, #9e9993 below, light gradient; the
// active window's button looks pressed (darker gradient #dcd6d0..#ebe7e3,
// inset shadow); a minimized window's button is flat and its icon and
// title dimmed. Dark text always, no orange (the orange belongs to menus).
//
// Status applets (QD-045; volume G2-J, network G2-M and later ones, class
// panel::STATUS_CLASS): like a launcher a bare icon with the neutral
// highlight, greyed out (as a missing launcher) without their service. As
// in the GNOME 2 notification area they stand close: no padding, only the
// 1 px hover border, and panel::STATUS_SPACING (2 px) between two of them,
// so neighbouring icons are 4 px apart. One rule for all of them, so every
// status icon has the same inner border.
// The slider of its popup is filled in the orange of the menu highlight.
//
// Clock calendar (G2-AD): GTK's calendar in the popover; the selected day in
// the orange of the menu highlight with white text instead of the theme's
// blue, today in bold and, while another day is selected, framed in orange.
//
// Network applet (G2-M): the icon as the volume applet (neutral highlight,
// dimmed without any network data). Its left-click menu has the classic
// menu look (`querencia-menu`): rows highlighted on hover and keyboard
// focus in the menu orange, bold group titles, grey notes and second
// lines (addresses).
//
// Application launchers (HMP-11): only the icon, no frame, as in the GNOME 2
// panel; on hover and while pressed the neutral highlight of the main menu
// buttons (never orange); a launcher whose application is missing has a
// dimmed generic icon.
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
// - submenu arrows grey #686463 (white when highlighted); a shortcut
//   (G2-AK, "Bildschirm sperren") lighter grey #7a7671, white when
//   highlighted;
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
.querencia-panel button.window-button { min-height: 0; padding-top: 0; padding-bottom: 0; }
.querencia-panel box.workspace-pager { border: 1px solid #9e9993; border-right-color: #fdfcfb; border-bottom-color: #fdfcfb; border-radius: 0; background-color: #d8d4d0; }
.querencia-panel box.workspace-pager > button.workspace-cell { min-width: 0; min-height: 0; padding: 0; margin: 0; border: none; border-radius: 0; background-color: #eeece9; background-image: linear-gradient(to bottom, #f4f2f0, #e4e1de); box-shadow: inset 1px 1px #ffffff, inset -1px -1px #c6c1bb; outline: none; transition: none; }
.querencia-panel box.workspace-pager > button.workspace-cell + button.workspace-cell { border-left: 1px solid #aaa6a2; }
.querencia-panel box.workspace-pager > button.workspace-cell:hover { background-image: linear-gradient(to bottom, #fbfaf9, #ecebe8); }
.querencia-panel box.workspace-pager > button.workspace-cell.active { background-color: #f07746; background-image: linear-gradient(to bottom, #f8875d, #ea703d); box-shadow: inset 1px 1px #fd9975, inset -1px -1px #b36c4e; }
.querencia-panel box.workspace-pager > button.workspace-cell:disabled { opacity: 0.5; }
.querencia-panel windowlist button.window-button { margin: 2px 0; padding: 0 6px 0 4px; border-radius: 2px; border: 1px solid #b7b3b0; border-top-color: #c6c1bb; border-bottom-color: #9e9993; background-color: #f4f2f0; background-image: linear-gradient(to bottom, #fdfcfb, #ebe8e5); box-shadow: none; outline: none; color: #3c3c3c; font-weight: normal; }
.querencia-panel windowlist button.window-button:hover { background-image: linear-gradient(to bottom, #ffffff, #f1efec); }
.querencia-panel windowlist button.window-button.active { background-color: #e4dfd9; background-image: linear-gradient(to bottom, #dcd6d0, #ebe7e3); border-color: #9e9993; box-shadow: inset 0 1px 2px alpha(#000000, 0.15); }
.querencia-panel windowlist button.window-button.minimized { background-image: none; background-color: #f7f6f4; border-color: #cdc8c3; }
.querencia-panel windowlist button.window-button.minimized > box { opacity: 0.55; }
.querencia-panel button.querencia-launcher { min-width: 0; min-height: 0; padding: 0 2px; margin: 0; border-radius: 0; border: 1px solid transparent; background-image: none; box-shadow: none; outline: none; }
.querencia-panel button.querencia-launcher:hover { background-color: #ebe7e3; border-color: #c4bdb7; }
.querencia-panel button.querencia-launcher:active { background-color: #dcd6d0; border-color: #9e9993; }
.querencia-panel button.querencia-launcher.missing > image { opacity: 0.45; }
.querencia-panel button.querencia-status { min-width: 0; min-height: 0; padding: 0; margin: 0; border-radius: 0; border: 1px solid transparent; background-image: none; box-shadow: none; outline: none; }
.querencia-panel button.querencia-status:hover { background-color: #ebe7e3; border-color: #c4bdb7; }
.querencia-panel button.querencia-status:active { background-color: #dcd6d0; border-color: #9e9993; }
.querencia-panel button.querencia-status.unavailable > image { opacity: 0.45; }
.querencia-panel popover.querencia-volume-popup scale > trough > highlight { background-color: #f07746; background-image: linear-gradient(to bottom, #f8875d, #ea703d); border-color: #b36c4e; }
.querencia-panel popover.querencia-calendar calendar > grid > label.day-number:selected { background-color: #f07746; background-image: linear-gradient(to bottom, #f8875d, #ea703d); box-shadow: inset 0 0 0 1px #b36c4e; color: #ffffff; }
.querencia-panel popover.querencia-calendar calendar > grid > label.today { font-weight: bold; }
.querencia-panel popover.querencia-calendar calendar > grid > label.today:not(:selected) { box-shadow: inset 0 0 0 1px #f07746; }
.querencia-panel popover.querencia-network-menu .qnet { min-width: 230px; }
.querencia-panel popover.querencia-network-menu button.qnet-item { min-height: 20px; padding: 2px 10px 2px 4px; margin: 0; border: 1px solid transparent; border-radius: 0; background: none; box-shadow: none; outline: none; color: #3c3c3c; font-weight: normal; }
.querencia-panel popover.querencia-network-menu button.qnet-item:hover,
.querencia-panel popover.querencia-network-menu button.qnet-item.qmenu-active { background-color: #f07746; background-image: linear-gradient(to bottom, #f8875d, #ea703d); border-color: #b36c4e; box-shadow: inset 0 1px #fd9975; color: #ffffff; }
.querencia-panel popover.querencia-network-menu button.qnet-item:hover label.qnet-detail,
.querencia-panel popover.querencia-network-menu button.qnet-item.qmenu-active label.qnet-detail { color: #ffffff; }
.querencia-panel popover.querencia-network-menu button.qnet-item:disabled { color: @insensitive_fg_color; background: none; border-color: transparent; box-shadow: none; }
.querencia-panel popover.querencia-network-menu button.qnet-item.qnet-current { color: #3c3c3c; background: none; border-color: transparent; box-shadow: none; }
.querencia-panel popover.querencia-network-menu .qnet-icon-hell { opacity: 0; }
.querencia-panel popover.querencia-network-menu button.qnet-item:hover .qnet-icon-hell,
.querencia-panel popover.querencia-network-menu button.qnet-item.qmenu-active .qnet-icon-hell { opacity: 1; }
.querencia-panel popover.querencia-network-menu button.qnet-item:hover .qnet-icon-normal,
.querencia-panel popover.querencia-network-menu button.qnet-item.qmenu-active .qnet-icon-normal { opacity: 0; }
.querencia-panel popover.querencia-network-menu label.qnet-header { font-weight: bold; padding: 4px 10px 1px 6px; color: #3c3c3c; }
.querencia-panel popover.querencia-network-menu label.qnet-note { font-style: italic; padding: 1px 10px 2px 26px; color: #7a7671; }
.querencia-panel popover.querencia-network-menu label.qnet-detail { font-size: smaller; color: #6e6a66; }
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
popover.querencia-menu .qmenu-item .qmenu-accel { color: #7a7671; }
popover.querencia-menu .qmenu-item.qmenu-active .qmenu-accel { color: #ffffff; }
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

/// The panel's icon theme and style sheet for the whole display; also used
/// by the isolated GUI test (`classic_menu/gui_test.rs`).
fn install_panel_style() {
    icon_theme::follow("querencia-panel");
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
    // Teil 6: icons the user gets later appear without a restart.
    icon_reload::start();

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
    // G2-AK: builds the panels and follows later changes of panel.ini
    neuladen::start(app, Rc::new(RefCell::new(editor)));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// G2-AD: the calendar's rules address the class the clock sets, and
    /// the selected day sets its text colour with its background (HMP-04).
    #[test]
    fn calendar_style_matches_the_clock_popover() {
        let selector = format!(
            ".querencia-panel popover.{} calendar",
            clock::CALENDAR_CLASS
        );
        let rules: Vec<_> = PANEL_CSS
            .lines()
            .filter(|l| l.starts_with(&selector))
            .collect();
        assert_eq!(rules.len(), 3, "{rules:?}");
        assert!(
            rules
                .iter()
                .any(|r| r.contains(":selected") && r.contains("color: #ffffff"))
        );
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

    /// HMP-11b: the pager looks like GNOME 2's in the light greys of
    /// Querencia-Klassisch, with the active cell in Querencia orange.
    #[test]
    fn workspace_pager_looks_like_the_gnome2_pager() {
        let rule = |sel: &str| {
            PANEL_CSS
                .lines()
                .find(|l| l.starts_with(sel))
                .unwrap_or_else(|| panic!("keine Regel für {sel}"))
        };
        let pager = rule(".querencia-panel box.workspace-pager {");
        // sunken frame: dark top and left, light bottom and right
        assert!(pager.contains("border: 1px solid #9e9993"), "{pager}");
        assert!(pager.contains("border-right-color: #fdfcfb"), "{pager}");
        assert!(pager.contains("border-bottom-color: #fdfcfb"), "{pager}");
        let cell = rule(".querencia-panel box.workspace-pager > button.workspace-cell {");
        assert!(
            cell.contains("background-color: #eeece9"),
            "hellgrau: {cell}"
        );
        // raised: light top-left, darker bottom-right edge
        assert!(
            cell.contains("inset 1px 1px #ffffff, inset -1px -1px #c6c1bb"),
            "{cell}"
        );
        assert!(
            cell.contains("min-width: 0") && cell.contains("padding: 0"),
            "{cell}"
        );
        assert!(cell.contains("transition: none"), "keine Animation: {cell}");
        let line = rule(
            ".querencia-panel box.workspace-pager > button.workspace-cell + button.workspace-cell {",
        );
        assert!(line.contains("border-left: 1px solid #aaa6a2"), "{line}");
        let active = rule(".querencia-panel box.workspace-pager > button.workspace-cell.active {");
        assert!(
            active.contains("#f8875d, #ea703d"),
            "Querencia-Orange: {active}"
        );
        assert!(active.contains("inset -1px -1px #b36c4e"), "{active}");
        for l in PANEL_CSS.lines().filter(|l| l.contains("workspace")) {
            assert!(l.starts_with(".querencia-panel "), "nicht begrenzt: {l}");
            assert!(!l.contains("font-weight"), "{l}");
            // square cells, no modern rounded cards
            assert!(
                !l.contains("border-radius") || l.contains("border-radius: 0"),
                "{l}"
            );
        }
    }

    /// HMP-06: window buttons are classic framed buttons, never orange or
    /// white text, active pressed, minimized dimmed; scoped to the panel.
    #[test]
    fn window_buttons_look_like_the_gnome2_window_list() {
        let rules: Vec<&str> = PANEL_CSS
            .lines()
            .filter(|l| l.contains("window-button"))
            .collect();
        let rule = |sel: &str| {
            *rules
                .iter()
                .find(|r| r.starts_with(sel))
                .unwrap_or_else(|| panic!("keine Regel für {sel}"))
        };
        let base = rule(".querencia-panel windowlist button.window-button {");
        assert!(base.contains("border-top-color: #c6c1bb"), "{base}");
        assert!(base.contains("border-bottom-color: #9e9993"), "{base}");
        assert!(base.contains("color: #3c3c3c"), "{base}");
        let active = rule(".querencia-panel windowlist button.window-button.active {");
        assert!(active.contains("inset"), "gedrückt: {active}");
        let minimized = rule(".querencia-panel windowlist button.window-button.minimized > box {");
        assert!(minimized.contains("opacity: 0.55"), "{minimized}");
        for r in &rules {
            assert!(r.starts_with(".querencia-panel "), "nicht begrenzt: {r}");
            for orange in ["#f07746", "#f8875d", "#ea703d", "orange", "#ffffff;"] {
                assert!(!r.contains(orange), "{r}");
            }
            assert!(!r.contains("font-weight: bold"), "{r}");
        }
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

    /// QD-045: one rule set for all status applets (no padding, 1 px hover
    /// border, neutral highlight, dimmed without service), none of their
    /// own per applet any more, so volume and network look alike.
    #[test]
    fn status_applets_share_one_compact_look() {
        let class = crate::panel::STATUS_CLASS;
        let rules: Vec<&str> = PANEL_CSS.lines().filter(|l| l.contains(class)).collect();
        let base = rules
            .iter()
            .find(|r| r.starts_with(&format!(".querencia-panel button.{class} {{")))
            .expect("Grundregel");
        for part in [
            "padding: 0;",
            "margin: 0;",
            "border: 1px solid transparent",
            "background-image: none",
            "outline: none",
        ] {
            assert!(base.contains(part), "{part}: {base}");
        }
        assert!(
            rules
                .iter()
                .any(|r| r.contains(":hover") && r.contains("#ebe7e3"))
        );
        assert!(
            rules
                .iter()
                .any(|r| r.contains(".unavailable") && r.contains("opacity"))
        );
        for rule in &rules {
            assert!(
                rule.starts_with(".querencia-panel "),
                "nicht begrenzt: {rule}"
            );
            for orange in ["#f07746", "#f8875d", "#ea703d", "orange"] {
                assert!(!rule.contains(orange), "{rule}");
            }
        }
        for own in [crate::volume_applet::CLASS, crate::network_applet::CLASS] {
            assert!(
                !PANEL_CSS.contains(&format!("button.{own}")),
                "eigene Knopfregel für {own}"
            );
        }
    }

    /// HMP-11: launchers are a bare icon like in the GNOME 2 panel; hover
    /// and press use the neutral highlight of the main menu buttons, never
    /// orange; a missing application is dimmed.
    #[test]
    fn launchers_are_compact_icons_without_orange() {
        let rules: Vec<&str> = PANEL_CSS
            .lines()
            .filter(|l| l.contains("querencia-launcher"))
            .collect();
        let base = rules
            .iter()
            .find(|r| r.starts_with(".querencia-panel button.querencia-launcher {"))
            .expect("Grundregel");
        for part in [
            "padding: 0 2px",
            "border: 1px solid transparent",
            "background-image: none",
            "outline: none",
        ] {
            assert!(base.contains(part), "{part}: {base}");
        }
        assert!(
            rules
                .iter()
                .any(|r| r.contains(":hover") && r.contains("#ebe7e3"))
        );
        assert!(
            rules
                .iter()
                .any(|r| r.contains(".missing") && r.contains("opacity"))
        );
        for rule in &rules {
            assert!(
                rule.starts_with(".querencia-panel "),
                "nicht begrenzt: {rule}"
            );
            for orange in ["#f07746", "#f8875d", "#ea703d", "orange"] {
                assert!(!rule.contains(orange), "{rule}");
            }
        }
    }
}
