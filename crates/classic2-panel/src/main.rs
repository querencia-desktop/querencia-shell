mod app_menu;
mod apps;
mod clock;
mod places_menu;
mod window_list;
mod workspace_switcher;

use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Box as GtkBox, CenterBox, Label, Orientation, Widget, gio, glib,
};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

const APP_ID: &str = "org.example.Classic2.Panel";
const NAMESPACE: &str = "classic2-panel";
const PANEL_HEIGHT: i32 = 32;

// GTK's default button min-height plus padding is 34 px and would push the
// panel past PANEL_HEIGHT; panel buttons fill the bar height instead. A
// window or workspace button is highlighted while labwc reports it active.
const PANEL_CSS: &str = "
.classic2-panel menubutton > button,
.classic2-panel button.window-button,
.classic2-panel button.workspace-button { min-height: 0; padding-top: 0; padding-bottom: 0; }
.classic2-panel button.window-button.active,
.classic2-panel button.workspace-button.active { background-color: alpha(currentColor, 0.15); font-weight: bold; }
";

fn main() -> glib::ExitCode {
    if let Err(err) = gtk4::init() {
        eprintln!("classic2-panel: GTK konnte nicht initialisiert werden: {err}");
        return glib::ExitCode::FAILURE;
    }
    if !gtk4_layer_shell::is_supported() {
        eprintln!(
            "classic2-panel: Der Compositor bietet kein wlr-layer-shell an. \
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

    top_panel(app).present();
    bottom_panel(app).present();
}

fn top_panel(app: &Application) -> ApplicationWindow {
    let menus = GtkBox::new(Orientation::Horizontal, 16);
    menus.append(&app_menu::button());
    menus.append(&places_menu::button());
    menus.append(&Label::new(Some("System")));

    let clock = Label::new(None);
    clock::attach(&clock);

    // Keyboard focus stays with the compositor: both panels keep
    // KeyboardMode::None (see panel_window). From labwc 0.9.4 on, labwc gives
    // the menu's grabbing popup the keyboard and returns it to the previously
    // focused window when the popup closes. labwc 0.9.3 never focuses layer
    // popups, so it is not supported. Evidence: docs/m1-1b0-labwc-ab-test.md.
    panel_window(app, Edge::Top, &menus, &clock)
}

fn bottom_panel(app: &Application) -> ApplicationWindow {
    panel_window(
        app,
        Edge::Bottom,
        &window_list::widget(),
        &workspace_switcher::widget(),
    )
}

fn panel_window(
    app: &Application,
    edge: Edge,
    start: &impl IsA<Widget>,
    end: &impl IsA<Widget>,
) -> ApplicationWindow {
    let bar = CenterBox::builder()
        .css_classes(["classic2-panel"])
        .height_request(PANEL_HEIGHT)
        .margin_start(10)
        .margin_end(10)
        .build();
    bar.set_start_widget(Some(start));
    bar.set_end_widget(Some(end));

    let window = ApplicationWindow::builder()
        .application(app)
        .title("Classic2 Panel")
        .default_height(PANEL_HEIGHT)
        .child(&bar)
        .build();

    window.init_layer_shell();
    window.set_namespace(Some(NAMESPACE));
    window.set_layer(Layer::Top);
    window.set_keyboard_mode(KeyboardMode::None);
    for anchor in [edge, Edge::Left, Edge::Right] {
        window.set_anchor(anchor, true);
    }
    window.auto_exclusive_zone_enable();
    window
}
