//! Desktop icons (HMP-09), the parts without a display: which icons there
//! are, their order and grid, and names for new folders and documents.
//!
//! The icons, as on a GNOME 2 desktop:
//! - Rechner, Persönlicher Ordner, Papierkorb: only when Querencia Files
//!   (or, for the home folder, any folder program) can open them;
//! - Querencia Terminal: only when its desktop entry is installed;
//! - every file and folder in the XDG desktop directory.
//!
//! Where the icons are is `layout.rs` and `store.rs` (HMP-10); here is
//! the order in which icons without a place get one: the order of the
//! remembered places, then (for icons that have none yet) the natural
//! order. "Nach Namen aufräumen" sorts by name: the fixed icons first,
//! then everything else by name. This changes only the layout, never a
//! file.

use std::path::{Path, PathBuf};

use crate::store::{self, Art};

/// What an icon stands for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Computer,
    Home,
    Trash,
    /// A built-in Querencia starter, by its desktop file ID.
    App(String),
    /// A starter (`.desktop` file) in the desktop directory (HMP-10).
    Launcher(PathBuf),
    /// A file in the desktop directory.
    File(PathBuf),
    /// A folder in the desktop directory (HMP-10).
    Folder(PathBuf),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    /// Stable identity in the state file (`store::key`).
    pub key: String,
    /// The name shown under the icon.
    pub name: String,
    /// The name without the user's own one (special icons, starters).
    pub default_name: String,
    pub kind: Kind,
}

impl Item {
    pub fn new(kind: Kind, name: &str) -> Self {
        let key = store::key(art(&kind), &ident(&kind));
        Self {
            key,
            name: name.to_owned(),
            default_name: name.to_owned(),
            kind,
        }
    }

    pub fn art(&self) -> Art {
        art(&self.kind)
    }

    /// The name part of the identity: `computer`, the desktop file ID or
    /// the file name.
    pub fn ident(&self) -> String {
        ident(&self.kind)
    }

    fn fixed_rank(&self) -> Option<usize> {
        match self.kind {
            Kind::Computer => Some(0),
            Kind::Home => Some(1),
            Kind::Trash => Some(2),
            Kind::App(_) => Some(3),
            Kind::Launcher(_) | Kind::File(_) | Kind::Folder(_) => None,
        }
    }
}

fn art(kind: &Kind) -> Art {
    match kind {
        Kind::Computer | Kind::Home | Kind::Trash => Art::Special,
        Kind::App(_) => Art::Program,
        Kind::Launcher(_) => Art::Launcher,
        Kind::File(_) => Art::File,
        Kind::Folder(_) => Art::Folder,
    }
}

fn ident(kind: &Kind) -> String {
    match kind {
        Kind::Computer => "computer".to_owned(),
        Kind::Home => "home".to_owned(),
        Kind::Trash => "trash".to_owned(),
        Kind::App(id) => id.clone(),
        Kind::Launcher(path) | Kind::File(path) | Kind::Folder(path) => path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
    }
}

/// The desktop directory: XDG's, unless it is the home folder itself (XDG
/// falls back to $HOME for unset directories; GNOME 2 then showed no files).
pub fn desktop_dir(special: Option<PathBuf>, home: &Path) -> Option<PathBuf> {
    special.filter(|dir| dir != home)
}

/// Valid templates: visible regular files directly in the templates
/// directory, by name. None when the directory is unset, the home folder or
/// missing.
pub fn templates(dir: Option<&Path>, home: &Path) -> Vec<PathBuf> {
    let Some(dir) = dir.filter(|dir| *dir != home) else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut list: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .map(|e| e.path())
        .collect();
    list.sort();
    list
}

/// "Name.ext" → ("Name", ".ext"); hidden files and names without a dot
/// have no extension.
pub fn split_extension(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(0) | None => (name, ""),
        Some(i) => (&name[..i], &name[i..]),
    }
}

/// `base` if free, else "stem 2.ext", "stem 3.ext", …: never a name for
/// which `exists` is true, so nothing is overwritten.
pub fn unique_name(base: &str, exists: impl Fn(&str) -> bool) -> String {
    if !exists(base) {
        return base.to_owned();
    }
    let (stem, ext) = split_extension(base);
    (2..)
        .map(|n| format!("{stem} {n}{ext}"))
        .find(|candidate| !exists(candidate))
        .unwrap_or_default()
}

/// `items` in the saved order; items without a saved place keep their
/// natural order (fixed icons, then files by name) after the others.
pub fn ordered(saved: &[String], items: Vec<Item>) -> Vec<Item> {
    let mut natural = by_name(items);
    let mut result = Vec::with_capacity(natural.len());
    for key in saved {
        if let Some(i) = natural.iter().position(|item| &item.key == key) {
            result.push(natural.remove(i));
        }
    }
    result.extend(natural);
    result
}

/// "Nach Namen aufräumen": the fixed icons in their GNOME 2 order, then the
/// rest by name, case-insensitively.
pub fn by_name(mut items: Vec<Item>) -> Vec<Item> {
    items.sort_by(|a, b| match (a.fixed_rank(), b.fixed_rank()) {
        (Some(x), Some(y)) => x.cmp(&y).then_with(|| a.name.cmp(&b.name)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a
            .name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.name.cmp(&b.name)),
    });
    items
}

/// Whether GIO's content type means an empty file.
pub fn is_empty_document(content_type: Option<&str>) -> bool {
    content_type == Some("application/x-zerosize")
}

/// The saved order: one key per line.
pub fn parse_order(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(name: &str) -> Item {
        Item::new(Kind::File(PathBuf::from("/d").join(name)), name)
    }

    #[test]
    fn new_names_never_overwrite() {
        let taken = ["Neuer Ordner", "Neuer Ordner 2", "Notiz.txt", ".hidden"];
        let exists = |n: &str| taken.contains(&n);
        assert_eq!(unique_name("Neues Dokument", exists), "Neues Dokument");
        assert_eq!(unique_name("Neuer Ordner", exists), "Neuer Ordner 3");
        assert_eq!(unique_name("Notiz.txt", exists), "Notiz 2.txt");
        assert_eq!(unique_name(".hidden", exists), ".hidden 2");
        assert_eq!(split_extension("archiv.tar.gz"), ("archiv.tar", ".gz"));
        assert_eq!(split_extension("README"), ("README", ""));
    }

    #[test]
    fn an_empty_file_is_a_blank_document() {
        assert!(is_empty_document(Some("application/x-zerosize")));
        assert!(!is_empty_document(Some("text/plain")));
        assert!(!is_empty_document(None));
    }

    #[test]
    fn the_desktop_directory_is_never_the_home_folder() {
        let home = Path::new("/home/u");
        assert_eq!(
            desktop_dir(Some("/home/u/Desktop".into()), home),
            Some("/home/u/Desktop".into())
        );
        assert_eq!(desktop_dir(Some("/home/u".into()), home), None);
        assert_eq!(desktop_dir(None, home), None);
    }

    #[test]
    fn only_real_template_files_are_offered() {
        let base = std::env::temp_dir().join(format!("hmp09-vorlagen-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("Unterordner")).expect("fixture");
        for name in ["Text.txt", "Brief.odt", ".versteckt"] {
            std::fs::write(base.join(name), "").expect("fixture");
        }
        let home = Path::new("/home/u");
        assert_eq!(
            templates(Some(&base), home),
            [base.join("Brief.odt"), base.join("Text.txt")]
        );
        assert!(templates(Some(home), home).is_empty());
        assert!(templates(None, home).is_empty());
        assert!(templates(Some(&base.join("fehlt")), home).is_empty());
        std::fs::remove_dir_all(&base).expect("cleanup");
    }

    #[test]
    fn arranging_by_name_puts_the_fixed_icons_first() {
        let items = vec![
            file("zebra"),
            Item::new(Kind::Trash, "Papierkorb"),
            file("Apfel"),
            Item::new(Kind::App("t.desktop".into()), "Terminal"),
            Item::new(Kind::Computer, "Rechner"),
            file("birne"),
            Item::new(Kind::Home, "Persönlicher Ordner"),
        ];
        let names: Vec<_> = by_name(items).into_iter().map(|i| i.name).collect();
        assert_eq!(
            names,
            [
                "Rechner",
                "Persönlicher Ordner",
                "Papierkorb",
                "Terminal",
                "Apfel",
                "birne",
                "zebra"
            ]
        );
    }

    #[test]
    fn the_saved_order_wins_and_new_icons_go_last() {
        let items = vec![
            file("a"),
            file("b"),
            Item::new(Kind::Home, "Home"),
            file("neu"),
        ];
        let saved = parse_order("flaeche:b\n\nsonder:home\nflaeche:weg\nflaeche:a\n");
        let keys: Vec<_> = ordered(&saved, items).into_iter().map(|i| i.key).collect();
        assert_eq!(
            keys,
            ["flaeche:b", "sonder:home", "flaeche:a", "flaeche:neu"]
        );
        // nothing saved: natural order
        let keys: Vec<_> = ordered(&[], vec![file("b"), Item::new(Kind::Trash, "P")])
            .into_iter()
            .map(|i| i.key)
            .collect();
        assert_eq!(keys, ["sonder:trash", "flaeche:b"]);
    }

    #[test]
    fn identities_name_kind_and_file() {
        assert_eq!(Item::new(Kind::Computer, "Rechner").key, "sonder:computer");
        assert_eq!(
            Item::new(Kind::App("t.desktop".into()), "T").key,
            "programm:t.desktop"
        );
        let folder = Item::new(Kind::Folder("/d/Neuer Ordner".into()), "Neuer Ordner");
        assert_eq!(folder.key, "flaeche:Neuer Ordner");
        assert_eq!(folder.art(), Art::Folder);
        let launcher = Item::new(Kind::Launcher("/d/b.desktop".into()), "Mein Browser");
        assert_eq!(launcher.key, "flaeche:b.desktop");
        assert_eq!(launcher.name, "Mein Browser");
        assert_eq!(Item::new(Kind::Home, "H").ident(), "home");
    }
}
