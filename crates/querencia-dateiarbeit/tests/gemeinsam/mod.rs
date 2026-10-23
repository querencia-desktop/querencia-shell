//! Shared helpers of the integration tests: scratch trees under cargo's
//! target directory (on disk, never the user's files), a scripted program
//! side for jobs, and a runner for one job.

#![allow(dead_code)]

use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::Command;

use querencia_dateiarbeit::auftraege::{
    Answer, Answered, Conflict, Event, Job, Op, Outcome, Steuerung, ausfuehren,
};

/// A fresh, empty folder for one test.
pub fn scratch(tag: &str) -> PathBuf {
    let base = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("dateiarbeit")
        .join(format!("{tag}-{}", std::process::id()));
    if base.exists() {
        make_writable(&base);
        std::fs::remove_dir_all(&base).expect("alter Testordner");
    }
    std::fs::create_dir_all(&base).expect("Testordner");
    base.canonicalize().expect("kanonisch")
}

/// Gives back write permissions a test took away, so that clean-up works.
pub fn make_writable(dir: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let Ok(meta) = std::fs::symlink_metadata(dir) else {
        return;
    };
    if meta.is_dir() {
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o755));
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                make_writable(&entry.path());
            }
        }
    } else if !meta.file_type().is_symlink() {
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o644));
    }
}

pub fn remove(dir: &Path) {
    make_writable(dir);
    std::fs::remove_dir_all(dir).expect("aufräumen");
}

pub fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("Elternordner");
    }
    std::fs::write(path, text).expect("schreiben");
}

pub fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|err| panic!("{}: {err}", path.display()))
}

pub fn uri(path: &Path) -> String {
    querencia_dateiarbeit::uri::of_path(path)
}

/// Names in `dir`, sorted (hidden ones too).
pub fn names(dir: &Path) -> Vec<String> {
    let mut list: Vec<String> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    list.sort();
    list
}

/// Build names (partial objects) anywhere below `dir`.
pub fn partials(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if querencia_dateiarbeit::namen::is_build_name(&name) {
                found.push(entry.path());
            }
            if entry.file_type().is_ok_and(|t| t.is_dir()) {
                stack.push(entry.path());
            }
        }
    }
    found
}

/// The program side for a job run in the test thread: records every event
/// and answers questions from a script (in order); without a scripted
/// answer the test fails.
pub struct Skript {
    pub events: Vec<Event>,
    pub answers: VecDeque<Answered>,
    pub questions: Vec<Conflict>,
    /// Cancels the job once this many bytes are done (Abbruch mitten im
    /// Baum).
    pub cancel_after_bytes: Option<(u64, gio::Cancellable)>,
}

impl Skript {
    pub fn new(answers: impl IntoIterator<Item = Answered>) -> Self {
        Self {
            events: Vec::new(),
            answers: answers.into_iter().collect(),
            questions: Vec::new(),
            cancel_after_bytes: None,
        }
    }

    pub fn progress(&self) -> Vec<(u64, u64)> {
        self.events
            .iter()
            .filter_map(|e| match e {
                Event::Progress {
                    done_items,
                    done_bytes,
                    ..
                } => Some((*done_items, *done_bytes)),
                _ => None,
            })
            .collect()
    }

    pub fn started(&self) -> (Option<u64>, Option<u64>) {
        self.events
            .iter()
            .find_map(|e| match e {
                Event::Started {
                    total_items,
                    total_bytes,
                    ..
                } => Some((*total_items, *total_bytes)),
                _ => None,
            })
            .expect("Started")
    }
}

impl Steuerung for Skript {
    fn melden(&mut self, event: Event) {
        if let (Event::Progress { done_bytes, .. }, Some((limit, cancellable))) =
            (&event, &self.cancel_after_bytes)
            && *done_bytes >= *limit
        {
            use gio::prelude::*;
            cancellable.cancel();
        }
        self.events.push(event);
    }

    fn fragen(&mut self, conflict: Conflict) -> Answered {
        self.questions.push(conflict.clone());
        self.answers
            .pop_front()
            .unwrap_or_else(|| panic!("unerwartete Frage: {conflict:?}"))
    }
}

/// Runs a job in the test thread: (outcome, cancelled, the program side).
pub fn run(job: Job, answers: impl IntoIterator<Item = Answered>) -> (Outcome, bool, Skript) {
    let mut skript = Skript::new(answers);
    let cancellable = gio::Cancellable::new();
    let (outcome, cancelled) = ausfuehren(1, &job, &cancellable, &mut skript);
    assert!(
        skript.answers.is_empty(),
        "unbenutzte Antworten: {:?}",
        skript.answers
    );
    (outcome, cancelled, skript)
}

pub fn job(op: Op, sources: &[&Path], target: Option<&Path>) -> Job {
    Job::new(
        op,
        sources.iter().map(|p| uri(p)).collect(),
        target.map(uri),
    )
}

pub fn once(answer: Answer) -> Answered {
    Answered::once(answer)
}

pub fn all(answer: Answer) -> Answered {
    Answered::for_all(answer)
}

/// Iterates the thread-default main context until `done` or 20 s.
pub fn iterate_until(context: &glib::MainContext, done: impl Fn() -> bool) {
    let begun = std::time::Instant::now();
    while !done() {
        assert!(
            begun.elapsed() < std::time::Duration::from_secs(20),
            "Zeitüberschreitung beim Warten auf Ereignisse"
        );
        if !context.iteration(false) {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
}

thread_local! {
    pub static LOG: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// The child's environment: nothing of the live session.
pub fn isolated(command: &mut Command, base: &Path) {
    let home = base.join("home");
    let run = base.join("run");
    for dir in [&home, &run] {
        std::fs::create_dir_all(dir).expect("Ordner");
    }
    std::fs::set_permissions(&run, std::os::unix::fs::PermissionsExt::from_mode(0o700))
        .expect("0700");
    command
        .env_clear()
        .env("HOME", &home)
        .env("XDG_DATA_HOME", home.join(".local/share"))
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("XDG_CACHE_HOME", home.join(".cache"))
        .env("XDG_STATE_HOME", home.join(".local/state"))
        .env("XDG_RUNTIME_DIR", &run)
        .env("PATH", "/usr/bin:/bin")
        .env("LANG", "C.UTF-8")
        .env("GIO_USE_VFS", "local")
        .env("GIO_USE_VOLUME_MONITOR", "unix")
        .env("GSETTINGS_BACKEND", "memory")
        .env("DBUS_SESSION_BUS_ADDRESS", "disabled:");
}
