//! The remembered arrangement and names of the desktop icons (HMP-10),
//! `$XDG_STATE_HOME/querencia/desktop-symbole` (default `~/.local/state`).
//!
//! A text file, one icon per line, columns separated by tabs (shown here
//! as `→`):
//!
//! ```text
//! version → 1
//! sonder → computer → rechts 8 → oben 8
//! programm → org.example.Querencia.Terminal.desktop → mitte 0 → mitte 0
//! ordner → Projektunterlagen → links 8 → oben 96
//! datei → Brief.txt → links 108 → oben 8
//! starter → Mein Browser.desktop → links 8 → unten 8
//! name → programm → org.example.Querencia.Terminal.desktop → Mein Terminal
//! ```
//!
//! - The kind (`Art`) is one of: `sonder` (Rechner, Persönlicher Ordner,
//!   Papierkorb), `programm` (a built-in Querencia starter, by desktop file
//!   ID), and for the entries of the desktop directory, by file name:
//!   `starter` (a `.desktop` file), `datei`, `ordner`.
//! - The positions: see `layout.rs` (edge and distance, logical pixels).
//! - `name` lines are the user's own names for icons whose name is not a
//!   file name of their own (special icons, built-in starters, starters the
//!   user may not change).
//! - Names are escaped: `%` `\t` `\n` `\r` as `%25` `%09` `%0A` `%0D`.
//! - Unknown or broken lines are skipped and reported, never fatal. A file
//!   of a newer version is read as far as understood but not overwritten.
//!
//! The identity of an icon is its namespace and name: `sonder:computer`,
//! `programm:<id>`, `flaeche:<file name>`; `starter`, `datei` and `ordner`
//! share the namespace `flaeche`, so a file that became a folder keeps its
//! place.

use crate::layout::{Anchor, Axis, Place};

pub const VERSION: u32 = 1;
pub const FILE_NAME: &str = "desktop-symbole";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Art {
    Special,
    Program,
    Launcher,
    File,
    Folder,
}

impl Art {
    pub fn word(self) -> &'static str {
        match self {
            Art::Special => "sonder",
            Art::Program => "programm",
            Art::Launcher => "starter",
            Art::File => "datei",
            Art::Folder => "ordner",
        }
    }

    fn from_word(word: &str) -> Option<Self> {
        [
            Art::Special,
            Art::Program,
            Art::Launcher,
            Art::File,
            Art::Folder,
        ]
        .into_iter()
        .find(|art| art.word() == word)
    }

    pub fn namespace(self) -> &'static str {
        match self {
            Art::Special => "sonder",
            Art::Program => "programm",
            Art::Launcher | Art::File | Art::Folder => "flaeche",
        }
    }
}

/// The identity of an icon (see the module documentation).
pub fn key(art: Art, name: &str) -> String {
    format!("{}:{name}", art.namespace())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub art: Art,
    pub name: String,
    pub place: Place,
}

impl Entry {
    pub fn key(&self) -> String {
        key(self.art, &self.name)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Alias {
    pub art: Art,
    pub name: String,
    pub label: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Store {
    pub entries: Vec<Entry>,
    pub aliases: Vec<Alias>,
    /// Written by a newer Querencia: do not overwrite.
    pub newer: Option<u32>,
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '%' => out.push_str("%25"),
            '\t' => out.push_str("%09"),
            '\n' => out.push_str("%0A"),
            '\r' => out.push_str("%0D"),
            c => out.push(c),
        }
    }
    out
}

fn unescape(text: &str) -> Option<String> {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find('%') {
        out.push_str(&rest[..i]);
        let code = rest.get(i + 1..i + 3)?;
        out.push(match code {
            "25" => '%',
            "09" => '\t',
            "0A" | "0a" => '\n',
            "0D" | "0d" => '\r',
            _ => return None,
        });
        rest = &rest[i + 3..];
    }
    out.push_str(rest);
    Some(out)
}

const X_WORDS: [&str; 3] = ["links", "mitte", "rechts"];
const Y_WORDS: [&str; 3] = ["oben", "mitte", "unten"];
const ANCHORS: [Anchor; 3] = [Anchor::Start, Anchor::Center, Anchor::End];

fn render_axis(axis: Axis, words: [&str; 3]) -> String {
    let i = ANCHORS.iter().position(|a| *a == axis.anchor).unwrap_or(0);
    format!("{} {}", words[i], axis.offset)
}

fn parse_axis(text: &str, words: [&str; 3]) -> Option<Axis> {
    let (word, number) = text.trim().split_once(' ')?;
    let anchor = ANCHORS[words.iter().position(|w| *w == word)?];
    let offset: i32 = number.trim().parse().ok()?;
    // anything beyond a few screens is a broken line, not a position
    (offset.abs() <= 100_000).then_some(Axis { anchor, offset })
}

impl Store {
    pub fn parse(text: &str) -> (Self, Vec<String>) {
        let mut store = Store::default();
        let mut problems = Vec::new();
        let mut version = None;
        for (number, line) in text.lines().enumerate() {
            let line = line.trim_end_matches('\r');
            if line.trim().is_empty() || line.starts_with('#') {
                continue;
            }
            let fields: Vec<&str> = line.split('\t').collect();
            let problem = |what: &str| format!("Zeile {}: {what}", number + 1);
            match fields.as_slice() {
                ["version", v] => match v.trim().parse::<u32>() {
                    Ok(v) => version = Some(v),
                    Err(_) => problems.push(problem("Version unlesbar")),
                },
                ["name", art, name, label] => {
                    match (Art::from_word(art), unescape(name), unescape(label)) {
                        (Some(art), Some(name), Some(label)) if !label.trim().is_empty() => {
                            store.set_alias(art, &name, Some(&label));
                        }
                        _ => problems.push(problem("Name unlesbar")),
                    }
                }
                [art, name, x, y] => match (
                    Art::from_word(art),
                    unescape(name),
                    parse_axis(x, X_WORDS),
                    parse_axis(y, Y_WORDS),
                ) {
                    (Some(art), Some(name), Some(x), Some(y)) if !name.is_empty() => {
                        store.set_place(art, &name, Place { x, y });
                    }
                    _ => problems.push(problem("Lage unlesbar")),
                },
                _ => problems.push(problem("unbekannter Eintrag")),
            }
        }
        match version {
            Some(VERSION) => {}
            Some(v) if v > VERSION => {
                problems.push(format!(
                    "Version {v} ist neuer als {VERSION}: wird gelesen, aber nicht überschrieben"
                ));
                store.newer = Some(v);
            }
            Some(v) => problems.push(format!("unbekannte Version {v}")),
            None if text.trim().is_empty() => {}
            None => problems.push("keine Versionszeile".to_owned()),
        }
        (store, problems)
    }

    pub fn render(&self) -> String {
        let mut out = String::from(
            "# Querencia Desktop: Lage und Namen der Arbeitsflächen-Symbole.\n\
             # Geschrieben vom Desktop; Spalten durch Tabulatoren getrennt.\n\
             # <Art> <Name> <waagrecht> <senkrecht>   Art: sonder programm starter datei ordner\n\
             # name <Art> <Name> <Anzeigename>\n",
        );
        out.push_str(&format!("version\t{VERSION}\n"));
        for entry in &self.entries {
            out.push_str(&format!(
                "{}\t{}\t{}\t{}\n",
                entry.art.word(),
                escape(&entry.name),
                render_axis(entry.place.x, X_WORDS),
                render_axis(entry.place.y, Y_WORDS)
            ));
        }
        for alias in &self.aliases {
            out.push_str(&format!(
                "name\t{}\t{}\t{}\n",
                alias.art.word(),
                escape(&alias.name),
                escape(&alias.label)
            ));
        }
        out
    }

    pub fn place(&self, key: &str) -> Option<Place> {
        self.entries
            .iter()
            .find(|e| e.key() == key)
            .map(|e| e.place)
    }

    /// Sets the place of an icon; the kind is updated too (a file that
    /// became a folder).
    pub fn set_place(&mut self, art: Art, name: &str, place: Place) {
        let k = key(art, name);
        match self.entries.iter_mut().find(|e| e.key() == k) {
            Some(entry) => {
                entry.art = art;
                entry.place = place;
            }
            None => self.entries.push(Entry {
                art,
                name: name.to_owned(),
                place,
            }),
        }
    }

    pub fn alias(&self, key: &str) -> Option<&str> {
        self.aliases
            .iter()
            .find(|a| self::key(a.art, &a.name) == key)
            .map(|a| a.label.as_str())
    }

    /// The user's own name for an icon; `None` removes it.
    pub fn set_alias(&mut self, art: Art, name: &str, label: Option<&str>) {
        let k = key(art, name);
        self.aliases.retain(|a| key(a.art, &a.name) != k);
        if let Some(label) = label {
            self.aliases.push(Alias {
                art,
                name: name.to_owned(),
                label: label.to_owned(),
            });
        }
    }

    /// A desktop directory entry was renamed from `old` to `new`: its
    /// place and name go with it. An entry for `new` (left over) is
    /// replaced.
    pub fn rename(&mut self, old: &str, new: &str) {
        let (old_key, new_key) = (key(Art::File, old), key(Art::File, new));
        self.entries.retain(|e| e.key() != new_key);
        for entry in &mut self.entries {
            if entry.key() == old_key {
                entry.name = new.to_owned();
            }
        }
        self.aliases.retain(|a| key(a.art, &a.name) != new_key);
        for alias in &mut self.aliases {
            if key(alias.art, &alias.name) == old_key {
                alias.name = new.to_owned();
            }
        }
    }

    /// Forgets desktop directory entries that are gone (`present` false);
    /// special icons and built-in starters are kept even while missing.
    pub fn forget_missing(&mut self, present: impl Fn(&str) -> bool) -> bool {
        let before = (self.entries.len(), self.aliases.len());
        self.entries
            .retain(|e| e.art.namespace() != "flaeche" || present(&e.name));
        self.aliases
            .retain(|a| a.art.namespace() != "flaeche" || present(&a.name));
        before != (self.entries.len(), self.aliases.len())
    }
}

/// HMP-09's `desktop-order` keys (`special:`, `app:`, `file:`) as HMP-10
/// identities, for the first start after HMP-09.
pub fn migrate_order_key(old: &str) -> Option<String> {
    let (kind, name) = old.split_once(':')?;
    let namespace = match kind {
        "special" => "sonder",
        "app" => "programm",
        "file" => "flaeche",
        _ => return None,
    };
    Some(format!("{namespace}:{name}"))
}

/// The HMP-10 identity as an HMP-09 `desktop-order` key, so that a return
/// to HMP-09 finds the icons in about the same order.
pub fn hmp09_order_key(key: &str) -> Option<String> {
    let (namespace, name) = key.split_once(':')?;
    let kind = match namespace {
        "sonder" => "special",
        "programm" => "app",
        "flaeche" => "file",
        _ => return None,
    };
    Some(format!("{kind}:{name}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn place(x: (Anchor, i32), y: (Anchor, i32)) -> Place {
        Place {
            x: Axis {
                anchor: x.0,
                offset: x.1,
            },
            y: Axis {
                anchor: y.0,
                offset: y.1,
            },
        }
    }

    #[test]
    fn the_file_reads_back_what_was_written() {
        let mut store = Store::default();
        store.set_place(
            Art::Special,
            "computer",
            place((Anchor::End, 8), (Anchor::Start, 8)),
        );
        store.set_place(
            Art::Folder,
            "Projekt\tunterlagen 100%",
            place((Anchor::Center, -40), (Anchor::End, 8)),
        );
        store.set_place(
            Art::Launcher,
            "Mein Browser.desktop",
            place((Anchor::Start, 8), (Anchor::Start, 96)),
        );
        store.set_alias(
            Art::Program,
            "org.example.Querencia.Terminal.desktop",
            Some("Mein Terminal"),
        );
        let text = store.render();
        assert!(text.contains("\nversion\t1\n"));
        assert!(text.contains("sonder\tcomputer\trechts 8\toben 8\n"));
        assert!(text.contains("ordner\tProjekt%09unterlagen 100%25\tmitte -40\tunten 8\n"));
        let (back, problems) = Store::parse(&text);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(back, store);
    }

    #[test]
    fn broken_lines_are_skipped_not_fatal() {
        let text = "version\t1\n\
                    sonder\tcomputer\trechts 8\toben 8\n\
                    ordner\tKaputt\tquer 8\toben 8\n\
                    datei\t\tlinks 8\toben 8\n\
                    irgendwas\n\
                    datei\tZebra.txt\tlinks 8\toben 999999999\n\
                    datei\tBrief.txt\tlinks 108\toben 8\n";
        let (store, problems) = Store::parse(text);
        assert_eq!(problems.len(), 4, "{problems:?}");
        let keys: Vec<_> = store.entries.iter().map(Entry::key).collect();
        assert_eq!(keys, ["sonder:computer", "flaeche:Brief.txt"]);
        assert_eq!(store.newer, None);
        // nothing at all is fine too
        assert_eq!(Store::parse(""), (Store::default(), vec![]));
    }

    #[test]
    fn a_newer_file_is_not_overwritten() {
        let (store, problems) = Store::parse("version\t7\nsonder\thome\tlinks 8\toben 8\n");
        assert_eq!(store.newer, Some(7));
        assert_eq!(store.entries.len(), 1);
        assert_eq!(problems.len(), 1);
    }

    #[test]
    fn kinds_of_the_desktop_directory_share_one_identity() {
        assert_eq!(key(Art::File, "x"), "flaeche:x");
        assert_eq!(key(Art::Folder, "x"), "flaeche:x");
        assert_eq!(key(Art::Launcher, "x"), "flaeche:x");
        assert_eq!(key(Art::Special, "trash"), "sonder:trash");
        let mut store = Store::default();
        let p = place((Anchor::Start, 8), (Anchor::Start, 8));
        store.set_place(Art::File, "x", p);
        store.set_place(Art::Folder, "x", p);
        assert_eq!(store.entries.len(), 1);
        assert_eq!(store.entries[0].art, Art::Folder);
    }

    #[test]
    fn renaming_takes_place_and_name_along() {
        let mut store = Store::default();
        let here = place((Anchor::End, 8), (Anchor::End, 8));
        store.set_place(Art::Folder, "Neuer Ordner", here);
        store.set_place(
            Art::File,
            "Projektunterlagen",
            place((Anchor::Start, 0), (Anchor::Start, 0)),
        );
        store.set_alias(Art::Launcher, "Neuer Ordner", Some("x"));
        store.rename("Neuer Ordner", "Projektunterlagen");
        assert_eq!(store.place("flaeche:Projektunterlagen"), Some(here));
        assert_eq!(store.place("flaeche:Neuer Ordner"), None);
        assert_eq!(store.entries.len(), 1, "alter Eintrag des Ziels ersetzt");
        assert_eq!(store.alias("flaeche:Projektunterlagen"), Some("x"));
    }

    #[test]
    fn missing_files_are_forgotten_special_icons_are_not() {
        let mut store = Store::default();
        let p = place((Anchor::Start, 8), (Anchor::Start, 8));
        store.set_place(Art::Special, "trash", p);
        store.set_place(Art::Program, "t.desktop", p);
        store.set_place(Art::File, "weg.txt", p);
        store.set_place(Art::File, "da.txt", p);
        assert!(store.forget_missing(|name| name == "da.txt"));
        let keys: Vec<_> = store.entries.iter().map(Entry::key).collect();
        assert_eq!(
            keys,
            ["sonder:trash", "programm:t.desktop", "flaeche:da.txt"]
        );
        assert!(!store.forget_missing(|name| name == "da.txt"));
    }

    #[test]
    fn aliases_are_set_and_removed() {
        let mut store = Store::default();
        store.set_alias(Art::Special, "computer", Some("Arbeitsplatz"));
        store.set_alias(Art::Special, "computer", Some("Mein Rechner"));
        assert_eq!(store.alias("sonder:computer"), Some("Mein Rechner"));
        assert_eq!(store.aliases.len(), 1);
        store.set_alias(Art::Special, "computer", None);
        assert_eq!(store.alias("sonder:computer"), None);
    }

    #[test]
    fn hmp_09_order_keys_carry_over() {
        assert_eq!(
            migrate_order_key("special:computer").as_deref(),
            Some("sonder:computer")
        );
        assert_eq!(
            migrate_order_key("app:t.desktop").as_deref(),
            Some("programm:t.desktop")
        );
        assert_eq!(
            migrate_order_key("file:Neuer Ordner").as_deref(),
            Some("flaeche:Neuer Ordner")
        );
        assert_eq!(migrate_order_key("quatsch"), None);
        assert_eq!(hmp09_order_key("flaeche:a:b").as_deref(), Some("file:a:b"));
    }
}
