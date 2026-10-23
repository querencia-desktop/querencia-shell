//! The window "Eigenschaften" (`win.eigenschaften`) after Nautilus 2.30
//! (`fm-properties-window.c`), Recherche C §6:
//!
//! - **Grundlegend:** symbol, name (renaming on Enter), type, link target,
//!   size – for folders the contents, counted in a thread of its own with
//!   running numbers ("12 Objekte, insgesamt 3,4 MB") –, location, free
//!   space, accessed, modified.
//! - **Zugriffsrechte:** owner and group (shown), access for owner, group
//!   and others as Nautilus 2.30's lists, "Datei als Programm ausführen";
//!   a change is written at once (`unix::mode`). Only the owner changes
//!   them; places without `unix::mode` show a note.
//! - **Öffnen mit** (files): the programs for the type; choosing one makes
//!   it the default (`set_as_default_for_type`, `~/.config/mimeapps.list`).
//! - No emblems, no notes (B §1.8: never).
//!
//! QD-076: built anew every time, destroyed on closing.

use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gtk4::prelude::*;
use gtk4::{Align, gdk, gio, glib};
use querencia_dateiarbeit::{oeffnen, typ};
use querencia_oberflaeche::symbole;

use crate::i18n::{gettext, gettext_f, ngettext_f};
use crate::operationen::{Operationen, fehler, protokoll};

const ATTRIBUTE: &str = "standard::*,time::modified,time::access,unix::mode,unix::uid,\
                         owner::user,owner::group,access::can-rename,access::can-write";

/// Who a permission line is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Klasse {
    Besitzer,
    Gruppe,
    Andere,
}

impl Klasse {
    fn shift(self) -> u32 {
        match self {
            Klasse::Besitzer => 6,
            Klasse::Gruppe => 3,
            Klasse::Andere => 0,
        }
    }
}

/// The choices of an access list: files "Keine / Nur lesen / Lesen und
/// Schreiben", folders "Keine / Nur Dateien auflisten / Zugriff auf Dateien /
/// Dateien erstellen und löschen" (the owner has no "Keine", as Nautilus).
pub fn stufen(ordner: bool, klasse: Klasse) -> Vec<String> {
    let mut list = Vec::new();
    if klasse != Klasse::Besitzer {
        list.push(gettext("None"));
    }
    if ordner {
        list.push(gettext("List files only"));
        list.push(gettext("Access files"));
        list.push(gettext("Create and delete files"));
    } else {
        list.push(gettext("Read-only"));
        list.push(gettext("Read and write"));
    }
    list
}

/// The bits (rwx) of each choice, in the order of [`stufen`] with "Keine".
fn stufen_bits(ordner: bool) -> &'static [u32] {
    if ordner {
        &[0o0, 0o4, 0o5, 0o7]
    } else {
        &[0o0, 0o4, 0o6]
    }
}

/// Which choice `mode` is for `klasse` (index into [`stufen`]); `None` when
/// the bits fit none of them (e.g. write without read: shown as "—").
pub fn stufe(mode: u32, ordner: bool, klasse: Klasse) -> Option<usize> {
    let mut bits = (mode >> klasse.shift()) & 0o7;
    if !ordner {
        // the x bit of files has its own check box
        bits &= 0o6;
    }
    let index = stufen_bits(ordner).iter().position(|b| *b == bits)?;
    if klasse == Klasse::Besitzer {
        index.checked_sub(1)
    } else {
        Some(index)
    }
}

/// `mode` with choice `index` of [`stufen`] for `klasse`; other bits stay
/// (for files the x bit too).
pub fn mit_stufe(mode: u32, ordner: bool, klasse: Klasse, index: usize) -> u32 {
    let index = if klasse == Klasse::Besitzer {
        index + 1
    } else {
        index
    };
    let Some(bits) = stufen_bits(ordner).get(index) else {
        return mode;
    };
    let shift = klasse.shift();
    let keep = if ordner { 0 } else { (mode >> shift) & 0o1 };
    (mode & !(0o7 << shift)) | ((bits | keep) << shift)
}

/// "Datei als Programm ausführen": x for every class that may read.
pub fn mit_ausfuehren(mode: u32, an: bool) -> u32 {
    let mut mode = mode;
    for klasse in [Klasse::Besitzer, Klasse::Gruppe, Klasse::Andere] {
        let shift = klasse.shift();
        if an && (mode >> shift) & 0o4 != 0 {
            mode |= 0o1 << shift;
        } else if !an {
            mode &= !(0o1 << shift);
        }
    }
    mode
}

fn zeit(dt: Option<glib::DateTime>) -> String {
    dt.and_then(|t| t.to_local().ok())
        // TRANSLATORS: date and time of the last change (strftime
        // format of GLib's GDateTime)
        .and_then(|t| t.format(&gettext("%Y-%m-%d %H:%M")).ok())
        .map(|t| t.to_string())
        .unwrap_or_else(|| gettext("unknown"))
}

fn zeile(grid: &gtk4::Grid, row: i32, key: &str, value: &impl IsA<gtk4::Widget>) {
    let label = gtk4::Label::new(None);
    label.set_markup(&format!("<b>{}</b>", glib::markup_escape_text(key)));
    label.set_halign(Align::End);
    label.set_valign(Align::Start);
    grid.attach(&label, 0, row, 1, 1);
    value.set_halign(Align::Start);
    value.set_hexpand(true);
    grid.attach(value, 1, row, 1, 1);
}

fn wert(text: &str) -> gtk4::Label {
    let label = gtk4::Label::new(Some(text));
    label.set_xalign(0.0);
    label.set_wrap(true);
    label.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
    label.set_max_width_chars(48);
    label.set_selectable(true);
    // selectable with the mouse, but no pre-selected text on opening
    label.set_focusable(false);
    label
}

fn raster() -> gtk4::Grid {
    gtk4::Grid::builder()
        .row_spacing(6)
        .column_spacing(12)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build()
}

/// Counts a folder in a thread of its own (not GLib's pool, QD-106); the
/// numbers are read from the main thread until it ends.
#[derive(Default)]
struct Zaehlung {
    items: u64,
    bytes: u64,
    unlesbar: bool,
    fertig: bool,
}

fn zaehlen(dir: gio::File, stand: Arc<Mutex<Zaehlung>>, stopp: Arc<AtomicBool>) {
    let fs = |f: &gio::File| {
        f.query_info(
            "id::filesystem",
            gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
            gio::Cancellable::NONE,
        )
        .ok()
        .and_then(|i| i.attribute_string("id::filesystem"))
    };
    let eigenes = fs(&dir);
    let mut stapel = vec![dir];
    while let Some(ordner) = stapel.pop() {
        if stopp.load(Ordering::Relaxed) {
            return;
        }
        let kinder = match ordner.enumerate_children(
            "standard::name,standard::type,standard::size,id::filesystem",
            gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
            gio::Cancellable::NONE,
        ) {
            Ok(kinder) => kinder,
            Err(_) => {
                if let Ok(mut s) = stand.lock() {
                    s.unlesbar = true;
                }
                continue;
            }
        };
        while let Ok(Some(info)) = kinder.next_file(gio::Cancellable::NONE) {
            let mut s = match stand.lock() {
                Ok(s) => s,
                Err(_) => return,
            };
            s.items += 1;
            s.bytes += u64::try_from(info.size()).unwrap_or(0);
            drop(s);
            // NO_XDEV: no other file system, as Nautilus counted
            if info.file_type() == gio::FileType::Directory
                && info.attribute_string("id::filesystem") == eigenes
            {
                stapel.push(ordner.child(info.name()));
            }
        }
    }
    if let Ok(mut s) = stand.lock() {
        s.fertig = true;
    }
}

fn inhalt_text(s: &Zaehlung) -> String {
    let text = ngettext_f(
        "{n} item, totalling {size}",
        "{n} items, totalling {size}",
        u32::try_from(s.items).unwrap_or(u32::MAX),
        &[("size", &typ::size_text(s.bytes))],
    );
    if s.unlesbar {
        gettext_f(
            "{contents} (some contents unreadable)",
            &[("contents", &text)],
        )
    } else {
        text
    }
}

/// Shows the properties of `datei` (a window of its own, built anew).
pub fn zeigen(parent: Option<&gtk4::Window>, datei: gio::File, ops: &Rc<Operationen>) {
    let info = match datei.query_info(
        ATTRIBUTE,
        gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
        gio::Cancellable::NONE,
    ) {
        Ok(info) => info,
        Err(err) => {
            fehler(
                parent,
                &gettext_f(
                    "The properties of “{name}” cannot be read.",
                    &[("name", &querencia_dateiarbeit::uri::name_of(&datei.uri()))],
                ),
                err.message(),
            );
            return;
        }
    };
    let name = info.display_name().to_string();
    let ordner = info.file_type() == gio::FileType::Directory;
    let window = gtk4::Window::builder()
        .title(gettext_f("“{name}” Properties", &[("name", &name)]))
        .resizable(false)
        .build();
    window.set_transient_for(parent);
    window.add_css_class("querencia-dateien-properties");
    let notebook = gtk4::Notebook::new();
    let stopp = Arc::new(AtomicBool::new(false));
    notebook.append_page(
        &grundlegend(&datei, &info, ops, &window, stopp.clone()),
        Some(&gtk4::Label::new(Some(&gettext("Basic")))),
    );
    notebook.append_page(
        &zugriffsrechte(&datei, &info, &window),
        Some(&gtk4::Label::new(Some(&gettext("Permissions")))),
    );
    if !ordner && info.file_type() != gio::FileType::SymbolicLink {
        let content_type = info
            .content_type()
            .map(|t| t.to_string())
            .unwrap_or_else(|| "application/octet-stream".to_owned());
        notebook.append_page(
            &oeffnen_mit(&content_type),
            Some(&gtk4::Label::new(Some(&gettext("Open With")))),
        );
    }
    let outer = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    outer.set_margin_top(6);
    outer.set_margin_bottom(12);
    outer.set_margin_start(6);
    outer.set_margin_end(6);
    outer.append(&notebook);
    let close = gtk4::Button::with_mnemonic(&gettext("_Close"));
    close.set_halign(Align::End);
    close.set_margin_end(6);
    let w = window.downgrade();
    close.connect_clicked(move |_| {
        if let Some(w) = w.upgrade() {
            w.destroy();
        }
    });
    outer.append(&close);
    window.set_child(Some(&outer));
    window.set_default_widget(Some(&close));
    let keys = gtk4::EventControllerKey::new();
    let w = window.downgrade();
    keys.connect_key_pressed(move |_, key, _, _| {
        if key == gdk::Key::Escape
            && let Some(w) = w.upgrade()
        {
            w.destroy();
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    window.add_controller(keys);
    window.connect_destroy(move |_| stopp.store(true, Ordering::Relaxed));
    window.present();
    close.grab_focus();
    protokoll(&format!("Eigenschaften: {name}"));
}

fn grundlegend(
    datei: &gio::File,
    info: &gio::FileInfo,
    ops: &Rc<Operationen>,
    window: &gtk4::Window,
    stopp: Arc<AtomicBool>,
) -> gtk4::Grid {
    let grid = raster();
    let ordner = info.file_type() == gio::FileType::Directory;
    let icon = symbole::image(&typ::icon_for(datei, info), 48);
    icon.set_valign(Align::Start);
    icon.set_halign(Align::Start);
    let mut row = 0;
    grid.attach(&icon, 0, row, 1, 1);
    row += 1;

    let name = gtk4::Entry::new();
    name.set_text(&info.display_name());
    name.set_width_chars(32);
    let kann = info.boolean("access::can-rename") || !info.has_attribute("access::can-rename");
    name.set_editable(kann);
    name.set_sensitive(kann);
    let weak = Rc::downgrade(ops);
    let datei_name = datei.clone();
    let w = window.downgrade();
    name.connect_activate(move |entry| {
        if let Some(ops) = weak.upgrade() {
            ops.umbenennen(&datei_name, &entry.text());
        }
        if let Some(w) = w.upgrade() {
            w.destroy();
        }
    });
    zeile(&grid, row, &gettext("Name:"), &name);
    row += 1;

    let content_type = info
        .content_type()
        .map(|t| t.to_string())
        .unwrap_or_default();
    let art = if info.file_type() == gio::FileType::SymbolicLink {
        gettext("Link")
    } else {
        format!(
            "{} ({content_type})",
            gio::content_type_get_description(&content_type)
        )
    };
    zeile(&grid, row, &gettext("Type:"), &wert(&art));
    row += 1;
    if let Some(ziel) = info.symlink_target() {
        zeile(
            &grid,
            row,
            &gettext("Link target:"),
            &wert(&ziel.display().to_string()),
        );
        row += 1;
    }
    if ordner {
        let inhalt = wert(&gettext("counting …"));
        zeile(&grid, row, &gettext("Contents:"), &inhalt);
        row += 1;
        let stand: Arc<Mutex<Zaehlung>> = Arc::default();
        let dir = datei.clone();
        let faden_stand = stand.clone();
        let started = std::thread::Builder::new()
            .name("dateien-groesse".to_owned())
            .spawn(move || zaehlen(dir, faden_stand, stopp));
        if started.is_ok() {
            let inhalt = inhalt.downgrade();
            glib::timeout_add_local(Duration::from_millis(200), move || {
                let Some(label) = inhalt.upgrade() else {
                    return glib::ControlFlow::Break;
                };
                let Ok(s) = stand.lock() else {
                    return glib::ControlFlow::Break;
                };
                label.set_text(&inhalt_text(&s));
                if s.fertig {
                    protokoll(&format!(
                        "Ordnergröße: {} Objekte, {} Bytes",
                        s.items, s.bytes
                    ));
                    glib::ControlFlow::Break
                } else {
                    glib::ControlFlow::Continue
                }
            });
        }
    } else {
        zeile(
            &grid,
            row,
            &gettext("Size:"),
            &wert(&typ::size_text(u64::try_from(info.size()).unwrap_or(0))),
        );
        row += 1;
    }
    let ort = datei
        .parent()
        .map(|p| {
            p.path()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| p.uri().to_string())
        })
        .unwrap_or_default();
    zeile(&grid, row, &gettext("Location:"), &wert(&ort));
    row += 1;
    if let Ok(fs) = datei.query_filesystem_info("filesystem::free", gio::Cancellable::NONE)
        && fs.has_attribute("filesystem::free")
    {
        zeile(
            &grid,
            row,
            &gettext("Free space:"),
            &wert(&glib::format_size(fs.attribute_uint64("filesystem::free"))),
        );
        row += 1;
    }
    zeile(
        &grid,
        row,
        &gettext("Accessed:"),
        &wert(&zeit(
            info.has_attribute("time::access")
                .then(|| info.attribute_uint64("time::access"))
                .and_then(|t| glib::DateTime::from_unix_utc(i64::try_from(t).ok()?).ok()),
        )),
    );
    row += 1;
    zeile(
        &grid,
        row,
        &gettext("Modified:"),
        &wert(&zeit(info.modification_date_time())),
    );
    grid
}

fn zugriffsrechte(datei: &gio::File, info: &gio::FileInfo, window: &gtk4::Window) -> gtk4::Grid {
    let grid = raster();
    let ordner = info.file_type() == gio::FileType::Directory;
    let name = info.display_name().to_string();
    let mut row = 0;
    if !info.has_attribute("unix::mode") {
        grid.attach(
            &wert(&gettext_f(
                "The permissions of “{name}” could not be determined.",
                &[("name", &name)],
            )),
            0,
            0,
            2,
            1,
        );
        return grid;
    }
    let mode = Rc::new(std::cell::Cell::new(info.attribute_uint32("unix::mode")));
    let besitzer = info
        .attribute_string("owner::user")
        .map(|s| s.to_string())
        .unwrap_or_default();
    let gruppe = info
        .attribute_string("owner::group")
        .map(|s| s.to_string())
        .unwrap_or_default();
    let eigen = besitzer == glib::user_name().to_string_lossy();
    zeile(&grid, row, &gettext("Owner:"), &wert(&besitzer));
    row += 1;
    let mut listen: Vec<gtk4::DropDown> = Vec::new();
    for (klasse, text) in [
        (Klasse::Besitzer, gettext("Access:")),
        (Klasse::Gruppe, gettext("Group:")),
        (Klasse::Andere, gettext("Others:")),
    ] {
        if klasse == Klasse::Gruppe {
            zeile(&grid, row, &gettext("Group:"), &wert(&gruppe));
            row += 1;
        }
        let texte = stufen(ordner, klasse);
        let refs: Vec<&str> = texte.iter().map(String::as_str).collect();
        let liste = gtk4::DropDown::from_strings(&refs);
        match stufe(mode.get(), ordner, klasse) {
            Some(i) => liste.set_selected(u32::try_from(i).unwrap_or(0)),
            None => liste.set_selected(gtk4::INVALID_LIST_POSITION),
        }
        liste.set_sensitive(eigen);
        let datei = datei.clone();
        let mode_zelle = mode.clone();
        let w = window.downgrade();
        liste.connect_selected_notify(move |liste| {
            let Ok(i) = usize::try_from(liste.selected()) else {
                return;
            };
            if i >= texte.len() {
                return;
            }
            let neu = mit_stufe(mode_zelle.get(), ordner, klasse, i);
            schreiben(&datei, &mode_zelle, neu, w.upgrade().as_ref());
        });
        let label = if klasse == Klasse::Gruppe {
            gettext("Access:")
        } else {
            text
        };
        if klasse == Klasse::Andere {
            zeile(&grid, row, &gettext("Others:"), &gtk4::Label::new(None));
            row += 1;
            zeile(&grid, row, &gettext("Access:"), &liste);
        } else {
            zeile(&grid, row, &label, &liste);
        }
        row += 1;
        listen.push(liste);
    }
    if !ordner {
        let ausfuehren =
            gtk4::CheckButton::with_mnemonic(&gettext("Allow _executing file as program"));
        ausfuehren.set_active(mode.get() & 0o111 != 0);
        ausfuehren.set_sensitive(eigen);
        let datei = datei.clone();
        let mode_zelle = mode.clone();
        let w = window.downgrade();
        ausfuehren.connect_toggled(move |check| {
            let neu = mit_ausfuehren(mode_zelle.get(), check.is_active());
            schreiben(&datei, &mode_zelle, neu, w.upgrade().as_ref());
        });
        zeile(&grid, row, &gettext("Execute:"), &ausfuehren);
        row += 1;
    }
    if !eigen {
        let hinweis = wert(&gettext(
            "You are not the owner, so you cannot change these permissions.",
        ));
        grid.attach(&hinweis, 0, row, 2, 1);
    }
    grid
}

fn schreiben(
    datei: &gio::File,
    mode: &std::cell::Cell<u32>,
    neu: u32,
    parent: Option<&gtk4::Window>,
) {
    if neu == mode.get() {
        return;
    }
    match datei.set_attribute_uint32(
        "unix::mode",
        neu,
        gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
        gio::Cancellable::NONE,
    ) {
        Ok(()) => {
            protokoll(&format!(
                "Zugriffsrechte {}: {:o} → {:o}",
                datei.uri(),
                mode.get(),
                neu
            ));
            mode.set(neu);
        }
        Err(err) => fehler(
            parent,
            &gettext("The permissions could not be changed."),
            err.message(),
        ),
    }
}

fn oeffnen_mit(content_type: &str) -> gtk4::Box {
    let outer = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    outer.set_margin_top(12);
    outer.set_margin_bottom(12);
    outer.set_margin_start(12);
    outer.set_margin_end(12);
    let text = wert(&gettext_f(
        "Select an application to open “{type}” and other files of this type:",
        &[("type", &gio::content_type_get_description(content_type))],
    ));
    outer.append(&text);
    let list = gtk4::ListBox::new();
    list.set_selection_mode(gtk4::SelectionMode::None);
    list.add_css_class("frame");
    let standard = gio::AppInfo::default_for_type(content_type, false).and_then(|a| a.id());
    // radio buttons need a group, also with a single program: an unshown
    // leader holds it
    let gruppe = gtk4::CheckButton::new();
    let apps: Vec<gio::AppInfo> = oeffnen::apps_for(content_type)
        .into_iter()
        .filter(|a| a.should_show())
        .collect();
    if apps.is_empty() {
        outer.append(&wert(&gettext(
            "No applications are installed for this type.",
        )));
        return outer;
    }
    for app in apps {
        let radio = gtk4::CheckButton::new();
        radio.set_group(Some(&gruppe));
        radio.set_active(app.id().is_some() && app.id() == standard);
        radio.set_margin_top(3);
        radio.set_margin_bottom(3);
        radio.set_margin_start(6);
        let inhalt = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        if let Some(icon) = app.icon() {
            inhalt.append(&symbole::image(&icon, 16));
        }
        inhalt.append(&gtk4::Label::new(Some(&app.display_name())));
        radio.set_child(Some(&inhalt));
        let content_type = content_type.to_owned();
        radio.connect_toggled(move |radio| {
            if !radio.is_active() {
                return;
            }
            match app.set_as_default_for_type(&content_type) {
                Ok(()) => protokoll(&format!(
                    "Standardprogramm für {content_type}: {}",
                    app.id().unwrap_or_default()
                )),
                Err(err) => protokoll(&format!("Standard nicht gesetzt: {err}")),
            }
        });
        list.append(&radio);
    }
    let scroll = gtk4::ScrolledWindow::builder()
        .min_content_height(160)
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .child(&list)
        .build();
    outer.append(&scroll);
    outer
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_permissions_as_nautilus_lists_them() {
        // rw-r--r--
        assert_eq!(stufe(0o644, false, Klasse::Besitzer), Some(1));
        assert_eq!(stufe(0o644, false, Klasse::Gruppe), Some(1));
        assert_eq!(stufe(0o640, false, Klasse::Andere), Some(0));
        // the x bit does not change the choice of a file
        assert_eq!(stufe(0o755, false, Klasse::Besitzer), Some(1));
        // write without read fits no choice
        assert_eq!(stufe(0o624, false, Klasse::Gruppe), None);
        // owner: no "Keine"
        assert_eq!(stufe(0o044, false, Klasse::Besitzer), None);
        assert_eq!(stufen(false, Klasse::Besitzer).len(), 2);
        assert_eq!(stufen(false, Klasse::Andere).len(), 3);
    }

    #[test]
    fn folder_permissions_as_nautilus_lists_them() {
        assert_eq!(stufe(0o755, true, Klasse::Besitzer), Some(2));
        assert_eq!(stufe(0o755, true, Klasse::Gruppe), Some(2));
        assert_eq!(stufe(0o741, true, Klasse::Gruppe), Some(1));
        assert_eq!(stufe(0o700, true, Klasse::Andere), Some(0));
        assert_eq!(stufen(true, Klasse::Gruppe).len(), 4);
        assert_eq!(mit_stufe(0o755, true, Klasse::Andere, 0), 0o750);
        assert_eq!(mit_stufe(0o700, true, Klasse::Gruppe, 3), 0o770);
        assert_eq!(mit_stufe(0o755, true, Klasse::Besitzer, 0), 0o455);
    }

    #[test]
    fn setting_a_choice_keeps_the_other_bits() {
        assert_eq!(mit_stufe(0o644, false, Klasse::Andere, 0), 0o640);
        assert_eq!(mit_stufe(0o640, false, Klasse::Gruppe, 2), 0o660);
        // an executable stays executable for its class
        assert_eq!(mit_stufe(0o755, false, Klasse::Gruppe, 2), 0o775);
        // the x bit of a file belongs to its own check box and stays
        assert_eq!(mit_stufe(0o4755, false, Klasse::Andere, 0), 0o4751);
        assert_eq!(mit_stufe(0o644, false, Klasse::Andere, 9), 0o644);
    }

    #[test]
    fn executing_goes_with_reading() {
        assert_eq!(mit_ausfuehren(0o644, true), 0o755);
        assert_eq!(mit_ausfuehren(0o640, true), 0o750);
        assert_eq!(mit_ausfuehren(0o755, false), 0o644);
    }

    #[test]
    fn a_folder_is_counted_in_a_thread() {
        let basis = std::env::current_dir()
            .unwrap()
            .join("target/tmp/eigenschaften-zaehlen");
        let _ = std::fs::remove_dir_all(&basis);
        std::fs::create_dir_all(basis.join("a/b")).unwrap();
        std::fs::write(basis.join("a/eins"), vec![0u8; 1000]).unwrap();
        std::fs::write(basis.join("a/b/zwei"), vec![0u8; 24]).unwrap();
        std::os::unix::fs::symlink("/usr", basis.join("a/link")).unwrap();
        let stand: Arc<Mutex<Zaehlung>> = Arc::default();
        zaehlen(
            gio::File::for_path(basis.join("a")),
            stand.clone(),
            Arc::new(AtomicBool::new(false)),
        );
        let s = stand.lock().unwrap();
        assert!(s.fertig && !s.unlesbar);
        // eins, b, zwei, link (not followed)
        assert_eq!(s.items, 4);
        assert!(s.bytes >= 1024);
        drop(s);
        let _ = std::fs::remove_dir_all(&basis);
    }
}
