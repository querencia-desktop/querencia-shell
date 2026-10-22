//! The network applet (G2-M), after the GNOME 2 NetworkManager applet: an
//! icon of the connection in the panel (own Querencia icons: cable
//! connected or not, WLAN in four signal stages, WLAN off, connecting, no
//! network, no network service), the tooltip with connection and address.
//!
//! - left click: a menu in the classic look ([`rows`]): the cable devices
//!   with their connection and address, the WLANs in reach with signal and
//!   lock, „Mit verborgenem Netz verbinden …“, „WLAN aktiviert“,
//!   „Flugmodus“, „Verbindungsinformationen …“, „Netzwerkeinstellungen …“;
//! - right click (through `panel_menu.rs`, [`menu`]): „WLAN aktiviert“,
//!   „Flugmodus“, „Verbindungsinformationen …“, „Netzwerkeinstellungen …“,
//!   „Aus dem Panel entfernen“.
//!
//! Everything goes through the process-wide service of
//! `querencia_netzwerk::service`: NetworkManager when it runs (connecting,
//! WLAN keys through its secret agent and the password dialog here), else
//! the read-only fallback, whose menu says „Verwaltet von systemd-networkd,
//! nur Anzeige“ and offers nothing that would change the network.
//!
//! „Netzwerkeinstellungen …“ starts the desktop action `netzwerk` of
//! Querencia Einstellungen (`org.querencia.Einstellungen.desktop`, Exec
//! `querencia-einstellungen --netzwerk`); while Einstellungen has no such
//! page the entry is greyed out as „Netzwerkeinstellungen (noch nicht
//! verfügbar)“.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gio_unix::DesktopAppInfo;
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, EventControllerKey, Image, Label, Orientation, Popover,
    PropagationPhase, Separator, Widget, gdk, gio, glib,
};

use querencia_netzwerk::icons::{self, Small};
use querencia_netzwerk::model::{self, AccessPoint, DeviceState, Icon, Kind, State};
use querencia_netzwerk::service::{self, Service};

use crate::i18n::{gettext, gettext_f};
use crate::{logo, network_dialogs};

/// CSS class of the applet's button. Its look is the one of all status
/// applets (`panel::STATUS_CLASS`, style in `main.rs`).
pub const CLASS: &str = "querencia-network";
/// CSS class while there is no network data at all.
pub const UNAVAILABLE_CLASS: &str = "unavailable";
/// CSS class of the left-click menu (with the classic menu's
/// `querencia-menu`).
pub const MENU_CLASS: &str = "querencia-network-menu";
/// Action group of the right-click menu.
pub const ACTION_GROUP: &str = "network";
const WIRELESS: &str = "wireless";
const AIRPLANE: &str = "airplane";
const INFO: &str = "info";
const SETTINGS: &str = "settings";
/// Desktop ID of Querencia Einstellungen and the action of its network
/// page (not there yet in Einstellungen 0.1.4).
pub const SETTINGS_APP: &str = "org.querencia.Einstellungen.desktop";
pub const SETTINGS_ACTION: &str = "netzwerk";
/// WLANs shown before „Weitere Netze …“.
pub const MAX_NETWORKS: usize = 8;
const ROW_ICON: i32 = 16;
/// CSS class of the row chosen with the keyboard, as in the classic menus.
const ACTIVE_CLASS: &str = crate::classic_menu::ACTIVE_CLASS;
const LOG: &str = "querencia-panel";

/// Whether the network page of Querencia Einstellungen exists.
pub fn settings_available() -> bool {
    DesktopAppInfo::new(SETTINGS_APP)
        .is_some_and(|app| app.list_actions().iter().any(|a| a == SETTINGS_ACTION))
}

fn settings_label(available: bool) -> String {
    if available {
        gettext("_Network Settings…")
    } else {
        gettext("Network Settings (not available yet)")
    }
}

/// The right-click menu (see `panel_menu.rs`).
pub fn menu() -> gio::Menu {
    let menu = gio::Menu::new();
    menu.append(
        Some(&gettext("_Wireless Enabled")),
        Some(&format!("{ACTION_GROUP}.{WIRELESS}")),
    );
    menu.append(
        Some(&gettext("_Airplane Mode")),
        Some(&format!("{ACTION_GROUP}.{AIRPLANE}")),
    );
    let more = gio::Menu::new();
    more.append(
        Some(&gettext("Connection _Information…")),
        Some(&format!("{ACTION_GROUP}.{INFO}")),
    );
    more.append(
        Some(&settings_label(settings_available())),
        Some(&format!("{ACTION_GROUP}.{SETTINGS}")),
    );
    menu.append_section(None, &more);
    let object = gio::Menu::new();
    object.append(
        Some(&gettext("Remove From Panel")),
        Some(crate::panel_menu::REMOVE_ACTION),
    );
    menu.append_section(None, &object);
    menu
}

/// What a row of the left-click menu does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Connect WLAN device `device` through the access point.
    ConnectAp {
        device: String,
        ap: AccessPoint,
    },
    /// Activate the best profile on a cable device.
    ConnectDevice(String),
    Disconnect(String),
    /// „Mit verborgenem Netz verbinden …“ on WLAN device.
    Hidden(String),
    SetWireless(bool),
    SetAirplane(bool),
    Info,
    Settings,
    /// Show the WLANs beyond [`MAX_NETWORKS`].
    MoreNetworks,
}

/// Icon of a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowIcon {
    Network(Icon),
    Check(bool),
}

/// One row of the left-click menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Row {
    Separator,
    /// Bold group title („Kabelnetzwerk“, „WLAN-Netze“).
    Header(String),
    /// Grey, not selectable.
    Note(String),
    Item {
        label: String,
        /// Second line (address of a connection).
        detail: Option<String>,
        icon: Option<RowIcon>,
        bold: bool,
        lock: bool,
        /// `None`: shown insensitive.
        action: Option<Action>,
    },
}

impl Row {
    fn item(label: &str, action: Option<Action>) -> Row {
        Row::Item {
            label: label.to_string(),
            detail: None,
            icon: None,
            bold: false,
            lock: false,
            action,
        }
    }

    fn check(label: &str, on: bool, action: Option<Action>) -> Row {
        Row::Item {
            label: label.to_string(),
            detail: None,
            icon: Some(RowIcon::Check(on)),
            bold: false,
            lock: false,
            action,
        }
    }

    #[cfg(test)]
    pub fn label(&self) -> &str {
        match self {
            Row::Separator => "",
            Row::Header(text) | Row::Note(text) => text,
            Row::Item { label, .. } => label,
        }
    }
}

/// The rows of the left-click menu for `state`. `settings` tells whether
/// the network settings exist; `all_networks` shows every WLAN.
pub fn rows(state: &State, settings: bool, all_networks: bool) -> Vec<Row> {
    let mut rows = Vec::new();
    let control = state.backend.can_control();
    match &state.backend {
        model::Backend::Starting => {
            rows.push(Row::Note(gettext("Reading network…")));
        }
        model::Backend::Unavailable(reason) => {
            rows.push(Row::Note(gettext("No network service")));
            if !reason.is_empty() {
                rows.push(Row::Note(reason.clone()));
            }
        }
        _ => {}
    }
    if let Some(note) = state.backend.read_only_note() {
        rows.push(Row::Note(note.to_string()));
    }
    if !state.networking_enabled {
        rows.push(Row::Note(gettext("Networking is disabled")));
    }
    let wired: Vec<_> = state
        .devices
        .iter()
        .filter(|d| d.kind == Kind::Wired)
        .collect();
    let wireless: Vec<_> = state
        .devices
        .iter()
        .filter(|d| d.kind == Kind::Wireless)
        .collect();
    for device in &wired {
        if !rows.is_empty() {
            rows.push(Row::Separator);
        }
        rows.push(Row::Header(if wired.len() > 1 {
            gettext_f(
                "Wired Network ({interface})",
                &[("interface", &device.interface)],
            )
        } else {
            gettext("Wired Network")
        }));
        if device.connected() {
            rows.push(Row::Item {
                label: device
                    .connection
                    .clone()
                    .unwrap_or_else(|| device.interface.clone()),
                detail: device.main_address().map(|a| a.to_string()),
                icon: Some(RowIcon::Network(Icon::WiredConnected)),
                bold: true,
                lock: false,
                action: None,
            });
            if control {
                rows.push(Row::item(
                    &gettext("Disconnect"),
                    Some(Action::Disconnect(device.id.clone())),
                ));
            }
        } else if device.carrier == Some(false) || device.state == DeviceState::Unavailable {
            rows.push(Row::Note(device.state_text().to_string()));
        } else if device.state == DeviceState::Connecting {
            rows.push(Row::Note(gettext("Connecting…")));
        } else if control {
            rows.push(Row::item(
                &gettext("Connect"),
                Some(Action::ConnectDevice(device.id.clone())),
            ));
        } else {
            rows.push(Row::Note(device.state_text().to_string()));
        }
    }
    for device in &wireless {
        if !rows.is_empty() {
            rows.push(Row::Separator);
        }
        rows.push(Row::Header(if wireless.len() > 1 {
            gettext_f(
                "Wireless Networks ({interface})",
                &[("interface", &device.interface)],
            )
        } else {
            gettext("Wireless Networks")
        }));
        if state.airplane() == Some(true) {
            rows.push(Row::Note(gettext("Airplane mode is on")));
            continue;
        }
        if state.wireless_enabled == Some(false) {
            rows.push(Row::Note(gettext("Wireless is disabled")));
            continue;
        }
        if !state.wireless_hardware_enabled {
            rows.push(Row::Note(gettext(
                "Wireless is disabled by a hardware switch",
            )));
            continue;
        }
        let list = model::networks(device);
        if !control {
            // Fallback: the kernel tells no network names in reach.
            if device.connected() {
                rows.push(Row::Item {
                    label: device
                        .ssid
                        .clone()
                        .unwrap_or_else(|| device.interface.clone()),
                    detail: device.main_address().map(|a| a.to_string()),
                    icon: Some(RowIcon::Network(Icon::Wireless(model::signal_stage(
                        device.signal.unwrap_or(0),
                    )))),
                    bold: true,
                    lock: false,
                    action: None,
                });
            } else {
                rows.push(Row::Note(device.state_text().to_string()));
            }
            continue;
        }
        if device.connected() && !list.iter().any(|ap| ap.active) {
            // A hidden network: its access point has no name in the list.
            rows.push(Row::Item {
                label: device
                    .ssid
                    .clone()
                    .or_else(|| device.connection.clone())
                    .unwrap_or_else(|| device.interface.clone()),
                detail: device.main_address().map(|a| a.to_string()),
                icon: Some(RowIcon::Network(Icon::Wireless(model::signal_stage(
                    device.signal.unwrap_or(0),
                )))),
                bold: true,
                lock: false,
                action: None,
            });
        } else if list.is_empty() {
            rows.push(Row::Note(gettext("No networks found")));
        }
        let shown = if all_networks {
            list.len()
        } else {
            list.len().min(MAX_NETWORKS)
        };
        for ap in &list[..shown] {
            let usable = ap.known.is_some() || ap.security.supported();
            rows.push(Row::Item {
                label: ap.ssid.clone(),
                detail: if ap.active {
                    device.main_address().map(|a| a.to_string()).or_else(|| {
                        (device.state == DeviceState::Connecting).then(|| gettext("connecting…"))
                    })
                } else if !usable {
                    Some(gettext_f(
                        "{security} – not supported",
                        &[("security", &ap.security.label())],
                    ))
                } else {
                    None
                },
                icon: Some(RowIcon::Network(Icon::Wireless(model::signal_stage(
                    ap.strength,
                )))),
                bold: ap.active,
                lock: ap.security.secured(),
                action: (!ap.active && usable).then(|| Action::ConnectAp {
                    device: device.id.clone(),
                    ap: ap.clone(),
                }),
            });
        }
        if shown < list.len() {
            rows.push(Row::item(
                &gettext("More Networks…"),
                Some(Action::MoreNetworks),
            ));
        }
        if device.connected() || device.state == DeviceState::Connecting {
            rows.push(Row::item(
                &gettext("Disconnect"),
                Some(Action::Disconnect(device.id.clone())),
            ));
        }
        rows.push(Row::item(
            &gettext("Connect to Hidden Network…"),
            Some(Action::Hidden(device.id.clone())),
        ));
    }
    if state.devices.is_empty()
        && !matches!(
            state.backend,
            model::Backend::Starting | model::Backend::Unavailable(_)
        )
    {
        rows.push(Row::Note(gettext("No network device")));
    }
    rows.push(Row::Separator);
    let hardware = state.wireless_hardware_enabled;
    if state.has_wireless() || control {
        let on = state.wireless_enabled.unwrap_or(false) && hardware;
        rows.push(Row::check(
            &gettext("Wireless Enabled"),
            on,
            (control && hardware && state.has_wireless()).then_some(Action::SetWireless(!on)),
        ));
    }
    let airplane = state.airplane() == Some(true);
    rows.push(Row::check(
        &gettext("Airplane Mode"),
        airplane,
        control.then_some(Action::SetAirplane(!airplane)),
    ));
    rows.push(Row::Separator);
    rows.push(Row::item(
        &gettext("Connection Information…"),
        (!state.devices.is_empty()).then_some(Action::Info),
    ));
    rows.push(Row::item(
        settings_label(settings).trim_start_matches('_'),
        settings.then_some(Action::Settings),
    ));
    rows
}

/// The applet for a panel `panel_height` pixels high.
pub fn widget(panel_height: i32) -> Button {
    let size = logo::size_for_panel(panel_height);
    let image = Image::new();
    image.set_pixel_size(size);
    let button = Button::builder().has_frame(false).child(&image).build();
    button.add_css_class(CLASS);
    let service = service::get(LOG);
    network_dialogs::install_prompt(&service);

    let popup = Popup::new(&service);
    popup.popover.set_parent(&button);
    button.connect_clicked(glib::clone!(
        #[strong]
        popup,
        #[strong]
        service,
        move |_| {
            popup.all_networks.set(false);
            popup.fill(&service.state());
            popup.popover.popup();
            querencia_netzwerk::say!("querencia-panel: Netzwerk: Menü geöffnet");
            service.request_scan();
        }
    ));

    // Actions of the right-click menu (opened by panel_menu.rs).
    let wireless = gio::SimpleAction::new_stateful(WIRELESS, None, &false.to_variant());
    wireless.connect_change_state(glib::clone!(
        #[strong]
        service,
        move |_, value| {
            if let Some(on) = value.and_then(|v| v.get::<bool>()) {
                querencia_netzwerk::say!(
                    "querencia-panel: Netzwerk: Appletmenü → WLAN {}",
                    if on { "an" } else { "aus" }
                );
                service.set_wireless(on, network_dialogs::report(&gettext("Wireless")));
            }
        }
    ));
    let airplane = gio::SimpleAction::new_stateful(AIRPLANE, None, &false.to_variant());
    airplane.connect_change_state(glib::clone!(
        #[strong]
        service,
        move |_, value| {
            if let Some(on) = value.and_then(|v| v.get::<bool>()) {
                querencia_netzwerk::say!(
                    "querencia-panel: Netzwerk: Appletmenü → Flugmodus {}",
                    if on { "an" } else { "aus" }
                );
                service.set_airplane(on, network_dialogs::report(&gettext("Airplane Mode")));
            }
        }
    ));
    let info = gio::SimpleAction::new(INFO, None);
    let info_slot = Rc::new(glib::WeakRef::<gtk4::Window>::new());
    info.connect_activate(glib::clone!(
        #[strong]
        service,
        #[strong]
        info_slot,
        move |_, _| network_dialogs::info(&info_slot, &service)
    ));
    let settings = gio::SimpleAction::new(SETTINGS, None);
    settings.connect_activate(|_, _| open_settings());
    let actions = gio::SimpleActionGroup::new();
    actions.add_action(&wireless);
    actions.add_action(&airplane);
    actions.add_action(&info);
    actions.add_action(&settings);
    button.insert_action_group(ACTION_GROUP, Some(&actions));
    popup.info_slot.replace(Some(info_slot));

    let id = service.subscribe(glib::clone!(
        #[weak]
        button,
        #[weak]
        image,
        #[strong]
        popup,
        #[strong]
        wireless,
        #[strong]
        airplane,
        #[strong]
        info,
        #[strong]
        settings,
        move |state| {
            show(&button, &image, size, state);
            let control = state.backend.can_control();
            wireless.set_state(&(state.wireless_enabled == Some(true)).to_variant());
            wireless
                .set_enabled(control && state.has_wireless() && state.wireless_hardware_enabled);
            airplane.set_state(&(state.airplane() == Some(true)).to_variant());
            airplane.set_enabled(control);
            info.set_enabled(!state.devices.is_empty());
            settings.set_enabled(settings_available());
            if popup.popover.is_visible() {
                popup.fill(state);
            }
        }
    ));
    image.connect_scale_factor_notify(glib::clone!(
        #[strong]
        service,
        #[weak]
        button,
        move |image| show(&button, image, size, &service.state())
    ));
    // QD-081: the icon follows the dark look at once
    crate::symbol_farbe::bei_wechsel(
        &button,
        glib::clone!(
            #[strong]
            service,
            #[weak]
            button,
            #[weak]
            image,
            move || show(&button, &image, size, &service.state())
        ),
    );
    // Querencia Einstellungen with its network page may be installed or
    // removed while the panel runs: follow the installed
    // applications, as the menus do.
    let monitor = gio::AppInfoMonitor::get();
    let installed = Cell::new(Some(monitor.connect_changed(glib::clone!(
        #[strong]
        settings,
        move |_| settings.set_enabled(settings_available())
    ))));
    button.connect_destroy(glib::clone!(
        #[strong]
        service,
        #[strong]
        popup,
        move |_| {
            if let Some(handler) = installed.take() {
                monitor.disconnect(handler);
            }
            service.unsubscribe(id);
            if popup.popover.parent().is_some() {
                popup.popover.unparent();
            }
        }
    ));
    button
}

/// The applet's icon (cable connected), `size` pixels square, for the „Zum
/// Panel hinzufügen“ dialog.
pub fn image_of_size(size: i32) -> Image {
    let image = Image::new();
    image.set_pixel_size(size);
    let draw = move |image: &Image| {
        if let Some(texture) = symbol(Icon::WiredConnected, size * image.scale_factor().max(1)) {
            image.set_paintable(Some(&texture));
        }
    };
    draw(&image);
    image.connect_scale_factor_notify(draw);
    image
}

/// The icon `icon`, recoloured on the dark panel and in its dark menu
/// (QD-081).
fn symbol(icon: Icon, pixels: i32) -> Option<gtk4::gdk::Texture> {
    crate::symbol_farbe::texture(icons::svg(icon), pixels, || icons::texture(icon, pixels))
}

/// The small menu icon `icon`, recoloured like [`symbol`].
fn kleines_symbol(icon: Small, pixels: i32) -> Option<gtk4::gdk::Texture> {
    crate::symbol_farbe::texture(icons::small_svg(icon), pixels, || {
        icons::small_texture(icon, pixels)
    })
}

/// Shows `state` on the applet: icon, tooltip, dimmed without data.
fn show(button: &Button, image: &Image, size: i32, state: &State) {
    let icon = model::icon(state);
    let pixels = size * image.scale_factor().max(1);
    match symbol(icon, pixels) {
        Some(texture) => image.set_paintable(Some(&texture)),
        None => crate::symbole::set_named(image, "network-wired"),
    }
    let tooltip = model::tooltip(state);
    if button.tooltip_text().as_deref() != Some(tooltip.as_str()) {
        querencia_netzwerk::say!(
            "querencia-panel: Netzwerk: Tooltip „{}“",
            tooltip.replace('\n', " | ")
        );
        button.set_tooltip_text(Some(&tooltip));
    }
    if icon == Icon::NoService {
        button.add_css_class(UNAVAILABLE_CLASS);
    } else {
        button.remove_css_class(UNAVAILABLE_CLASS);
    }
}

/// `QUERENCIA_NETZWERK_DEBUG=1`: every built menu is logged row by row
/// (the isolated GUI test checks it).
fn debug() -> bool {
    std::env::var_os("QUERENCIA_NETZWERK_DEBUG").is_some_and(|v| v == "1")
}

/// A row as one log word group: `[Titel]`, `(Hinweis)`, `Eintrag`, a
/// greyed entry with ` –`, a lock with ` 🔒`, a bold one with `*`, the
/// second line in `‹…›`.
pub fn row_text(row: &Row) -> String {
    match row {
        Row::Separator => "—".to_string(),
        Row::Header(text) => format!("[{text}]"),
        Row::Note(text) => format!("({text})"),
        Row::Item {
            label,
            detail,
            icon,
            bold,
            lock,
            action,
        } => {
            let mut text = String::new();
            if let Some(RowIcon::Check(on)) = icon {
                text.push_str(if *on { "☑ " } else { "☐ " });
            }
            if let Some(RowIcon::Network(Icon::Wireless(stage))) = icon {
                text.push_str(&format!("{{{stage}}} "));
            }
            text.push_str(label);
            if *bold {
                text.push('*');
            }
            if *lock {
                text.push_str(" 🔒");
            }
            if let Some(detail) = detail {
                text.push_str(&format!(" ‹{detail}›"));
            }
            if action.is_none() {
                text.push_str(" –");
            }
            text
        }
    }
}

/// Starts the network page of Querencia Einstellungen through GIO.
fn open_settings() {
    let Some(app) = DesktopAppInfo::new(SETTINGS_APP) else {
        querencia_netzwerk::say!("querencia-panel: Netzwerk: {SETTINGS_APP} fehlt");
        return;
    };
    if !app.list_actions().iter().any(|a| a == SETTINGS_ACTION) {
        querencia_netzwerk::say!(
            "querencia-panel: Netzwerk: {SETTINGS_APP} hat keine Aktion {SETTINGS_ACTION} (noch nicht verfügbar)"
        );
        return;
    }
    let context = gdk::Display::default().map(|display| display.app_launch_context());
    if let Some(context) = &context {
        crate::portal::for_children(context);
    }
    querencia_netzwerk::say!(
        "querencia-panel: Netzwerk: Netzwerkeinstellungen über {SETTINGS_APP}"
    );
    app.launch_action(SETTINGS_ACTION, context.as_ref());
}

/// The left-click menu.
struct Popup {
    popover: Popover,
    list: GtkBox,
    service: Rc<Service>,
    all_networks: Cell<bool>,
    /// Row chosen with the keyboard (index into `buttons`).
    selected: Cell<Option<usize>>,
    /// Buttons of the selectable rows, top to bottom, with their keys.
    buttons: RefCell<Vec<(String, Button)>>,
    info_slot: RefCell<Option<Rc<glib::WeakRef<gtk4::Window>>>>,
    hidden_slot: Rc<glib::WeakRef<gtk4::Window>>,
}

impl Popup {
    fn new(service: &Rc<Service>) -> Rc<Popup> {
        let list = GtkBox::new(Orientation::Vertical, 0);
        list.add_css_class("qnet");
        let popover = Popover::builder().has_arrow(false).child(&list).build();
        popover.add_css_class(crate::classic_menu::MENU_CLASS);
        popover.add_css_class(MENU_CLASS);
        popover.set_halign(Align::End);
        let popup = Rc::new(Popup {
            popover,
            list,
            service: service.clone(),
            all_networks: Cell::new(false),
            selected: Cell::new(None),
            buttons: RefCell::new(Vec::new()),
            info_slot: RefCell::new(None),
            hidden_slot: Rc::new(glib::WeakRef::new()),
        });
        // Keyboard as in a GTK 2 menu: Up/Down/Home/End move between the
        // selectable rows, Enter or Space activate, Escape closes. The
        // popover grabs, so labwc gives it the keyboard.
        let keys = EventControllerKey::new();
        keys.set_propagation_phase(PropagationPhase::Capture);
        let weak = Rc::downgrade(&popup);
        keys.connect_key_pressed(move |_, key, _, _| {
            let Some(popup) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            popup.on_key(key)
        });
        popup.popover.add_controller(keys);
        let weak = Rc::downgrade(&popup);
        popup.popover.connect_closed(move |_| {
            if let Some(popup) = weak.upgrade() {
                popup.all_networks.set(false);
                // A new opening starts without a keyboard row, as GTK 2.
                popup.select(None);
                querencia_netzwerk::say!("querencia-panel: Netzwerk: Menü geschlossen");
            }
        });
        popup
    }

    fn on_key(&self, key: gdk::Key) -> glib::Propagation {
        let count = self.buttons.borrow().len();
        if count == 0 {
            return glib::Propagation::Proceed;
        }
        let current = self.selected.get().filter(|&i| i < count);
        let target = match key {
            gdk::Key::Down | gdk::Key::KP_Down => Some(current.map_or(0, |i| (i + 1) % count)),
            gdk::Key::Up | gdk::Key::KP_Up => {
                Some(current.map_or(count - 1, |i| (i + count - 1) % count))
            }
            gdk::Key::Home => Some(0),
            gdk::Key::End => Some(count - 1),
            gdk::Key::Return | gdk::Key::KP_Enter | gdk::Key::space => {
                if let Some(i) = current {
                    let button = self.buttons.borrow()[i].1.clone();
                    button.emit_clicked();
                    return glib::Propagation::Stop;
                }
                None
            }
            _ => None,
        };
        match target {
            Some(i) => {
                self.select(Some(i));
                querencia_netzwerk::say!(
                    "querencia-panel: Netzwerk: Menü, Tastatur: Zeile {} von {count} ({})",
                    i + 1,
                    self.buttons.borrow()[i].0
                );
                glib::Propagation::Stop
            }
            None => glib::Propagation::Proceed,
        }
    }

    /// Highlights row `index` of the selectable rows (keyboard), as the
    /// classic menus do with `qmenu-active`.
    fn select(&self, index: Option<usize>) {
        self.selected.set(index);
        for (i, (_, button)) in self.buttons.borrow().iter().enumerate() {
            if Some(i) == index {
                button.add_css_class(ACTIVE_CLASS);
                button.grab_focus();
            } else {
                button.remove_css_class(ACTIVE_CLASS);
            }
        }
    }

    /// Builds the rows for `state`, keeping the keyboard on the same row.
    fn fill(self: &Rc<Self>, state: &State) {
        let focused = self
            .selected
            .get()
            .and_then(|i| self.buttons.borrow().get(i).map(|(k, _)| k.clone()));
        self.selected.set(None);
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        self.buttons.borrow_mut().clear();
        let rows = rows(state, settings_available(), self.all_networks.get());
        if debug() {
            querencia_netzwerk::say!(
                "querencia-panel: Netzwerk: Menü: {}",
                rows.iter().map(row_text).collect::<Vec<_>>().join(" | ")
            );
        }
        let mut reselect = None;
        for (index, row) in rows.into_iter().enumerate() {
            let widget: Widget = match row {
                Row::Separator => Separator::new(Orientation::Horizontal).upcast(),
                Row::Header(text) => {
                    let label = Label::new(Some(&text));
                    label.add_css_class("qnet-header");
                    label.set_xalign(0.0);
                    label.upcast()
                }
                Row::Note(text) => {
                    let label = Label::new(Some(&text));
                    label.add_css_class("qnet-note");
                    label.set_xalign(0.0);
                    label.set_wrap(true);
                    label.set_max_width_chars(40);
                    label.upcast()
                }
                Row::Item {
                    label,
                    detail,
                    icon,
                    bold,
                    lock,
                    action,
                } => {
                    let button = self.item_button(
                        &label,
                        detail.as_deref(),
                        icon,
                        bold,
                        lock,
                        action.clone(),
                    );
                    if action.is_some() {
                        let key = format!("{index}:{label}");
                        if focused.as_deref() == Some(key.as_str()) {
                            reselect = Some(self.buttons.borrow().len());
                        }
                        self.buttons.borrow_mut().push((key, button.clone()));
                    }
                    button.upcast()
                }
            };
            self.list.append(&widget);
        }
        if reselect.is_some() {
            self.select(reselect);
        }
    }

    fn item_button(
        self: &Rc<Self>,
        label: &str,
        detail: Option<&str>,
        icon: Option<RowIcon>,
        bold: bool,
        lock: bool,
        action: Option<Action>,
    ) -> Button {
        let row = GtkBox::new(Orientation::Horizontal, 6);
        let picture = Image::new();
        picture.set_pixel_size(ROW_ICON);
        picture.add_css_class("qnet-icon");
        let scale = self.popover.scale_factor().max(1);
        let texture = match icon {
            Some(RowIcon::Network(icon)) => symbol(icon, ROW_ICON * scale),
            Some(RowIcon::Check(on)) => kleines_symbol(
                if on { Small::Checked } else { Small::Unchecked },
                ROW_ICON * scale,
            ),
            None => None,
        };
        if let Some(texture) = texture {
            picture.set_paintable(Some(&texture));
        }
        // On the orange highlight the orange WLAN arcs would vanish: a
        // white variant lies on top, shown only there (CSS opacity).
        let icon_widget = gtk4::Overlay::new();
        icon_widget.set_child(Some(&picture));
        if let Some(RowIcon::Network(icon)) = icon
            && let Some(light) = icons::highlight_texture(icon, ROW_ICON * scale)
        {
            let over = Image::from_paintable(Some(&light));
            over.set_pixel_size(ROW_ICON);
            over.add_css_class("qnet-icon-hell");
            picture.add_css_class("qnet-icon-normal");
            icon_widget.add_overlay(&over);
        }
        row.append(&icon_widget);
        let texts = GtkBox::new(Orientation::Vertical, 0);
        let title = Label::new(None);
        if bold {
            title.set_markup(&format!("<b>{}</b>", glib::markup_escape_text(label)));
        } else {
            title.set_text(label);
        }
        title.set_xalign(0.0);
        texts.append(&title);
        if let Some(detail) = detail {
            let second = Label::new(Some(detail));
            second.add_css_class("qnet-detail");
            second.set_xalign(0.0);
            texts.append(&second);
        }
        texts.set_hexpand(true);
        row.append(&texts);
        if lock {
            let padlock = Image::new();
            padlock.set_pixel_size(ROW_ICON);
            if let Some(texture) = kleines_symbol(Small::Lock, ROW_ICON * scale) {
                padlock.set_paintable(Some(&texture));
            }
            padlock.set_valign(Align::Center);
            padlock.set_tooltip_text(Some(&gettext("Secured")));
            row.append(&padlock);
        }
        let button = Button::builder().child(&row).has_frame(false).build();
        button.add_css_class("qnet-item");
        if action.is_none() && bold {
            // The current connection: information, not a choice; shown in
            // normal colours, never highlighted.
            button.add_css_class("qnet-current");
            button.set_can_target(false);
            button.set_focusable(false);
            return button;
        }
        // The pointer takes over from the keyboard, as in GTK 2 menus.
        let motion = gtk4::EventControllerMotion::new();
        let weak = Rc::downgrade(self);
        motion.connect_enter(move |_, _, _| {
            if let Some(popup) = weak.upgrade()
                && popup.selected.get().is_some()
            {
                popup.select(None);
            }
        });
        button.add_controller(motion);
        button.set_sensitive(action.is_some());
        if let Some(action) = action {
            let weak = Rc::downgrade(self);
            let name = label.to_string();
            button.connect_clicked(move |_| {
                if let Some(popup) = weak.upgrade() {
                    querencia_netzwerk::say!("querencia-panel: Netzwerk: Menü → {name}");
                    popup.activate(&action);
                }
            });
        }
        button
    }

    fn activate(self: &Rc<Self>, action: &Action) {
        if *action == Action::MoreNetworks {
            self.all_networks.set(true);
            self.fill(&self.service.state());
            return;
        }
        self.popover.popdown();
        let service = &self.service;
        match action {
            Action::ConnectAp { device, ap } => {
                let report = network_dialogs::report(&gettext_f(
                    "Wireless network “{ssid}”",
                    &[("ssid", &ap.ssid)],
                ));
                if ap.known.is_none() && ap.security.secured() {
                    let service = service.clone();
                    let device = device.clone();
                    let target = ap.clone();
                    // A dialog of our own, nothing to withdraw.
                    let _close = network_dialogs::password(
                        querencia_netzwerk::service::Prompt {
                            ssid: ap.ssid.clone(),
                            security: ap.security,
                            retry: false,
                        },
                        Box::new(move |answer| {
                            if let Some(password) = answer {
                                service.connect_wifi(&device, &target, Some(password), report);
                            }
                        }),
                    );
                } else {
                    service.connect_wifi(device, ap, None, report);
                }
            }
            Action::ConnectDevice(device) => {
                service.connect_device(device, network_dialogs::report(&gettext("Wired Network")));
            }
            Action::Disconnect(device) => {
                service.disconnect(device, network_dialogs::report(&gettext("Disconnect")));
            }
            Action::Hidden(device) => network_dialogs::hidden(&self.hidden_slot, service, device),
            Action::SetWireless(on) => {
                service.set_wireless(*on, network_dialogs::report(&gettext("Wireless")))
            }
            Action::SetAirplane(on) => {
                service.set_airplane(*on, network_dialogs::report(&gettext("Airplane Mode")));
            }
            Action::Info => {
                if let Some(slot) = self.info_slot.borrow().as_ref() {
                    network_dialogs::info(slot, service);
                }
            }
            Action::Settings => open_settings(),
            Action::MoreNetworks => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use querencia_netzwerk::model::{Address, Backend, Device, Security};
    use std::net::Ipv4Addr;

    fn label(menu: &gio::MenuModel, i: i32) -> Option<String> {
        menu.item_attribute_value(i, "label", Some(glib::VariantTy::STRING))
            .and_then(|v| v.get::<String>())
    }

    fn action(menu: &gio::MenuModel, i: i32) -> Option<String> {
        menu.item_attribute_value(i, "action", Some(glib::VariantTy::STRING))
            .and_then(|v| v.get::<String>())
    }

    #[test]
    fn right_click_menu_switches_shows_info_settings_and_removes() {
        let menu = menu().upcast::<gio::MenuModel>();
        assert_eq!(label(&menu, 0).as_deref(), Some("_Wireless Enabled"));
        assert_eq!(action(&menu, 0).as_deref(), Some("network.wireless"));
        assert_eq!(label(&menu, 1).as_deref(), Some("_Airplane Mode"));
        assert_eq!(action(&menu, 1).as_deref(), Some("network.airplane"));
        let more = menu.item_link(2, "section").expect("Abschnitt");
        assert_eq!(label(&more, 0).as_deref(), Some("Connection _Information…"));
        assert_eq!(action(&more, 0).as_deref(), Some("network.info"));
        assert_eq!(action(&more, 1).as_deref(), Some("network.settings"));
        let object = menu.item_link(3, "section").expect("Abschnitt");
        assert_eq!(action(&object, 0).as_deref(), Some("panel.applet-remove"));
    }

    #[test]
    fn settings_entry_says_it_is_not_there_yet() {
        assert_eq!(settings_label(true), "_Network Settings…");
        assert_eq!(
            settings_label(false),
            "Network Settings (not available yet)"
        );
    }

    fn device(kind: Kind, state: DeviceState) -> Device {
        let name = if kind == Kind::Wired { "eth0" } else { "wlan0" };
        let mut d = Device::new(&format!("/d/{name}"), name, kind);
        d.state = state;
        if kind == Kind::Wired {
            d.carrier = Some(state != DeviceState::Unavailable);
        }
        if state == DeviceState::Connected {
            d.ipv4 = vec![Address::v4(Ipv4Addr::new(192, 0, 2, 23), 24)];
        }
        d
    }

    fn ap(ssid: &str, strength: u8, security: Security) -> AccessPoint {
        AccessPoint {
            id: format!("/ap/{ssid}"),
            ssid: ssid.into(),
            strength,
            security,
            frequency_mhz: 2437,
            bssid: String::new(),
            active: false,
            known: None,
        }
    }

    fn nm(devices: Vec<Device>) -> State {
        State {
            backend: Backend::NetworkManager {
                version: "1.54".into(),
            },
            devices,
            wireless_enabled: Some(true),
            wwan_enabled: Some(true),
            ..State::starting()
        }
    }

    fn labels(rows: &[Row]) -> Vec<String> {
        rows.iter()
            .map(|r| match r {
                Row::Separator => "—".to_string(),
                Row::Header(t) => format!("[{t}]"),
                Row::Note(t) => format!("({t})"),
                Row::Item { label, action, .. } => {
                    format!("{label}{}", if action.is_some() { "" } else { " –" })
                }
            })
            .collect()
    }

    #[test]
    fn fallback_menu_only_shows() {
        let state = State {
            backend: Backend::Fallback { networkd: true },
            devices: vec![device(Kind::Wired, DeviceState::Connected)],
            ..State::starting()
        };
        let rows = rows(&state, false, false);
        assert_eq!(
            labels(&rows),
            [
                "(Managed by systemd-networkd, display only)",
                "—",
                "[Wired Network]",
                "eth0 –",
                "—",
                "Airplane Mode –",
                "—",
                &gettext("Connection Information…"),
                "Network Settings (not available yet) –"
            ]
        );
        match &rows[3] {
            Row::Item { detail, bold, .. } => {
                assert_eq!(detail.as_deref(), Some("192.0.2.23/24"));
                assert!(*bold);
            }
            other => panic!("{other:?}"),
        }
        // nothing in it changes the network
        assert!(rows.iter().all(|r| !matches!(
            r,
            Row::Item {
                action: Some(
                    Action::Disconnect(_)
                        | Action::ConnectAp { .. }
                        | Action::SetWireless(_)
                        | Action::SetAirplane(_)
                        | Action::Hidden(_)
                ),
                ..
            }
        )));
    }

    #[test]
    fn networkmanager_menu_lists_networks_with_lock_and_actions() {
        let mut wlan = device(Kind::Wireless, DeviceState::Disconnected);
        wlan.access_points = vec![
            ap("Heimnetz", 82, Security::WpaPsk),
            ap("Cafe offen", 45, Security::Open),
            ap("Firma", 60, Security::Enterprise),
        ];
        let state = nm(vec![device(Kind::Wired, DeviceState::Unavailable), wlan]);
        let rows = rows(&state, true, false);
        assert_eq!(
            labels(&rows),
            [
                "[Wired Network]",
                "(Cable unplugged)",
                "—",
                "[Wireless Networks]",
                "Heimnetz",
                "Firma –",
                "Cafe offen",
                &gettext("Connect to Hidden Network…"),
                "—",
                &gettext("Wireless Enabled"),
                "Airplane Mode",
                "—",
                &gettext("Connection Information…"),
                "Network Settings…"
            ]
        );
        let lock = |i: usize| matches!(&rows[i], Row::Item { lock: true, .. });
        assert!(lock(4) && lock(5) && !lock(6));
        match &rows[5] {
            Row::Item { detail, .. } => assert_eq!(
                detail.as_deref(),
                Some("WPA/WPA2 Enterprise – not supported")
            ),
            other => panic!("{other:?}"),
        }
        match &rows[9] {
            Row::Item { icon, action, .. } => {
                assert_eq!(*icon, Some(RowIcon::Check(true)));
                assert_eq!(*action, Some(Action::SetWireless(false)));
            }
            other => panic!("{other:?}"),
        }
        match &rows[10] {
            Row::Item { action, .. } => assert_eq!(*action, Some(Action::SetAirplane(true))),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn connected_network_first_bold_with_address_and_disconnect() {
        let mut wlan = device(Kind::Wireless, DeviceState::Connected);
        let mut home = ap("Heimnetz", 70, Security::WpaPsk);
        home.active = true;
        wlan.access_points = vec![ap("Nachbar", 90, Security::WpaPsk), home];
        let rows = rows(&nm(vec![wlan]), true, false);
        assert_eq!(
            labels(&rows)[..5],
            [
                "[Wireless Networks]",
                "Heimnetz –",
                "Nachbar",
                "Disconnect",
                "Connect to Hidden Network…"
            ]
        );
        match &rows[1] {
            Row::Item {
                bold, detail, icon, ..
            } => {
                assert!(*bold);
                assert_eq!(detail.as_deref(), Some("192.0.2.23/24"));
                assert_eq!(*icon, Some(RowIcon::Network(Icon::Wireless(2))));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn connected_hidden_network_is_the_current_row() {
        let mut wlan = device(Kind::Wireless, DeviceState::Connected);
        wlan.ssid = Some("Versteckt".into());
        wlan.signal = Some(70);
        let mut hidden = ap("", 70, Security::WpaPsk);
        hidden.active = true;
        wlan.access_points = vec![hidden, ap("Nachbar", 20, Security::WpaPsk)];
        let rows = rows(&nm(vec![wlan]), true, false);
        let text: Vec<String> = rows.iter().map(row_text).collect();
        assert_eq!(
            text[..4],
            [
                "[Wireless Networks]",
                "{2} Versteckt* ‹192.0.2.23/24› –",
                "{0} Nachbar 🔒",
                "Disconnect"
            ]
        );
    }

    #[test]
    fn many_networks_behind_more() {
        let mut wlan = device(Kind::Wireless, DeviceState::Disconnected);
        wlan.access_points = (0..12)
            .map(|i| ap(&format!("Netz {i:02}"), 90 - i as u8, Security::Open))
            .collect();
        let few = rows(&nm(vec![wlan.clone()]), true, false);
        let names = |rows: &[Row]| {
            rows.iter()
                .filter(|r| r.label().starts_with("Netz "))
                .count()
        };
        assert_eq!(names(&few), MAX_NETWORKS);
        assert!(few.iter().any(|r| r.label() == "More Networks…"));
        let all = rows(&nm(vec![wlan]), true, true);
        assert_eq!(names(&all), 12);
        assert!(!all.iter().any(|r| r.label() == "More Networks…"));
    }

    #[test]
    fn wlan_off_airplane_and_no_service() {
        let mut state = nm(vec![device(Kind::Wireless, DeviceState::Unavailable)]);
        state.wireless_enabled = Some(false);
        let r = labels(&rows(&state, false, false));
        assert!(r.contains(&"(Wireless is disabled)".to_string()), "{r:?}");
        assert!(r.contains(&"Wireless Enabled".to_string()));
        state.wwan_enabled = Some(false);
        let rows_air = rows(&state, false, false);
        assert!(labels(&rows_air).contains(&"(Airplane mode is on)".to_string()));
        assert!(rows_air.iter().any(|r| matches!(
            r,
            Row::Item {
                icon: Some(RowIcon::Check(true)),
                action: Some(Action::SetAirplane(false)),
                ..
            }
        )));
        let none = labels(&rows(&State::unavailable("kaputt"), false, false));
        assert_eq!(none[..2], ["(No network service)", "(kaputt)"]);
        assert!(none.contains(&"Connection Information… –".to_string()));
    }

    #[test]
    fn wired_with_cable_but_not_connected_offers_connect() {
        let state = nm(vec![device(Kind::Wired, DeviceState::Disconnected)]);
        let rows = rows(&state, true, false);
        assert!(rows.contains(&Row::item(
            "Connect",
            Some(Action::ConnectDevice("/d/eth0".into()))
        )));
    }

    #[test]
    fn rows_as_log_text() {
        let mut wlan = device(Kind::Wireless, DeviceState::Disconnected);
        wlan.access_points = vec![ap("Heimnetz", 82, Security::WpaPsk)];
        let rows = rows(&nm(vec![wlan]), false, false);
        let text: Vec<String> = rows.iter().map(row_text).collect();
        assert_eq!(text[0], "[Wireless Networks]");
        assert_eq!(text[1], "{3} Heimnetz 🔒");
        assert!(text.contains(&"☑ Wireless Enabled".to_string()));
        assert!(text.contains(&"Network Settings (not available yet) –".to_string()));
    }

    #[test]
    fn action_group_does_not_clash() {
        assert!(
            !["panel", "menu", "system", "session", "places", "volume"].contains(&ACTION_GROUP)
        );
    }
}
