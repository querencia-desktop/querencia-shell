//! The background of one monitor (HMP-08): fills itself with the colour
//! and draws the image into the rectangle `config::placement` gives, so the
//! image is never distorted in zoom and scaled mode.

use std::cell::{Cell, RefCell};

use gtk4::prelude::*;
use gtk4::subclass::prelude::*;
use gtk4::{gdk, glib, graphene};

use crate::config::{self, Background};

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct Wallpaper {
        pub texture: RefCell<Option<gdk::Texture>>,
        pub settings: RefCell<Background>,
        /// Last allocated size, logged on change: the surface must cover
        /// the whole monitor, panels included (exclusive zone -1).
        pub size: Cell<(i32, i32)>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Wallpaper {
        const NAME: &'static str = "QuerenciaWallpaper";
        type Type = super::Wallpaper;
        type ParentType = gtk4::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("wallpaper");
        }
    }

    impl ObjectImpl for Wallpaper {}

    impl WidgetImpl for Wallpaper {
        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            self.parent_size_allocate(width, height, baseline);
            if self.size.replace((width, height)) != (width, height) {
                eprintln!("querencia-desktop: Fläche {width}x{height}");
            }
        }

        fn snapshot(&self, snapshot: &gtk4::Snapshot) {
            let widget = self.obj();
            let (w, h) = (f64::from(widget.width()), f64::from(widget.height()));
            let (r, g, b) = self.settings.borrow().color;
            let bounds = graphene::Rect::new(0.0, 0.0, w as f32, h as f32);
            snapshot.append_color(
                &gdk::RGBA::new(
                    f32::from(r) / 255.0,
                    f32::from(g) / 255.0,
                    f32::from(b) / 255.0,
                    1.0,
                ),
                &bounds,
            );
            let texture = self.texture.borrow();
            let Some(texture) = texture.as_ref() else {
                return;
            };
            let img = (f64::from(texture.width()), f64::from(texture.height()));
            let (x, y, tw, th) = config::placement(self.settings.borrow().mode, img, (w, h));
            snapshot.push_clip(&bounds);
            snapshot.append_scaled_texture(
                texture,
                gtk4::gsk::ScalingFilter::Trilinear,
                &graphene::Rect::new(x as f32, y as f32, tw as f32, th as f32),
            );
            snapshot.pop();
        }
    }
}

glib::wrapper! {
    /// A monitor's background (`wallpaper` in CSS).
    pub struct Wallpaper(ObjectSubclass<imp::Wallpaper>)
        @extends gtk4::Widget,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget;
}

impl Default for Wallpaper {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl Wallpaper {
    /// Shows `settings` with `texture` (`None`: colour only).
    pub fn show(&self, settings: &Background, texture: Option<&gdk::Texture>) {
        self.imp().settings.replace(settings.clone());
        self.imp().texture.replace(texture.cloned());
        self.queue_draw();
    }
}
