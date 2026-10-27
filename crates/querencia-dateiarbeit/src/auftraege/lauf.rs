//! One job from start to end, in its own thread: count, then do each
//! source, asking where the rules say so (see the module above).

use std::time::Instant;

use gio::prelude::*;

use super::{Answer, Answered, Conflict, Event, FileFacts, Job, JobId, Op, Outcome};
use crate::atomar;
use crate::i18n::{gettext, gettext_f};
use crate::namen;
use crate::papierkorb::{self, wiederherstellen};

/// The job's link to the program: events out, answers in.
pub trait Steuerung {
    fn melden(&mut self, event: Event);
    /// Blocks until the program answers (or the job is cancelled).
    fn fragen(&mut self, conflict: Conflict) -> Answered;
}

/// Runs `job` to its end (blocking) and gives what it did and whether it
/// was cancelled (by `cancellable` or an answer). [`super::Queue`] calls
/// this in a thread of its own; tests and command-line tools may call it
/// directly.
pub fn ausfuehren(
    id: JobId,
    job: &Job,
    cancellable: &gio::Cancellable,
    steuerung: &mut dyn Steuerung,
) -> (Outcome, bool) {
    let mut lauf = Lauf {
        id,
        job,
        cancel: cancellable,
        ctl: steuerung,
        outcome: Outcome::default(),
        done_items: 0,
        done_bytes: 0,
        deep: false,
        begun: Instant::now(),
        last: None,
        current: String::new(),
        for_all: Vec::new(),
        serial: 0,
        stopped: false,
        no_undo: false,
    };
    lauf.run();
    let cancelled = lauf.stopped || cancellable.is_cancelled();
    (lauf.outcome, cancelled)
}

/// Why an object did not arrive.
enum Stop {
    Cancelled,
    Skipped,
    Failed(String),
    /// The whole object fails, not only a file inside it: the target is
    /// full, read-only or gone. Its partial copy goes.
    Whole(String),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Copy,
    Move,
}

const FLAGS: gio::FileCopyFlags =
    gio::FileCopyFlags::NOFOLLOW_SYMLINKS.union(gio::FileCopyFlags::ALL_METADATA);
const NOFOLLOW: gio::FileQueryInfoFlags = gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS;

fn file_type(file: &gio::File) -> gio::FileType {
    file.query_file_type(NOFOLLOW, gio::Cancellable::NONE)
}

fn exists(file: &gio::File) -> bool {
    file_type(file) != gio::FileType::Unknown
}

fn name_of(file: &gio::File) -> String {
    file.basename()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| file.uri().to_string())
}

/// Whether `dir` is `folder` or lies inside it.
fn inside(dir: &gio::File, folder: &gio::File) -> bool {
    dir.equal(folder) || dir.has_prefix(folder)
}

fn facts(file: &gio::File) -> FileFacts {
    let info = file
        .query_info(
            "standard::type,standard::size,time::modified",
            NOFOLLOW,
            gio::Cancellable::NONE,
        )
        .ok();
    FileFacts {
        uri: file.uri().to_string(),
        name: name_of(file),
        is_dir: info
            .as_ref()
            .is_some_and(|i| i.file_type() == gio::FileType::Directory),
        size: info
            .as_ref()
            .map_or(0, |i| u64::try_from(i.size()).unwrap_or(0)),
        modified: info.as_ref().and_then(|i| {
            i.has_attribute("time::modified")
                .then(|| i64::try_from(i.attribute_uint64("time::modified")).unwrap_or(0))
        }),
    }
}

fn filesystem_of(file: &gio::File) -> Option<String> {
    file.query_info("id::filesystem", NOFOLLOW, gio::Cancellable::NONE)
        .ok()
        .and_then(|info| info.attribute_string("id::filesystem"))
        .map(|id| id.to_string())
}

/// Why an operation failed, in the user's language first; GIO's own text
/// where nothing better is known.
pub(crate) fn reason(err: &glib::Error, op: &Op) -> String {
    match err.kind::<gio::IOErrorEnum>() {
        Some(gio::IOErrorEnum::PermissionDenied) => gettext("permission denied"),
        Some(gio::IOErrorEnum::NotFound) => gettext("no longer there"),
        Some(gio::IOErrorEnum::Exists) => {
            gettext("There is already an item with this name at the target.")
        }
        Some(gio::IOErrorEnum::NoSpace) => gettext("There is no space left at the target."),
        Some(gio::IOErrorEnum::ReadOnly) => gettext("The target is read-only."),
        Some(gio::IOErrorEnum::InvalidFilename) => {
            gettext("The name is not allowed at the target.")
        }
        Some(gio::IOErrorEnum::NotSupported) if *op == Op::Trash => {
            gettext("There is no trash at this location.")
        }
        Some(
            gio::IOErrorEnum::NotMounted
            | gio::IOErrorEnum::HostNotFound
            | gio::IOErrorEnum::HostUnreachable
            | gio::IOErrorEnum::TimedOut,
        ) => gettext("volume not reachable"),
        _ => err.message().to_owned(),
    }
}

/// Errors after which the rest of an object cannot go on (target full or
/// read-only, cancelled); others concern one file inside it.
fn fatal(err: &glib::Error) -> bool {
    matches!(
        err.kind::<gio::IOErrorEnum>(),
        Some(
            gio::IOErrorEnum::NoSpace
                | gio::IOErrorEnum::ReadOnly
                | gio::IOErrorEnum::Cancelled
                | gio::IOErrorEnum::NotMounted
        )
    )
}

struct Lauf<'a> {
    id: JobId,
    job: &'a Job,
    cancel: &'a gio::Cancellable,
    ctl: &'a mut dyn Steuerung,
    outcome: Outcome,
    done_items: u64,
    done_bytes: u64,
    /// Objects were counted deeply (copy, delete, a move across file
    /// systems): a renamed folder then counts with all it contains.
    deep: bool,
    begun: Instant,
    last: Option<Instant>,
    current: String,
    for_all: Vec<(u8, Answer)>,
    serial: u64,
    stopped: bool,
    /// The object in work cannot simply go back (see `Outcome::undo`).
    no_undo: bool,
}

impl Lauf<'_> {
    fn cancelled(&self) -> bool {
        self.stopped || self.cancel.is_cancelled()
    }

    fn progress(&mut self, force: bool) {
        let now = Instant::now();
        if !force
            && self
                .last
                .is_some_and(|last| now - last < self.job.fortschritt)
        {
            return;
        }
        self.last = Some(now);
        let seconds = (now - self.begun).as_secs_f64();
        #[allow(clippy::cast_precision_loss)]
        let bytes_per_s = if seconds > 0.0 {
            self.done_bytes as f64 / seconds
        } else {
            0.0
        };
        self.ctl.melden(Event::Progress {
            id: self.id,
            done_items: self.done_items,
            done_bytes: self.done_bytes,
            current: self.current.clone(),
            bytes_per_s,
        });
    }

    fn item_done(&mut self, bytes: u64) {
        self.done_items += 1;
        self.done_bytes += bytes;
        self.progress(false);
    }

    /// The answer to `conflict`: a remembered "für alle" answer, else the
    /// program's. A remembered rename means "a free name of its own"
    /// (`Rename("")`).
    fn ask(&mut self, conflict: Conflict) -> Answer {
        let kind = conflict.kind();
        if let Some((_, answer)) = self.for_all.iter().find(|(k, _)| *k == kind) {
            return answer.clone();
        }
        if self.cancelled() {
            return Answer::Cancel;
        }
        let answered = self.ctl.fragen(conflict);
        if answered.for_all {
            let remembered = match &answered.answer {
                Answer::Rename(_) => Answer::Rename(String::new()),
                other => other.clone(),
            };
            self.for_all.push((kind, remembered));
        }
        if answered.answer == Answer::Cancel {
            self.stopped = true;
        }
        answered.answer
    }

    fn failed(&mut self, name: String, why: String) {
        self.ctl.melden(Event::Failed {
            id: self.id,
            name: name.clone(),
            why: why.clone(),
        });
        self.outcome.failed.push((name, why));
    }

    // ------------------------------------------------------------ counting

    /// Objects and bytes in `file` (itself included), links not followed.
    fn count(&self, file: &gio::File) -> (u64, u64) {
        let Ok(info) =
            file.query_info("standard::type,standard::size", NOFOLLOW, Some(self.cancel))
        else {
            return (1, 0);
        };
        if info.file_type() != gio::FileType::Directory {
            return (1, u64::try_from(info.size()).unwrap_or(0));
        }
        let (mut items, mut bytes) = (1, 0);
        let Ok(children) = file.enumerate_children("standard::name", NOFOLLOW, Some(self.cancel))
        else {
            return (items, bytes);
        };
        while let Ok(Some(child)) = children.next_file(Some(self.cancel)) {
            let (i, b) = self.count(&file.child(child.name()));
            items += i;
            bytes += b;
        }
        (items, bytes)
    }

    fn start(&mut self, sources: &[gio::File], target: Option<&gio::File>) {
        let (mut items, mut bytes) = (Some(0u64), Some(0u64));
        match self.job.op {
            Op::Copy | Op::Delete => {
                self.deep = true;
                for source in sources {
                    let (i, b) = self.count(source);
                    items = items.map(|n| n + i);
                    bytes = bytes.map(|n| n + b);
                }
            }
            Op::Move => {
                let to = target.and_then(filesystem_of);
                // sources that are gone fail later; they say nothing here
                let across = sources
                    .iter()
                    .filter_map(filesystem_of)
                    .any(|fs| Some(&fs) != to.as_ref());
                if across {
                    self.deep = true;
                    for source in sources {
                        let (i, b) = self.count(source);
                        items = items.map(|n| n + i);
                        bytes = bytes.map(|n| n + b);
                    }
                } else {
                    items = Some(sources.len() as u64);
                    bytes = None;
                }
            }
            _ => {
                items = Some(sources.len() as u64);
                bytes = None;
            }
        }
        self.ctl.melden(Event::Started {
            id: self.id,
            op: self.job.op.clone(),
            total_items: items,
            total_bytes: bytes,
        });
    }

    // ------------------------------------------------------------- the run

    fn run(&mut self) {
        let job = self.job;
        let target = job.target.as_deref().map(gio::File::for_uri);
        let sources: Vec<gio::File> = match &job.op {
            Op::EmptyTrash => match papierkorb::contents() {
                Ok(list) => list,
                Err(err) => {
                    self.start(&[], None);
                    self.failed(gettext("Trash"), reason(&err, &job.op));
                    return;
                }
            },
            _ => job
                .sources
                .iter()
                .map(|uri| gio::File::for_uri(uri))
                .collect(),
        };
        self.start(&sources, target.as_ref());
        match &job.op {
            Op::NewFolder { name } | Op::NewFile { name, .. } => {
                let Some(dir) = target else {
                    self.failed(name.clone(), gettext("no target"));
                    return;
                };
                self.current = name.clone();
                let result = self.create(&dir, name);
                self.settle(name.clone(), result);
                self.progress(true);
                return;
            }
            Op::Rename { new } => {
                let Some(source) = sources.first() else {
                    return;
                };
                self.current = name_of(source);
                let result = self.rename(source, new);
                self.note_undo(source, &result);
                self.settle(name_of(source), result);
                self.progress(true);
                return;
            }
            _ => {}
        }
        for source in &sources {
            if self.cancelled() {
                break;
            }
            let name = name_of(source);
            self.current = name.clone();
            self.no_undo = false;
            let result = match (&job.op, &target) {
                (Op::Copy, Some(dir)) => self.deliver(source, dir, &name, Mode::Copy, true),
                (Op::Move, Some(dir)) => self.deliver(source, dir, &name, Mode::Move, true),
                (Op::Link, Some(dir)) => self.link(source, dir, &name),
                (Op::Trash, _) => self.trash(source),
                (Op::Delete | Op::EmptyTrash, _) => self.delete(source),
                (Op::Restore, _) => self.restore(source),
                (Op::Copy | Op::Move | Op::Link, None) => Err(Stop::Failed(gettext("no target"))),
                _ => Ok(None),
            };
            let cancelled = matches!(result, Err(Stop::Cancelled));
            self.note_undo(source, &result);
            self.settle(name, result);
            self.progress(true);
            if cancelled {
                break;
            }
        }
    }

    /// Notes `source` for undo when it is done and can simply go back.
    fn note_undo(&mut self, source: &gio::File, result: &Result<Option<String>, Stop>) {
        if let Ok(arrived) = result
            && !self.no_undo
        {
            self.outcome.undo.push((
                source.uri().to_string(),
                arrived.clone().unwrap_or_default(),
            ));
        }
    }

    fn settle(&mut self, name: String, result: Result<Option<String>, Stop>) {
        match result {
            Ok(arrived) => {
                self.outcome.done += 1;
                self.outcome.arrived.extend(arrived);
            }
            Err(Stop::Skipped) => self.outcome.skipped += 1,
            Err(Stop::Failed(why) | Stop::Whole(why)) => self.failed(name, why),
            Err(Stop::Cancelled) => {}
        }
    }

    // ------------------------------------------------------ copy and move

    /// Brings `source` into `dir` as `name` (copy or move), with the
    /// conflict rules. `top`: an object the user chose (a copy into its own
    /// folder becomes a duplicate with a free name).
    fn deliver(
        &mut self,
        source: &gio::File,
        dir: &gio::File,
        name: &str,
        mode: Mode,
        top: bool,
    ) -> Result<Option<String>, Stop> {
        let source_type = file_type(source);
        if source_type == gio::FileType::Unknown {
            return Err(Stop::Failed(gettext("no longer there")));
        }
        let source_dir = source_type == gio::FileType::Directory;
        if source_dir && inside(dir, source) {
            return Err(Stop::Failed(match mode {
                Mode::Copy => gettext("A folder cannot be copied into itself."),
                Mode::Move => gettext("A folder cannot be moved into itself."),
            }));
        }
        let same_parent = source.parent().is_some_and(|parent| parent.equal(dir));
        if same_parent && mode == Mode::Move {
            // a drop or paste into its own folder: nothing to move
            self.no_undo = true;
            self.item_done(0);
            return Ok(Some(name.to_owned()));
        }
        let mut name = if same_parent && top {
            namen::copy_name(name, source_dir, |n| exists(&dir.child(n)))
        } else {
            name.to_owned()
        };
        // a finished copy under its build name, waiting for its place
        let mut built: Option<gio::File> = None;
        loop {
            if self.cancelled() {
                self.discard(built.take());
                return Err(Stop::Cancelled);
            }
            let dest = dir.child(&name);
            let dest_type = file_type(&dest);
            if dest_type == gio::FileType::Unknown {
                if mode == Mode::Move && built.is_none() {
                    let counted = if self.deep {
                        self.count(source)
                    } else {
                        (1, 0)
                    };
                    match atomar::put_in_place(source, &dest) {
                        Ok(()) => {
                            self.done_items += counted.0;
                            self.done_bytes += counted.1;
                            self.progress(false);
                            return Ok(Some(name));
                        }
                        Err(err) if err.matches(gio::IOErrorEnum::Exists) => continue,
                        Err(err) if across(&err) => {}
                        Err(err) => return Err(Stop::Failed(reason(&err, &self.job.op))),
                    }
                }
                let temp = match built.take() {
                    Some(temp) => temp,
                    None => self.build(source, dir, &name)?,
                };
                match atomar::put_in_place(&temp, &dest) {
                    Ok(()) => {
                        if mode == Mode::Move {
                            self.remove_original(source)?;
                        }
                        return Ok(Some(name));
                    }
                    Err(err) if err.matches(gio::IOErrorEnum::Exists) => {
                        built = Some(temp);
                        continue;
                    }
                    Err(err) => {
                        self.discard(Some(temp));
                        return Err(Stop::Failed(reason(&err, &self.job.op)));
                    }
                }
            }
            let both_dirs = source_dir && dest_type == gio::FileType::Directory;
            let vorschlag = namen::copy_name(&name, source_dir, |n| exists(&dir.child(n)));
            let (source_facts, target_facts) = (facts(source), facts(&dest));
            let conflict = if both_dirs {
                Conflict::FolderMerge {
                    source: source_facts,
                    target: target_facts,
                    vorschlag: vorschlag.clone(),
                }
            } else {
                Conflict::Exists {
                    source: source_facts,
                    target: target_facts,
                    vorschlag: vorschlag.clone(),
                }
            };
            match self.ask(conflict) {
                Answer::Cancel => {
                    self.discard(built.take());
                    return Err(Stop::Cancelled);
                }
                Answer::Rename(new) => {
                    let new = if new.is_empty() { vorschlag } else { new };
                    if let Err(problem) = namen::check_file_name(&new) {
                        self.discard(built.take());
                        return Err(Stop::Failed(problem.text(&new)));
                    }
                    if new != name {
                        // a build made for the old name fits the new one
                        name = new;
                        // undo moves back into the folder: the name differs
                        if mode == Mode::Move {
                            self.no_undo = true;
                        }
                    }
                }
                Answer::Merge if both_dirs => {
                    self.no_undo = true;
                    self.discard(built.take());
                    self.merge(source, &dest, mode)?;
                    return Ok(Some(name));
                }
                Answer::Replace => {
                    self.no_undo = true;
                    return self.replace(source, dir, &dest, &name, mode, built.take());
                }
                // Skip, and answers that do not fit this question
                _ => {
                    self.discard(built.take());
                    return Err(Stop::Skipped);
                }
            }
        }
    }

    /// "Ersetzen": the new object is complete before it takes the place;
    /// the old one is deleted after the swap.
    fn replace(
        &mut self,
        source: &gio::File,
        dir: &gio::File,
        dest: &gio::File,
        name: &str,
        mode: Mode,
        built: Option<gio::File>,
    ) -> Result<Option<String>, Stop> {
        // the folder to replace holds the source: replacing would delete it
        if source.has_prefix(dest) {
            self.discard(built);
            return Err(Stop::Failed(gettext(
                "A folder cannot be replaced by an item it contains.",
            )));
        }
        // a move within a local file system: the object itself is swapped in
        if mode == Mode::Move
            && built.is_none()
            && let (Some(from), Some(to)) = (atomar::local_path(source), atomar::local_path(dest))
        {
            let counted = if self.deep {
                self.count(source)
            } else {
                (1, 0)
            };
            match atomar::rename_exchange(&from, &to) {
                Ok(()) => {
                    // the replaced object is at the source's old name now
                    if let Err(err) = self.delete_tree(source, false) {
                        crate::say!(
                            "Auftrag {}: ersetztes {} nicht gelöscht: {err}",
                            self.id,
                            from.display()
                        );
                    }
                    self.done_items += counted.0;
                    self.done_bytes += counted.1;
                    self.progress(false);
                    return Ok(Some(name.to_owned()));
                }
                Err(err) if err.raw_os_error() == Some(libc::EXDEV) => {}
                Err(err)
                    if matches!(err.raw_os_error(), Some(libc::EINVAL) | Some(libc::ENOSYS)) => {}
                Err(err) => {
                    return Err(Stop::Failed(reason(&atomar::gio_error(&err), &self.job.op)));
                }
            }
        }
        let temp = match built {
            Some(temp) => temp,
            None => self.build(source, dir, name)?,
        };
        self.serial += 1;
        let spare = dir.child(namen::build_name(name, self.id, self.serial));
        match atomar::swap_in(&temp, dest, &spare) {
            Ok(old) => {
                if let Err(err) = self.delete_tree(&old, false) {
                    crate::say!(
                        "Auftrag {}: ersetztes {} nicht gelöscht: {err}",
                        self.id,
                        old.uri()
                    );
                }
                if mode == Mode::Move {
                    self.remove_original(source)?;
                }
                Ok(Some(name.to_owned()))
            }
            Err(err) => {
                self.discard(Some(temp));
                Err(Stop::Failed(reason(&err, &self.job.op)))
            }
        }
    }

    /// "Zusammenführen": every object of `source` into `dest`, each with
    /// the conflict rules; a moved folder goes when it is empty.
    fn merge(&mut self, source: &gio::File, dest: &gio::File, mode: Mode) -> Result<(), Stop> {
        let children = source
            .enumerate_children("standard::name", NOFOLLOW, Some(self.cancel))
            .map_err(|err| Stop::Failed(reason(&err, &self.job.op)))?;
        let mut names = Vec::new();
        while let Some(info) = children
            .next_file(Some(self.cancel))
            .map_err(|err| self.stop_for(&err))?
        {
            names.push(info.name());
        }
        self.item_done(0);
        for child_name in names {
            let child = source.child(&child_name);
            let name = child_name.to_string_lossy().into_owned();
            self.current = name.clone();
            match self.deliver(&child, dest, &name, mode, false) {
                Ok(_) | Err(Stop::Skipped) => {}
                Err(Stop::Cancelled) => return Err(Stop::Cancelled),
                Err(Stop::Whole(why)) => return Err(Stop::Whole(why)),
                Err(Stop::Failed(why)) => {
                    let shown = format!("{}/{name}", name_of(source));
                    self.failed(shown, why);
                }
            }
        }
        if mode == Mode::Move
            && let Err(err) = source.delete(gio::Cancellable::NONE)
            && !err.matches(gio::IOErrorEnum::NotEmpty)
        {
            crate::say!("Auftrag {}: {} bleibt: {err}", self.id, source.uri());
        }
        Ok(())
    }

    fn stop_for(&self, err: &glib::Error) -> Stop {
        if err.matches(gio::IOErrorEnum::Cancelled) {
            Stop::Cancelled
        } else if fatal(err) {
            Stop::Whole(reason(err, &self.job.op))
        } else {
            Stop::Failed(reason(err, &self.job.op))
        }
    }

    /// Copies `source` to a build name in `dir`; the caller puts it in
    /// place. On cancel or failure the partial copy goes again.
    fn build(
        &mut self,
        source: &gio::File,
        dir: &gio::File,
        name: &str,
    ) -> Result<gio::File, Stop> {
        self.serial += 1;
        let temp = dir.child(namen::build_name(name, self.id, self.serial));
        match self.copy_tree(source, &temp, name) {
            Ok(()) => Ok(temp),
            Err(stop) => {
                self.discard(Some(temp));
                Err(stop)
            }
        }
    }

    /// Removes a partial or replaced object; what cannot go keeps its build
    /// name (the mark of a partial object) and is logged.
    fn discard(&mut self, file: Option<gio::File>) {
        let Some(file) = file else { return };
        if exists(&file)
            && let Err(err) = self.delete_tree(&file, false)
        {
            crate::say!(
                "Auftrag {}: Teilobjekt {} bleibt: {err}",
                self.id,
                file.uri()
            );
        }
    }

    /// After a move by copying: the original goes, all of it, without
    /// stopping for a cancel (the copy is complete already).
    fn remove_original(&mut self, source: &gio::File) -> Result<(), Stop> {
        self.delete_tree(source, false).map_err(|err| {
            Stop::Failed(gettext_f(
                "copied, but the original remained: {why}",
                &[("why", &reason(&err, &Op::Delete))],
            ))
        })
    }

    /// Copies `source` to `dest` (which must not exist); folders with all
    /// they contain, links as links. A file inside a folder that fails is
    /// reported (or asked about) and the rest goes on; a full or read-only
    /// target and a cancel stop the whole object.
    fn copy_tree(&mut self, source: &gio::File, dest: &gio::File, shown: &str) -> Result<(), Stop> {
        if self.cancelled() {
            return Err(Stop::Cancelled);
        }
        let info = source
            .query_info("standard::type,standard::size", NOFOLLOW, Some(self.cancel))
            .map_err(|err| self.stop_for(&err))?;
        if info.file_type() != gio::FileType::Directory {
            return self.copy_file(source, dest, u64::try_from(info.size()).unwrap_or(0));
        }
        dest.make_directory(Some(self.cancel))
            .map_err(|err| self.stop_for(&err))?;
        let children = source
            .enumerate_children("standard::name", NOFOLLOW, Some(self.cancel))
            .map_err(|err| self.stop_for(&err))?;
        self.item_done(0);
        loop {
            let child = match children.next_file(Some(self.cancel)) {
                Ok(Some(child)) => child,
                Ok(None) => break,
                Err(err) => return Err(self.stop_for(&err)),
            };
            let child_name = child.name();
            let child_shown = format!("{shown}/{}", child_name.to_string_lossy());
            self.current = child_shown.clone();
            loop {
                match self.copy_tree(
                    &source.child(&child_name),
                    &dest.child(&child_name),
                    &child_shown,
                ) {
                    Ok(()) => break,
                    Err(Stop::Failed(why)) => {
                        if !self.job.fehler_fragen {
                            self.failed(child_shown.clone(), why);
                            break;
                        }
                        let file = facts(&source.child(&child_name));
                        match self.ask(Conflict::Fehler { file, why }) {
                            Answer::Retry => {
                                let partial = dest.child(&child_name);
                                if exists(&partial) {
                                    let _ = self.delete_tree(&partial, false);
                                }
                            }
                            Answer::Cancel => return Err(Stop::Cancelled),
                            _ => {
                                self.outcome.skipped += 1;
                                break;
                            }
                        }
                    }
                    Err(Stop::Skipped) => break,
                    Err(Stop::Cancelled) => return Err(Stop::Cancelled),
                    Err(Stop::Whole(why)) => return Err(Stop::Whole(why)),
                }
            }
        }
        // permissions and times of the folder last, after its content
        let _ = source.copy_attributes(dest, FLAGS, gio::Cancellable::NONE);
        Ok(())
    }

    fn copy_file(&mut self, source: &gio::File, dest: &gio::File, size: u64) -> Result<(), Stop> {
        let base = self.done_bytes;
        let cancel = self.cancel;
        let mut flags = FLAGS;
        loop {
            let result = {
                let mut on_progress = |current: i64, _total: i64| {
                    self.done_bytes = base + u64::try_from(current).unwrap_or(0);
                    self.progress(false);
                };
                source.copy(dest, flags, Some(cancel), Some(&mut on_progress))
            };
            match result {
                Ok(()) => {
                    self.done_bytes = base;
                    self.item_done(size);
                    return Ok(());
                }
                // FAT, exFAT, SMB refuse owners and modes: copy without them
                Err(err)
                    if flags.contains(gio::FileCopyFlags::ALL_METADATA)
                        && matches!(
                            err.kind::<gio::IOErrorEnum>(),
                            Some(
                                gio::IOErrorEnum::NotSupported | gio::IOErrorEnum::PermissionDenied
                            )
                        )
                        && exists(dest) =>
                {
                    let _ = dest.delete(gio::Cancellable::NONE);
                    flags = gio::FileCopyFlags::NOFOLLOW_SYMLINKS;
                }
                Err(err) => {
                    self.done_bytes = base;
                    return Err(self.stop_for(&err));
                }
            }
        }
    }

    // ---------------------------------------------------- delete and trash

    /// Deletes `file`; a folder with all it contains; links are deleted,
    /// never followed. `counting`: progress per object and stop on cancel
    /// (the job "Delete"); otherwise it runs to the end.
    fn delete_tree(&mut self, file: &gio::File, counting: bool) -> Result<(), glib::Error> {
        let cancel = if counting { Some(self.cancel) } else { None };
        // GVfs deletes a top entry of trash:/// with all it contains, and
        // refuses single objects inside it
        let in_trash = file.uri_scheme().as_deref() == Some("trash");
        if !in_trash && file_type(file) == gio::FileType::Directory {
            let children = file.enumerate_children("standard::name", NOFOLLOW, cancel)?;
            while let Some(info) = children.next_file(cancel)? {
                self.delete_tree(&file.child(info.name()), counting)?;
            }
        }
        if counting && self.cancel.is_cancelled() {
            return Err(glib::Error::new(gio::IOErrorEnum::Cancelled, "abgebrochen"));
        }
        file.delete(cancel)?;
        if counting {
            self.item_done(0);
        }
        Ok(())
    }

    fn delete(&mut self, source: &gio::File) -> Result<Option<String>, Stop> {
        if !exists(source) {
            return Err(Stop::Failed(gettext("no longer there")));
        }
        loop {
            match self.delete_tree(source, true) {
                Ok(()) => {
                    papierkorb::forget_info(source);
                    return Ok(None);
                }
                Err(err) if err.matches(gio::IOErrorEnum::Cancelled) => {
                    return Err(Stop::Cancelled);
                }
                Err(err) => {
                    let why = reason(&err, &self.job.op);
                    if !self.job.fehler_fragen {
                        return Err(Stop::Failed(why));
                    }
                    match self.ask(Conflict::Fehler {
                        file: facts(source),
                        why,
                    }) {
                        Answer::Retry => continue,
                        Answer::Cancel => return Err(Stop::Cancelled),
                        _ => return Err(Stop::Skipped),
                    }
                }
            }
        }
    }

    fn trash(&mut self, source: &gio::File) -> Result<Option<String>, Stop> {
        loop {
            match source.trash(Some(self.cancel)) {
                Ok(()) => {
                    self.item_done(0);
                    return Ok(None);
                }
                Err(err) if err.matches(gio::IOErrorEnum::NotSupported) => {
                    match self.ask(Conflict::NoTrash {
                        file: facts(source),
                    }) {
                        Answer::Delete => {
                            self.no_undo = true;
                            return self
                                .delete_tree(source, true)
                                .map(|()| None)
                                .map_err(|err| self.stop_for(&err));
                        }
                        Answer::Cancel => return Err(Stop::Cancelled),
                        _ => return Err(Stop::Skipped),
                    }
                }
                Err(err) if err.matches(gio::IOErrorEnum::Cancelled) => {
                    return Err(Stop::Cancelled);
                }
                Err(err) => {
                    let why = reason(&err, &self.job.op);
                    if !self.job.fehler_fragen {
                        return Err(Stop::Failed(why));
                    }
                    match self.ask(Conflict::Fehler {
                        file: facts(source),
                        why,
                    }) {
                        Answer::Retry => continue,
                        Answer::Cancel => return Err(Stop::Cancelled),
                        _ => return Err(Stop::Skipped),
                    }
                }
            }
        }
    }

    fn restore(&mut self, source: &gio::File) -> Result<Option<String>, Stop> {
        let context = wiederherstellen::RestoreContext::current();
        let plan = wiederherstellen::plan_for_file(source, &context)
            .map_err(|grund| Stop::Failed(grund.text()))?;
        let mut name: Option<String> = None;
        loop {
            match wiederherstellen::execute(&plan, name.as_deref(), &context) {
                Ok(path) => {
                    self.item_done(0);
                    return Ok(Some(path.to_string_lossy().into_owned()));
                }
                Err(wiederherstellen::Grund::Exists) => {
                    let wanted = name.clone().unwrap_or_else(|| {
                        plan.destination
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default()
                    });
                    let dir = plan.destination.parent().map(gio::File::for_path);
                    let target = dir.as_ref().map(|d| d.child(&wanted));
                    let is_dir = file_type(&gio::File::for_path(&plan.source_path))
                        == gio::FileType::Directory;
                    let vorschlag = namen::copy_name(&wanted, is_dir, |n| {
                        dir.as_ref().is_some_and(|d| exists(&d.child(n)))
                    });
                    let conflict = Conflict::Exists {
                        source: facts(source),
                        target: target.as_ref().map(facts).unwrap_or_default(),
                        vorschlag: vorschlag.clone(),
                    };
                    match self.ask(conflict) {
                        Answer::Rename(new) => {
                            name = Some(if new.is_empty() { vorschlag } else { new });
                        }
                        Answer::Cancel => return Err(Stop::Cancelled),
                        // restoring never replaces (docs/dateiarbeit.md)
                        _ => return Err(Stop::Skipped),
                    }
                }
                Err(grund) => return Err(Stop::Failed(grund.text())),
            }
        }
    }

    // --------------------------------------------- links, new, rename

    fn link(
        &mut self,
        source: &gio::File,
        dir: &gio::File,
        name: &str,
    ) -> Result<Option<String>, Stop> {
        let points_to = source
            .path()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| source.uri().to_string());
        loop {
            if self.cancelled() {
                return Err(Stop::Cancelled);
            }
            let link = namen::link_name(name, |n| exists(&dir.child(n)));
            match dir
                .child(&link)
                .make_symbolic_link(&points_to, Some(self.cancel))
            {
                Ok(()) => {
                    self.item_done(0);
                    return Ok(Some(link));
                }
                Err(err) if err.matches(gio::IOErrorEnum::Exists) => continue,
                Err(err) => return Err(self.stop_for(&err)),
            }
        }
    }

    fn create(&mut self, dir: &gio::File, base: &str) -> Result<Option<String>, Stop> {
        if let Err(problem) = namen::check_file_name(base) {
            return Err(Stop::Failed(problem.text(base)));
        }
        loop {
            let name = namen::unique_name(base, |n| exists(&dir.child(n)));
            let dest = dir.child(&name);
            let result = match &self.job.op {
                Op::NewFolder { .. } => dest.make_directory(Some(self.cancel)),
                Op::NewFile {
                    template: Some(template),
                    ..
                } => {
                    let template = gio::File::for_uri(template);
                    let temp = self.build(&template, dir, &name)?;
                    match atomar::put_in_place(&temp, &dest) {
                        Ok(()) => Ok(()),
                        Err(err) => {
                            self.discard(Some(temp));
                            Err(err)
                        }
                    }
                }
                _ => dest
                    .create(gio::FileCreateFlags::NONE, Some(self.cancel))
                    .map(|_| ()),
            };
            match result {
                Ok(()) => {
                    self.item_done(0);
                    return Ok(Some(name));
                }
                Err(err) if err.matches(gio::IOErrorEnum::Exists) => continue,
                Err(err) => return Err(self.stop_for(&err)),
            }
        }
    }

    fn rename(&mut self, source: &gio::File, new: &str) -> Result<Option<String>, Stop> {
        if let Err(problem) = namen::check_file_name(new) {
            return Err(Stop::Failed(problem.text(new)));
        }
        if !exists(source) {
            return Err(Stop::Failed(gettext("no longer there")));
        }
        if name_of(source) == new {
            return Ok(Some(new.to_owned()));
        }
        let Some(parent) = source.parent() else {
            return Err(Stop::Failed(gettext(
                "The name is not allowed at the target.",
            )));
        };
        let result = match (atomar::local_path(source), atomar::local_path(&parent)) {
            (Some(from), Some(dir)) => atomar::rename_noreplace(&from, &dir.join(new))
                .map_err(|err| atomar::gio_error(&err)),
            _ => source.set_display_name(new, Some(self.cancel)).map(|_| ()),
        };
        match result {
            Ok(()) => {
                self.item_done(0);
                Ok(Some(new.to_owned()))
            }
            Err(err) => Err(self.stop_for(&err)),
        }
    }
}

/// A rename could not move the object (another file system, or GVfs
/// cannot): it is copied instead.
fn across(err: &glib::Error) -> bool {
    matches!(
        err.kind::<gio::IOErrorEnum>(),
        Some(gio::IOErrorEnum::WouldRecurse | gio::IOErrorEnum::NotSupported)
    )
}
