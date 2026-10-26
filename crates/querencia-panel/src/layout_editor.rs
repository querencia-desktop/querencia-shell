//! The panel layout the running panel shows, together with the file it came
//! from. Every change goes through here: the change is made on a copy of the
//! layout, checked, saved, and only then becomes the current layout. So the
//! layout in memory and `panel.ini` never part ways, and a change that cannot
//! be saved is not made at all. No GTK; `panel_menu.rs` puts the widgets of
//! a change on screen.

use std::path::{Path, PathBuf};

use crate::layout::{AddError, Added, Instance, Layout, Region};
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

    /// What `panel.ini` held when it was last read or written (G2-AK:
    /// reloading compares it with the file on disk).
    pub fn disk(&self) -> &Disk {
        &self.disk
    }

    /// Whether one more instance of `applet_id` may be added: by the rules
    /// of the layout, and not a second one of a type offered only once
    /// ([`crate::applets::AppletType::once`], the clock) on any panel.
    pub fn can_add(&self, applet_id: &str) -> bool {
        self.layout.can_add(applet_id, self.applet) && !self.once_and_present(applet_id)
    }

    fn once_and_present(&self, applet_id: &str) -> bool {
        crate::applets::find(applet_id).is_some_and(|a| a.once) && self.layout.has_applet(applet_id)
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
        if self.once_and_present(applet_id) {
            return Err(AddError::SingleInstance(applet_id.to_string()).to_string());
        }
        let (added, backup) = self.change(|layout, applet| {
            layout
                .add_applet(panel_id, applet_id, region, applet)
                .map_err(|e| e.to_string())
        })?;
        Ok(Change { added, backup })
    }

    /// Adds a launcher for the application with desktop ID `app` to
    /// `region` of panel `panel_id` (see [`Layout::add_launcher`]) and saves
    /// the layout, like [`Editor::add`].
    pub fn add_launcher(
        &mut self,
        panel_id: &str,
        app: &str,
        region: Region,
    ) -> Result<Change, String> {
        let (added, backup) = self.change(|layout, applet| {
            layout
                .add_launcher(panel_id, app, region, applet)
                .map_err(|e| e.to_string())
        })?;
        Ok(Change { added, backup })
    }

    /// Removes the applet instance `instance_id` (see
    /// [`Layout::remove_applet`]) and saves the layout. On any error,
    /// including a failed save, the current layout stays as it was.
    pub fn remove(&mut self, instance_id: &str) -> Result<Removed, String> {
        let (instance, backup) = self.change(|layout, _| {
            layout
                .remove_applet(instance_id)
                .ok_or_else(|| format!("Applet „{instance_id}“ gibt es nicht"))
        })?;
        Ok(Removed { instance, backup })
    }

    /// Moves the applet instance `instance_id` within its panel (see
    /// [`Layout::move_applet`]) and saves the layout. `Ok(None)`: it is
    /// already there, nothing saved.
    pub fn move_applet(
        &mut self,
        instance_id: &str,
        how: crate::layout::Move,
    ) -> Result<Option<crate::layout::Moved>, String> {
        let Some(panel) = self
            .layout
            .panels
            .iter()
            .find(|p| p.position(instance_id).is_some())
        else {
            return Err(format!("Applet „{instance_id}“ gibt es nicht"));
        };
        let from = panel.position(instance_id).expect("found above");
        if panel.move_target(from, how).is_none() {
            return Ok(None);
        }
        let (moved, _) = self.change(|layout, _| {
            layout
                .move_applet(instance_id, how)
                .ok_or_else(|| format!("Applet „{instance_id}“ nicht verschoben"))
        })?;
        Ok(Some(moved))
    }

    /// The position of the applet instance `instance_id` in its panel.
    pub fn position(&self, instance_id: &str) -> Option<(Region, usize)> {
        self.layout
            .panels
            .iter()
            .find_map(|p| p.position(instance_id))
    }

    /// Where `how` would move `instance_id`, `None` if nowhere (it is at
    /// that edge, or unknown): for greying out menu entries.
    pub fn can_move(&self, instance_id: &str, how: crate::layout::Move) -> bool {
        self.layout.panels.iter().any(|p| {
            p.position(instance_id)
                .is_some_and(|from| p.move_target(from, how).is_some())
        })
    }

    /// Makes `edit` on a copy of the layout, checks and saves the copy, and
    /// only then makes it the current layout. Returns what `edit` gave and
    /// the copy of the file the save replaced, if one was made.
    fn change<T>(
        &mut self,
        edit: impl FnOnce(&mut Layout, AppletLookup) -> Result<T, String>,
    ) -> Result<(T, Option<PathBuf>), String> {
        let mut next = self.layout.clone();
        let result = edit(&mut next, self.applet)?;
        // The edits keep the rules; check anyway before anything is saved.
        let checked = next.clone().validated(self.applet);
        if checked.fallback || !checked.problems.is_empty() || checked.layout != next {
            return Err(format!(
                "geändertes Layout ungültig, nichts gespeichert: {}",
                checked.problems.join("; ")
            ));
        }
        let saved = layout_store::save_checked(&next, &self.path, &self.disk, self.lossy)?;
        self.layout = next;
        self.disk = saved.disk;
        self.lossy = false;
        Ok((result, saved.backup))
    }
}

/// An applet added and saved.
#[derive(Debug)]
pub struct Change {
    pub added: Added,
    /// Copy of the file the save replaced, if one was made.
    pub backup: Option<PathBuf>,
}

/// An applet removed and saved.
#[derive(Debug)]
pub struct Removed {
    pub instance: Instance,
    /// Copy of the file the save replaced, if one was made.
    pub backup: Option<PathBuf>,
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
        let change = editor.add("top", "places", Region::Center).expect("add");
        assert_eq!(change.added.instance, Instance::new("places-2", "places"));
        assert_eq!(change.backup, None);

        let mut expected = Layout::standard();
        expected
            .add_applet("top", "places", Region::Center, registry)
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
            ("bottom", "places", Region::End),
            ("bottom", "places", Region::End),
            ("top", "system", Region::Center),
        ] {
            editor.add(panel, applet, region).expect("add");
        }
        let layout = restart(&scratch.ini());
        assert_eq!(&layout, editor.layout());
        let bottom = &layout.panels[1];
        let end: Vec<_> = bottom.region(Region::End).iter().map(|i| &i.id).collect();
        // each new one went first in the end region
        assert_eq!(end, ["places-4", "places-3", "workspace-switcher-1"]);
    }

    #[test]
    fn moves_are_saved_and_come_back_after_a_restart() {
        use crate::layout::Move;
        let scratch = Scratch::new("verschieben");
        let (mut editor, _, _) = Editor::load(scratch.ini(), registry);
        assert!(editor.can_move("clock-1", Move::Left));
        assert!(!editor.can_move("clock-1", Move::Right), "rechter Rand");
        assert!(!editor.can_move("applications-1", Move::StartEdge));
        assert!(!editor.can_move("fehlt-1", Move::Left));
        // nothing to do: nothing saved
        assert!(
            editor
                .move_applet("clock-1", Move::EndEdge)
                .unwrap()
                .is_none()
        );
        assert!(!scratch.ini().exists(), "nichts geschrieben");
        // G2-AL and 0.4.0: first past the bell and the notification area,
        // then into the center
        let moved = editor.move_applet("clock-1", Move::Left).unwrap().unwrap();
        assert_eq!(moved.to, (Region::End, 1));
        let moved = editor.move_applet("clock-1", Move::Left).unwrap().unwrap();
        assert_eq!(moved.to, (Region::End, 0));
        let moved = editor.move_applet("clock-1", Move::Left).unwrap().unwrap();
        assert_eq!(moved.to, (Region::Center, 0));
        assert_eq!(editor.position("clock-1"), Some((Region::Center, 0)));
        editor.move_applet("places-1", Move::EndEdge).unwrap();
        editor
            .move_applet("window-list-1", Move::To(Region::End, 1))
            .unwrap();
        let layout = restart(&scratch.ini());
        assert_eq!(&layout, editor.layout());
        let top = &layout.panels[0];
        let ids =
            |region| -> Vec<&str> { top.region(region).iter().map(|i| i.id.as_str()).collect() };
        assert_eq!(ids(Region::Start), ["applications-1", "system-1"]);
        assert_eq!(ids(Region::Center), ["clock-1"]);
        assert_eq!(
            ids(Region::End),
            ["notification-area-1", "benachrichtigungen-1", "places-1"]
        );
        let bottom: Vec<&str> = layout.panels[1]
            .region(Region::End)
            .iter()
            .map(|i| i.id.as_str())
            .collect();
        assert_eq!(bottom, ["workspace-switcher-1", "window-list-1"]);
        assert!(editor.move_applet("fehlt-1", Move::Left).is_err());
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
        // this layout has no clock: one may be added, then no second one
        assert!(editor.can_add("clock"));
        editor.add("top", "clock", Region::End).unwrap();
        assert!(!editor.can_add("clock"));
    }

    #[test]
    fn a_file_changed_by_someone_else_is_not_overwritten() {
        let scratch = Scratch::new("fremd");
        let (mut editor, _, _) = Editor::load(scratch.ini(), registry);
        editor.add("top", "places", Region::End).unwrap();
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
        // (this layout had no clock)
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
        let change = editor.add("bottom", "places", Region::Center).unwrap();
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
        let error = editor.add("top", "places", Region::Start).unwrap_err();
        assert!(error.contains("nicht lesbar"), "{error}");
        assert!(path.is_dir());
        assert_eq!(editor.layout(), &Layout::standard());
    }

    // ---- HMP-11: application launchers ----

    #[test]
    fn launchers_are_saved_and_survive_a_restart() {
        let scratch = Scratch::new("starter");
        let (mut editor, _, _) = Editor::load(scratch.ini(), registry);
        let first = editor
            .add_launcher("top", "firefox_firefox.desktop", Region::Start)
            .expect("Firefox");
        let second = editor
            .add_launcher("top", "beispiel.desktop", Region::Start)
            .expect("Beispiel");
        assert_eq!(
            first.added.instance,
            Instance::launcher("launcher-1", "firefox_firefox.desktop")
        );
        assert_eq!(
            second.added.instance,
            Instance::launcher("launcher-2", "beispiel.desktop")
        );
        assert_eq!(second.added.index, 4, "hinter dem ersten Starter");
        assert_eq!(restart(&scratch.ini()), *editor.layout());
    }

    #[test]
    fn removing_a_launcher_is_saved_and_a_bad_one_changes_nothing() {
        let scratch = Scratch::new("entfernen");
        let (mut editor, _, _) = Editor::load(scratch.ini(), registry);
        editor
            .add_launcher("bottom", "a.desktop", Region::End)
            .unwrap();
        editor
            .add_launcher("bottom", "b.desktop", Region::End)
            .unwrap();
        let before = std::fs::read(scratch.ini()).unwrap();
        assert!(editor.remove("unbekannt-1").is_err());
        assert!(
            editor
                .add_launcher("bottom", "kein-starter", Region::End)
                .is_err()
        );
        assert_eq!(
            std::fs::read(scratch.ini()).unwrap(),
            before,
            "nichts geschrieben"
        );

        let removed = editor.remove("launcher-1").expect("remove");
        assert_eq!(
            removed.instance,
            Instance::launcher("launcher-1", "a.desktop")
        );
        let after = restart(&scratch.ini());
        let end: Vec<_> = after.panels[1]
            .region(Region::End)
            .iter()
            .map(|i| i.id.as_str())
            .collect();
        assert_eq!(end, ["launcher-2", "workspace-switcher-1"]);
    }

    // ---- QREL-04d: one clock ----

    #[test]
    fn the_clock_is_offered_once_on_all_panels_and_again_after_removal() {
        let scratch = Scratch::new("uhr");
        let (mut editor, _, _) = Editor::load(scratch.ini(), registry);
        // the standard layout has its clock on top: none for either panel
        assert!(!editor.can_add("clock"));
        for panel in ["top", "bottom"] {
            let error = editor.add(panel, "clock", Region::End).unwrap_err();
            assert!(error.contains("nur einmal"), "{error}");
        }
        assert!(!scratch.ini().exists(), "nothing written");
        // moving it to the bottom panel: remove on top, add at the bottom
        editor.remove("clock-1").unwrap();
        assert!(editor.can_add("clock"));
        let added = editor.add("bottom", "clock", Region::End).unwrap();
        assert_eq!(added.added.instance, Instance::new("clock-1", "clock"));
        assert!(!editor.can_add("clock"));
        // and after a new login the same
        let (again, problems, _) = Editor::load(scratch.ini(), registry);
        assert!(problems.is_empty(), "{problems:?}");
        assert!(!again.can_add("clock"));
        let bottom_end: Vec<_> = again.layout().panels[1]
            .region(Region::End)
            .iter()
            .map(|i| i.id.as_str())
            .collect();
        assert_eq!(bottom_end, ["clock-1", "workspace-switcher-1"]);
        // other types stay multiple
        assert!(again.can_add("places") && again.can_add(crate::layout::LAUNCHER));
    }

    #[test]
    fn a_stored_layout_with_two_clocks_keeps_both_untouched() {
        let scratch = Scratch::new("zwei-uhren");
        let path = scratch.ini();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut two = Layout::standard();
        two.add_applet("bottom", "clock", Region::End, registry)
            .unwrap();
        crate::layout_store::save(&two, &path).unwrap();
        let before = std::fs::read(&path).unwrap();
        let (editor, problems, _) = Editor::load(path.clone(), registry);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(editor.layout(), &two, "both clocks shown");
        assert!(!editor.can_add("clock"));
        assert_eq!(std::fs::read(&path).unwrap(), before, "file untouched");
    }
}
