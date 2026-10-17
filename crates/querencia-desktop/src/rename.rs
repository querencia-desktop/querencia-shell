//! Renaming desktop icons (HMP-10), the parts without a display.
//!
//! - Files and folders of the desktop directory are renamed in the file
//!   system (GIO `set_display_name`, which refuses an existing name);
//!   [`check_file_name`] rejects what can never be a file name first.
//! - A starter (`.desktop` file) of the user's own changes its visible name
//!   only: the one `Name` line the desktop shows (`Name[de]` when that is
//!   what the current language uses, else `Name`), every other byte of
//!   the file stays as it is ([`with_name`]).
//! - Special icons, built-in starters and starters the user may not change
//!   get a name of the user's own in the state file (`store.rs`); their
//!   function never depends on the name.

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
            Problem::Empty => "Der Name darf nicht leer sein.".to_owned(),
            Problem::Slash => format!(
                "Der Name „{name}“ ist ungültig, weil er das Zeichen „/“ enthält. \
                 Bitte verwenden Sie einen anderen Namen."
            ),
            Problem::Dots => format!(
                "„{name}“ ist kein gültiger Dateiname. Bitte verwenden Sie einen anderen Namen."
            ),
            Problem::Nul => "Der Name enthält ein ungültiges Zeichen.".to_owned(),
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
        assert!(Problem::Slash.text("a/b").contains("„a/b“"));
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
        let file = gtk4::glib::KeyFile::new();
        file.load_from_data(&new, gtk4::glib::KeyFileFlags::KEEP_TRANSLATIONS)
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
        file.load_from_data(&spaced, gtk4::glib::KeyFileFlags::NONE)
            .expect("gültig");
        assert_eq!(file.string(group, "Name").as_deref().ok(), Some(" a\\b"));
    }
}
