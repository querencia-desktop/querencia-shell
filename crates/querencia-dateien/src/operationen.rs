//! The file operations of a window, as actions (`win.*`) for menus,
//! context menus and keys, on the job queue of `querencia_dateiarbeit`.
//!
//! | Action | Does | Key |
//! |---|---|---|
//! | `win.oeffnen` | opens the selection (folders in the window, files in their program) | Strg+O |
//! | `win.oeffnen-mit` (s) | with the program of that ID; empty: "Mit anderer Anwendung …" | – |
//! | `win.ausschneiden`, `win.kopieren` | onto the clipboard (all formats, `zwischenablage`) | Strg+X, Strg+C |
//! | `win.einfuegen` | the clipboard's files into the shown folder | Strg+V |
//! | `win.einfuegen-in-ordner` | … into the selected folder | – |
//! | `win.umbenennen` | starts renaming (in the view, else a small dialog) | F2 |
//! | `win.papierkorb` | into the trash, **without** a question (in the trash: deleting, with one) | Entf |
//! | `win.loeschen` | deletes for good after a question, default Abbrechen | Umschalt+Entf |
//! | `win.papierkorb-leeren` | empties the trash after a question, default Abbrechen | – |
//! | `win.wiederherstellen` | puts trash entries back where they were | – |
//! | `win.neuer-ordner` | "Neuer Ordner", then renaming | Umschalt+Strg+N |
//! | `win.neues-dokument` (s) | a copy of a template (path), empty: "Leere Datei" | – |
//! | `win.eigenschaften` | the properties of the selection, else of the folder | Alt+Eingabe, Strg+I |
//!
//! Every change of files is a job (its own thread) with progress
//! (`fortschritt`), cancel and questions (`konflikt`, E2: Überspringen is
//! the default). One queue, one progress window and one set of questions
//! serve every window of the process ([`Dienst`]). Undo comes after the
//! beta (E8).
//!
//! The window tells this module what it shows through [`Kontext`].

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gio_unix::DesktopAppInfo;
use gtk4::prelude::*;
use gtk4::{gdk, gio, glib};
use querencia_dateiarbeit::auftraege::{self, Event, JobId, Op, Outcome, Queue};
use querencia_dateiarbeit::papierkorb::{self, Papierkorb};
use querencia_dateiarbeit::{namen, oeffnen, typ, uri};

use crate::fortschritt::{self, Fortschritt};
use crate::i18n::{gettext, gettext_f, ngettext_f};
use crate::konflikt::Fragen;
use crate::ziehen::{self, Ablage, Ziel};
use crate::zwischenablage::Zwischenablage;

/// A log line on stderr.
pub fn protokoll(text: &str) {
    querencia_dateiarbeit::say!("querencia-dateien: {text}");
}

/// `QUERENCIA_DATEIEN_DEBUG=1`: more log lines (positions for the isolated
/// GUI test, every progress step).
pub fn debugging() -> bool {
    thread_local! {
        static AN: bool = std::env::var_os("QUERENCIA_DATEIEN_DEBUG").is_some_and(|v| v == "1");
    }
    AN.with(|a| *a)
}

/// What the window shows; the window (B1's `Ansicht`) implements it.
pub trait Kontext {
    /// The folder shown (`trash:///` for the trash).
    fn ordner(&self) -> gio::File;
    /// The selected files, in view order.
    fn auswahl(&self) -> Vec<gio::File>;
    /// Shows `ort` in this window (a folder, `trash:///`, `computer:///`).
    fn ordner_oeffnen(&self, ort: &gio::File);
    /// Shows `ort` in a new window (several folders opened at once).
    fn neues_fenster(&self, ort: &gio::File);
    /// Starts renaming `datei` in the view (F2 in place); `false` when the
    /// view cannot, then a small dialog asks. The view calls
    /// [`Operationen::umbenennen`] with the new name.
    fn umbenennen_beginnen(&self, _datei: &gio::File) -> bool {
        false
    }
    /// Selects `dateien` once they are shown (after paste, new folder).
    fn auswaehlen(&self, _dateien: &[gio::File]) {}
}

/// What follows when a job ends: (outcome, cancelled).
type Nachher = Box<dyn FnOnce(&Outcome, bool)>;

struct Laufend {
    op: Op,
    ziel_name: String,
    fenster: glib::WeakRef<gtk4::Window>,
    nachher: Option<Nachher>,
}

/// The job queue, the window "Dateioperationen" and the open questions of
/// the process; every window starts its jobs here.
pub struct Dienst {
    queue: Queue,
    fortschritt: Rc<Fortschritt>,
    fragen: Rc<Fragen>,
    laufend: RefCell<HashMap<JobId, Laufend>>,
}

/// A job as a window starts it.
pub struct Auftrag {
    pub op: Op,
    pub quellen: Vec<gio::File>,
    pub ziel: Option<gio::File>,
    pub fenster: Option<gtk4::Window>,
    nachher: Option<Nachher>,
}

impl Auftrag {
    pub fn new(op: Op, quellen: Vec<gio::File>, ziel: Option<gio::File>) -> Self {
        Self {
            op,
            quellen,
            ziel,
            fenster: None,
            nachher: None,
        }
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
    /// The one of this process (made on the GTK main thread).
    pub fn get() -> Rc<Self> {
        thread_local! {
            static EINER: RefCell<Option<Rc<Dienst>>> = const { RefCell::new(None) };
        }
        EINER.with(|einer| {
            if let Some(d) = einer.borrow().as_ref() {
                return d.clone();
            }
            let queue = Queue::new();
            let cancel_queue = queue.clone();
            let dienst = Rc::new(Dienst {
                fortschritt: Fortschritt::new(move |id| {
                    protokoll(&format!("Auftrag {id} abbrechen"));
                    cancel_queue.cancel(id);
                }),
                fragen: Rc::default(),
                laufend: RefCell::default(),
                queue,
            });
            let weak = Rc::downgrade(&dienst);
            dienst.queue.subscribe(move |event| {
                if let Some(dienst) = weak.upgrade() {
                    dienst.ereignis(event);
                }
            });
            *einer.borrow_mut() = Some(dienst.clone());
            dienst
        })
    }

    /// Starts `auftrag`; its progress shows after a second.
    pub fn starten(&self, auftrag: Auftrag) -> JobId {
        let Auftrag {
            op,
            quellen,
            ziel,
            fenster,
            nachher,
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
        let id = self.queue.start(auftraege::Job::new(
            op.clone(),
            quellen.iter().map(|f| f.uri().to_string()).collect(),
            ziel.as_ref().map(|z| z.uri().to_string()),
        ));
        let weak = glib::WeakRef::new();
        weak.set(fenster.as_ref());
        self.laufend.borrow_mut().insert(
            id,
            Laufend {
                op: op.clone(),
                ziel_name,
                fenster: weak,
                nachher,
            },
        );
        self.fortschritt.begonnen(id, op, title);
        id
    }

    fn ereignis(&self, event: &Event) {
        match event {
            Event::Started {
                id,
                total_items,
                total_bytes,
                ..
            } => {
                if debugging() {
                    protokoll(&format!(
                        "Auftrag {id} gezählt: {total_items:?} Objekte, {total_bytes:?} Bytes"
                    ));
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
                let (op, folder, parent) = self
                    .laufend
                    .borrow()
                    .get(id)
                    .map(|l| (l.op.clone(), l.ziel_name.clone(), l.fenster.upgrade()))
                    .unwrap_or((Op::Copy, String::new(), None));
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
                    parent.as_ref(),
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
                let laufend = self.laufend.borrow_mut().remove(id);
                if let Some(laufend) = laufend {
                    self.fertig(laufend, outcome, *cancelled);
                }
            }
        }
    }

    fn fertig(&self, laufend: Laufend, outcome: &Outcome, cancelled: bool) {
        let Laufend {
            op,
            fenster,
            nachher,
            ..
        } = laufend;
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
        let fehlgeschlagen = !outcome.failed.is_empty() && !cancelled;
        let umbenennen = matches!(op, Op::Rename { .. });
        if let Some(nachher) = nachher {
            nachher(outcome, cancelled);
        }
        // renaming shows its own message (in `nachher`)
        if fehlgeschlagen && !umbenennen {
            fehler(
                fenster.upgrade().as_ref(),
                &op.failure_title(),
                &auftraege::failure_text(&op, outcome),
            );
        }
    }

    /// The jobs that still run (for closing the last window: they go on
    /// while the process lives).
    pub fn laeuft(&self) -> bool {
        !self.laufend.borrow().is_empty()
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

/// Templates for "Dokument anlegen": the files of the XDG templates folder
/// (hidden ones left out), sorted, with their subfolders (one level); none
/// when the folder is the home folder or missing (Nautilus 2.30).
pub fn vorlagen(dir: Option<&Path>, home: &Path) -> Vec<(String, Vec<PathBuf>)> {
    let Some(dir) = dir.filter(|dir| *dir != home) else {
        return Vec::new();
    };
    let lesen = |dir: &Path| -> (Vec<PathBuf>, Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return (Vec::new(), Vec::new());
        };
        let mut files = Vec::new();
        let mut dirs = Vec::new();
        for entry in entries.filter_map(Result::ok) {
            if entry.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            match std::fs::metadata(entry.path()) {
                Ok(m) if m.is_file() => files.push(entry.path()),
                Ok(m) if m.is_dir() => dirs.push(entry.path()),
                _ => {}
            }
        }
        files.sort();
        dirs.sort();
        (files, dirs)
    };
    let (files, dirs) = lesen(dir);
    let mut groups = vec![(String::new(), files)];
    for sub in dirs {
        let (files, _) = lesen(&sub);
        if !files.is_empty() {
            let name = sub
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            groups.push((name, files));
        }
    }
    groups.retain(|(_, files)| !files.is_empty());
    groups
}

/// The name a template's copy gets: the template's name without its
/// extension's dot rules changed ("Brief.odt" stays "Brief.odt").
fn vorlagen_name(template: &Path) -> String {
    template
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| gettext("Template"))
}

/// "Dokument anlegen ▸": the templates, then "Leere Datei"; "Keine Vorlagen
/// installiert" (disabled) when there are none.
pub fn vorlagen_menue() -> gio::Menu {
    let menu = gio::Menu::new();
    let groups = vorlagen(
        glib::user_special_dir(glib::UserDirectory::Templates).as_deref(),
        &glib::home_dir(),
    );
    let eintrag = |path: &Path| {
        let label = path
            .file_stem()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let item = gio::MenuItem::new(Some(&label), None);
        item.set_action_and_target_value(
            Some("win.neues-dokument"),
            Some(&path.to_string_lossy().to_variant()),
        );
        item
    };
    let vorne = gio::Menu::new();
    if groups.is_empty() {
        vorne.append(
            Some(&gettext("No templates installed")),
            Some("win.keine-vorlagen"),
        );
    }
    for (name, files) in &groups {
        if name.is_empty() {
            for file in files {
                vorne.append_item(&eintrag(file));
            }
        } else {
            let sub = gio::Menu::new();
            for file in files {
                sub.append_item(&eintrag(file));
            }
            vorne.append_submenu(Some(name), &sub);
        }
    }
    menu.append_section(None, &vorne);
    let leer = gio::Menu::new();
    let item = gio::MenuItem::new(Some(&gettext("Empty File")), None);
    item.set_action_and_target_value(Some("win.neues-dokument"), Some(&"".to_variant()));
    leer.append_item(&item);
    menu.append_section(None, &leer);
    menu
}

/// The content type a program is chosen for (an empty file: the type its
/// name suggests, Desktop 0.2.2).
fn programmtyp(info: &gio::FileInfo, file: &gio::File) -> String {
    let found = info.content_type().map(|t| t.to_string());
    if typ::is_empty_document(found.as_deref())
        && let Some(by_name) = file
            .basename()
            .and_then(|n| typ::type_from_name(&n.to_string_lossy()))
    {
        return by_name;
    }
    found.unwrap_or_else(|| "application/octet-stream".to_owned())
}

fn programmtyp_von(file: &gio::File) -> Option<String> {
    file.query_info(
        "standard::content-type",
        gio::FileQueryInfoFlags::NONE,
        gio::Cancellable::NONE,
    )
    .ok()
    .map(|info| programmtyp(&info, file))
}

/// The one content type of `dateien` ("Öffnen mit" only for one type, as
/// Nautilus 2.30).
pub fn gemeinsamer_typ(dateien: &[gio::File]) -> Option<String> {
    let mut types = dateien.iter().map(programmtyp_von);
    let first = types.next()??;
    types
        .all(|t| t.as_deref() == Some(first.as_str()))
        .then_some(first)
}

/// "Öffnen mit ▸": the programs for the selection's type
/// (`oeffnen::apps_for`), then "Mit anderer Anwendung öffnen …".
pub fn oeffnen_mit_menue(dateien: &[gio::File]) -> Option<gio::Menu> {
    let content_type = gemeinsamer_typ(dateien)?;
    let menu = gio::Menu::new();
    let apps = gio::Menu::new();
    for app in oeffnen::apps_for(&content_type) {
        let Some(id) = app.id().map(|id| id.to_string()) else {
            continue;
        };
        if !app.should_show() {
            continue;
        }
        let item = gio::MenuItem::new(Some(&app.display_name()), None);
        item.set_action_and_target_value(Some("win.oeffnen-mit"), Some(&id.to_variant()));
        if let Some(icon) = app.icon() {
            item.set_icon(&icon);
        }
        apps.append_item(&item);
    }
    menu.append_section(None, &apps);
    let other = gio::Menu::new();
    let item = gio::MenuItem::new(Some(&gettext("Open With Other Application…")), None);
    item.set_action_and_target_value(Some("win.oeffnen-mit"), Some(&"".to_variant()));
    other.append_item(&item);
    menu.append_section(None, &other);
    Some(menu)
}

/// The first entry of a file's context menu: "Mit „Editor“ öffnen" when one
/// program opens all of `dateien`, else "Öffnen".
pub fn oeffnen_beschriftung(dateien: &[gio::File]) -> String {
    let app = gemeinsamer_typ(dateien)
        .and_then(|t| gio::AppInfo::default_for_type(&t, false))
        .filter(|_| {
            !dateien.iter().any(|f| {
                f.query_file_type(gio::FileQueryInfoFlags::NONE, gio::Cancellable::NONE)
                    == gio::FileType::Directory
            })
        });
    match app {
        Some(app) => gettext_f("Open With {app}", &[("app", &app.display_name())]),
        None => gettext("Open"),
    }
}

/// The keys of the actions that hold everywhere in the window (application
/// accelerators): (action, keys).
pub const KUERZEL_FENSTER: &[(&str, &[&str])] = &[
    ("win.oeffnen", &["<Control>o"]),
    ("win.neuer-ordner", &["<Shift><Control>n"]),
    ("win.eigenschaften", &["<Alt>Return", "<Control>i"]),
];

/// The keys that hold only in the view (they would take Strg+C and Entf
/// from text fields such as the location bar).
pub const KUERZEL_ANSICHT: &[(&str, &[&str])] = &[
    ("win.ausschneiden", &["<Control>x"]),
    ("win.kopieren", &["<Control>c"]),
    ("win.einfuegen", &["<Control>v"]),
    ("win.umbenennen", &["F2"]),
    ("win.papierkorb", &["Delete", "KP_Delete"]),
    ("win.loeschen", &["<Shift>Delete", "<Shift>KP_Delete"]),
];

/// Sets [`KUERZEL_FENSTER`] for the application.
pub fn kuerzel_fenster(app: &impl IsA<gtk4::Application>) {
    for (action, keys) in KUERZEL_FENSTER {
        app.set_accels_for_action(action, keys);
    }
}

/// Puts [`KUERZEL_ANSICHT`] on `ansicht` (a view: only while it or a child
/// has the keyboard).
pub fn kuerzel_ansicht(ansicht: &impl IsA<gtk4::Widget>) {
    let controller = gtk4::ShortcutController::new();
    controller.set_scope(gtk4::ShortcutScope::Local);
    for (action, keys) in KUERZEL_ANSICHT {
        for key in *keys {
            if let Some(trigger) = gtk4::ShortcutTrigger::parse_string(key) {
                controller.add_shortcut(gtk4::Shortcut::new(
                    Some(trigger),
                    Some(gtk4::NamedAction::new(action)),
                ));
            }
        }
    }
    ansicht.add_controller(controller);
}

const AKTIONEN: [&str; 15] = [
    "oeffnen",
    "oeffnen-mit",
    "ausschneiden",
    "kopieren",
    "einfuegen",
    "einfuegen-in-ordner",
    "umbenennen",
    "papierkorb",
    "loeschen",
    "papierkorb-leeren",
    "wiederherstellen",
    "neuer-ordner",
    "neues-dokument",
    "eigenschaften",
    "keine-vorlagen",
];

/// The operations of one window.
pub struct Operationen {
    fenster: glib::WeakRef<gtk4::ApplicationWindow>,
    kontext: Rc<dyn Kontext>,
    aktionen: RefCell<HashMap<&'static str, gio::SimpleAction>>,
    papierkorb: Papierkorb,
}

fn ist_ordner(file: &gio::File) -> bool {
    file.query_file_type(gio::FileQueryInfoFlags::NONE, gio::Cancellable::NONE)
        == gio::FileType::Directory
}

impl Operationen {
    /// Adds the actions to `fenster`; `kontext` tells what it shows.
    pub fn einrichten(
        fenster: &impl IsA<gtk4::ApplicationWindow>,
        kontext: Rc<dyn Kontext>,
    ) -> Rc<Self> {
        let ops = Rc::new(Self {
            fenster: fenster.upcast_ref::<gtk4::ApplicationWindow>().downgrade(),
            kontext,
            aktionen: RefCell::default(),
            papierkorb: Papierkorb::new(),
        });
        for name in AKTIONEN {
            let param = matches!(name, "oeffnen-mit" | "neues-dokument");
            let action = gio::SimpleAction::new(name, param.then_some(glib::VariantTy::STRING));
            let weak = Rc::downgrade(&ops);
            action.connect_activate(move |_, value| {
                if let Some(ops) = weak.upgrade() {
                    let value = value.and_then(|v| v.str()).unwrap_or_default().to_owned();
                    ops.ausfuehren(name, &value);
                }
            });
            fenster
                .upcast_ref::<gtk4::ApplicationWindow>()
                .add_action(&action);
            ops.aktionen.borrow_mut().insert(name, action);
        }
        let weak = Rc::downgrade(&ops);
        ops.papierkorb.connect_changed(move |_| {
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
        // the queue and its windows exist before the first job
        Dienst::get();
        ops.aktualisieren();
        ops
    }

    fn fenster(&self) -> Option<gtk4::Window> {
        self.fenster.upgrade().map(|w| w.upcast())
    }

    fn auftrag(&self, op: Op, quellen: Vec<gio::File>, ziel: Option<gio::File>) -> Auftrag {
        Auftrag::new(op, quellen, ziel).fenster(self.fenster())
    }

    /// Whether the shown folder is the trash.
    pub fn im_papierkorb(&self) -> bool {
        let ordner = self.kontext.ordner();
        ordner.uri_scheme().is_some_and(|s| s == "trash")
            || ordner.equal(&gio::File::for_path(papierkorb::home_trash().join("files")))
    }

    /// Enables the actions for the selection; the window calls it when the
    /// selection or the folder changes.
    pub fn aktualisieren(&self) {
        let auswahl = self.kontext.auswahl();
        let eine = auswahl.len() == 1;
        let etwas = !auswahl.is_empty();
        let papierkorb = self.im_papierkorb();
        let leer = self.papierkorb.anzahl().is_some_and(|n| n == 0);
        let einfuegen = Zwischenablage::get().kann_einfuegen() && !papierkorb;
        let aktionen = self.aktionen.borrow();
        let setzen = |name: &str, an: bool| {
            if let Some(action) = aktionen.get(name) {
                action.set_enabled(an);
            }
        };
        setzen("oeffnen", etwas);
        setzen("oeffnen-mit", etwas && !papierkorb);
        setzen("ausschneiden", etwas);
        setzen("kopieren", etwas);
        setzen("einfuegen", einfuegen);
        setzen(
            "einfuegen-in-ordner",
            einfuegen && eine && ist_ordner(&auswahl[0]),
        );
        setzen("umbenennen", eine && !papierkorb);
        setzen("papierkorb", etwas);
        setzen("loeschen", etwas);
        setzen("papierkorb-leeren", !leer);
        setzen("wiederherstellen", etwas && papierkorb);
        setzen("neuer-ordner", !papierkorb);
        setzen("neues-dokument", !papierkorb);
        setzen("eigenschaften", true);
        setzen("keine-vorlagen", false);
    }

    fn ausfuehren(self: &Rc<Self>, name: &str, wert: &str) {
        let auswahl = self.kontext.auswahl();
        if debugging() {
            protokoll(&format!(
                "Aktion win.{name} ({wert}) für {} Objekte",
                auswahl.len()
            ));
        }
        match name {
            "oeffnen" => self.oeffnen(&auswahl),
            "oeffnen-mit" => self.oeffnen_mit(&auswahl, wert),
            "ausschneiden" => self.in_zwischenablage(&auswahl, true),
            "kopieren" => self.in_zwischenablage(&auswahl, false),
            "einfuegen" => self.einfuegen(self.kontext.ordner()),
            "einfuegen-in-ordner" => {
                if let [ordner] = auswahl.as_slice() {
                    self.einfuegen(ordner.clone());
                }
            }
            "umbenennen" => {
                if let [datei] = auswahl.as_slice() {
                    self.umbenennen_beginnen(datei);
                }
            }
            "papierkorb" if self.im_papierkorb() => self.loeschen_fragen(auswahl),
            "papierkorb" => self.in_papierkorb(auswahl),
            "loeschen" => self.loeschen_fragen(auswahl),
            "papierkorb-leeren" => self.leeren_fragen(),
            "wiederherstellen" => self.wiederherstellen(auswahl),
            "neuer-ordner" => self.neuer_ordner(),
            "neues-dokument" => self.neues_dokument(wert),
            "eigenschaften" => {
                let ziele = if auswahl.is_empty() {
                    vec![self.kontext.ordner()]
                } else {
                    auswahl
                };
                // one window each, as many as Nautilus 2.30 opened at most
                for datei in ziele.into_iter().take(8) {
                    crate::eigenschaften_dialog::zeigen(self.fenster().as_ref(), datei, self);
                }
            }
            _ => {}
        }
    }

    fn launch_context(&self) -> gio::AppLaunchContext {
        let display = self
            .fenster()
            .map(|w| WidgetExt::display(&w))
            .or_else(gdk::Display::default);
        let context: gio::AppLaunchContext = match display {
            Some(display) => display.app_launch_context().upcast(),
            None => gio::AppLaunchContext::new(),
        };
        querencia_oberflaeche::portal::for_children(&context);
        context
    }

    /// Öffnen: folders in this window (several: a window each), places of
    /// GVfs too, files in their program (`oeffnen::plan_open`).
    pub fn oeffnen(self: &Rc<Self>, dateien: &[gio::File]) {
        let viele_ordner = dateien.iter().filter(|f| ist_ordner(f)).count() > 1;
        for datei in dateien {
            let weak = Rc::downgrade(self);
            let datei = datei.clone();
            glib::MainContext::default().spawn_local(async move {
                let info = datei
                    .query_info_future(
                        oeffnen::OPEN_ATTRIBUTES,
                        gio::FileQueryInfoFlags::NONE,
                        glib::Priority::DEFAULT,
                    )
                    .await;
                let Some(ops) = weak.upgrade() else { return };
                match info {
                    Ok(info) => ops.oeffnen_nach_plan(&datei, &info, viele_ordner),
                    Err(err) if datei.uri_scheme().is_some_and(|s| s != "file") => {
                        // computer:///, network:/// … without GVfs info
                        protokoll(&format!("{}: keine Angaben ({err}), als Ort", datei.uri()));
                        ops.kontext.ordner_oeffnen(&datei);
                    }
                    Err(err) => fehler(
                        ops.fenster().as_ref(),
                        &gettext_f(
                            "“{name}” could not be opened.",
                            &[("name", &anzeigename(&datei))],
                        ),
                        err.message(),
                    ),
                }
            });
        }
    }

    fn oeffnen_nach_plan(self: &Rc<Self>, datei: &gio::File, info: &gio::FileInfo, viele: bool) {
        let name = anzeigename(datei);
        let context = self.launch_context();
        let starten = |app: &gio::AppInfo| {
            let ergebnis = if app.supports_uris() {
                app.launch_uris(&[datei.uri().as_str()], Some(&context))
            } else {
                app.launch(std::slice::from_ref(datei), Some(&context))
            };
            match ergebnis {
                Ok(()) => protokoll(&format!(
                    "geöffnet mit {}: {name}",
                    app.id().unwrap_or_default()
                )),
                Err(err) => fehler(
                    self.fenster().as_ref(),
                    &gettext_f("“{name}” could not be opened.", &[("name", &name)]),
                    err.message(),
                ),
            }
        };
        match oeffnen::plan_open(datei, info) {
            oeffnen::OpenPlan::Folder(_) | oeffnen::OpenPlan::Location(..) => {
                if viele {
                    self.kontext.neues_fenster(datei);
                } else {
                    self.kontext.ordner_oeffnen(datei);
                }
            }
            oeffnen::OpenPlan::Default(app) | oeffnen::OpenPlan::ByName(_, app) => starten(&app),
            oeffnen::OpenPlan::NoProgram(content_type) => {
                self.kein_programm(datei, &name, &content_type)
            }
            oeffnen::OpenPlan::Starter { trusted } => self.starter(datei, &name, trusted),
            oeffnen::OpenPlan::Executable { text } => {
                self.ausfuehrbar(datei, &name, text, programmtyp(info, datei))
            }
        }
    }

    /// QD-102: no program opens the file: says so and offers the choice.
    fn kein_programm(self: &Rc<Self>, datei: &gio::File, name: &str, content_type: &str) {
        let description = gio::content_type_get_description(content_type);
        let dialog = gtk4::AlertDialog::builder()
            .message(gettext_f("“{name}” cannot be opened.", &[("name", name)]))
            .detail(gettext_f(
                "There is no application installed for “{type}” files ({mime}).",
                &[("type", &description), ("mime", content_type)],
            ))
            .buttons([gettext("Close"), gettext("Open With Other Application…")])
            .cancel_button(0)
            .default_button(1)
            .modal(true)
            .build();
        protokoll(&format!("Hinweis: kein Programm für {name}"));
        let weak = Rc::downgrade(self);
        let datei = datei.clone();
        dialog.choose(
            self.fenster().as_ref(),
            gio::Cancellable::NONE,
            move |answer| {
                if answer == Ok(1)
                    && let Some(ops) = weak.upgrade()
                {
                    ops.programm_waehlen(&[datei]);
                }
            },
        );
    }

    /// Nautilus 2.30's question for an untrusted starter (texts as the
    /// desktop).
    fn starter(self: &Rc<Self>, datei: &gio::File, name: &str, trusted: bool) {
        let Some(path) = datei.path() else { return };
        let context = self.launch_context();
        let starten = move |path: &Path, context: &gio::AppLaunchContext| {
            match DesktopAppInfo::from_filename(path) {
                Some(app) => app.launch(&[], Some(context)),
                None => Err(glib::Error::new(
                    gio::IOErrorEnum::NotFound,
                    "Starter ungültig",
                )),
            }
        };
        if trusted {
            if let Err(err) = starten(&path, &context) {
                fehler(
                    self.fenster().as_ref(),
                    &gettext_f("“{name}” could not be opened.", &[("name", name)]),
                    err.message(),
                );
            }
            return;
        }
        let dialog = gtk4::AlertDialog::builder()
            .message(gettext("Untrusted application launcher"))
            .detail(gettext_f(
                "The application launcher “{name}” has not been marked as trusted. \
                 If you do not know the source of this file, launching it may be unsafe.",
                &[("name", name)],
            ))
            .buttons([
                gettext("Cancel"),
                gettext("Mark as Trusted"),
                gettext("Launch Anyway"),
            ])
            .cancel_button(0)
            .default_button(0)
            .modal(true)
            .build();
        let name = name.to_owned();
        dialog.choose(
            self.fenster().as_ref(),
            gio::Cancellable::NONE,
            move |answer| {
                let answer = answer.unwrap_or(0);
                if answer == 0 {
                    protokoll(&format!("Starter {name} nicht gestartet"));
                    return;
                }
                if answer == 1
                    && let Err(err) = oeffnen::mark_trusted(&path)
                {
                    protokoll(&format!("Starter {name} nicht markiert: {err}"));
                }
                match starten(&path, &context) {
                    Ok(()) => protokoll(&format!("gestartet: {name}")),
                    Err(err) => protokoll(&format!("{name} nicht gestartet: {err}")),
                }
            },
        );
    }

    /// Nautilus 2.30: an executable text file asks "Ausführen oder
    /// anzeigen?", a program asks whether to run it. Abbrechen and Anzeigen
    /// are safe; nothing runs without a click on Ausführen.
    fn ausfuehrbar(
        self: &Rc<Self>,
        datei: &gio::File,
        name: &str,
        text: bool,
        content_type: String,
    ) {
        let Some(path) = datei.path() else { return };
        let (message, detail, buttons, default) = if text {
            (
                gettext_f(
                    "Do you want to run “{name}”, or display its contents?",
                    &[("name", name)],
                ),
                gettext_f("“{name}” is an executable text file.", &[("name", name)]),
                vec![gettext("Cancel"), gettext("Display"), gettext("Run")],
                1,
            )
        } else {
            (
                gettext_f("Do you want to run “{name}”?", &[("name", name)]),
                gettext_f("“{name}” is a program.", &[("name", name)]),
                vec![gettext("Cancel"), gettext("Run")],
                0,
            )
        };
        let run = buttons.len() - 1;
        let dialog = gtk4::AlertDialog::builder()
            .message(message)
            .detail(detail)
            .buttons(buttons.iter().map(String::as_str).collect::<Vec<_>>())
            .cancel_button(0)
            .default_button(default)
            .modal(true)
            .build();
        let weak = Rc::downgrade(self);
        let datei = datei.clone();
        let name = name.to_owned();
        dialog.choose(
            self.fenster().as_ref(),
            gio::Cancellable::NONE,
            move |answer| {
                let Some(ops) = weak.upgrade() else { return };
                match answer.map(|a| a as usize) {
                    Ok(a) if a == run => ops.ausfuehren_programm(&path, &name),
                    Ok(1) if text => {
                        // Anzeigen: the program for its type, never itself
                        match gio::AppInfo::default_for_type(&content_type, false)
                            .or_else(|| gio::AppInfo::default_for_type("text/plain", false))
                        {
                            Some(app) => {
                                let _ = app.launch_uris(
                                    &[datei.uri().as_str()],
                                    Some(&ops.launch_context()),
                                );
                                protokoll(&format!("angezeigt: {name}"));
                            }
                            None => ops.programm_waehlen(&[datei]),
                        }
                    }
                    _ => protokoll(&format!("nicht ausgeführt: {name}")),
                }
            },
        );
    }

    fn ausfuehren_programm(&self, path: &Path, name: &str) {
        let launcher = gio::SubprocessLauncher::new(gio::SubprocessFlags::NONE);
        if let Some(dir) = path.parent() {
            launcher.set_cwd(dir);
        }
        querencia_oberflaeche::portal::launcher_for_children(&launcher);
        match launcher.spawn(&[path.as_os_str()]) {
            Ok(_) => protokoll(&format!("ausgeführt: {name}")),
            Err(err) => fehler(
                self.fenster().as_ref(),
                &gettext_f("“{name}” could not be opened.", &[("name", name)]),
                err.message(),
            ),
        }
    }

    /// Öffnen mit `app_id`; empty: GTK's choice of programs.
    pub fn oeffnen_mit(self: &Rc<Self>, dateien: &[gio::File], app_id: &str) {
        if dateien.is_empty() {
            return;
        }
        if app_id.is_empty() {
            self.programm_waehlen(dateien);
            return;
        }
        match DesktopAppInfo::new(app_id) {
            Some(app) => self.mit_programm(&app.upcast(), dateien),
            None => protokoll(&format!("Programm {app_id} fehlt")),
        }
    }

    fn mit_programm(&self, app: &gio::AppInfo, dateien: &[gio::File]) {
        let context = self.launch_context();
        let uris: Vec<String> = dateien.iter().map(|f| f.uri().to_string()).collect();
        let refs: Vec<&str> = uris.iter().map(String::as_str).collect();
        let ergebnis = if app.supports_uris() {
            app.launch_uris(&refs, Some(&context))
        } else {
            app.launch(dateien, Some(&context))
        };
        let namen: Vec<String> = dateien.iter().map(anzeigename).collect();
        match ergebnis {
            Ok(()) => protokoll(&format!(
                "geöffnet mit {}: {}",
                app.id().unwrap_or_default(),
                namen.join(", ")
            )),
            Err(err) => fehler(
                self.fenster().as_ref(),
                &gettext_f(
                    "“{name}” could not be opened.",
                    &[("name", &namen.join(", "))],
                ),
                err.message(),
            ),
        }
    }

    /// "Mit anderer Anwendung öffnen …": GTK's app chooser for the type;
    /// the default program stays (Nautilus 2.30).
    #[allow(deprecated)] // GtkAppChooserDialog: GTK 4 has no replacement yet
    fn programm_waehlen(self: &Rc<Self>, dateien: &[gio::File]) {
        let content_type =
            gemeinsamer_typ(dateien).unwrap_or_else(|| "application/octet-stream".to_owned());
        let parent = self.fenster();
        let dialog = gtk4::AppChooserDialog::for_content_type(
            parent.as_ref(),
            gtk4::DialogFlags::MODAL | gtk4::DialogFlags::DESTROY_WITH_PARENT,
            &content_type,
        );
        dialog.set_title(Some(&gettext("Open With")));
        let erster = dateien.first().map(anzeigename).unwrap_or_default();
        dialog.set_heading(&match dateien.len() {
            1 => gettext_f(
                "Select an application to open “{name}”",
                &[("name", &erster)],
            ),
            n => ngettext_f(
                "Select an application to open {n} file",
                "Select an application to open {n} files",
                u32::try_from(n).unwrap_or(u32::MAX),
                &[],
            ),
        });
        let weak = Rc::downgrade(self);
        let dateien = dateien.to_vec();
        dialog.connect_response(move |dialog, response| {
            if response == gtk4::ResponseType::Ok
                && let Some(app) = dialog.app_info()
                && let Some(ops) = weak.upgrade()
            {
                ops.mit_programm(&app, &dateien);
            } else {
                protokoll("Öffnen mit abgebrochen");
            }
            dialog.destroy();
        });
        dialog.present();
        protokoll(&format!("Öffnen mit: Auswahl für {content_type}"));
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
        let auftrag = self
            .auftrag(op, quellen, Some(ordner))
            .nachher(move |outcome, _| {
                if ausgeschnitten && outcome.done > 0 {
                    // cut files are where they belong now: nothing to paste
                    Zwischenablage::get().leeren();
                }
                for alt in &aus_papierkorb {
                    if !alt.query_exists(gio::Cancellable::NONE) {
                        papierkorb::forget_info(alt);
                    }
                }
                if let Some(ops) = weak.upgrade() {
                    let neu: Vec<gio::File> =
                        outcome.arrived.iter().map(gio::File::for_path).collect();
                    if !neu.is_empty() {
                        ops.kontext.auswaehlen(&neu);
                    }
                }
            });
        Dienst::get().starten(auftrag);
    }

    /// A drop (`ziehen`): `ablage` of `quellen` onto `ziel`.
    pub fn ablegen(self: &Rc<Self>, ablage: Ablage, quellen: &[gio::File], ziel: &Ziel) {
        let quellen = quellen.to_vec();
        match (ablage, ziel) {
            (Ablage::Papierkorb, _) | (_, Ziel::Papierkorb) => self.in_papierkorb(quellen),
            (Ablage::Kopieren, Ziel::Ordner(o)) => {
                self.uebertragen(Op::Copy, quellen, o.clone(), false)
            }
            (Ablage::Verschieben, Ziel::Ordner(o)) => {
                self.uebertragen(Op::Move, quellen, o.clone(), false)
            }
            (Ablage::Verknuepfen, Ziel::Ordner(o)) => {
                Dienst::get().starten(self.auftrag(Op::Link, quellen, Some(o.clone())));
            }
        }
    }

    fn umbenennen_beginnen(self: &Rc<Self>, datei: &gio::File) {
        if !self.kontext.umbenennen_beginnen(datei) {
            self.umbenennen_dialog(datei);
        }
    }

    /// Umbenennen, executed: checks the name (GNOME 2's messages), then a
    /// job (atomic, never replaces). The view calls this after F2.
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
                self.fenster().as_ref(),
                &gettext_f("“{name}” cannot be renamed.", &[("name", &alt)]),
                &problem.text(neu),
            );
            return;
        }
        let weak = Rc::downgrade(self);
        let neu_name = neu.to_owned();
        let auftrag = self
            .auftrag(
                Op::Rename {
                    new: neu.to_owned(),
                },
                vec![datei.clone()],
                None,
            )
            .nachher(move |outcome, cancelled| {
                let Some(ops) = weak.upgrade() else { return };
                if let Some(neu) = outcome.arrived.first() {
                    ops.kontext.auswaehlen(&[gio::File::for_path(neu)]);
                } else if !cancelled && !outcome.failed.is_empty() {
                    fehler(
                        ops.fenster().as_ref(),
                        &gettext_f(
                            "“{name}” could not be renamed to “{new}”.",
                            &[("name", &alt), ("new", &neu_name)],
                        ),
                        &outcome
                            .failed
                            .iter()
                            .map(|(_, why)| why.as_str())
                            .collect::<Vec<_>>()
                            .join("\n"),
                    );
                }
            });
        Dienst::get().starten(auftrag);
    }

    /// When the view cannot rename in place: a small window with the name,
    /// the name without its extension selected (Nautilus 2.30).
    fn umbenennen_dialog(self: &Rc<Self>, datei: &gio::File) {
        let name = datei
            .basename()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let window = gtk4::Window::builder()
            .title(gettext("Rename"))
            .resizable(false)
            .modal(true)
            .build();
        window.set_transient_for(self.fenster().as_ref());
        window.add_css_class("querencia-dateien-rename");
        let outer = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
        outer.set_margin_top(12);
        outer.set_margin_bottom(12);
        outer.set_margin_start(12);
        outer.set_margin_end(12);
        let label = gtk4::Label::with_mnemonic(&gettext("_Name:"));
        label.set_xalign(0.0);
        let entry = gtk4::Entry::new();
        entry.set_text(&name);
        entry.set_width_chars(40);
        entry.set_activates_default(true);
        label.set_mnemonic_widget(Some(&entry));
        let buttons = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        buttons.set_halign(gtk4::Align::End);
        let cancel = gtk4::Button::with_mnemonic(&gettext("_Cancel"));
        let ok = gtk4::Button::with_mnemonic(&gettext("_Rename"));
        buttons.append(&cancel);
        buttons.append(&ok);
        outer.append(&label);
        outer.append(&entry);
        outer.append(&buttons);
        window.set_child(Some(&outer));
        window.set_default_widget(Some(&ok));
        let w = window.downgrade();
        cancel.connect_clicked(move |_| {
            if let Some(w) = w.upgrade() {
                w.destroy();
            }
        });
        let w = window.downgrade();
        let weak = Rc::downgrade(self);
        let datei_fuer_auswahl = datei.clone();
        let datei = datei.clone();
        let e = entry.clone();
        ok.connect_clicked(move |_| {
            let neu = e.text().to_string();
            if let Some(w) = w.upgrade() {
                w.destroy();
            }
            if let Some(ops) = weak.upgrade() {
                ops.umbenennen(&datei, &neu);
            }
        });
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
        window.present();
        entry.grab_focus();
        let is_file = !ist_ordner(&datei_fuer_auswahl);
        let stem = namen::selected_chars(&name, is_file);
        entry.select_region(0, i32::try_from(stem).unwrap_or(-1));
    }

    /// In den Papierkorb, without a question (E8, `confirm_trash` only for
    /// emptying); a place without trash asks (`Conflict::NoTrash`).
    pub fn in_papierkorb(self: &Rc<Self>, dateien: Vec<gio::File>) {
        if dateien.is_empty() {
            return;
        }
        Dienst::get().starten(self.auftrag(Op::Trash, dateien, None));
    }

    /// Umschalt+Entf: asks first (texts as the desktop), Abbrechen is the
    /// default and Escape cancels; only "Löschen" deletes.
    pub fn loeschen_fragen(self: &Rc<Self>, dateien: Vec<gio::File>) {
        if dateien.is_empty() {
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
            self.fenster().as_ref(),
            gio::Cancellable::NONE,
            move |answer| match answer {
                Ok(1) => {
                    if let Some(ops) = weak.upgrade() {
                        Dienst::get().starten(ops.auftrag(Op::Delete, dateien, None));
                    }
                }
                _ => protokoll("endgültig löschen abgebrochen"),
            },
        );
    }

    /// "Papierkorb leeren": asks first (texts as the desktop), Abbrechen is
    /// the default.
    pub fn leeren_fragen(self: &Rc<Self>) {
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
            self.fenster().as_ref(),
            gio::Cancellable::NONE,
            move |answer| match answer {
                Ok(1) => {
                    if let Some(ops) = weak.upgrade() {
                        Dienst::get().starten(ops.auftrag(Op::EmptyTrash, Vec::new(), None));
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
        Dienst::get().starten(self.auftrag(Op::Restore, dateien, None));
    }

    /// "Ordner anlegen": "Neuer Ordner" (a free name), selected, renaming.
    pub fn neuer_ordner(self: &Rc<Self>) {
        let ordner = self.kontext.ordner();
        self.neu_anlegen(
            Op::NewFolder {
                name: gettext("New Folder"),
            },
            ordner,
        );
    }

    /// "Dokument anlegen": `template` a path, empty for "Leere Datei".
    pub fn neues_dokument(self: &Rc<Self>, template: &str) {
        let op = if template.is_empty() {
            Op::NewFile {
                name: gettext("New Document"),
                template: None,
            }
        } else {
            let path = Path::new(template);
            Op::NewFile {
                name: vorlagen_name(path),
                template: Some(gio::File::for_path(path).uri().to_string()),
            }
        };
        let ordner = self.kontext.ordner();
        self.neu_anlegen(op, ordner);
    }

    fn neu_anlegen(self: &Rc<Self>, op: Op, ordner: gio::File) {
        let weak = Rc::downgrade(self);
        let auftrag = self
            .auftrag(op, Vec::new(), Some(ordner))
            .nachher(move |outcome, _| {
                let Some(ops) = weak.upgrade() else { return };
                if let Some(neu) = outcome.arrived.first() {
                    let neu = gio::File::for_path(neu);
                    ops.kontext.auswaehlen(std::slice::from_ref(&neu));
                    ops.umbenennen_beginnen(&neu);
                }
            });
        Dienst::get().starten(auftrag);
    }

    /// For the views: whether `uri` is cut and waits (shown faded).
    pub fn ist_ausgeschnitten(&self, uri: &str) -> bool {
        Zwischenablage::get().ist_ausgeschnitten(uri)
    }

    /// The trash's icon and count for menus and the sidebar.
    pub fn papierkorb(&self) -> &Papierkorb {
        &self.papierkorb
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn templates_come_sorted_without_hidden_ones() {
        let basis = std::env::current_dir().unwrap().join("target/tmp/vorlagen");
        let _ = std::fs::remove_dir_all(&basis);
        let dir = basis.join("Vorlagen");
        std::fs::create_dir_all(dir.join("Büro")).unwrap();
        std::fs::create_dir_all(dir.join("leer")).unwrap();
        for name in ["Text.txt", "Brief.odt", ".versteckt"] {
            std::fs::write(dir.join(name), b"").unwrap();
        }
        std::fs::write(dir.join("Büro/Rechnung.ods"), b"").unwrap();
        let groups = vorlagen(Some(&dir), &basis);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].0, "");
        assert_eq!(
            groups[0].1,
            vec![dir.join("Brief.odt"), dir.join("Text.txt")]
        );
        assert_eq!(
            groups[1],
            ("Büro".to_owned(), vec![dir.join("Büro/Rechnung.ods")])
        );
        // the home folder or no folder: no templates (Nautilus 2.30)
        assert!(vorlagen(Some(&basis), &basis).is_empty());
        assert!(vorlagen(None, &basis).is_empty());
        assert!(vorlagen(Some(&basis.join("fehlt")), &basis).is_empty());
        assert_eq!(vorlagen_name(&dir.join("Brief.odt")), "Brief.odt");
        let _ = std::fs::remove_dir_all(&basis);
    }

    #[test]
    fn view_keys_and_window_keys_do_not_overlap() {
        for (action, _) in KUERZEL_ANSICHT {
            assert!(!KUERZEL_FENSTER.iter().any(|(a, _)| a == action));
            assert!(AKTIONEN.contains(&action.trim_start_matches("win.")));
        }
        for (action, keys) in KUERZEL_FENSTER.iter().chain(KUERZEL_ANSICHT) {
            assert!(AKTIONEN.contains(&action.trim_start_matches("win.")));
            // GTK's accelerator syntax: modifiers in <>, then a key name
            for key in *keys {
                let name = key.rsplit('>').next().unwrap_or_default();
                assert!(!name.is_empty() && !name.contains(' '), "{key}");
            }
        }
    }
}
