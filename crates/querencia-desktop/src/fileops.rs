//! Desktop 0.2.3 (QD-108): file operations of the desktop, the parts
//! without a display.
//!
//! - URI lists (`text/uri-list`) and the clipboard text of GNOME file
//!   managers (`x-special/gnome-copied-files`: `copy` or `cut`, then one URI
//!   per line), which Querencia Files, Nautilus, Caja, Nemo and Thunar read
//!   and write;
//! - names for copies ("notiz (Kopie).txt", "notiz (Kopie 2).txt");
//! - copy, move, trash and delete through GIO, one object after the other
//!   ([`run`], called off the main thread by `desktop_view::transfer`).
//!
//! Nothing is ever overwritten: a copy onto an existing name gets a free
//! name, a move onto one is refused. Links are copied, moved and deleted as
//! links, never followed. A folder is never copied or moved into itself.

use std::path::Path;

use gtk4::gio;
use gtk4::gio::prelude::*;
use gtk4::glib;

use crate::icons;

pub const URI_LIST: &str = "text/uri-list";
pub const GNOME_COPIED_FILES: &str = "x-special/gnome-copied-files";
/// KDE's cut marker beside `text/uri-list`; `1`: the files were cut.
pub const KDE_CUT_SELECTION: &str = "application/x-kde-cutselection";
/// The desktop's own drags of icons that are no files (Rechner, Papierkorb,
/// …): only the desktop itself accepts it.
pub const OWN_ICONS: &str = "application/x-querencia-desktop-icons";

/// `text/uri-list`: one URI per line, each ended by CR LF (RFC 2483).
pub fn uri_list(uris: &[String]) -> String {
    uris.iter().map(|uri| format!("{uri}\r\n")).collect()
}

/// The URIs of a `text/uri-list`; comments (`#`) and empty lines are left out.
pub fn parse_uri_list(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

/// `x-special/gnome-copied-files`: `cut` or `copy`, then one URI per line.
pub fn gnome_copied_files(cut: bool, uris: &[String]) -> String {
    let mut text = String::from(if cut { "cut" } else { "copy" });
    for uri in uris {
        text.push('\n');
        text.push_str(uri);
    }
    text
}

/// Reads `x-special/gnome-copied-files`: whether the files were cut, and
/// their URIs. Without a `copy`/`cut` first line or without URIs: `None`.
pub fn parse_gnome_copied_files(data: &[u8]) -> Option<(bool, Vec<String>)> {
    let text = std::str::from_utf8(data).ok()?;
    let mut lines = text.split('\n').map(|line| line.trim_end_matches('\r'));
    let cut = match lines.next()?.trim() {
        "cut" => true,
        "copy" => false,
        _ => return None,
    };
    let uris: Vec<String> = lines
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_owned)
        .collect();
    (!uris.is_empty()).then_some((cut, uris))
}

/// The name for a copy of `name` where `exists` says which names are taken:
/// `name` itself if free, else "stem (Kopie).ext", "stem (Kopie 2).ext", …
/// A folder keeps a dot in its name ("a.b (Kopie)").
pub fn copy_name(name: &str, is_dir: bool, exists: impl Fn(&str) -> bool) -> String {
    if !exists(name) {
        return name.to_owned();
    }
    let (stem, ext) = if is_dir {
        (name, "")
    } else {
        icons::split_extension(name)
    };
    std::iter::once(format!("{stem} (Kopie){ext}"))
        .chain((2..).map(|n| format!("{stem} (Kopie {n}){ext}")))
        .find(|candidate| !exists(candidate))
        .unwrap_or_default()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Copy,
    Move,
    Trash,
    /// Delete for good (Umschalt+Entf, after a question).
    Delete,
}

impl Op {
    /// For the log.
    pub fn verb(self) -> &'static str {
        match self {
            Op::Copy => "kopieren",
            Op::Move => "verschieben",
            Op::Trash => "in den Papierkorb",
            Op::Delete => "endgültig löschen",
        }
    }

    /// The title of the error dialog.
    pub fn failure_title(self) -> &'static str {
        match self {
            Op::Copy => "Beim Kopieren ist ein Fehler aufgetreten.",
            Op::Move => "Beim Verschieben ist ein Fehler aufgetreten.",
            Op::Trash => "Nicht alles konnte in den Papierkorb verschoben werden.",
            Op::Delete => "Nicht alles konnte gelöscht werden.",
        }
    }
}

/// What a [`run`] did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    /// The names in the target folder of what was copied or moved there
    /// (also of objects that were in it already).
    pub arrived: Vec<String>,
    /// Objects done.
    pub done: usize,
    /// (name, why) for every object that failed.
    pub failed: Vec<(String, String)>,
}

/// The error dialog's text: up to five objects, then how many more.
pub fn failure_text(op: Op, outcome: &Outcome) -> String {
    let total = outcome.done + outcome.failed.len();
    let mut text = if total == 1 {
        String::new()
    } else {
        format!(
            "{} von {total} Objekten {}:\n",
            outcome.failed.len(),
            match (op, outcome.failed.len()) {
                (Op::Copy, 1) => "wurde nicht kopiert",
                (Op::Copy, _) => "wurden nicht kopiert",
                (Op::Move, 1) => "wurde nicht verschoben",
                (Op::Move, _) => "wurden nicht verschoben",
                (Op::Trash, 1) => "wurde nicht in den Papierkorb verschoben",
                (Op::Trash, _) => "wurden nicht in den Papierkorb verschoben",
                (Op::Delete, 1) => "wurde nicht gelöscht",
                (Op::Delete, _) => "wurden nicht gelöscht",
            }
        )
    };
    let lines: Vec<String> = outcome
        .failed
        .iter()
        .take(5)
        .map(|(name, why)| format!("„{name}“: {why}"))
        .collect();
    text.push_str(&lines.join("\n"));
    if outcome.failed.len() > 5 {
        text.push_str(&format!("\n… und {} weitere", outcome.failed.len() - 5));
    }
    text
}

/// Runs `op` on `sources` (URIs) one after the other; `target` (a folder
/// URI) for copy and move. Blocking: call it off the main thread.
pub fn run(op: Op, sources: &[String], target: Option<&str>) -> Outcome {
    let mut outcome = Outcome::default();
    let target = target.map(gio::File::for_uri);
    for uri in sources {
        let source = gio::File::for_uri(uri);
        let name = source
            .basename()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| uri.clone());
        let result = match (op, &target) {
            (Op::Copy, Some(dir)) => copy_into(&source, dir, &name),
            (Op::Move, Some(dir)) => move_into(&source, dir, &name),
            (Op::Trash, _) => source
                .trash(gio::Cancellable::NONE)
                .map(|()| None)
                .map_err(|err| reason(&err, op)),
            (Op::Delete, _) => delete_recursive(&source)
                .map(|()| None)
                .map_err(|err| reason(&err, op)),
            (Op::Copy | Op::Move, None) => Err("kein Ziel".to_owned()),
        };
        match result {
            Ok(arrived) => {
                outcome.done += 1;
                outcome.arrived.extend(arrived);
            }
            Err(why) => outcome.failed.push((name, why)),
        }
    }
    outcome
}

/// Whether `dir` is `folder` or lies inside it.
fn inside(dir: &gio::File, folder: &gio::File) -> bool {
    dir.equal(folder) || dir.has_prefix(folder)
}

fn is_dir(file: &gio::File) -> bool {
    file.query_file_type(
        gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
        gio::Cancellable::NONE,
    ) == gio::FileType::Directory
}

fn exists(file: &gio::File) -> bool {
    file.query_file_type(
        gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
        gio::Cancellable::NONE,
    ) != gio::FileType::Unknown
}

fn copy_into(source: &gio::File, dir: &gio::File, name: &str) -> Result<Option<String>, String> {
    if !exists(source) {
        return Err("nicht mehr vorhanden".to_owned());
    }
    let folder = is_dir(source);
    if folder && inside(dir, source) {
        return Err("Ein Ordner kann nicht in sich selbst kopiert werden.".to_owned());
    }
    let free = copy_name(name, folder, |n| exists(&dir.child(n)));
    let dest = dir.child(&free);
    match copy_recursive(source, &dest) {
        Ok(()) => Ok(Some(free)),
        Err(err) => Err(reason(&err, Op::Copy)),
    }
}

fn move_into(source: &gio::File, dir: &gio::File, name: &str) -> Result<Option<String>, String> {
    if !exists(source) {
        return Err("nicht mehr vorhanden".to_owned());
    }
    // already there: nothing to move (a drop or paste into its own folder)
    if source.parent().is_some_and(|parent| parent.equal(dir)) {
        return Ok(Some(name.to_owned()));
    }
    if is_dir(source) && inside(dir, source) {
        return Err("Ein Ordner kann nicht in sich selbst verschoben werden.".to_owned());
    }
    let dest = dir.child(name);
    if exists(&dest) {
        return Err("Am Ziel gibt es schon ein Objekt mit diesem Namen.".to_owned());
    }
    let flags = gio::FileCopyFlags::NOFOLLOW_SYMLINKS | gio::FileCopyFlags::ALL_METADATA;
    match source.move_(&dest, flags, gio::Cancellable::NONE, None) {
        Ok(()) => Ok(Some(name.to_owned())),
        // a folder onto another file system: copy, then delete the original
        // (only after the whole copy succeeded)
        Err(err) if err.matches(gio::IOErrorEnum::WouldRecurse) => {
            copy_recursive(source, &dest).map_err(|err| reason(&err, Op::Move))?;
            delete_recursive(source).map_err(|err| {
                format!(
                    "kopiert, aber das Original blieb: {}",
                    reason(&err, Op::Delete)
                )
            })?;
            Ok(Some(name.to_owned()))
        }
        Err(err) => Err(reason(&err, Op::Move)),
    }
}

/// Copies `source` to `dest` (which must not exist); folders with all they
/// contain. Links are copied as links.
pub fn copy_recursive(source: &gio::File, dest: &gio::File) -> Result<(), glib::Error> {
    let flags = gio::FileCopyFlags::NOFOLLOW_SYMLINKS | gio::FileCopyFlags::ALL_METADATA;
    if !is_dir(source) {
        return source.copy(dest, flags, gio::Cancellable::NONE, None);
    }
    dest.make_directory(gio::Cancellable::NONE)?;
    let children = source.enumerate_children(
        "standard::name",
        gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
        gio::Cancellable::NONE,
    )?;
    while let Some(info) = children.next_file(gio::Cancellable::NONE)? {
        let name = info.name();
        copy_recursive(&source.child(&name), &dest.child(&name))?;
    }
    // permissions and times of the folder last, after its content
    let _ = source.copy_attributes(dest, flags, gio::Cancellable::NONE);
    Ok(())
}

/// Deletes `file`; a folder with all it contains. Links are deleted, never
/// followed.
pub fn delete_recursive(file: &gio::File) -> Result<(), glib::Error> {
    if is_dir(file) {
        let children = file.enumerate_children(
            "standard::name",
            gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
            gio::Cancellable::NONE,
        )?;
        while let Some(info) = children.next_file(gio::Cancellable::NONE)? {
            delete_recursive(&file.child(info.name()))?;
        }
    }
    file.delete(gio::Cancellable::NONE)
}

/// Why an operation failed, in German first; GIO's own text where nothing
/// better is known.
fn reason(err: &glib::Error, op: Op) -> String {
    match err.kind::<gio::IOErrorEnum>() {
        Some(gio::IOErrorEnum::PermissionDenied) => "keine Berechtigung".to_owned(),
        Some(gio::IOErrorEnum::NotFound) => "nicht mehr vorhanden".to_owned(),
        Some(gio::IOErrorEnum::Exists) => {
            "Am Ziel gibt es schon ein Objekt mit diesem Namen.".to_owned()
        }
        Some(gio::IOErrorEnum::NoSpace) => "Auf dem Ziel ist kein Platz mehr.".to_owned(),
        Some(gio::IOErrorEnum::ReadOnly) => "Das Ziel ist schreibgeschützt.".to_owned(),
        Some(gio::IOErrorEnum::InvalidFilename) => "Der Name ist am Ziel nicht erlaubt.".to_owned(),
        Some(gio::IOErrorEnum::NotSupported) if op == Op::Trash => {
            "An diesem Ort gibt es keinen Papierkorb.".to_owned()
        }
        _ => err.message().to_owned(),
    }
}

/// Objects inside `path` (not counting it) and their size in bytes, links
/// not followed; for "Eigenschaften" of a folder.
pub fn tally(path: &Path) -> (u64, u64) {
    let mut count = 0;
    let mut bytes = 0;
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let Ok(meta) = entry.path().symlink_metadata() else {
                continue;
            };
            count += 1;
            if meta.is_dir() {
                stack.push(entry.path());
            } else {
                bytes += meta.len();
            }
        }
    }
    (count, bytes)
}

/// A size as GNOME 2's properties showed it: "1,2 kB (1.234 Bytes)".
pub fn size_text(bytes: u64) -> String {
    let short = glib::format_size(bytes).to_string();
    if bytes < 1000 {
        return short;
    }
    let digits = bytes.to_string();
    let mut grouped = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            grouped.push('.');
        }
        grouped.push(c);
    }
    format!("{short} ({grouped} Bytes)")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("desktop-023-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("fixture");
        dir
    }

    fn uri(path: &Path) -> String {
        gio::File::for_path(path).uri().to_string()
    }

    #[test]
    fn uri_lists_round_trip_with_crlf_and_skip_comments() {
        let uris = vec![
            "file:///tmp/a%20b.txt".to_owned(),
            "file:///tmp/%C3%A4.txt".to_owned(),
        ];
        let text = uri_list(&uris);
        assert_eq!(text, "file:///tmp/a%20b.txt\r\nfile:///tmp/%C3%A4.txt\r\n");
        assert_eq!(parse_uri_list(&text), uris);
        assert_eq!(
            parse_uri_list("# Kommentar\r\n\r\nfile:///x\nfile:///y"),
            vec!["file:///x".to_owned(), "file:///y".to_owned()]
        );
        assert!(parse_uri_list("").is_empty());
    }

    #[test]
    fn the_gnome_clipboard_text_says_copy_or_cut() {
        let uris = vec!["file:///a".to_owned(), "file:///b".to_owned()];
        assert_eq!(
            gnome_copied_files(false, &uris),
            "copy\nfile:///a\nfile:///b"
        );
        assert_eq!(gnome_copied_files(true, &uris), "cut\nfile:///a\nfile:///b");
        assert_eq!(
            parse_gnome_copied_files(b"cut\r\nfile:///a\r\nfile:///b\r\n"),
            Some((true, uris.clone()))
        );
        assert_eq!(
            parse_gnome_copied_files(gnome_copied_files(false, &uris).as_bytes()),
            Some((false, uris))
        );
        // a plain URI list or text is not this format; no URIs: nothing
        assert_eq!(parse_gnome_copied_files(b"file:///a\nfile:///b"), None);
        assert_eq!(parse_gnome_copied_files(b"copy\n"), None);
        assert_eq!(parse_gnome_copied_files(&[0xff, 0xfe]), None);
    }

    #[test]
    fn copies_get_a_free_name_and_keep_the_extension() {
        let taken = ["notiz.txt", "notiz (Kopie).txt", "Ordner.alt", ".profil"];
        let exists = |n: &str| taken.contains(&n);
        assert_eq!(copy_name("neu.txt", false, exists), "neu.txt");
        assert_eq!(copy_name("notiz.txt", false, exists), "notiz (Kopie 2).txt");
        assert_eq!(copy_name("Ordner.alt", true, exists), "Ordner.alt (Kopie)");
        assert_eq!(copy_name(".profil", false, exists), ".profil (Kopie)");
        assert_eq!(copy_name("a.txt", false, |n| n == "a.txt"), "a (Kopie).txt");
    }

    #[test]
    fn copy_takes_folders_with_their_content_and_links_as_links() {
        let base = scratch("kopie");
        let (from, to) = (base.join("quelle"), base.join("ziel"));
        std::fs::create_dir_all(from.join("Ordner/innen")).expect("fixture");
        std::fs::create_dir_all(&to).expect("fixture");
        std::fs::write(from.join("Ordner/innen/a.txt"), "a").expect("fixture");
        std::fs::write(from.join("b.txt"), "b").expect("fixture");
        std::os::unix::fs::symlink(base.join("draussen"), from.join("Ordner/verweis"))
            .expect("fixture");
        std::fs::write(to.join("b.txt"), "alt").expect("fixture");
        let outcome = run(
            Op::Copy,
            &[uri(&from.join("Ordner")), uri(&from.join("b.txt"))],
            Some(&uri(&to)),
        );
        assert_eq!(outcome.failed, vec![]);
        assert_eq!(
            outcome.arrived,
            vec!["Ordner".to_owned(), "b (Kopie).txt".to_owned()]
        );
        assert_eq!(
            std::fs::read_to_string(to.join("Ordner/innen/a.txt")).expect("kopiert"),
            "a"
        );
        // the existing b.txt is untouched, the copy has a free name
        assert_eq!(
            std::fs::read_to_string(to.join("b.txt")).expect("alt"),
            "alt"
        );
        assert_eq!(
            std::fs::read_to_string(to.join("b (Kopie).txt")).expect("neu"),
            "b"
        );
        let link = to.join("Ordner/verweis").symlink_metadata().expect("link");
        assert!(link.file_type().is_symlink());
        // the originals stay
        assert!(from.join("Ordner/innen/a.txt").exists() && from.join("b.txt").exists());
        std::fs::remove_dir_all(&base).expect("cleanup");
    }

    #[test]
    fn a_copy_into_its_own_folder_is_a_duplicate_and_never_into_itself() {
        let base = scratch("selbst");
        std::fs::create_dir_all(base.join("Ordner/unter")).expect("fixture");
        std::fs::write(base.join("notiz.txt"), "n").expect("fixture");
        let outcome = run(Op::Copy, &[uri(&base.join("notiz.txt"))], Some(&uri(&base)));
        assert_eq!(outcome.arrived, vec!["notiz (Kopie).txt".to_owned()]);
        for target in [base.join("Ordner"), base.join("Ordner/unter")] {
            let outcome = run(Op::Copy, &[uri(&base.join("Ordner"))], Some(&uri(&target)));
            assert_eq!(outcome.done, 0);
            assert_eq!(outcome.failed.len(), 1, "{outcome:?}");
            assert!(outcome.failed[0].1.contains("in sich selbst"));
        }
        let outcome = run(
            Op::Move,
            &[uri(&base.join("Ordner"))],
            Some(&uri(&base.join("Ordner/unter"))),
        );
        assert!(outcome.failed[0].1.contains("in sich selbst"));
        assert!(base.join("Ordner/unter").is_dir());
        std::fs::remove_dir_all(&base).expect("cleanup");
    }

    #[test]
    fn a_move_never_overwrites_and_a_move_into_the_same_folder_changes_nothing() {
        let base = scratch("verschieben");
        let (from, to) = (base.join("quelle"), base.join("ziel"));
        std::fs::create_dir_all(from.join("Ordner")).expect("fixture");
        std::fs::create_dir_all(&to).expect("fixture");
        std::fs::write(from.join("Ordner/x.txt"), "x").expect("fixture");
        std::fs::write(from.join("a.txt"), "neu").expect("fixture");
        std::fs::write(from.join("c.txt"), "c").expect("fixture");
        std::fs::write(to.join("a.txt"), "alt").expect("fixture");
        let outcome = run(
            Op::Move,
            &[
                uri(&from.join("Ordner")),
                uri(&from.join("a.txt")),
                uri(&from.join("c.txt")),
                uri(&from.join("fehlt.txt")),
            ],
            Some(&uri(&to)),
        );
        assert_eq!(outcome.done, 2);
        assert_eq!(
            outcome.arrived,
            vec!["Ordner".to_owned(), "c.txt".to_owned()]
        );
        assert_eq!(outcome.failed.len(), 2);
        assert_eq!(outcome.failed[0].0, "a.txt");
        assert_eq!(
            outcome.failed[1],
            ("fehlt.txt".to_owned(), "nicht mehr vorhanden".to_owned())
        );
        assert_eq!(
            std::fs::read_to_string(to.join("a.txt")).expect("alt"),
            "alt"
        );
        assert_eq!(
            std::fs::read_to_string(from.join("a.txt")).expect("neu"),
            "neu"
        );
        assert!(to.join("Ordner/x.txt").exists() && !from.join("Ordner").exists());
        assert!(to.join("c.txt").exists() && !from.join("c.txt").exists());
        let outcome = run(Op::Move, &[uri(&to.join("c.txt"))], Some(&uri(&to)));
        assert_eq!(
            (outcome.done, outcome.arrived),
            (1, vec!["c.txt".to_owned()])
        );
        assert!(to.join("c.txt").exists());
        let text = failure_text(
            Op::Move,
            &Outcome {
                arrived: vec![],
                done: 2,
                failed: vec![("a.txt".to_owned(), "x".to_owned())],
            },
        );
        assert_eq!(text, "1 von 3 Objekten wurde nicht verschoben:\n„a.txt“: x");
        std::fs::remove_dir_all(&base).expect("cleanup");
    }

    #[test]
    fn delete_removes_folders_but_never_what_a_link_points_to() {
        let base = scratch("loeschen");
        std::fs::create_dir_all(base.join("bleibt")).expect("fixture");
        std::fs::write(base.join("bleibt/wichtig.txt"), "w").expect("fixture");
        std::fs::create_dir_all(base.join("weg/innen")).expect("fixture");
        std::fs::write(base.join("weg/innen/a.txt"), "a").expect("fixture");
        std::os::unix::fs::symlink(base.join("bleibt"), base.join("weg/verweis")).expect("fixture");
        let outcome = run(Op::Delete, &[uri(&base.join("weg"))], None);
        assert_eq!((outcome.done, outcome.failed), (1, vec![]));
        assert!(!base.join("weg").exists());
        assert!(base.join("bleibt/wichtig.txt").exists());
        std::fs::remove_dir_all(&base).expect("cleanup");
    }

    #[test]
    fn tally_counts_folder_content_and_sizes_read_like_gnome() {
        let base = scratch("zaehlen");
        std::fs::create_dir_all(base.join("u")).expect("fixture");
        std::fs::write(base.join("a"), "1234").expect("fixture");
        std::fs::write(base.join("u/b"), "56").expect("fixture");
        assert_eq!(tally(&base), (3, 6));
        std::fs::remove_dir_all(&base).expect("cleanup");
        assert!(size_text(1_234_567).ends_with(" (1.234.567 Bytes)"));
        assert!(!size_text(12).contains('('));
    }
}
