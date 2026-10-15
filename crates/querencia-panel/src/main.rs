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
const PANEL_CSS: &str = "
.querencia-panel menubutton > button,
.querencia-panel button.window-button,
.querencia-panel button.workspace-button { min-height: 0; padding-top: 0; padding-bottom: 0; }
.querencia-panel button.window-button.active,
.querencia-panel button.workspace-button.active { background-color: alpha(currentColor, 0.15); font-weight: bold; }
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

fn build_panels(app: &Application) {
    // A second launch only re-activates the running instance; never stack panels.
    if !app.windows().is_empty() {
        return;
    }

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
