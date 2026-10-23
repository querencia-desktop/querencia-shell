//! Names and icons of places (window title, side pane, path bar): the home
//! folder, the file system, "Rechner", "Netzwerk", "Papierkorb" as the
//! panel's "Orte" menu calls them (`places_menu.rs`).

use gtk4::prelude::*;
use gtk4::{gio, glib};

use crate::i18n::gettext;

/// The name of a place for the title and the bars.
pub fn anzeigename(ort: &gio::File) -> String {
    match ort.uri_scheme().as_deref() {
        Some("trash") => return gettext("Trash"),
        Some("computer") => return gettext("Computer"),
        Some("network") => return gettext("Network"),
        _ => {}
    }
    if let Some(pfad) = ort.path() {
        if pfad == std::path::Path::new("/") {
            return gettext("File System");
        }
        if pfad == glib::home_dir() {
            return pfad
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| gettext("Home Folder"));
        }
        return pfad
            .file_name()
            .map(|n| glib::filename_display_name(n).to_string())
            .unwrap_or_else(|| pfad.display().to_string());
    }
    ort.basename()
        .map(|n| n.to_string_lossy().into_owned())
        .filter(|n| n != "/")
        .unwrap_or_else(|| ort.parse_name().to_string())
}

/// The icon of a place.
pub fn symbol(ort: &gio::File) -> &'static str {
    match ort.uri_scheme().as_deref() {
        Some("trash") => "user-trash",
        Some("computer") => "computer",
        Some("network") => "network-workgroup",
        _ if ort.path().as_deref() == Some(std::path::Path::new("/")) => "drive-harddisk",
        _ if !ort.is_native() => "folder-remote",
        _ => crate::eintrag::ordnersymbol(ort),
    }
}

/// Whether GIO can show `scheme` here (GVfs backends; without gvfsd only
/// `file`), as the panel checks before it offers a place.
pub fn schema_verfuegbar(scheme: &str) -> bool {
    gio::Vfs::default()
        .supported_uri_schemes()
        .iter()
        .any(|s| s.as_str() == scheme)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn places_are_named_like_the_panel() {
        assert_eq!(anzeigename(&gio::File::for_path("/")), "File System");
        assert_eq!(anzeigename(&gio::File::for_path("/usr/share")), "share");
        assert_eq!(symbol(&gio::File::for_path("/")), "drive-harddisk");
        assert_eq!(symbol(&gio::File::for_path("/usr")), "folder");
    }
}
