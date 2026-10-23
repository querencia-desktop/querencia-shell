// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 LGSE Ltd. (Strata)
// SPDX-FileCopyrightText: 2026 Querencia Desktop contributors
//
// Adapted from Strata `src/adapters/trash_restore.rs` (Querencia Files,
// branch `querencia` at 3dfdb25f), MIT License, see
// `LICENSES/MIT-Strata.txt`, and its notes `docs/trash-restore-testing.md`.
// Changes: synchronous (the job runs in its own thread), local paths
// instead of Strata's `Location`, the same-volume check by device number,
// reasons as an enum with gettext texts, the move itself here
// ([`execute`], `RENAME_NOREPLACE` as in Strata, without `openat2`: the
// parent is checked again right before the rename).

//! Restoring from the trash. Trash metadata (`.trashinfo`, `trash::orig-path`)
//! is untrusted: the destination must lie on the volume of the trash entry
//! (no other mount, no bind mount or subvolume boundary, not inside the
//! trash, no symlinked parent), and the move never replaces anything.
//! Relative `Path=` entries resolve against the volume's top directory,
//! including the shared `.Trash/$uid` layout.

use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};

use gio::prelude::*;

use super::mounts::MountTable;
use crate::atomar;
use crate::i18n::gettext;

const MAX_RESTORE_PATH_BYTES: usize = 4096;

/// Why an object cannot be restored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Grund {
    Invalid,
    Unavailable,
    UnknownOrigin,
    NoMountTable,
    OutsideVolume,
    BindMount,
    InsideTrash,
    ParentMissing,
    SymlinkedParent,
    /// Something exists at the destination (the job asks then).
    Exists,
    /// The file system cannot rename without replacing (`RENAME_NOREPLACE`).
    NoAtomicRename,
    System(String),
}

impl Grund {
    pub fn text(&self) -> String {
        match self {
            Grund::Invalid => gettext("The original location is invalid."),
            Grund::Unavailable => gettext("The original location is unavailable."),
            Grund::UnknownOrigin => {
                gettext("It cannot be determined where this item was deleted from.")
            }
            Grund::NoMountTable => gettext(
                "The volume of the trash cannot be determined because the mount table is unavailable.",
            ),
            Grund::OutsideVolume => gettext(
                "The original location is outside the volume of the trash and cannot be restored.",
            ),
            Grund::BindMount => gettext(
                "The original location crosses a bind mount or subvolume boundary and cannot be restored.",
            ),
            Grund::InsideTrash => gettext("The original location must not be inside the trash."),
            Grund::ParentMissing => gettext("The original folder no longer exists."),
            Grund::SymlinkedParent => gettext(
                "The original folder is a symbolic link; the item cannot be restored there.",
            ),
            Grund::Exists => gettext("There is already an item with this name at the target."),
            Grund::NoAtomicRename => gettext(
                "The file system cannot restore without the risk of replacing data; copy the item instead.",
            ),
            Grund::System(message) => message.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RestorePlan {
    pub source_path: PathBuf,
    pub destination: PathBuf,
    pub allowed_root: PathBuf,
    pub trash_info: Option<PathBuf>,
}

#[derive(Clone, Debug)]
pub struct RestoreContext {
    pub home_trash_root: PathBuf,
    pub uid: u32,
    pub mounts: MountTable,
}

impl RestoreContext {
    pub fn current() -> Self {
        Self {
            home_trash_root: glib::user_data_dir().join("Trash"),
            uid: current_uid(),
            mounts: MountTable::current(),
        }
    }
}

/// The user's ID, from the owner of `/proc/self` (no libc call needed).
pub fn current_uid() -> u32 {
    std::fs::metadata("/proc/self").map_or(u32::MAX, |m| m.uid())
}

pub fn decode_trashinfo_path(encoded: &str) -> Option<PathBuf> {
    let encoded = encoded.trim();
    if encoded.is_empty() {
        return None;
    }
    let encoded = encoded
        .strip_prefix("file://")
        .map(|rest| rest.strip_prefix("//").unwrap_or(rest))
        .unwrap_or(encoded);
    let decoded = percent_decode(encoded.as_bytes())?;
    if decoded.is_empty() || decoded.contains(&0) || decoded.len() > MAX_RESTORE_PATH_BYTES {
        return None;
    }
    Some(PathBuf::from(OsString::from_vec(decoded)))
}

/// Whether `a` and the nearest existing ancestor of `b` are on one device.
fn same_device(a: &Path, b: &Path) -> bool {
    let Ok(source) = std::fs::symlink_metadata(a) else {
        return false;
    };
    b.ancestors()
        .find_map(|p| std::fs::metadata(p).ok())
        .is_some_and(|dest| dest.dev() == source.dev())
}

pub fn plan_from_known_paths(
    source_path: &Path,
    orig_path: &Path,
    trash_root: &Path,
    trash_info: Option<PathBuf>,
    context: &RestoreContext,
) -> Result<RestorePlan, Grund> {
    let (destination, allowed_root) =
        resolve_restore_destination(orig_path, source_path, trash_root, context)?;
    if !destination.starts_with(&allowed_root) || !same_device(source_path, &destination) {
        return Err(Grund::OutsideVolume);
    }
    if context.mounts.mount_point_for(&destination) != Some(allowed_root.as_path()) {
        return Err(Grund::BindMount);
    }
    let trash_tree = trash_tree_root(trash_root);
    if path_is_within(&destination, &trash_tree) {
        return Err(Grund::InsideTrash);
    }
    let trash_tree = trash_tree
        .canonicalize()
        .unwrap_or_else(|_| trash_tree.clone());
    if path_is_within(&destination, &trash_tree) {
        return Err(Grund::InsideTrash);
    }
    if !destination.parent().is_some_and(Path::is_dir) {
        return Err(Grund::ParentMissing);
    }
    Ok(RestorePlan {
        source_path: source_path.to_path_buf(),
        destination,
        allowed_root,
        trash_info,
    })
}

/// The plan for an object in a trash `files` folder (its physical path).
pub fn plan_for_path(physical: &Path, context: &RestoreContext) -> Result<RestorePlan, Grund> {
    let trash_root = trash_root_from_files_path(physical).ok_or(Grund::UnknownOrigin)?;
    let info = info_path_for_files_path(physical);
    let orig = info
        .as_deref()
        .and_then(orig_path_from_trashinfo)
        .ok_or(Grund::Unavailable)?;
    plan_from_known_paths(physical, &orig, &trash_root, info, context)
}

/// The plan for `file`: a `trash:///` entry (GVfs tells its physical place
/// and `trash::orig-path`) or a local path in a trash `files` folder (also
/// without GVfs, `GIO_USE_VFS=local`).
pub fn plan_for_file(file: &gio::File, context: &RestoreContext) -> Result<RestorePlan, Grund> {
    if file.uri_scheme().as_deref() != Some("trash") {
        let path = atomar::local_path(file).ok_or(Grund::UnknownOrigin)?;
        return plan_for_path(&path, context);
    }
    let info = file
        .query_info(
            "trash::orig-path,standard::target-uri",
            gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
            gio::Cancellable::NONE,
        )
        .map_err(|err| Grund::System(err.message().to_owned()))?;
    let physical = info
        .attribute_string(gio::FILE_ATTRIBUTE_STANDARD_TARGET_URI)
        .and_then(|target| glib::filename_from_uri(&target).ok())
        .and_then(|(path, host)| {
            host.is_none_or(|h| h.eq_ignore_ascii_case("localhost"))
                .then_some(path)
        })
        .ok_or(Grund::UnknownOrigin)?;
    let trash_root = trash_root_from_files_path(&physical).ok_or(Grund::UnknownOrigin)?;
    let orig = info
        .attribute_byte_string("trash::orig-path")
        .map(|p| PathBuf::from(p.as_str()))
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or(Grund::Unavailable)?;
    let trash_info = info_path_for_files_path(&physical);
    plan_from_known_paths(&physical, &orig, &trash_root, trash_info, context)
}

/// Moves the object of `plan` back, to `name` instead of its old name when
/// given (a conflict answered with "rename"). Never replaces: `Exists` when
/// the destination is taken. The parent is checked again right before the
/// rename (it may have changed since planning). Afterwards the `.trashinfo`
/// goes; a failure there is logged, the object is back. Returns where.
pub fn execute(
    plan: &RestorePlan,
    name: Option<&str>,
    context: &RestoreContext,
) -> Result<PathBuf, Grund> {
    let destination = match name {
        Some(name) => {
            if crate::namen::check_file_name(name).is_err() {
                return Err(Grund::Invalid);
            }
            plan.destination.with_file_name(name)
        }
        None => plan.destination.clone(),
    };
    let parent = destination.parent().ok_or(Grund::Invalid)?;
    match std::fs::symlink_metadata(parent) {
        Ok(meta) if meta.file_type().is_symlink() => return Err(Grund::SymlinkedParent),
        Ok(meta) if meta.is_dir() => {}
        _ => return Err(Grund::ParentMissing),
    }
    if parent.canonicalize().ok().as_deref() != Some(parent)
        || !destination.starts_with(&plan.allowed_root)
        || context.mounts.mount_point_for(&destination) != Some(plan.allowed_root.as_path())
    {
        return Err(Grund::OutsideVolume);
    }
    match atomar::rename_noreplace(&plan.source_path, &destination) {
        Ok(()) => {}
        Err(err) => {
            return Err(match err.raw_os_error() {
                Some(libc::EEXIST) | Some(libc::ENOTEMPTY) => Grund::Exists,
                Some(libc::EXDEV) | Some(libc::EINVAL) | Some(libc::ENOSYS) => {
                    Grund::NoAtomicRename
                }
                Some(libc::ENOENT) => Grund::Unavailable,
                _ => Grund::System(err.to_string()),
            });
        }
    }
    if let Some(info) = &plan.trash_info
        && let Err(err) = std::fs::remove_file(info)
    {
        crate::say!(
            "Papierkorb: {} wiederhergestellt, {} nicht entfernt: {err}",
            destination.display(),
            info.display()
        );
    }
    Ok(destination)
}

fn resolve_restore_destination(
    orig_path: &Path,
    source_path: &Path,
    trash_root: &Path,
    context: &RestoreContext,
) -> Result<(PathBuf, PathBuf), Grund> {
    if orig_path.as_os_str().is_empty()
        || orig_path.as_os_str().as_bytes().contains(&0)
        || orig_path.as_os_str().len() > MAX_RESTORE_PATH_BYTES
        || orig_path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
    {
        return Err(Grund::Invalid);
    }
    let allowed_root = context
        .mounts
        .mount_point_for(source_path)
        .map(Path::to_path_buf)
        .ok_or(Grund::NoMountTable)?;
    let absolute = if orig_path.is_absolute() {
        orig_path.to_path_buf()
    } else {
        topdir_for_trash_root(trash_root)
            .ok_or(Grund::Invalid)?
            .join(orig_path)
    };
    let normalized = lexically_normalize(&absolute).ok_or(Grund::Invalid)?;
    let destination = canonical_restore_destination(&normalized)?;
    if destination.file_name().is_none() {
        return Err(Grund::Invalid);
    }
    let allowed_root = allowed_root
        .canonicalize()
        .map_err(|_| Grund::OutsideVolume)?;
    Ok((destination, allowed_root))
}

fn orig_path_from_trashinfo(info_path: &Path) -> Option<PathBuf> {
    let contents = std::fs::read_to_string(info_path).ok()?;
    contents
        .lines()
        .find_map(|line| line.strip_prefix("Path="))
        .and_then(decode_trashinfo_path)
}

fn shared_trash_dir(trash_root: &Path) -> Option<&Path> {
    trash_root
        .parent()
        .filter(|parent| parent.file_name() == Some(OsStr::new(".Trash")))
}

fn trash_tree_root(trash_root: &Path) -> PathBuf {
    shared_trash_dir(trash_root)
        .unwrap_or(trash_root)
        .to_path_buf()
}

pub fn trash_root_from_files_path(path: &Path) -> Option<PathBuf> {
    let files = path.parent()?;
    if files.file_name()? != "files" {
        return None;
    }
    files.parent().map(Path::to_path_buf)
}

/// The trash spec anchors relative `Path=` above `.Trash` in the shared layout.
fn topdir_for_trash_root(trash_root: &Path) -> Option<&Path> {
    match shared_trash_dir(trash_root) {
        Some(shared) => shared.parent(),
        None => trash_root.parent(),
    }
}

pub fn info_path_for_files_path(path: &Path) -> Option<PathBuf> {
    let name = path.file_name()?;
    let trash_root = trash_root_from_files_path(path)?;
    let mut info_name = name.as_bytes().to_vec();
    info_name.extend_from_slice(b".trashinfo");
    Some(trash_root.join("info").join(OsStr::from_bytes(&info_name)))
}

fn percent_decode(input: &[u8]) -> Option<Vec<u8>> {
    let mut decoded = Vec::with_capacity(input.len());
    let mut index = 0;
    while index < input.len() {
        match input[index] {
            b'%' => {
                let hex = input.get(index + 1..index + 3)?;
                let value = u8::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok()?;
                decoded.push(value);
                index += 3;
            }
            byte => {
                decoded.push(byte);
                index += 1;
            }
        }
    }
    Some(decoded)
}

fn lexically_normalize(path: &Path) -> Option<PathBuf> {
    if !path.is_absolute() {
        return None;
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::RootDir => normalized.push(Component::RootDir),
            Component::CurDir => {}
            Component::ParentDir | Component::Prefix(_) => return None,
            Component::Normal(part) => {
                if part.as_bytes().is_empty() || part.as_bytes().contains(&0) {
                    return None;
                }
                normalized.push(part);
            }
        }
    }
    if normalized.as_os_str().is_empty() {
        return None;
    }
    Some(normalized)
}

fn canonical_restore_destination(path: &Path) -> Result<PathBuf, Grund> {
    let file_name = path.file_name().ok_or(Grund::Invalid)?;
    let parent = path.parent().ok_or(Grund::Invalid)?;
    if std::fs::symlink_metadata(parent).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Err(Grund::SymlinkedParent);
    }
    let mut existing = parent.to_path_buf();
    let mut missing = Vec::new();
    while !existing.as_os_str().is_empty() && !existing.exists() {
        let Some(name) = existing.file_name() else {
            break;
        };
        missing.push(name.to_os_string());
        match existing.parent() {
            Some(parent) => existing = parent.to_path_buf(),
            None => break,
        }
    }
    let mut canonical = if existing.exists() {
        existing.canonicalize().map_err(|_| Grund::OutsideVolume)?
    } else {
        existing
    };
    for name in missing.into_iter().rev() {
        if name.as_bytes() == b".." || name.as_bytes() == b"." || name.as_bytes().contains(&0) {
            return Err(Grund::Invalid);
        }
        canonical.push(name);
    }
    canonical.push(file_name);
    Ok(canonical)
}

fn path_is_within(path: &Path, root: &Path) -> bool {
    path == root || path.starts_with(root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("dateiarbeit-wh-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("fixture");
        dir.canonicalize().expect("kanonisch")
    }

    fn context_for(home_trash: &Path, uid: u32, mount_point: &Path) -> RestoreContext {
        RestoreContext {
            home_trash_root: home_trash.to_path_buf(),
            uid,
            mounts: MountTable::parse(format!(
                "1 0 8:1 / / rw - ext4 /dev/sda1 rw\n22 1 8:2 / {} rw - ext4 /dev/sdb1 rw\n",
                mount_point.display()
            )),
        }
    }

    fn volume_trash(root: &Path, uid: u32) -> PathBuf {
        let trash = root.join(format!(".Trash-{uid}"));
        fs::create_dir_all(trash.join("files")).expect("files");
        fs::create_dir_all(trash.join("info")).expect("info");
        trash
    }

    #[test]
    fn decode_trashinfo_path_percent_decodes_and_rejects_empty() {
        assert_eq!(
            decode_trashinfo_path("/home/user/My%20File.txt").as_deref(),
            Some(Path::new("/home/user/My File.txt"))
        );
        assert_eq!(
            decode_trashinfo_path("Documents/%2e%2e/%2e%2e/etc/passwd").as_deref(),
            Some(Path::new("Documents/../../etc/passwd"))
        );
        assert_eq!(decode_trashinfo_path(""), None);
        assert_eq!(decode_trashinfo_path("   "), None);
        assert_eq!(decode_trashinfo_path("%00"), None);
    }

    #[test]
    fn relative_orig_path_is_joined_to_the_trash_parent_and_restored() {
        let fixture = scratch("relativ");
        let trash = volume_trash(&fixture, 1000);
        let source = trash.join("files/report.txt");
        fs::write(&source, b"ok").expect("source");
        let info = trash.join("info/report.txt.trashinfo");
        fs::write(&info, "[Trash Info]\nPath=Documents/report.txt\n").expect("info");
        let context = context_for(&fixture.join("home-trash"), 1000, &fixture);
        fs::create_dir_all(fixture.join("Documents")).expect("dest parent");
        let plan = plan_for_path(&source, &context).expect("in-scope relative path");
        assert_eq!(plan.destination, fixture.join("Documents/report.txt"));
        assert_eq!(plan.allowed_root, fixture);
        let back = execute(&plan, None, &context).expect("zurück");
        assert_eq!(fs::read(&back).expect("da"), b"ok");
        assert!(!info.exists() && !source.exists());
        fs::remove_dir_all(&fixture).expect("cleanup");
    }

    #[test]
    fn crafted_paths_never_leave_the_volume_or_enter_the_trash() {
        let fixture = scratch("boese");
        let trash = volume_trash(&fixture, 1000);
        let source = trash.join("files/report");
        fs::write(&source, b"original").expect("source");
        let context = context_for(&fixture.join("home-trash"), 1000, &fixture);
        for (orig, grund) in [
            (PathBuf::from("/outside/report"), Grund::OutsideVolume),
            (PathBuf::from("Documents/../../etc/passwd"), Grund::Invalid),
            (fixture.join(".Trash-1000/files/x"), Grund::InsideTrash),
            (fixture.join("fehlt/report"), Grund::ParentMissing),
        ] {
            let err = plan_from_known_paths(&source, &orig, &trash, None, &context)
                .expect_err("abgelehnt");
            assert_eq!(err, grund, "{}", orig.display());
        }
        // a symlinked parent is refused (it could point anywhere)
        fs::create_dir_all(fixture.join("echt")).expect("fixture");
        std::os::unix::fs::symlink(fixture.join("echt"), fixture.join("verweis")).expect("link");
        let err = plan_from_known_paths(
            &source,
            &fixture.join("verweis/report"),
            &trash,
            None,
            &context,
        )
        .expect_err("Verweis");
        assert_eq!(err, Grund::SymlinkedParent);
        assert_eq!(fs::read(&source).expect("source"), b"original");
        fs::remove_dir_all(&fixture).expect("cleanup");
    }

    #[test]
    fn a_nested_mount_on_the_same_file_system_is_refused() {
        let fixture = scratch("bind");
        let trash = volume_trash(&fixture, 1000);
        let source = trash.join("files/report");
        fs::write(&source, b"original").expect("source");
        let nested = fixture.join("nested");
        fs::create_dir(&nested).expect("nested");
        let context = RestoreContext {
            home_trash_root: fixture.join("home-trash"),
            uid: 1000,
            mounts: MountTable::parse(format!(
                "1 0 8:1 / / rw - ext4 /dev/root rw\n2 1 8:1 / {} rw - ext4 /dev/root rw\n3 2 8:1 /original {} rw - ext4 /dev/root rw\n",
                fixture.display(),
                nested.display()
            )),
        };
        let err = plan_from_known_paths(&source, &nested.join("report"), &trash, None, &context)
            .expect_err("bind");
        assert_eq!(err, Grund::BindMount);
        fs::remove_dir_all(&fixture).expect("cleanup");
    }

    #[test]
    fn restoring_never_replaces_and_can_take_another_name() {
        let fixture = scratch("belegt");
        let trash = volume_trash(&fixture, 1000);
        let source = trash.join("files/brief.txt");
        fs::write(&source, b"alt").expect("source");
        let info = trash.join("info/brief.txt.trashinfo");
        let orig = fixture.join("brief.txt");
        fs::write(&info, format!("[Trash Info]\nPath={}\n", orig.display())).expect("info");
        fs::write(&orig, b"neu").expect("belegt");
        let context = context_for(&fixture.join("home-trash"), 1000, &fixture);
        let plan = plan_for_path(&source, &context).expect("plan");
        assert_eq!(execute(&plan, None, &context), Err(Grund::Exists));
        assert_eq!(fs::read(&orig).expect("neu"), b"neu");
        assert!(source.exists() && info.exists(), "Eintrag bleibt");
        let back = execute(&plan, Some("brief (alt).txt"), &context).expect("anderer Name");
        assert_eq!(fs::read(back).expect("alt"), b"alt");
        assert_eq!(execute(&plan, Some("a/b"), &context), Err(Grund::Invalid));
        fs::remove_dir_all(&fixture).expect("cleanup");
    }
}
