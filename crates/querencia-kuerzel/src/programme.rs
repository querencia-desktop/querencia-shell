//! The fixed shortcuts of the Querencia programs (data/programme.txt).
//! The file is English (msgid); the window translates names and
//! descriptions with gettext and the keys with the context "key".
//!
//! The file is built into the program; a file of the same name in
//! `$XDG_DATA_HOME/querencia-kuerzel/` or `$XDG_DATA_DIRS/querencia-kuerzel/`
//! takes its place (e.g. for a newer program version).

use std::path::PathBuf;

use crate::i18n::{gettext, gettext_f};

pub const BUILT_IN: &str = include_str!("../data/programme.txt");

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Program {
    pub name: String,
    /// (keys as printed, what they do)
    pub entries: Vec<(String, String)>,
}

/// Reads the format of data/programme.txt; lines it cannot read are
/// returned with their line number.
pub fn parse(text: &str) -> (Vec<Program>, Vec<String>) {
    let mut programs: Vec<Program> = Vec::new();
    let mut problems = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            programs.push(Program {
                name: name.trim().to_owned(),
                entries: Vec::new(),
            });
            continue;
        }
        // the description follows the last " = " ("Eingabe / = = Ergebnis")
        match (line.rsplit_once(" = "), programs.last_mut()) {
            (Some((keys, what)), Some(program))
                if !keys.trim().is_empty() && !what.trim().is_empty() =>
            {
                program
                    .entries
                    .push((keys.trim().to_owned(), what.trim().to_owned()));
            }
            _ => problems.push(format!("Zeile {}: {line}", n + 1)),
        }
    }
    programs.retain(|p| !p.entries.is_empty());
    (programs, problems)
}

/// The file in use: a replacement in the XDG data directories, or the
/// built-in one.
pub fn load() -> (Vec<Program>, String) {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(home) = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
    {
        dirs.push(home);
    } else if let Some(home) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(home).join(".local/share"));
    }
    let system =
        std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".to_owned());
    dirs.extend(
        system
            .split(':')
            .filter(|d| !d.is_empty())
            .map(PathBuf::from),
    );
    for dir in dirs {
        let path = dir.join("querencia-kuerzel").join("programme.txt");
        if let Ok(text) = std::fs::read_to_string(&path) {
            return (
                parse(&text).0,
                gettext_f("Programs: {path}", &[("path", &path.display().to_string())]),
            );
        }
    }
    (parse(BUILT_IN).0, gettext("Programs: built-in list"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_file_is_clean() {
        let (programs, problems) = parse(BUILT_IN);
        assert!(problems.is_empty(), "{problems:?}");
        let names: Vec<&str> = programs.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "Desktop",
                "Files",
                "Querencia Terminal",
                "Text Editor",
                "Image Viewer",
                "Screenshot",
                "Calculator",
                "Search for Files",
                "Keyboard Shortcuts (this window)"
            ]
        );
        // Querencia Dateien (Welle C): 38 keys in the order of its menus,
        // as its menus print them (Shift before Ctrl); the keys that
        // changed against Querencia Files keep their new meaning
        let dateien = &programs[1];
        assert_eq!(dateien.entries.len(), 38);
        let keys: Vec<&str> = dateien.entries.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(
            keys[..5],
            ["Ctrl+T", "Ctrl+N", "Shift+Ctrl+N", "Ctrl+O", "Shift+Ctrl+O"]
        );
        assert_eq!(keys.last(), Some(&"F1"));
        for (key, what) in [
            ("Ctrl+D", "Add Bookmark"),
            ("F9", "Sidebar on or off"),
            ("Ctrl+3", "Compact View"),
            ("Shift+Ctrl+I", "Invert Selection"),
            ("Shift+Delete", "Delete (asks first)"),
            ("Ctrl+W", "Close tab (the last one closes the window)"),
            ("Alt+1 … Alt+0", "Switch to tab 1 … 10"),
        ] {
            assert!(
                dateien.entries.contains(&(key.to_owned(), what.to_owned())),
                "{key}"
            );
        }
        assert!(
            dateien
                .entries
                .iter()
                .any(|(k, w)| k == "Ctrl+F" && w.starts_with("Search"))
        );
        // Files' keys that Dateien does not have
        for gone in ["Ctrl+K", "Ctrl+B", "Ctrl+,", "Ctrl+Z"] {
            assert!(!keys.contains(&gone), "{gone}");
        }
        // Terminal 0.3.0: the tab keys come first, as in its Help → Shortcuts
        let terminal = &programs[2];
        let keys: Vec<&str> = terminal.entries.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(
            keys[..6],
            [
                "Ctrl+Shift+T",
                "Ctrl+Shift+W",
                "Ctrl+Shift+Q",
                "Ctrl+Page Up / Ctrl+Page Down",
                "Ctrl+Shift+Page Up / Ctrl+Shift+Page Down",
                "Alt+1 … Alt+9"
            ]
        );
        assert!(
            terminal
                .entries
                .contains(&("Ctrl+Shift+C".to_owned(), "Copy".to_owned()))
        );
        let calc = &programs[6];
        assert!(
            calc.entries
                .contains(&("Enter / =".to_owned(), "Result".to_owned()))
        );
        // English key names only (msgid; the catalog has the German ones)
        for p in &programs {
            for (keys, _) in &p.entries {
                for german in ["Strg", "Umschalt", "Eingabe", "Entf", "Bild", "Pos1"] {
                    assert!(!keys.contains(german), "{keys}");
                }
            }
        }
    }

    #[test]
    fn broken_lines_are_reported() {
        let (programs, problems) =
            parse("Strg+A = vor jeder Überschrift\n[A]\nohne Gleichheitszeichen\nX = ok\n[Leer]\n");
        assert_eq!(
            programs,
            [Program {
                name: "A".into(),
                entries: vec![("X".into(), "ok".into())]
            }]
        );
        assert_eq!(problems.len(), 2);
        assert!(problems[0].starts_with("Zeile 1"));
    }
}
