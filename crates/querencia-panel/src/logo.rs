//! The Querencia logo before "Anwendungen" (HMP-11a): the orange Q of
//! variant C2 ("Harmonisch verbunden"), in place of the distribution logo
//! `start-here` that HMP-02 to HMP-11 showed there. The image files are the
//! ones the project lead chose (`data/logo/`, with the SVG master), built
//! into the binary, so the logo needs no installed file and no icon theme;
//! the theme's `start-here` stays untouched for everything else.
//!
//! The logo is as high as the former icon on the standard panel (22 px on
//! 32 px) and follows the panel height in the same ratio. Of the bitmaps the
//! smallest one at least as large as needed (times the display scale) is
//! drawn, so it is only ever scaled down, square, never distorted.

use gtk4::prelude::*;
use gtk4::{Image, gdk, glib};

/// The bitmaps of the logo by edge length, smallest first; all are square
/// RGBA PNGs with a transparent background.
const BITMAPS: [(i32, &[u8]); 8] = [
    (16, include_bytes!("../data/logo/querencia-logo-16.png")),
    (22, include_bytes!("../data/logo/querencia-logo-22.png")),
    (24, include_bytes!("../data/logo/querencia-logo-24.png")),
    (32, include_bytes!("../data/logo/querencia-logo-32.png")),
    (48, include_bytes!("../data/logo/querencia-logo-48.png")),
    (64, include_bytes!("../data/logo/querencia-logo-64.png")),
    (128, include_bytes!("../data/logo/querencia-logo-128.png")),
    (256, include_bytes!("../data/logo/querencia-logo-256.png")),
];

/// Logo size on a panel `panel_height` pixels high: 22 on the standard 32,
/// the same ratio (11/16, rounded down) on any other height, so 16 on a 24
/// px panel; at least 1.
pub fn size_for_panel(panel_height: i32) -> i32 {
    (panel_height * 11 / 16).max(1)
}

/// Edge length of the bitmap to draw a logo of `size` logical pixels at
/// display scale `scale`: the smallest one at least `size * scale`, else
/// the largest.
pub fn bitmap_for(size: i32, scale: i32) -> i32 {
    let needed = size * scale.max(1);
    BITMAPS
        .iter()
        .map(|(edge, _)| *edge)
        .find(|edge| *edge >= needed)
        .unwrap_or(BITMAPS[BITMAPS.len() - 1].0)
}

/// The logo for a panel `panel_height` pixels high. It picks its bitmap
/// again when the display scale changes. A bitmap that cannot be read
/// leaves the image empty with a log line; the panel goes on.
pub fn image(panel_height: i32) -> Image {
    let size = size_for_panel(panel_height);
    let image = Image::new();
    image.set_pixel_size(size);
    show(&image, size);
    image.connect_scale_factor_notify(move |image| show(image, size));
    image
}

fn show(image: &Image, size: i32) {
    let edge = bitmap_for(size, image.scale_factor());
    let bytes = BITMAPS
        .iter()
        .find(|(e, _)| *e == edge)
        .map(|(_, bytes)| *bytes)
        .unwrap_or_default();
    match gdk::Texture::from_bytes(&glib::Bytes::from_static(bytes)) {
        Ok(texture) => image.set_paintable(Some(&texture)),
        Err(err) => {
            image.clear();
            eprintln!("querencia-panel: Querencia-Logo {edge} px nicht lesbar: {err}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Width, height and colour type from a PNG header.
    fn png_header(bytes: &[u8]) -> (u32, u32, u8) {
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n", "keine PNG-Signatur");
        assert_eq!(&bytes[12..16], b"IHDR");
        let be = |r: std::ops::Range<usize>| u32::from_be_bytes(bytes[r].try_into().unwrap());
        (be(16..20), be(20..24), bytes[25])
    }

    #[test]
    fn every_bitmap_is_a_square_rgba_png_of_its_size() {
        for (edge, bytes) in BITMAPS {
            // colour type 6: RGBA, so the background can be transparent
            assert_eq!(
                png_header(bytes),
                (edge as u32, edge as u32, 6),
                "{edge} px"
            );
        }
        let edges: Vec<_> = BITMAPS.iter().map(|(e, _)| *e).collect();
        assert!(edges.windows(2).all(|w| w[0] < w[1]), "{edges:?}");
    }

    #[test]
    fn the_logo_is_as_high_as_the_former_icon_and_follows_the_panel() {
        assert_eq!(size_for_panel(32), 22);
        assert_eq!(size_for_panel(24), 16);
        assert_eq!(size_for_panel(48), 33);
        assert_eq!(size_for_panel(128), 88);
        assert_eq!(size_for_panel(16), 11);
        assert_eq!(size_for_panel(0), 1);
    }

    #[test]
    fn the_bitmap_is_never_scaled_up_while_one_is_large_enough() {
        assert_eq!(bitmap_for(22, 1), 22);
        assert_eq!(bitmap_for(16, 1), 16);
        assert_eq!(bitmap_for(22, 2), 48);
        assert_eq!(bitmap_for(33, 1), 48);
        assert_eq!(bitmap_for(88, 2), 256);
        assert_eq!(bitmap_for(200, 2), 256, "größer als alles: die größte");
        assert_eq!(bitmap_for(22, 0), 22, "Skalierung 0 zählt als 1");
    }
}
