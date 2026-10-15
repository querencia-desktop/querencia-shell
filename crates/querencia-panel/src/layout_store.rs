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
//! The panel writes the file only when the user changes the layout (M2.2c,
//! [`save_checked`]): never over a file that changed since it was read, and
//! after a copy of a file it could read only in part.
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
/// Only reads; a missing file or directory is never created. The panel uses
/// [`load_stored`], which also remembers what it read.
#[cfg(test)]
pub fn load(path: &Path, applet: impl Fn(&str) -> Option<bool>) -> (Layout, Vec<String>, Source) {
    let loaded = load_stored(path, applet);
    (loaded.layout, loaded.problems, loaded.source)
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
/// file. Blocks for the syncs; not for code that must not wait. Returns
/// the bytes written. The panel saves through [`save_checked`], which never
/// replaces a file changed since it was read.
pub fn save(layout: &Layout, path: &Path) -> Result<Vec<u8>, String> {
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
    .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(data.as_bytes().to_vec())
}

/// What `panel.ini` held when the panel last read or wrote it. Saving
/// compares it with the file on disk, so a change made meanwhile by someone
/// else is never overwritten.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Disk {
    /// There was no file.
    Missing,
    /// The file's exact bytes.
    Content(Vec<u8>),
    /// The file could not be read (message); it is never replaced.
    Unreadable(String),
}

/// Result of [`load_stored`].
#[derive(Debug)]
pub struct Loaded {
    pub layout: Layout,
    pub problems: Vec<String>,
    pub source: Source,
    pub disk: Disk,
    /// The file is exactly what saving `layout` would write.
    canonical: bool,
}

impl Loaded {
    /// Whether saving `layout` would lose something the file held: entries
    /// were dropped, the file was unusable, or it has comments, keys or a
    /// form the format does not keep. Saving then keeps a copy of the
    /// original file (see [`save_checked`]).
    pub fn lossy(&self) -> bool {
        self.source == Source::Fallback || !self.problems.is_empty() || !self.canonical
    }
}

/// Like [`load`], and remembers the bytes read, for [`save_checked`]. Reads
/// the file once, so layout and snapshot always belong together.
pub fn load_stored(path: &Path, applet: impl Fn(&str) -> Option<bool>) -> Loaded {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Loaded {
                layout: Layout::standard(),
                problems: Vec::new(),
                source: Source::Standard,
                disk: Disk::Missing,
                canonical: true,
            };
        }
        Err(err) => {
            return Loaded {
                layout: Layout::standard(),
                problems: vec![format!("{} nicht lesbar: {err}", path.display())],
                source: Source::Fallback,
                disk: Disk::Unreadable(err.to_string()),
                canonical: false,
            };
        }
    };
    let fallback = |problems| Loaded {
        layout: Layout::standard(),
        problems,
        source: Source::Fallback,
        disk: Disk::Content(bytes.clone()),
        canonical: false,
    };
    let file = glib::KeyFile::new();
    if let Err(err) = file.load_from_bytes(&glib::Bytes::from(&bytes), glib::KeyFileFlags::NONE) {
        return fallback(vec![format!("{} nicht lesbar: {err}", path.display())]);
    }
    let mut problems = Vec::new();
    let Some(layout) = parse(&file, &mut problems) else {
        return fallback(problems);
    };
    let validated = layout.validated(applet);
    problems.extend(validated.problems);
    let source = if validated.fallback {
        Source::Fallback
    } else {
        Source::File
    };
    let canonical = to_key_file(&validated.layout).to_data().as_bytes() == bytes.as_slice();
    Loaded {
        layout: validated.layout,
        problems,
        source,
        disk: Disk::Content(bytes),
        canonical,
    }
}

/// Result of [`save_checked`].
#[derive(Debug)]
pub struct Saved {
    /// What the file holds now; pass it to the next [`save_checked`].
    pub disk: Disk,
    /// Copy of the replaced file, if one was made.
    pub backup: Option<PathBuf>,
}

/// Saves `layout` like [`save`], but only if `path` still holds what
/// `expected` says, so a file changed or created by someone else since the
/// panel read it stays as it is. A file that could not be read is never
/// replaced. With `keep_original`, an existing file is first copied to
/// `panel.ini.vorher-<date>-<time>` next to it (never overwriting a file):
/// for a file the panel could only read in part, see [`Loaded::lossy`].
pub fn save_checked(
    layout: &Layout,
    path: &Path,
    expected: &Disk,
    keep_original: bool,
) -> Result<Saved, String> {
    if let Disk::Unreadable(err) = expected {
        return Err(format!(
            "{} war beim Laden nicht lesbar ({err}), nichts gespeichert",
            path.display()
        ));
    }
    let current = read_disk(path);
    if &current != expected {
        return Err(format!(
            "{} wurde seit dem Laden verändert, nichts gespeichert; \
             das Panel neu starten, um die Datei neu zu lesen",
            path.display()
        ));
    }
    let backup = match &current {
        Disk::Content(bytes) if keep_original => Some(write_backup(path, bytes)?),
        _ => None,
    };
    let written = save(layout, path)?;
    Ok(Saved {
        disk: Disk::Content(written),
        backup,
    })
}

fn read_disk(path: &Path) -> Disk {
    match std::fs::read(path) {
        Ok(bytes) => Disk::Content(bytes),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Disk::Missing,
        Err(err) => Disk::Unreadable(err.to_string()),
    }
}

/// Writes `bytes` to a new file `<path>.vorher-<date>-<time>[-<n>]` next to
/// `path`, readable by the owner only, and returns its path.
fn write_backup(path: &Path, bytes: &[u8]) -> Result<PathBuf, String> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    let stamp = glib::DateTime::now_local()
        .and_then(|now| now.format("%Y%m%d-%H%M%S"))
        .map(|s| s.to_string())
        .unwrap_or_else(|_| "unbekannt".to_string());
    let base = format!("{}.vorher-{stamp}", path.display());
    for n in 1..=99 {
        let candidate = PathBuf::from(if n == 1 {
            base.clone()
        } else {
            format!("{base}-{n}")
        });
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&candidate);
        match file {
            Ok(mut file) => {
                return file
                    .write_all(bytes)
                    .and_then(|()| file.sync_all())
                    .map(|()| candidate.clone())
                    .map_err(|e| format!("Sicherung {}: {e}", candidate.display()));
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(format!("Sicherung {}: {err}", candidate.display())),
        }
    }
    Err(format!("{base}: kein freier Name für die Sicherung"))
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

    fn with_clock(region: Region) -> Layout {
        let mut layout = Layout::standard();
        layout
            .add_applet("top", "clock", region, known)
            .expect("clock may be added");
        layout
    }

    #[test]
    fn load_stored_remembers_what_was_on_disk() {
        let scratch = Scratch::new("schnappschuss");
        let missing = scratch.0.join("panel.ini");
        let loaded = load_stored(&missing, known);
        assert_eq!(loaded.disk, Disk::Missing);
        assert_eq!(loaded.source, Source::Standard);
        assert!(!loaded.lossy());

        save(&Layout::standard(), &missing).unwrap();
        let bytes = std::fs::read(&missing).unwrap();
        let loaded = load_stored(&missing, known);
        assert_eq!(loaded.disk, Disk::Content(bytes));
        assert_eq!(loaded.source, Source::File);
        assert!(!loaded.lossy());

        // a directory in place of the file cannot be read
        let dir = scratch.0.join("verzeichnis");
        std::fs::create_dir(&dir).unwrap();
        let loaded = load_stored(&dir, known);
        assert!(
            matches!(loaded.disk, Disk::Unreadable(_)),
            "{:?}",
            loaded.disk
        );
        assert_eq!(loaded.source, Source::Fallback);
    }

    #[test]
    fn dropped_entries_or_fallback_make_a_load_lossy() {
        let scratch = Scratch::new("verlust");
        let partly = scratch.write(
            "teilweise",
            "[Layout]\nVersion=1\nPanels=top\n[Panel top]\nEdge=top\nStart=x-1\n\
             [Applet x-1]\nType=wetter\n",
        );
        let loaded = load_stored(&partly, known);
        assert_eq!(loaded.source, Source::File);
        assert!(loaded.lossy());

        let broken = scratch.write("kaputt", "kein [key file");
        let loaded = load_stored(&broken, known);
        assert_eq!(loaded.source, Source::Fallback);
        assert_eq!(loaded.disk, Disk::Content(b"kein [key file".to_vec()));
        assert!(loaded.lossy());
    }

    #[test]
    fn save_checked_writes_the_first_file_and_reports_its_content() {
        let scratch = Scratch::new("erstmals");
        let path = scratch.0.join("querencia").join("panel.ini");
        let layout = with_clock(Region::Center);
        let saved = save_checked(&layout, &path, &Disk::Missing, false).expect("save");
        assert_eq!(saved.backup, None);
        assert_eq!(saved.disk, Disk::Content(std::fs::read(&path).unwrap()));
        let (loaded, problems, source) = load(&path, known);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!((loaded, source), (layout, Source::File));
        assert_eq!(names(path.parent().unwrap()), ["panel.ini"]);
    }

    #[test]
    fn save_checked_chains_with_its_own_result() {
        let scratch = Scratch::new("kette");
        let path = scratch.0.join("panel.ini");
        let first = save_checked(&with_clock(Region::Start), &path, &Disk::Missing, false)
            .expect("first save");
        let mut second_layout = with_clock(Region::Start);
        second_layout
            .add_applet("bottom", "places", Region::End, known)
            .unwrap();
        save_checked(&second_layout, &path, &first.disk, false).expect("second save");
        assert_eq!(load(&path, known).0, second_layout);
    }

    #[test]
    fn save_checked_refuses_a_file_changed_since_loading() {
        let scratch = Scratch::new("fremd");
        let path = scratch.0.join("panel.ini");
        save(&Layout::standard(), &path).unwrap();
        let loaded = load_stored(&path, known);
        // someone else edits the file after the panel has read it
        std::fs::write(&path, "[Layout]\nVersion=1\nPanels=bottom\n").unwrap();
        let error = save_checked(&with_clock(Region::End), &path, &loaded.disk, false).unwrap_err();
        assert!(error.contains("seit dem Laden verändert"), "{error}");
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "[Layout]\nVersion=1\nPanels=bottom\n"
        );

        // a file that appeared after a load without one is not replaced either
        let fresh = scratch.0.join("neu.ini");
        std::fs::write(&fresh, "fremd").unwrap();
        assert!(save_checked(&Layout::standard(), &fresh, &Disk::Missing, false).is_err());
        assert_eq!(std::fs::read_to_string(&fresh).unwrap(), "fremd");
        assert_eq!(names(&scratch.0), ["neu.ini", "panel.ini"]);
    }

    #[test]
    fn save_checked_refuses_a_file_it_could_not_read() {
        let scratch = Scratch::new("unlesbar");
        let path = scratch.0.join("panel.ini");
        let error = save_checked(
            &Layout::standard(),
            &path,
            &Disk::Unreadable("Zugriff verweigert".into()),
            false,
        )
        .unwrap_err();
        assert!(error.contains("nicht lesbar"), "{error}");
        assert!(!path.exists());
    }

    #[test]
    fn save_checked_keeps_the_original_of_a_lossy_file() {
        let scratch = Scratch::new("sicherung");
        let original = "[Layout]\nVersion=1\nPanels=top\n[Panel top]\nEdge=top\n\
                        Start=a-1;x-1\n[Applet a-1]\nType=applications\n\
                        [Applet x-1]\nType=wetter\n";
        let path = scratch.write("panel.ini", original);
        let loaded = load_stored(&path, known);
        assert!(loaded.lossy());

        let mut layout = loaded.layout.clone();
        layout
            .add_applet("top", "clock", Region::End, known)
            .unwrap();
        let saved = save_checked(&layout, &path, &loaded.disk, loaded.lossy()).expect("save");
        let backup = saved.backup.expect("a backup of the original");
        assert_eq!(backup.parent(), path.parent());
        let name = backup.file_name().unwrap().to_string_lossy().into_owned();
        assert!(name.starts_with("panel.ini.vorher-"), "{name}");
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), original);
        assert_eq!(load(&path, known).0, layout);

        // a second backup in the same second gets its own name
        let again = save_checked(&layout, &path, &saved.disk, true).expect("save again");
        let second = again.backup.expect("second backup");
        assert_ne!(second, backup);
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), original);
        assert_eq!(names(&scratch.0).len(), 3, "{:?}", names(&scratch.0));
    }

    #[test]
    fn a_hand_edited_file_counts_as_lossy() {
        // comments and keys the format does not know would not survive a
        // save, so the first save keeps a copy
        let scratch = Scratch::new("handarbeit");
        save(&Layout::standard(), &scratch.0.join("panel.ini")).unwrap();
        let canonical = std::fs::read_to_string(scratch.0.join("panel.ini")).unwrap();
        let commented = scratch.write("kommentar.ini", &format!("# meine Panels\n{canonical}"));
        let extra = scratch.write(
            "extra.ini",
            &canonical.replace("[Applet clock-1]\n", "[Applet clock-1]\nFormat=%H:%M\n"),
        );
        for path in [&commented, &extra] {
            let loaded = load_stored(path, known);
            assert!(loaded.problems.is_empty(), "{:?}", loaded.problems);
            assert_eq!(loaded.layout, Layout::standard());
            assert!(loaded.lossy(), "{}", path.display());
        }
        assert!(!load_stored(&scratch.0.join("panel.ini"), known).lossy());
    }

    #[test]
    fn save_checked_makes_no_backup_without_a_file() {
        let scratch = Scratch::new("ohne-datei");
        let path = scratch.0.join("panel.ini");
        let saved = save_checked(&Layout::standard(), &path, &Disk::Missing, true).unwrap();
        assert_eq!(saved.backup, None);
        assert_eq!(names(&scratch.0), ["panel.ini"]);
    }

    #[test]
    fn split_ignores_empty_items_and_spaces() {
        assert_eq!(split("a;b;;c;"), ["a", "b", "c"]);
        assert_eq!(split(" a ; b "), ["a", "b"]);
        assert!(split("").is_empty());
    }
}
