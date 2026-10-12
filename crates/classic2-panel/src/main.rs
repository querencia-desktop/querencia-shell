mod clock;

use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Box as GtkBox, CenterBox, Label, Orientation, Widget, gio, glib,
};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

const APP_ID: &str = "org.example.Classic2.Panel";
const NAMESPACE: &str = "classic2-panel";
const PANEL_HEIGHT: i32 = 32;

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
    top_panel(app).present();
    bottom_panel(app).present();
}

fn top_panel(app: &Application) -> ApplicationWindow {
    let menus = GtkBox::new(Orientation::Horizontal, 16);
    for name in ["Anwendungen", "Orte", "System"] {
        menus.append(&Label::new(Some(name)));
    }

    let clock = Label::new(None);
    clock::attach(&clock);

    panel_window(app, Edge::Top, &menus, &clock)
}

fn bottom_panel(app: &Application) -> ApplicationWindow {
    panel_window(
        app,
        Edge::Bottom,
        &placeholder("Fensterliste"),
        &placeholder("Arbeitsflächen"),
    )
}

fn placeholder(text: &str) -> Label {
    let label = Label::new(Some(text));
    label.add_css_class("dim-label");
    label
}

fn panel_window(
    app: &Application,
    edge: Edge,
    start: &impl IsA<Widget>,
    end: &impl IsA<Widget>,
) -> ApplicationWindow {
    let bar = CenterBox::builder()
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
