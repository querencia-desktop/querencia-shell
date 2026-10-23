//! File operations as jobs: copy, move, link, trash, delete, restore,
//! empty the trash, new folder and document, rename – each with progress,
//! cancel and questions (conflicts, missing trash, errors) back to the
//! program, as Nautilus 2.30's operations had them.
//!
//! A [`Queue`] per process runs every job in its own thread (never in GLib's
//! thread pool, which GTK uses for icons, QD-106) and hands the [`Event`]s
//! to its subscribers on the main context of the thread that made it. A
//! question ([`Event::Ask`]) carries a [`Reply`]; the job waits for the
//! answer (or for its cancel). The events contain no GTK or GIO objects, so
//! that a D-Bus service can carry them later (Recherche A §2.9, variant B).
//!
//! The rules (docs/dateiarbeit.md):
//! 1. Nothing is overwritten without an answer; "Überspringen" is the
//!    program's default button (E2).
//! 2. A new object is built under a hidden build name in the target folder
//!    and gets its name in one step that never replaces (`RENAME_NOREPLACE`,
//!    on GVfs GIO's move without `OVERWRITE`). A cancel or failure removes
//!    the partial object; should that fail too, its build name says what it
//!    is (`namen::is_build_name`). A partial object never has the real name.
//! 3. "Ersetzen" builds the new object completely first, then swaps it in
//!    (`RENAME_EXCHANGE`) and deletes the old one.
//! 4. A move across file systems copies the whole object first and deletes
//!    the original only afterwards; a cancel during the copy leaves the
//!    original untouched.
//! 5. Links are copied, moved and deleted as links, never followed; a
//!    folder never goes into itself.

mod lauf;

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::{Rc, Weak};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::task::{Poll, Waker};
use std::time::Duration;

pub use lauf::{Steuerung, ausfuehren};

use gio::prelude::*;

use crate::i18n::{gettext, gettext_f, ngettext_f};

pub type JobId = u64;

/// What a job does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Op {
    /// Copy `sources` into the folder `target`.
    Copy,
    /// Move `sources` into the folder `target`.
    Move,
    /// Symbolic links to `sources` in the folder `target`.
    Link,
    Trash,
    /// Delete for good (Umschalt+Entf, after the program's question; also
    /// single entries of the trash).
    Delete,
    /// Restore `sources` (entries of `trash:///` or of a trash `files`
    /// folder) to where they were deleted from.
    Restore,
    /// Delete everything in the trash (after the program's question).
    EmptyTrash,
    /// A new folder in `target`; a free name like "name 2" if taken.
    NewFolder {
        name: String,
    },
    /// A new document in `target`: empty, or a copy of the template (URI).
    NewFile {
        name: String,
        template: Option<String>,
    },
    /// Give the one source a new name in its folder.
    Rename {
        new: String,
    },
}

impl Op {
    /// For the log.
    pub fn verb(&self) -> &'static str {
        match self {
            Op::Copy => "kopieren",
            Op::Move => "verschieben",
            Op::Link => "verknüpfen",
            Op::Trash => "in den Papierkorb",
            Op::Delete => "endgültig löschen",
            Op::Restore => "wiederherstellen",
            Op::EmptyTrash => "Papierkorb leeren",
            Op::NewFolder { .. } => "neuer Ordner",
            Op::NewFile { .. } => "neues Dokument",
            Op::Rename { .. } => "umbenennen",
        }
    }

    /// The title of the error dialog.
    pub fn failure_title(&self) -> String {
        match self {
            Op::Copy => gettext("Error while copying."),
            Op::Move => gettext("Error while moving."),
            Op::Link => gettext("Error while creating links."),
            Op::Trash => gettext("Not everything could be moved to the trash."),
            Op::Delete => gettext("Not everything could be deleted."),
            Op::Restore => gettext("Not everything could be restored."),
            Op::EmptyTrash => gettext("The trash could not be emptied completely."),
            Op::NewFolder { .. } => gettext("The folder could not be created."),
            Op::NewFile { .. } => gettext("The document could not be created."),
            Op::Rename { .. } => gettext("The item could not be renamed."),
        }
    }
}

/// A job: what, which objects (URIs), where to (a folder URI).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Job {
    pub op: Op,
    pub sources: Vec<String>,
    pub target: Option<String>,
    /// Ask about each object that fails ("Überspringen / Wiederholen /
    /// Abbrechen") instead of reporting it and going on.
    pub fehler_fragen: bool,
    /// Progress at most this often ([`PROGRESS_EVERY`]; zero: every step).
    pub fortschritt: Duration,
}

impl Job {
    pub fn new(op: Op, sources: Vec<String>, target: Option<String>) -> Self {
        Self {
            op,
            sources,
            target,
            fehler_fragen: false,
            fortschritt: PROGRESS_EVERY,
        }
    }
}

/// What the program needs to know about one side of a conflict.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileFacts {
    pub uri: String,
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    /// Seconds since 1970, if known.
    pub modified: Option<i64>,
}

/// A question to the program.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Conflict {
    /// `target` exists where `source` should go (at least one is no
    /// folder): Ersetzen, Überspringen, Umbenennen, Abbrechen.
    Exists {
        source: FileFacts,
        target: FileFacts,
        /// A free name for "Umbenennen".
        vorschlag: String,
    },
    /// A folder onto a folder: Zusammenführen, Ersetzen, Überspringen,
    /// Umbenennen, Abbrechen.
    FolderMerge {
        source: FileFacts,
        target: FileFacts,
        vorschlag: String,
    },
    /// The place has no trash (network, some media): "Endgültig löschen?"
    /// (Löschen, Überspringen, Abbrechen; the default is Abbrechen).
    NoTrash { file: FileFacts },
    /// With [`Job::fehler_fragen`]: an object failed (Wiederholen,
    /// Überspringen, Abbrechen).
    Fehler { file: FileFacts, why: String },
}

impl Conflict {
    /// For "für alle": conflicts of the same kind get the same answer.
    fn kind(&self) -> u8 {
        match self {
            Conflict::Exists { .. } => 0,
            Conflict::FolderMerge { .. } => 1,
            Conflict::NoTrash { .. } => 2,
            Conflict::Fehler { .. } => 3,
        }
    }
}

/// An answer to a [`Conflict`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Answer {
    Replace,
    Skip,
    /// Use this name instead. "Für alle": every later conflict of the kind
    /// gets a free name of its own ([`crate::namen::copy_name`]).
    Rename(String),
    Merge,
    /// Delete for good ([`Conflict::NoTrash`]).
    Delete,
    Retry,
    Cancel,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Answered {
    pub answer: Answer,
    pub for_all: bool,
}

impl Answered {
    pub fn once(answer: Answer) -> Self {
        Self {
            answer,
            for_all: false,
        }
    }

    pub fn for_all(answer: Answer) -> Self {
        Self {
            answer,
            for_all: true,
        }
    }
}

/// Where the answer to an [`Event::Ask`] goes; the job waits for it. Can be
/// sent from any thread, once (later answers are ignored).
#[derive(Clone)]
pub struct Reply {
    job: JobId,
    frage: u64,
    tx: mpsc::Sender<Answered>,
}

impl Reply {
    pub fn send(&self, answered: Answered) {
        let _ = self.tx.send(answered);
    }

    pub fn job(&self) -> JobId {
        self.job
    }

    /// The number of the question within its job (1, 2, …).
    pub fn frage(&self) -> u64 {
        self.frage
    }
}

impl std::fmt::Debug for Reply {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Reply(job {}, frage {})", self.job, self.frage)
    }
}

/// What happened in a job.
#[derive(Clone, Debug)]
pub enum Event {
    /// The job counted what it will do (`None`: not counted, e.g. a move
    /// within one file system).
    Started {
        id: JobId,
        op: Op,
        total_items: Option<u64>,
        total_bytes: Option<u64>,
    },
    /// At most every [`Job::fortschritt`] and after each chosen object.
    Progress {
        id: JobId,
        done_items: u64,
        done_bytes: u64,
        current: String,
        bytes_per_s: f64,
    },
    Ask {
        id: JobId,
        conflict: Conflict,
        reply: Reply,
    },
    /// One object failed; the job goes on.
    Failed {
        id: JobId,
        name: String,
        why: String,
    },
    Finished {
        id: JobId,
        outcome: Outcome,
        cancelled: bool,
    },
}

impl Event {
    pub fn id(&self) -> JobId {
        match self {
            Event::Started { id, .. }
            | Event::Progress { id, .. }
            | Event::Ask { id, .. }
            | Event::Failed { id, .. }
            | Event::Finished { id, .. } => *id,
        }
    }
}

/// How often a job reports progress at most.
pub const PROGRESS_EVERY: Duration = Duration::from_millis(100);

/// What a job did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    /// The names in the target folder of what arrived there (copied, moved,
    /// linked, created, renamed, restored: the full path).
    pub arrived: Vec<String>,
    /// Objects done.
    pub done: usize,
    /// Objects skipped by an answer.
    pub skipped: usize,
    /// (name, why) for every object that failed.
    pub failed: Vec<(String, String)>,
}

/// The error dialog's text: up to five objects, then how many more.
pub fn failure_text(op: &Op, outcome: &Outcome) -> String {
    let total = outcome.done + outcome.skipped + outcome.failed.len();
    let failed = u32::try_from(outcome.failed.len()).unwrap_or(u32::MAX);
    let total_text = total.to_string();
    let args = [("total", total_text.as_str())];
    let mut text = if total == 1 {
        String::new()
    } else {
        let head = match op {
            Op::Copy => ngettext_f(
                "{n} of {total} items was not copied:",
                "{n} of {total} items were not copied:",
                failed,
                &args,
            ),
            Op::Move => ngettext_f(
                "{n} of {total} items was not moved:",
                "{n} of {total} items were not moved:",
                failed,
                &args,
            ),
            Op::Trash => ngettext_f(
                "{n} of {total} items was not moved to the trash:",
                "{n} of {total} items were not moved to the trash:",
                failed,
                &args,
            ),
            Op::Delete => ngettext_f(
                "{n} of {total} items was not deleted:",
                "{n} of {total} items were not deleted:",
                failed,
                &args,
            ),
            Op::EmptyTrash => ngettext_f(
                "{n} of {total} items could not be deleted and is still in the trash:",
                "{n} of {total} items could not be deleted and are still in the trash:",
                failed,
                &args,
            ),
            Op::Restore => ngettext_f(
                "{n} of {total} items was not restored:",
                "{n} of {total} items were not restored:",
                failed,
                &args,
            ),
            _ => ngettext_f(
                "{n} of {total} items failed:",
                "{n} of {total} items failed:",
                failed,
                &args,
            ),
        };
        format!("{head}\n")
    };
    let lines: Vec<String> = outcome
        .failed
        .iter()
        .take(5)
        // TRANSLATORS: one line of the error list: file name and reason
        .map(|(name, why)| gettext_f("“{name}”: {why}", &[("name", name), ("why", why)]))
        .collect();
    text.push_str(&lines.join("\n"));
    if outcome.failed.len() > 5 {
        let more = u32::try_from(outcome.failed.len() - 5).unwrap_or(u32::MAX);
        text.push('\n');
        text.push_str(&ngettext_f("… and {n} more", "… and {n} more", more, &[]));
    }
    text
}

/// Events from the job threads to the main context.
#[derive(Default)]
struct Kanal {
    events: Mutex<VecDeque<Event>>,
    waker: Mutex<Option<Waker>>,
    closed: std::sync::atomic::AtomicBool,
}

impl Kanal {
    fn push(&self, event: Event) {
        if let Ok(mut events) = self.events.lock() {
            events.push_back(event);
        }
        self.wake();
    }

    fn wake(&self) {
        if let Some(waker) = self.waker.lock().ok().and_then(|mut w| w.take()) {
            waker.wake();
        }
    }
}

/// The worker side of a queued job.
struct Bote {
    id: JobId,
    kanal: Arc<Kanal>,
    cancellable: gio::Cancellable,
    fragen: u64,
}

impl Steuerung for Bote {
    fn melden(&mut self, event: Event) {
        self.kanal.push(event);
    }

    fn fragen(&mut self, conflict: Conflict) -> Answered {
        self.fragen += 1;
        let (tx, rx) = mpsc::channel();
        self.kanal.push(Event::Ask {
            id: self.id,
            conflict,
            reply: Reply {
                job: self.id,
                frage: self.fragen,
                tx,
            },
        });
        loop {
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(answered) => return answered,
                Err(mpsc::RecvTimeoutError::Timeout) if !self.cancellable.is_cancelled() => {}
                Err(_) => return Answered::once(Answer::Cancel),
            }
        }
    }
}

type Subscriber = Rc<dyn Fn(&Event)>;

struct QueueInner {
    next_id: Cell<JobId>,
    next_abo: Cell<u64>,
    running: RefCell<Vec<(JobId, Op, gio::Cancellable)>>,
    subscribers: RefCell<Vec<(u64, Subscriber)>>,
    kanal: Arc<Kanal>,
}

impl Drop for QueueInner {
    fn drop(&mut self) {
        for (_, _, cancellable) in self.running.get_mut().iter() {
            cancellable.cancel();
        }
        self.kanal
            .closed
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.kanal.wake();
    }
}

/// A subscription of [`Queue::subscribe`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Abo(u64);

/// The job queue of a process: one thread per job, events on the main
/// context of the thread that made the queue. Dropping the queue cancels
/// its running jobs.
#[derive(Clone)]
pub struct Queue(Rc<QueueInner>);

impl Queue {
    /// A queue delivering on the thread-default main context (the GTK main
    /// context in programs; a context of their own in tests).
    pub fn new() -> Self {
        let queue = Self(Rc::new(QueueInner {
            next_id: Cell::new(1),
            next_abo: Cell::new(1),
            running: RefCell::default(),
            subscribers: RefCell::default(),
            kanal: Arc::default(),
        }));
        let weak = Rc::downgrade(&queue.0);
        let kanal = queue.0.kanal.clone();
        glib::MainContext::ref_thread_default().spawn_local(deliver(weak, kanal));
        queue
    }

    /// Starts `job` in its own thread; its events follow.
    pub fn start(&self, job: Job) -> JobId {
        let id = self.0.next_id.get();
        self.0.next_id.set(id + 1);
        let cancellable = gio::Cancellable::new();
        self.0
            .running
            .borrow_mut()
            .push((id, job.op.clone(), cancellable.clone()));
        let kanal = self.0.kanal.clone();
        let started = std::thread::Builder::new()
            .name(format!("dateiarbeit-{id}"))
            .spawn(move || {
                let mut bote = Bote {
                    id,
                    kanal: kanal.clone(),
                    cancellable: cancellable.clone(),
                    fragen: 0,
                };
                let (outcome, cancelled) = ausfuehren(id, &job, &cancellable, &mut bote);
                kanal.push(Event::Finished {
                    id,
                    outcome,
                    cancelled,
                });
            });
        if let Err(err) = started {
            crate::say!("Auftrag {id}: kein Faden: {err}");
            self.0.kanal.push(Event::Finished {
                id,
                outcome: Outcome {
                    failed: vec![(String::new(), err.to_string())],
                    ..Outcome::default()
                },
                cancelled: false,
            });
        }
        id
    }

    /// Cancels a running job: it stops after the current step, removes its
    /// partial object and reports `Finished { cancelled: true }`. A waiting
    /// question counts as answered with "Abbrechen".
    pub fn cancel(&self, id: JobId) {
        if let Some((_, _, cancellable)) = self.0.running.borrow().iter().find(|(j, _, _)| *j == id)
        {
            cancellable.cancel();
        }
    }

    /// `f` sees every event of every job, on the main context.
    pub fn subscribe(&self, f: impl Fn(&Event) + 'static) -> Abo {
        let abo = self.0.next_abo.get();
        self.0.next_abo.set(abo + 1);
        self.0.subscribers.borrow_mut().push((abo, Rc::new(f)));
        Abo(abo)
    }

    pub fn unsubscribe(&self, abo: Abo) {
        self.0.subscribers.borrow_mut().retain(|(a, _)| *a != abo.0);
    }

    /// The jobs that have not finished, oldest first.
    pub fn running(&self) -> Vec<(JobId, Op)> {
        self.0
            .running
            .borrow()
            .iter()
            .map(|(id, op, _)| (*id, op.clone()))
            .collect()
    }
}

impl Default for Queue {
    fn default() -> Self {
        Self::new()
    }
}

/// Hands the events of the job threads to the subscribers, on the main
/// context; ends with the queue.
async fn deliver(queue: Weak<QueueInner>, kanal: Arc<Kanal>) {
    loop {
        let batch: Vec<Event> = std::future::poll_fn(|cx| {
            let mut events = match kanal.events.lock() {
                Ok(events) => events,
                Err(_) => return Poll::Ready(Vec::new()),
            };
            if !events.is_empty() {
                return Poll::Ready(events.drain(..).collect());
            }
            if kanal.closed.load(std::sync::atomic::Ordering::SeqCst) {
                return Poll::Ready(Vec::new());
            }
            if let Ok(mut waker) = kanal.waker.lock() {
                *waker = Some(cx.waker().clone());
            }
            Poll::Pending
        })
        .await;
        let Some(inner) = queue.upgrade() else { return };
        if batch.is_empty() {
            return;
        }
        for event in batch {
            if let Event::Finished { id, .. } = &event {
                inner.running.borrow_mut().retain(|(j, _, _)| j != id);
            }
            let subscribers: Vec<Subscriber> = inner
                .subscribers
                .borrow()
                .iter()
                .map(|(_, f)| f.clone())
                .collect();
            for subscriber in subscribers {
                subscriber(&event);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_error_text_names_up_to_five_objects() {
        let text = failure_text(
            &Op::Move,
            &Outcome {
                done: 2,
                failed: vec![("a.txt".to_owned(), "x".to_owned())],
                ..Outcome::default()
            },
        );
        // without a catalog (tests): the English msgids
        assert_eq!(text, "1 of 3 items was not moved:\n“a.txt”: x");
        let many = Outcome {
            failed: (1..=7).map(|n| (format!("f{n}"), "y".to_owned())).collect(),
            ..Outcome::default()
        };
        let text = failure_text(&Op::Copy, &many);
        assert!(text.starts_with("7 of 7 items were not copied:\n"));
        assert!(text.contains("“f5”: y") && !text.contains("f6") && text.ends_with("… and 2 more"));
        let one = Outcome {
            failed: vec![("b".to_owned(), "z".to_owned())],
            ..Outcome::default()
        };
        assert_eq!(failure_text(&Op::Trash, &one), "“b”: z");
    }
}
