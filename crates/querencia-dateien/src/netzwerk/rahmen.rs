//! The frame of the network dialogs (password, question, "Mit Server
//! verbinden …"): a small window in the classic GNOME 2 shape, transient
//! for the browser window: a 48 px icon on the left, a bold first line, an
//! explanation, room for fields, the buttons at the bottom right. Built new
//! for every use (QD-076: no hide/present).

use gtk4::prelude::*;
use gtk4::{Align, Orientation, glib};
use querencia_oberflaeche::symbole;

pub struct Rahmen {
    pub window: gtk4::Window,
    /// Below the texts: fields, check boxes.
    pub inhalt: gtk4::Box,
    /// Right-aligned row of buttons.
    pub knoepfe: gtk4::Box,
}

/// `erst` bold, `zweit` below it (left out when empty).
pub fn neu(
    parent: Option<&gtk4::Window>,
    titel: &str,
    symbol: &str,
    erst: &str,
    zweit: &str,
) -> Rahmen {
    let window = gtk4::Window::builder()
        .title(titel)
        .resizable(false)
        .modal(true)
        .build();
    window.set_transient_for(parent);
    window.add_css_class("querencia-dateien-question");
    let aussen = gtk4::Box::new(Orientation::Vertical, 12);
    aussen.set_margin_top(12);
    aussen.set_margin_bottom(12);
    aussen.set_margin_start(12);
    aussen.set_margin_end(12);
    let kopf = gtk4::Box::new(Orientation::Horizontal, 12);
    let bild = symbole::image_named(symbol, 48);
    bild.set_valign(Align::Start);
    let rechts = gtk4::Box::new(Orientation::Vertical, 6);
    let eins = gtk4::Label::new(None);
    eins.set_markup(&format!(
        "<b><big>{}</big></b>",
        glib::markup_escape_text(erst)
    ));
    eins.set_xalign(0.0);
    eins.set_wrap(true);
    eins.set_max_width_chars(50);
    eins.set_selectable(false);
    rechts.append(&eins);
    if !zweit.is_empty() {
        let zwei = gtk4::Label::new(Some(zweit));
        zwei.set_xalign(0.0);
        zwei.set_wrap(true);
        zwei.set_max_width_chars(60);
        // fingerprints and messages can be copied
        zwei.set_selectable(true);
        zwei.set_can_focus(false);
        rechts.append(&zwei);
    }
    let inhalt = gtk4::Box::new(Orientation::Vertical, 6);
    rechts.append(&inhalt);
    kopf.append(&bild);
    kopf.append(&rechts);
    aussen.append(&kopf);
    let knoepfe = gtk4::Box::new(Orientation::Horizontal, 6);
    knoepfe.set_halign(Align::End);
    aussen.append(&knoepfe);
    window.set_child(Some(&aussen));
    Rahmen {
        window,
        inhalt,
        knoepfe,
    }
}

/// Escape and closing the window run `abbrechen` (once, by the caller's
/// guard).
pub fn bei_abbruch(window: &gtk4::Window, abbrechen: impl Fn() + 'static) {
    let abbrechen = std::rc::Rc::new(abbrechen);
    let tasten = gtk4::EventControllerKey::new();
    let a = abbrechen.clone();
    let w = window.downgrade();
    tasten.connect_key_pressed(move |_, taste, _, _| {
        if taste == gtk4::gdk::Key::Escape {
            a();
            if let Some(w) = w.upgrade() {
                w.destroy();
            }
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    window.add_controller(tasten);
    window.connect_close_request(move |_| {
        abbrechen();
        glib::Propagation::Proceed
    });
}

/// A grid row "Label: field" with the label as mnemonic for the field.
pub fn zeile(
    gitter: &gtk4::Grid,
    reihe: i32,
    text: &str,
    feld: &impl IsA<gtk4::Widget>,
) -> gtk4::Label {
    let label = gtk4::Label::with_mnemonic(text);
    label.set_xalign(1.0);
    label.set_mnemonic_widget(Some(feld));
    gitter.attach(&label, 0, reihe, 1, 1);
    feld.set_hexpand(true);
    gitter.attach(feld, 1, reihe, 1, 1);
    label
}
