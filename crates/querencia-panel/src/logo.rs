//! The Querencia logo before "Anwendungen" (HMP-11a): the orange Q of
//! variant C2 ("Harmonisch verbunden"), in place of the distribution logo
//! `start-here` that HMP-02 to HMP-11 showed there. The image files are the
//! ones the project lead chose (`data/logo/`, with the SVG master), built
//! into the binary, so the logo needs no installed file and no icon theme;
//! the theme's `start-here` stays untouched. QREL-04c: the same logo also
//! replaces `start-here` in "Informationen über Querencia" (`about.rs`) and
//! in the "Zum Panel hinzufügen" row of "Anwendungen" (`add_dialog.rs`,
//! marked by [`ICON_NAME`]), so no Debian swirl is left in the panel.
//!
//! The logo is as high as the former icon on the standard panel (22 px on
//! 32 px) and follows the panel height in the same ratio. Of the bitmaps the
//! smallest one at least as large as needed (times the display scale) is
//! drawn, so it is only ever scaled down, square, never distorted.

use gtk4::prelude::*;
use gtk4::{Image, gdk, gio, glib};

/// Icon name an applet type gives (`applets.rs`) to show the Querencia logo
/// instead of a theme icon. No theme has it; the panel draws the built-in
/// bitmaps for it.
pub const ICON_NAME: &str = "querencia-logo";

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

/// Window icon of the panel's own windows (QD-129): the SVG master of the
/// logo in the built-in resource (`data/panel.gresource`), under a name no
/// theme has. labwc finds no starter for the panel's app ID and so shows
/// the icon the window sends.
pub const WINDOW_ICON: &str = "org.querencia.Panel-fenster";
const RESOURCE: &[u8] = include_bytes!("../data/panel.gresource");
const RESOURCE_ICONS: &str = "/org/querencia/Panel/icons";

/// Registers the resource and makes [`WINDOW_ICON`] the icon of every
/// window of this process; before the first window.
pub fn set_window_icon(display: &gdk::Display) {
    match gio::Resource::from_data(&glib::Bytes::from_static(RESOURCE)) {
        Ok(resource) => {
            gio::resources_register(&resource);
            gtk4::IconTheme::for_display(display).add_resource_path(RESOURCE_ICONS);
            gtk4::Window::set_default_icon_name(WINDOW_ICON);
        }
        Err(err) => eprintln!("querencia-panel: Fenstersymbol nicht lesbar: {err}"),
    }
}

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
    image_of_size(size_for_panel(panel_height))
}

/// The logo `size` logical pixels square, e.g. 64 in the about window, like
/// [`image`] otherwise.
pub fn image_of_size(size: i32) -> Image {
    let image = Image::new();
    image.set_pixel_size(size);
    show(&image, size);
    image.connect_scale_factor_notify(move |image| show(image, size));
    image
}

/// The PNG bytes of the bitmap with edge length `edge` (one of
/// [`bitmap_for`]'s results).
fn bitmap(edge: i32) -> &'static [u8] {
    BITMAPS
        .iter()
        .find(|(e, _)| *e == edge)
        .map(|(_, bytes)| *bytes)
        .unwrap_or_default()
}

/// The logo as a `GIcon` for places that take one (the window list), from
/// the bitmap for `size` logical pixels at scale 1.
pub fn gicon(size: i32) -> gio::Icon {
    let bytes = glib::Bytes::from_static(bitmap(bitmap_for(size, 1)));
    gio::BytesIcon::new(&bytes).upcast()
}

fn show(image: &Image, size: i32) {
    let edge = bitmap_for(size, image.scale_factor());
    let bytes = bitmap(edge);
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

    /// The resource holds the logo's SVG master byte for byte under the
    /// window icon's name, and nothing else (no PNG next to the SVG).
    #[test]
    fn resource_holds_the_logo_svg_as_window_icon() {
        let resource = gio::Resource::from_data(&glib::Bytes::from_static(RESOURCE)).unwrap();
        let apps = format!("{RESOURCE_ICONS}/scalable/apps");
        let names = resource
            .enumerate_children(&apps, gio::ResourceLookupFlags::NONE)
            .unwrap();
        assert_eq!(names, vec![format!("{WINDOW_ICON}.svg")]);
        let svg = resource
            .lookup_data(
                &format!("{apps}/{WINDOW_ICON}.svg"),
                gio::ResourceLookupFlags::NONE,
            )
            .unwrap();
        assert_eq!(&svg[..], include_bytes!("../data/logo/querencia-logo.svg"));
        assert!(
            !String::from_utf8_lossy(&svg).contains("<text"),
            "kein Text im Symbol"
        );
        let icons = resource
            .enumerate_children(RESOURCE_ICONS, gio::ResourceLookupFlags::NONE)
            .unwrap();
        assert_eq!(icons, vec!["scalable/".to_string()]);
    }

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

    /// QREL-04c: no surface of the panel asks the icon theme for the
    /// distribution logo any more (GNOME Human resolves it to the Debian
    /// swirl); "Anwendungen" carries the Querencia logo instead.
    #[test]
    fn no_panel_surface_shows_the_distribution_logo() {
        let sources = [
            ("about.rs", include_str!("about.rs")),
            ("add_dialog.rs", include_str!("add_dialog.rs")),
            ("app_chooser.rs", include_str!("app_chooser.rs")),
            ("app_menu.rs", include_str!("app_menu.rs")),
            ("applets.rs", include_str!("applets.rs")),
            ("launcher.rs", include_str!("launcher.rs")),
            ("panel_menu.rs", include_str!("panel_menu.rs")),
            ("places_menu.rs", include_str!("places_menu.rs")),
            ("system_menu.rs", include_str!("system_menu.rs")),
            ("window_icons.rs", include_str!("window_icons.rs")),
            ("window_list.rs", include_str!("window_list.rs")),
            (
                "workspace_switcher.rs",
                include_str!("workspace_switcher.rs"),
            ),
        ];
        for (file, text) in sources {
            for name in ["start-here", "distributor-logo"] {
                assert!(!text.contains(&format!("\"{name}")), "{file}: {name}");
            }
        }
        assert_eq!(
            crate::applets::find("applications").unwrap().icon,
            ICON_NAME
        );
    }

    #[test]
    fn the_panels_own_windows_get_the_logo_in_the_window_list() {
        let own = crate::window_icons::icon_for(crate::APP_ID);
        let bytes = own
            .downcast_ref::<gio::BytesIcon>()
            .expect("eingebautes Logo")
            .bytes();
        assert_eq!(png_header(&bytes), (16, 16, 6));
        // every other window keeps its own lookup and placeholder
        let other = crate::window_icons::icon_for("unbekannt.app");
        assert!(other.downcast_ref::<gio::BytesIcon>().is_none());
    }
}
