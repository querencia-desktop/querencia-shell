//! One object of a folder in the folder model: a GObject that carries the
//! `gio::FileInfo`, what the views show and the sort keys, computed once
//! when the folder is read (interface of Welle B, `docs/dateien.md` §3).
//!
//! An entry never changes. When the file changes, the folder model puts a
//! new entry in its place (`Ordnermodell`), so recycled cells of the views
//! are bound again through `items-changed` and never show stale data.

use std::cell::{OnceCell, RefCell};
use std::collections::HashMap;

use gtk4::gio;
use gtk4::glib;
use gtk4::glib::subclass::prelude::*;
use gtk4::prelude::*;

use crate::regeln::{self, Sortdaten};

/// What the folder read asks GIO for in local folders. No `standard::icon`
/// and no `standard::content-type`: both sniff the file contents, which is
/// slow for 10,000 files (Recherche C §1). The icon comes from the fast type.
pub const ATTRIBUTE: &str = "standard::name,standard::display-name,standard::type,\
standard::size,standard::fast-content-type,standard::is-hidden,standard::is-backup,\
standard::is-symlink,standard::symlink-target,time::modified,unix::mode,\
access::can-read,access::can-write,access::can-execute,access::can-rename,\
access::can-trash,access::can-delete,owner::user,owner::group";

/// Additionally in places that are no plain folders (`trash:///`,
/// `computer:///`, GVfs mounts): the backend's own icon, the real place
/// behind an entry and where a trashed file came from.
pub const ATTRIBUTE_ORTE: &str = "standard::icon,standard::target-uri,\
trash::orig-path,trash::deletion-date";

/// The attributes for a folder at `ordner`.
pub fn attribute_fuer(ordner: &gio::File) -> String {
    if ordner.is_native() {
        ATTRIBUTE.to_owned()
    } else {
        format!("{ATTRIBUTE},{ATTRIBUTE_ORTE}")
    }
}

/// What an entry knows, fixed when it is made.
#[derive(Debug)]
pub struct Daten {
    pub datei: gio::File,
    /// The file name (identity within the folder).
    pub name: String,
    /// The name under the icon: display name, a starter's own name.
    pub anzeige: String,
    pub symbol: gio::Icon,
    pub info: gio::FileInfo,
    pub ist_ordner: bool,
    pub verborgen: bool,
    pub verweis: bool,
    /// A `.desktop` file shown with its own name and icon.
    pub starter: bool,
    /// The fast content type ("inode/directory" for folders).
    pub typ: String,
    /// The type-ahead form of `anzeige`.
    pub suchname: String,
    pub sort: Sortdaten,
}

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct Eintrag {
        pub daten: OnceCell<Daten>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Eintrag {
        const NAME: &'static str = "QuerenciaDateienEintrag";
        type Type = super::Eintrag;
    }

    impl ObjectImpl for Eintrag {}
}

glib::wrapper! {
    /// One file or folder in the folder model (see the module).
    pub struct Eintrag(ObjectSubclass<imp::Eintrag>);
}

thread_local! {
    /// Type descriptions as collation keys, per content type (few types,
    /// many files).
    static TYPEN: RefCell<HashMap<String, glib::CollationKey>> = RefCell::new(HashMap::new());
}

fn typ_schluessel(typ: &str) -> glib::CollationKey {
    TYPEN.with(|typen| {
        typen
            .borrow_mut()
            .entry(typ.to_owned())
            .or_insert_with(|| {
                glib::CollationKey::from(gio::content_type_get_description(typ).as_str())
            })
            .clone()
    })
}

/// Icons of the XDG folders and the home folder, as GLib gives them when
/// `standard::icon` is asked for.
pub fn ordnersymbol(datei: &gio::File) -> &'static str {
    let Some(pfad) = datei.path() else {
        return "folder";
    };
    if pfad == glib::home_dir() {
        return "user-home";
    }
    use glib::UserDirectory as U;
    let besondere = [
        (U::Desktop, "user-desktop"),
        (U::Documents, "folder-documents"),
        (U::Downloads, "folder-download"),
        (U::Music, "folder-music"),
        (U::Pictures, "folder-pictures"),
        (U::PublicShare, "folder-publicshare"),
        (U::Templates, "folder-templates"),
        (U::Videos, "folder-videos"),
    ];
    besondere
        .into_iter()
        .find(|(dir, _)| glib::user_special_dir(*dir).as_deref() == Some(pfad.as_path()))
        .map_or("folder", |(_, symbol)| symbol)
}

impl Eintrag {
    /// The entry for `info` (asked with [`attribute_fuer`]) in `ordner`.
    pub fn aus_info(ordner: &gio::File, info: &gio::FileInfo) -> Self {
        let name = info.name().to_string_lossy().into_owned();
        let datei = ordner.child(&name);
        let ist_ordner = matches!(
            info.file_type(),
            gio::FileType::Directory | gio::FileType::Mountable
        );
        let typ = if ist_ordner {
            "inode/directory".to_owned()
        } else {
            info.attribute_string("standard::fast-content-type")
                .map(|t| t.to_string())
                .unwrap_or_else(|| "application/octet-stream".to_owned())
        };
        let mut anzeige = info.display_name().to_string();
        let mut starter = false;
        let eigenes = info
            .has_attribute("standard::icon")
            .then(|| info.icon())
            .flatten();
        let symbol = if let Some(symbol) = eigenes.filter(|_| !ordner.is_native()) {
            symbol
        } else if ist_ordner {
            gio::ThemedIcon::new(ordnersymbol(&datei)).upcast()
        } else if typ == querencia_dateiarbeit::typ::DESKTOP_ENTRY
            && let Some(app) = datei
                .path()
                .and_then(gio_unix::DesktopAppInfo::from_filename)
        {
            starter = true;
            anzeige = app.display_name().to_string();
            app.icon()
                .unwrap_or_else(|| gio::ThemedIcon::new("application-x-executable").upcast())
        } else {
            gio::content_type_get_icon(&typ)
        };
        let zeit = info.modification_date_time().map_or(0, |t| t.to_unix());
        let daten = Daten {
            suchname: regeln::suchform(&anzeige),
            verborgen: info.is_hidden() || info.is_backup(),
            verweis: info.is_symlink(),
            starter,
            symbol,
            sort: Sortdaten {
                ist_ordner,
                name: glib::FilenameCollationKey::from(anzeige.as_str()),
                typ: typ_schluessel(&typ),
                groesse: u64::try_from(info.size()).unwrap_or(0),
                zeit,
                dateiname: name.clone(),
            },
            typ,
            info: info.clone(),
            ist_ordner,
            datei,
            name,
            anzeige,
        };
        let eintrag: Self = glib::Object::new();
        let _ = eintrag.imp().daten.set(daten);
        eintrag
    }

    pub fn daten(&self) -> &Daten {
        self.imp().daten.get().expect("Eintrag ohne Daten")
    }

    pub fn datei(&self) -> &gio::File {
        &self.daten().datei
    }

    /// The file name (identity within its folder).
    pub fn name(&self) -> &str {
        &self.daten().name
    }

    pub fn ist_ordner(&self) -> bool {
        self.daten().ist_ordner
    }

    /// The place the entry stands for: `standard::target-uri` (entries of
    /// `computer:///`, the trash) or the file itself.
    pub fn ziel(&self) -> gio::File {
        self.daten()
            .info
            .attribute_string("standard::target-uri")
            .map(|uri| gio::File::for_uri(&uri))
            .unwrap_or_else(|| self.datei().clone())
    }

    /// The type description ("Ordner", "Textdokument"), as the Typ column
    /// and the properties show it.
    pub fn beschreibung(&self) -> String {
        gio::content_type_get_description(&self.daten().typ).to_string()
    }

    /// The entry as a `regeln::Gewaehlt` for the status bar.
    pub fn gewaehlt(&self) -> regeln::Gewaehlt<'_> {
        let d = self.daten();
        regeln::Gewaehlt {
            name: &d.anzeige,
            ist_ordner: d.ist_ordner,
            groesse: d.sort.groesse,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_from_info_keeps_name_type_and_sort_keys() {
        let ordner = gio::File::for_path("/nirgends");
        let info = gio::FileInfo::new();
        info.set_name("bericht.final.odt");
        info.set_display_name("bericht.final.odt");
        info.set_file_type(gio::FileType::Regular);
        info.set_size(1234);
        info.set_attribute_string(
            "standard::fast-content-type",
            "application/vnd.oasis.opendocument.text",
        );
        let e = Eintrag::aus_info(&ordner, &info);
        assert_eq!(e.name(), "bericht.final.odt");
        assert_eq!(
            e.datei().path().unwrap(),
            std::path::Path::new("/nirgends/bericht.final.odt")
        );
        assert!(!e.ist_ordner());
        assert_eq!(e.daten().sort.groesse, 1234);
        assert_eq!(e.daten().typ, "application/vnd.oasis.opendocument.text");
        assert!(!e.daten().verborgen);
        assert_eq!(e.ziel().uri(), e.datei().uri());

        let info = gio::FileInfo::new();
        info.set_name(".versteckt");
        info.set_display_name(".versteckt");
        info.set_file_type(gio::FileType::Directory);
        info.set_is_hidden(true);
        let e = Eintrag::aus_info(&ordner, &info);
        assert!(e.ist_ordner());
        assert!(e.daten().verborgen);
        assert_eq!(e.daten().typ, "inode/directory");
    }

    #[test]
    fn places_ask_for_icons_and_targets() {
        assert_eq!(attribute_fuer(&gio::File::for_path("/tmp")), ATTRIBUTE);
        assert!(attribute_fuer(&gio::File::for_uri("trash:///")).contains("trash::orig-path"));
    }
}
