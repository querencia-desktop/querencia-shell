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
//!
//! 0.2.3: a change of the installed applications reads the theme again
//! only if one of their icons is unknown to it and was not already unknown
//! after the last reload ([`fehlende_symbole`]).
//! Every reload drops GTK's icon cache while images and icon loads in GTK's
//! thread pool still use it; on 22.10.2026 the live panel crashed in that
//! cache (a freed icon taken from it, `object_ref` on a finalized object)
//! minutes after three reloads that only came from integration scripts
//! rewriting the desktop files of programs whose icons were all known.

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use gtk4::prelude::*;
use gtk4::{IconTheme, gdk, gio, glib};

/// Wait this long after a change before reading the theme again (an
/// installer writes several files).
pub const DELAY: Duration = Duration::from_millis(400);
/// The reason of a reload after a change of the installed applications.
const ANWENDUNGEN: &str = "Anwendungen geändert";

/// The first names of `icons` (the names of each themed icon of the
/// installed applications) that `known` finds under none of their names,
/// sorted and without repeats; icons from files and applications without
/// an icon count as found.
pub fn fehlende_symbole(
    icons: impl IntoIterator<Item = Vec<String>>,
    known: impl Fn(&[String]) -> bool,
) -> BTreeSet<String> {
    icons
        .into_iter()
        .filter(|names| !names.is_empty() && !known(names))
        .map(|names| names[0].clone())
        .collect()
}

/// The names of each themed application icon.
fn programmsymbole() -> Vec<Vec<String>> {
    gio::AppInfo::all()
        .iter()
        .filter_map(|app| app.icon())
        .filter_map(|icon| {
            icon.downcast_ref::<gio::ThemedIcon>()
                .map(|t| t.names().iter().map(|n| n.to_string()).collect())
        })
        .collect()
}

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
    /// Application icons still unknown after the last reload (or at start).
    fehlend: RefCell<BTreeSet<String>>,
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
        fehlend: RefCell::new(BTreeSet::new()),
    });
    watch.fehlend.replace(watch.fehlende());
    watch.rewatch();
    let weak = Rc::downgrade(&watch);
    apps.connect_changed(move |_| {
        if let Some(watch) = weak.upgrade() {
            watch.schedule(ANWENDUNGEN);
        }
    });
    WATCH.with(|w| w.replace(Some(watch)));
}

impl Watch {
    /// [`fehlende_symbole`] of the installed applications in this theme.
    fn fehlende(&self) -> BTreeSet<String> {
        fehlende_symbole(programmsymbole(), |names| {
            names.iter().any(|name| self.theme.has_icon(name))
        })
    }

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
        if why == ANWENDUNGEN {
            let fehlend = self.fehlende();
            let bisher = self.fehlend.borrow().clone();
            let neu: Vec<&str> = fehlend.difference(&bisher).map(String::as_str).collect();
            if neu.is_empty() {
                eprintln!(
                    "querencia-panel: Symbolthema nicht neu eingelesen ({why}, kein neu fehlendes Programmsymbol)"
                );
                return;
            }
            eprintln!(
                "querencia-panel: Programmsymbole unbekannt: {}",
                neu.join(", ")
            );
        }
        let path = self.theme.search_path();
        let refs: Vec<&Path> = path.iter().map(PathBuf::as_path).collect();
        self.theme.set_search_path(&refs);
        eprintln!("querencia-panel: Symbolthema neu eingelesen ({why})");
        self.fehlend.replace(self.fehlende());
        // New theme or size folders get their monitors.
        self.rewatch();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_application_icons_by_first_name() {
        let known = |names: &[String]| names.iter().any(|n| n == "da" || n == "auch-da");
        let icons = |list: &[&[&str]]| {
            list.iter()
                .map(|names| names.iter().map(|n| n.to_string()).collect::<Vec<_>>())
                .collect::<Vec<_>>()
        };
        let fehlend = fehlende_symbole(
            icons(&[
                &["da"],
                &["neu-1"],
                &["weg", "auch-da"],
                &[],
                &["neu-1"],
                &["neu-0"],
            ]),
            known,
        );
        assert_eq!(fehlend.into_iter().collect::<Vec<_>>(), ["neu-0", "neu-1"]);
        assert!(fehlende_symbole(icons(&[&["da"], &[]]), known).is_empty());
    }

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
