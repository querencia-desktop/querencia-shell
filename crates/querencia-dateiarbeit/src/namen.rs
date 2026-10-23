//! Names: for copies and new objects (never one that exists), checks of
//! file names, and the visible name of starters (`.desktop` files).
//!
//! From Desktop 0.2.3 (`fileops.rs`: names for copies), HMP-09
//! (`icons.rs`: free names for new objects) and HMP-10 (`rename.rs`:
//! checks, `Name[xx]` of a starter; every other byte of the file stays).

use crate::i18n::{gettext, gettext_f};

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

/// The name for a copy of `name` where `exists` says which names are taken:
/// `name` itself if free, else "stem (copy).ext", "stem (copy 2).ext", …
/// (German "Kopie" from the catalog). A folder keeps a dot in its name
/// ("a.b (copy)").
pub fn copy_name(name: &str, is_dir: bool, exists: impl Fn(&str) -> bool) -> String {
    if !exists(name) {
        return name.to_owned();
    }
    let (stem, ext) = if is_dir {
        (name, "")
    } else {
        split_extension(name)
    };
    // TRANSLATORS: name of a copy next to the original, e.g. "notes (copy).txt"
    std::iter::once(gettext_f(
        "{stem} (copy){ext}",
        &[("stem", stem), ("ext", ext)],
    ))
    .chain((2..).map(|n: u32| {
        // TRANSLATORS: the second, third … copy, e.g. "notes (copy 2).txt"
        gettext_f(
            "{stem} (copy {n}){ext}",
            &[("stem", stem), ("n", &n.to_string()), ("ext", ext)],
        )
    }))
    .find(|candidate| !exists(candidate))
    .unwrap_or_default()
}

/// The name of a link to `name`: "Link to name", then "Link to name 2", …
/// (GNOME 2's wording; German from the catalog).
pub fn link_name(name: &str, exists: impl Fn(&str) -> bool) -> String {
    // TRANSLATORS: name of a symbolic link, as Nautilus 2.30 named it
    let base = gettext_f("Link to {name}", &[("name", name)]);
    unique_name(&base, exists)
}

/// The hidden name under which a new object is built in its folder before
/// it gets its real name in one atomic step: a partial object never has the
/// real name, and its name says what it is (`auftraege`).
pub fn build_name(name: &str, job: u64, serial: u64) -> String {
    format!(
        ".{name}.querencia-teil-{}-{job}-{serial}",
        std::process::id()
    )
}

/// Whether `name` is a build name of [`build_name`] (left behind only when
/// the clean-up after a cancel failed too).
pub fn is_build_name(name: &str) -> bool {
    name.starts_with('.') && name.contains(".querencia-teil-")
}

/// Why a name cannot be a file name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Problem {
    Empty,
    Slash,
    Dots,
    Nul,
}

impl Problem {
    /// The explanation shown to the user (GNOME 2's wording).
    pub fn text(self, name: &str) -> String {
        match self {
            Problem::Empty => gettext("The name cannot be empty."),
            Problem::Slash => gettext_f(
                "The name “{name}” is not valid because it contains the character “/”. \
                 Please use a different name.",
                &[("name", name)],
            ),
            Problem::Dots => gettext_f(
                "“{name}” is not a valid file name. Please use a different name.",
                &[("name", name)],
            ),
            Problem::Nul => gettext("The name contains an invalid character."),
        }
    }
}

/// Whether `name` can be the name of a file in a folder.
pub fn check_file_name(name: &str) -> Result<(), Problem> {
    if name.trim().is_empty() {
        Err(Problem::Empty)
    } else if name.contains('/') {
        Err(Problem::Slash)
    } else if name == "." || name == ".." {
        Err(Problem::Dots)
    } else if name.contains('\0') {
        Err(Problem::Nul)
    } else {
        Ok(())
    }
}

/// Whether `name` can be the visible name of a starter (any text but an
/// empty one).
pub fn check_label(name: &str) -> Result<(), Problem> {
    if name.trim().is_empty() {
        Err(Problem::Empty)
    } else if name.contains('\0') {
        Err(Problem::Nul)
    } else {
        Ok(())
    }
}

/// How many characters of `name` the rename box selects at first: the name
/// without its extension for files ("Brief" of "Brief.txt", as Nautilus),
/// everything for folders, starters and hidden files without extension.
pub fn selected_chars(name: &str, is_file: bool) -> usize {
    let all = name.chars().count();
    if !is_file {
        return all;
    }
    match name.rfind('.') {
        Some(0) | None => all,
        Some(i) => name[..i].chars().count(),
    }
}

/// A desktop entry string value, escaped (`\s` for a leading space, `\n`,
/// `\t`, `\r`, `\\`; Desktop Entry Specification, "Possible value types").
pub fn escape_value(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for (i, c) in value.chars().enumerate() {
        match c {
            ' ' if i == 0 => out.push_str("\\s"),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out
}

/// The `Name` key the desktop shows for `locale` (as GLib's
/// `g_key_file_get_locale_for_key` reports it): `Name[de]` or `Name`.
pub fn name_key(locale: Option<&str>) -> String {
    match locale {
        Some(locale) if !locale.is_empty() => format!("Name[{locale}]"),
        _ => "Name".to_owned(),
    }
}

/// `text` (a desktop entry) with the value of `key` in the
/// `[Desktop Entry]` group set to `value`; every other line, comment and
/// line ending stays byte for byte. A missing key is added after the
/// group's last `Name` line (or its header). `None` without a
/// `[Desktop Entry]` group.
pub fn with_name(text: &str, key: &str, value: &str) -> Option<String> {
    let escaped = escape_value(value);
    let mut out = String::with_capacity(text.len() + escaped.len());
    let mut in_group = false;
    let mut seen_group = false;
    let mut replaced = false;
    // where to insert the key if the group has none: after this many bytes
    let mut insert_at: Option<usize> = None;
    let mut newline = "\n";
    for line in text.split_inclusive('\n') {
        let body = line.trim_end_matches(['\n', '\r']);
        let ending = &line[body.len()..];
        if ending == "\r\n" {
            newline = "\r\n";
        }
        let trimmed = body.trim_start();
        if trimmed.starts_with('[') {
            in_group = trimmed.trim_end() == "[Desktop Entry]";
            if in_group {
                seen_group = true;
                out.push_str(line);
                if insert_at.is_none() {
                    insert_at = Some(out.len());
                }
                continue;
            }
        } else if in_group
            && !replaced
            && !trimmed.starts_with('#')
            && let Some((k, _)) = body.split_once('=')
        {
            let k = k.trim();
            if k == key {
                // keep "Name=" or "Name = " as it was written
                let eq = body.find('=').unwrap_or(0);
                let after = &body[eq + 1..];
                let spaces = after.len() - after.trim_start_matches([' ', '\t']).len();
                out.push_str(&body[..eq + 1 + spaces]);
                out.push_str(&escaped);
                out.push_str(ending);
                replaced = true;
                continue;
            }
            if k == "Name" || k.starts_with("Name[") {
                out.push_str(line);
                insert_at = Some(out.len());
                continue;
            }
        }
        out.push_str(line);
    }
    if !seen_group {
        return None;
    }
    if !replaced {
        let at = insert_at.unwrap_or(out.len());
        let mut line = String::new();
        if at > 0 && !out[..at].ends_with('\n') {
            line.push_str(newline);
        }
        line.push_str(&format!("{key}={escaped}{newline}"));
        out.insert_str(at, &line);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENTRY: &str = "[Desktop Entry]\n\
        # Kommentar bleibt\n\
        Type=Application\n\
        Name=My Browser\n\
        Name[de]=Mein Browser\n\
        Name[fr]=Mon navigateur\n\
        GenericName[de]=Webbrowser\n\
        Exec=env A=1 firefox %u\n\
        TryExec=firefox\n\
        Icon=firefox\n\
        Terminal=false\n\
        MimeType=text/html;x-scheme-handler/http;\n\
        X-Eigenes=bleibt\n\
        \n\
        [Desktop Action new-window]\n\
        Name=New Window\n\
        Exec=firefox --new-window\n";

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
    fn copies_get_a_free_name_and_keep_the_extension() {
        let taken = ["notiz.txt", "notiz (copy).txt", "Ordner.alt", ".profil"];
        let exists = |n: &str| taken.contains(&n);
        assert_eq!(copy_name("neu.txt", false, exists), "neu.txt");
        assert_eq!(copy_name("notiz.txt", false, exists), "notiz (copy 2).txt");
        assert_eq!(copy_name("Ordner.alt", true, exists), "Ordner.alt (copy)");
        assert_eq!(copy_name(".profil", false, exists), ".profil (copy)");
        assert_eq!(copy_name("a.txt", false, |n| n == "a.txt"), "a (copy).txt");
    }

    #[test]
    fn links_and_build_names() {
        assert_eq!(link_name("Bilder", |_| false), "Link to Bilder");
        assert_eq!(
            link_name("a.txt", |n| n == "Link to a.txt"),
            "Link to a 2.txt"
        );
        let build = build_name("Brief.txt", 7, 3);
        assert!(build.starts_with(".Brief.txt.querencia-teil-"));
        assert!(build.ends_with("-7-3"));
        assert!(is_build_name(&build));
        assert!(!is_build_name("Brief.txt") && !is_build_name(".bashrc"));
    }

    #[test]
    fn file_names_are_checked() {
        assert_eq!(check_file_name("Projektunterlagen"), Ok(()));
        assert_eq!(check_file_name("Übersicht für Jörg – ß.txt"), Ok(()));
        assert_eq!(check_file_name(".versteckt"), Ok(()));
        assert_eq!(check_file_name(""), Err(Problem::Empty));
        assert_eq!(check_file_name("   "), Err(Problem::Empty));
        assert_eq!(check_file_name("a/b"), Err(Problem::Slash));
        assert_eq!(check_file_name("/"), Err(Problem::Slash));
        assert_eq!(check_file_name(".."), Err(Problem::Dots));
        assert_eq!(check_file_name("."), Err(Problem::Dots));
        assert_eq!(check_label("Internet/Web"), Ok(()));
        assert_eq!(check_label(" "), Err(Problem::Empty));
        // without a catalog (tests): the English msgid
        assert!(Problem::Slash.text("a/b").contains("“a/b”"));
    }

    #[test]
    fn the_extension_stays_unselected() {
        assert_eq!(selected_chars("Brief.txt", true), 5);
        assert_eq!(selected_chars("Grüße.tar.gz", true), 9);
        assert_eq!(selected_chars("README", true), 6);
        assert_eq!(selected_chars(".bashrc", true), 7);
        assert_eq!(selected_chars("Ordner.alt", false), 10);
    }

    #[test]
    fn only_the_shown_name_changes() {
        let new = with_name(ENTRY, "Name[de]", "Internet").expect("Gruppe");
        let expected = ENTRY.replace("Name[de]=Mein Browser", "Name[de]=Internet");
        assert_eq!(new, expected);
        // the action's Name and every other field are untouched
        assert!(new.contains("[Desktop Action new-window]\nName=New Window\n"));
        assert!(new.contains("Exec=env A=1 firefox %u\n"));
        let new = with_name(ENTRY, "Name", "Internet").expect("Gruppe");
        assert_eq!(new, ENTRY.replace("Name=My Browser", "Name=Internet"));
    }

    #[test]
    fn a_missing_translation_is_added_after_the_names() {
        let new = with_name(ENTRY, "Name[es]", "Navegador").expect("Gruppe");
        assert_eq!(
            new,
            ENTRY.replace(
                "Name[fr]=Mon navigateur\n",
                "Name[fr]=Mon navigateur\nName[es]=Navegador\n"
            )
        );
    }

    #[test]
    fn spacing_line_endings_and_escapes_are_kept() {
        let text = "[Desktop Entry]\r\nName = Alt\r\nExec=x\r\n";
        assert_eq!(
            with_name(text, "Name", " Neu\\Name").as_deref(),
            Some("[Desktop Entry]\r\nName = \\sNeu\\\\Name\r\nExec=x\r\n")
        );
        let text = "[Desktop Entry]\nExec=x";
        assert_eq!(
            with_name(text, "Name", "N").as_deref(),
            Some("[Desktop Entry]\nName=N\nExec=x")
        );
        assert_eq!(with_name("[Other]\nName=x\n", "Name", "y"), None);
    }

    #[test]
    fn umlauts_survive() {
        let new = with_name(ENTRY, "Name[de]", "Größere Übersicht").expect("Gruppe");
        assert!(new.contains("Name[de]=Größere Übersicht\n"));
        assert_eq!(name_key(Some("de")), "Name[de]");
        assert_eq!(name_key(Some("de_DE")), "Name[de_DE]");
        assert_eq!(name_key(None), "Name");
    }

    #[test]
    fn glib_reads_back_the_new_name() {
        let new = with_name(ENTRY, "Name[de]", "Internet").expect("Gruppe");
        let file = glib::KeyFile::new();
        file.load_from_data(&new, glib::KeyFileFlags::KEEP_TRANSLATIONS)
            .expect("gültige Desktop-Datei");
        let group = "Desktop Entry";
        assert_eq!(
            file.locale_string(group, "Name", Some("de"))
                .as_deref()
                .ok(),
            Some("Internet")
        );
        assert_eq!(
            file.locale_for_key(group, "Name", Some("de")).as_deref(),
            Some("de")
        );
        assert_eq!(file.locale_for_key(group, "Name", Some("it")), None);
        for (key, value) in [
            ("Exec", "env A=1 firefox %u"),
            ("TryExec", "firefox"),
            ("Icon", "firefox"),
            ("Terminal", "false"),
            ("MimeType", "text/html;x-scheme-handler/http;"),
            ("X-Eigenes", "bleibt"),
        ] {
            assert_eq!(
                file.string(group, key).as_deref().ok(),
                Some(value),
                "{key}"
            );
        }
        let spaced = with_name(ENTRY, "Name", " a\\b").expect("Gruppe");
        file.load_from_data(&spaced, glib::KeyFileFlags::NONE)
            .expect("gültig");
        assert_eq!(file.string(group, "Name").as_deref().ok(), Some(" a\\b"));
    }
}
