//! The own icons of the volume applet and the mixer (Querencia style: grey
//! speaker, waves in Querencia orange #F07746, flat), built into the
//! binary as SVG, so they need no installed icon theme. They are drawn
//! through GDK-Pixbuf's SVG loader at the exact pixel size, never scaled
//! up from a bitmap.

use std::cell::RefCell;
use std::collections::HashMap;

use gtk4::gdk;
use gtk4::gdk_pixbuf::Pixbuf;
use gtk4::gio;
use gtk4::glib;

use crate::model::Level;

/// SVG source of the applet icon for `level`.
pub fn svg(level: Level) -> &'static [u8] {
    match level {
        Level::Muted => include_bytes!("../data/symbole/lautstaerke-stumm.svg"),
        Level::Low => include_bytes!("../data/symbole/lautstaerke-leise.svg"),
        Level::Medium => include_bytes!("../data/symbole/lautstaerke-mittel.svg"),
        Level::High => include_bytes!("../data/symbole/lautstaerke-laut.svg"),
    }
}

/// SVG source of the mixer's application icon (`org.querencia.Lautstaerke`).
pub const APP_SVG: &[u8] = include_bytes!("../data/symbole/org.querencia.Lautstaerke.svg");

/// `svg` drawn `pixels` square, `None` (with a log line) if it cannot be
/// read.
pub fn render(svg: &'static [u8], pixels: i32) -> Option<gdk::Texture> {
    let stream = gio::MemoryInputStream::from_bytes(&glib::Bytes::from_static(svg));
    match Pixbuf::from_stream_at_scale(&stream, pixels, pixels, true, gio::Cancellable::NONE) {
        Ok(pixbuf) => Some(gdk::Texture::for_pixbuf(&pixbuf)),
        Err(err) => {
            crate::say!("querencia-lautstaerke: Symbol nicht lesbar: {err}");
            None
        }
    }
}

thread_local! {
    static CACHE: RefCell<HashMap<(Level, i32), Option<gdk::Texture>>> = RefCell::new(HashMap::new());
}

/// The applet icon for `level`, `pixels` square (logical size times the
/// display scale), drawn once and kept.
pub fn texture(level: Level, pixels: i32) -> Option<gdk::Texture> {
    CACHE.with(|cache| {
        cache
            .borrow_mut()
            .entry((level, pixels))
            .or_insert_with(|| render(svg(level), pixels))
            .clone()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use gtk4::prelude::TextureExt;

    #[test]
    fn every_level_has_its_own_svg_in_querencia_colours() {
        let mut seen = Vec::new();
        for level in Level::ALL {
            let text = std::str::from_utf8(svg(level)).unwrap();
            assert!(text.starts_with("<?xml"), "{level:?}");
            assert!(text.contains("#5a5a5a"), "grauer Lautsprecher: {level:?}");
            assert!(text.contains("#F07746"), "Querencia-Orange: {level:?}");
            assert!(!seen.contains(&text), "{level:?} doppelt");
            seen.push(text);
        }
        // one, two, three orange waves from quiet to loud
        let waves = |l| {
            std::str::from_utf8(svg(l))
                .unwrap()
                .matches("stroke=\"#F07746\" stroke-width=\"2.2\"")
                .count()
        };
        assert_eq!(
            [waves(Level::Low), waves(Level::Medium), waves(Level::High)],
            [1, 2, 3]
        );
        let app = std::str::from_utf8(APP_SVG).unwrap();
        assert!(app.contains("#F07746") && app.contains("viewBox=\"0 0 64 64\""));
    }

    #[test]
    fn icons_render_at_the_requested_size() {
        for level in Level::ALL {
            for pixels in [16, 22, 24, 44] {
                let texture = render(svg(level), pixels).expect("SVG-Lader");
                assert_eq!((texture.width(), texture.height()), (pixels, pixels));
            }
        }
        let app = render(APP_SVG, 48).expect("SVG-Lader");
        assert_eq!(app.width(), 48);
    }
}
