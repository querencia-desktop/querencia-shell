//! The notification area (Panel 0.4.0, QD-164): the status icons of other
//! programs (Webex, Nextcloud, Discord, …) through the StatusNotifierItem
//! protocol, as the GNOME 2 notification area showed tray icons.
//!
//! - `watcher.rs`: the StatusNotifierWatcher, offered while nobody else
//!   offers one;
//! - `host.rs`: the host, its items and their properties;
//! - `eintrag.rs`: what an item says and how it is shown (pure);
//! - `dbusmenu.rs`: the items' menus (`com.canonical.dbusmenu`) as classic
//!   menus of `querencia-oberflaeche`.
//!
//! Each shown item is a status button like volume and network (same size,
//! same hover look): left click `Activate` (the menu instead for an item
//! that is only a menu, or when `Activate` fails), middle click
//! `SecondaryActivate` (a middle drag moves the applet, `applet_move.rs`),
//! the wheel `Scroll`, right click the item's menu (without one
//! `ContextMenu`). Items with status `Passive` are hidden, `NeedsAttention`
//! shows the attention icon on a highlighted button. Before `Activate` the
//! item gets an xdg-activation token (`ProvideXdgActivationToken`), so a
//! Wayland program may raise its window.
//!
//! Without any shown item the applet is hidden, as an empty GNOME 2
//! notification area took no room. A right click on its edge (the 2 px
//! padding left and right of the icons, or between two of them) opens the
//! applet menu (remove, move; `panel_menu.rs`).

pub mod dbusmenu;
pub mod eintrag;
pub mod host;
pub mod watcher;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, EventControllerScroll, EventControllerScrollFlags,
    EventSequenceState, GestureClick, IconLookupFlags, IconTheme, Image, MenuButton, Orientation,
    Overlay, TextDirection, gdk, gio, glib,
};
use querencia_oberflaeche::classic_menu;

use crate::i18n::gettext;
use crate::logo;
use dbusmenu::Ziel;
use eintrag::{Eintrag, Quelle, Status};
use host::{Host, Item};

/// CSS class of the applet (the row of icons).
pub const CLASS: &str = "querencia-infobereich";
/// CSS class of an item's button (beside `panel::STATUS_CLASS`).
pub const ITEM_CLASS: &str = "querencia-sni";
/// CSS class of a button whose item needs attention.
pub const AUFMERKSAM_CLASS: &str = "aufmerksam";
/// CSS class of the hidden anchor of an item's menu.
const ANKER_CLASS: &str = "querencia-sni-anker";
/// Space between two icons, as between the panel's status applets
/// (`panel::STATUS_SPACING`).
const SPACING: i32 = 2;
/// Icon shown when an item gives nothing drawable.
const ERSATZ: &str = "application-x-executable";

/// The applet's right-click menu (`panel_menu.rs` adds the moves): only
/// „Aus dem Panel entfernen“; the icons have their programs' menus.
pub fn menu() -> gio::Menu {
    let menu = gio::Menu::new();
    let object = gio::Menu::new();
    object.append(
        Some(&gettext("Remove From Panel")),
        Some(crate::panel_menu::REMOVE_ACTION),
    );
    menu.append_section(None, &object);
    menu
}

/// The applet for a panel `panel_height` pixels high.
pub fn widget(panel_height: i32) -> GtkBox {
    let row = GtkBox::new(Orientation::Horizontal, SPACING);
    row.add_css_class(CLASS);
    row.set_valign(Align::Fill);
    row.set_visible(false);
    let Some(host) = Host::holen() else {
        return row;
    };
    let bereich = Rc::new(Bereich {
        host,
        row: row.downgrade(),
        size: logo::size_for_panel(panel_height),
        knoepfe: RefCell::default(),
    });
    let weak = Rc::downgrade(&bereich);
    let callback: Rc<dyn Fn()> = Rc::new(move || {
        if let Some(bereich) = weak.upgrade() {
            bereich.abgleichen();
        }
    });
    bereich.host.beobachten(&callback);
    bereich.abgleichen();
    // Ends with the applet: the callback, the buttons and, with the last
    // applet, host and watcher.
    let halten = RefCell::new(Some((bereich, callback)));
    row.connect_destroy(move |_| {
        if let Some((bereich, _)) = halten.take() {
            for knopf in bereich.knoepfe.take() {
                knopf.anker.popdown();
            }
        }
    });
    row
}

/// One applet.
struct Bereich {
    host: Rc<Host>,
    row: glib::WeakRef<GtkBox>,
    size: i32,
    knoepfe: RefCell<Vec<Rc<Knopf>>>,
}

/// The button of one item.
struct Knopf {
    item: Rc<Item>,
    root: Overlay,
    button: Button,
    image: Image,
    overlay: Image,
    anker: MenuButton,
    /// The model the classic menu shows, filled before each opening.
    menu: gio::Menu,
    gezeigt: RefCell<Option<Eintrag>>,
    /// Set while the menu is loaded or open.
    menu_aktiv: Cell<bool>,
    /// The menu the open one came from (for `Event`).
    ziel: RefCell<Option<Ziel>>,
    size: i32,
}

impl Bereich {
    /// Shows the host's visible items in its order: new buttons for new
    /// items, changed ones drawn again, gone ones removed.
    fn abgleichen(self: &Rc<Self>) {
        let Some(row) = self.row.upgrade() else {
            return;
        };
        let soll: Vec<(Rc<Item>, Eintrag)> = self
            .host
            .items()
            .into_iter()
            .filter(|(_, e)| e.sichtbar())
            .collect();
        let mut alt = self.knoepfe.take();
        let mut neu = Vec::new();
        for (item, eintrag) in &soll {
            let knopf = match alt.iter().position(|k| Rc::ptr_eq(&k.item, item)) {
                Some(i) => alt.remove(i),
                None => {
                    let knopf = Knopf::neu(item.clone(), self.size);
                    row.append(&knopf.root);
                    knopf
                }
            };
            knopf.zeigen(eintrag);
            neu.push(knopf);
        }
        for knopf in alt {
            knopf.anker.popdown();
            row.remove(&knopf.root);
        }
        // the order of the watcher, left to right
        let mut vorher: Option<gtk4::Widget> = None;
        for knopf in &neu {
            row.reorder_child_after(&knopf.root, vorher.as_ref());
            vorher = Some(knopf.root.clone().upcast());
        }
        row.set_visible(!neu.is_empty());
        *self.knoepfe.borrow_mut() = neu;
    }
}

impl Knopf {
    fn neu(item: Rc<Item>, size: i32) -> Rc<Knopf> {
        let image = Image::new();
        image.set_pixel_size(size);
        let overlay = Image::new();
        overlay.set_pixel_size((size / 2).max(1));
        overlay.set_halign(Align::End);
        overlay.set_valign(Align::End);
        overlay.set_can_target(false);
        overlay.set_visible(false);
        let bild = Overlay::new();
        bild.set_child(Some(&image));
        bild.add_overlay(&overlay);
        let button = Button::builder().has_frame(false).child(&bild).build();
        button.add_css_class(crate::panel::STATUS_CLASS);
        button.add_css_class(ITEM_CLASS);

        // the menu's anchor: an empty menu button over the icon that the
        // pointer never hits; its popover is the classic menu
        let anker = MenuButton::new();
        anker.set_child(Some(&GtkBox::new(Orientation::Horizontal, 0)));
        anker.add_css_class(ANKER_CLASS);
        anker.set_can_target(false);
        anker.set_focus_on_click(false);
        let menu = gio::Menu::new();
        classic_menu::attach_popup(&anker, menu.upcast_ref());
        let root = Overlay::new();
        root.set_child(Some(&button));
        root.add_overlay(&anker);

        let knopf = Rc::new(Knopf {
            item,
            root,
            button: button.clone(),
            image,
            overlay,
            anker: anker.clone(),
            menu,
            gezeigt: RefCell::default(),
            menu_aktiv: Cell::new(false),
            ziel: RefCell::default(),
            size,
        });

        let klick = gio::SimpleAction::new(dbusmenu::KLICK, Some(glib::VariantTy::INT32));
        let weak = Rc::downgrade(&knopf);
        klick.connect_activate(move |_, id| {
            let (Some(knopf), Some(id)) = (weak.upgrade(), id.and_then(|v| v.get::<i32>())) else {
                return;
            };
            if let Some(ziel) = knopf.ziel.borrow().as_ref() {
                eprintln!(
                    "querencia-panel: Infobereich: {}: Menü → Eintrag {id}",
                    knopf.item.key
                );
                ziel.ereignis(id, "clicked");
            }
        });
        let actions = gio::SimpleActionGroup::new();
        actions.add_action(&klick);
        anker.insert_action_group(dbusmenu::ACTION_GROUP, Some(&actions));
        if let Some(popover) = anker.popover() {
            let weak = Rc::downgrade(&knopf);
            popover.connect_closed(move |_| {
                let Some(knopf) = weak.upgrade() else { return };
                knopf.menu_aktiv.set(false);
                if let Some(ziel) = knopf.ziel.borrow().as_ref() {
                    ziel.ereignis(0, "closed");
                }
            });
        }

        let weak = Rc::downgrade(&knopf);
        button.connect_clicked(move |_| {
            if let Some(knopf) = weak.upgrade() {
                knopf.links();
            }
        });
        let rechts = GestureClick::new();
        rechts.set_button(gdk::BUTTON_SECONDARY);
        let weak = Rc::downgrade(&knopf);
        rechts.connect_pressed(move |gesture, presses, _, _| {
            // the item's menu, not the panel's (`panel_menu.rs`)
            gesture.set_state(EventSequenceState::Claimed);
            if presses == 1
                && let Some(knopf) = weak.upgrade()
            {
                knopf.rechts();
            }
        });
        button.add_controller(rechts);
        // On release, and not claimed at the press: dragging with the
        // middle button moves the applet (applet_move.rs).
        let mitte = GestureClick::new();
        mitte.set_button(gdk::BUTTON_MIDDLE);
        let weak = Rc::downgrade(&knopf);
        mitte.connect_released(move |_, _, _, _| {
            if crate::applet_move::dragged_recently() {
                return;
            }
            if let Some(knopf) = weak.upgrade() {
                eprintln!(
                    "querencia-panel: Infobereich: {}: mittlere Taste → SecondaryActivate",
                    knopf.item.key
                );
                knopf.item.token_geben(&aktivierungs_token(&knopf.button));
                knopf
                    .item
                    .aufrufen("SecondaryActivate", (0i32, 0i32).to_variant(), |_| {});
            }
        });
        button.add_controller(mitte);
        let rad = EventControllerScroll::new(
            EventControllerScrollFlags::BOTH_AXES | EventControllerScrollFlags::DISCRETE,
        );
        let weak = Rc::downgrade(&knopf);
        rad.connect_scroll(move |_, dx, dy| {
            let Some(knopf) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            // KDE's delta: 120 per notch, positive upwards
            for (wert, richtung) in [(dy, "vertical"), (dx, "horizontal")] {
                if wert != 0.0 {
                    let delta = (-wert * 120.0).round() as i32;
                    eprintln!(
                        "querencia-panel: Infobereich: {}: Mausrad {richtung} {delta}",
                        knopf.item.key
                    );
                    knopf
                        .item
                        .aufrufen("Scroll", (delta, richtung).to_variant(), |_| {});
                }
            }
            glib::Propagation::Stop
        });
        button.add_controller(rad);
        let weak = Rc::downgrade(&knopf);
        button.connect_scale_factor_notify(move |_| {
            if let Some(knopf) = weak.upgrade() {
                let eintrag = knopf.gezeigt.take();
                if let Some(eintrag) = eintrag {
                    knopf.zeigen(&eintrag);
                }
            }
        });
        knopf
    }

    /// Draws `eintrag` unless it is shown already.
    fn zeigen(&self, eintrag: &Eintrag) {
        if self.gezeigt.borrow().as_ref() == Some(eintrag) {
            return;
        }
        let scale = self.button.scale_factor().max(1);
        let pfad = &eintrag.icon_theme_path;
        if !zeichnen(&self.image, &eintrag.symbol(), pfad, self.size, scale) {
            querencia_oberflaeche::symbole::set_named(&self.image, ERSATZ);
        }
        let overlay = eintrag.overlay();
        let mit_overlay = !overlay.is_empty()
            && zeichnen(&self.overlay, &overlay, pfad, (self.size / 2).max(1), scale);
        self.overlay.set_visible(mit_overlay);
        self.button.set_tooltip_text(Some(&eintrag.tooltip_text()));
        if eintrag.status == Status::NeedsAttention {
            self.button.add_css_class(AUFMERKSAM_CLASS);
        } else {
            self.button.remove_css_class(AUFMERKSAM_CLASS);
        }
        self.gezeigt.replace(Some(eintrag.clone()));
    }

    fn eintrag(&self) -> Option<Eintrag> {
        self.gezeigt.borrow().clone()
    }

    fn links(self: &Rc<Self>) {
        let Some(eintrag) = self.eintrag() else {
            return;
        };
        let key = &self.item.key;
        if eintrag.item_is_menu {
            eprintln!("querencia-panel: Infobereich: {key}: Linksklick → Menü (ItemIsMenu)");
            self.menu_oeffnen();
            return;
        }
        eprintln!("querencia-panel: Infobereich: {key}: Linksklick → Activate");
        self.item.token_geben(&aktivierungs_token(&self.button));
        let weak = Rc::downgrade(self);
        self.item
            .aufrufen("Activate", (0i32, 0i32).to_variant(), move |_| {
                // a program without Activate: its menu, if it has one
                if let Some(knopf) = weak.upgrade()
                    && knopf.item.menu_ziel().is_some()
                {
                    knopf.menu_oeffnen();
                }
            });
    }

    fn rechts(self: &Rc<Self>) {
        if self.item.menu_ziel().is_some() {
            eprintln!(
                "querencia-panel: Infobereich: {}: Rechtsklick → Menü",
                self.item.key
            );
            self.menu_oeffnen();
        } else {
            self.kontextmenue();
        }
    }

    fn kontextmenue(&self) {
        eprintln!(
            "querencia-panel: Infobereich: {}: Rechtsklick → ContextMenu",
            self.item.key
        );
        self.item
            .aufrufen("ContextMenu", (0i32, 0i32).to_variant(), |_| {});
    }

    /// Reads the item's menu and opens it; without a readable menu the
    /// item gets `ContextMenu`.
    fn menu_oeffnen(self: &Rc<Self>) {
        let Some(ziel) = self.item.menu_ziel() else {
            self.kontextmenue();
            return;
        };
        if self.menu_aktiv.replace(true) {
            return;
        }
        let weak = Rc::downgrade(self);
        let geladen = ziel.clone();
        ziel.laden(move |result| {
            let Some(knopf) = weak.upgrade() else { return };
            let root = match result {
                Ok(root) if root.children.iter().any(|c| c.visible && !c.separator) => root,
                Ok(_) => {
                    eprintln!(
                        "querencia-panel: Infobereich: {}: Menü leer, ContextMenu",
                        knopf.item.key
                    );
                    knopf.menu_aktiv.set(false);
                    knopf.kontextmenue();
                    return;
                }
                Err(err) => {
                    eprintln!(
                        "querencia-panel: Infobereich: {}: Menü nicht lesbar ({err}), ContextMenu",
                        knopf.item.key
                    );
                    knopf.menu_aktiv.set(false);
                    knopf.kontextmenue();
                    return;
                }
            };
            // the item may have gone meanwhile
            if knopf.root.parent().is_none() {
                knopf.menu_aktiv.set(false);
                return;
            }
            let neu = dbusmenu::menu(&root);
            knopf.menu.remove_all();
            for i in 0..neu.n_items() {
                knopf.menu.append_item(&gio::MenuItem::from_model(&neu, i));
            }
            eprintln!(
                "querencia-panel: Infobereich: {}: Menü mit {} Einträgen geöffnet",
                knopf.item.key,
                zaehlen(&root)
            );
            knopf.ziel.replace(Some(geladen.clone()));
            // as a click on a menu button would: the anchor takes the focus
            // inside the panel window (GTK reports a critical when a popover
            // opens while the window has no focus widget at all)
            knopf.anker.grab_focus();
            knopf.anker.popup();
            geladen.ereignis(0, "opened");
        });
    }
}

/// Entries of a layout (without separators and hidden ones), for the log.
fn zaehlen(node: &dbusmenu::Knoten) -> usize {
    node.children
        .iter()
        .filter(|c| c.visible && !c.separator)
        .map(|c| 1 + zaehlen(c))
        .sum()
}

/// Draws the first of `quellen` that can be drawn, `size` logical pixels at
/// `scale`. Theme icons go through `symbole` (no preloading, QD-106), icons
/// of the item's own folder are looked up there without preloading too.
fn zeichnen(image: &Image, quellen: &[Quelle], pfad: &str, size: i32, scale: i32) -> bool {
    let Some(display) = gdk::Display::default() else {
        return false;
    };
    let pixels = size * scale;
    for quelle in quellen {
        match quelle {
            Quelle::Name(name) => {
                if !pfad.is_empty()
                    && let Some(thema) = eigenes_thema(&display, pfad)
                    && thema.has_icon(name)
                {
                    let paintable = thema.lookup_icon(
                        name,
                        &[],
                        size,
                        scale,
                        TextDirection::None,
                        IconLookupFlags::empty(),
                    );
                    image.set_paintable(Some(&paintable));
                    return true;
                }
                if IconTheme::for_display(&display).has_icon(name) {
                    querencia_oberflaeche::symbole::set_named(image, name);
                    return true;
                }
            }
            Quelle::Datei(datei) => {
                match gtk4::gdk_pixbuf::Pixbuf::from_file_at_scale(datei, pixels, pixels, true) {
                    Ok(pixbuf) => {
                        image.set_paintable(Some(&gdk::Texture::for_pixbuf(&pixbuf)));
                        return true;
                    }
                    Err(err) => eprintln!(
                        "querencia-panel: Infobereich: Symbol {} nicht lesbar: {err}",
                        datei.display()
                    ),
                }
            }
            Quelle::Pixmaps(pixmaps) => {
                if let Some(texture) = pixmap_texture(pixmaps, pixels) {
                    image.set_paintable(Some(&texture));
                    return true;
                }
            }
        }
    }
    false
}

/// The best of `pixmaps` scaled to `pixels` (the longer side).
fn pixmap_texture(pixmaps: &[eintrag::Pixmap], pixels: i32) -> Option<gdk::Texture> {
    use gtk4::gdk_pixbuf::{Colorspace, InterpType, Pixbuf};
    let pixmap = eintrag::best_pixmap(pixmaps, pixels)?;
    let pixbuf = Pixbuf::from_bytes(
        &glib::Bytes::from_owned(pixmap.rgba()),
        Colorspace::Rgb,
        true,
        8,
        pixmap.width,
        pixmap.height,
        pixmap.width * 4,
    );
    let side = pixmap.width.max(pixmap.height);
    let pixbuf = if side == pixels {
        pixbuf
    } else {
        let w = (pixmap.width * pixels / side).max(1);
        let h = (pixmap.height * pixels / side).max(1);
        pixbuf.scale_simple(w, h, InterpType::Hyper)?
    };
    Some(gdk::Texture::for_pixbuf(&pixbuf))
}

thread_local! {
    /// Icon themes of the items' own folders (`IconThemePath`), by folder
    /// and the user's theme name they were made for.
    static THEMEN: RefCell<HashMap<(PathBuf, String), IconTheme>> = RefCell::default();
}

/// An icon theme that looks in `pfad` first, then where the display's theme
/// looks, with the user's theme name.
fn eigenes_thema(display: &gdk::Display, pfad: &str) -> Option<IconTheme> {
    let pfad = PathBuf::from(pfad);
    if !pfad.is_absolute() {
        return None;
    }
    let standard = IconTheme::for_display(display);
    let name = standard.theme_name().to_string();
    let key = (pfad.clone(), name.clone());
    THEMEN.with(|themen| {
        let mut themen = themen.borrow_mut();
        if let Some(thema) = themen.get(&key) {
            return Some(thema.clone());
        }
        // a theme switch makes the old entries useless
        themen.retain(|(_, n), _| *n == name);
        let thema = IconTheme::new();
        let mut such = vec![pfad];
        such.extend(standard.search_path());
        let such: Vec<&std::path::Path> = such.iter().map(PathBuf::as_path).collect();
        thema.set_search_path(&such);
        thema.set_theme_name(Some(&name));
        themen.insert(key, thema.clone());
        Some(thema)
    })
}

/// An xdg-activation token for the window the program raises, from the
/// panel's display (the click is the last input); empty if there is none.
fn aktivierungs_token(button: &Button) -> String {
    button
        .display()
        .app_launch_context()
        .startup_notify_id(None::<&gio::AppInfo>, &[])
        .map(|id| id.to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applet_menu_only_removes() {
        let menu = menu();
        assert_eq!(menu.n_items(), 1);
        let section = menu.item_link(0, "section").expect("Abschnitt");
        let action = section
            .item_attribute_value(0, "action", Some(glib::VariantTy::STRING))
            .and_then(|v| v.get::<String>());
        assert_eq!(action.as_deref(), Some(crate::panel_menu::REMOVE_ACTION));
    }

    #[test]
    fn pixmaps_are_scaled_to_the_icon_size() {
        let p = |w: i32, h: i32| eintrag::Pixmap {
            width: w,
            height: h,
            data: vec![0xff; (w * h * 4) as usize],
        };
        let t = pixmap_texture(&[p(16, 16), p(64, 32)], 22).unwrap();
        assert_eq!((t.width(), t.height()), (22, 11));
        let t = pixmap_texture(&[p(22, 22)], 22).unwrap();
        assert_eq!((t.width(), t.height()), (22, 22));
        assert!(pixmap_texture(&[], 22).is_none());
    }

    #[test]
    fn class_names_do_not_clash() {
        assert_ne!(CLASS, crate::panel::STATUS_CLASS);
        assert_ne!(ITEM_CLASS, crate::panel::STATUS_CLASS);
        assert_ne!(dbusmenu::ACTION_GROUP, "panel");
        assert!(!["glocke", "volume", "network", "menu"].contains(&dbusmenu::ACTION_GROUP));
    }
}
