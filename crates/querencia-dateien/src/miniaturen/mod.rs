//! Thumbnails in the icon and list view (Welle C, M9 of B §4): the shared
//! freedesktop.org cache (`cache.rs`, the same as Querencia Files'), made
//! by GTK, GdkPixbuf or the installed thumbnailers (`erzeuger.rs`) in
//! worker threads (`warteschlange.rs`), scaled and framed as Nautilus
//! 2.30 showed them (`bild.rs`).
//!
//! The views reach this module only through the **Bildquelle je Eintrag**
//! ([`bildquelle`]): `binden` when a cell shows an entry, `loesen` when it
//! lets it go. Without this module registered they show the theme icon,
//! as before. Settings: Einstellungen → Vorschau, „Miniaturen anzeigen“
//! (Immer/Nur lokale Dateien/Nie) and „Nur für Dateien kleiner als“.

pub mod bild;
pub mod cache;
pub mod erzeuger;
pub mod warteschlange;

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use gtk4::prelude::*;
use gtk4::{gdk, glib};

use crate::ansichtskern::protokoll;
use crate::app::Register;
use crate::eintrag::Eintrag;
use crate::konfig::{Werte, Wo};
use warteschlange::{Anfrage, Auftrag, Ergebnis, Herkunft, Pool};

/// When a file gets a thumbnail (Nautilus 2.30's rules).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Regel {
    pub wo: Wo,
    /// Files larger than this get none (`Nur für Dateien kleiner als`).
    pub max_bytes: u64,
}

impl Regel {
    pub fn aus(werte: &Werte) -> Self {
        Self {
            wo: werte.vorschaubilder,
            max_bytes: u64::from(werte.vorschau_max_mb) * 1024 * 1024,
        }
    }
}

/// What the rules look at, taken from an entry.
#[derive(Clone, Debug, PartialEq)]
pub struct Kandidat<'a> {
    pub ist_ordner: bool,
    pub starter: bool,
    pub lokal: bool,
    pub groesse: u64,
    pub typ: &'a str,
    pub pfad: Option<&'a std::path::Path>,
}

/// Whether a file may get a thumbnail under `regel` (the type is checked
/// separately: [`erzeuger::moeglich`]). Folders and starters keep their
/// icons; files in the thumbnail cache itself never get one.
pub fn erlaubt(regel: Regel, k: &Kandidat, cache_wurzel: &std::path::Path) -> bool {
    let wo = match regel.wo {
        Wo::Nie => false,
        Wo::NurLokal => k.lokal,
        Wo::Immer => true,
    };
    wo && !k.ist_ordner
        && !k.starter
        && k.groesse <= regel.max_bytes
        && !k.typ.is_empty()
        && !k.pfad.is_some_and(|p| cache::im_cache(cache_wurzel, p))
}

/// The view's sizes for one cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Groesse {
    /// The theme icon (px).
    pub symbol: i32,
    /// The thumbnail's long edge, frame included (px).
    pub vorschau: i32,
}

impl Groesse {
    /// Icon view: thumbnails a third larger than the icon, as Nautilus
    /// 2.30 (`icon_view/thumbnail_size` 64 at the 48 px standard size):
    /// 24 → 32, 32 → 43, 48 → 64, 72 → 96, 96 → 128.
    pub fn symbolansicht(symbol: i32) -> Self {
        Self {
            symbol,
            vorschau: (symbol * 4 + 1) / 3,
        }
    }

    /// List view: thumbnails as big as the icon (rows keep their height).
    pub fn liste(symbol: i32) -> Self {
        Self {
            symbol,
            vorschau: symbol,
        }
    }
}

/// A cell's image and what it shows.
struct Bindung {
    bild: glib::WeakRef<gtk4::Image>,
    eintrag: Eintrag,
    groesse: Groesse,
    /// The thumbnail asked for (if any).
    anfrage: Option<Anfrage>,
}

/// A small LRU of finished pictures (`None`: none to be had).
#[derive(Default)]
struct Speicher {
    eintraege: HashMap<Anfrage, (Option<gdk::Texture>, u64, usize)>,
    takt: u64,
    bytes: usize,
}

const SPEICHER_MAX: usize = 48 * 1024 * 1024;

impl Speicher {
    fn holen(&mut self, a: &Anfrage) -> Option<Option<gdk::Texture>> {
        self.takt += 1;
        let takt = self.takt;
        self.eintraege.get_mut(a).map(|e| {
            e.1 = takt;
            e.0.clone()
        })
    }

    fn ablegen(&mut self, a: Anfrage, t: Option<gdk::Texture>) {
        let groesse = t
            .as_ref()
            .map_or(64, |t| (t.width() * t.height() * 4) as usize);
        self.takt += 1;
        if let Some(alt) = self.eintraege.insert(a, (t, self.takt, groesse)) {
            self.bytes -= alt.2;
        }
        self.bytes += groesse;
        while self.bytes > SPEICHER_MAX {
            let Some(aeltester) = self
                .eintraege
                .iter()
                .min_by_key(|(_, e)| e.1)
                .map(|(k, _)| k.clone())
            else {
                break;
            };
            if let Some(e) = self.eintraege.remove(&aeltester) {
                self.bytes -= e.2;
            }
        }
    }
}

#[derive(Default)]
struct Statistik {
    beginn: Option<Instant>,
    cache: u32,
    erzeugt: u32,
    fehl: u32,
}

/// The state on the main thread.
struct Zustand {
    regel: Regel,
    wurzel: PathBuf,
    /// Every bound image, by its address.
    bindungen: HashMap<usize, Bindung>,
    /// Images waiting for a request.
    wartende: HashMap<Anfrage, Vec<usize>>,
    speicher: Speicher,
    pool: Option<Pool>,
    statistik: Statistik,
}

thread_local! {
    /// `None` until [`registrieren`]'s startup hook ran: the views then
    /// show only theme icons.
    static ZUSTAND: RefCell<Option<Zustand>> = const { RefCell::new(None) };
}

fn schluessel(bild: &gtk4::Image) -> usize {
    bild.as_ptr() as usize
}

/// The number of worker threads: half the cores, two to four.
fn faeden() -> usize {
    std::thread::available_parallelism()
        .map(|n| (n.get() / 2).clamp(2, 4))
        .unwrap_or(2)
}

/// The **Bildquelle je Eintrag**: the one interface between the views
/// (icon, list, later compact view) and the thumbnails. A view calls
/// [`binden`] in its bind handler instead of setting the theme icon, and
/// [`loesen`] in its unbind handler.
pub mod bildquelle {
    pub use super::Groesse;
    use super::*;

    /// Shows `eintrag` in `bild`: its thumbnail if there is one (at once
    /// from memory, else when a worker has it), the theme icon until then
    /// or instead (QD-106: through `querencia_oberflaeche::symbole`).
    pub fn binden(bild: &gtk4::Image, eintrag: &Eintrag, groesse: Groesse) {
        let k = schluessel(bild);
        let textur = ZUSTAND.with(|z| {
            let mut z = z.borrow_mut();
            let z = z.as_mut()?;
            let anfrage = anfrage_fuer(z, eintrag, groesse);
            let alt = z.bindungen.insert(
                k,
                Bindung {
                    bild: bild.downgrade(),
                    eintrag: eintrag.clone(),
                    groesse,
                    anfrage: anfrage.clone(),
                },
            );
            // bound again to the same picture (zoom of other cells,
            // selection): keep waiting, no new job
            let gleich = anfrage.is_some() && alt.as_ref().is_some_and(|b| b.anfrage == anfrage);
            if let Some(a) = alt.and_then(|b| b.anfrage).filter(|_| !gleich) {
                weg_von(z, &a, k);
            }
            let anfrage = anfrage?;
            match z.speicher.holen(&anfrage) {
                Some(t) => t,
                None => {
                    if !gleich {
                        let neu = !z.wartende.contains_key(&anfrage);
                        z.wartende.entry(anfrage.clone()).or_default().push(k);
                        if neu {
                            einreihen(z, eintrag, anfrage);
                        }
                    }
                    None
                }
            }
        });
        match textur {
            Some(t) => zeigen(bild, &t, groesse),
            None => {
                querencia_oberflaeche::symbole::set(bild, &eintrag.daten().symbol);
                if bild.pixel_size() != groesse.symbol {
                    bild.set_pixel_size(groesse.symbol);
                }
            }
        }
    }

    /// The cell lets its entry go (unbind): its request is withdrawn when
    /// no other cell waits for it.
    pub fn loesen(bild: &gtk4::Image) {
        ZUSTAND.with(|z| {
            let mut z = z.borrow_mut();
            let Some(z) = z.as_mut() else { return };
            let k = schluessel(bild);
            let Some(alt) = z.bindungen.remove(&k) else {
                return;
            };
            if let Some(a) = alt.anfrage {
                weg_von(z, &a, k);
            }
        });
    }
}

fn weg_von(z: &mut Zustand, a: &Anfrage, k: usize) {
    let leer = z.wartende.get_mut(a).is_some_and(|v| {
        v.retain(|x| *x != k);
        v.is_empty()
    });
    if leer {
        z.wartende.remove(a);
        if let Some(p) = &z.pool {
            p.zuruecknehmen(a);
        }
    }
}

fn anfrage_fuer(z: &Zustand, e: &Eintrag, g: Groesse) -> Option<Anfrage> {
    let d = e.daten();
    let pfad = d.datei.path();
    let k = Kandidat {
        ist_ordner: d.ist_ordner,
        starter: d.starter,
        lokal: d.datei.is_native(),
        groesse: d.sort.groesse,
        typ: &d.typ,
        pfad: pfad.as_deref(),
    };
    (erlaubt(z.regel, &k, &z.wurzel) && erzeuger::moeglich(&d.typ)).then(|| Anfrage {
        uri: d.datei.uri().to_string(),
        mtime: d.sort.zeit,
        px: g.vorschau.max(1) as u32,
    })
}

fn einreihen(z: &mut Zustand, e: &Eintrag, anfrage: Anfrage) {
    let d = e.daten();
    let wurzel = z.wurzel.clone();
    let pool = z.pool.get_or_insert_with(|| {
        Pool::new(faeden(), wurzel, warteschlange::an_hauptfaden(angekommen))
    });
    if z.statistik.beginn.is_none() {
        z.statistik.beginn = Some(Instant::now());
    }
    pool.einreihen(Auftrag {
        quelle: erzeuger::Quelle {
            uri: anfrage.uri.clone(),
            pfad: d.datei.path(),
            typ: d.typ.clone(),
            mtime: d.sort.zeit,
            groesse: d.sort.groesse,
        },
        anfrage,
    });
}

fn zeigen(bild: &gtk4::Image, t: &gdk::Texture, g: Groesse) {
    bild.set_paintable(Some(t));
    if bild.pixel_size() != g.vorschau {
        bild.set_pixel_size(g.vorschau);
    }
}

/// A worker finished `e` (main thread).
fn angekommen(e: Ergebnis) {
    ZUSTAND.with(|z| {
        let mut z = z.borrow_mut();
        let Some(z) = z.as_mut() else { return };
        match e.herkunft {
            Herkunft::Cache => z.statistik.cache += 1,
            Herkunft::Erzeugt(_) => z.statistik.erzeugt += 1,
            Herkunft::Fehl => z.statistik.fehl += 1,
            Herkunft::KeinWeg | Herkunft::Abgebrochen => {}
        }
        if e.herkunft != Herkunft::Abgebrochen {
            z.speicher.ablegen(e.anfrage.clone(), e.textur.clone());
        }
        for k in z.wartende.remove(&e.anfrage).unwrap_or_default() {
            let Some(b) = z.bindungen.get(&k) else {
                continue;
            };
            if b.anfrage.as_ref() != Some(&e.anfrage) {
                continue;
            }
            if let (Some(bild), Some(t)) = (b.bild.upgrade(), &e.textur) {
                zeigen(&bild, t, b.groesse);
            }
        }
        if z.wartende.is_empty()
            && z.pool.as_ref().is_some_and(|p| p.offen() == (0, 0))
            && let Some(beginn) = z.statistik.beginn.take()
        {
            let s = std::mem::take(&mut z.statistik);
            protokoll(&format!(
                "Miniaturen fertig: {} aus dem Cache, {} erzeugt, {} fehlgeschlagen in {} ms",
                s.cache,
                s.erzeugt,
                s.fehl,
                beginn.elapsed().as_millis()
            ));
        }
    });
}

/// New settings: every bound cell is shown again under the new rule.
fn regel_setzen(regel: Regel) {
    let neu: Vec<(gtk4::Image, Eintrag, Groesse)> = ZUSTAND.with(|z| {
        let mut z = z.borrow_mut();
        let Some(z) = z.as_mut() else {
            return Vec::new();
        };
        if z.regel == regel {
            return Vec::new();
        }
        z.regel = regel;
        z.bindungen
            .values()
            .filter_map(|b| Some((b.bild.upgrade()?, b.eintrag.clone(), b.groesse)))
            .collect()
    });
    for (bild, e, g) in neu {
        bildquelle::binden(&bild, &e, g);
    }
}

/// After a change of folder: queued jobs no bound cell waits for any more
/// are dropped (the views unbind their cells, this catches the rest).
fn aufraeumen() {
    ZUSTAND.with(|z| {
        let mut z = z.borrow_mut();
        let Some(z) = z.as_mut() else { return };
        z.bindungen.retain(|_, b| b.bild.upgrade().is_some());
        let lebend: std::collections::HashSet<Anfrage> = z
            .bindungen
            .values()
            .filter_map(|b| b.anfrage.clone())
            .collect();
        z.wartende.retain(|a, _| lebend.contains(a));
        if let Some(p) = &z.pool {
            let n = p.sieben(|a| lebend.contains(a));
            if n > 0 {
                protokoll(&format!(
                    "Miniaturen: {n} Aufträge verworfen (Ordnerwechsel)"
                ));
            }
        }
    });
}

// Welle C2 (dateien-miniaturen-suche): registered in main.rs.
/// Starts the thumbnails: settings from `dateien.ini`, followed on every
/// change; the queue is cleaned after every change of folder.
pub fn registrieren(register: &mut Register) {
    register.anwendung(|app| {
        let regel = Regel::aus(&app.konfig().werte());
        ZUSTAND.with(|z| {
            z.replace(Some(Zustand {
                regel,
                wurzel: cache::wurzel(),
                bindungen: HashMap::new(),
                wartende: HashMap::new(),
                speicher: Speicher::default(),
                pool: None,
                statistik: Statistik::default(),
            }));
        });
        protokoll(&format!(
            "Miniaturen: {:?}, bis {} MB, {} Thumbnailer, {} Fäden",
            regel.wo,
            regel.max_bytes / 1024 / 1024,
            erzeuger::alle().len(),
            faeden()
        ));
        app.konfig().bei_aenderung(|w| regel_setzen(Regel::aus(w)));
        app.gtk().connect_shutdown(|_| {
            ZUSTAND.with(|z| {
                if let Some(z) = z.borrow_mut().as_mut() {
                    z.pool = None;
                }
            });
            erzeuger::aufraeumen();
        });
    });
    register.fenster(|f| {
        f.bei_ortswechsel(|_| {
            glib::idle_add_local_once(aufraeumen);
        })
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn k<'a>(typ: &'a str, groesse: u64, lokal: bool) -> Kandidat<'a> {
        Kandidat {
            ist_ordner: false,
            starter: false,
            lokal,
            groesse,
            typ,
            pfad: None,
        }
    }

    #[test]
    fn rules_follow_nautilus_2_30() {
        let w = Path::new("/c/thumbnails");
        let lokal = Regel {
            wo: Wo::NurLokal,
            max_bytes: 10 * 1024 * 1024,
        };
        assert!(erlaubt(lokal, &k("image/png", 1000, true), w));
        assert!(!erlaubt(lokal, &k("image/png", 1000, false), w), "entfernt");
        assert!(
            !erlaubt(lokal, &k("image/png", 10 * 1024 * 1024 + 1, true), w),
            "zu groß"
        );
        assert!(erlaubt(lokal, &k("image/png", 10 * 1024 * 1024, true), w));
        let immer = Regel {
            wo: Wo::Immer,
            ..lokal
        };
        assert!(erlaubt(immer, &k("image/png", 1, false), w));
        let nie = Regel {
            wo: Wo::Nie,
            ..lokal
        };
        assert!(!erlaubt(nie, &k("image/png", 1, true), w));
        let mut ordner = k("inode/directory", 0, true);
        ordner.ist_ordner = true;
        assert!(!erlaubt(immer, &ordner, w));
        let mut starter = k("application/x-desktop", 10, true);
        starter.starter = true;
        assert!(!erlaubt(immer, &starter, w));
        let p = Path::new("/c/thumbnails/normal/abc.png");
        let mut im_cache = k("image/png", 10, true);
        im_cache.pfad = Some(p);
        assert!(!erlaubt(immer, &im_cache, w), "keine Miniatur der Miniatur");
        let mut werte = Werte::default();
        assert_eq!(Regel::aus(&werte).wo, Wo::NurLokal);
        assert_eq!(Regel::aus(&werte).max_bytes, 10 * 1024 * 1024);
        werte.vorschau_max_mb = 3;
        assert_eq!(Regel::aus(&werte).max_bytes, 3 * 1024 * 1024);
    }

    #[test]
    fn icon_view_thumbnails_are_a_third_larger() {
        let v: Vec<i32> = [24, 32, 48, 72, 96]
            .into_iter()
            .map(|s| Groesse::symbolansicht(s).vorschau)
            .collect();
        assert_eq!(v, [32, 43, 64, 96, 128]);
        assert_eq!(Groesse::liste(24).vorschau, 24);
    }
}
