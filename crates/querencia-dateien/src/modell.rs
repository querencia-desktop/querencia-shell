//! The folder model of a window (interface of Welle B, `docs/dateien.md`
//! §3): `gio::ListStore` of [`Eintrag`] → `FilterListModel` (hidden files)
//! → `SortListModel` (Nautilus 2.30's order) → `MultiSelection`. Every view
//! shows the same `MultiSelection`, so switching views keeps order and
//! selection.
//!
//! The folder is read asynchronously in blocks (first block 200, then
//! 1000, as the prototype measured) and kept current through
//! `querencia_dateiarbeit::ueberwachung::FolderWatch`: new and changed files
//! are asked for again and their entry replaced, renames keep the
//! selection. An entry is never changed in place (see `eintrag.rs`).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use gtk4::prelude::*;
use gtk4::{gio, glib};
use querencia_dateiarbeit::ueberwachung::{Aenderung, FolderWatch};

use crate::eintrag::{self, Eintrag};
use crate::regeln::{self, Sortierung};

const ERSTER_BLOCK: i32 = 200;
const BLOCK: i32 = 1000;

/// What happens to the folder of the model; handlers registered with
/// [`Ordnermodell::bei_zustand`].
#[derive(Clone, Debug)]
pub enum Zustand {
    /// Reading `ordner` began (the model is empty now).
    Laedt(gio::File),
    /// The first block is in the model.
    ErsterBlock,
    /// Everything is read; number of entries (hidden ones included).
    Geladen(u32),
    /// Reading stopped (Ansicht → Stopp); what was read stays.
    Gestoppt,
    /// The folder could not be read (message for the status bar/dialog).
    Fehler(String),
    /// The folder itself went away (deleted, moved, unmounted).
    OrdnerWeg,
}

type ZustandHandler = Rc<dyn Fn(&Zustand)>;
type SortierHandler = Rc<dyn Fn(Sortierung)>;
type Warter = (String, Box<dyn FnOnce(&Eintrag)>);

struct Inner {
    store: gio::ListStore,
    filter: gtk4::CustomFilter,
    sorter: gtk4::CustomSorter,
    sortiert: gtk4::SortListModel,
    auswahl: gtk4::MultiSelection,
    sortierung: Rc<Cell<Sortierung>>,
    verborgene: Rc<Cell<bool>>,
    ordner: RefCell<Option<gio::File>>,
    /// Entries by file name (the store's identity).
    namen: RefCell<HashMap<String, Eintrag>>,
    generation: Cell<u64>,
    abbruch: RefCell<Option<gio::Cancellable>>,
    watch: RefCell<Option<FolderWatch>>,
    laedt: Cell<bool>,
    handler: RefCell<Vec<ZustandHandler>>,
    sortier_handler: RefCell<Vec<SortierHandler>>,
    /// Callbacks waiting for a file name to appear ([`Ordnermodell::bei_erscheinen`]).
    warter: RefCell<Vec<Warter>>,
}

/// The folder model (cheap to clone: a handle).
#[derive(Clone)]
pub struct Ordnermodell(Rc<Inner>);

impl Default for Ordnermodell {
    fn default() -> Self {
        Self::new(Sortierung::default(), false)
    }
}

impl Ordnermodell {
    pub fn new(sortierung: Sortierung, verborgene_zeigen: bool) -> Self {
        let sortierung = Rc::new(Cell::new(sortierung));
        let verborgene = Rc::new(Cell::new(verborgene_zeigen));
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
                regeln::vergleiche(&a.daten().sort, &b.daten().sort, sortierung.get()).into()
            })
        };
        let sortiert = gtk4::SortListModel::new(Some(gefiltert), Some(sorter.clone()));
        let auswahl = gtk4::MultiSelection::new(Some(sortiert.clone()));
        Self(Rc::new(Inner {
            store,
            filter,
            sorter,
            sortiert,
            auswahl,
            sortierung,
            verborgene,
            ordner: RefCell::default(),
            namen: RefCell::default(),
            generation: Cell::new(0),
            abbruch: RefCell::default(),
            watch: RefCell::default(),
            laedt: Cell::new(false),
            handler: RefCell::default(),
            sortier_handler: RefCell::default(),
            warter: RefCell::default(),
        }))
    }

    fn weak(&self) -> Weak<Inner> {
        Rc::downgrade(&self.0)
    }

    fn from_weak(weak: &Weak<Inner>) -> Option<Self> {
        weak.upgrade().map(Self)
    }

    // ---------------------------------------------------------------- models

    /// All entries of the folder, unsorted, hidden ones included.
    #[cfg(test)]
    pub fn alle(&self) -> &gio::ListStore {
        &self.0.store
    }

    /// The selection on top of [`Self::sortiert`]: the model of every view.
    pub fn auswahl(&self) -> &gtk4::MultiSelection {
        &self.0.auswahl
    }

    /// The shown entry at `pos`.
    pub fn eintrag(&self, pos: u32) -> Option<Eintrag> {
        self.0.sortiert.item(pos).and_downcast::<Eintrag>()
    }

    /// Number of shown entries.
    pub fn anzahl(&self) -> u32 {
        self.0.sortiert.n_items()
    }

    /// The entry of `datei` (shown or hidden), if it is in the folder.
    pub fn eintrag_von(&self, datei: &gio::File) -> Option<Eintrag> {
        let name = self.name_in_ordner(datei)?;
        self.0.namen.borrow().get(&name).cloned()
    }

    /// Position of `datei` among the shown entries.
    pub fn position_von(&self, datei: &gio::File) -> Option<u32> {
        let name = self.name_in_ordner(datei)?;
        self.position_von_name(&name)
    }

    fn position_von_name(&self, name: &str) -> Option<u32> {
        (0..self.anzahl()).find(|&i| self.eintrag(i).is_some_and(|e| e.name() == name))
    }

    fn name_in_ordner(&self, datei: &gio::File) -> Option<String> {
        let ordner = self.0.ordner.borrow().clone()?;
        let eltern = datei.parent()?;
        eltern
            .equal(&ordner)
            .then(|| datei.basename())
            .flatten()
            .map(|n| n.to_string_lossy().into_owned())
    }

    // ------------------------------------------------------------- selection

    /// The selected entries in shown order.
    pub fn gewaehlte(&self) -> Vec<Eintrag> {
        let menge = self.0.auswahl.selection();
        let mut out = Vec::new();
        if let Some((mut iter, erster)) = gtk4::BitsetIter::init_first(&menge) {
            if let Some(e) = self.eintrag(erster) {
                out.push(e);
            }
            for pos in iter.by_ref() {
                if let Some(e) = self.eintrag(pos) {
                    out.push(e);
                }
            }
        }
        out
    }

    /// Selects exactly the shown entries of `dateien` (others unselected).
    pub fn waehlen(&self, dateien: &[gio::File]) {
        let namen: Vec<String> = dateien
            .iter()
            .filter_map(|d| self.name_in_ordner(d))
            .collect();
        self.waehlen_namen(&namen);
    }

    fn waehlen_namen(&self, namen: &[String]) {
        let menge = gtk4::Bitset::new_empty();
        for i in 0..self.anzahl() {
            if self
                .eintrag(i)
                .is_some_and(|e| namen.iter().any(|n| n == e.name()))
            {
                menge.add(i);
            }
        }
        let alle = gtk4::Bitset::new_range(0, self.anzahl());
        self.0.auswahl.set_selection(&menge, &alle);
    }

    pub fn alle_waehlen(&self) {
        self.0.auswahl.select_all();
    }

    /// Beta (Bearbeiten → Auswahl umkehren); here because it only needs the model.
    pub fn auswahl_umkehren(&self) {
        let vorher = self.0.auswahl.selection();
        let menge = gtk4::Bitset::new_range(0, self.anzahl());
        menge.subtract(&vorher);
        let alle = gtk4::Bitset::new_range(0, self.anzahl());
        self.0.auswahl.set_selection(&menge, &alle);
    }

    /// Runs `aendern` and selects the same files afterwards (renamed ones
    /// under their new name, `umbenannt` = (alt, neu)).
    fn mit_auswahl(&self, umbenannt: &[(String, String)], aendern: impl FnOnce()) {
        let mut namen: Vec<String> = self
            .gewaehlte()
            .iter()
            .map(|e| e.name().to_owned())
            .collect();
        aendern();
        for (alt, neu) in umbenannt {
            if let Some(n) = namen.iter_mut().find(|n| *n == alt) {
                n.clone_from(neu);
            }
        }
        if !namen.is_empty() {
            self.waehlen_namen(&namen);
        }
    }

    // ---------------------------------------------------- sorting and filter

    pub fn sortierung(&self) -> Sortierung {
        self.0.sortierung.get()
    }

    pub fn sortierung_setzen(&self, sortierung: Sortierung) {
        if self.0.sortierung.get() == sortierung {
            return;
        }
        self.0.sortierung.set(sortierung);
        self.0.sorter.changed(gtk4::SorterChange::Different);
        let handler: Vec<SortierHandler> = self.0.sortier_handler.borrow().clone();
        for h in handler {
            h(sortierung);
        }
    }

    /// `f` gets every new sort order (View menu, settings, a column head),
    /// e.g. to show the arrow in the list view's column heads.
    pub fn bei_sortierung(&self, f: impl Fn(Sortierung) + 'static) {
        self.0.sortier_handler.borrow_mut().push(Rc::new(f));
    }

    pub fn verborgene_setzen(&self, zeigen: bool) {
        if self.0.verborgene.get() == zeigen {
            return;
        }
        self.0.verborgene.set(zeigen);
        self.0.filter.changed(if zeigen {
            gtk4::FilterChange::LessStrict
        } else {
            gtk4::FilterChange::MoreStrict
        });
    }

    // ---------------------------------------------------------------- folder

    /// The folder shown (also while it is read).
    pub fn ordner(&self) -> Option<gio::File> {
        self.0.ordner.borrow().clone()
    }

    /// Whether the folder is being read.
    pub fn laedt(&self) -> bool {
        self.0.laedt.get()
    }

    /// `f` gets every change of state (see [`Zustand`]).
    pub fn bei_zustand(&self, f: impl Fn(&Zustand) + 'static) {
        self.0.handler.borrow_mut().push(Rc::new(f));
    }

    fn melden(&self, zustand: Zustand) {
        crate::log(&match &zustand {
            Zustand::Laedt(o) => format!("Ordner {}: lesen", o.uri()),
            Zustand::ErsterBlock => "erster Block".to_owned(),
            Zustand::Geladen(n) => format!("Ordner gelesen: {n} Einträge"),
            Zustand::Gestoppt => "Lesen gestoppt".to_owned(),
            Zustand::Fehler(t) => format!("Ordner nicht lesbar: {t}"),
            Zustand::OrdnerWeg => "Ordner verschwunden".to_owned(),
        });
        let handler: Vec<ZustandHandler> = self.0.handler.borrow().clone();
        for h in handler {
            h(&zustand);
        }
    }

    /// `f` gets the entry of `datei` as soon as it is in the model (at once
    /// when it is already there); used to rename a new folder or document
    /// right away and to select the result of an operation. Forgotten when
    /// another folder is opened.
    pub fn bei_erscheinen(&self, datei: &gio::File, f: impl FnOnce(&Eintrag) + 'static) {
        let Some(name) = self.name_in_ordner(datei) else {
            return;
        };
        let schon = self.0.namen.borrow().get(&name).cloned();
        match schon {
            Some(e) => f(&e),
            None => self.0.warter.borrow_mut().push((name, Box::new(f))),
        }
    }

    /// Opens `ordner`: empties the model, reads the folder in blocks and
    /// watches it.
    pub fn laden(&self, ordner: &gio::File) {
        self.abbrechen();
        let generation = self.0.generation.get() + 1;
        self.0.generation.set(generation);
        self.0.ordner.replace(Some(ordner.clone()));
        self.0.warter.borrow_mut().clear();
        self.0.namen.borrow_mut().clear();
        self.0.store.remove_all();
        let watch = FolderWatch::new(ordner, false);
        let weak = self.weak();
        watch.connect(move |aenderungen| {
            if let Some(m) = Self::from_weak(&weak)
                && m.0.generation.get() == generation
            {
                m.aenderungen(aenderungen);
            }
        });
        self.0.watch.replace(Some(watch));
        let abbruch = gio::Cancellable::new();
        self.0.abbruch.replace(Some(abbruch.clone()));
        self.0.laedt.set(true);
        self.melden(Zustand::Laedt(ordner.clone()));
        let weak = self.weak();
        let ordner = ordner.clone();
        glib::spawn_future_local(async move {
            lesen(weak, ordner, generation, abbruch).await;
        });
    }

    /// Reads the folder again, keeping the selection.
    pub fn neu_laden(&self) {
        let Some(ordner) = self.ordner() else {
            return;
        };
        let namen: Vec<String> = self
            .gewaehlte()
            .iter()
            .map(|e| e.name().to_owned())
            .collect();
        self.laden(&ordner);
        if !namen.is_empty() {
            let weak = self.weak();
            let gen_jetzt = self.0.generation.get();
            self.bei_zustand_einmal(move |z| {
                if let Some(m) = Self::from_weak(&weak)
                    && m.0.generation.get() == gen_jetzt
                    && matches!(z, Zustand::Geladen(_))
                {
                    m.waehlen_namen(&namen);
                }
            });
        }
    }

    fn bei_zustand_einmal(&self, f: impl Fn(&Zustand) + 'static) {
        let fertig = Rc::new(Cell::new(false));
        self.bei_zustand(move |z| {
            if !fertig.get()
                && matches!(
                    z,
                    Zustand::Geladen(_) | Zustand::Fehler(_) | Zustand::Gestoppt
                )
            {
                fertig.set(true);
                f(z);
            }
        });
    }

    /// Stops reading (Ansicht → Stopp); what was read stays.
    pub fn stoppen(&self) {
        if self.0.laedt.get() {
            self.abbrechen();
            self.melden(Zustand::Gestoppt);
        }
    }

    fn abbrechen(&self) {
        if let Some(c) = self.0.abbruch.take() {
            c.cancel();
        }
        self.0.laedt.set(false);
    }

    // -------------------------------------------------------- store changes

    /// Puts `neu` into the store: replaces the entry of the same name or
    /// appends; calls waiting callbacks.
    fn einsetzen(&self, neu: &[Eintrag]) {
        let mut anhaengen = Vec::with_capacity(neu.len());
        for e in neu {
            let alt = self.0.namen.borrow().get(e.name()).cloned();
            match alt.and_then(|alt| self.0.store.find(&alt)) {
                Some(pos) => self.0.store.splice(pos, 1, std::slice::from_ref(e)),
                None => anhaengen.push(e.clone()),
            }
            self.0
                .namen
                .borrow_mut()
                .insert(e.name().to_owned(), e.clone());
        }
        if !anhaengen.is_empty() {
            self.0.store.splice(self.0.store.n_items(), 0, &anhaengen);
        }
        let erschienen: Vec<Warter> = {
            let mut warter = self.0.warter.borrow_mut();
            let (da, offen): (Vec<Warter>, Vec<Warter>) = warter
                .drain(..)
                .partition(|(name, _)| neu.iter().any(|e| e.name() == name));
            *warter = offen;
            da
        };
        for (name, f) in erschienen {
            if let Some(e) = self.0.namen.borrow().get(&name).cloned() {
                f(&e);
            }
        }
    }

    fn entfernen(&self, name: &str) {
        let alt = self.0.namen.borrow_mut().remove(name);
        if let Some(pos) = alt.and_then(|alt| self.0.store.find(&alt)) {
            self.0.store.remove(pos);
        }
    }

    fn aenderungen(&self, aenderungen: &[Aenderung]) {
        let Some(ordner) = self.ordner() else {
            return;
        };
        let mut abfragen: Vec<gio::File> = Vec::new();
        let mut umbenannt: Vec<(String, String)> = Vec::new();
        let mut weg: Vec<String> = Vec::new();
        for a in aenderungen {
            match a {
                Aenderung::Neu(f) | Aenderung::Geaendert(f) => abfragen.push(f.clone()),
                Aenderung::Weg(f) => weg.extend(basename(f)),
                Aenderung::Umbenannt(alt, neu) => {
                    if let (Some(a), Some(n)) = (basename(alt), basename(neu)) {
                        umbenannt.push((a.clone(), n));
                        weg.push(a);
                    }
                    abfragen.push(neu.clone());
                }
                Aenderung::NeuLaden => {
                    self.neu_laden();
                    return;
                }
                Aenderung::OrdnerWeg => {
                    self.melden(Zustand::OrdnerWeg);
                    return;
                }
                Aenderung::OrdnerDa => {}
            }
        }
        if !weg.is_empty() && abfragen.is_empty() {
            self.mit_auswahl(&[], || {
                for name in &weg {
                    self.entfernen(name);
                }
            });
            return;
        }
        let generation = self.0.generation.get();
        let weak = self.weak();
        let attribute = eintrag::attribute_fuer(&ordner);
        glib::spawn_future_local(async move {
            let mut neu = Vec::new();
            for datei in abfragen {
                let info = datei
                    .query_info_future(
                        &attribute,
                        gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
                        glib::Priority::DEFAULT,
                    )
                    .await;
                // a file gone again before it could be asked for is left out
                if let Ok(info) = info {
                    neu.push(Eintrag::aus_info(&ordner, &info));
                }
            }
            let Some(m) = Self::from_weak(&weak).filter(|m| m.0.generation.get() == generation)
            else {
                return;
            };
            m.mit_auswahl(&umbenannt, || {
                for name in &weg {
                    if !neu.iter().any(|e| e.name() == name) {
                        m.entfernen(name);
                    }
                }
                m.einsetzen(&neu);
            });
        });
    }
}

fn basename(datei: &gio::File) -> Option<String> {
    datei.basename().map(|n| n.to_string_lossy().into_owned())
}

async fn lesen(weak: Weak<Inner>, ordner: gio::File, generation: u64, abbruch: gio::Cancellable) {
    let aktuell = |m: &Ordnermodell| m.0.generation.get() == generation && !abbruch.is_cancelled();
    let attribute = eintrag::attribute_fuer(&ordner);
    let aufzaehlung = ordner
        .enumerate_children_future(
            &attribute,
            gio::FileQueryInfoFlags::NONE,
            glib::Priority::DEFAULT,
        )
        .await;
    let aufzaehlung = match aufzaehlung {
        Ok(a) => a,
        Err(fehler) => {
            if let Some(m) = Ordnermodell::from_weak(&weak).filter(aktuell) {
                m.0.laedt.set(false);
                m.melden(Zustand::Fehler(fehler.message().to_owned()));
            }
            return;
        }
    };
    let mut block = ERSTER_BLOCK;
    let mut gesamt = 0u32;
    loop {
        let infos = aufzaehlung
            .next_files_future(block, glib::Priority::DEFAULT)
            .await;
        let Some(m) = Ordnermodell::from_weak(&weak).filter(aktuell) else {
            return;
        };
        let infos = match infos {
            Ok(i) => i,
            Err(fehler) => {
                m.0.laedt.set(false);
                m.melden(Zustand::Fehler(fehler.message().to_owned()));
                return;
            }
        };
        if infos.is_empty() {
            break;
        }
        let neu: Vec<Eintrag> = infos
            .iter()
            .map(|i| Eintrag::aus_info(&ordner, i))
            .collect();
        m.einsetzen(&neu);
        if gesamt == 0 {
            m.melden(Zustand::ErsterBlock);
        }
        gesamt += u32::try_from(neu.len()).unwrap_or(0);
        block = BLOCK;
    }
    let _ = aufzaehlung.close_future(glib::Priority::DEFAULT).await;
    if let Some(m) = Ordnermodell::from_weak(&weak).filter(aktuell) {
        m.0.laedt.set(false);
        m.0.abbruch.replace(None);
        let anzahl = m.0.store.n_items();
        m.melden(Zustand::Geladen(anzahl));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    fn baum(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("qdat-modell-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("Briefe")).unwrap();
        std::fs::create_dir_all(dir.join("Archiv")).unwrap();
        for (n, inhalt) in [
            ("datei10.txt", "x"),
            ("datei2.txt", "xxxxxxxxxx"),
            ("bild.png", "xxxxx"),
            (".versteckt", ""),
            ("sicherung.txt~", ""),
        ] {
            std::fs::write(dir.join(n), inhalt).unwrap();
        }
        dir
    }

    /// Runs the main context until `fertig` or 5 s.
    fn warten(fertig: impl Fn() -> bool) {
        let ctx = glib::MainContext::default();
        let start = Instant::now();
        while !fertig() && start.elapsed() < Duration::from_secs(5) {
            ctx.iteration(false);
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(fertig(), "Zeitüberschreitung");
    }

    fn namen(m: &Ordnermodell) -> Vec<String> {
        (0..m.anzahl())
            .map(|i| m.eintrag(i).unwrap().name().to_owned())
            .collect()
    }

    fn geladen(m: &Ordnermodell) -> Rc<Cell<bool>> {
        let fertig = Rc::new(Cell::new(false));
        let f = fertig.clone();
        m.bei_zustand(move |z| {
            if matches!(z, Zustand::Geladen(_)) {
                f.set(true);
            }
        });
        fertig
    }

    /// GTK's list models need an initialised GTK, that is a display: this
    /// test runs only in the isolated environment (`werkzeug/gui-tests.sh`:
    /// test harness with headless labwc, own HOME and runtime folder) and
    /// refuses the live session. One function runs the steps in order on
    /// the default main context.
    #[test]
    #[ignore = "braucht GTK: werkzeug/gui-tests.sh (isoliert)"]
    fn folder_model_reads_sorts_filters_selects_and_follows_changes() {
        let laufzeit = std::env::var("XDG_RUNTIME_DIR").unwrap_or_default();
        assert!(
            !laufzeit.starts_with("/run/user/"),
            "nur isoliert ausführen (XDG_RUNTIME_DIR={laufzeit})"
        );
        gtk4::init().expect("GTK in der isolierten Sitzung");
        let ctx = glib::MainContext::default();
        let _guard = ctx.acquire().expect("Hauptkontext");
        let dir = baum("ablauf");
        let m = Ordnermodell::default();
        let fertig = geladen(&m);
        m.laden(&gio::File::for_path(&dir));
        assert!(m.laedt());
        warten(|| fertig.get());
        assert_eq!(
            namen(&m),
            ["Archiv", "Briefe", "bild.png", "datei2.txt", "datei10.txt"]
        );
        assert_eq!(m.alle().n_items(), 7);

        m.verborgene_setzen(true);
        assert_eq!(m.anzahl(), 7);
        m.verborgene_setzen(false);

        m.sortierung_setzen(Sortierung {
            kriterium: regeln::Kriterium::Groesse,
            ..Sortierung::default()
        });
        assert_eq!(
            namen(&m),
            ["Archiv", "Briefe", "datei2.txt", "bild.png", "datei10.txt"]
        );
        m.sortierung_setzen(Sortierung::default());

        // selection by identity, kept over a rename by another program
        m.waehlen(&[gio::File::for_path(dir.join("bild.png"))]);
        assert_eq!(m.gewaehlte()[0].name(), "bild.png");
        std::fs::rename(dir.join("bild.png"), dir.join("aaa.png")).unwrap();
        warten(|| {
            m.position_von(&gio::File::for_path(dir.join("aaa.png")))
                .is_some()
        });
        assert_eq!(m.gewaehlte().len(), 1);
        assert_eq!(m.gewaehlte()[0].name(), "aaa.png");
        assert!(
            m.eintrag_von(&gio::File::for_path(dir.join("bild.png")))
                .is_none()
        );

        // new file appears, a waiting callback gets it
        let gesehen = Rc::new(RefCell::new(String::new()));
        let g = gesehen.clone();
        m.bei_erscheinen(&gio::File::for_path(dir.join("neu.txt")), move |e| {
            g.replace(e.name().to_owned());
        });
        std::fs::write(dir.join("neu.txt"), "neu").unwrap();
        warten(|| gesehen.borrow().as_str() == "neu.txt");

        // changed file: same place, new entry with the new size
        std::fs::write(dir.join("neu.txt"), "viel mehr Inhalt").unwrap();
        warten(|| {
            m.eintrag_von(&gio::File::for_path(dir.join("neu.txt")))
                .is_some_and(|e| e.daten().sort.groesse == 16)
        });

        // deleted file goes
        std::fs::remove_file(dir.join("datei10.txt")).unwrap();
        warten(|| {
            m.position_von(&gio::File::for_path(dir.join("datei10.txt")))
                .is_none()
        });
        assert_eq!(m.alle().n_items(), 7);

        m.alle_waehlen();
        assert_eq!(m.gewaehlte().len() as u32, m.anzahl());
        m.auswahl_umkehren();
        assert!(m.gewaehlte().is_empty());

        // reload keeps the selection
        m.waehlen(&[gio::File::for_path(dir.join("Briefe"))]);
        let fertig = geladen(&m);
        m.neu_laden();
        warten(|| fertig.get());
        let ctx2 = glib::MainContext::default();
        for _ in 0..10 {
            ctx2.iteration(false);
        }
        assert_eq!(m.gewaehlte().len(), 1);
        assert_eq!(m.gewaehlte()[0].name(), "Briefe");

        // the folder itself goes away
        let weg = Rc::new(Cell::new(false));
        let w = weg.clone();
        m.bei_zustand(move |z| {
            if matches!(z, Zustand::OrdnerWeg) {
                w.set(true);
            }
        });
        std::fs::remove_dir_all(&dir).unwrap();
        warten(|| weg.get());

        // an unreadable folder reports an error
        let fehler = Rc::new(Cell::new(false));
        let f = fehler.clone();
        m.bei_zustand(move |z| {
            if matches!(z, Zustand::Fehler(_)) {
                f.set(true);
            }
        });
        m.laden(&gio::File::for_path(dir.join("gibt-es-nicht")));
        warten(|| fehler.get());
        assert!(!m.laedt());
    }
}
