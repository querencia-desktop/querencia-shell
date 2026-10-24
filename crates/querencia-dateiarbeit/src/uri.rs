//! URI lists (`text/uri-list`) and the clipboard text of GNOME file
//! managers (`x-special/gnome-copied-files`: `copy` or `cut`, then one URI
//! per line), which Querencia Files, Nautilus, Caja, Nemo and Thunar read
//! and write; KDE marks cut files with its own type beside the URI list.
//! From Desktop 0.2.3 (`fileops.rs`, QD-108).

use gio::prelude::*;

pub const URI_LIST: &str = "text/uri-list";
pub const GNOME_COPIED_FILES: &str = "x-special/gnome-copied-files";
/// KDE's cut marker beside `text/uri-list`; `1`: the files were cut.
pub const KDE_CUT_SELECTION: &str = "application/x-kde-cutselection";

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

/// KDE's cut marker: `1` (cut) or anything else (copied).
pub fn parse_kde_cut_selection(data: &[u8]) -> bool {
    data.first() == Some(&b'1')
}

/// The URI of a local path (`file:///a%20b`).
pub fn of_path(path: &std::path::Path) -> String {
    gio::File::for_path(path).uri().to_string()
}

/// The shown name of the object a URI names: its base name (percent
/// escapes decoded), else the URI itself.
pub fn name_of(uri: &str) -> String {
    gio::File::for_uri(uri)
        .basename()
        .map(|n| n.to_string_lossy().into_owned())
        .filter(|n| !n.is_empty() && n != "/")
        .unwrap_or_else(|| uri.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(parse_kde_cut_selection(b"1"));
        assert!(!parse_kde_cut_selection(b"0") && !parse_kde_cut_selection(b""));
    }

    #[test]
    fn names_and_uris_of_paths() {
        let uri = of_path(std::path::Path::new("/tmp/a b/Grüße.txt"));
        assert_eq!(uri, "file:///tmp/a%20b/Gr%C3%BC%C3%9Fe.txt");
        assert_eq!(name_of(&uri), "Grüße.txt");
        assert_eq!(name_of("file:///"), "file:///");
    }
}
