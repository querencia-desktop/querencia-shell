//! Datei → Eigenschaften (B §1.8, stage 0.1): the tab "Grundlegend" for
//! one entry (or the shown folder without selection), read only: name,
//! type, link target, size, location, modified, accessed. Permissions,
//! "Öffnen mit", folder contents and several entries come with the Beta.

use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{gio, glib};

use crate::fenster::Fenster;
use crate::i18n::{gettext, gettext_f};

const ATTRIBUTE: &str = "standard::display-name,standard::type,standard::size,\
standard::content-type,standard::symlink-target,standard::is-symlink,\
time::modified,time::access";

fn zeit(info: &gio::FileInfo, attr: &str) -> Option<String> {
    if !info.has_attribute(attr) {
        return None;
    }
    let t =
        glib::DateTime::from_unix_local(i64::try_from(info.attribute_uint64(attr)).ok()?).ok()?;
    // TRANSLATORS: date and time in the properties, strftime format;
    // German "%d.%m.%Y %H:%M"
    t.format(&gettext("%m/%d/%Y %I:%M %p"))
        .ok()
        .map(|s| s.to_string())
}

/// Opens the properties of the selection's first entry (or the folder).
pub fn zeigen(fenster: &Rc<Fenster>) {
    let datei = fenster
        .modell()
        .gewaehlte()
        .first()
        .map(|e| e.datei().clone())
        .or_else(|| fenster.ort());
    let Some(datei) = datei else { return };
    let parent: gtk4::Window = fenster.window().clone().upcast();
    let w = Rc::downgrade(fenster);
    glib::spawn_future_local(async move {
        let info = datei
            .query_info_future(
                ATTRIBUTE,
                gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
                glib::Priority::DEFAULT,
            )
            .await;
        let Ok(info) = info else { return };
        if w.upgrade().is_none() {
            return;
        }
        let name = info.display_name().to_string();
        let window = crate::info::dialogfenster(
            &parent,
            &gettext_f("{name} Properties", &[("name", &name)]),
            false,
        );
        window.set_resizable(false);
        let raster = gtk4::Grid::new();
        raster.set_row_spacing(6);
        raster.set_column_spacing(12);
        let typ = info
            .content_type()
            .map(|t| gio::content_type_get_description(&t).to_string())
            .unwrap_or_default();
        let mut zeilen: Vec<(String, String)> =
            vec![(gettext("Name:"), name.clone()), (gettext("Type:"), typ)];
        if info.is_symlink()
            && let Some(ziel) = info.symlink_target()
        {
            zeilen.push((gettext("Link target:"), ziel.display().to_string()));
        }
        if info.file_type() != gio::FileType::Directory {
            zeilen.push((
                gettext("Size:"),
                querencia_dateiarbeit::typ::size_text(u64::try_from(info.size()).unwrap_or(0)),
            ));
        }
        if let Some(eltern) = datei.parent() {
            zeilen.push((gettext("Location:"), eltern.parse_name().to_string()));
        }
        if let Some(t) = zeit(&info, "time::modified") {
            zeilen.push((gettext("Modified:"), t));
        }
        if let Some(t) = zeit(&info, "time::access") {
            zeilen.push((gettext("Accessed:"), t));
        }
        for (i, (k, v)) in zeilen.iter().enumerate() {
            let l = gtk4::Label::new(None);
            l.set_markup(&format!("<b>{}</b>", glib::markup_escape_text(k)));
            l.set_xalign(1.0);
            let wert = gtk4::Label::new(Some(v));
            wert.set_xalign(0.0);
            wert.set_selectable(true);
            wert.set_wrap(true);
            wert.set_max_width_chars(48);
            raster.attach(&l, 0, i as i32, 1, 1);
            raster.attach(&wert, 1, i as i32, 1, 1);
        }
        let zu = gtk4::Button::with_mnemonic(&gettext("_Close"));
        let knoepfe = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        knoepfe.set_halign(gtk4::Align::End);
        knoepfe.append(&zu);
        let aussen = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
        aussen.set_margin_top(12);
        aussen.set_margin_bottom(12);
        aussen.set_margin_start(12);
        aussen.set_margin_end(12);
        let reiter = gtk4::Notebook::new();
        raster.set_margin_top(12);
        raster.set_margin_bottom(12);
        raster.set_margin_start(12);
        raster.set_margin_end(12);
        reiter.append_page(&raster, Some(&gtk4::Label::new(Some(&gettext("Basic")))));
        aussen.append(&reiter);
        aussen.append(&knoepfe);
        window.set_child(Some(&aussen));
        window.set_default_widget(Some(&zu));
        let ww = window.downgrade();
        zu.connect_clicked(move |_| {
            if let Some(w) = ww.upgrade() {
                glib::idle_add_local_once(move || w.close());
            }
        });
        window.present();
        zu.grab_focus();
    });
}
