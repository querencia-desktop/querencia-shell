//! The file operations of a window (Welle B3, `docs/dateien.md` §3.6), as
//! `win.` actions for menus, context menus and the keys the window binds
//! on its view area (`menue::KUERZEL_IM_BEREICH`):
//!
//! | Action | Does | Key |
//! |---|---|---|
//! | `win.ausschneiden`, `win.kopieren` | onto the clipboard (all formats, `zwischenablage`) | Strg+X, Strg+C |
//! | `win.einfuegen` | the clipboard's files into the shown folder | Strg+V |
//! | `win.in-ordner-einfuegen` | … into the one selected folder | – |
//! | `win.papierkorb` | into the trash, **without** a question (in the trash: deleting, with one) | Entf |
//! | `win.loeschen` | deletes for good after a question, default Abbrechen | Umschalt+Entf |
//! | `win.papierkorb-leeren` | empties the trash after a question, default Abbrechen | – |
//! | `win.wiederherstellen` | puts trash entries back where they were | – |
//! | `win.ordner-anlegen` | "Neuer Ordner", then renaming in the view | Umschalt+Strg+N |
//! | `win.dokument-anlegen` (s) | a copy of the template (URI), `""`: "Leere Datei" | – |
//! | `win.rueckgaengig`, `win.wiederholen` | undo and redo (0.3.0, `rueckgaengig.rs`) | Strg+Z, Umschalt+Strg+Z |
//!
//! Öffnen, Öffnen mit and Eigenschaften are actions of the window (B1);
//! `eigenschaften_dialog::zeigen` is the function B1 hangs on
//! `win.eigenschaften`. "Mit anderer Anwendung öffnen …" comes with the beta
//! (B §1.10).
//!
//! Every change of files is a job of the process's queue (`App::queue`,
//! its own thread) with progress (`fortschritt`), cancel and questions
//! (`konflikt`, E2: Überspringen is the default). [`Dienst`] shows them
//! for every job of the queue, also the ones the window starts itself
//! (renaming). Every job a window starts is remembered in the window's
//! undo stack when it can go back (0.3.0, `rueckgaengig.rs`).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use gtk4::prelude::*;
use gtk4::{gdk, gio, glib};
use querencia_dateiarbeit::auftraege::{self, Event, Job, JobId, Op, Outcome, Queue};
use querencia_dateiarbeit::papierkorb;
use querencia_dateiarbeit::{namen, uri};
use std::path::PathBuf;

use crate::app::{self, Register};
use crate::eintrag::Eintrag;
use crate::fenster::Fenster;
use crate::fortschritt::{self, Fortschritt};
use crate::i18n::{gettext, gettext_f, ngettext_f};
use crate::konflikt::Fragen;
use crate::rueckgaengig::{self, Art, Schritt, Stapel, Vorgang};
use crate::ziehen::{self, Ablage, Ziel};
use crate::zwischenablage::Zwischenablage;

/// A log line on stderr.
pub fn protokoll(text: &str) {
    crate::log(text);
}

/// `QUERENCIA_DATEIEN_DEBUG=1`: more log lines (positions for the isolated
/// GUI test, every progress step).
pub fn debugging() -> bool {
    thread_local! {
        static AN: bool = std::env::var_os("QUERENCIA_DATEIEN_DEBUG").is_some_and(|v| v == "1");
    }
    AN.with(|a| *a)
}

/// The hooks of this module (`main.rs`, `erweiterungen`).
pub fn registrieren(register: &mut Register) {
    register.anwendung(|app| {
        Dienst::starten(app.queue());
    });
    register.fenster(|fenster| {
        let ops = Operationen::fuer(fenster);
        ops.aktualisieren();
    });
    register.neue_ansicht(|fenster, ansicht| {
        let ops = Operationen::fuer(fenster);
        ziehen::an_ansicht(fenster, ansicht, &ops);
    });
    register.ablage(|fenster, widget, zielfinder| {
        let ops = Operationen::fuer(fenster);
        ziehen::ziel(
            widget,
            &ops,
            move |x, y| zielfinder(x, y).map(Ziel::Ordner),
            |_| {},
        );
    });
}

/// What follows when a job ends: (outcome, cancelled).
type Nachher = Box<dyn FnOnce(&Outcome, bool)>;

/// What a finished job means for the undo stack of its window.
#[derive(Clone, Debug)]
pub enum Merken {
    /// Nothing (jobs not started by a window).
    Nein,
    /// A new operation.
    Neu,
    /// The job undoes this operation: it can be redone afterwards.
    Rueckgaengig(Vorgang),
    /// One more job of an undo (several folders): nothing to remember.
    Teil,
    /// The job redoes an operation: what it did can be undone again.
    Wiederholen,
}

/// A job as this module knows it.
struct Laufend {
    op: Op,
    ziel_name: String,
    nachher: Option<Nachher>,
    quellen: Vec<String>,
    ziel: Option<String>,
    fenster: Option<glib::WeakRef<gtk4::Window>>,
    merken: Merken,
    beginn: String,
}

/// The window "Dateioperationen" and the open questions of the process,
/// for every job of the queue.
pub struct Dienst {
    queue: Queue,
    fortschritt: Rc<Fortschritt>,
    fragen: Rc<Fragen>,
    laufend: RefCell<HashMap<JobId, Laufend>>,
}

thread_local! {
    static DIENST: RefCell<Option<Rc<Dienst>>> = const { RefCell::new(None) };
}

/// A job as a window starts it.
pub struct Auftrag {
    pub op: Op,
    pub quellen: Vec<gio::File>,
    pub ziel: Option<gio::File>,
    pub fenster: Option<gtk4::Window>,
    nachher: Option<Nachher>,
    merken: Merken,
}

impl Auftrag {
    pub fn new(op: Op, quellen: Vec<gio::File>, ziel: Option<gio::File>) -> Self {
        Self {
            op,
            quellen,
            ziel,
            fenster: None,
            nachher: None,
            merken: Merken::Neu,
        }
    }

    pub fn merken(mut self, merken: Merken) -> Self {
        self.merken = merken;
        self
    }

    pub fn fenster(mut self, fenster: Option<gtk4::Window>) -> Self {
        self.fenster = fenster;
        self
    }

    pub fn nachher(mut self, f: impl FnOnce(&Outcome, bool) + 'static) -> Self {
        self.nachher = Some(Box::new(f));
        self
    }
}

/// An entry of `Outcome::arrived` as a file: the jobs report names in the
/// target folder (restoring: full paths), not always full paths as the
/// crate's text says (measured; handover §6).
pub fn angekommen(ordner: &gio::File, eintrag: &str) -> gio::File {
    if eintrag.starts_with('/') {
        gio::File::for_path(eintrag)
    } else {
        ordner.child(eintrag)
    }
}

fn anzeigename(file: &gio::File) -> String {
    file.query_info(
        "standard::display-name",
        gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
        gio::Cancellable::NONE,
    )
    .ok()
    .map(|i| i.display_name().to_string())
    .filter(|n| !n.is_empty())
    .unwrap_or_else(|| uri::name_of(&file.uri()))
}

impl Dienst {
    /// Follows `queue` (once, at startup).
    pub fn starten(queue: &Queue) -> Rc<Self> {
        let cancel_queue = queue.clone();
        let dienst = Rc::new(Dienst {
            fortschritt: Fortschritt::new(move |id| {
                protokoll(&format!("Auftrag {id} abbrechen"));
                cancel_queue.cancel(id);
            }),
            fragen: Rc::default(),
            laufend: RefCell::default(),
            queue: queue.clone(),
        });
        let weak = Rc::downgrade(&dienst);
        dienst.queue.subscribe(move |event| {
            if let Some(dienst) = weak.upgrade() {
                dienst.ereignis(event);
            }
        });
        DIENST.with(|d| *d.borrow_mut() = Some(dienst.clone()));
        dienst
    }

    /// The one of this process (after [`Self::starten`]).
    pub fn get() -> Option<Rc<Self>> {
        DIENST.with(|d| d.borrow().clone())
    }

    /// Starts `auftrag`; its progress shows after a second.
    pub fn auftrag(&self, auftrag: Auftrag) -> Option<JobId> {
        let Auftrag {
            op,
            quellen,
            ziel,
            fenster,
            nachher,
            merken,
        } = auftrag;
        let namen: Vec<String> = quellen.iter().map(anzeigename).collect();
        let ziel_name = ziel.as_ref().map(anzeigename).unwrap_or_default();
        protokoll(&format!(
            "{}: {}{}",
            op.verb(),
            namen.join(", "),
            ziel.as_ref()
                .map_or(String::new(), |z| format!(" → {}", z.uri()))
        ));
        let title = fortschritt::title(
            &op,
            namen.len() as u64,
            namen.first().map_or("", String::as_str),
            &ziel_name,
        );
        let job_quellen: Vec<String> = quellen.iter().map(|f| f.uri().to_string()).collect();
        let job = Job::new(
            op.clone(),
            job_quellen.clone(),
            ziel.as_ref().map(|z| z.uri().to_string()),
        );
        let beginn = rueckgaengig::jetzt_zeit();
        let id = match app::app_wenn() {
            Some(a) => a.auftrag_starten(job, fenster.as_ref()),
            None => self.queue.start(job),
        };
        self.laufend.borrow_mut().insert(
            id,
            Laufend {
                op: op.clone(),
                ziel_name,
                nachher,
                quellen: job_quellen,
                ziel: ziel.as_ref().map(|z| z.uri().to_string()),
                fenster: fenster.as_ref().map(|f| f.downgrade()),
                merken,
                beginn,
            },
        );
        self.fortschritt.begonnen(id, op, title);
        Some(id)
    }

    fn fenster_fuer(id: JobId) -> Option<gtk4::Window> {
        app::app_wenn().and_then(|a| a.fenster_fuer_auftrag(id))
    }

    fn ereignis(&self, event: &Event) {
        match event {
            Event::Started {
                id,
                op,
                total_items,
                total_bytes,
            } => {
                if debugging() {
                    protokoll(&format!(
                        "Auftrag {id} gezählt: {total_items:?} Objekte, {total_bytes:?} Bytes"
                    ));
                }
                // a job of the window (renaming): shown like the own ones
                if !self.laufend.borrow().contains_key(id) {
                    self.laufend.borrow_mut().insert(
                        *id,
                        Laufend {
                            op: op.clone(),
                            ziel_name: String::new(),
                            nachher: None,
                            quellen: Vec::new(),
                            ziel: None,
                            fenster: None,
                            merken: Merken::Nein,
                            beginn: String::new(),
                        },
                    );
                    let title = fortschritt::title(op, total_items.unwrap_or(1), "", "");
                    self.fortschritt.begonnen(*id, op.clone(), title);
                }
                self.fortschritt.stand(*id, None, |stand| {
                    stand.total_items = *total_items;
                    stand.total_bytes = *total_bytes;
                });
            }
            Event::Progress {
                id,
                done_items,
                done_bytes,
                current,
                bytes_per_s,
            } => {
                if debugging() {
                    protokoll(&format!(
                        "Fortschritt {id}: {done_items} Objekte, {done_bytes} Bytes, {current}"
                    ));
                }
                self.fortschritt.stand(*id, None, |stand| {
                    stand.done_items = *done_items;
                    stand.done_bytes = *done_bytes;
                    stand.current = current.clone();
                    stand.bytes_per_s = *bytes_per_s;
                });
            }
            Event::Ask {
                id,
                conflict,
                reply,
            } => {
                let (op, folder) = self
                    .laufend
                    .borrow()
                    .get(id)
                    .map(|l| (l.op.clone(), l.ziel_name.clone()))
                    .unwrap_or((Op::Copy, String::new()));
                // the window "Dateioperationen" waits for the answer: it would
                // take the keyboard from the question
                self.fortschritt.frage(*id, true);
                let fortschritt = Rc::downgrade(&self.fortschritt);
                let job = *id;
                self.fragen.fragen(
                    *id,
                    &op,
                    conflict,
                    reply.clone(),
                    &folder,
                    Self::fenster_fuer(*id).as_ref(),
                    move || {
                        if let Some(fortschritt) = fortschritt.upgrade() {
                            fortschritt.frage(job, false);
                        }
                    },
                );
            }
            Event::Failed { id, name, why } => {
                protokoll(&format!("Auftrag {id}: {name}: {why}"));
            }
            Event::Finished {
                id,
                outcome,
                cancelled,
            } => {
                self.fragen.schliessen(*id);
                self.fortschritt.beendet(*id);
                let fenster = Self::fenster_fuer(*id);
                if let Some(a) = app::app_wenn() {
                    a.auftrag_vergessen(*id);
                }
                let laufend = self.laufend.borrow_mut().remove(id);
                if let Some(laufend) = laufend {
                    fertig(laufend, outcome, *cancelled, fenster.as_ref());
                }
            }
        }
    }
}

fn fertig(laufend: Laufend, outcome: &Outcome, cancelled: bool, fenster: Option<&gtk4::Window>) {
    let Laufend {
        op,
        nachher,
        quellen,
        ziel,
        fenster: von,
        merken,
        beginn,
        ..
    } = laufend;
    if let Some(ops) = von
        .and_then(|w| w.upgrade())
        .and_then(|w| Operationen::von_window(&w))
    {
        ops.merken(
            merken,
            &op,
            &quellen,
            ziel.as_deref(),
            outcome,
            cancelled,
            &beginn,
        );
    }
    protokoll(&format!(
        "{} fertig: {} erledigt, {} übersprungen, {} nicht{}{}",
        op.verb(),
        outcome.done,
        outcome.skipped,
        outcome.failed.len(),
        if cancelled { ", abgebrochen" } else { "" },
        if outcome.arrived.is_empty() {
            String::new()
        } else {
            format!(" (angekommen: {})", outcome.arrived.join(", "))
        }
    ));
    for (name, why) in &outcome.failed {
        protokoll(&format!("nicht {}: {name}: {why}", op.verb()));
    }
    if let Some(nachher) = nachher {
        nachher(outcome, cancelled);
    }
    if !outcome.failed.is_empty() && !cancelled {
        fehler(
            fenster,
            &op.failure_title(),
            &auftraege::failure_text(&op, outcome),
        );
    }
}

/// An error dialog for `parent` (Nautilus 2.30 put them on the window).
pub fn fehler(parent: Option<&gtk4::Window>, titel: &str, text: &str) {
    protokoll(&format!("Fehler gezeigt: {titel} {text}"));
    let dialog = gtk4::AlertDialog::builder()
        .message(titel)
        .detail(text)
        .buttons([gettext("Close")])
        .cancel_button(0)
        .default_button(0)
        .modal(true)
        .build();
    dialog.show(parent);
}

/// Einstellungen → Verhalten „Vor dem Leeren des Papierkorbs oder Löschen
/// nachfragen“ (`dateien.ini`, Vorgabe an).
fn nachfragen() -> bool {
    app::app_wenn().is_none_or(|a| a.konfig().werte().nachfragen)
}

/// The actions of this module.
const AKTIONEN: [&str; 12] = [
    "ausschneiden",
    "kopieren",
    "einfuegen",
    "in-ordner-einfuegen",
    "papierkorb",
    "loeschen",
    "papierkorb-leeren",
    "wiederherstellen",
    "ordner-anlegen",
    "dokument-anlegen",
    "rueckgaengig",
    "wiederholen",
];

thread_local! {
    /// The operations of every open window.
    static ALLE: RefCell<Vec<Rc<Operationen>>> = const { RefCell::new(Vec::new()) };
}

/// The operations of one window.
pub struct Operationen {
    fenster: Weak<Fenster>,
    window: glib::WeakRef<gtk4::ApplicationWindow>,
    aktionen: RefCell<HashMap<&'static str, gio::SimpleAction>>,
    /// Undo and redo of this window, for all its tabs.
    stapel: RefCell<Stapel>,
    /// Undo or redo jobs of this window still running.
    umkehr_laeuft: Cell<u32>,
}

impl Operationen {
    /// The operations of `fenster`, made on first use (views and drop
    /// targets are made while the window is built, before its hook runs).
    pub fn fuer(fenster: &Rc<Fenster>) -> Rc<Self> {
        let window = fenster.window().clone();
        let da = ALLE.with(|alle| {
            alle.borrow_mut().retain(|o| o.window.upgrade().is_some());
            alle.borrow()
                .iter()
                .find(|o| o.window.upgrade().as_ref() == Some(&window))
                .cloned()
        });
        if let Some(ops) = da {
            return ops;
        }
        let ops = Rc::new(Self {
            fenster: Rc::downgrade(fenster),
            window: window.downgrade(),
            aktionen: RefCell::default(),
            stapel: RefCell::default(),
            umkehr_laeuft: Cell::new(0),
        });
        ops.aktionen_anlegen(&window);
        ALLE.with(|alle| alle.borrow_mut().push(ops.clone()));
        let weak = Rc::downgrade(&ops);
        fenster.bei_auswahl(move || {
            if let Some(ops) = weak.upgrade() {
                ops.aktualisieren();
            }
        });
        let weak = Rc::downgrade(&ops);
        fenster.bei_ortswechsel(move |_| {
            if let Some(ops) = weak.upgrade() {
                ops.aktualisieren();
            }
        });
        let weak = Rc::downgrade(&ops);
        Zwischenablage::get().beobachten(move || {
            if let Some(ops) = weak.upgrade() {
                ops.aktualisieren();
            }
        });
        // Wayland hands the clipboard to a window only with the keyboard:
        // "Einfügen" is checked again when the window becomes active
        let weak = Rc::downgrade(&ops);
        window.connect_is_active_notify(move |_| {
            if let Some(ops) = weak.upgrade() {
                ops.aktualisieren();
            }
        });
        if let Some(app) = fenster.app() {
            let weak = Rc::downgrade(&ops);
            app.papierkorb().connect_changed(move |_| {
                if let Some(ops) = weak.upgrade() {
                    ops.aktualisieren();
                }
            });
        }
        ops
    }

    /// The operations of the window `window`, if it has them.
    pub fn von_window(window: &gtk4::Window) -> Option<Rc<Self>> {
        ALLE.with(|alle| {
            alle.borrow()
                .iter()
                .find(|o| {
                    o.window
                        .upgrade()
                        .is_some_and(|w| w.upcast_ref::<gtk4::Window>() == window)
                })
                .cloned()
        })
    }

    fn aktionen_anlegen(self: &Rc<Self>, window: &gtk4::ApplicationWindow) {
        for name in AKTIONEN {
            let param = name == "dokument-anlegen";
            let action = gio::SimpleAction::new(name, param.then_some(glib::VariantTy::STRING));
            let weak = Rc::downgrade(self);
            action.connect_activate(move |_, value| {
                if let Some(ops) = weak.upgrade() {
                    let value = value.and_then(|v| v.str()).unwrap_or_default().to_owned();
                    ops.ausfuehren(name, &value);
                }
            });
            window.add_action(&action);
            self.aktionen.borrow_mut().insert(name, action);
        }
    }

    fn fenster(&self) -> Option<Rc<Fenster>> {
        self.fenster.upgrade()
    }

    fn window(&self) -> Option<gtk4::Window> {
        self.window.upgrade().map(|w| w.upcast())
    }

    fn ordner(&self) -> Option<gio::File> {
        self.fenster().and_then(|f| f.ort())
    }

    fn auswahl(&self) -> Vec<Eintrag> {
        self.fenster()
            .map(|f| f.modell().gewaehlte())
            .unwrap_or_default()
    }

    fn auswahl_dateien(&self) -> Vec<gio::File> {
        self.auswahl().iter().map(|e| e.datei().clone()).collect()
    }

    fn starten(&self, auftrag: Auftrag) {
        match Dienst::get() {
            Some(dienst) => {
                dienst.auftrag(auftrag.fenster(self.window()));
            }
            None => protokoll("kein Auftragsdienst (Anwendung nicht gestartet)"),
        }
    }

    /// Whether the shown folder is the trash (`trash:///`, or the home
    /// trash's `files` folder without GVfs).
    pub fn im_papierkorb(&self) -> bool {
        self.ordner().is_some_and(|o| ziehen::im_papierkorb(&o))
    }

    /// Enables the actions for the selection, the place, the clipboard and
    /// the trash (the menus show only enabled actions, `docs/dateien.md`
    /// §3.6).
    pub fn aktualisieren(&self) {
        // a closed window is freed now (Welle C3): its actions stay alone
        if self.fenster.upgrade().is_none() || self.window.upgrade().is_none() {
            return;
        }
        let auswahl = self.auswahl();
        let eine_ordner = matches!(auswahl.as_slice(), [e] if e.ist_ordner());
        let etwas = !auswahl.is_empty();
        let papierkorb = self.im_papierkorb();
        let ort = self.ordner().is_some();
        let leer = app::app_wenn()
            .and_then(|a| a.papierkorb().anzahl())
            .is_some_and(|n| n == 0);
        let einfuegen = ort && !papierkorb && Zwischenablage::get().kann_einfuegen();
        let aktionen = self.aktionen.borrow();
        let setzen = |name: &str, an: bool| {
            if let Some(action) = aktionen.get(name) {
                action.set_enabled(an);
            }
        };
        setzen("ausschneiden", etwas);
        setzen("kopieren", etwas);
        setzen("einfuegen", einfuegen);
        setzen("in-ordner-einfuegen", einfuegen && eine_ordner);
        setzen("papierkorb", etwas);
        setzen("loeschen", etwas);
        setzen("papierkorb-leeren", !leer);
        setzen("wiederherstellen", etwas && papierkorb);
        setzen("ordner-anlegen", ort && !papierkorb);
        setzen("dokument-anlegen", ort && !papierkorb);
        let frei = self.umkehr_laeuft.get() == 0;
        let stapel = self.stapel.borrow();
        setzen("rueckgaengig", frei && stapel.naechstes_zurueck().is_some());
        setzen("wiederholen", frei && stapel.naechstes_vor().is_some());
        drop(stapel);
        if debugging() {
            let formate = gdk::Display::default()
                .map(|d| d.clipboard().formats().to_str().to_string())
                .unwrap_or_default();
            protokoll(&format!(
                "Aktionen: {} gewählt, Einfügen {einfuegen}, Papierkorb {papierkorb} (Zwischenablage {formate})",
                auswahl.len()
            ));
        }
    }

    fn ausfuehren(self: &Rc<Self>, name: &str, wert: &str) {
        let auswahl = self.auswahl_dateien();
        if debugging() {
            protokoll(&format!(
                "Aktion win.{name} ({wert}) für {} Objekte",
                auswahl.len()
            ));
        }
        match name {
            "ausschneiden" => self.in_zwischenablage(&auswahl, true),
            "kopieren" => self.in_zwischenablage(&auswahl, false),
            "einfuegen" => {
                if let Some(ordner) = self.ordner() {
                    self.einfuegen(ordner);
                }
            }
            "in-ordner-einfuegen" => {
                if let [ordner] = self.auswahl().as_slice() {
                    self.einfuegen(ordner.ziel());
                }
            }
            "papierkorb" if self.im_papierkorb() => self.loeschen_fragen(auswahl),
            "papierkorb" => self.in_papierkorb(auswahl),
            "loeschen" => self.loeschen_fragen(auswahl),
            "papierkorb-leeren" => self.leeren_fragen(),
            "wiederherstellen" => self.wiederherstellen(auswahl),
            "ordner-anlegen" => self.ordner_anlegen(),
            "dokument-anlegen" => self.dokument_anlegen(wert),
            "rueckgaengig" => self.rueckgaengig(),
            "wiederholen" => self.wiederholen(),
            _ => {}
        }
    }

    fn in_zwischenablage(&self, dateien: &[gio::File], cut: bool) {
        if dateien.is_empty() {
            return;
        }
        match Zwischenablage::get().setzen(dateien, cut) {
            Ok(()) => protokoll(&format!(
                "{}: {}",
                if cut { "ausgeschnitten" } else { "kopiert" },
                dateien
                    .iter()
                    .map(|f| f.uri().to_string())
                    .collect::<Vec<_>>()
                    .join(" ")
            )),
            Err(err) => protokoll(&format!("Zwischenablage nicht gesetzt: {err}")),
        }
    }

    /// Einfügen into `ordner`: cut files are moved (then the clipboard is
    /// emptied), copied ones copied.
    pub fn einfuegen(self: &Rc<Self>, ordner: gio::File) {
        let weak = Rc::downgrade(self);
        glib::MainContext::default().spawn_local(async move {
            let inhalt = Zwischenablage::get().holen().await;
            let Some(ops) = weak.upgrade() else { return };
            let Some(inhalt) = inhalt else {
                protokoll("Einfügen: keine Dateien in der Zwischenablage");
                return;
            };
            let quellen: Vec<gio::File> =
                inhalt.uris.iter().map(|u| gio::File::for_uri(u)).collect();
            let op = if inhalt.ausgeschnitten {
                Op::Move
            } else {
                Op::Copy
            };
            protokoll(&format!(
                "Einfügen ({}): {} nach {}",
                if inhalt.ausgeschnitten {
                    "ausgeschnitten"
                } else {
                    "kopiert"
                },
                inhalt.uris.join(" "),
                ordner.uri()
            ));
            ops.uebertragen(op, quellen, ordner, inhalt.ausgeschnitten);
        });
    }

    /// Copy or move `quellen` into `ordner`; afterwards the arrived files are
    /// selected. Moves out of the home trash take their `.trashinfo` along.
    fn uebertragen(
        self: &Rc<Self>,
        op: Op,
        quellen: Vec<gio::File>,
        ordner: gio::File,
        ausgeschnitten: bool,
    ) {
        let weak = Rc::downgrade(self);
        let aus_papierkorb: Vec<gio::File> = quellen
            .iter()
            .filter(|q| ziehen::im_papierkorb(q) && q.is_native())
            .cloned()
            .collect();
        let ziel = ordner.clone();
        let auftrag = Auftrag::new(op, quellen, Some(ordner)).nachher(move |outcome, _| {
            if ausgeschnitten && outcome.done > 0 {
                // cut files are where they belong now: nothing to paste
                Zwischenablage::get().leeren();
            }
            for alt in &aus_papierkorb {
                if !alt.query_exists(gio::Cancellable::NONE) {
                    papierkorb::forget_info(alt);
                }
            }
            if let Some(f) = weak.upgrade().and_then(|o| o.fenster()) {
                let neu: Vec<gio::File> = outcome
                    .arrived
                    .iter()
                    .map(|a| angekommen(&ziel, a))
                    .collect();
                if !neu.is_empty() && f.ort().is_some_and(|o| o.equal(&ziel)) {
                    f.nach_erscheinen_waehlen(&neu);
                }
            }
        });
        self.starten(auftrag);
    }

    /// A drop (`ziehen`): `ablage` of `quellen` onto `ziel`.
    pub fn ablegen(self: &Rc<Self>, ablage: Ablage, quellen: &[gio::File], ziel: &Ziel) {
        let quellen = quellen.to_vec();
        match (ablage, ziel) {
            (Ablage::Papierkorb, _) => self.in_papierkorb(quellen),
            (Ablage::Kopieren, Ziel::Ordner(o)) => {
                self.uebertragen(Op::Copy, quellen, o.clone(), false)
            }
            (Ablage::Verschieben, Ziel::Ordner(o)) => {
                self.uebertragen(Op::Move, quellen, o.clone(), false)
            }
            (Ablage::Verknuepfen, Ziel::Ordner(o)) => {
                self.starten(Auftrag::new(Op::Link, quellen, Some(o.clone())));
            }
        }
    }

    /// Umbenennen from the properties: checks the name (GNOME 2's
    /// messages), then a job (atomic, never replaces).
    pub fn umbenennen(self: &Rc<Self>, datei: &gio::File, neu: &str) {
        let alt = datei
            .basename()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if neu == alt {
            return;
        }
        if let Err(problem) = namen::check_file_name(neu) {
            fehler(
                self.window().as_ref(),
                &gettext_f("“{name}” cannot be renamed.", &[("name", &alt)]),
                &problem.text(neu),
            );
            return;
        }
        let weak = Rc::downgrade(self);
        let eltern = datei.parent();
        self.starten(
            Auftrag::new(
                Op::Rename {
                    new: neu.to_owned(),
                },
                vec![datei.clone()],
                None,
            )
            .nachher(move |outcome, _| {
                if let (Some(neu), Some(eltern), Some(f)) = (
                    outcome.arrived.first(),
                    &eltern,
                    weak.upgrade().and_then(|o| o.fenster()),
                ) {
                    f.nach_erscheinen_waehlen(&[angekommen(eltern, neu)]);
                }
            }),
        );
    }

    /// In den Papierkorb, without a question (`confirm_trash` only for
    /// emptying); a place without trash asks (`Conflict::NoTrash`).
    pub fn in_papierkorb(self: &Rc<Self>, dateien: Vec<gio::File>) {
        if dateien.is_empty() {
            return;
        }
        self.starten(Auftrag::new(Op::Trash, dateien, None));
    }

    /// Umschalt+Entf: asks first (texts as the desktop), Abbrechen is the
    /// default and Escape cancels; only "Löschen" deletes.
    pub fn loeschen_fragen(self: &Rc<Self>, dateien: Vec<gio::File>) {
        if dateien.is_empty() {
            return;
        }
        if !nachfragen() {
            self.starten(Auftrag::new(Op::Delete, dateien, None));
            return;
        }
        let message = match dateien.as_slice() {
            [one] => gettext_f(
                "Are you sure you want to permanently delete “{name}”?",
                &[("name", &anzeigename(one))],
            ),
            more => ngettext_f(
                "Are you sure you want to permanently delete the {n} selected item?",
                "Are you sure you want to permanently delete the {n} selected items?",
                u32::try_from(more.len()).unwrap_or(u32::MAX),
                &[],
            ),
        };
        let dialog = gtk4::AlertDialog::builder()
            .message(message)
            .detail(gettext(
                "If you delete an item, it will be permanently lost.",
            ))
            .buttons([gettext("Cancel"), gettext("Delete")])
            .cancel_button(0)
            .default_button(0)
            .modal(true)
            .build();
        protokoll("Frage: endgültig löschen");
        let weak = Rc::downgrade(self);
        dialog.choose(
            self.window().as_ref(),
            gio::Cancellable::NONE,
            move |answer| match answer {
                Ok(1) => {
                    if let Some(ops) = weak.upgrade() {
                        ops.starten(Auftrag::new(Op::Delete, dateien, None));
                    }
                }
                _ => protokoll("endgültig löschen abgebrochen"),
            },
        );
    }

    /// "Papierkorb leeren": asks first (texts as the desktop), Abbrechen is
    /// the default.
    pub fn leeren_fragen(self: &Rc<Self>) {
        if !nachfragen() {
            self.starten(Auftrag::new(Op::EmptyTrash, Vec::new(), None));
            return;
        }
        let dialog = gtk4::AlertDialog::builder()
            .message(gettext("Empty all items from the trash?"))
            .detail(gettext(
                "If you empty the trash, all items in it will be permanently lost.",
            ))
            .buttons([gettext("Cancel"), gettext("Empty Trash")])
            .cancel_button(0)
            .default_button(0)
            .modal(true)
            .build();
        protokoll("Frage: Papierkorb leeren");
        let weak = Rc::downgrade(self);
        dialog.choose(
            self.window().as_ref(),
            gio::Cancellable::NONE,
            move |answer| match answer {
                Ok(1) => {
                    if let Some(ops) = weak.upgrade() {
                        ops.starten(Auftrag::new(Op::EmptyTrash, Vec::new(), None));
                    }
                }
                _ => protokoll("Papierkorb leeren abgebrochen"),
            },
        );
    }

    /// Wiederherstellen: trash entries back to where they were deleted from
    /// (a taken place asks; it is never replaced).
    pub fn wiederherstellen(self: &Rc<Self>, dateien: Vec<gio::File>) {
        let dateien: Vec<gio::File> = dateien.into_iter().filter(ziehen::im_papierkorb).collect();
        if dateien.is_empty() {
            return;
        }
        self.starten(Auftrag::new(Op::Restore, dateien, None));
    }

    /// "Ordner anlegen": "Neuer Ordner" (a free name), then renaming.
    pub fn ordner_anlegen(self: &Rc<Self>) {
        self.neu_anlegen(Op::NewFolder {
            name: gettext("New Folder"),
        });
    }

    /// "Dokument anlegen": `template` the URI of a template, empty for
    /// "Leere Datei".
    pub fn dokument_anlegen(self: &Rc<Self>, template: &str) {
        let op = if template.is_empty() {
            Op::NewFile {
                name: gettext("New Document"),
                template: None,
            }
        } else {
            let vorlage = gio::File::for_uri(template);
            Op::NewFile {
                name: vorlage
                    .basename()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| gettext("Template")),
                template: Some(template.to_owned()),
            }
        };
        self.neu_anlegen(op);
    }

    fn neu_anlegen(self: &Rc<Self>, op: Op) {
        let Some(ordner) = self.ordner() else { return };
        let weak = Rc::downgrade(self);
        let ziel = ordner.clone();
        self.starten(
            Auftrag::new(op, Vec::new(), Some(ordner)).nachher(move |outcome, _| {
                if let (Some(neu), Some(f)) = (
                    outcome.arrived.first(),
                    weak.upgrade().and_then(|o| o.fenster()),
                ) {
                    f.nach_erscheinen_umbenennen(&angekommen(&ziel, neu));
                }
            }),
        );
    }

    // ------------------------------------------------------ undo and redo

    /// After a job of this window: what it means for the undo stack.
    #[allow(clippy::too_many_arguments)]
    fn merken(
        &self,
        merken: Merken,
        op: &Op,
        quellen: &[String],
        ziel: Option<&str>,
        outcome: &Outcome,
        cancelled: bool,
        beginn: &str,
    ) {
        if !matches!(merken, Merken::Nein | Merken::Neu) {
            self.umkehr_laeuft
                .set(self.umkehr_laeuft.get().saturating_sub(1));
        }
        let vorgang = Vorgang::aus(op, quellen, ziel, outcome, beginn);
        {
            let mut stapel = self.stapel.borrow_mut();
            match merken {
                Merken::Nein | Merken::Teil => {}
                Merken::Neu => match vorgang {
                    Some(v) => {
                        protokoll(&format!(
                            "Rückgängig gemerkt: {} ({} Objekte{})",
                            v.beschreibung(),
                            v.paare.len(),
                            if cancelled { ", abgebrochen" } else { "" }
                        ));
                        stapel.neu(v);
                    }
                    None if matches!(op, Op::Delete | Op::EmptyTrash) && outcome.done > 0 => {
                        protokoll("Rückgängig: Stapel geleert (endgültig gelöscht)");
                        stapel.leeren();
                    }
                    None => {}
                },
                Merken::Rueckgaengig(v) if outcome.done > 0 => {
                    protokoll(&format!(
                        "Rückgängig fertig: {} ({} Objekte)",
                        v.beschreibung(),
                        outcome.done
                    ));
                    stapel.nach_rueckgaengig(v);
                }
                Merken::Rueckgaengig(v) => {
                    protokoll(&format!("Rückgängig ohne Wirkung: {}", v.beschreibung()));
                }
                Merken::Wiederholen => match vorgang {
                    Some(v) => {
                        protokoll(&format!("Wiederholt: {}", v.beschreibung()));
                        stapel.nach_wiederholen(v);
                    }
                    None => protokoll("Wiederholen ohne Wirkung"),
                },
            }
        }
        self.aktualisieren();
    }

    /// The menu text of "Rückgängig" (`vor`: "Wiederholen") with what it
    /// would do.
    pub fn umkehr_text(&self, vor: bool) -> String {
        let stapel = self.stapel.borrow();
        let naechstes = if vor {
            stapel.naechstes_vor()
        } else {
            stapel.naechstes_zurueck()
        };
        match (vor, naechstes) {
            (false, Some(v)) => gettext_f("_Undo: {what}", &[("what", &v.beschreibung())]),
            (false, None) => gettext("_Undo"),
            (true, Some(v)) => gettext_f("_Redo: {what}", &[("what", &v.beschreibung())]),
            (true, None) => gettext("_Redo"),
        }
    }

    /// Strg+Z: undoes the newest operation of the window.
    fn rueckgaengig(self: &Rc<Self>) {
        if self.umkehr_laeuft.get() > 0 {
            return;
        }
        let Some(v) = self.stapel.borrow_mut().rueckgaengig() else {
            return;
        };
        protokoll(&format!("Rückgängig: {}", v.beschreibung()));
        if v.art == Art::Papierkorb {
            let eintraege = papierkorb_eintraege();
            let uris = rueckgaengig::passende_eintraege(&eintraege, &v.originale(), &v.beginn);
            protokoll(&format!(
                "Rückgängig: {} von {} Einträgen im Papierkorb gefunden",
                uris.len(),
                v.paare.len()
            ));
            if uris.is_empty() {
                self.nicht_umkehrbar(&v);
            } else {
                self.umkehr_starten(
                    vec![Schritt {
                        op: Op::Restore,
                        quellen: uris,
                        ziel: None,
                    }],
                    v,
                );
            }
            return;
        }
        let da: Vec<String> = v.jetzt().into_iter().filter(|u| vorhanden(u)).collect();
        let schritte = v.umkehren(&da, leer);
        if schritte.is_empty() {
            self.nicht_umkehrbar(&v);
            return;
        }
        if v.art != Art::Kopieren || !nachfragen() {
            self.umkehr_starten(schritte, v);
            return;
        }
        let message = match da.as_slice() {
            [eine] => gettext_f(
                "Delete the copy “{name}”?",
                &[("name", &uri::name_of(eine))],
            ),
            mehr => ngettext_f(
                "Delete the {n} copied item?",
                "Delete the {n} copied items?",
                u32::try_from(mehr.len()).unwrap_or(u32::MAX),
                &[],
            ),
        };
        let dialog = gtk4::AlertDialog::builder()
            .message(message)
            .detail(gettext("Undoing the copy deletes the copies permanently."))
            .buttons([gettext("Cancel"), gettext("Delete")])
            .cancel_button(0)
            .default_button(0)
            .modal(true)
            .build();
        protokoll("Frage: Kopie rückgängig (endgültig löschen)");
        let weak = Rc::downgrade(self);
        dialog.choose(
            self.window().as_ref(),
            gio::Cancellable::NONE,
            move |answer| {
                let Some(ops) = weak.upgrade() else { return };
                if answer == Ok(1) {
                    ops.umkehr_starten(schritte, v);
                } else {
                    protokoll("Kopie rückgängig abgebrochen");
                    ops.stapel.borrow_mut().nach_wiederholen(v);
                    ops.aktualisieren();
                }
            },
        );
    }

    /// Umschalt+Strg+Z: does the newest undone operation again.
    fn wiederholen(self: &Rc<Self>) {
        if self.umkehr_laeuft.get() > 0 {
            return;
        }
        let Some(v) = self.stapel.borrow_mut().wiederholen() else {
            return;
        };
        protokoll(&format!("Wiederholen: {}", v.beschreibung()));
        let s = v.wiederholen();
        self.umkehr_laeuft.set(self.umkehr_laeuft.get() + 1);
        self.starten(
            Auftrag::new(
                s.op,
                s.quellen.iter().map(|u| gio::File::for_uri(u)).collect(),
                s.ziel.as_deref().map(gio::File::for_uri),
            )
            .merken(Merken::Wiederholen),
        );
        self.aktualisieren();
    }

    fn umkehr_starten(&self, schritte: Vec<Schritt>, v: Vorgang) {
        let n = schritte.len();
        for (i, s) in schritte.into_iter().enumerate() {
            let merken = if i + 1 == n {
                Merken::Rueckgaengig(v.clone())
            } else {
                Merken::Teil
            };
            self.umkehr_laeuft.set(self.umkehr_laeuft.get() + 1);
            self.starten(
                Auftrag::new(
                    s.op,
                    s.quellen.iter().map(|u| gio::File::for_uri(u)).collect(),
                    s.ziel.as_deref().map(gio::File::for_uri),
                )
                .merken(merken),
            );
        }
        self.aktualisieren();
    }

    /// The objects of `v` are gone: the entry is dropped with a message.
    fn nicht_umkehrbar(&self, v: &Vorgang) {
        protokoll(&format!("Rückgängig nicht möglich: {}", v.beschreibung()));
        fehler(
            self.window().as_ref(),
            &gettext("The operation cannot be undone."),
            &gettext("The items are no longer where the operation left them."),
        );
        self.aktualisieren();
    }
}

/// Whether `uri` names an object (a link counts as itself, also a dead one).
fn vorhanden(uri: &str) -> bool {
    gio::File::for_uri(uri).query_file_type(
        gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
        gio::Cancellable::NONE,
    ) != gio::FileType::Unknown
}

/// Whether a new object is still empty (an empty file, a folder without
/// entries): undoing it then deletes it, else it goes into the trash.
fn leer(uri: &str) -> bool {
    let datei = gio::File::for_uri(uri);
    match datei.query_file_type(
        gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
        gio::Cancellable::NONE,
    ) {
        gio::FileType::Directory => datei
            .enumerate_children(
                "standard::name",
                gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
                gio::Cancellable::NONE,
            )
            .ok()
            .is_some_and(|e| matches!(e.next_file(gio::Cancellable::NONE), Ok(None))),
        gio::FileType::Regular => datei
            .query_info(
                "standard::size",
                gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
                gio::Cancellable::NONE,
            )
            .is_ok_and(|i| i.size() == 0),
        _ => false,
    }
}

/// The entries of the trash with origin and deletion date: `trash:///`
/// with GVfs, else the home trash and its `.trashinfo` files.
fn papierkorb_eintraege() -> Vec<rueckgaengig::Eintrag> {
    let mut liste = Vec::new();
    if papierkorb::gvfs_trash() {
        let trash = gio::File::for_uri(papierkorb::TRASH_URI);
        if let Ok(kinder) = trash.enumerate_children(
            "standard::name,trash::orig-path,trash::deletion-date",
            gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
            gio::Cancellable::NONE,
        ) {
            while let Ok(Some(info)) = kinder.next_file(gio::Cancellable::NONE) {
                let (Some(herkunft), Some(datum)) = (
                    papierkorb::orig_path(&info),
                    info.attribute_string("trash::deletion-date"),
                ) else {
                    continue;
                };
                liste.push(rueckgaengig::Eintrag {
                    uri: trash.child(info.name()).uri().to_string(),
                    herkunft,
                    datum: datum.to_string(),
                });
            }
        }
        return liste;
    }
    let oben = papierkorb::home_trash();
    let Ok(dateien) = std::fs::read_dir(oben.join("files")) else {
        return liste;
    };
    for d in dateien.filter_map(Result::ok) {
        let mut name = d.file_name();
        name.push(".trashinfo");
        let info: PathBuf = oben.join("info").join(name);
        let Some((herkunft, datum)) = std::fs::read_to_string(&info)
            .ok()
            .and_then(|t| rueckgaengig::trashinfo_lesen(&t))
        else {
            continue;
        };
        liste.push(rueckgaengig::Eintrag {
            uri: gio::File::for_path(d.path()).uri().to_string(),
            herkunft: rueckgaengig::herkunft_absolut(herkunft, std::path::Path::new("/")),
            datum,
        });
    }
    liste
}

/// For the views (B2): whether `uri` was cut and waits (shown faded).
pub fn ist_ausgeschnitten(uri: &str) -> bool {
    Zwischenablage::get().ist_ausgeschnitten(uri)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrived_names_become_files_of_the_target() {
        let ordner = gio::File::for_path("/tmp/ziel");
        assert_eq!(
            angekommen(&ordner, "x (Kopie).txt").path().unwrap(),
            std::path::PathBuf::from("/tmp/ziel/x (Kopie).txt")
        );
        // restoring reports where the object went back to
        assert_eq!(
            angekommen(&ordner, "/home/a/b.txt").path().unwrap(),
            std::path::PathBuf::from("/home/a/b.txt")
        );
    }

    #[test]
    fn the_actions_are_those_of_the_interface() {
        // docs/dateien.md §3.6, B3 rows; keys come from menue.rs
        for name in AKTIONEN {
            assert!(
                crate::menue::KUERZEL_IM_BEREICH
                    .iter()
                    .any(|(a, _)| *a == name)
                    || [
                        "in-ordner-einfuegen",
                        "papierkorb-leeren",
                        "wiederherstellen",
                        "ordner-anlegen",
                        "dokument-anlegen"
                    ]
                    .contains(&name),
                "{name}"
            );
        }
    }
}
