//! Reads the icon theme again when icons of the user arrive (Teil 6, from
//! SW-6): GTK only notices new icon files when the modification time of a
//! theme's top folder changes. An icon put into an existing folder such as
//! `~/.local/share/icons/hicolor/scalable/apps` after the panel started
//! stayed invisible in the menus until `touch ~/.local/share/icons/hicolor`
//! or a panel restart.
//!
//! The panel now watches `~/.local/share/icons`, each theme folder in it
//! and their `<size>/<context>` folders (GIO file monitors, no polling),
//! and the installed applications (`GAppInfoMonitor`). [`DELAY`] after a
//! change (several changes count once) it hands GTK the same search path again
//! (`gtk_icon_theme_set_search_path`), which makes GTK read the themes anew;
//! the menus look their icons up again when they open.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use gtk4::prelude::*;
use gtk4::{IconTheme, gdk, gio, glib};

/// Wait this long after a change before reading the theme again (an
/// installer writes several files).
pub const DELAY: Duration = Duration::from_millis(400);

/// The folders to watch below `icons` (the user's icon folder): the folder
/// itself, every theme folder, and every `<theme>/<size>/<context>` folder.
pub fn folders(icons: &Path) -> Vec<PathBuf> {
    let mut list = vec![icons.to_path_buf()];
    let subdirs = |dir: &Path| -> Vec<PathBuf> {
        let mut dirs: Vec<PathBuf> = std::fs::read_dir(dir)
            .map(|entries| {
                entries
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| p.is_dir())
                    .collect()
            })
            .unwrap_or_default();
        dirs.sort();
        dirs
    };
    for theme in subdirs(icons) {
        list.push(theme.clone());
        for size in subdirs(&theme) {
            list.push(size.clone());
            list.extend(subdirs(&size));
        }
    }
    list
}

struct Watch {
    theme: IconTheme,
    _apps: gio::AppInfoMonitor,
    icons: PathBuf,
    monitors: RefCell<Vec<gio::FileMonitor>>,
    scheduled: Cell<bool>,
}

thread_local! {
    static WATCH: RefCell<Option<Rc<Watch>>> = const { RefCell::new(None) };
}

/// Starts watching for the display's icon theme (once per process).
pub fn start() {
    if WATCH.with(|w| w.borrow().is_some()) {
        return;
    }
    let Some(display) = gdk::Display::default() else {
        return;
    };
    let apps = gio::AppInfoMonitor::get();
    let watch = Rc::new(Watch {
        theme: IconTheme::for_display(&display),
        _apps: apps.clone(),
        icons: glib::user_data_dir().join("icons"),
        monitors: RefCell::new(Vec::new()),
        scheduled: Cell::new(false),
    });
    watch.rewatch();
    let weak = Rc::downgrade(&watch);
    apps.connect_changed(move |_| {
        if let Some(watch) = weak.upgrade() {
            watch.schedule("Anwendungen geändert");
        }
    });
    WATCH.with(|w| w.replace(Some(watch)));
}

impl Watch {
    /// Monitors every folder of [`folders`] (again: new folders appear).
    fn rewatch(self: &Rc<Self>) {
        let mut monitors = Vec::new();
        for folder in folders(&self.icons) {
            let file = gio::File::for_path(&folder);
            match file.monitor_directory(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE) {
                Ok(monitor) => {
                    let weak = Rc::downgrade(self);
                    monitor.connect_changed(move |_, file, _, event| {
                        if matches!(
                            event,
                            gio::FileMonitorEvent::Created
                                | gio::FileMonitorEvent::Deleted
                                | gio::FileMonitorEvent::ChangesDoneHint
                                | gio::FileMonitorEvent::MovedIn
                                | gio::FileMonitorEvent::MovedOut
                                | gio::FileMonitorEvent::Renamed
                        ) && let Some(watch) = weak.upgrade()
                        {
                            let name = file
                                .path()
                                .map(|p| p.display().to_string())
                                .unwrap_or_default();
                            watch.schedule(&format!("Symbol {name}"));
                        }
                    });
                    monitors.push(monitor);
                }
                Err(err) => eprintln!(
                    "querencia-panel: Symbolordner {} nicht überwacht: {err}",
                    folder.display()
                ),
            }
        }
        self.monitors.replace(monitors);
    }

    fn schedule(self: &Rc<Self>, why: &str) {
        if self.scheduled.replace(true) {
            return;
        }
        let weak = Rc::downgrade(self);
        let why = why.to_string();
        glib::timeout_add_local_once(DELAY, move || {
            if let Some(watch) = weak.upgrade() {
                watch.scheduled.set(false);
                watch.reload(&why);
            }
        });
    }

    fn reload(self: &Rc<Self>, why: &str) {
        let path = self.theme.search_path();
        let refs: Vec<&Path> = path.iter().map(PathBuf::as_path).collect();
        self.theme.set_search_path(&refs);
        eprintln!("querencia-panel: Symbolthema neu eingelesen ({why})");
        // New theme or size folders get their monitors.
        self.rewatch();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folders_are_the_icon_folder_themes_sizes_and_contexts() {
        let root = std::env::temp_dir().join(format!("querencia-symbole-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for dir in [
            "hicolor/scalable/apps",
            "hicolor/48x48/apps",
            "Eigen/16x16/actions",
        ] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        std::fs::write(root.join("hicolor/index.theme"), "").unwrap();
        let list = folders(&root);
        let rel: Vec<String> = list
            .iter()
            .map(|p| p.strip_prefix(&root).unwrap().display().to_string())
            .collect();
        assert_eq!(
            rel,
            [
                "",
                "Eigen",
                "Eigen/16x16",
                "Eigen/16x16/actions",
                "hicolor",
                "hicolor/48x48",
                "hicolor/48x48/apps",
                "hicolor/scalable",
                "hicolor/scalable/apps"
            ]
        );
        std::fs::remove_dir_all(&root).unwrap();
        assert_eq!(
            folders(&root),
            std::slice::from_ref(&root),
            "fehlender Ordner"
        );
    }
}
