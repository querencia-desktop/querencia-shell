//! Desktop 0.3.0: the window "Dateioperationen", as Nautilus 2.30 showed it
//! for copies, moves and the trash.
//!
//! - One row per running job: what it does ("Kopieren von 3 Dateien nach
//!   „Schreibtisch“"), a progress bar, how much is done and how long it may
//!   still take ("12,4 MB von 100 MB – noch 1 Minute (8,2 MB/s)"), the file
//!   at hand, and "Abbrechen".
//! - The window appears only when a job still runs after `SHOW_AFTER`
//!   (short jobs show nothing, as on GNOME 2) and closes when no job runs.
//! - QD-076: closing destroys the window; the next job builds it anew.
//!   Closed by hand while jobs run, it stays away until a new job starts.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::time::Duration;

use gtk4::prelude::*;
use gtk4::{Align, Orientation, glib};
use querencia_dateiarbeit::auftraege::{JobId, Op};
use querencia_oberflaeche::symbole;

use crate::i18n::{gettext, gettext_f, ngettext_f};

/// How long a job runs before the window shows it.
pub const SHOW_AFTER: Duration = Duration::from_secs(1);

/// What a row knows about its job.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stand {
    pub total_items: Option<u64>,
    pub total_bytes: Option<u64>,
    pub done_items: u64,
    pub done_bytes: u64,
    pub current: String,
    pub bytes_per_s: f64,
}

/// The first line of a row: what the job does. `first`: the name of the
/// only object, `target`: the target folder's name.
pub fn title(op: &Op, count: u64, first: &str, target: &str) -> String {
    let n = u32::try_from(count).unwrap_or(u32::MAX);
    let one = count == 1;
    match op {
        Op::Copy if one => gettext_f(
            "Copying “{name}” to “{target}”",
            &[("name", first), ("target", target)],
        ),
        Op::Copy => ngettext_f(
            "Copying {n} file to “{target}”",
            "Copying {n} files to “{target}”",
            n,
            &[("target", target)],
        ),
        Op::Move if one => gettext_f(
            "Moving “{name}” to “{target}”",
            &[("name", first), ("target", target)],
        ),
        Op::Move => ngettext_f(
            "Moving {n} file to “{target}”",
            "Moving {n} files to “{target}”",
            n,
            &[("target", target)],
        ),
        Op::Trash if one => gettext_f("Moving “{name}” to the trash", &[("name", first)]),
        Op::Trash => ngettext_f(
            "Moving {n} file to the trash",
            "Moving {n} files to the trash",
            n,
            &[],
        ),
        Op::Delete if one => gettext_f("Deleting “{name}”", &[("name", first)]),
        Op::Delete => ngettext_f("Deleting {n} file", "Deleting {n} files", n, &[]),
        Op::EmptyTrash => gettext("Emptying the trash"),
        _ => gettext_f("Working on “{name}”", &[("name", first)]),
    }
}

/// "noch 1 Minute", "noch 12 Sekunden" … for `seconds`.
pub fn time_left(seconds: u64) -> String {
    let count = |n: u64| u32::try_from(n).unwrap_or(u32::MAX);
    if seconds < 60 {
        ngettext_f(
            "{n} second left",
            "{n} seconds left",
            count(seconds.max(1)),
            &[],
        )
    } else if seconds < 3600 {
        let minutes = seconds.div_ceil(60);
        ngettext_f("{n} minute left", "{n} minutes left", count(minutes), &[])
    } else {
        let hours = seconds.div_ceil(3600);
        ngettext_f("{n} hour left", "{n} hours left", count(hours), &[])
    }
}

/// The second line: how much is done (bytes when counted, else objects) and,
/// with a known rate, the time left. `None` before the job counted.
pub fn detail(stand: &Stand) -> Option<String> {
    match (stand.total_bytes, stand.total_items) {
        (Some(total), _) if total > 0 => {
            let amount = gettext_f(
                "{done} of {total}",
                &[
                    ("done", &glib::format_size(stand.done_bytes)),
                    ("total", &glib::format_size(total)),
                ],
            );
            let rest = total.saturating_sub(stand.done_bytes);
            if stand.bytes_per_s >= 1.0 && rest > 0 {
                let seconds = (rest as f64 / stand.bytes_per_s).ceil() as u64;
                Some(gettext_f(
                    "{amount} — {left} ({rate}/s)",
                    &[
                        ("amount", &amount),
                        ("left", &time_left(seconds)),
                        ("rate", &glib::format_size(stand.bytes_per_s as u64)),
                    ],
                ))
            } else {
                Some(amount)
            }
        }
        (_, Some(total)) => Some(ngettext_f(
            "{done} of {n} file",
            "{done} of {n} files",
            u32::try_from(total).unwrap_or(u32::MAX),
            &[("done", &stand.done_items.to_string())],
        )),
        _ => None,
    }
}

/// How full the bar is: by bytes when counted, else by objects.
pub fn fraction(stand: &Stand) -> Option<f64> {
    let part = |done: u64, total: u64| (done as f64 / total as f64).clamp(0.0, 1.0);
    match (stand.total_bytes, stand.total_items) {
        (Some(total), _) if total > 0 => Some(part(stand.done_bytes, total)),
        (_, Some(total)) if total > 0 => Some(part(stand.done_items, total)),
        _ => None,
    }
}

/// One job in the window.
struct Zeile {
    title: gtk4::Label,
    bar: gtk4::ProgressBar,
    detail: gtk4::Label,
    current: gtk4::Label,
    cancel: gtk4::Button,
    frame: gtk4::Box,
}

/// A job as the window shows it.
struct Eintrag {
    title: String,
    stand: Stand,
    zeile: Option<Zeile>,
}

/// The window and its jobs.
pub struct Fortschritt {
    eintraege: RefCell<BTreeMap<JobId, Eintrag>>,
    window: RefCell<Option<(gtk4::Window, gtk4::Box)>>,
    /// Closed by hand: no window until a new job starts.
    weggeklickt: std::cell::Cell<bool>,
    /// "Abbrechen" of a row.
    on_cancel: Box<dyn Fn(JobId)>,
}

fn log(text: &str) {
    super::log(text);
}

impl Fortschritt {
    pub fn new(on_cancel: impl Fn(JobId) + 'static) -> Rc<Self> {
        Rc::new(Self {
            eintraege: RefCell::default(),
            window: RefCell::default(),
            weggeklickt: std::cell::Cell::new(false),
            on_cancel: Box::new(on_cancel),
        })
    }

    /// A job started: its row shows once it ran for `SHOW_AFTER`.
    pub fn begonnen(self: &Rc<Self>, id: JobId, title: String) {
        self.weggeklickt.set(false);
        self.eintraege.borrow_mut().insert(
            id,
            Eintrag {
                title,
                stand: Stand::default(),
                zeile: None,
            },
        );
        let weak = Rc::downgrade(self);
        glib::timeout_add_local_once(SHOW_AFTER, move || {
            if let Some(fortschritt) = weak.upgrade() {
                fortschritt.zeigen(id);
            }
        });
    }

    /// The job counted, made progress or knows its title better.
    pub fn stand(&self, id: JobId, title: Option<String>, change: impl FnOnce(&mut Stand)) {
        let mut eintraege = self.eintraege.borrow_mut();
        let Some(eintrag) = eintraege.get_mut(&id) else {
            return;
        };
        if let Some(title) = title {
            eintrag.title = title;
        }
        change(&mut eintrag.stand);
        if let Some(zeile) = &eintrag.zeile {
            fill(zeile, &eintrag.title, &eintrag.stand);
        }
    }

    /// The job ended: its row goes; without rows the window closes.
    pub fn beendet(&self, id: JobId) {
        let removed = self.eintraege.borrow_mut().remove(&id);
        if let Some(Eintrag {
            zeile: Some(zeile), ..
        }) = removed
            && let Some((_, list)) = self.window.borrow().as_ref()
        {
            list.remove(&zeile.frame);
        }
        if self.eintraege.borrow().is_empty() {
            self.schliessen();
        }
    }

    fn schliessen(&self) {
        if let Some((window, _)) = self.window.borrow_mut().take() {
            log("Fortschrittsfenster geschlossen");
            window.destroy();
        }
    }

    /// After `SHOW_AFTER`: the job still runs, so the window shows it.
    fn zeigen(self: &Rc<Self>, id: JobId) {
        if self.weggeklickt.get() {
            return;
        }
        if !self.eintraege.borrow().contains_key(&id) {
            return;
        }
        let list = self.fenster();
        let mut eintraege = self.eintraege.borrow_mut();
        let Some(eintrag) = eintraege.get_mut(&id) else {
            return;
        };
        if eintrag.zeile.is_some() {
            return;
        }
        let zeile = self.zeile(id);
        fill(&zeile, &eintrag.title, &eintrag.stand);
        list.append(&zeile.frame);
        log(&format!(
            "Fortschritt gezeigt: Auftrag {id}: {}",
            eintrag.title
        ));
        let cancel = zeile.cancel.clone();
        eintrag.zeile = Some(zeile);
        drop(eintraege);
        // QUERENCIA_DESKTOP_DEBUG=1: where "Abbrechen" is, for the isolated
        // test (window coordinates, once drawn)
        if super::debugging()
            && let Some((window, _)) = self.window.borrow().as_ref()
        {
            let window = window.clone();
            glib::timeout_add_local_once(Duration::from_millis(300), move || {
                if let Some(bounds) = cancel.compute_bounds(&window) {
                    log(&format!(
                        "Fortschritt: Abbrechen für Auftrag {id} bei {},{} ({}x{})",
                        (bounds.x() + bounds.width() / 2.0).round(),
                        (bounds.y() + bounds.height() / 2.0).round(),
                        bounds.width().round(),
                        bounds.height().round()
                    ));
                }
            });
        }
    }

    /// The window, built if there is none (QD-076: never shown again after
    /// a close).
    fn fenster(self: &Rc<Self>) -> gtk4::Box {
        if let Some((_, list)) = self.window.borrow().as_ref() {
            return list.clone();
        }
        let window = gtk4::Window::builder()
            .title(gettext("File Operations"))
            .default_width(440)
            .resizable(true)
            .build();
        window.add_css_class("querencia-desktop-progress");
        let list = gtk4::Box::new(Orientation::Vertical, 12);
        list.set_margin_top(12);
        list.set_margin_bottom(12);
        list.set_margin_start(12);
        list.set_margin_end(12);
        window.set_child(Some(&list));
        let weak = Rc::downgrade(self);
        window.connect_close_request(move |window| {
            if let Some(fortschritt) = weak.upgrade() {
                // the jobs go on; the window stays away until the next one
                fortschritt.weggeklickt.set(true);
                fortschritt.window.borrow_mut().take();
                for eintrag in fortschritt.eintraege.borrow_mut().values_mut() {
                    eintrag.zeile = None;
                }
                log("Fortschrittsfenster weggeklickt, Aufträge laufen weiter");
            }
            window.destroy();
            glib::Propagation::Stop
        });
        window.present();
        *self.window.borrow_mut() = Some((window, list.clone()));
        list
    }

    fn zeile(self: &Rc<Self>, id: JobId) -> Zeile {
        let frame = gtk4::Box::new(Orientation::Vertical, 4);
        let title = gtk4::Label::new(None);
        title.set_xalign(0.0);
        title.set_wrap(true);
        title.set_max_width_chars(48);
        title.add_css_class("querencia-progress-title");
        let line = gtk4::Box::new(Orientation::Horizontal, 6);
        let bar = gtk4::ProgressBar::new();
        bar.set_hexpand(true);
        bar.set_valign(Align::Center);
        let cancel = gtk4::Button::new();
        cancel.set_child(Some(&symbole::image_named("process-stop", 16)));
        cancel.set_tooltip_text(Some(&gettext("Cancel")));
        cancel.set_valign(Align::Center);
        let weak = Rc::downgrade(self);
        cancel.connect_clicked(move |button| {
            button.set_sensitive(false);
            if let Some(fortschritt) = weak.upgrade() {
                log(&format!("Abbrechen gedrückt: Auftrag {id}"));
                (fortschritt.on_cancel)(id);
            }
        });
        line.append(&bar);
        line.append(&cancel);
        let detail = gtk4::Label::new(None);
        detail.set_xalign(0.0);
        detail.add_css_class("dim-label");
        let current = gtk4::Label::new(None);
        current.set_xalign(0.0);
        current.set_ellipsize(gtk4::pango::EllipsizeMode::Middle);
        current.add_css_class("dim-label");
        frame.append(&title);
        frame.append(&line);
        frame.append(&detail);
        frame.append(&current);
        Zeile {
            title,
            bar,
            detail,
            current,
            cancel,
            frame,
        }
    }
}

fn fill(zeile: &Zeile, title: &str, stand: &Stand) {
    zeile
        .title
        .set_markup(&format!("<b>{}</b>", glib::markup_escape_text(title)));
    match fraction(stand) {
        Some(part) => zeile.bar.set_fraction(part),
        None => zeile.bar.pulse(),
    }
    zeile
        .detail
        .set_text(&detail(stand).unwrap_or_else(|| gettext("Preparing…")));
    zeile.current.set_text(&stand.current);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_title_says_what_where() {
        // without a catalog: the English msgids
        assert_eq!(
            title(&Op::Copy, 1, "notiz.txt", "Schreibtisch"),
            "Copying “notiz.txt” to “Schreibtisch”"
        );
        assert_eq!(
            title(&Op::Copy, 3, "notiz.txt", "Ablage"),
            "Copying 3 files to “Ablage”"
        );
        assert_eq!(
            title(&Op::Move, 2, "a", "Ordner A"),
            "Moving 2 files to “Ordner A”"
        );
        assert_eq!(
            title(&Op::Trash, 1, "alt.txt", ""),
            "Moving “alt.txt” to the trash"
        );
        assert_eq!(title(&Op::Delete, 4, "x", ""), "Deleting 4 files");
        assert_eq!(title(&Op::EmptyTrash, 9, "", ""), "Emptying the trash");
    }

    #[test]
    fn the_time_left_is_rounded_up() {
        assert_eq!(time_left(0), "1 second left");
        assert_eq!(time_left(42), "42 seconds left");
        assert_eq!(time_left(61), "2 minutes left");
        assert_eq!(time_left(60), "1 minute left");
        assert_eq!(time_left(3601), "2 hours left");
    }

    #[test]
    fn the_detail_counts_bytes_else_files() {
        let mut stand = Stand::default();
        assert_eq!(detail(&stand), None);
        assert_eq!(fraction(&stand), None);
        stand.total_items = Some(4);
        stand.done_items = 1;
        assert_eq!(detail(&stand).as_deref(), Some("1 of 4 files"));
        assert_eq!(fraction(&stand), Some(0.25));
        stand.total_bytes = Some(2_000_000);
        stand.done_bytes = 500_000;
        assert_eq!(
            detail(&stand).as_deref(),
            Some("500.0\u{a0}kB of 2.0\u{a0}MB")
        );
        assert_eq!(fraction(&stand), Some(0.25));
        stand.bytes_per_s = 500_000.0;
        assert_eq!(
            detail(&stand).as_deref(),
            Some("500.0\u{a0}kB of 2.0\u{a0}MB — 3 seconds left (500.0\u{a0}kB/s)")
        );
        // a move within one file system counts no bytes
        let moved = Stand {
            total_items: Some(2),
            total_bytes: None,
            done_items: 2,
            ..Stand::default()
        };
        assert_eq!(fraction(&moved), Some(1.0));
    }
}
