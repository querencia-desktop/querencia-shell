mod clock;

use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Box as GtkBox, CenterBox, Label, MenuButton, Orientation,
    Widget, gio, glib,
};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

const APP_ID: &str = "org.example.Classic2.Panel";
const NAMESPACE: &str = "classic2-panel";
const PANEL_HEIGHT: i32 = 32;

// GTK's default button min-height plus padding is 34 px and would push the
// panel past PANEL_HEIGHT; panel buttons fill the bar height instead.
const PANEL_CSS: &str =
    ".classic2-panel menubutton > button { min-height: 0; padding-top: 0; padding-bottom: 0; }";

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
    let menu = applications_menu();
    menus.append(&menu);
    for name in ["Orte", "System"] {
        menus.append(&Label::new(Some(name)));
    }

    let clock = Label::new(None);
    clock::attach(&clock);

    let window = panel_window(app, Edge::Top, &menus, &clock);

    // labwc focuses an OnDemand layer surface when it maps and when it is
    // pressed, then keeps it focused. The menu's popup grab freezes keyboard
    // focus where it is, so the press must already find OnDemand. Hence: map
    // with None (no focus stealing at startup), switch to OnDemand once the
    // pointer is over the panel, and commit None when the menu closes, which
    // makes labwc return focus to the topmost window.
    let hover = gtk4::EventControllerMotion::new();
    hover.connect_enter(glib::clone!(
        #[weak]
        window,
        move |_, _, _| window.set_keyboard_mode(KeyboardMode::OnDemand)
    ));
    window.add_controller(hover);

    menu.popover()
        .expect("a menu model always creates a popover")
        .connect_closed(glib::clone!(
            #[weak]
            window,
            move |_| {
                // Both requests usually reach labwc in one commit. labwc 0.9.3
                // releases a focused layer surface for a committed None and for
                // a re-committed OnDemand alike (src/layers.c), and OnDemand
                // is needed again for the next press.
                window.set_keyboard_mode(KeyboardMode::None);
                window.set_keyboard_mode(KeyboardMode::OnDemand);
            }
        ));

    window
}

fn applications_menu() -> MenuButton {
    let model = gio::Menu::new();
    model.append(Some("Testaktion A"), Some("menu.test-a"));
    model.append(Some("Testaktion B"), Some("menu.test-b"));
    model.append(Some("Menü schließen"), Some("menu.close"));

    let button = MenuButton::builder()
        .label("Anwendungen")
        .menu_model(&model)
        .has_frame(false)
        .build();

    let actions = gio::SimpleActionGroup::new();
    actions.add_action_entries([
        gio::ActionEntry::builder("test-a")
            .activate(|_: &gio::SimpleActionGroup, _, _| {
                eprintln!("classic2-panel: Testaktion A ausgelöst")
            })
            .build(),
        gio::ActionEntry::builder("test-b")
            .activate(|_: &gio::SimpleActionGroup, _, _| {
                eprintln!("classic2-panel: Testaktion B ausgelöst")
            })
            .build(),
    ]);
    let close = gio::SimpleAction::new("close", None);
    close.connect_activate(glib::clone!(
        #[weak]
        button,
        move |_, _| button.popdown()
    ));
    actions.add_action(&close);
    button.insert_action_group("menu", Some(&actions));

    button
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
