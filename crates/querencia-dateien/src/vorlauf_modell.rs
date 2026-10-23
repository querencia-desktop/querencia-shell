//! Provisional folder model of the views worker, taken from the prototype
//! (`dateien/prototyp-2`, eintrag.rs and the model chain of ansicht.rs):
//! `gio::ListStore<Eintrag>` → `FilterListModel` (hidden files) →
//! `SortListModel` → `MultiSelection`. It stands in for B1's modell.rs
//! until the tag `dateien/geruest-1` is merged and then goes away; the
//! views only use what [`crate::ansichtskern`] re-exports from here.

use std::cell::{Cell, OnceCell, RefCell};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::rc::{Rc, Weak};
use std::time::Instant;

use gtk4::gio;
use gtk4::glib;
use gtk4::glib::subclass::prelude::*;
use gtk4::prelude::*;

/// What the folder read asks GIO for. No `standard::icon` and no
/// `standard::content-type`: both sniff the file contents (Recherche C §1).
pub const ATTRIBUTE: &str = "standard::name,standard::display-name,standard::type,\
standard::size,standard::fast-content-type,standard::is-hidden,standard::is-backup,\
standard::is-symlink,time::modified,unix::mode,owner::user,owner::group";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kriterium {
    Name,
    Groesse,
    Typ,
    Datum,
}

impl Kriterium {
    pub const ALLE: [Kriterium; 4] = [
        Kriterium::Name,
        Kriterium::Groesse,
        Kriterium::Typ,
        Kriterium::Datum,
    ];

    /// The target of the action `win.anordnen`.
    pub fn kennung(self) -> &'static str {
        match self {
            Kriterium::Name => "name",
            Kriterium::Groesse => "groesse",
            Kriterium::Typ => "typ",
            Kriterium::Datum => "datum",
        }
    }

    pub fn aus_kennung(kennung: &str) -> Option<Self> {
        Self::ALLE.into_iter().find(|k| k.kennung() == kennung)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sortierung {
    pub kriterium: Kriterium,
    pub ordner_zuerst: bool,
    pub umgekehrt: bool,
}

impl Default for Sortierung {
    /// Nautilus 2.30's defaults: by name, ascending, folders first.
    fn default() -> Self {
        Self {
            kriterium: Kriterium::Name,
            ordner_zuerst: true,
            umgekehrt: false,
        }
    }
}

/// What the sorter compares; computed once when the folder is read.
pub struct Sortdaten {
    pub ist_ordner: bool,
    pub name: glib::FilenameCollationKey,
    pub typ: glib::CollationKey,
    pub groesse: u64,
    pub zeit: i64,
    pub dateiname: String,
}

/// Nautilus 2.30's order (`nautilus_file_compare_for_sort`).
pub fn vergleiche(a: &Sortdaten, b: &Sortdaten, s: Sortierung) -> Ordering {
    if s.ordner_zuerst && a.ist_ordner != b.ist_ordner {
        return if a.ist_ordner {
            Ordering::Less
        } else {
            Ordering::Greater
        };
    }
    let nach_name = || {
        a.name
            .cmp(&b.name)
            .then_with(|| a.dateiname.cmp(&b.dateiname))
    };
    let ergebnis = match s.kriterium {
        Kriterium::Name => nach_name(),
        Kriterium::Groesse => {
            if a.ist_ordner && b.ist_ordner {
                nach_name()
            } else {
                b.groesse.cmp(&a.groesse).then_with(nach_name)
            }
        }
        Kriterium::Typ => a.typ.cmp(&b.typ).then_with(nach_name),
        Kriterium::Datum => a.zeit.cmp(&b.zeit).then_with(nach_name),
    };
    if s.umgekehrt {
        ergebnis.reverse()
    } else {
        ergebnis
    }
}

pub struct Daten {
    pub datei: gio::File,
    pub name: String,
    pub anzeige: String,
    pub symbol: gio::Icon,
    pub verborgen: bool,
    pub starter: bool,
    pub ist_ordner: bool,
    pub groesse: u64,
    pub zeit: i64,
    /// The fast content type and its description.
    pub typ: String,
    pub typ_text: String,
    pub modus: Option<u32>,
    pub eigentuemer: String,
    pub gruppe: String,
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
        const NAME: &'static str = "QuerenciaDateienVorlaufEintrag";
        type Type = super::Eintrag;
    }

    impl ObjectImpl for Eintrag {}
}

glib::wrapper! {
    pub struct Eintrag(ObjectSubclass<imp::Eintrag>);
}

thread_local! {
    static TYPEN: RefCell<HashMap<String, (String, glib::CollationKey)>> =
        RefCell::new(HashMap::new());
}

fn typ_text(typ: &str) -> (String, glib::CollationKey) {
    TYPEN.with(|typen| {
        typen
            .borrow_mut()
            .entry(typ.to_owned())
            .or_insert_with(|| {
                let text = gio::content_type_get_description(typ).to_string();
                let key = glib::CollationKey::from(text.as_str());
                (text, key)
            })
            .clone()
    })
}

fn ordnersymbol(datei: &gio::File) -> &'static str {
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
    pub fn aus_info(ordner: &gio::File, info: &gio::FileInfo) -> Self {
        let name = info.name().to_string_lossy().into_owned();
        let datei = ordner.child(&name);
        let ist_ordner = info.file_type() == gio::FileType::Directory;
        let typ = if ist_ordner {
            "inode/directory".to_owned()
        } else {
            info.attribute_string("standard::fast-content-type")
                .map(|t| t.to_string())
                .unwrap_or_else(|| "application/octet-stream".to_owned())
        };
        let mut anzeige = info.display_name().to_string();
        let mut starter = false;
        let symbol = if ist_ordner {
            gio::ThemedIcon::new(ordnersymbol(&datei)).upcast()
        } else if typ == "application/x-desktop"
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
        let (typ_text, typ_schluessel) = typ_text(&typ);
        let groesse = u64::try_from(info.size()).unwrap_or(0);
        let modus = info
            .has_attribute("unix::mode")
            .then(|| info.attribute_uint32("unix::mode"));
        let text = |a: &str| {
            info.attribute_string(a)
                .map(|s| s.to_string())
                .unwrap_or_default()
        };
        let daten = Daten {
            sort: Sortdaten {
                ist_ordner,
                name: glib::FilenameCollationKey::from(anzeige.as_str()),
                typ: typ_schluessel,
                groesse,
                zeit,
                dateiname: name.clone(),
            },
            datei,
            verborgen: info.is_hidden() || info.is_backup(),
            starter,
            symbol,
            ist_ordner,
            groesse,
            zeit,
            typ,
            typ_text,
            modus,
            eigentuemer: text("owner::user"),
            gruppe: text("owner::group"),
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
}

type SortRueckruf = Box<dyn Fn(Sortierung)>;

/// The model chain of one folder view.
pub struct Modell {
    pub store: gio::ListStore,
    filter: gtk4::CustomFilter,
    sorter: gtk4::CustomSorter,
    pub sortiert: gtk4::SortListModel,
    pub auswahl: gtk4::MultiSelection,
    sortierung: Rc<Cell<Sortierung>>,
    verborgene: Rc<Cell<bool>>,
    ordner: RefCell<Option<gio::File>>,
    generation: Cell<u64>,
    bei_sortierung: RefCell<Vec<SortRueckruf>>,
}

impl Modell {
    pub fn new() -> Rc<Self> {
        let sortierung = Rc::new(Cell::new(Sortierung::default()));
        let verborgene = Rc::new(Cell::new(false));
        let store = gio::ListStore::new::<Eintrag>();
        let filter = {
            let verborgene = verborgene.clone();
            gtk4::CustomFilter::new(move |o| {
                verborgene.get()
                    || !o
                        .downcast_ref::<Eintrag>()
                        .is_some_and(|e| e.daten().verborgen)
            })
        };
        let gefiltert = gtk4::FilterListModel::new(Some(store.clone()), Some(filter.clone()));
        let sorter = {
            let sortierung = sortierung.clone();
            gtk4::CustomSorter::new(move |a, b| {
                let (Some(a), Some(b)) = (a.downcast_ref::<Eintrag>(), b.downcast_ref::<Eintrag>())
                else {
                    return gtk4::Ordering::Equal;
                };
                vergleiche(&a.daten().sort, &b.daten().sort, sortierung.get()).into()
            })
        };
        let sortiert = gtk4::SortListModel::new(Some(gefiltert), Some(sorter.clone()));
        let auswahl = gtk4::MultiSelection::new(Some(sortiert.clone()));
        Rc::new(Self {
            store,
            filter,
            sorter,
            sortiert,
            auswahl,
            sortierung,
            verborgene,
            ordner: RefCell::new(None),
            generation: Cell::new(0),
            bei_sortierung: RefCell::new(Vec::new()),
        })
    }

    pub fn ordner(&self) -> Option<gio::File> {
        self.ordner.borrow().clone()
    }

    pub fn sortierung(&self) -> Sortierung {
        self.sortierung.get()
    }

    pub fn bei_sortierung(&self, f: impl Fn(Sortierung) + 'static) {
        self.bei_sortierung.borrow_mut().push(Box::new(f));
    }

    pub fn sortierung_setzen(&self, s: Sortierung) {
        if s == self.sortierung.get() {
            return;
        }
        let t0 = Instant::now();
        self.sortierung.set(s);
        self.sorter.changed(gtk4::SorterChange::Different);
        eprintln!(
            "querencia-dateien: Messung Sortierung {} umgekehrt {}: {} Objekte in {} ms",
            s.kriterium.kennung(),
            s.umgekehrt,
            self.sortiert.n_items(),
            t0.elapsed().as_millis()
        );
        for f in self.bei_sortierung.borrow().iter() {
            f(s);
        }
    }

    pub fn verborgene(&self) -> bool {
        self.verborgene.get()
    }

    pub fn verborgene_setzen(&self, zeigen: bool) {
        if zeigen == self.verborgene.get() {
            return;
        }
        self.verborgene.set(zeigen);
        self.filter.changed(if zeigen {
            gtk4::FilterChange::LessStrict
        } else {
            gtk4::FilterChange::MoreStrict
        });
    }

    /// Reads `ordner` in blocks; `fertig` runs once everything is in.
    pub fn oeffne(self: &Rc<Self>, ordner: gio::File, fertig: impl FnOnce() + 'static) {
        self.generation.set(self.generation.get() + 1);
        *self.ordner.borrow_mut() = Some(ordner.clone());
        self.store.remove_all();
        glib::spawn_future_local(laden(
            Rc::downgrade(self),
            ordner,
            self.generation.get(),
            fertig,
        ));
    }

    /// Replaces `alt` by a fresh entry of `datei` (after a rename).
    pub fn ersetzen(&self, alt: &Eintrag, neu: Eintrag) {
        if let Some(i) = self.store.find(alt) {
            self.store.splice(i, 1, &[neu]);
        }
    }

    /// Adds the entry of `datei` if it lies in the folder shown (after
    /// creating a folder).
    pub fn hinzufuegen(&self, neu: Eintrag) {
        self.store.append(&neu);
    }
}

async fn laden(weak: Weak<Modell>, ordner: gio::File, generation: u64, fertig: impl FnOnce()) {
    let t0 = Instant::now();
    let pfad = ordner.parse_name().to_string();
    let aktuell = |m: &Modell| m.generation.get() == generation;
    let aufzaehlung = match ordner
        .enumerate_children_future(
            ATTRIBUTE,
            gio::FileQueryInfoFlags::NONE,
            glib::Priority::DEFAULT,
        )
        .await
    {
        Ok(a) => a,
        Err(e) => {
            eprintln!("querencia-dateien: {pfad} nicht lesbar: {}", e.message());
            return;
        }
    };
    let mut block = 200;
    let mut gesamt = 0usize;
    loop {
        let infos = aufzaehlung
            .next_files_future(block, glib::Priority::DEFAULT)
            .await;
        let Some(m) = weak.upgrade().filter(|m| aktuell(m)) else {
            return;
        };
        let Ok(infos) = infos else {
            break;
        };
        if infos.is_empty() {
            break;
        }
        let neu: Vec<Eintrag> = infos
            .iter()
            .map(|i| Eintrag::aus_info(&ordner, i))
            .collect();
        m.store.splice(m.store.n_items(), 0, &neu);
        if gesamt == 0 {
            eprintln!(
                "querencia-dateien: Messung {pfad}: erster Block ({}) nach {} ms",
                neu.len(),
                t0.elapsed().as_millis()
            );
        }
        gesamt += neu.len();
        block = 1000;
    }
    let _ = aufzaehlung.close_future(glib::Priority::DEFAULT).await;
    if weak.upgrade().is_some_and(|m| aktuell(&m)) {
        eprintln!(
            "querencia-dateien: Messung {pfad}: {gesamt} Einträge gelesen und sortiert nach {} ms",
            t0.elapsed().as_millis()
        );
        fertig();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn daten(name: &str, ordner: bool, groesse: u64, zeit: i64, typ: &str) -> Sortdaten {
        Sortdaten {
            ist_ordner: ordner,
            name: glib::FilenameCollationKey::from(name),
            typ: glib::CollationKey::from(typ),
            groesse,
            zeit,
            dateiname: name.to_owned(),
        }
    }

    fn sortiert(liste: &mut [Sortdaten], s: Sortierung) -> Vec<String> {
        liste.sort_by(|a, b| vergleiche(a, b, s));
        liste.iter().map(|d| d.dateiname.clone()).collect()
    }

    fn beispiel() -> Vec<Sortdaten> {
        vec![
            daten("datei10.txt", false, 10, 300, "Text"),
            daten("bilder", true, 0, 100, "Ordner"),
            daten("datei2.txt", false, 2000, 100, "Text"),
            daten("archiv.zip", false, 500, 200, "ZIP-Archiv"),
            daten("ablage", true, 0, 400, "Ordner"),
        ]
    }

    #[test]
    fn name_sorts_numbers_naturally_and_folders_first() {
        assert_eq!(
            sortiert(&mut beispiel(), Sortierung::default()),
            [
                "ablage",
                "bilder",
                "archiv.zip",
                "datei2.txt",
                "datei10.txt"
            ]
        );
    }

    #[test]
    fn reversed_keeps_folders_first() {
        let s = Sortierung {
            umgekehrt: true,
            ..Sortierung::default()
        };
        assert_eq!(
            sortiert(&mut beispiel(), s),
            [
                "bilder",
                "ablage",
                "datei10.txt",
                "datei2.txt",
                "archiv.zip"
            ]
        );
    }

    #[test]
    fn size_larger_first_date_older_first_type_by_description() {
        let groesse = Sortierung {
            kriterium: Kriterium::Groesse,
            ..Sortierung::default()
        };
        assert_eq!(
            sortiert(&mut beispiel(), groesse),
            [
                "ablage",
                "bilder",
                "datei2.txt",
                "archiv.zip",
                "datei10.txt"
            ]
        );
        let datum = Sortierung {
            kriterium: Kriterium::Datum,
            ordner_zuerst: false,
            umgekehrt: false,
        };
        assert_eq!(
            sortiert(&mut beispiel(), datum),
            [
                "bilder",
                "datei2.txt",
                "archiv.zip",
                "datei10.txt",
                "ablage"
            ]
        );
    }
}
