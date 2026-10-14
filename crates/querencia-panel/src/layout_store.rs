//! Reads and writes the panel layout as a GLib key file,
//! `$XDG_CONFIG_HOME/querencia/panel.ini`:
//!
//! ```ini
//! [Layout]
//! Version=1
//! Panels=top;bottom
//!
//! [Panel top]
//! Edge=top
//! Height=32
//! Start=applications-1;places-1;system-1
//! Center=
//! End=clock-1
//!
//! [Applet applications-1]
//! Type=applications
//! ```
//!
//! Lists are `;`-separated; IDs never contain `;` (see
//! [`crate::layout::valid_id`]). Reading never fails: a missing file gives
//! the standard layout, an unreadable file or an unknown format version
//! falls back to it, and single bad entries are dropped with a message.
//! Writing replaces the target file atomically through
//! `g_file_set_contents_full` with GLib's `CONSISTENT` and `DURABLE`
//! guarantees: GLib writes a temporary file next to the target, syncs it,
//! renames it into place and syncs the directory holding it. A reader sees
//! either the old or the new file, never a partial one. These guarantees
//! cover the file only: if saving had to create the `querencia` directory
//! first, its entry in the parent directory is not synced and may be missing
//! after a crash. Querencia then starts with the standard layout, as if
//! nothing had been saved.

use std::fmt;
use std::path::{Path, PathBuf};

use gtk4::glib;

use crate::layout::{DEFAULT_HEIGHT, Edge, FORMAT_VERSION, Instance, Layout, PanelSpec, Region};

const LAYOUT_GROUP: &str = "Layout";

/// Where a loaded layout came from.
#[derive(Debug, PartialEq, Eq)]
pub enum Source {
    /// No `panel.ini`: the standard layout.
    Standard,
    /// Read from `panel.ini` (possibly with dropped entries).
    File,
    /// `panel.ini` was unusable: the standard layout instead.
    Fallback,
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Source::Standard => "Standardlayout (keine panel.ini)",
            Source::File => "aus panel.ini",
            Source::Fallback => "Standardlayout (panel.ini unbrauchbar)",
        })
    }
}

/// `$XDG_CONFIG_HOME/querencia/panel.ini`.
pub fn config_path() -> PathBuf {
    glib::user_config_dir().join("querencia").join("panel.ini")
}

/// The layout stored at `path`, validated with `applet` (see
/// [`Layout::validated`]), with every problem found and where it came from.
/// Only reads; a missing file or directory is never created.
pub fn load(path: &Path, applet: impl Fn(&str) -> Option<bool>) -> (Layout, Vec<String>, Source) {
    if !path.exists() {
        return (Layout::standard(), Vec::new(), Source::Standard);
    }
    let file = glib::KeyFile::new();
    if let Err(err) = file.load_from_file(path, glib::KeyFileFlags::NONE) {
        let problem = format!("{} nicht lesbar: {err}", path.display());
        return (Layout::standard(), vec![problem], Source::Fallback);
    }
    let mut problems = Vec::new();
    let Some(layout) = parse(&file, &mut problems) else {
        return (Layout::standard(), problems, Source::Fallback);
    };
    let validated = layout.validated(applet);
    problems.extend(validated.problems);
    let source = if validated.fallback {
        Source::Fallback
    } else {
        Source::File
    };
    (validated.layout, problems, source)
}

/// The layout described by `file`, or `None` if the file has no usable
/// `[Layout]` group (missing, wrong version, no panel list). Panels and
/// instances that cannot be read are skipped with a message in `problems`;
/// rules on IDs and applet types are left to [`Layout::validated`].
fn parse(file: &glib::KeyFile, problems: &mut Vec<String>) -> Option<Layout> {
    let version = match file.integer(LAYOUT_GROUP, "Version") {
        Ok(version) => version,
        Err(err) => {
            problems.push(format!(
                "[{LAYOUT_GROUP}] Version fehlt oder ungültig: {err}"
            ));
            return None;
        }
    };
    if version != FORMAT_VERSION {
        problems.push(format!(
            "Formatversion {version} nicht unterstützt (erwartet {FORMAT_VERSION})"
        ));
        return None;
    }
    let panel_ids = match file.string(LAYOUT_GROUP, "Panels") {
        Ok(list) => split(&list),
        Err(err) => {
            problems.push(format!("[{LAYOUT_GROUP}] Panels fehlt: {err}"));
            return None;
        }
    };

    let mut panels = Vec::new();
    for id in panel_ids {
        let group = format!("Panel {id}");
        let edge = match file.string(&group, "Edge") {
            Ok(text) => Edge::parse(&text),
            Err(_) => None,
        };
        let Some(edge) = edge else {
            problems.push(format!(
                "[{group}] Edge fehlt oder ungültig, Panel übersprungen"
            ));
            continue;
        };
        let height = if file.has_key(&group, "Height").unwrap_or(false) {
            file.integer(&group, "Height").unwrap_or_else(|_| {
                problems.push(format!(
                    "[{group}] Height ungültig, {DEFAULT_HEIGHT} verwendet"
                ));
                DEFAULT_HEIGHT
            })
        } else {
            DEFAULT_HEIGHT
        };
        let mut panel = PanelSpec::new(&id, edge, height);
        for (region, key) in [
            (Region::Start, "Start"),
            (Region::Center, "Center"),
            (Region::End, "End"),
        ] {
            let instance_ids = file.string(&group, key).map(|s| split(&s));
            for instance_id in instance_ids.unwrap_or_default() {
                match file.string(&format!("Applet {instance_id}"), "Type") {
                    Ok(applet) => panel.push(region, Instance::new(&instance_id, &applet)),
                    Err(_) => problems.push(format!(
                        "[Applet {instance_id}] Type fehlt, Applet übersprungen"
                    )),
                }
            }
        }
        panels.push(panel);
    }
    Some(Layout { panels })
}

/// `layout` as a key file in the format above.
fn to_key_file(layout: &Layout) -> glib::KeyFile {
    let file = glib::KeyFile::new();
    file.set_integer(LAYOUT_GROUP, "Version", FORMAT_VERSION);
    let panel_ids: Vec<&str> = layout.panels.iter().map(|p| p.id.as_str()).collect();
    file.set_string(LAYOUT_GROUP, "Panels", &panel_ids.join(";"));
    for panel in &layout.panels {
        let group = format!("Panel {}", panel.id);
        file.set_string(&group, "Edge", panel.edge.as_str());
        file.set_integer(&group, "Height", panel.height);
        for (region, key) in [
            (Region::Start, "Start"),
            (Region::Center, "Center"),
            (Region::End, "End"),
        ] {
            let ids: Vec<&str> = panel.region(region).iter().map(|i| i.id.as_str()).collect();
            file.set_string(&group, key, &ids.join(";"));
        }
        for instance in panel.instances() {
            file.set_string(&format!("Applet {}", instance.id), "Type", &instance.applet);
        }
    }
    file
}

/// Replaces `path` with `layout` atomically, with GLib's consistency and
/// durability guarantees for the file, creating the directory if needed. A
/// directory created here may be missing after a crash, and the next start
/// then uses the standard layout (see the module documentation). Only meant
/// for layouts that passed [`Layout::validated`].
///
/// `g_key_file_save_to_file` is not enough: it uses `g_file_set_contents`,
/// whose `ONLY_EXISTING` skips the sync for a file that does not exist yet,
/// so a crash during the first save could leave an empty or zero-filled
/// file. Blocks for the syncs; not for code that must not wait.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "first used when the panel can change its layout (M2.2c)"
    )
)]
pub fn save(layout: &Layout, path: &Path) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let data = to_key_file(layout).to_data();
    glib::file_set_contents_full(
        path,
        data.as_bytes(),
        glib::FileSetContentsFlags::CONSISTENT | glib::FileSetContentsFlags::DURABLE,
        // A new file gets 0666 minus the umask, like any other config file;
        // an existing file keeps its permissions.
        0o666,
    )
    .map_err(|e| format!("{}: {e}", path.display()))
}

fn split(list: &str) -> Vec<String> {
    list.split(';')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn known(id: &str) -> Option<bool> {
        crate::applets::find(id).map(|applet| applet.multiple)
    }

    /// A fresh directory below the system temp dir, removed on drop.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(label: &str) -> Scratch {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock after 1970")
                .as_nanos();
            let dir = std::env::temp_dir().join(format!(
                "querencia-layout-{label}-{}-{nanos}",
                std::process::id()
            ));
            std::fs::create_dir_all(&dir).expect("scratch directory");
            Scratch(dir)
        }

        fn write(&self, name: &str, text: &str) -> PathBuf {
            let path = self.0.join(name);
            std::fs::write(&path, text).expect("write test file");
            path
        }
    }

    /// Names in `dir`, sorted; shows leftover temporary files.
    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn config_path_is_below_the_user_config_dir() {
        let path = config_path();
        assert!(path.starts_with(glib::user_config_dir()));
        assert!(path.ends_with("querencia/panel.ini"));
    }

    #[test]
    fn missing_file_gives_the_standard_layout_and_creates_nothing() {
        let scratch = Scratch::new("fehlt");
        let path = scratch.0.join("querencia").join("panel.ini");
        let (layout, problems, source) = load(&path, known);
        assert_eq!(layout, Layout::standard());
        assert!(problems.is_empty());
        assert_eq!(source, Source::Standard);
        assert!(
            !path.parent().unwrap().exists(),
            "load must not create files"
        );
    }

    #[test]
    fn save_creates_a_new_file_and_load_round_trips() {
        let scratch = Scratch::new("rundreise");
        let path = scratch.0.join("querencia").join("panel.ini");
        assert!(!path.exists() && !path.parent().unwrap().exists());
        let mut layout = Layout::standard();
        layout.panels[1].push(Region::Center, Instance::new("clock-2", "clock"));
        save(&layout, &path).expect("save");

        let (loaded, problems, source) = load(&path, known);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(source, Source::File);
        assert_eq!(loaded, layout);

        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            text.contains("[Layout]\nVersion=1\nPanels=top;bottom\n"),
            "{text}"
        );
        assert!(
            text.contains("Start=applications-1;places-1;system-1\n"),
            "{text}"
        );
        assert!(text.contains("[Applet clock-2]\nType=clock\n"), "{text}");
        // the atomic write leaves no temporary file behind
        assert_eq!(names(path.parent().unwrap()), ["panel.ini"]);
    }

    #[test]
    fn save_replaces_a_valid_layout_and_keeps_its_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let scratch = Scratch::new("gueltig");
        let path = scratch.0.join("panel.ini");
        save(&Layout::standard(), &path).expect("first save");
        assert_eq!(load(&path, known).0, Layout::standard());
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();

        let mut changed = Layout::standard();
        changed.panels.truncate(1);
        changed.panels[0].push(Region::Center, Instance::new("clock-2", "clock"));
        save(&changed, &path).expect("second save");

        let (loaded, problems, source) = load(&path, known);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(source, Source::File);
        assert_eq!(loaded, changed);
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        assert_eq!(names(&scratch.0), ["panel.ini"]);
    }

    #[test]
    fn save_replaces_an_unusable_file() {
        let scratch = Scratch::new("ersetzen");
        let path = scratch.write("panel.ini", "alt");
        assert_eq!(load(&path, known).2, Source::Fallback);
        save(&Layout::standard(), &path).expect("save");
        let (loaded, problems, source) = load(&path, known);
        assert!(problems.is_empty());
        assert_eq!(source, Source::File);
        assert_eq!(loaded, Layout::standard());
    }

    #[test]
    fn write_errors_are_returned() {
        let scratch = Scratch::new("schreibfehler");

        // The directory cannot be created: its place is taken by a file.
        let blocker = scratch.write("querencia", "keine Ablage");
        let error = save(&Layout::standard(), &blocker.join("panel.ini")).unwrap_err();
        assert!(error.contains("querencia"), "{error}");
        assert_eq!(std::fs::read_to_string(&blocker).unwrap(), "keine Ablage");

        // GLib cannot rename over the target: it is a directory. Neither the
        // directory nor a temporary file may be left changed or behind.
        let target = scratch.0.join("panel.ini");
        std::fs::create_dir(&target).unwrap();
        let error = save(&Layout::standard(), &target).unwrap_err();
        assert!(error.contains("panel.ini"), "{error}");
        assert!(target.is_dir());
        assert!(names(&target).is_empty());
        assert_eq!(names(&scratch.0), ["panel.ini", "querencia"]);
    }

    #[test]
    fn unusable_files_fall_back_to_the_standard_layout() {
        let scratch = Scratch::new("kaputt");
        for (name, text) in [
            ("syntax", "das ist [keine key file\n=\n"),
            ("ohne-layout", "[Panel top]\nEdge=top\n"),
            (
                "version",
                "[Layout]\nVersion=2\nPanels=top\n[Panel top]\nEdge=top\n",
            ),
            ("version-text", "[Layout]\nVersion=eins\nPanels=top\n"),
            ("ohne-panels", "[Layout]\nVersion=1\n"),
            ("leere-panels", "[Layout]\nVersion=1\nPanels=\n"),
            (
                "nur-kaputte-panels",
                "[Layout]\nVersion=1\nPanels=top\n[Panel top]\nEdge=links\n",
            ),
        ] {
            let path = scratch.write(name, text);
            let (layout, problems, source) = load(&path, known);
            assert_eq!(layout, Layout::standard(), "{name}");
            assert_eq!(source, Source::Fallback, "{name}");
            assert!(!problems.is_empty(), "{name}");
        }
    }

    #[test]
    fn bad_entries_are_dropped_good_ones_kept() {
        let scratch = Scratch::new("teilweise");
        let path = scratch.write(
            "panel.ini",
            "[Layout]\nVersion=1\nPanels=top;rechts;bottom\n\
             [Panel top]\nEdge=top\nHeight=hoch\nStart=a-1;ohne-typ;fremd-1\nEnd=c-1;\n\
             [Panel rechts]\nEdge=right\n\
             [Panel bottom]\nEdge=bottom\nHeight=24\nCenter=w-1;w-2\n\
             [Applet a-1]\nType=applications\n\
             [Applet fremd-1]\nType=wetter\n\
             [Applet c-1]\nType=clock\n\
             [Applet w-1]\nType=window-list\n\
             [Applet w-2]\nType=window-list\n",
        );
        let (layout, problems, source) = load(&path, known);
        assert_eq!(source, Source::File);
        assert_eq!(layout.panels.len(), 2);
        let top = &layout.panels[0];
        assert_eq!(top.height, DEFAULT_HEIGHT);
        let start: Vec<_> = top.region(Region::Start).iter().map(|i| &i.id).collect();
        assert_eq!(start, ["a-1"]);
        assert_eq!(top.region(Region::End)[0].id, "c-1");
        let bottom = &layout.panels[1];
        assert_eq!(bottom.height, 24);
        let center: Vec<_> = bottom
            .region(Region::Center)
            .iter()
            .map(|i| &i.id)
            .collect();
        assert_eq!(center, ["w-1"]);
        // height, missing type, panel edge, unknown type, second window list
        assert_eq!(problems.len(), 5, "{problems:#?}");
    }

    #[test]
    fn split_ignores_empty_items_and_spaces() {
        assert_eq!(split("a;b;;c;"), ["a", "b", "c"]);
        assert_eq!(split(" a ; b "), ["a", "b"]);
        assert!(split("").is_empty());
    }
}
