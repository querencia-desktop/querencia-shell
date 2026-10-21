//! The panel's own status icons on the dark panel (QD-081, decision of the
//! start session 21.10.2026): volume, network and the bell are flat grey
//! SVGs made for the light panel (`#5a5a5a` with a `#3c3c3c` outline, about
//! 1.5:1 on Thema 0.2's `#3c3b37`). While `farbschema::dunkel` holds, the
//! panel draws them with the greys swapped ([`UMFAERBUNG`]); the SVG sources
//! in the crates stay as they are, and in the light look the crates' own
//! textures are used unchanged (pixel-identical). Orange, white and the
//! orange variants for highlighted menu rows stay.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{gdk, gio, glib};

use crate::farbschema;

/// Grey of the light icons → grey for the dark panel. The first two are the
/// start session's decision (body and outline); the others keep what the
/// light icons say: faint "empty" bars and arcs stay faint, a greyed-out
/// icon stays greyed out, a light screen becomes a dark one.
pub const UMFAERBUNG: [(&str, &str); 9] = [
    ("#5a5a5a", "#bdb7b1"), // body
    ("#3c3c3c", "#23221f"), // outline
    ("#c8c8c8", "#5f5d57"), // empty bars and arcs (faint)
    ("#c9c6c2", "#5f5d57"), // empty volume arcs (faint)
    ("#9a9a9a", "#858079"), // greyed-out body
    ("#9a9794", "#858079"), // bell at rest ("Nicht stören")
    ("#7a7a7a", "#2b2a27"), // greyed-out outline
    ("#dddddd", "#a5a09a"), // marks on a greyed-out body
    ("#e6e6e6", "#2b2a27"), // screen of "no service"
];
/// The bell's light edge: lighter than the new body, as it is on the light
/// one.
const KANTE: (&str, &str) = ("#8a8784", "#dfdbd2");

/// The settings whose change can turn the dark look on or off.
const EINSTELLUNGEN: [&str; 3] = [
    "gtk-interface-color-scheme",
    "gtk-application-prefer-dark-theme",
    "gtk-theme-name",
];

thread_local! {
    static DUNKEL: Cell<Option<bool>> = const { Cell::new(None) };
    static CACHE: RefCell<HashMap<(usize, i32), Option<gdk::Texture>>> = RefCell::new(HashMap::new());
}

/// Is the dark look on (`farbschema::dunkel`, kept until a setting changes)?
pub fn dunkel() -> bool {
    DUNKEL.get().unwrap_or_else(|| {
        let an = gtk4::Settings::default().is_some_and(|s| farbschema::dunkel(&s));
        DUNKEL.set(Some(an));
        an
    })
}

/// `svg` with the greys for the dark panel.
pub fn fuer_dunkel(svg: &[u8]) -> Vec<u8> {
    let mut text = String::from_utf8_lossy(svg).into_owned();
    for (hell, dunkel) in UMFAERBUNG.iter().chain([&KANTE]) {
        text = text.replace(&format!("\"{hell}\""), &format!("\"{dunkel}\""));
    }
    text.into_bytes()
}

/// The icon `svg`, `pixels` square: in the light look what `hell` draws
/// (the crate's own texture), in the dark look `svg` recoloured, drawn once
/// and kept.
pub fn texture(
    svg: &'static [u8],
    pixels: i32,
    hell: impl FnOnce() -> Option<gdk::Texture>,
) -> Option<gdk::Texture> {
    if !dunkel() {
        return hell();
    }
    CACHE.with(|cache| {
        cache
            .borrow_mut()
            .entry((svg.as_ptr() as usize, pixels))
            .or_insert_with(|| render(fuer_dunkel(svg), pixels))
            .clone()
    })
}

fn render(svg: Vec<u8>, pixels: i32) -> Option<gdk::Texture> {
    let stream = gio::MemoryInputStream::from_bytes(&glib::Bytes::from_owned(svg));
    match gtk4::gdk_pixbuf::Pixbuf::from_stream_at_scale(
        &stream,
        pixels,
        pixels,
        true,
        gio::Cancellable::NONE,
    ) {
        Ok(pixbuf) => Some(gdk::Texture::for_pixbuf(&pixbuf)),
        Err(err) => {
            eprintln!("querencia-panel: Symbol (dunkel) nicht lesbar: {err}");
            None
        }
    }
}

/// Calls `redraw` after every change of the colour scheme or the theme,
/// while `widget` lives, so its icon follows the dark look at once.
pub fn bei_wechsel(widget: &impl IsA<gtk4::Widget>, redraw: impl Fn() + 'static) {
    let Some(settings) = gtk4::Settings::default() else {
        return;
    };
    let redraw = Rc::new(redraw);
    let ids: Vec<_> = EINSTELLUNGEN
        .into_iter()
        .map(|name| {
            let redraw = redraw.clone();
            settings.connect_notify_local(Some(name), move |s, _| {
                DUNKEL.set(Some(farbschema::dunkel(s)));
                redraw();
            })
        })
        .collect();
    let ids = Cell::new(Some(ids));
    widget.connect_destroy(move |_| {
        for id in ids.take().into_iter().flatten() {
            settings.disconnect(id);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every grey of the applet icons has a dark counterpart; only orange,
    /// its edge and white stay.
    #[test]
    fn every_grey_of_the_status_icons_is_recoloured() {
        use querencia_lautstaerke::model::Level;
        use querencia_netzwerk::icons::Small;
        use querencia_netzwerk::model::Icon;
        let mut svgs: Vec<&[u8]> = Level::ALL
            .into_iter()
            .map(querencia_lautstaerke::icons::svg)
            .collect();
        svgs.extend(Icon::ALL.into_iter().map(querencia_netzwerk::icons::svg));
        svgs.extend(
            [Small::Lock, Small::Checked, Small::Unchecked]
                .into_iter()
                .map(querencia_netzwerk::icons::small_svg),
        );
        svgs.extend(crate::glocke::alle_svgs());
        let bleibt = ["#F07746", "#ffffff", "#b36c4e"];
        for svg in svgs {
            let dunkel = String::from_utf8(fuer_dunkel(svg)).unwrap();
            for (i, _) in dunkel.match_indices("=\"#") {
                let farbe = &dunkel[i + 2..i + 9];
                assert!(
                    bleibt.contains(&farbe)
                        || UMFAERBUNG.iter().chain([&KANTE]).any(|(_, d)| *d == farbe),
                    "{farbe} ohne dunkles Gegenstück"
                );
            }
            // the orange stays exactly where it was
            let hell = std::str::from_utf8(svg).unwrap();
            assert_eq!(
                hell.matches("#F07746").count(),
                dunkel.matches("#F07746").count()
            );
        }
    }

    #[test]
    fn dark_icons_use_the_decided_greys() {
        let svg = querencia_lautstaerke::icons::svg(querencia_lautstaerke::model::Level::Low);
        let dunkel = fuer_dunkel(svg);
        assert_ne!(dunkel, svg);
        let text = String::from_utf8(dunkel).unwrap();
        assert!(text.contains("\"#bdb7b1\"") && text.contains("\"#23221f\""));
        assert!(!text.contains("#5a5a5a") && !text.contains("#3c3c3c"));
        // the decided pair: body and outline
        assert_eq!(UMFAERBUNG[0], ("#5a5a5a", "#bdb7b1"));
        assert_eq!(UMFAERBUNG[1], ("#3c3c3c", "#23221f"));
    }
}
