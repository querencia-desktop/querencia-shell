//! The windows of the network applet (G2-M), ordinary toplevels as the
//! other panel dialogs (labwc gives them the keyboard, no transient parent:
//! the panels are layer surfaces):
//!
//! - „Verbindungsinformationen“ ([`info`]): one tab per device with
//!   interface, hardware address, speed, IPv4/IPv6 addresses, gateway and
//!   DNS, updated while open; „Schließen“ has the focus;
//! - the password question for a WLAN ([`password`]), also used by the
//!   secret agent when NetworkManager asks ([`install_prompt`]);
//! - „Mit verborgenem Netz verbinden“ ([`hidden`]);
//! - a short message when NetworkManager refuses ([`report`]).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, CheckButton, DropDown, Entry, Grid, Label, Notebook, Orientation,
    Window, glib,
};

use querencia_netzwerk::model::{self, Security, State};
use querencia_netzwerk::service::{Answer, Close, Done, Prompt, Service};

use crate::{add_dialog, session};

pub const INFO_TITLE: &str = "Verbindungsinformationen";
pub const PASSWORD_TITLE: &str = "WLAN-Passwort";
pub const HIDDEN_TITLE: &str = "Mit verborgenem Netz verbinden";
pub const ERROR_TITLE: &str = "Netzwerk";
/// Least width of the notebook in the information window.
const INFO_WIDTH: i32 = 500;

fn window(title: &str) -> Window {
    let window = Window::builder().title(title).resizable(false).build();
    window.set_application(session::panel_application().as_ref());
    session::close_on_escape(&window);
    window
}

fn content() -> GtkBox {
    GtkBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(8)
        .margin_top(14)
        .margin_bottom(12)
        .margin_start(16)
        .margin_end(16)
        .build()
}

fn buttons(children: &[&Button]) -> GtkBox {
    let row = GtkBox::new(Orientation::Horizontal, 6);
    row.set_halign(Align::End);
    row.set_margin_top(8);
    for child in children {
        row.append(*child);
    }
    row
}

/// The `done` of an action: logs nothing more (the service did) and shows
/// a message when it failed, titled with `what`.
pub fn report(what: &str) -> Done {
    let what = what.to_string();
    Box::new(move |result| {
        if let Err(reason) = result {
            message(&format!("{what}: Das hat nicht geklappt."), &reason);
        }
    })
}

/// A small message window with „Schließen“ focused.
pub fn message(headline: &str, text: &str) {
    let window = window(ERROR_TITLE);
    let body = content();
    let head = Label::new(None);
    head.set_markup(&format!("<b>{}</b>", glib::markup_escape_text(headline)));
    head.set_xalign(0.0);
    body.append(&head);
    let detail = Label::new(Some(text));
    detail.set_wrap(true);
    detail.set_max_width_chars(50);
    detail.set_xalign(0.0);
    body.append(&detail);
    let close = add_dialog::stock_button("_Schließen", "window-close");
    close.connect_clicked(glib::clone!(
        #[weak]
        window,
        move |_| window.close()
    ));
    body.append(&buttons(&[&close]));
    window.set_child(Some(&body));
    GtkWindowExt::set_focus(&window, Some(&close));
    window.present();
    querencia_netzwerk::say!("querencia-panel: Netzwerk: Meldung „{headline}“: {text}");
}

/// The page of one device: the groups of [`model::info`] as a grid.
fn device_page(device: &model::Device, backend: &model::Backend) -> Grid {
    let grid = Grid::builder()
        .row_spacing(3)
        .column_spacing(14)
        .margin_top(10)
        .margin_bottom(10)
        .margin_start(12)
        .margin_end(12)
        .build();
    let mut line = 0;
    for (group, rows) in model::info(device, backend) {
        let head = Label::new(None);
        head.set_markup(&format!("<b>{group}</b>"));
        head.set_xalign(0.0);
        if line > 0 {
            head.set_margin_top(8);
        }
        grid.attach(&head, 0, line, 2, 1);
        line += 1;
        for (label, value) in rows {
            let name = Label::new(Some(&format!("{label}:")));
            name.set_xalign(1.0);
            name.add_css_class("dim-label");
            name.set_margin_start(12);
            let value = Label::new(Some(&value));
            value.set_xalign(0.0);
            value.set_selectable(true);
            value.set_focusable(false);
            grid.attach(&name, 0, line, 1, 1);
            grid.attach(&value, 1, line, 1, 1);
            line += 1;
        }
    }
    grid
}

/// The devices in the order of the tabs: connected ones first (the
/// primary first), then the others as listed.
pub fn info_order(state: &State) -> Vec<&model::Device> {
    let mut list: Vec<&model::Device> = state.devices.iter().collect();
    list.sort_by_key(|d| (!d.connected(), !d.primary));
    list
}

fn fill_info(notebook: &Notebook, state: &State) {
    let current = notebook.current_page();
    while notebook.n_pages() > 0 {
        notebook.remove_page(Some(0));
    }
    let devices = info_order(state);
    if devices.is_empty() {
        let none = Label::new(Some("Kein Netzwerkgerät"));
        none.set_margin_top(20);
        none.set_margin_bottom(20);
        notebook.append_page(&none, Some(&Label::new(Some("–"))));
    }
    for device in devices {
        let title = format!(
            "{} ({})",
            match device.kind {
                model::Kind::Wired => "Kabel",
                model::Kind::Wireless => "WLAN",
            },
            device.interface
        );
        notebook.append_page(
            &device_page(device, &state.backend),
            Some(&Label::new(Some(&title))),
        );
    }
    if let Some(page) = current.filter(|&p| p < notebook.n_pages()) {
        notebook.set_current_page(Some(page));
    }
}

/// Raises the information window in `slot` or opens a new one there; it
/// follows the service while open.
pub fn info(slot: &glib::WeakRef<Window>, service: &Rc<Service>) {
    if let Some(window) = slot.upgrade() {
        window.present();
        return;
    }
    let window = window(INFO_TITLE);
    let body = content();
    let notebook = Notebook::new();
    notebook.set_focusable(false);
    // A fixed least width keeps the window (and its tabs) in place while
    // the values change.
    notebook.set_size_request(INFO_WIDTH, -1);
    body.append(&notebook);
    let close = add_dialog::stock_button("_Schließen", "window-close");
    close.connect_clicked(glib::clone!(
        #[weak]
        window,
        move |_| window.close()
    ));
    body.append(&buttons(&[&close]));
    window.set_child(Some(&body));
    let shown: Rc<RefCell<Option<State>>> = Rc::default();
    let id = service.subscribe(glib::clone!(
        #[weak]
        notebook,
        #[strong]
        shown,
        move |state| {
            if shown.borrow().as_ref() != Some(state) {
                fill_info(&notebook, state);
                shown.replace(Some(state.clone()));
            }
        }
    ));
    window.connect_destroy(glib::clone!(
        #[strong]
        service,
        move |_| service.unsubscribe(id)
    ));
    GtkWindowExt::set_focus(&window, Some(&close));
    slot.set(Some(&window));
    window.present();
    querencia_netzwerk::say!("querencia-panel: Netzwerk: Dialog „{INFO_TITLE}“ geöffnet");
}

/// The question text for `prompt`.
pub fn password_text(prompt: &Prompt) -> String {
    if prompt.retry {
        format!(
            "Das Passwort für das WLAN „{}“ wurde nicht angenommen. Bitte noch einmal eingeben.",
            prompt.ssid
        )
    } else {
        format!("Für das WLAN „{}“ ist ein Passwort nötig.", prompt.ssid)
    }
}

/// A password entry with „Passwort anzeigen“ below it.
fn password_entry() -> (Entry, CheckButton) {
    let entry = Entry::builder()
        .visibility(false)
        .activates_default(true)
        .width_chars(30)
        .input_purpose(gtk4::InputPurpose::Password)
        .build();
    let show = CheckButton::with_mnemonic("Passwort _anzeigen");
    show.connect_toggled(glib::clone!(
        #[weak]
        entry,
        move |show| entry.set_visibility(show.is_active())
    ));
    (entry, show)
}

/// Asks for the key of `prompt`; `answer` gets it, or `None` when the user
/// cancels or closes the window, exactly once. The returned closer shuts
/// the window without answering again.
pub fn password(prompt: Prompt, answer: Answer) -> Close {
    let window = window(PASSWORD_TITLE);
    let body = content();
    let head = Label::new(None);
    head.set_markup(&format!(
        "<b>{}</b>",
        glib::markup_escape_text("Anmeldung im WLAN")
    ));
    head.set_xalign(0.0);
    body.append(&head);
    let text = Label::new(Some(&password_text(&prompt)));
    text.set_wrap(true);
    text.set_max_width_chars(46);
    text.set_xalign(0.0);
    body.append(&text);
    let security = Label::new(Some(&format!("Sicherheit: {}", prompt.security.label())));
    security.set_xalign(0.0);
    security.add_css_class("dim-label");
    body.append(&security);
    let (entry, show) = password_entry();
    let label = Label::with_mnemonic("_Passwort:");
    label.set_mnemonic_widget(Some(&entry));
    label.set_xalign(0.0);
    body.append(&label);
    body.append(&entry);
    body.append(&show);
    let cancel = add_dialog::stock_button("_Abbrechen", "process-stop");
    let connect = add_dialog::stock_button("_Verbinden", "network-wireless");
    connect.set_sensitive(false);
    entry.connect_changed(glib::clone!(
        #[weak]
        connect,
        move |entry| connect.set_sensitive(model::valid_wpa_key(&entry.text()))
    ));
    body.append(&buttons(&[&cancel, &connect]));
    window.set_child(Some(&body));
    window.set_default_widget(Some(&connect));

    let answer: Rc<Cell<Option<Answer>>> = Rc::new(Cell::new(Some(answer)));
    let ssid = prompt.ssid.clone();
    connect.connect_clicked(glib::clone!(
        #[weak]
        window,
        #[weak]
        entry,
        #[strong]
        answer,
        move |_| {
            let key = entry.text().to_string();
            if !model::valid_wpa_key(&key) {
                return;
            }
            if let Some(answer) = answer.take() {
                querencia_netzwerk::say!(
                    "querencia-panel: Netzwerk: Passwortdialog „{ssid}“: Verbinden ({} Zeichen)",
                    key.chars().count()
                );
                answer(Some(key));
            }
            window.close();
        }
    ));
    cancel.connect_clicked(glib::clone!(
        #[weak]
        window,
        move |_| window.close()
    ));
    let ssid = prompt.ssid.clone();
    window.connect_close_request(glib::clone!(
        #[strong]
        answer,
        move |_| {
            if let Some(answer) = answer.take() {
                querencia_netzwerk::say!(
                    "querencia-panel: Netzwerk: Passwortdialog „{ssid}“: abgebrochen"
                );
                answer(None);
            }
            glib::Propagation::Proceed
        }
    ));
    GtkWindowExt::set_focus(&window, Some(&entry));
    window.present();
    querencia_netzwerk::say!(
        "querencia-panel: Netzwerk: Dialog „{PASSWORD_TITLE}“ für „{}“ geöffnet{}",
        prompt.ssid,
        if prompt.retry { " (erneut)" } else { "" }
    );
    let weak = window.downgrade();
    Box::new(move || {
        // NetworkManager withdrew: no answer any more.
        answer.take();
        if let Some(window) = weak.upgrade() {
            window.close();
        }
    })
}

/// Lets the secret agent of the service ask through [`password`].
pub fn install_prompt(service: &Rc<Service>) {
    service.set_prompt(Rc::new(|prompt, answer| password(prompt, answer)));
}

/// The choices of the security list of [`hidden`], in order.
pub const HIDDEN_SECURITY: [Security; 3] = [Security::Open, Security::WpaPsk, Security::Sae];

/// Whether the hidden-network form can be sent.
pub fn hidden_ready(ssid: &str, security: Security, password: &str) -> bool {
    let ssid = ssid.trim();
    !ssid.is_empty() && ssid.len() <= 32 && (!security.secured() || model::valid_wpa_key(password))
}

/// „Mit verborgenem Netz verbinden“ for WLAN device `device`; raises the
/// open one in `slot`.
pub fn hidden(slot: &glib::WeakRef<Window>, service: &Rc<Service>, device: &str) {
    if let Some(window) = slot.upgrade() {
        window.present();
        return;
    }
    let window = window(HIDDEN_TITLE);
    let body = content();
    let text = Label::new(Some(
        "Name und Sicherheit des WLANs eingeben, das seinen Namen nicht sendet.",
    ));
    text.set_wrap(true);
    text.set_max_width_chars(46);
    text.set_xalign(0.0);
    body.append(&text);
    let grid = Grid::builder().row_spacing(6).column_spacing(10).build();
    let ssid = Entry::builder()
        .width_chars(28)
        .activates_default(true)
        .build();
    let names: Vec<&str> = HIDDEN_SECURITY.iter().map(|s| s.label()).collect();
    let security = DropDown::from_strings(&names);
    security.set_selected(1);
    let (key, show) = password_entry();
    let rows: [(&str, &gtk4::Widget); 3] = [
        ("_Netzname (SSID):", ssid.upcast_ref()),
        ("_Sicherheit:", security.upcast_ref()),
        ("_Passwort:", key.upcast_ref()),
    ];
    for (line, (text, widget)) in rows.iter().enumerate() {
        let label = Label::with_mnemonic(text);
        label.set_mnemonic_widget(Some(*widget));
        label.set_xalign(1.0);
        grid.attach(&label, 0, line as i32, 1, 1);
        grid.attach(*widget, 1, line as i32, 1, 1);
    }
    grid.attach(&show, 1, 3, 1, 1);
    body.append(&grid);
    let cancel = add_dialog::stock_button("_Abbrechen", "process-stop");
    let connect = add_dialog::stock_button("_Verbinden", "network-wireless");
    connect.set_sensitive(false);
    body.append(&buttons(&[&cancel, &connect]));
    window.set_child(Some(&body));
    window.set_default_widget(Some(&connect));

    let chosen = |dropdown: &DropDown| {
        HIDDEN_SECURITY
            .get(dropdown.selected() as usize)
            .copied()
            .unwrap_or(Security::Open)
    };
    let check = Rc::new(glib::clone!(
        #[weak]
        ssid,
        #[weak]
        security,
        #[weak]
        key,
        #[weak]
        show,
        #[weak]
        connect,
        move || {
            let kind = chosen(&security);
            key.set_sensitive(kind.secured());
            show.set_sensitive(kind.secured());
            connect.set_sensitive(hidden_ready(&ssid.text(), kind, &key.text()));
        }
    ));
    let c = check.clone();
    ssid.connect_changed(move |_| c());
    let c = check.clone();
    key.connect_changed(move |_| c());
    let c = check.clone();
    security.connect_selected_notify(move |_| c());
    check();
    let device = device.to_string();
    connect.connect_clicked(glib::clone!(
        #[weak]
        window,
        #[weak]
        ssid,
        #[weak]
        security,
        #[weak]
        key,
        #[strong]
        service,
        move |_| {
            let kind = chosen(&security);
            let name = ssid.text().trim().to_string();
            if !hidden_ready(&name, kind, &key.text()) {
                return;
            }
            let password = kind.secured().then(|| key.text().to_string());
            querencia_netzwerk::say!(
                "querencia-panel: Netzwerk: verborgenes Netz „{name}“, {}",
                kind.label()
            );
            service.connect_hidden(
                &device,
                &name,
                kind,
                password,
                report(&format!("WLAN „{name}“")),
            );
            window.close();
        }
    ));
    cancel.connect_clicked(glib::clone!(
        #[weak]
        window,
        move |_| window.close()
    ));
    GtkWindowExt::set_focus(&window, Some(&ssid));
    slot.set(Some(&window));
    window.present();
    querencia_netzwerk::say!("querencia-panel: Netzwerk: Dialog „{HIDDEN_TITLE}“ geöffnet");
}

#[cfg(test)]
mod tests {
    use super::*;
    use querencia_netzwerk::model::{Backend, Device, DeviceState, Kind};

    #[test]
    fn password_question_says_which_network_and_why_again() {
        let mut prompt = Prompt {
            ssid: "Heimnetz".into(),
            security: Security::WpaPsk,
            retry: false,
        };
        assert_eq!(
            password_text(&prompt),
            "Für das WLAN „Heimnetz“ ist ein Passwort nötig."
        );
        prompt.retry = true;
        assert!(password_text(&prompt).contains("nicht angenommen"));
    }

    #[test]
    fn hidden_network_needs_a_name_and_a_valid_key() {
        assert!(!hidden_ready("", Security::Open, ""));
        assert!(!hidden_ready("   ", Security::Open, ""));
        assert!(hidden_ready("Versteckt", Security::Open, ""));
        assert!(!hidden_ready("Versteckt", Security::WpaPsk, "kurz"));
        assert!(hidden_ready("Versteckt", Security::WpaPsk, "geheim123"));
        assert!(!hidden_ready(&"x".repeat(33), Security::Open, ""));
        assert_eq!(HIDDEN_SECURITY[1], Security::WpaPsk);
    }

    #[test]
    fn info_tabs_connected_first() {
        let mut wired = Device::new("/1", "eth0", Kind::Wired);
        wired.state = DeviceState::Unavailable;
        let mut wifi = Device::new("/2", "wlan0", Kind::Wireless);
        wifi.state = DeviceState::Connected;
        let state = State {
            backend: Backend::Fallback { networkd: true },
            devices: vec![wired, wifi],
            ..State::starting()
        };
        let order: Vec<&str> = info_order(&state)
            .iter()
            .map(|d| d.interface.as_str())
            .collect();
        assert_eq!(order, ["wlan0", "eth0"]);
    }
}
