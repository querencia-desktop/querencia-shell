//! The volume applet (G2-J), after the GNOME 2 volume control applet: an
//! icon of the output volume in the panel, in four levels (stumm, leise,
//! mittel, laut; own Querencia icons, `querencia_lautstaerke::icons`).
//!
//! - left click: a small popup with a vertical slider 0–100 %, the
//!   percentage and „Stumm“;
//! - mouse wheel over the icon: ±5 %;
//! - middle click: mute on and off;
//! - right click (through `panel_menu.rs`, [`menu`]): „Stumm“,
//!   „Lautstärkeregler öffnen …“, „Aus dem Panel entfernen“;
//! - tooltip: percent and device.
//!
//! It always acts on the default output device; the mixer
//! (`querencia-lautstaerke`) chooses devices. Everything goes through the
//! process-wide service of `querencia_lautstaerke::service`, which hears
//! every change from the audio system (no polling). Without an audio
//! service the icon is greyed out, the tooltip says „Kein Audiodienst“, and
//! the controls do nothing; the panel goes on as before.

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gio_unix::DesktopAppInfo;
use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, Button, CheckButton, EventControllerScroll, EventControllerScrollFlags,
    GestureClick, Image, Label, Orientation, Popover, Scale, gdk, gio, glib,
};

use querencia_lautstaerke::model::{self, Look};
use querencia_lautstaerke::service::{self, Service};
use querencia_lautstaerke::{MIXER_APP_ID, MIXER_PROGRAM, icons};

use crate::{app_menu, logo};

/// CSS class of the applet's button (style in `main.rs`).
pub const CLASS: &str = "querencia-volume";
/// CSS class while there is no audio service or no output device.
pub const UNAVAILABLE_CLASS: &str = "unavailable";
/// Action group of the applet's menu (`volume.mute`, `volume.mixer`).
pub const ACTION_GROUP: &str = "volume";
const MUTE: &str = "mute";
const MIXER: &str = "mixer";
/// Height of the slider in the popup.
const SLIDER_HEIGHT: i32 = 140;
/// After the user moved the slider, events from the service do not move it
/// for this long (a dragged slider must not jump back).
const HOLD: Duration = Duration::from_millis(600);
/// Log prefix of the shared audio service in the panel.
const LOG: &str = "querencia-panel";

/// The right-click menu (see `panel_menu.rs`): „Stumm“ as a check item,
/// „Lautstärkeregler öffnen …“, then the panel object entry.
pub fn menu() -> gio::Menu {
    let menu = gio::Menu::new();
    menu.append(Some("_Stumm"), Some(&format!("{ACTION_GROUP}.{MUTE}")));
    menu.append(
        Some("_Lautstärkeregler öffnen …"),
        Some(&format!("{ACTION_GROUP}.{MIXER}")),
    );
    let object = gio::Menu::new();
    object.append(
        Some("Aus dem Panel entfernen"),
        Some(crate::panel_menu::REMOVE_ACTION),
    );
    menu.append_section(None, &object);
    menu
}

/// The applet for a panel `panel_height` pixels high.
pub fn widget(panel_height: i32) -> Button {
    let size = logo::size_for_panel(panel_height);
    let image = Image::new();
    image.set_pixel_size(size);
    let button = Button::builder().has_frame(false).child(&image).build();
    button.add_css_class(CLASS);
    let service = service::get(LOG);

    let popup = Popup::new(&service);
    popup.popover.set_parent(&button);
    button.connect_clicked(glib::clone!(
        #[strong]
        popup,
        move |_| {
            popup.popover.popup();
            querencia_lautstaerke::say!("querencia-panel: Lautstärke: Regler geöffnet");
        }
    ));

    // Menu actions; the menu itself is opened by panel_menu.rs.
    let mute = gio::SimpleAction::new_stateful(MUTE, None, &false.to_variant());
    mute.connect_change_state(glib::clone!(
        #[strong]
        service,
        move |_, value| {
            if let Some(muted) = value.and_then(|v| v.get::<bool>()) {
                querencia_lautstaerke::say!(
                    "querencia-panel: Lautstärke: Menü → {}",
                    if muted { "stumm" } else { "laut" }
                );
                service.set_output_muted(muted);
            }
        }
    ));
    let mixer = gio::SimpleAction::new(MIXER, None);
    mixer.connect_activate(|_, _| open_mixer());
    let actions = gio::SimpleActionGroup::new();
    actions.add_action(&mute);
    actions.add_action(&mixer);
    button.insert_action_group(ACTION_GROUP, Some(&actions));

    // Middle click: mute on/off, as in GNOME 2.
    let middle = GestureClick::new();
    middle.set_button(gdk::BUTTON_MIDDLE);
    middle.connect_pressed(glib::clone!(
        #[strong]
        service,
        move |gesture, _, _, _| {
            gesture.set_state(gtk4::EventSequenceState::Claimed);
            querencia_lautstaerke::say!(
                "querencia-panel: Lautstärke: mittlere Taste → stumm umschalten"
            );
            service.toggle_output_mute();
        }
    ));
    button.add_controller(middle);

    // Mouse wheel: ±5 % per notch.
    let scroll = EventControllerScroll::new(
        EventControllerScrollFlags::VERTICAL | EventControllerScrollFlags::DISCRETE,
    );
    scroll.connect_scroll(glib::clone!(
        #[strong]
        service,
        move |_, _, dy| {
            if dy == 0.0 {
                return glib::Propagation::Proceed;
            }
            let state = service.state();
            if let Some(device) = state.output() {
                let target = model::scroll_target(device.percent(), dy < 0.0);
                querencia_lautstaerke::say!(
                    "querencia-panel: Lautstärke: Mausrad {} → {target} %",
                    if dy < 0.0 { "hoch" } else { "runter" }
                );
                service.set_output_percent(target);
            }
            glib::Propagation::Stop
        }
    ));
    button.add_controller(scroll);

    let id = service.subscribe(glib::clone!(
        #[weak]
        button,
        #[weak]
        image,
        #[strong]
        popup,
        #[strong]
        mute,
        move |state| {
            let look = model::look(state);
            show(&button, &image, size, &look);
            mute.set_state(&look.muted.to_variant());
            mute.set_enabled(look.available);
            popup.update(&look);
        }
    ));
    // The icon is drawn for the display scale; draw it again when it
    // changes.
    image.connect_scale_factor_notify(glib::clone!(
        #[strong]
        service,
        #[weak]
        button,
        move |image| show(&button, image, size, &model::look(&service.state()))
    ));
    button.connect_destroy(glib::clone!(
        #[strong]
        service,
        #[strong]
        popup,
        move |_| {
            service.unsubscribe(id);
            if popup.popover.parent().is_some() {
                popup.popover.unparent();
            }
        }
    ));
    button
}

/// The applet's icon at full volume, `size` pixels square, for the „Zum
/// Panel hinzufügen“ dialog; drawn again when the display scale changes.
pub fn image_of_size(size: i32) -> Image {
    let image = Image::new();
    image.set_pixel_size(size);
    let draw = move |image: &Image| {
        if let Some(texture) =
            icons::texture(model::Level::High, size * image.scale_factor().max(1))
        {
            image.set_paintable(Some(&texture));
        }
    };
    draw(&image);
    image.connect_scale_factor_notify(draw);
    image
}

/// Shows `look` on the applet: icon, tooltip, greyed out or not.
fn show(button: &Button, image: &Image, size: i32, look: &Look) {
    let pixels = size * image.scale_factor().max(1);
    match icons::texture(look.level, pixels) {
        Some(texture) => image.set_paintable(Some(&texture)),
        None => image.set_icon_name(Some("audio-volume-muted")),
    }
    button.set_tooltip_text(Some(&look.tooltip));
    if look.available {
        button.remove_css_class(UNAVAILABLE_CLASS);
    } else {
        button.add_css_class(UNAVAILABLE_CLASS);
    }
}

/// The popup of a left click.
struct Popup {
    popover: Popover,
    percent: Label,
    scale: Scale,
    mute: CheckButton,
    /// Set while the popup changes its own controls.
    updating: Cell<bool>,
    last_change: Cell<Option<Instant>>,
}

impl Popup {
    fn new(service: &Rc<Service>) -> Rc<Popup> {
        let percent = Label::new(None);
        percent.set_width_chars(5);
        let scale = Scale::with_range(
            Orientation::Vertical,
            0.0,
            f64::from(model::MAX_PERCENT),
            1.0,
        );
        // Loud at the top, as in GNOME 2.
        scale.set_inverted(true);
        scale.set_draw_value(false);
        scale.set_increments(1.0, f64::from(model::SCROLL_STEP));
        scale.set_height_request(SLIDER_HEIGHT);
        scale.set_halign(gtk4::Align::Center);
        let mute = CheckButton::with_label("Stumm");
        let content = GtkBox::new(Orientation::Vertical, 6);
        content.set_margin_top(6);
        content.set_margin_bottom(6);
        content.set_margin_start(6);
        content.set_margin_end(6);
        content.append(&percent);
        content.append(&scale);
        content.append(&mute);
        let popover = Popover::builder().has_arrow(true).child(&content).build();
        popover.add_css_class("querencia-volume-popup");

        let popup = Rc::new(Popup {
            popover,
            percent,
            scale,
            mute,
            updating: Cell::new(false),
            last_change: Cell::new(None),
        });
        // Keyboard: the slider has the focus when the popup opens (arrow
        // keys, Page Up/Down); labwc gives the grabbing popup the keyboard.
        popup.popover.connect_show(glib::clone!(
            #[weak(rename_to = scale)]
            popup.scale,
            move |_| {
                scale.grab_focus();
            }
        ));
        let weak = Rc::downgrade(&popup);
        let svc = service.clone();
        popup.scale.connect_value_changed(move |scale| {
            let Some(popup) = weak.upgrade() else { return };
            let value = scale.value().round() as u32;
            popup.percent.set_text(&format!("{value} %"));
            if popup.updating.get() {
                return;
            }
            popup.last_change.set(Some(Instant::now()));
            svc.set_output_percent(value);
        });
        let weak = Rc::downgrade(&popup);
        let svc = service.clone();
        popup.mute.connect_toggled(move |mute| {
            let Some(popup) = weak.upgrade() else { return };
            if popup.updating.get() {
                return;
            }
            querencia_lautstaerke::say!(
                "querencia-panel: Lautstärke: Regler → {}",
                if mute.is_active() { "stumm" } else { "laut" }
            );
            svc.set_output_muted(mute.is_active());
        });
        let weak = Rc::downgrade(&popup);
        popup.popover.connect_closed(move |_| {
            if let Some(popup) = weak.upgrade() {
                querencia_lautstaerke::say!(
                    "querencia-panel: Lautstärke: Regler geschlossen ({})",
                    popup.percent.text()
                );
            }
        });
        popup
    }

    fn update(&self, look: &Look) {
        self.updating.set(true);
        if look.available {
            let held = self.last_change.get().is_some_and(|t| t.elapsed() < HOLD);
            if !held {
                self.scale
                    .set_value(f64::from(look.percent.min(model::MAX_PERCENT)));
            }
            self.percent.set_text(&format!("{} %", look.percent));
        } else {
            self.scale.set_value(0.0);
            self.percent.set_text("–");
        }
        self.mute.set_active(look.muted);
        self.scale.set_sensitive(look.available);
        self.mute.set_sensitive(look.available);
        self.updating.set(false);
    }
}

/// Starts the mixer: through its desktop entry if installed (GIO, as the
/// menus start applications), else the program from `PATH` or
/// `~/opt/querencia/bin`.
fn open_mixer() {
    let id = format!("{MIXER_APP_ID}.desktop");
    if DesktopAppInfo::new(&id).is_some() {
        querencia_lautstaerke::say!("querencia-panel: Lautstärke: Lautstärkeregler über {id}");
        app_menu::launch_app(&id);
        return;
    }
    let path = std::env::var_os("PATH");
    let home = glib::home_dir();
    match mixer_program(path.as_deref(), &home, |p| p.is_file()) {
        Some(program) => {
            querencia_lautstaerke::say!(
                "querencia-panel: Lautstärke: Lautstärkeregler {}",
                program.display()
            );
            let launched =
                gio::Subprocess::newv(&[program.as_os_str()], gio::SubprocessFlags::STDIN_INHERIT);
            if let Err(err) = launched {
                querencia_lautstaerke::say!(
                    "querencia-panel: Lautstärkeregler nicht gestartet: {err}"
                );
            }
        }
        None => querencia_lautstaerke::say!(
            "querencia-panel: Lautstärkeregler {MIXER_PROGRAM} ist nicht installiert ({id} fehlt)"
        ),
    }
}

/// Where the mixer program is: the first `PATH` entry holding it, else
/// `<home>/opt/querencia/bin/querencia-lautstaerke`; `exists` tells whether
/// a file is there.
fn mixer_program(
    path: Option<&std::ffi::OsStr>,
    home: &std::path::Path,
    exists: impl Fn(&std::path::Path) -> bool,
) -> Option<std::path::PathBuf> {
    path.into_iter()
        .flat_map(std::env::split_paths)
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join(MIXER_PROGRAM))
        .chain(std::iter::once(
            home.join("opt/querencia/bin").join(MIXER_PROGRAM),
        ))
        .find(|candidate| exists(candidate))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn label(menu: &gio::MenuModel, i: i32) -> Option<String> {
        menu.item_attribute_value(i, "label", Some(glib::VariantTy::STRING))
            .and_then(|v| v.get::<String>())
    }

    fn action(menu: &gio::MenuModel, i: i32) -> Option<String> {
        menu.item_attribute_value(i, "action", Some(glib::VariantTy::STRING))
            .and_then(|v| v.get::<String>())
    }

    #[test]
    fn menu_mutes_opens_the_mixer_and_removes() {
        let menu = menu().upcast::<gio::MenuModel>();
        assert_eq!(label(&menu, 0).as_deref(), Some("_Stumm"));
        assert_eq!(action(&menu, 0).as_deref(), Some("volume.mute"));
        assert_eq!(
            label(&menu, 1).as_deref(),
            Some("_Lautstärkeregler öffnen …")
        );
        assert_eq!(action(&menu, 1).as_deref(), Some("volume.mixer"));
        let section = menu.item_link(2, "section").expect("Abschnitt");
        assert_eq!(
            label(&section, 0).as_deref(),
            Some("Aus dem Panel entfernen")
        );
        assert_eq!(action(&section, 0).as_deref(), Some("panel.applet-remove"));
    }

    #[test]
    fn mixer_is_found_in_path_first_then_in_opt() {
        let home = Path::new("/home/x");
        let opt = PathBuf::from("/home/x/opt/querencia/bin/querencia-lautstaerke");
        let in_path = PathBuf::from("/usr/local/bin/querencia-lautstaerke");
        let path = std::ffi::OsString::from("relativ:/usr/bin:/usr/local/bin");
        let both = |p: &Path| p == opt || p == in_path;
        assert_eq!(
            mixer_program(Some(&path), home, both),
            Some(in_path.clone())
        );
        let only_opt = |p: &Path| p == opt;
        assert_eq!(
            mixer_program(Some(&path), home, only_opt),
            Some(opt.clone())
        );
        assert_eq!(mixer_program(None, home, only_opt), Some(opt));
        // a relative PATH entry is never used
        let relative = |p: &Path| p == Path::new("relativ/querencia-lautstaerke");
        assert_eq!(mixer_program(Some(&path), home, relative), None);
    }

    #[test]
    fn action_group_does_not_clash_with_the_panel_or_other_applets() {
        assert!(!["panel", "menu", "system", "session", "places"].contains(&ACTION_GROUP));
    }
}
