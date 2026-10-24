//! Types, icons and sizes of files, as GNOME 2 showed them.
//!
//! From Desktop 0.2.2 (`icons.rs`: an empty file opens by its name) and the
//! desktop's `icon_for` (an empty file is a blank page, a starter shows its
//! own icon) and `fileops.rs` (`tally`, `size_text`).

use std::path::Path;

use gio::prelude::*;
use gio_unix::DesktopAppInfo;

use crate::i18n::{gettext_f, pgettext};

/// GIO's type of an empty file; no program takes it.
pub const EMPTY: &str = "application/x-zerosize";
/// GIO's type of a starter.
pub const DESKTOP_ENTRY: &str = "application/x-desktop";
/// Query these with a folder listing, so that [`icon_for`] decides without
/// a second query per file.
pub const ICON_ATTRIBUTES: &str = "standard::icon,standard::content-type,standard::type";

/// Whether GIO's content type means an empty file.
pub fn is_empty_document(content_type: Option<&str>) -> bool {
    content_type == Some(EMPTY)
}

/// Desktop 0.2.2: the type a file opens as when no program handles GIO's
/// own type for it. An empty file is `application/x-zerosize`, which no
/// program takes; as Nautilus does, the file name decides then
/// (`leer.txt` → `text/plain`). `None` when the name says nothing
/// either (no or an unknown extension): then nothing is started.
pub fn type_from_name(file_name: &str) -> Option<String> {
    let (guess, _) = gio::content_type_guess(Some(file_name), None);
    (!gio::content_type_is_unknown(&guess) && !is_empty_document(Some(&guess)))
        .then(|| guess.to_string())
}

fn themed(name: &str) -> gio::Icon {
    gio::ThemedIcon::new(name).upcast()
}

/// The icon of a file by GNOME 2's rules: an empty file is a blank page (not
/// the theme's grey "zero size" picture), else GIO's icon, else a blank page.
pub fn icon_for_info(info: &gio::FileInfo) -> gio::Icon {
    if is_empty_document(info.content_type().as_deref()) {
        return themed("text-x-generic");
    }
    info.icon().unwrap_or_else(|| themed("text-x-generic"))
}

/// [`icon_for_info`], and a starter (`.desktop` file) shows its own icon.
/// `info` needs [`ICON_ATTRIBUTES`].
pub fn icon_for(file: &gio::File, info: &gio::FileInfo) -> gio::Icon {
    let starter = info.content_type().as_deref() == Some(DESKTOP_ENTRY)
        && info.file_type() == gio::FileType::Regular;
    if starter && let Some(path) = file.path() {
        return DesktopAppInfo::from_filename(path)
            .and_then(|app| app.icon())
            .unwrap_or_else(|| themed("application-x-executable"));
    }
    icon_for_info(info)
}

/// The trash icon of the theme: empty or full (GNOME 2's names).
pub fn trash_icon(full: bool) -> gio::Icon {
    themed(if full {
        "user-trash-full"
    } else {
        "user-trash"
    })
}

/// Objects and bytes, as a job counts them before it starts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tally {
    pub items: u64,
    pub bytes: u64,
}

/// Objects inside `path` (not counting it) and their size in bytes, links
/// not followed; for "Eigenschaften" of a folder. Unreadable folders count
/// as empty.
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

/// A size as GNOME 2's properties showed it: "1.2 kB (1,234 bytes)", in
/// German "1,2 kB (1.234 Bytes)" (separator and words from the catalog).
pub fn size_text(bytes: u64) -> String {
    let short = glib::format_size(bytes).to_string();
    if bytes < 1000 {
        return short;
    }
    let digits = bytes.to_string();
    // TRANSLATORS: digit group separator in "1.2 kB (1,234 bytes)"
    let separator = pgettext("thousands separator", ",");
    let mut grouped = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            grouped.push_str(&separator);
        }
        grouped.push(c);
    }
    gettext_f(
        "{size} ({bytes} bytes)",
        &[("size", &short), ("bytes", &grouped)],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_file_is_a_blank_document() {
        assert!(is_empty_document(Some("application/x-zerosize")));
        assert!(!is_empty_document(Some("text/plain")));
        assert!(!is_empty_document(None));
        let info = gio::FileInfo::new();
        info.set_content_type(EMPTY);
        info.set_icon(&themed("application-x-zerosize"));
        let icon = icon_for_info(&info);
        assert_eq!(
            icon.downcast_ref::<gio::ThemedIcon>()
                .map(|t| t.names()[0].to_string()),
            Some("text-x-generic".to_owned())
        );
        info.set_content_type("image/png");
        info.set_icon(&themed("image-png"));
        assert!(icon_for_info(&info).equal(Some(&themed("image-png"))));
        assert!(trash_icon(true).equal(Some(&themed("user-trash-full"))));
    }

    /// Desktop 0.2.2: an empty file opens by its name; without a telling
    /// name nothing is guessed (shared-mime-info of the system).
    #[test]
    fn an_empty_file_opens_as_the_type_of_its_name() {
        assert_eq!(type_from_name("leer.txt").as_deref(), Some("text/plain"));
        assert_eq!(type_from_name("Notiz.TXT").as_deref(), Some("text/plain"));
        assert_eq!(type_from_name("bild.png").as_deref(), Some("image/png"));
        assert_eq!(type_from_name("Leere Datei"), None);
        assert_eq!(type_from_name("notiz"), None);
    }

    #[test]
    fn a_starter_shows_its_own_icon() {
        let base = std::env::temp_dir().join(format!("dateiarbeit-typ-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).expect("fixture");
        let path = base.join("probe.desktop");
        std::fs::write(
            &path,
            "[Desktop Entry]\nType=Application\nName=Probe\nExec=true\nIcon=accessories-calculator\n",
        )
        .expect("fixture");
        let info = gio::FileInfo::new();
        info.set_content_type(DESKTOP_ENTRY);
        info.set_file_type(gio::FileType::Regular);
        let icon = icon_for(&gio::File::for_path(&path), &info);
        let text = IconExt::to_string(&icon)
            .map(|t| t.to_string())
            .unwrap_or_default();
        assert!(text.contains("accessories-calculator"), "{text}");
        std::fs::remove_dir_all(&base).expect("cleanup");
    }

    #[test]
    fn tally_counts_folder_content_and_sizes_read_like_gnome() {
        let base = std::env::temp_dir().join(format!("dateiarbeit-zaehlen-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("u")).expect("fixture");
        std::fs::write(base.join("a"), "1234").expect("fixture");
        std::fs::write(base.join("u/b"), "56").expect("fixture");
        assert_eq!(tally(&base), (3, 6));
        std::fs::remove_dir_all(&base).expect("cleanup");
        assert!(size_text(1_234_567).ends_with(" (1,234,567 bytes)"));
        assert!(!size_text(12).contains('('));
    }
}
