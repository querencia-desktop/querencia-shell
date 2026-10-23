//! Atomic renames: put a finished object under its name without ever
//! replacing what is there (`RENAME_NOREPLACE`), or swap it with what is
//! there in one step (`RENAME_EXCHANGE`). Local paths use `renameat2(2)`;
//! GVfs places use GIO's move without `OVERWRITE`, which is as close as GIO
//! gets (backends check and rename in two steps; docs/dateiarbeit.md).

use std::ffi::CString;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use gio::prelude::*;

fn c_path(path: &Path) -> io::Result<CString> {
    CString::new(path.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Pfad mit NUL-Byte"))
}

fn renameat2(from: &Path, to: &Path, flags: libc::c_uint) -> io::Result<()> {
    let (from, to) = (c_path(from)?, c_path(to)?);
    // SAFETY: both pointers come from live CStrings (NUL-terminated, valid
    // for the call); AT_FDCWD makes relative paths relative to the working
    // directory, absolute ones ignore it. The call reads nothing else.
    let result = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            from.as_ptr(),
            libc::AT_FDCWD,
            to.as_ptr(),
            flags,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

/// Renames `from` to `to` only if `to` does not exist (atomically:
/// `EEXIST` otherwise). `EXDEV` across file systems; `EINVAL` where the
/// file system has no `RENAME_NOREPLACE` (some FUSE and NFS).
pub fn rename_noreplace(from: &Path, to: &Path) -> io::Result<()> {
    renameat2(from, to, libc::RENAME_NOREPLACE)
}

/// Swaps `a` and `b` atomically (both must exist, any types).
pub fn rename_exchange(a: &Path, b: &Path) -> io::Result<()> {
    renameat2(a, b, libc::RENAME_EXCHANGE)
}

/// The local path of `file` when it is a plain local file (`file://`), not
/// a GVfs place seen through FUSE.
pub fn local_path(file: &gio::File) -> Option<std::path::PathBuf> {
    (file.uri_scheme().as_deref() == Some("file"))
        .then(|| file.path())
        .flatten()
}

/// An `io::Error` as GIO error, so that all failures read the same way.
pub fn gio_error(err: &io::Error) -> glib::Error {
    let kind = match err.raw_os_error() {
        Some(libc::EEXIST) | Some(libc::ENOTEMPTY) => gio::IOErrorEnum::Exists,
        Some(libc::ENOENT) => gio::IOErrorEnum::NotFound,
        Some(libc::EACCES) | Some(libc::EPERM) => gio::IOErrorEnum::PermissionDenied,
        Some(libc::ENOSPC) => gio::IOErrorEnum::NoSpace,
        Some(libc::EROFS) => gio::IOErrorEnum::ReadOnly,
        Some(libc::EXDEV) => gio::IOErrorEnum::WouldRecurse,
        Some(libc::EINVAL) | Some(libc::ENOSYS) => gio::IOErrorEnum::NotSupported,
        Some(libc::EISDIR) => gio::IOErrorEnum::IsDirectory,
        Some(libc::ENOTDIR) => gio::IOErrorEnum::NotDirectory,
        Some(libc::ENAMETOOLONG) => gio::IOErrorEnum::FilenameTooLong,
        _ => gio::IOErrorEnum::Failed,
    };
    glib::Error::new(kind, &err.to_string())
}

/// Gives `from` (a finished object) the name `to` without replacing
/// anything: `Exists` when `to` exists. Local: `RENAME_NOREPLACE`; GVfs:
/// GIO's move without `OVERWRITE` and without a copy fallback.
pub fn put_in_place(from: &gio::File, to: &gio::File) -> Result<(), glib::Error> {
    if let (Some(a), Some(b)) = (local_path(from), local_path(to)) {
        return rename_noreplace(&a, &b).map_err(|err| gio_error(&err));
    }
    from.move_(
        to,
        gio::FileCopyFlags::NOFOLLOW_SYMLINKS | gio::FileCopyFlags::NO_FALLBACK_FOR_MOVE,
        gio::Cancellable::NONE,
        None,
    )
}

/// Puts `new` (a finished object) in the place of `old` (which exists) and
/// leaves the replaced object at `new`'s former name, where the caller
/// deletes it. Local: one `RENAME_EXCHANGE`, so `old`'s name never
/// disappears; where the file system has none, `old` first moves to a
/// spare name (`NOREPLACE`) and comes back if `new` cannot take its place.
/// GVfs: the same two steps through GIO. Returns where the replaced object
/// is now.
pub fn swap_in(
    new: &gio::File,
    old: &gio::File,
    spare: &gio::File,
) -> Result<gio::File, glib::Error> {
    if let (Some(a), Some(b)) = (local_path(new), local_path(old)) {
        match rename_exchange(&a, &b) {
            Ok(()) => return Ok(new.clone()),
            Err(err) if matches!(err.raw_os_error(), Some(libc::EINVAL) | Some(libc::ENOSYS)) => {}
            Err(err) => return Err(gio_error(&err)),
        }
    }
    put_in_place(old, spare)?;
    match put_in_place(new, old) {
        Ok(()) => Ok(spare.clone()),
        Err(err) => {
            // the old object goes back under its name; if even that fails,
            // it stays under the spare name, which says what it is
            let _ = put_in_place(spare, old);
            Err(err)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noreplace_and_exchange_on_a_local_file_system() {
        let base = std::env::temp_dir().join(format!("dateiarbeit-atomar-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("ordner")).expect("fixture");
        std::fs::write(base.join("a"), "a").expect("fixture");
        std::fs::write(base.join("b"), "b").expect("fixture");
        let err = rename_noreplace(&base.join("a"), &base.join("b")).expect_err("b existiert");
        assert_eq!(err.raw_os_error(), Some(libc::EEXIST));
        assert_eq!(std::fs::read_to_string(base.join("b")).expect("b"), "b");
        rename_noreplace(&base.join("a"), &base.join("c")).expect("frei");
        rename_exchange(&base.join("c"), &base.join("ordner")).expect("tauschen");
        assert!(base.join("c").is_dir());
        assert_eq!(
            std::fs::read_to_string(base.join("ordner")).expect("a"),
            "a"
        );
        // GIO side: Exists, nothing replaced
        let (b, ordner) = (
            gio::File::for_path(base.join("b")),
            gio::File::for_path(base.join("ordner")),
        );
        let err = put_in_place(&b, &ordner).expect_err("belegt");
        assert!(err.matches(gio::IOErrorEnum::Exists), "{err}");
        let old = swap_in(&b, &ordner, &gio::File::for_path(base.join("ersatz"))).expect("tausch");
        assert_eq!(
            std::fs::read_to_string(base.join("ordner")).expect("neu"),
            "b"
        );
        assert_eq!(old.path().as_deref(), Some(base.join("b").as_path()));
        assert_eq!(std::fs::read_to_string(base.join("b")).expect("alt"), "a");
        std::fs::remove_dir_all(&base).expect("cleanup");
    }
}
