//! The own icons of the network applet (Querencia style: grey with
//! Querencia orange #F07746, flat), built into the binary as SVG, so they
//! need no installed icon theme. They are drawn through GDK-Pixbuf's SVG
//! loader at the exact pixel size. `tools/netzwerk-pruefung/symbole-machen.py`
//! writes them.

use std::cell::RefCell;
use std::collections::HashMap;

use gtk4::gdk;
use gtk4::gdk_pixbuf::Pixbuf;
use gtk4::gio;
use gtk4::glib;

use crate::model::Icon;

/// Small icons of the menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Small {
    /// A secured WLAN.
    Lock,
    /// Check item on.
    Checked,
    /// Check item off.
    Unchecked,
}

/// SVG source of the applet icon `icon`.
pub fn svg(icon: Icon) -> &'static [u8] {
    match icon {
        Icon::WiredConnected => include_bytes!("../data/symbole/netzwerk-kabel.svg"),
        Icon::WiredDisconnected => include_bytes!("../data/symbole/netzwerk-kabel-getrennt.svg"),
        Icon::Wireless(0) => include_bytes!("../data/symbole/netzwerk-wlan-0.svg"),
        Icon::Wireless(1) => include_bytes!("../data/symbole/netzwerk-wlan-1.svg"),
        Icon::Wireless(2) => include_bytes!("../data/symbole/netzwerk-wlan-2.svg"),
        Icon::Wireless(_) => include_bytes!("../data/symbole/netzwerk-wlan-3.svg"),
        Icon::WirelessOff => include_bytes!("../data/symbole/netzwerk-wlan-aus.svg"),
        Icon::Connecting => include_bytes!("../data/symbole/netzwerk-verbindet.svg"),
        Icon::NoNetwork => include_bytes!("../data/symbole/netzwerk-kein-netz.svg"),
        Icon::NoService => include_bytes!("../data/symbole/netzwerk-kein-dienst.svg"),
    }
}

/// The menu variant of `icon` for a highlighted (orange) row: white arcs
/// instead of orange ones. Only the WLAN icons need one.
pub fn highlight_svg(icon: Icon) -> Option<&'static [u8]> {
    match icon {
        Icon::Wireless(0) => Some(include_bytes!("../data/symbole/netzwerk-wlan-0-hell.svg")),
        Icon::Wireless(1) => Some(include_bytes!("../data/symbole/netzwerk-wlan-1-hell.svg")),
        Icon::Wireless(2) => Some(include_bytes!("../data/symbole/netzwerk-wlan-2-hell.svg")),
        Icon::Wireless(_) => Some(include_bytes!("../data/symbole/netzwerk-wlan-3-hell.svg")),
        _ => None,
    }
}

pub fn small_svg(icon: Small) -> &'static [u8] {
    match icon {
        Small::Lock => include_bytes!("../data/symbole/netzwerk-schloss.svg"),
        Small::Checked => include_bytes!("../data/symbole/netzwerk-haken-an.svg"),
        Small::Unchecked => include_bytes!("../data/symbole/netzwerk-haken-aus.svg"),
    }
}

/// `svg` drawn `pixels` square, `None` (with a log line) if it cannot be
/// read.
pub fn render(svg: &'static [u8], pixels: i32) -> Option<gdk::Texture> {
    let stream = gio::MemoryInputStream::from_bytes(&glib::Bytes::from_static(svg));
    match Pixbuf::from_stream_at_scale(&stream, pixels, pixels, true, gio::Cancellable::NONE) {
        Ok(pixbuf) => Some(gdk::Texture::for_pixbuf(&pixbuf)),
        Err(err) => {
            crate::say!("querencia-netzwerk: Symbol nicht lesbar: {err}");
            None
        }
    }
}

thread_local! {
    static CACHE: RefCell<HashMap<(usize, i32), Option<gdk::Texture>>> = RefCell::new(HashMap::new());
}

fn cached(svg: &'static [u8], pixels: i32) -> Option<gdk::Texture> {
    CACHE.with(|cache| {
        cache
            .borrow_mut()
            .entry((svg.as_ptr() as usize, pixels))
            .or_insert_with(|| render(svg, pixels))
            .clone()
    })
}

/// The applet icon `icon`, `pixels` square (logical size times the display
/// scale), drawn once and kept.
pub fn texture(icon: Icon, pixels: i32) -> Option<gdk::Texture> {
    cached(svg(icon), pixels)
}

pub fn highlight_texture(icon: Icon, pixels: i32) -> Option<gdk::Texture> {
    cached(highlight_svg(icon)?, pixels)
}

pub fn small_texture(icon: Small, pixels: i32) -> Option<gdk::Texture> {
    cached(small_svg(icon), pixels)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gtk4::prelude::TextureExt;

    #[test]
    fn every_icon_has_its_own_svg_in_querencia_colours() {
        let mut seen = Vec::new();
        for icon in Icon::ALL {
            let text = std::str::from_utf8(svg(icon)).unwrap();
            assert!(text.starts_with("<?xml"), "{icon:?}");
            assert!(text.contains("viewBox=\"0 0 24 24\""), "{icon:?}");
            assert!(text.contains("#F07746"), "Querencia-Orange: {icon:?}");
            // grey everywhere but in the full WLAN signal (all orange)
            assert!(
                icon == Icon::Wireless(3)
                    || text.contains("#5a5a5a")
                    || text.contains("#c8c8c8")
                    || text.contains("#9a9a9a"),
                "Grau: {icon:?}"
            );
            assert!(!seen.contains(&text), "{icon:?} doppelt");
            seen.push(text);
        }
        // more orange arcs with more signal
        let arcs = |stage| {
            std::str::from_utf8(svg(Icon::Wireless(stage)))
                .unwrap()
                .matches("stroke=\"#F07746\" stroke-width=\"2.6\"")
                .count()
        };
        assert_eq!([arcs(0), arcs(1), arcs(2), arcs(3)], [0, 1, 2, 3]);
        assert_eq!(svg(Icon::Wireless(9)), svg(Icon::Wireless(3)));
        // highlight variants: white, no orange, only for WLAN
        for stage in 0..4 {
            let text = std::str::from_utf8(highlight_svg(Icon::Wireless(stage)).unwrap()).unwrap();
            assert!(text.contains("#ffffff") && !text.contains("#F07746"));
        }
        assert!(highlight_svg(Icon::WiredConnected).is_none());
        for small in [Small::Lock, Small::Checked, Small::Unchecked] {
            let text = std::str::from_utf8(small_svg(small)).unwrap();
            assert!(text.contains("viewBox=\"0 0 16 16\""), "{small:?}");
        }
    }

    #[test]
    fn icons_render_at_the_requested_size() {
        for icon in Icon::ALL {
            for pixels in [16, 22, 24, 44] {
                let texture = render(svg(icon), pixels).expect("SVG-Lader");
                assert_eq!((texture.width(), texture.height()), (pixels, pixels));
            }
        }
        for stage in 0..4 {
            let texture = highlight_texture(Icon::Wireless(stage), 16).expect("SVG-Lader");
            assert_eq!(texture.width(), 16);
        }
        for small in [Small::Lock, Small::Checked, Small::Unchecked] {
            let texture = render(small_svg(small), 16).expect("SVG-Lader");
            assert_eq!(texture.width(), 16);
        }
    }
}
