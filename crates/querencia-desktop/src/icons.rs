//! Desktop icons (HMP-09), the parts without a display: which icons there
//! are, their order and grid, and names for new folders and documents.
//!
//! The icons, as on a GNOME 2 desktop:
//! - Rechner, Persönlicher Ordner, Papierkorb: only when Querencia Files
//!   (or, for the home folder, any folder program) can open them;
//! - Querencia Terminal: only when its desktop entry is installed;
//! - every file and folder in the XDG desktop directory.
//!
//! The order is the order icons appeared (kept in the state file
//! `desktop-order`); new icons go to the end. "Nach Namen aufräumen" sorts
//! by name: the fixed icons first, then everything else by name. This
//! changes only the layout, never a file.

use std::path::{Path, PathBuf};

/// What an icon stands for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Computer,
    Home,
    Trash,
    /// An application by its desktop file ID.
    App(String),
    /// A file or folder in the desktop directory.
    File(PathBuf),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    /// Stable key for the saved order.
    pub key: String,
    pub name: String,
    pub kind: Kind,
}

impl Item {
    pub fn new(kind: Kind, name: &str) -> Self {
        let key = match &kind {
            Kind::Computer => "special:computer".to_owned(),
            Kind::Home => "special:home".to_owned(),
            Kind::Trash => "special:trash".to_owned(),
            Kind::App(id) => format!("app:{id}"),
            Kind::File(path) => format!(
                "file:{}",
                path.file_name()
                    .map(|n| n.to_string_lossy())
                    .unwrap_or_default()
            ),
        };
        Self {
            key,
            name: name.to_owned(),
            kind,
        }
    }

    fn fixed_rank(&self) -> Option<usize> {
        match self.kind {
            Kind::Computer => Some(0),
            Kind::Home => Some(1),
            Kind::Trash => Some(2),
            Kind::App(_) => Some(3),
            Kind::File(_) => None,
        }
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

/// Grid cell of icon `index`: columns from the left, each filled from the
/// top, as GNOME 2 arranges desktop icons.
pub fn cell(index: usize, rows: usize) -> (usize, usize) {
    let rows = rows.max(1);
    (index / rows, index % rows)
}

/// How many icons fit below each other in `height` pixels.
pub fn rows_for(height: i32, margin: i32, cell_height: i32) -> usize {
    usize::try_from((height - 2 * margin) / cell_height.max(1))
        .unwrap_or(0)
        .max(1)
}

/// The index of the icon whose cell contains `(x, y)`, if any.
pub fn hit(
    count: usize,
    rows: usize,
    origin: (f64, f64),
    size: (f64, f64),
    point: (f64, f64),
) -> Option<usize> {
    let (dx, dy) = (point.0 - origin.0, point.1 - origin.1);
    if dx < 0.0 || dy < 0.0 {
        return None;
    }
    let (col, row) = ((dx / size.0) as usize, (dy / size.1) as usize);
    let rows = rows.max(1);
    if row >= rows {
        return None;
    }
    let index = col * rows + row;
    (index < count).then_some(index)
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
        let saved = parse_order("file:b\n\nspecial:home\nfile:weg\nfile:a\n");
        let keys: Vec<_> = ordered(&saved, items).into_iter().map(|i| i.key).collect();
        assert_eq!(keys, ["file:b", "special:home", "file:a", "file:neu"]);
        // nothing saved: natural order
        let keys: Vec<_> = ordered(&[], vec![file("b"), Item::new(Kind::Trash, "P")])
            .into_iter()
            .map(|i| i.key)
            .collect();
        assert_eq!(keys, ["special:trash", "file:b"]);
    }

    #[test]
    fn icons_fill_columns_from_the_top_left() {
        assert_eq!(rows_for(658, 8, 88), 7);
        assert_eq!(rows_for(50, 8, 88), 1);
        assert_eq!(cell(0, 7), (0, 0));
        assert_eq!(cell(6, 7), (0, 6));
        assert_eq!(cell(7, 7), (1, 0));
        let (origin, size) = ((8.0, 8.0), (96.0, 88.0));
        assert_eq!(hit(9, 7, origin, size, (10.0, 10.0)), Some(0));
        assert_eq!(hit(9, 7, origin, size, (110.0, 100.0)), Some(8));
        assert_eq!(hit(9, 7, origin, size, (110.0, 190.0)), None, "freie Zelle");
        assert_eq!(hit(9, 7, origin, size, (4.0, 10.0)), None, "Rand");
        assert_eq!(
            hit(9, 7, origin, size, (10.0, 8.0 + 7.0 * 88.0 + 1.0)),
            None,
            "unter der letzten Zeile"
        );
    }
}
