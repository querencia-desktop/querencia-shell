//! The panel layout the running panel shows, together with the file it came
//! from. Every change goes through here: the change is made on a copy of the
//! layout, checked, saved, and only then becomes the current layout. So the
//! layout in memory and `panel.ini` never part ways, and a change that cannot
//! be saved is not made at all. No GTK; `panel_menu.rs` puts the widgets of
//! a change on screen.

use std::path::{Path, PathBuf};

use crate::layout::{Added, Layout, Region};
use crate::layout_store::{self, Disk, Source};

/// Looks up an applet type: `None` if unknown, otherwise whether it allows
/// several instances (see [`Layout::validated`]).
pub type AppletLookup = fn(&str) -> Option<bool>;

/// The applet types of the registry.
pub fn registry(id: &str) -> Option<bool> {
    crate::applets::find(id).map(|applet| applet.multiple)
}

pub struct Editor {
    layout: Layout,
    path: PathBuf,
    disk: Disk,
    /// The file held more than `layout` shows; the next save keeps a copy.
    lossy: bool,
    applet: AppletLookup,
}

/// A change that was made and saved.
#[derive(Debug)]
pub struct Change {
    pub added: Added,
    /// Copy of the file the save replaced, if one was made.
    pub backup: Option<PathBuf>,
}

impl Editor {
    /// Reads the layout from `path` (see [`layout_store::load_stored`]),
    /// with the problems found and where the layout came from.
    pub fn load(path: PathBuf, applet: AppletLookup) -> (Editor, Vec<String>, Source) {
        let loaded = layout_store::load_stored(&path, applet);
        let lossy = loaded.lossy();
        let editor = Editor {
            layout: loaded.layout,
            path,
            disk: loaded.disk,
            lossy,
            applet,
        };
        (editor, loaded.problems, loaded.source)
    }

    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Whether one more instance of `applet_id` may be added.
    pub fn can_add(&self, applet_id: &str) -> bool {
        self.layout.can_add(applet_id, self.applet)
    }

    /// Adds a new instance of `applet_id` to `region` of panel `panel_id`
    /// (see [`Layout::add_applet`]) and saves the layout. On any error,
    /// including a failed save, the current layout stays as it was.
    pub fn add(
        &mut self,
        panel_id: &str,
        applet_id: &str,
        region: Region,
    ) -> Result<Change, String> {
        let mut next = self.layout.clone();
        let added = next
            .add_applet(panel_id, applet_id, region, self.applet)
            .map_err(|e| e.to_string())?;
        // add_applet keeps the rules; check anyway before anything is saved.
        let checked = next.clone().validated(self.applet);
        if checked.fallback || !checked.problems.is_empty() {
            return Err(format!(
                "geändertes Layout ungültig, nichts gespeichert: {}",
                checked.problems.join("; ")
            ));
        }
        let saved = layout_store::save_checked(&next, &self.path, &self.disk, self.lossy)?;
        self.layout = next;
        self.disk = saved.disk;
        self.lossy = false;
        Ok(Change {
            added,
            backup: saved.backup,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::Instance;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(label: &str) -> Scratch {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock after 1970")
                .as_nanos();
            let dir = std::env::temp_dir().join(format!(
                "querencia-editor-{label}-{}-{nanos}",
                std::process::id()
            ));
            std::fs::create_dir_all(&dir).expect("scratch directory");
            Scratch(dir)
        }

        fn ini(&self) -> PathBuf {
            self.0.join("querencia").join("panel.ini")
        }

        fn names(&self, dir: &Path) -> Vec<String> {
            let mut names: Vec<_> = std::fs::read_dir(dir)
                .unwrap()
                .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .collect();
            names.sort();
            names
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// What a restarted panel would show.
    fn restart(path: &Path) -> Layout {
        let (editor, problems, source) = Editor::load(path.to_path_buf(), registry);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(source, Source::File);
        editor.layout().clone()
    }

    #[test]
    fn without_a_file_the_standard_layout_is_shown_and_nothing_written() {
        let scratch = Scratch::new("ohne");
        let (editor, problems, source) = Editor::load(scratch.ini(), registry);
        assert_eq!(editor.layout(), &Layout::standard());
        assert!(problems.is_empty());
        assert_eq!(source, Source::Standard);
        assert_eq!(editor.path(), scratch.ini());
        assert!(scratch.names(&scratch.0).is_empty());
    }

    #[test]
    fn adding_changes_the_layout_and_saves_it() {
        let scratch = Scratch::new("hinzu");
        let (mut editor, _, _) = Editor::load(scratch.ini(), registry);
        let change = editor.add("top", "clock", Region::Center).expect("add");
        assert_eq!(change.added.instance, Instance::new("clock-2", "clock"));
        assert_eq!(change.backup, None);

        let mut expected = Layout::standard();
        expected
            .add_applet("top", "clock", Region::Center, registry)
            .unwrap();
        assert_eq!(editor.layout(), &expected);
        assert_eq!(restart(&scratch.ini()), expected);
    }

    #[test]
    fn several_additions_survive_a_restart_in_order() {
        let scratch = Scratch::new("mehrere");
        let (mut editor, _, _) = Editor::load(scratch.ini(), registry);
        for (panel, applet, region) in [
            ("top", "places", Region::Start),
            ("bottom", "clock", Region::End),
            ("bottom", "clock", Region::End),
            ("top", "system", Region::Center),
        ] {
            editor.add(panel, applet, region).expect("add");
        }
        let layout = restart(&scratch.ini());
        assert_eq!(&layout, editor.layout());
        let bottom = &layout.panels[1];
        let end: Vec<_> = bottom.region(Region::End).iter().map(|i| &i.id).collect();
        // each new clock went first in the end region
        assert_eq!(end, ["clock-3", "clock-2", "workspace-switcher-1"]);
    }

    #[test]
    fn refused_additions_change_and_write_nothing() {
        let scratch = Scratch::new("abgelehnt");
        let (mut editor, _, _) = Editor::load(scratch.ini(), registry);
        for (panel, applet) in [
            ("top", "wetter"),
            ("links", "clock"),
            ("top", "window-list"),
            ("bottom", "workspace-switcher"),
        ] {
            let error = editor.add(panel, applet, Region::Start).unwrap_err();
            assert!(!error.is_empty());
        }
        assert_eq!(editor.layout(), &Layout::standard());
        assert!(!scratch.ini().exists());
    }

    #[test]
    fn can_add_reflects_the_current_layout() {
        let scratch = Scratch::new("verfuegbar");
        let path = scratch.ini();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            "[Layout]\nVersion=1\nPanels=top\n[Panel top]\nEdge=top\nStart=a-1\n\
             [Applet a-1]\nType=applications\n",
        )
        .unwrap();
        let (mut editor, problems, _) = Editor::load(path, registry);
        assert!(problems.is_empty(), "{problems:?}");
        assert!(editor.can_add("window-list"));
        assert!(!editor.can_add("wetter"));
        editor.add("top", "window-list", Region::Center).unwrap();
        assert!(!editor.can_add("window-list"));
        assert!(editor.can_add("clock"));
    }

    #[test]
    fn a_file_changed_by_someone_else_is_not_overwritten() {
        let scratch = Scratch::new("fremd");
        let (mut editor, _, _) = Editor::load(scratch.ini(), registry);
        editor.add("top", "clock", Region::End).unwrap();
        let mine = editor.layout().clone();

        std::fs::write(scratch.ini(), "[Layout]\nVersion=1\nPanels=bottom\n").unwrap();
        let error = editor.add("top", "places", Region::End).unwrap_err();
        assert!(error.contains("verändert"), "{error}");
        assert_eq!(
            editor.layout(),
            &mine,
            "a change that was not saved is not made"
        );
        assert_eq!(
            std::fs::read_to_string(scratch.ini()).unwrap(),
            "[Layout]\nVersion=1\nPanels=bottom\n"
        );
    }

    #[test]
    fn a_partly_read_file_is_kept_once_before_the_first_save() {
        let scratch = Scratch::new("teilweise");
        let path = scratch.ini();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let original = "[Layout]\nVersion=1\nPanels=top\n[Panel top]\nEdge=top\n\
                        Start=a-1;x-1\n[Applet a-1]\nType=applications\n\
                        [Applet x-1]\nType=wetter\n";
        std::fs::write(&path, original).unwrap();
        let (mut editor, problems, source) = Editor::load(path.clone(), registry);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert_eq!(source, Source::File);

        let first = editor.add("top", "clock", Region::End).unwrap();
        let backup = first.backup.expect("original kept");
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), original);
        let second = editor.add("top", "places", Region::End).unwrap();
        assert_eq!(second.backup, None, "the saved file is complete now");
        assert_eq!(scratch.names(path.parent().unwrap()).len(), 2);
        assert_eq!(restart(&path), *editor.layout());
    }

    #[test]
    fn an_unusable_file_is_kept_before_it_is_replaced() {
        let scratch = Scratch::new("unbrauchbar");
        let path = scratch.ini();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "kaputt [").unwrap();
        let (mut editor, problems, source) = Editor::load(path.clone(), registry);
        assert!(!problems.is_empty());
        assert_eq!(source, Source::Fallback);
        assert_eq!(editor.layout(), &Layout::standard());
        let change = editor.add("bottom", "clock", Region::Center).unwrap();
        let backup = change.backup.expect("original kept");
        assert_eq!(std::fs::read_to_string(backup).unwrap(), "kaputt [");
        assert_eq!(restart(&path), *editor.layout());
    }

    #[test]
    fn an_unreadable_file_is_never_replaced() {
        let scratch = Scratch::new("unlesbar");
        // a directory where the file should be cannot be read as a file
        let path = scratch.ini();
        std::fs::create_dir_all(&path).unwrap();
        let (mut editor, problems, source) = Editor::load(path.clone(), registry);
        assert!(!problems.is_empty());
        assert_eq!(source, Source::Fallback);
        let error = editor.add("top", "clock", Region::Start).unwrap_err();
        assert!(error.contains("nicht lesbar"), "{error}");
        assert!(path.is_dir());
        assert_eq!(editor.layout(), &Layout::standard());
    }
}
