//! Desktop 0.3.0 (E3): the questions of mounting a volume – a password, a
//! choice – in a window of the desktop's own.
//!
//! GTK's `GtkMountOperation` shows a modal dialog; in the desktop (a
//! layer-shell program) closing that dialog after a successful mount made
//! gtk4-layer-shell crash (isolated test 23.10.2026, `wl_proxy_marshal_flags`
//! while GTK freed the dialog). The desktop therefore answers GIO's
//! `ask-password` and `ask-question` itself, with non-modal windows like its
//! other dialogs (no parent: the desktop is a layer surface). "Still in use"
//! is `ask_when_busy` in `desktop_view.rs`.

use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Align, EventControllerKey, Orientation, gdk, gio, glib};
use querencia_oberflaeche::symbole;

use crate::i18n::gettext;

/// A mount operation whose questions the desktop asks itself.
pub fn mount_operation() -> gio::MountOperation {
    let operation = gio::MountOperation::new();
    let open: Rc<RefCell<Option<gtk4::Window>>> = Rc::default();
    let shown = open.clone();
    operation.connect_ask_password(move |operation, message, _user, _domain, flags| {
        // GIO's own handler would answer "not handled" at once
        operation.stop_signal_emission_by_name("ask-password");
        super::log(&format!("Kennwortfrage: {message}"));
        let window = password_window(operation, message, flags);
        if let Some(old) = shown.replace(Some(window)) {
            old.destroy();
        }
    });
    // GIO's bindings have no typed handler for this signal (a string array)
    operation.connect_local("ask-question", false, |values| {
        let operation = values.first()?.get::<gio::MountOperation>().ok()?;
        operation.stop_signal_emission_by_name("ask-question");
        let message = values.get(1)?.get::<String>().unwrap_or_default();
        let choices = values.get(2)?.get::<glib::StrV>().ok()?;
        let choices: Vec<String> = choices.iter().map(|c| c.to_string()).collect();
        super::log(&format!("Frage beim Einhängen: {message}"));
        let dialog = gtk4::AlertDialog::builder()
            .message(message)
            .buttons(choices.clone())
            .modal(false)
            .build();
        dialog.choose(
            None::<&gtk4::Window>,
            gio::Cancellable::NONE,
            move |answer| match answer.ok() {
                Some(n) if usize::try_from(n).is_ok_and(|i| i < choices.len()) => {
                    operation.set_choice(n);
                    operation.reply(gio::MountOperationResult::Handled);
                }
                _ => operation.reply(gio::MountOperationResult::Aborted),
            },
        );
        None
    });
    operation.connect_aborted(move |_| {
        if let Some(window) = open.borrow_mut().take() {
            window.destroy();
        }
    });
    operation
}

/// The password window: the message of GIO, the password, and Abbrechen /
/// Verbinden (the default).
fn password_window(
    operation: &gio::MountOperation,
    message: &str,
    flags: gio::AskPasswordFlags,
) -> gtk4::Window {
    let window = gtk4::Window::builder()
        .title(gettext("Password"))
        .resizable(false)
        .build();
    window.add_css_class("querencia-desktop-question");
    let outer = gtk4::Box::new(Orientation::Vertical, 12);
    outer.set_margin_top(12);
    outer.set_margin_bottom(12);
    outer.set_margin_start(12);
    outer.set_margin_end(12);
    let head = gtk4::Box::new(Orientation::Horizontal, 12);
    let icon = symbole::image_named("dialog-password", 48);
    icon.set_valign(Align::Start);
    let label = gtk4::Label::new(Some(message));
    label.set_xalign(0.0);
    label.set_wrap(true);
    label.set_max_width_chars(50);
    head.append(&icon);
    head.append(&label);
    outer.append(&head);
    let entry = gtk4::PasswordEntry::new();
    entry.set_show_peek_icon(true);
    entry.set_activates_default(true);
    entry.set_sensitive(flags.contains(gio::AskPasswordFlags::NEED_PASSWORD));
    outer.append(&entry);
    let buttons = gtk4::Box::new(Orientation::Horizontal, 6);
    buttons.set_halign(Align::End);
    let cancel = gtk4::Button::with_mnemonic(&gettext("_Cancel"));
    let connect = gtk4::Button::with_mnemonic(&gettext("C_onnect"));
    buttons.append(&cancel);
    buttons.append(&connect);
    outer.append(&buttons);
    window.set_child(Some(&outer));
    window.set_default_widget(Some(&connect));

    let replied = Rc::new(std::cell::Cell::new(false));
    let answer = {
        let operation = operation.clone();
        let window = window.downgrade();
        let entry = entry.clone();
        let replied = replied.clone();
        Rc::new(move |ok: bool| {
            if replied.replace(true) {
                return;
            }
            if ok {
                operation.set_password(Some(entry.text().as_str()));
                super::log("Kennwort eingegeben");
                operation.reply(gio::MountOperationResult::Handled);
            } else {
                super::log("Kennwortfrage abgebrochen");
                operation.reply(gio::MountOperationResult::Aborted);
            }
            if let Some(window) = window.upgrade() {
                window.destroy();
            }
        })
    };
    let yes = answer.clone();
    connect.connect_clicked(move |_| yes(true));
    let no = answer.clone();
    cancel.connect_clicked(move |_| no(false));
    let keys = EventControllerKey::new();
    let no = answer.clone();
    keys.connect_key_pressed(move |_, key, _, _| {
        if key == gdk::Key::Escape {
            no(false);
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    window.add_controller(keys);
    let no = answer;
    window.connect_close_request(move |_| {
        no(false);
        glib::Propagation::Proceed
    });
    window.present();
    entry.grab_focus();
    window
}
