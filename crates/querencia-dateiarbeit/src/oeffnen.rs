//! What opening a file means, decided without a display; the programs show
//! the dialogs and start the programs (with their own launch context, which
//! carries the xdg-activation token under Wayland).
//!
//! From the desktop (`desktop_view.rs` `open`, `open_file`, `is_trusted`;
//! Desktop 0.2.2 and QD-102) and the panel's places menu: one list of the
//! programs that open folders, [`FILE_MANAGERS`].

use std::path::Path;

use gio::prelude::*;
use gio_unix::DesktopAppInfo;

use crate::typ;

/// The programs that open folders, Rechner, the trash and volumes: Querencia
/// Dateien first, then Querencia Files and its desktop ID up to QF-0s (kept
/// as a hidden bridge), so that whichever is installed opens them.
pub const FILE_MANAGERS: [&str; 3] = [
    "org.querencia.Dateien.desktop",
    "org.querencia.Files.desktop",
    "io.github.lgse.Strata.desktop",
];

/// The type GIO gives folders.
pub const FOLDER_TYPE: &str = "inode/directory";

/// The first of `ids` that `lookup` finds; without GIO, so that the order is
/// tested without installed starters.
pub fn first_found<T>(ids: &[&str], lookup: impl Fn(&str) -> Option<T>) -> Option<T> {
    ids.iter().find_map(|id| lookup(id))
}

/// The installed file manager of [`FILE_MANAGERS`], if any.
pub fn file_manager() -> Option<DesktopAppInfo> {
    first_found(&FILE_MANAGERS, DesktopAppInfo::new)
}

/// What to do when a file is opened.
#[derive(Clone, Debug)]
pub enum OpenPlan {
    /// The default program for the file's type.
    Default(gio::AppInfo),
    /// Desktop 0.2.2: no program for the file's type (an empty file is
    /// `application/x-zerosize`), but one for the type its name suggests.
    ByName(String, gio::AppInfo),
    /// QD-102: no program at all; the file's type. The program says so and
    /// offers "Open With Other Application…".
    NoProgram(String),
    /// A starter (`.desktop` file): start it when trusted, else ask
    /// (Nautilus 2.30's "Untrusted application launcher").
    Starter { trusted: bool },
    /// An executable program or script (x bit). Nautilus 2.30 asked "Run or
    /// display?" for text (`text`), and whether to run a binary.
    Executable { text: bool },
    /// A folder: the default program for folders, else the file manager;
    /// `None` when neither is installed.
    Folder(Option<gio::AppInfo>),
    /// A place that is no file (`computer:///`, `trash:///`, `network:///`,
    /// volumes of GVfs): the file manager opens it, `None` without one.
    Location(String, Option<gio::AppInfo>),
}

/// The attributes [`plan_open`] needs.
pub const OPEN_ATTRIBUTES: &str =
    "standard::type,standard::content-type,access::can-execute,unix::mode";

/// Places GIO knows that are no files: opened by the file manager.
const LOCATIONS: [&str; 5] = ["computer", "trash", "network", "recent", "burn"];

fn folder_app() -> Option<gio::AppInfo> {
    gio::AppInfo::default_for_type(FOLDER_TYPE, false)
        .or_else(|| file_manager().map(|app| app.upcast()))
}

/// Decides how `file` opens; `info` needs [`OPEN_ATTRIBUTES`] (queried
/// without following links: a link to a folder opens as the folder, GIO
/// resolves it on launch).
pub fn plan_open(file: &gio::File, info: &gio::FileInfo) -> OpenPlan {
    let scheme = file.uri_scheme().map(|s| s.to_string()).unwrap_or_default();
    if LOCATIONS.contains(&scheme.as_str()) {
        let app = file_manager().map(|app| app.upcast());
        return OpenPlan::Location(file.uri().to_string(), app);
    }
    if info.file_type() == gio::FileType::Directory || info.file_type() == gio::FileType::Mountable
    {
        return OpenPlan::Folder(folder_app());
    }
    let content_type = info
        .content_type()
        .map(|t| t.to_string())
        .unwrap_or_else(|| "application/octet-stream".to_owned());
    if content_type == typ::DESKTOP_ENTRY
        && let Some(path) = file.path()
    {
        return OpenPlan::Starter {
            trusted: is_trusted(&path),
        };
    }
    let executable = info.attribute_uint32("unix::mode") & 0o111 != 0
        && info.file_type() == gio::FileType::Regular;
    if executable && gio::content_type_can_be_executable(&content_type) {
        let text = gio::content_type_is_a(&content_type, "text/plain");
        return OpenPlan::Executable { text };
    }
    if let Some(app) = gio::AppInfo::default_for_type(&content_type, false) {
        return OpenPlan::Default(app);
    }
    let by_name = file
        .basename()
        .and_then(|name| typ::type_from_name(&name.to_string_lossy()));
    match by_name.and_then(|t| gio::AppInfo::default_for_type(&t, false).map(|app| (t, app))) {
        Some((t, app)) => OpenPlan::ByName(t, app),
        None => OpenPlan::NoProgram(content_type),
    }
}

/// The programs for "Öffnen mit": the default first, then the recommended,
/// then all others for the type, each once.
pub fn apps_for(content_type: &str) -> Vec<gio::AppInfo> {
    let mut list: Vec<gio::AppInfo> = Vec::new();
    let candidates = gio::AppInfo::default_for_type(content_type, false)
        .into_iter()
        .chain(gio::AppInfo::recommended_for_type(content_type))
        .chain(gio::AppInfo::all_for_type(content_type));
    for app in candidates {
        let id = app.id();
        if !list.iter().any(|known| known.id() == id && id.is_some()) {
            list.push(app);
        }
    }
    list
}

/// A starter runs only when it is executable for its owner, as Nautilus 2.30
/// required ("vertrauenswürdig"), or when it is a link to an installed
/// starter (`…/applications` of the XDG data directories).
pub fn is_trusted(path: &Path) -> bool {
    let executable = std::fs::metadata(path)
        .is_ok_and(|m| std::os::unix::fs::PermissionsExt::mode(&m.permissions()) & 0o100 != 0);
    let installed = std::fs::canonicalize(path).is_ok_and(|real| {
        std::iter::once(glib::user_data_dir())
            .chain(glib::system_data_dirs())
            .filter_map(|dir| std::fs::canonicalize(dir.join("applications")).ok())
            .any(|apps| real.starts_with(apps))
    });
    executable || installed
}

/// "Als vertrauenswürdig markieren": sets the owner's x bit (GIO, so that it
/// works on GVfs places too).
pub fn mark_trusted(path: &Path) -> Result<(), glib::Error> {
    let mode = std::fs::metadata(path)
        .map(|m| std::os::unix::fs::PermissionsExt::mode(&m.permissions()))
        .unwrap_or(0o644);
    gio::File::for_path(path).set_attribute_uint32(
        "unix::mode",
        (mode | 0o100) & 0o7777,
        gio::FileQueryInfoFlags::NONE,
        gio::Cancellable::NONE,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_managers_in_their_order() {
        let installed = [
            "org.querencia.Files.desktop",
            "io.github.lgse.Strata.desktop",
        ];
        let found = first_found(&FILE_MANAGERS, |id| {
            installed.contains(&id).then(|| id.to_owned())
        });
        assert_eq!(found.as_deref(), Some("org.querencia.Files.desktop"));
        let only_bridge = first_found(&FILE_MANAGERS, |id| {
            (id == "io.github.lgse.Strata.desktop").then(|| id.to_owned())
        });
        assert_eq!(
            only_bridge.as_deref(),
            Some("io.github.lgse.Strata.desktop")
        );
        assert_eq!(FILE_MANAGERS[0], "org.querencia.Dateien.desktop");
        assert_eq!(first_found(&FILE_MANAGERS, |_| None::<()>), None);
    }

    fn info(kind: gio::FileType, content_type: &str, mode: u32) -> gio::FileInfo {
        let info = gio::FileInfo::new();
        info.set_file_type(kind);
        info.set_content_type(content_type);
        info.set_attribute_uint32("unix::mode", mode);
        info
    }

    #[test]
    fn places_folders_starters_and_programs_are_told_apart() {
        let none = gio::FileInfo::new();
        assert!(matches!(
            plan_open(&gio::File::for_uri("trash:///"), &none),
            OpenPlan::Location(uri, _) if uri == "trash:///"
        ));
        let folder = info(gio::FileType::Directory, FOLDER_TYPE, 0o755);
        assert!(matches!(
            plan_open(&gio::File::for_path("/tmp"), &folder),
            OpenPlan::Folder(_)
        ));
        let base = std::env::temp_dir().join(format!("dateiarbeit-oeffnen-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).expect("fixture");
        let starter = base.join("s.desktop");
        std::fs::write(
            &starter,
            "[Desktop Entry]\nType=Application\nName=S\nExec=true\n",
        )
        .expect("fixture");
        let starter_info = info(gio::FileType::Regular, typ::DESKTOP_ENTRY, 0o644);
        assert!(matches!(
            plan_open(&gio::File::for_path(&starter), &starter_info),
            OpenPlan::Starter { trusted: false }
        ));
        mark_trusted(&starter).expect("x-Bit");
        assert!(is_trusted(&starter));
        assert!(matches!(
            plan_open(&gio::File::for_path(&starter), &starter_info),
            OpenPlan::Starter { trusted: true }
        ));
        let script = info(gio::FileType::Regular, "application/x-shellscript", 0o755);
        assert!(matches!(
            plan_open(&gio::File::for_path(base.join("lauf.sh")), &script),
            OpenPlan::Executable { text: true }
        ));
        // without the x bit a script is just a text file
        let plain = info(gio::FileType::Regular, "application/x-shellscript", 0o644);
        assert!(!matches!(
            plan_open(&gio::File::for_path(base.join("lauf.sh")), &plain),
            OpenPlan::Executable { .. }
        ));
        // a type nobody opens and a name that says nothing: no program
        let odd = info(
            gio::FileType::Regular,
            "application/x-querencia-probe",
            0o644,
        );
        assert!(matches!(
            plan_open(&gio::File::for_path(base.join("ohne")), &odd),
            OpenPlan::NoProgram(t) if t == "application/x-querencia-probe"
        ));
        std::fs::remove_dir_all(&base).expect("cleanup");
    }
}
