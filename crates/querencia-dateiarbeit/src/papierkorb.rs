//! The trash: how many objects it holds (a monitor, no polling; QE-02),
//! its icon, the objects "Papierkorb leeren" deletes, and restoring
//! ([`wiederherstellen`], MIT from the Strata fork).
//!
//! With GVfs the trash is `trash:///` (all trash folders of all volumes).
//! Without it (`GIO_USE_VFS=local`, isolated tests) the home trash
//! `$XDG_DATA_HOME/Trash` stands in, so that counting, emptying and
//! restoring work there too. Emptying and deleting single entries run as
//! jobs ([`crate::auftraege`]), with progress and cancel.

pub mod mounts;
pub mod wiederherstellen;

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use gio::prelude::*;

pub const TRASH_URI: &str = "trash:///";

/// The home trash folder (`$XDG_DATA_HOME/Trash`).
pub fn home_trash() -> PathBuf {
    glib::user_data_dir().join("Trash")
}

/// Whether GIO can list `trash:///` (GVfs runs).
pub fn gvfs_trash() -> bool {
    gio::File::for_uri(TRASH_URI)
        .query_info(
            "trash::item-count",
            gio::FileQueryInfoFlags::NONE,
            gio::Cancellable::NONE,
        )
        .is_ok()
}

/// `trash::orig-path` of a `trash:///` entry: where it was deleted from.
pub fn orig_path(info: &gio::FileInfo) -> Option<PathBuf> {
    info.attribute_byte_string("trash::orig-path")
        .map(|p| PathBuf::from(p.as_str()))
        .filter(|p| !p.as_os_str().is_empty())
}

fn count_dir(dir: &Path) -> Option<u32> {
    let entries = std::fs::read_dir(dir).ok()?;
    Some(u32::try_from(entries.count()).unwrap_or(u32::MAX))
}

/// Objects in the trash, now (blocking; jobs and tests). `None` when it is
/// not reachable.
pub fn count_now() -> Option<u32> {
    match gio::File::for_uri(TRASH_URI).query_info(
        "trash::item-count",
        gio::FileQueryInfoFlags::NONE,
        gio::Cancellable::NONE,
    ) {
        Ok(info) => Some(info.attribute_uint32("trash::item-count")),
        Err(_) => count_dir(&home_trash().join("files")).or(Some(0)),
    }
}

/// What "Papierkorb leeren" deletes: every top object of `trash:///`, or
/// without GVfs every object of the home trash (its `.trashinfo` goes with
/// it, see `auftraege`).
pub fn contents() -> Result<Vec<gio::File>, glib::Error> {
    let trash = gio::File::for_uri(TRASH_URI);
    let (dir, local) = match trash.enumerate_children(
        "standard::name",
        gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
        gio::Cancellable::NONE,
    ) {
        Ok(children) => (Some(children), None),
        Err(_) => (None, Some(gio::File::for_path(home_trash().join("files")))),
    };
    let (children, parent) = match (dir, local) {
        (Some(children), _) => (children, trash),
        (None, Some(files)) => match files.enumerate_children(
            "standard::name",
            gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
            gio::Cancellable::NONE,
        ) {
            Ok(children) => (children, files),
            Err(err) if err.matches(gio::IOErrorEnum::NotFound) => return Ok(Vec::new()),
            Err(err) => return Err(err),
        },
        (None, None) => return Ok(Vec::new()),
    };
    let mut list = Vec::new();
    while let Some(info) = children.next_file(gio::Cancellable::NONE)? {
        list.push(parent.child(info.name()));
    }
    Ok(list)
}

/// After an object of a trash `files` folder was deleted for good: its
/// `.trashinfo` goes too (GVfs does this itself for `trash:///` entries).
pub fn forget_info(deleted: &gio::File) {
    if let Some(path) = crate::atomar::local_path(deleted)
        && let Some(info) = wiederherstellen::info_path_for_files_path(&path)
        && info.exists()
        && let Err(err) = std::fs::remove_file(&info)
    {
        crate::say!("Papierkorb: {} nicht entfernt: {err}", info.display());
    }
}

type Handler = Box<dyn Fn(Option<u32>)>;

struct Inner {
    count: Cell<Option<u32>>,
    handlers: RefCell<Vec<Handler>>,
    monitor: RefCell<Option<gio::FileMonitor>>,
    /// The newest `count_soon`; older ones end without counting.
    generation: Cell<u64>,
}

/// The trash state for an icon or a menu: count and icon, with a signal
/// when the count changes. Lives on the main thread; the count is asked
/// asynchronously after changes (entprellt, 120 ms), never polled.
#[derive(Clone)]
pub struct Papierkorb(Rc<Inner>);

impl Papierkorb {
    pub fn new() -> Self {
        let inner = Rc::new(Inner {
            count: Cell::new(None),
            handlers: RefCell::default(),
            monitor: RefCell::default(),
            generation: Cell::new(0),
        });
        let trash = Self(inner);
        trash.watch();
        trash.count_soon();
        trash
    }

    /// Objects in the trash, as last counted; `None` before the first count
    /// or when the trash is not reachable.
    pub fn anzahl(&self) -> Option<u32> {
        self.0.count.get()
    }

    /// `f` runs after every count that changed the number.
    pub fn connect_changed(&self, f: impl Fn(Option<u32>) + 'static) {
        self.0.handlers.borrow_mut().push(Box::new(f));
    }

    /// Empty or full, as GNOME 2 drew it.
    pub fn icon(&self) -> gio::Icon {
        crate::typ::trash_icon(self.anzahl().unwrap_or(0) > 0)
    }

    fn watch(&self) {
        // trash:/// with GVfs; without it GIO has no monitor for it, then
        // the home trash (no query on the main thread either way)
        let flags = gio::FileMonitorFlags::WATCH_MOVES;
        let trash = gio::File::for_uri(TRASH_URI);
        let local = gio::File::for_path(home_trash().join("files"));
        let (target, watched) = match trash.monitor_directory(flags, gio::Cancellable::NONE) {
            Ok(monitor) => (trash, Ok(monitor)),
            Err(_) => {
                let watched = local.monitor_directory(flags, gio::Cancellable::NONE);
                (local, watched)
            }
        };
        match watched {
            Ok(monitor) => {
                let weak = Rc::downgrade(&self.0);
                monitor.connect_changed(move |_, _, _, event| {
                    if matches!(event, gio::FileMonitorEvent::PreUnmount) {
                        return;
                    }
                    if let Some(inner) = weak.upgrade() {
                        Papierkorb(inner).count_soon();
                    }
                });
                *self.0.monitor.borrow_mut() = Some(monitor);
            }
            Err(err) => crate::say!("Papierkorb: keine Überwachung von {}: {err}", target.uri()),
        }
    }

    /// Counts shortly after monitor events (they come in bursts): only the
    /// last call of a burst counts, 120 ms after it. Runs on the
    /// thread-default main context (GTK's in programs, a test's own).
    pub fn count_soon(&self) {
        let generation = self.0.generation.get() + 1;
        self.0.generation.set(generation);
        let weak = Rc::downgrade(&self.0);
        glib::MainContext::ref_thread_default().spawn_local(async move {
            glib::timeout_future(Duration::from_millis(120)).await;
            if weak
                .upgrade()
                .is_none_or(|inner| inner.generation.get() != generation)
            {
                return;
            }
            let count = match gio::File::for_uri(TRASH_URI)
                .query_info_future(
                    "trash::item-count",
                    gio::FileQueryInfoFlags::NONE,
                    glib::Priority::DEFAULT,
                )
                .await
            {
                Ok(info) => Some(info.attribute_uint32("trash::item-count")),
                Err(_) => count_dir(&home_trash().join("files")).or(Some(0)),
            };
            if let Some(inner) = weak.upgrade()
                && inner.count.replace(count) != count
            {
                for handler in inner.handlers.borrow().iter() {
                    handler(count);
                }
            }
        });
    }
}

impl Default for Papierkorb {
    fn default() -> Self {
        Self::new()
    }
}
