use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Box as GtkBox, CenterBox, Label, Orientation, gio, glib,
};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

const APP_ID: &str = "org.example.Classic2.Panel";
const NAMESPACE: &str = "classic2-panel";
const PANEL_HEIGHT: i32 = 34;

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
    app.connect_activate(build_panel);
    app.run()
}

fn build_panel(app: &Application) {
    // A second launch only re-activates the running instance; never stack panels.
    if let Some(window) = app.windows().first() {
        window.present();
        return;
    }

    let window = ApplicationWindow::builder()
        .application(app)
        .title("Classic2 Panel")
        .default_height(PANEL_HEIGHT)
        .build();

    window.init_layer_shell();
    window.set_namespace(Some(NAMESPACE));
    window.set_layer(Layer::Top);
    window.set_keyboard_mode(KeyboardMode::None);
    for edge in [Edge::Top, Edge::Left, Edge::Right] {
        window.set_anchor(edge, true);
    }
    window.auto_exclusive_zone_enable();

    let menus = GtkBox::new(Orientation::Horizontal, 16);
    for name in ["Anwendungen", "Orte", "System"] {
        menus.append(&Label::new(Some(name)));
    }

    let bar = CenterBox::builder()
        .height_request(PANEL_HEIGHT)
        .margin_start(10)
        .margin_end(10)
        .build();
    bar.set_start_widget(Some(&menus));
    bar.set_end_widget(Some(&Label::new(Some("Classic2 – GTK4/Wayland"))));

    window.set_child(Some(&bar));
    window.present();
}
