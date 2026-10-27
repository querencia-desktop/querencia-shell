//! The status bar (B §1.6): "12 Objekte, freier Speicherplatz: 3,4 GB"
//! without selection, "»a.txt« ausgewählt (12,3 kB)" and the mixed forms
//! with one; while loading "Lade …"; a message of the window (e.g. a
//! location not found) until the next change. While search results stand
//! in for the folder, their text ([`Statusleiste::ersatz_setzen`]).
//! Menu hints: Beta.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use gtk4::prelude::*;
use gtk4::{gio, glib};

use crate::ansichtskern::protokoll;
use crate::fenster::Fenster;
use crate::i18n::{gettext, gettext_f};
use crate::modell::Zustand;
use crate::regeln;

pub struct Statusleiste {
    fenster: Weak<Fenster>,
    leiste: gtk4::Box,
    text: gtk4::Label,
    frei: Cell<Option<u64>>,
    meldung: RefCell<Option<String>>,
    fehler: RefCell<Option<String>>,
    generation: Cell<u64>,
    /// The text while the search results are shown (`suche/`).
    ersatz: RefCell<Option<Box<dyn Fn() -> String>>>,
}

impl Statusleiste {
    pub fn new(fenster: &Rc<Fenster>) -> Rc<Self> {
        let leiste = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        leiste.add_css_class("querencia-statusleiste");
        let text = gtk4::Label::new(None);
        text.set_xalign(0.0);
        text.set_hexpand(true);
        text.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        text.set_margin_start(6);
        text.set_margin_end(6);
        text.set_margin_top(2);
        text.set_margin_bottom(2);
        leiste.append(&text);
        Rc::new(Self {
            fenster: Rc::downgrade(fenster),
            leiste,
            text,
            frei: Cell::new(None),
            meldung: RefCell::default(),
            fehler: RefCell::default(),
            generation: Cell::new(0),
            ersatz: RefCell::default(),
        })
    }

    pub fn widget(&self) -> &gtk4::Box {
        &self.leiste
    }

    /// A message until the next change of selection or folder.
    pub fn melden(&self, text: &str) {
        self.meldung.replace(Some(text.to_owned()));
        self.zeigen(text);
    }

    /// Sets the text; with `QUERENCIA_DATEIEN_DEBUG=1` each new text goes
    /// to the log (the checks read it there).
    fn zeigen(&self, text: &str) {
        if self.text.text() != text {
            protokoll(&format!("Statusleiste: {text}"));
            self.text.set_text(text);
        }
    }

    /// While `ersatz` is set, it gives the text instead of the folder (the
    /// search results, QD-173 P-3); `None` shows the folder again.
    pub fn ersatz_setzen(&self, ersatz: Option<Box<dyn Fn() -> String>>) {
        self.ersatz.replace(ersatz);
        self.aktualisieren();
    }

    pub fn zustand(self: &Rc<Self>, z: &Zustand) {
        match z {
            Zustand::Laedt(ort) => {
                self.fehler.replace(None);
                self.meldung.replace(None);
                self.frei.set(None);
                self.generation.set(self.generation.get() + 1);
                self.frei_abfragen(ort);
            }
            Zustand::Fehler(text) => {
                self.fehler.replace(Some(gettext_f(
                    "The folder could not be read: {error}",
                    &[("error", text)],
                )));
            }
            _ => {}
        }
        self.aktualisieren();
    }

    fn frei_abfragen(self: &Rc<Self>, ort: &gio::File) {
        let weak = Rc::downgrade(self);
        let generation = self.generation.get();
        let ort = ort.clone();
        glib::spawn_future_local(async move {
            let info = ort
                .query_filesystem_info_future("filesystem::free", glib::Priority::DEFAULT_IDLE)
                .await;
            let Some(s) = weak.upgrade().filter(|s| s.generation.get() == generation) else {
                return;
            };
            if let Ok(info) = info
                && info.has_attribute("filesystem::free")
            {
                s.frei.set(Some(info.attribute_uint64("filesystem::free")));
                s.aktualisieren();
            }
        });
    }

    /// Shows the text for the current state (selection, folder, loading).
    pub fn aktualisieren(&self) {
        self.meldung.replace(None);
        if let Some(ersatz) = self.ersatz.borrow().as_ref() {
            self.zeigen(&ersatz());
            return;
        }
        let Some(f) = self.fenster.upgrade() else {
            return;
        };
        if let Some(fehler) = self.fehler.borrow().as_ref() {
            self.zeigen(fehler);
            return;
        }
        let m = f.modell();
        let gewaehlt = m.gewaehlte();
        let text = if !gewaehlt.is_empty() {
            let g: Vec<regeln::Gewaehlt> = gewaehlt.iter().map(|e| e.gewaehlt()).collect();
            regeln::status_mit_auswahl(&g)
        } else if m.laedt() && m.anzahl() == 0 {
            gettext("Loading…")
        } else {
            regeln::status_ohne_auswahl(m.anzahl(), self.frei.get())
        };
        self.zeigen(&text);
    }
}
