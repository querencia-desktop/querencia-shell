//! `querencia-lautstaerke`, the small mixer of Querencia Desktop
//! („Lautstärkeregler“, like the GNOME 2 volume control): one window with
//! a classic menu bar and three tabs,
//! - Wiedergabe: output device (choosing one makes it the default), its
//!   volume and „Stumm“;
//! - Aufnahme: the same for input devices (monitors are left out);
//! - Anwendungen: one row per application stream with volume and „Stumm“.
//!
//! Every change goes to the audio service at once; what is shown always
//! comes from the service's events (`querencia_lautstaerke::service`), so
//! the panel applet and this window agree. Without an audio service the
//! window says so and waits for one. Single instance: starting it again
//! raises the open window.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, CheckButton, DropDown, Image,
    Label, Notebook, Orientation, PolicyType, Scale, ScrolledWindow, StringList, Window, gdk, gio,
    glib,
};

use querencia_lautstaerke::i18n::{gettext, gettext_f};
use querencia_lautstaerke::model::{self, Device, State, Stream};
use querencia_lautstaerke::service::{self, Service};
use querencia_lautstaerke::{MIXER_APP_ID, MIXER_PROGRAM, icons};

/// The window title (msgid English, German from po/de.po).
fn title() -> String {
    gettext("Volume Control")
}

/// After a change by the user, events of the service do not move that
/// slider for this long, so a dragged slider does not jump back to a value
/// the server reported a moment earlier.
const HOLD: Duration = Duration::from_millis(600);

fn main() -> glib::ExitCode {
    if std::env::args().nth(1).as_deref() == Some("--zustand") {
        return print_state();
    }
    querencia_lautstaerke::i18n::init();
    let app = Application::builder().application_id(MIXER_APP_ID).build();
    app.connect_startup(|app| {
        app.set_menubar(Some(&menu_model()));
        app.add_action_entries([
            gio::ActionEntry::builder("quit")
                .activate(|app: &Application, _, _| {
                    for window in app.windows() {
                        window.close();
                    }
                })
                .build(),
            gio::ActionEntry::builder("about")
                .activate(|app: &Application, _, _| about(app))
                .build(),
        ]);
        app.set_accels_for_action("app.quit", &["<Control>q"]);
        app.set_accels_for_action("window.close", &["<Control>w"]);
    });
    app.connect_activate(|app| {
        if let Some(window) = app
            .windows()
            .into_iter()
            .find(|w| w.is::<ApplicationWindow>())
        {
            window.present();
            return;
        }
        build_window(app).present();
        querencia_lautstaerke::say!("{MIXER_PROGRAM}: Fenster geöffnet");
    });
    app.connect_shutdown(|_| querencia_lautstaerke::say!("{MIXER_PROGRAM}: beendet"));
    app.run()
}

/// The classic menu bar: File and Help, with mnemonics.
fn menu_model() -> gio::Menu {
    let file = gio::Menu::new();
    file.append(Some(&gettext("_Close")), Some("window.close"));
    file.append(Some(&gettext("_Quit")), Some("app.quit"));
    let help = gio::Menu::new();
    help.append(Some(&gettext("_About")), Some("app.about"));
    let bar = gio::Menu::new();
    bar.append_submenu(Some(&gettext("_File")), &file);
    bar.append_submenu(Some(&gettext("_Help")), &help);
    bar
}

fn build_window(app: &Application) -> ApplicationWindow {
    let window = ApplicationWindow::builder()
        .application(app)
        .title(title())
        .icon_name(MIXER_APP_ID)
        .default_width(520)
        .default_height(400)
        .show_menubar(true)
        .build();
    let service = service::get(MIXER_PROGRAM);

    let notice = Label::builder()
        .xalign(0.0)
        .wrap(true)
        .margin_start(12)
        .margin_end(12)
        .margin_top(8)
        .build();
    notice.add_css_class("dim-label");

    let playback = DevicePage::new(&service, true);
    let recording = DevicePage::new(&service, false);
    let streams = StreamsPage::new(&service);
    let notebook = Notebook::builder()
        .margin_start(8)
        .margin_end(8)
        .margin_top(8)
        .vexpand(true)
        .build();
    notebook.append_page(
        &playback.root,
        Some(&Label::with_mnemonic(&gettext("_Playback"))),
    );
    notebook.append_page(
        &recording.root,
        Some(&Label::with_mnemonic(&gettext("_Recording"))),
    );
    notebook.append_page(
        &streams.root,
        Some(&Label::with_mnemonic(&gettext("_Applications"))),
    );

    let close = Button::with_mnemonic(&gettext("_Close"));
    close.set_halign(Align::End);
    close.set_margin_end(8);
    close.set_margin_top(8);
    close.set_margin_bottom(8);
    close.connect_clicked(glib::clone!(
        #[weak]
        window,
        move |_| window.close()
    ));

    let content = GtkBox::new(Orientation::Vertical, 0);
    content.append(&notice);
    content.append(&notebook);
    content.append(&close);
    window.set_child(Some(&content));

    let id = service.subscribe(glib::clone!(
        #[weak]
        notice,
        #[weak]
        notebook,
        move |state| {
            let (text, usable) = match &state.connection {
                model::Connection::Ready => (String::new(), true),
                model::Connection::Connecting => {
                    (gettext("Connecting to the audio service…"), false)
                }
                model::Connection::Unavailable(_) => (
                    gettext(
                        "No audio service. As soon as PipeWire or PulseAudio is running, \
                         the volume control connects by itself.",
                    ),
                    false,
                ),
            };
            notice.set_text(&text);
            notice.set_visible(!usable);
            notebook.set_sensitive(usable);
            playback.update(state);
            recording.update(state);
            streams.update(state);
        }
    ));
    window.connect_destroy(glib::clone!(
        #[strong]
        service,
        move |_| service.unsubscribe(id)
    ));
    window
}

/// Whether the user changed this control less than [`HOLD`] ago.
fn held(last: &Cell<Option<Instant>>) -> bool {
    last.get().is_some_and(|t| t.elapsed() < HOLD)
}

fn percent_text(percent: u32) -> String {
    gettext_f("{percent} %", &[("percent", &percent.to_string())])
}

fn volume_scale() -> Scale {
    let scale = Scale::with_range(
        Orientation::Horizontal,
        0.0,
        f64::from(model::MAX_PERCENT),
        1.0,
    );
    scale.set_hexpand(true);
    scale.set_draw_value(false);
    scale.set_increments(1.0, f64::from(model::SCROLL_STEP));
    scale.add_mark(0.0, gtk4::PositionType::Bottom, None);
    scale.add_mark(100.0, gtk4::PositionType::Bottom, Some(&percent_text(100)));
    scale
}

/// The Wiedergabe or Aufnahme tab.
struct DevicePage {
    root: GtkBox,
    sink: bool,
    devices: DropDown,
    labels: StringList,
    /// Device names in the order of `labels`.
    names: RefCell<Vec<String>>,
    /// The device the slider and „Stumm“ belong to.
    current: RefCell<Option<String>>,
    scale: Scale,
    percent: Label,
    mute: CheckButton,
    /// Set while the page itself changes its controls.
    updating: Cell<bool>,
    last_change: Cell<Option<Instant>>,
}

impl DevicePage {
    fn new(service: &Rc<Service>, sink: bool) -> Rc<DevicePage> {
        let labels = StringList::new(&[]);
        let devices = DropDown::builder().model(&labels).hexpand(true).build();
        let scale = volume_scale();
        let percent = Label::builder().width_chars(6).xalign(1.0).build();
        let mute = CheckButton::with_mnemonic(&gettext("_Mute"));
        let device_label = Label::builder()
            .label(if sink {
                gettext("_Output device:")
            } else {
                gettext("_Input device:")
            })
            .use_underline(true)
            .mnemonic_widget(&devices)
            .xalign(0.0)
            .build();
        let volume_label = Label::builder()
            .label(gettext("_Volume:"))
            .use_underline(true)
            .mnemonic_widget(&scale)
            .xalign(0.0)
            .build();
        let grid = gtk4::Grid::builder()
            .row_spacing(12)
            .column_spacing(12)
            .margin_top(18)
            .margin_bottom(18)
            .margin_start(18)
            .margin_end(18)
            .build();
        grid.attach(&device_label, 0, 0, 1, 1);
        grid.attach(&devices, 1, 0, 2, 1);
        grid.attach(&volume_label, 0, 1, 1, 1);
        grid.attach(&scale, 1, 1, 1, 1);
        grid.attach(&percent, 2, 1, 1, 1);
        grid.attach(&mute, 1, 2, 2, 1);
        let root = GtkBox::new(Orientation::Vertical, 0);
        root.append(&grid);

        let page = Rc::new(DevicePage {
            root,
            sink,
            devices,
            labels,
            names: RefCell::new(Vec::new()),
            current: RefCell::new(None),
            scale,
            percent,
            mute,
            updating: Cell::new(false),
            last_change: Cell::new(None),
        });

        let weak = Rc::downgrade(&page);
        let svc = service.clone();
        page.devices.connect_selected_notify(move |dropdown| {
            let Some(page) = weak.upgrade() else { return };
            if page.updating.get() {
                return;
            }
            let selected = dropdown.selected() as usize;
            if let Some(name) = page.names.borrow().get(selected) {
                svc.set_default(page.sink, name);
            }
        });
        let weak = Rc::downgrade(&page);
        let svc = service.clone();
        page.scale.connect_value_changed(move |scale| {
            let Some(page) = weak.upgrade() else { return };
            let percent = scale.value().round() as u32;
            page.percent.set_text(&percent_text(percent));
            if page.updating.get() {
                return;
            }
            page.last_change.set(Some(Instant::now()));
            if let Some(name) = page.current.borrow().as_deref() {
                svc.set_device_percent(page.sink, name, percent);
            }
        });
        let weak = Rc::downgrade(&page);
        let svc = service.clone();
        page.mute.connect_toggled(move |mute| {
            let Some(page) = weak.upgrade() else { return };
            if page.updating.get() {
                return;
            }
            if let Some(name) = page.current.borrow().as_deref() {
                querencia_lautstaerke::say!(
                    "{MIXER_PROGRAM}: {} {name} {}",
                    if page.sink { "Ausgabe" } else { "Eingabe" },
                    if mute.is_active() { "stumm" } else { "laut" }
                );
                svc.set_device_muted(page.sink, name, mute.is_active());
            }
        });
        page
    }

    fn update(&self, state: &State) {
        let devices: Vec<&Device> = if self.sink {
            state.sinks.iter().collect()
        } else {
            state.inputs().collect()
        };
        let default = if self.sink {
            state.output()
        } else {
            state.input()
        };
        self.updating.set(true);
        let names: Vec<String> = devices.iter().map(|d| d.name.clone()).collect();
        let no_device = gettext("No device");
        let mut labels: Vec<&str> = devices.iter().map(|d| d.label()).collect();
        // Never GTK's untranslated „(None)“ for an empty list.
        if labels.is_empty() {
            labels.push(&no_device);
        }
        let shown: Vec<String> = (0..self.labels.n_items())
            .filter_map(|i| self.labels.string(i).map(|s| s.to_string()))
            .collect();
        // Replace the list only when it changed (never from the selection
        // handler, see G2-G: a DropDown whose model is replaced there stays
        // open without input).
        if *self.names.borrow() != names || shown != labels {
            self.labels.splice(0, self.labels.n_items(), &labels);
            *self.names.borrow_mut() = names.clone();
        }
        let selected = default
            .and_then(|d| names.iter().position(|n| *n == d.name))
            .map_or(
                if names.is_empty() {
                    0
                } else {
                    gtk4::INVALID_LIST_POSITION
                },
                |i| i as u32,
            );
        if self.devices.selected() != selected {
            self.devices.set_selected(selected);
        }
        *self.current.borrow_mut() = default.map(|d| d.name.clone());
        match default {
            Some(device) => {
                if !held(&self.last_change) {
                    self.scale
                        .set_value(f64::from(device.percent().min(model::MAX_PERCENT)));
                }
                self.percent.set_text(&percent_text(device.percent()));
                self.mute.set_active(device.muted);
            }
            None => {
                self.scale.set_value(0.0);
                self.percent.set_text("");
                self.mute.set_active(false);
            }
        }
        let present = default.is_some();
        self.devices.set_sensitive(!devices.is_empty());
        self.scale.set_sensitive(present);
        self.mute.set_sensitive(present);
        self.updating.set(false);
    }
}

/// One application stream in the Anwendungen tab.
struct StreamRow {
    scale: Scale,
    percent: Label,
    mute: CheckButton,
    updating: Rc<Cell<bool>>,
    last_change: Rc<Cell<Option<Instant>>>,
}

/// The Anwendungen tab.
struct StreamsPage {
    root: GtkBox,
    list: GtkBox,
    empty: Label,
    service: Rc<Service>,
    /// (playback, index, label) of the rows shown, in order.
    shown: RefCell<Vec<(bool, u32, String)>>,
    rows: RefCell<Vec<StreamRow>>,
}

impl StreamsPage {
    fn new(service: &Rc<Service>) -> Rc<StreamsPage> {
        let list = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .spacing(12)
            .margin_top(18)
            .margin_bottom(18)
            .margin_start(18)
            .margin_end(18)
            .build();
        let empty = Label::new(Some(&gettext(
            "No application is playing or recording sound right now.",
        )));
        empty.add_css_class("dim-label");
        empty.set_margin_top(24);
        let scrolled = ScrolledWindow::builder()
            .hscrollbar_policy(PolicyType::Never)
            .vexpand(true)
            .child(&list)
            .build();
        let root = GtkBox::new(Orientation::Vertical, 0);
        root.append(&empty);
        root.append(&scrolled);
        Rc::new(StreamsPage {
            root,
            list,
            empty,
            service: service.clone(),
            shown: RefCell::new(Vec::new()),
            rows: RefCell::new(Vec::new()),
        })
    }

    fn update(&self, state: &State) {
        let streams: Vec<(bool, &Stream)> = state
            .playback
            .iter()
            .map(|s| (true, s))
            .chain(state.recording.iter().map(|s| (false, s)))
            .collect();
        let wanted: Vec<(bool, u32, String)> = streams
            .iter()
            .map(|(p, s)| (*p, s.index, row_label(*p, s)))
            .collect();
        if *self.shown.borrow() != wanted {
            while let Some(child) = self.list.first_child() {
                self.list.remove(&child);
            }
            let mut rows = Vec::new();
            for (playback, stream) in &streams {
                let (widget, row) = self.new_row(*playback, stream);
                self.list.append(&widget);
                rows.push(row);
            }
            *self.rows.borrow_mut() = rows;
            *self.shown.borrow_mut() = wanted;
        }
        for (row, (_, stream)) in self.rows.borrow().iter().zip(&streams) {
            row.updating.set(true);
            if !held(&row.last_change) {
                row.scale
                    .set_value(f64::from(stream.percent().min(model::MAX_PERCENT)));
            }
            row.percent.set_text(&percent_text(stream.percent()));
            row.mute.set_active(stream.muted);
            row.scale.set_sensitive(stream.adjustable);
            row.updating.set(false);
        }
        self.empty.set_visible(streams.is_empty());
    }

    fn new_row(&self, playback: bool, stream: &Stream) -> (GtkBox, StreamRow) {
        let icon = Image::from_icon_name(stream.icon.as_deref().unwrap_or(if playback {
            "audio-x-generic"
        } else {
            "audio-input-microphone"
        }));
        icon.set_pixel_size(24);
        let title = Label::builder()
            .label(row_label(playback, stream))
            .xalign(0.0)
            .ellipsize(gtk4::pango::EllipsizeMode::End)
            .hexpand(true)
            .build();
        let head = GtkBox::new(Orientation::Horizontal, 8);
        head.append(&icon);
        head.append(&title);
        let scale = volume_scale();
        let percent = Label::builder().width_chars(6).xalign(1.0).build();
        let mute = CheckButton::with_label(&gettext("Mute"));
        let controls = GtkBox::new(Orientation::Horizontal, 8);
        controls.set_margin_start(32);
        controls.append(&scale);
        controls.append(&percent);
        controls.append(&mute);
        let widget = GtkBox::new(Orientation::Vertical, 4);
        widget.append(&head);
        widget.append(&controls);

        let updating = Rc::new(Cell::new(false));
        let last_change = Rc::new(Cell::new(None));
        let index = stream.index;
        let svc = self.service.clone();
        scale.connect_value_changed(glib::clone!(
            #[strong]
            updating,
            #[strong]
            last_change,
            #[weak]
            percent,
            move |scale| {
                let value = scale.value().round() as u32;
                percent.set_text(&percent_text(value));
                if !updating.get() {
                    last_change.set(Some(Instant::now()));
                    svc.set_stream_percent(playback, index, value);
                }
            }
        ));
        let svc = self.service.clone();
        mute.connect_toggled(glib::clone!(
            #[strong]
            updating,
            move |mute| {
                if !updating.get() {
                    svc.set_stream_muted(playback, index, mute.is_active());
                }
            }
        ));
        (
            widget,
            StreamRow {
                scale,
                percent,
                mute,
                updating,
                last_change,
            },
        )
    }
}

fn row_label(playback: bool, stream: &Stream) -> String {
    if playback {
        stream.label()
    } else {
        gettext_f("{stream} (recording)", &[("stream", &stream.label())])
    }
}

/// „Info“ from the Hilfe menu: a small dialog over the mixer; the
/// „Schließen“ button has the keyboard focus, Escape closes it.
fn about(app: &Application) {
    let parent = app
        .windows()
        .into_iter()
        .find(|w| w.is::<ApplicationWindow>());
    let dialog = Window::builder()
        .title(gettext_f("About {program}", &[("program", &title())]))
        .resizable(false)
        .modal(true)
        .build();
    dialog.set_transient_for(parent.as_ref());
    dialog.set_application(Some(app));
    let logo = Image::new();
    logo.set_pixel_size(64);
    if let Some(texture) = icons::render(icons::APP_SVG, 64 * dialog.scale_factor().max(1)) {
        logo.set_paintable(Some(&texture));
    }
    let name = Label::builder()
        .label(format!(
            "<b><big>{}</big></b>",
            glib::markup_escape_text(&title())
        ))
        .use_markup(true)
        .build();
    let server = service::get(MIXER_PROGRAM).state().server;
    let mut lines = vec![
        gettext_f(
            "Version {version}",
            &[("version", env!("CARGO_PKG_VERSION"))],
        ),
        gettext("Adjust the volume of devices and applications"),
        gettext("Part of Querencia Desktop"),
    ];
    lines.push(if server.is_empty() {
        gettext("Audio service: none")
    } else {
        gettext_f("Audio service: {server}", &[("server", &server)])
    });
    lines.push(gettext_f(
        "License: {license}",
        &[("license", env!("CARGO_PKG_LICENSE"))],
    ));
    let content = GtkBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(6)
        .margin_top(18)
        .margin_bottom(12)
        .margin_start(18)
        .margin_end(18)
        .build();
    content.append(&logo);
    content.append(&name);
    for line in lines {
        content.append(
            &Label::builder()
                .label(line)
                .wrap(true)
                .max_width_chars(40)
                .justify(gtk4::Justification::Center)
                .build(),
        );
    }
    let close = Button::with_mnemonic(&gettext("_Close"));
    close.set_halign(Align::End);
    close.set_margin_top(12);
    close.connect_clicked(glib::clone!(
        #[weak]
        dialog,
        move |_| dialog.close()
    ));
    content.append(&close);
    dialog.set_child(Some(&content));
    let escape = gtk4::ShortcutController::new();
    escape.add_shortcut(gtk4::Shortcut::new(
        Some(gtk4::KeyvalTrigger::new(
            gdk::Key::Escape,
            gdk::ModifierType::empty(),
        )),
        Some(gtk4::NamedAction::new("window.close")),
    ));
    dialog.add_controller(escape);
    dialog.connect_close_request(|_| {
        querencia_lautstaerke::say!("{MIXER_PROGRAM}: Info geschlossen");
        glib::Propagation::Proceed
    });
    GtkWindowExt::set_focus(&dialog, Some(&close));
    dialog.present();
    querencia_lautstaerke::say!("{MIXER_PROGRAM}: Info geöffnet (Fokus auf „Schließen“)");
}

/// `querencia-lautstaerke --zustand`: connects, waits for the complete
/// state (at most 5 s), prints devices and streams as text and exits; opens
/// no window. Exit 0 with an audio service, 1 without (or on timeout). For
/// diagnosis and the isolated test.
fn print_state() -> glib::ExitCode {
    let main_loop = glib::MainLoop::new(None, false);
    let service = service::get(MIXER_PROGRAM);
    let id = service.subscribe(glib::clone!(
        #[strong]
        main_loop,
        move |state| {
            if state.connection != model::Connection::Connecting {
                main_loop.quit();
            }
        }
    ));
    let timeout = glib::timeout_add_local_once(
        Duration::from_secs(5),
        glib::clone!(
            #[strong]
            main_loop,
            move || main_loop.quit()
        ),
    );
    if service.state().connection == model::Connection::Connecting {
        main_loop.run();
    }
    service.unsubscribe(id);
    let state = service.state();
    if state.connection == model::Connection::Connecting {
        println!("Verbindung: Zeitüberschreitung");
        return glib::ExitCode::FAILURE;
    }
    timeout.remove();
    print!("{}", state_text(&state));
    if state.ready() {
        glib::ExitCode::SUCCESS
    } else {
        glib::ExitCode::FAILURE
    }
}

/// The state as lines of text (see [`print_state`]).
fn state_text(state: &State) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    match &state.connection {
        model::Connection::Ready => {
            let _ = writeln!(out, "Verbindung: bereit");
        }
        model::Connection::Connecting => {
            let _ = writeln!(out, "Verbindung: wird aufgebaut");
        }
        model::Connection::Unavailable(reason) => {
            let _ = writeln!(out, "Verbindung: {} ({reason})", model::NO_SERVICE);
            return out;
        }
    }
    let _ = writeln!(out, "Server: {}", state.server);
    let _ = writeln!(
        out,
        "Standardausgabe: {}",
        state.output().map_or("–", |d| d.name.as_str())
    );
    let _ = writeln!(
        out,
        "Standardeingabe: {}",
        state.default_source.as_deref().unwrap_or("–")
    );
    let device = |out: &mut String, kind: &str, d: &Device, default: bool| {
        let _ = writeln!(
            out,
            "{kind} {} {} „{}“ {} %{}{}",
            d.index,
            d.name,
            d.label(),
            d.percent(),
            if d.muted { " stumm" } else { "" },
            if default { " Standard" } else { "" }
        );
    };
    for d in &state.sinks {
        let default = state.output().is_some_and(|o| o.name == d.name);
        device(&mut out, "Ausgabe", d, default);
    }
    for d in &state.sources {
        let default = state.default_source.as_deref() == Some(d.name.as_str());
        device(
            &mut out,
            if d.monitor { "Monitor" } else { "Eingabe" },
            d,
            default,
        );
    }
    for (kind, streams) in [
        ("Wiedergabe", &state.playback),
        ("Aufnahme", &state.recording),
    ] {
        for s in streams {
            let _ = writeln!(
                out,
                "{kind} {} „{}“ {} %{} Gerät {}",
                s.index,
                s.label(),
                s.percent(),
                if s.muted { " stumm" } else { "" },
                s.device
            );
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_text_lists_devices_streams_and_defaults() {
        let unavailable = state_text(&State::unavailable("Verbindung verweigert"));
        assert_eq!(
            unavailable,
            "Verbindung: Kein Audiodienst (Verbindung verweigert)\n"
        );
        let state = State {
            connection: model::Connection::Ready,
            server: "pulseaudio 17.0".into(),
            default_sink: Some("b".into()),
            default_source: None,
            sinks: vec![
                Device {
                    index: 0,
                    name: "a".into(),
                    description: "A".into(),
                    volume: vec![model::raw_for_percent(40); 2],
                    muted: false,
                    monitor: false,
                },
                Device {
                    index: 1,
                    name: "b".into(),
                    description: "B".into(),
                    volume: vec![model::raw_for_percent(55)],
                    muted: true,
                    monitor: false,
                },
            ],
            sources: vec![Device {
                index: 0,
                name: "a.monitor".into(),
                description: "Monitor of A".into(),
                volume: vec![model::VOLUME_NORM],
                muted: false,
                monitor: true,
            }],
            playback: vec![Stream {
                index: 7,
                application: "Firefox".into(),
                title: "Musik".into(),
                icon: None,
                device: 1,
                volume: vec![model::raw_for_percent(80)],
                muted: false,
                adjustable: true,
            }],
            recording: Vec::new(),
        };
        assert_eq!(
            state_text(&state),
            "Verbindung: bereit\n\
             Server: pulseaudio 17.0\n\
             Standardausgabe: b\n\
             Standardeingabe: –\n\
             Ausgabe 0 a „A“ 40 %\n\
             Ausgabe 1 b „B“ 55 % stumm Standard\n\
             Monitor 0 a.monitor „Monitor of A“ 100 %\n\
             Wiedergabe 7 „Firefox: Musik“ 80 % Gerät 1\n"
        );
    }
}
