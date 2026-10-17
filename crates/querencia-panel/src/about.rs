//! "Informationen über Querencia" in the "System" menu, the counterpart of
//! GNOME 2's "Über GNOME": a small window with logo, name, version, a short
//! description and the license. An ordinary toplevel window, no
//! libadwaita, no header bar; labwc draws the title bar.

use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Label, Orientation, Window, glib};

use crate::{add_dialog, logo, session};

const TITLE: &str = "Informationen über Querencia";
/// Size of the logo at the top of the window.
const LOGO_SIZE: i32 = 64;

/// The text of the window, line by line: name, version, description,
/// license (version and license from `Cargo.toml`).
pub fn lines() -> Vec<String> {
    vec![
        "Querencia Desktop".to_string(),
        format!("Version {}", env!("CARGO_PKG_VERSION")),
        "Ein klassischer Desktop im Stil von GNOME 2 für Wayland und labwc".to_string(),
        format!("Lizenz: {}", env!("CARGO_PKG_LICENSE")),
    ]
}

/// Raises the window in `slot` or opens a new one there.
pub fn present(slot: &glib::WeakRef<Window>) {
    if let Some(window) = slot.upgrade() {
        window.present();
        return;
    }
    let window = Window::builder().title(TITLE).resizable(false).build();
    window.set_application(session::panel_application().as_ref());
    let lines = lines();

    // The official Querencia logo (C2), as large as the theme's
    // distribution logo `start-here` was here before QREL-04c.
    let logo = logo::image_of_size(LOGO_SIZE);
    let name = Label::builder()
        .label(format!(
            "<b><big>{}</big></b>",
            glib::markup_escape_text(&lines[0])
        ))
        .use_markup(true)
        .build();
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
    for line in &lines[1..] {
        content.append(
            &Label::builder()
                .label(line)
                .wrap(true)
                .max_width_chars(40)
                .justify(gtk4::Justification::Center)
                .build(),
        );
    }
    let close = add_dialog::stock_button("_Schließen", "window-close");
    close.set_halign(Align::End);
    close.set_margin_top(12);
    close.connect_clicked(glib::clone!(
        #[weak]
        window,
        move |_| window.close()
    ));
    content.append(&close);
    window.set_child(Some(&content));
    session::close_on_escape(&window);
    GtkWindowExt::set_focus(&window, Some(&close));
    slot.set(Some(&window));
    window.present();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn about_names_product_version_and_license() {
        let lines = lines();
        assert_eq!(lines[0], "Querencia Desktop");
        assert_eq!(lines[1], format!("Version {}", env!("CARGO_PKG_VERSION")));
        assert!(lines[2].contains("GNOME 2"), "{}", lines[2]);
        assert_eq!(lines[3], "Lizenz: GPL-3.0-or-later");
        assert_eq!(lines.len(), 4);
    }
}
