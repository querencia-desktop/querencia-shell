//! Folder monitors for views: GIO's directory monitor with renames as one
//! event (`WATCH_MOVES`), changes bundled over a short time (they come in
//! bursts), and – as the desktop does for its folder (QD-112) – waiting for
//! a folder that does not exist yet in its nearest existing ancestor.
//!
//! From the desktop (`desktop_view.rs` `watch`, `rewatch_soon`;
//! `icons.rs` `waiting_point`, `concerns_desktop`).

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use gio::prelude::*;

/// How long changes are collected before handlers see them.
pub const BUNDLE: Duration = Duration::from_millis(120);
/// More changes than this in one bundle: handlers get [`Aenderung::NeuLaden`]
/// instead (a view reads the folder again; Strata's limit was 4096).
pub const MAX_CHANGES: usize = 4096;

/// What changed in the watched folder.
#[derive(Clone, Debug)]
pub enum Aenderung {
    Neu(gio::File),
    Weg(gio::File),
    /// Content or attributes (including "changes done").
    Geaendert(gio::File),
    /// Renamed inside the folder: old, new.
    Umbenannt(gio::File, gio::File),
    /// The folder itself appeared (after waiting for it).
    OrdnerDa,
    /// The folder itself went away (deleted, moved, unmounted); the watch
    /// now waits for it in its nearest existing ancestor.
    OrdnerWeg,
    /// Too many changes at once: read the whole folder again.
    NeuLaden,
}

/// QD-112: where to wait for a folder that does not exist yet: its nearest
/// existing ancestor. `None` when the folder exists (it is watched itself)
/// or nothing above it does.
pub fn waiting_point(dir: &Path, exists: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    if exists(dir) {
        return None;
    }
    dir.ancestors()
        .skip(1)
        .find(|a| exists(a))
        .map(Path::to_path_buf)
}

/// QD-112: whether a change of `path` (an entry of the watched ancestor)
/// may have made the folder `dir` appear or vanish: `path` is the folder
/// itself or a folder on the way to it.
pub fn concerns(dir: &Path, path: &Path) -> bool {
    dir.starts_with(path)
}

/// One monitor event as a change of the folder, or nothing (attribute and
/// pre-unmount events of the folder itself).
fn change_of(
    event: gio::FileMonitorEvent,
    file: &gio::File,
    other: Option<&gio::File>,
    dir: &gio::File,
) -> Option<Aenderung> {
    if file.equal(dir) {
        return match event {
            gio::FileMonitorEvent::Deleted
            | gio::FileMonitorEvent::MovedOut
            | gio::FileMonitorEvent::Unmounted => Some(Aenderung::OrdnerWeg),
            _ => None,
        };
    }
    match event {
        gio::FileMonitorEvent::Created | gio::FileMonitorEvent::MovedIn => {
            Some(Aenderung::Neu(file.clone()))
        }
        gio::FileMonitorEvent::Deleted | gio::FileMonitorEvent::MovedOut => {
            Some(Aenderung::Weg(file.clone()))
        }
        gio::FileMonitorEvent::Renamed => Some(match other {
            Some(new) => Aenderung::Umbenannt(file.clone(), new.clone()),
            None => Aenderung::Weg(file.clone()),
        }),
        gio::FileMonitorEvent::ChangesDoneHint | gio::FileMonitorEvent::AttributeChanged => {
            Some(Aenderung::Geaendert(file.clone()))
        }
        gio::FileMonitorEvent::Unmounted => Some(Aenderung::OrdnerWeg),
        _ => None,
    }
}

/// Folds a bundle: a file's last "changed" counts once; more than
/// [`MAX_CHANGES`]: one [`Aenderung::NeuLaden`].
fn fold(changes: Vec<Aenderung>) -> Vec<Aenderung> {
    if changes.len() > MAX_CHANGES {
        return vec![Aenderung::NeuLaden];
    }
    let mut out: Vec<Aenderung> = Vec::with_capacity(changes.len());
    for change in changes {
        if let Aenderung::Geaendert(file) = &change
            && out
                .iter()
                .any(|known| matches!(known, Aenderung::Geaendert(f) | Aenderung::Neu(f) if f.equal(file)))
        {
            continue;
        }
        out.push(change);
    }
    out
}

type Handler = Rc<dyn Fn(&[Aenderung])>;

struct Inner {
    dir: gio::File,
    path: Option<PathBuf>,
    warten: bool,
    monitors: RefCell<Vec<gio::FileMonitor>>,
    /// The monitor is on the folder itself (not on an ancestor).
    on_folder: Cell<bool>,
    pending: RefCell<Vec<Aenderung>>,
    /// A bundle is being collected (its delivery is on its way).
    bundling: Cell<bool>,
    handlers: RefCell<Vec<Handler>>,
}

/// A folder monitor for a view, on the thread's main context.
#[derive(Clone)]
pub struct FolderWatch(Rc<Inner>);

impl FolderWatch {
    /// Watches `dir`. With `warten`, a missing local folder is awaited in its
    /// nearest existing ancestor (and again after it went away).
    pub fn new(dir: &gio::File, warten: bool) -> Self {
        let watch = Self(Rc::new(Inner {
            dir: dir.clone(),
            path: crate::atomar::local_path(dir),
            warten,
            monitors: RefCell::default(),
            on_folder: Cell::new(false),
            pending: RefCell::default(),
            bundling: Cell::new(false),
            handlers: RefCell::default(),
        }));
        watch.arm();
        watch
    }

    /// `f` gets every bundle of changes.
    pub fn connect(&self, f: impl Fn(&[Aenderung]) + 'static) {
        self.0.handlers.borrow_mut().push(Rc::new(f));
    }

    /// Whether the folder is watched itself (not awaited).
    pub fn watches_folder(&self) -> bool {
        self.0.on_folder.get()
    }

    fn arm(&self) {
        let mut monitors = Vec::new();
        let waiting = match &self.0.path {
            Some(path) if self.0.warten => waiting_point(path, Path::is_dir),
            _ => None,
        };
        match &waiting {
            Some(point) => {
                let Ok(monitor) = gio::File::for_path(point)
                    .monitor_directory(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE)
                    .inspect_err(|err| {
                        crate::say!("Überwachung: {} nicht überwacht: {err}", point.display())
                    })
                else {
                    return;
                };
                crate::say!(
                    "Überwachung: {} fehlt, warte in {}",
                    self.0.dir.uri(),
                    point.display()
                );
                let weak = Rc::downgrade(&self.0);
                let dir = self.0.path.clone().unwrap_or_default();
                monitor.connect_changed(move |_, file, other, _| {
                    let touched = [Some(file), other]
                        .into_iter()
                        .flatten()
                        .filter_map(|f| f.path())
                        .any(|path| concerns(&dir, &path));
                    if touched && let Some(inner) = weak.upgrade() {
                        let watch = FolderWatch(inner);
                        if dir.is_dir() {
                            watch.rearm(Aenderung::OrdnerDa);
                        }
                    }
                });
                monitors.push(monitor);
            }
            None => match self
                .0
                .dir
                .monitor_directory(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE)
            {
                Ok(monitor) => {
                    let weak = Rc::downgrade(&self.0);
                    monitor.connect_changed(move |_, file, other, event| {
                        let Some(inner) = weak.upgrade() else { return };
                        let Some(change) = change_of(event, file, other, &inner.dir) else {
                            return;
                        };
                        let watch = FolderWatch(inner);
                        if matches!(change, Aenderung::OrdnerWeg) && watch.0.warten {
                            watch.rearm(Aenderung::OrdnerWeg);
                        } else {
                            watch.push(change);
                        }
                    });
                    monitors.push(monitor);
                }
                Err(err) => crate::say!("Überwachung: {} nicht überwacht: {err}", self.0.dir.uri()),
            },
        }
        self.0
            .on_folder
            .set(waiting.is_none() && !monitors.is_empty());
        let old = std::mem::replace(&mut *self.0.monitors.borrow_mut(), monitors);
        for monitor in old {
            monitor.cancel();
        }
    }

    /// The folder appeared or went away: new monitors, and the change. A
    /// second event of the same kind (they come in bursts) changes nothing.
    fn rearm(&self, change: Aenderung) {
        let appeared = matches!(change, Aenderung::OrdnerDa);
        if self.0.on_folder.get() == appeared {
            return;
        }
        self.arm();
        self.push(change);
    }

    fn push(&self, change: Aenderung) {
        self.0.pending.borrow_mut().push(change);
        if self.0.bundling.replace(true) {
            return;
        }
        let weak = Rc::downgrade(&self.0);
        glib::MainContext::ref_thread_default().spawn_local(async move {
            glib::timeout_future(BUNDLE).await;
            let Some(inner) = weak.upgrade() else { return };
            inner.bundling.set(false);
            let changes = fold(inner.pending.take());
            let handlers: Vec<_> = inner.handlers.borrow().clone();
            for handler in handlers {
                handler(&changes);
            }
        });
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        for monitor in self.monitors.get_mut().drain(..) {
            monitor.cancel();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_folder_is_awaited_in_its_parent() {
        let home = Path::new("/home/u");
        let dir = home.join("Schreibtisch");
        let only_home = |p: &Path| p == home || p == Path::new("/home") || p == Path::new("/");
        assert_eq!(waiting_point(&dir, only_home), Some(home.to_path_buf()));
        // two levels missing: the nearest existing ancestor
        let deep = home.join("a/b/Desktop");
        assert_eq!(waiting_point(&deep, only_home), Some(home.to_path_buf()));
        // an existing folder is watched itself
        assert_eq!(waiting_point(&dir, |_| true), None);
        // only events on the way to the folder count
        assert!(concerns(&dir, &dir));
        assert!(concerns(&deep, &home.join("a")));
        assert!(!concerns(&dir, &home.join(".bash_history")));
        assert!(!concerns(&dir, &home.join("Schreibtisch2")));
    }

    #[test]
    fn bundles_fold_repeated_changes_and_storms() {
        let a = gio::File::for_path("/x/a");
        let b = gio::File::for_path("/x/b");
        let folded = fold(vec![
            Aenderung::Neu(a.clone()),
            Aenderung::Geaendert(a.clone()),
            Aenderung::Geaendert(b.clone()),
            Aenderung::Geaendert(b.clone()),
        ]);
        assert_eq!(folded.len(), 2, "{folded:?}");
        let storm = (0..=MAX_CHANGES)
            .map(|_| Aenderung::Geaendert(a.clone()))
            .collect();
        assert!(matches!(fold(storm).as_slice(), [Aenderung::NeuLaden]));
        let dir = gio::File::for_path("/x");
        assert!(matches!(
            change_of(gio::FileMonitorEvent::Deleted, &dir, None, &dir),
            Some(Aenderung::OrdnerWeg)
        ));
        assert!(matches!(
            change_of(gio::FileMonitorEvent::Renamed, &a, Some(&b), &dir),
            Some(Aenderung::Umbenannt(_, _))
        ));
        assert!(change_of(gio::FileMonitorEvent::PreUnmount, &a, None, &dir).is_none());
    }
}
