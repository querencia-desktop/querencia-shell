//! Theme icons without GTK's preloading in the thread pool (QD-106).
//!
//! A `GtkImage` given an icon name or a `GIcon` looks the icon up with
//! `GTK_ICON_LOOKUP_PRELOAD` (GTK 4.22 `gtkiconhelper.c`), and GTK loads it
//! in GLib's thread pool. The `GTask` there holds a reference to the
//! `GtkIconPaintable`. When the image is gone and the icon has dropped out
//! of GTK's LRU list (100 entries) before the task ends, the pool thread
//! drops the last reference. The paintable's finalizer removes it from
//! GTK's icon cache only after the reference count reached zero; a lookup
//! of the same icon on the main thread in between finds it in the cache,
//! `g_object_ref()` fails with `object_already_finalized` and returns NULL,
//! and GTK reads `NULL->desired_size`: the panel crashed this way live on
//! 22.10. 04:07:24 (`libgtk-4.so.1.2200.4[1c5e1e]`, "segfault at 50").
//! The window list and rebuilt menus look icons up all the time, and
//! re-reading the icon theme makes every image look its icon up again.
//!
//! Here the program looks icons up itself, without `PRELOAD`, and gives the
//! image the paintable. GTK then starts no task: the icon is loaded when it
//! is first drawn, on the main thread, and its last reference is always
//! dropped there. The paintables are kept per icon, size and scale until
//! the icon theme changes, so rebuilt menus need no new lookup. Images set
//! here follow theme and scale changes like GTK's own.

use std::cell::RefCell;
use std::collections::HashMap;

use gtk4::prelude::*;
use gtk4::{IconLookupFlags, IconPaintable, IconTheme, Image, TextDirection, gdk, gio, glib};

/// Size of an image without its own pixel size, as GTK's normal icons.
const NORMAL_SIZE: i32 = 16;

/// Paintable cache key: serialized icon, size, scale, right to left.
type Key = (String, i32, i32, bool);

/// An image set here: its icon and the paintable last shown for it.
struct Followed {
    image: glib::WeakRef<Image>,
    icon: gio::Icon,
    shown: Option<IconPaintable>,
}

impl Followed {
    /// The image, while it lives and still shows what was set here (the
    /// applets put their own textures on the same image).
    fn current(&self) -> Option<Image> {
        let image = self.image.upgrade()?;
        let shown = self.shown.as_ref()?.upcast_ref::<gdk::Paintable>();
        (image.paintable().as_ref() == Some(shown)).then_some(image)
    }
}

#[derive(Default)]
struct Store {
    /// Paintables looked up here, until the icon theme changes.
    paintables: HashMap<Key, IconPaintable>,
    /// Images set here, by the image's address, until destroyed.
    images: HashMap<usize, Followed>,
    /// The icon theme whose "changed" signal is connected.
    watched: Option<IconTheme>,
}

thread_local! {
    static STORE: RefCell<Store> = RefCell::new(Store::default());
}

/// A new image showing `icon` at `size` pixels.
pub fn image(icon: &impl IsA<gio::Icon>, size: i32) -> Image {
    let image = Image::new();
    image.set_pixel_size(size);
    set(&image, icon);
    image
}

/// A new image showing the theme icon `name` at `size` pixels.
pub fn image_named(name: &str, size: i32) -> Image {
    image(&gio::ThemedIcon::new(name), size)
}

/// Shows `icon` on `image`, at the image's pixel size (else 16). Replaces
/// `Image::set_from_gicon`.
pub fn set(image: &Image, icon: &impl IsA<gio::Icon>) {
    let icon = icon.as_ref().clone();
    let address = image.as_ptr() as usize;
    let new = STORE.with(|store| {
        let mut store = store.borrow_mut();
        match store.images.get_mut(&address) {
            Some(followed) if followed.image.upgrade().as_ref() == Some(image) => {
                followed.icon = icon.clone();
                false
            }
            // new, or an earlier image at the same address
            _ => {
                let followed = Followed {
                    image: image.downgrade(),
                    icon: icon.clone(),
                    shown: None,
                };
                store.images.insert(address, followed);
                true
            }
        }
    });
    if new {
        image.connect_destroy(move |_| {
            STORE.with(|store| store.borrow_mut().images.remove(&address));
        });
        image.connect_scale_factor_notify(show_again);
        image.connect_direction_changed(|image, _| show_again(image));
    }
    show(image, &icon);
}

/// Shows the theme icon `name` on `image`. Replaces `Image::set_icon_name`.
pub fn set_named(image: &Image, name: &str) {
    set(image, &gio::ThemedIcon::new(name));
}

fn show_again(image: &Image) {
    let address = image.as_ptr() as usize;
    let icon = STORE.with(|store| {
        let store = store.borrow();
        let followed = store.images.get(&address)?;
        followed.current().is_some().then(|| followed.icon.clone())
    });
    if let Some(icon) = icon {
        show(image, &icon);
    }
}

fn show(image: &Image, icon: &gio::Icon) {
    let size = match image.pixel_size() {
        size if size > 0 => size,
        _ => NORMAL_SIZE,
    };
    let scale = image.scale_factor().max(1);
    let direction = match image.direction() {
        TextDirection::None => gtk4::Widget::default_direction(),
        direction => direction,
    };
    let paintable = paintable(&image.display(), icon, size, scale, direction);
    image.set_paintable(Some(&paintable));
    let address = image.as_ptr() as usize;
    STORE.with(|store| {
        if let Some(followed) = store.borrow_mut().images.get_mut(&address) {
            followed.shown = Some(paintable);
        }
    });
}

/// The paintable of `icon`, looked up without `PRELOAD` and kept.
fn paintable(
    display: &gdk::Display,
    icon: &gio::Icon,
    size: i32,
    scale: i32,
    direction: TextDirection,
) -> IconPaintable {
    let theme = IconTheme::for_display(display);
    watch(&theme);
    let lookup = || theme.lookup_by_gicon(icon, size, scale, direction, IconLookupFlags::empty());
    // Icons that cannot be serialized are looked up each time.
    let Some(name) = IconExt::to_string(icon) else {
        return lookup();
    };
    let key = (
        name.to_string(),
        size,
        scale,
        direction == TextDirection::Rtl,
    );
    if let Some(found) = STORE.with(|store| store.borrow().paintables.get(&key).cloned()) {
        return found;
    }
    let found = lookup();
    STORE.with(|store| store.borrow_mut().paintables.insert(key, found.clone()));
    found
}

/// Connects the theme's "changed" signal once: the kept paintables are
/// dropped (on the main thread) and every image looks its icon up again.
fn watch(theme: &IconTheme) {
    let known = STORE.with(|store| store.borrow().watched.as_ref() == Some(theme));
    if known {
        return;
    }
    theme.connect_changed(|_| theme_changed());
    STORE.with(|store| store.borrow_mut().watched = Some(theme.clone()));
}

fn theme_changed() {
    let images: Vec<(Image, gio::Icon)> = STORE.with(|store| {
        let mut store = store.borrow_mut();
        store.paintables.clear();
        store
            .images
            .values()
            .filter_map(|followed| {
                followed
                    .current()
                    .map(|image| (image, followed.icon.clone()))
            })
            .collect()
    });
    eprintln!(
        "querencia-panel: Symbolthema geändert – {} Symbole neu nachgeschlagen",
        images.len()
    );
    for (image, icon) in images {
        show(&image, &icon);
    }
}

/// Number of kept paintables and of images followed (for tests).
#[cfg(test)]
pub fn counts() -> (usize, usize) {
    STORE.with(|store| {
        let store = store.borrow();
        (store.paintables.len(), store.images.len())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// GTK needs a display: only in the isolated session of the panel
    /// check (own labwc, `XDG_RUNTIME_DIR` below a qp24-runtime- directory), never the
    /// running desktop session.
    fn isolated_display() -> gdk::Display {
        let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_default();
        assert!(
            std::env::var("QUERENCIA_ISOLATED_GUI").as_deref() == Ok("1")
                && runtime.contains("/qp24-runtime-"),
            "Abbruch: nur in der isolierten Sitzung (XDG_RUNTIME_DIR={runtime})"
        );
        gtk4::init().expect("GTK in the isolated session");
        gdk::Display::default().expect("Display")
    }

    #[test]
    #[ignore = "nur in der isolierten GUI-Umgebung (Panel-Prüfung 0.2.4)"]
    fn isolated_images_share_one_paintable_and_follow_the_theme() {
        let display = isolated_display();
        let theme = IconTheme::for_display(&display);
        let a = image_named("folder", 16);
        let b = image_named("folder", 16);
        let c = image_named("folder", 24);
        let pa = a.paintable().expect("Paintable a");
        assert_eq!(
            Some(&pa),
            b.paintable().as_ref(),
            "gleiche Größe, gleiches Paintable"
        );
        assert_ne!(
            Some(&pa),
            c.paintable().as_ref(),
            "andere Größe, eigenes Paintable"
        );
        assert!(pa.is::<IconPaintable>());
        let (kept, followed) = counts();
        assert!(kept >= 2 && followed >= 3, "{kept} {followed}");
        // An image that got a texture of its own keeps it.
        let d = image_named("folder", 16);
        let bytes = glib::Bytes::from_static(&[0, 0, 0, 255]);
        let own = gdk::MemoryTexture::new(1, 1, gdk::MemoryFormat::R8g8b8a8, &bytes, 4);
        d.set_paintable(Some(&own));

        // A theme change looks the icons up again: new paintables.
        theme.set_search_path(
            &theme
                .search_path()
                .iter()
                .map(|p| p.as_path())
                .collect::<Vec<_>>(),
        );
        let context = glib::MainContext::default();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while a.paintable().as_ref() == Some(&pa) && std::time::Instant::now() < deadline {
            context.iteration(false);
        }
        assert_ne!(a.paintable().as_ref(), Some(&pa), "nach Themenwechsel neu");
        assert_eq!(a.paintable(), b.paintable());
        assert_eq!(
            d.paintable().as_ref(),
            Some(own.upcast_ref::<gdk::Paintable>())
        );

        // Destroyed images are no longer followed.
        drop((a, b, c, d));
        theme_changed();
        assert_eq!(counts().1, 0);
    }
}
