//! The search thread: walks the folder tree below the shown folder with
//! `g_file_enumerate_children` (breadth first, as Nautilus 2.30's simple
//! search engine), compares every display name with the words of the
//! query and hands the hits in batches to the main thread. No index.
//!
//! With „Auch Dateiinhalt“ (0.3.0, off by default) a file whose name does
//! not match is read as well, grep-like, when it is a text file
//! ([`im_inhalt`]: a local file up to [`Grenzen::datei_bytes`], no zero
//! byte): it is a hit when every word of the query is in its text; the
//! first line with one of the words comes along ([`Fundstelle`]).
//!
//! Rules of Nautilus 2.30 (`nautilus-search-engine-simple.c`): name and
//! query are compared in NFD and lower case, a hit contains every word of
//! the query; hidden files and folders are left out (here: unless the
//! window shows hidden files); a folder reached twice (bind mounts, links)
//! is read once (`id::file`); symbolic links to folders are not followed.
//! Limits ([`Grenzen`]): 64 folder levels, 50,000 hits.

use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use gtk4::prelude::*;
use gtk4::{gio, glib};

use crate::eintrag;

/// The words of a query: NFD, lower case, split at white space.
pub fn worte(anfrage: &str) -> Vec<String> {
    normal(anfrage)
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

/// A name in the form the words are compared with.
pub fn normal(text: &str) -> String {
    // G_NORMALIZE_DEFAULT is NFD
    glib::normalize(text, glib::NormalizeMode::Default).to_lowercase()
}

/// Whether `name` (display name) contains every word.
pub fn passt(worte: &[String], name: &str) -> bool {
    if worte.is_empty() {
        return false;
    }
    let n = normal(name);
    worte.iter().all(|w| n.contains(w.as_str()))
}

/// Where a word of the query stands in a file's text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fundstelle {
    /// Line number, from 1.
    pub zeile: u32,
    /// The line, shortened around the word ([`ausschnitt`]).
    pub text: String,
}

/// Characters of a line [`Fundstelle::text`] keeps at most.
pub const AUSSCHNITT: usize = 100;

/// `zeile` with white space folded, cut to [`AUSSCHNITT`] characters so
/// that `wort` (lower case) stays visible; cuts are marked with "…".
pub fn ausschnitt(zeile: &str, wort: &str) -> String {
    let zeichen: Vec<char> = zeile
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .collect();
    if zeichen.len() <= AUSSCHNITT {
        return zeichen.into_iter().collect();
    }
    let klein: String = zeichen.iter().collect::<String>().to_lowercase();
    // the word's position in characters (lower case may change the length
    // of some characters: then the start)
    let pos = klein
        .find(wort)
        .filter(|_| klein.chars().count() == zeichen.len())
        .map_or(0, |b| klein[..b].chars().count());
    let anfang = pos
        .saturating_sub(AUSSCHNITT / 3)
        .min(zeichen.len() - AUSSCHNITT);
    let mut out: String = zeichen[anfang..anfang + AUSSCHNITT].iter().collect();
    if anfang > 0 {
        out.insert(0, '…');
    }
    if anfang + AUSSCHNITT < zeichen.len() {
        out.push('…');
    }
    out
}

/// Searches the text of `datei` for `worte`: `Some` when every word is in
/// it, with the first line holding one of them. Only local files up to
/// `max` bytes; a zero byte marks a binary file (not searched). Invalid
/// UTF-8 is read lossily (Latin-1 umlauts then do not match).
pub fn im_inhalt(
    datei: &gio::File,
    worte: &[String],
    max: u64,
    abbruch: &gio::Cancellable,
) -> Option<Fundstelle> {
    use std::io::Read;
    if worte.is_empty() {
        return None;
    }
    let pfad = datei.path()?;
    let mut daten = Vec::new();
    std::fs::File::open(&pfad)
        .ok()?
        .take(max + 1)
        .read_to_end(&mut daten)
        .ok()?;
    if daten.len() as u64 > max || daten.contains(&0) {
        return None;
    }
    let text = String::from_utf8_lossy(&daten);
    let mut gefunden = vec![false; worte.len()];
    let mut erste: Option<Fundstelle> = None;
    for (i, zeile) in text.lines().enumerate() {
        if i % 4096 == 0 && abbruch.is_cancelled() {
            return None;
        }
        let n = normal(zeile);
        for (k, w) in worte.iter().enumerate() {
            if n.contains(w.as_str()) {
                gefunden[k] = true;
                if erste.is_none() {
                    erste = Some(Fundstelle {
                        zeile: u32::try_from(i + 1).unwrap_or(u32::MAX),
                        text: ausschnitt(zeile, w),
                    });
                }
            }
        }
        if erste.is_some() && gefunden.iter().all(|g| *g) {
            break;
        }
    }
    if gefunden.iter().all(|g| *g) {
        erste
    } else {
        None
    }
}

/// One hit, as plain data (a `gio::FileInfo` may not cross threads); the
/// main thread makes an `Eintrag` of it ([`Fund::eintrag`]).
#[derive(Clone, Debug)]
pub struct Fund {
    pub ordner: gio::File,
    pub name: String,
    pub anzeige: String,
    pub art: gio::FileType,
    pub groesse: i64,
    pub typ: Option<String>,
    pub verborgen: bool,
    pub sicherung: bool,
    pub verweis: bool,
    pub verweisziel: Option<String>,
    pub zeit: Option<(i64, i32)>,
    pub modus: Option<u32>,
    pub besitzer: Option<String>,
    pub gruppe: Option<String>,
    pub darf: [Option<bool>; 6],
    /// A hit in the text (not in the name): where.
    pub fundstelle: Option<Fundstelle>,
}

const DARF: [&str; 6] = [
    "access::can-read",
    "access::can-write",
    "access::can-execute",
    "access::can-rename",
    "access::can-trash",
    "access::can-delete",
];

impl Fund {
    fn aus(ordner: &gio::File, info: &gio::FileInfo) -> Self {
        let s = |a: &str| info.attribute_string(a).map(|v| v.to_string());
        let b = |a: &str| info.has_attribute(a).then(|| info.boolean(a));
        Self {
            ordner: ordner.clone(),
            name: info.name().to_string_lossy().into_owned(),
            anzeige: info.display_name().to_string(),
            art: info.file_type(),
            groesse: info.size(),
            typ: s("standard::fast-content-type"),
            verborgen: info.is_hidden(),
            sicherung: info.is_backup(),
            verweis: info.is_symlink(),
            verweisziel: s("standard::symlink-target"),
            zeit: info.has_attribute("time::modified").then(|| {
                (
                    info.attribute_uint64("time::modified") as i64,
                    info.attribute_uint32("time::modified-usec") as i32,
                )
            }),
            modus: info
                .has_attribute("unix::mode")
                .then(|| info.attribute_uint32("unix::mode")),
            besitzer: s("owner::user"),
            gruppe: s("owner::group"),
            darf: DARF.map(b),
            fundstelle: None,
        }
    }

    /// The `gio::FileInfo` again (main thread), then an `Eintrag`.
    pub fn eintrag(&self) -> eintrag::Eintrag {
        let info = gio::FileInfo::new();
        info.set_name(&self.name);
        info.set_display_name(&self.anzeige);
        info.set_file_type(self.art);
        info.set_size(self.groesse);
        if let Some(t) = &self.typ {
            info.set_attribute_string("standard::fast-content-type", t);
        }
        info.set_is_hidden(self.verborgen);
        info.set_attribute_boolean("standard::is-backup", self.sicherung);
        info.set_is_symlink(self.verweis);
        if let Some(z) = &self.verweisziel {
            info.set_symlink_target(z);
        }
        if let Some((s, us)) = self.zeit {
            info.set_attribute_uint64("time::modified", s as u64);
            info.set_attribute_uint32("time::modified-usec", us as u32);
        }
        if let Some(m) = self.modus {
            info.set_attribute_uint32("unix::mode", m);
        }
        if let Some(o) = &self.besitzer {
            info.set_attribute_string("owner::user", o);
        }
        if let Some(g) = &self.gruppe {
            info.set_attribute_string("owner::group", g);
        }
        for (a, w) in DARF.iter().zip(self.darf) {
            if let Some(w) = w {
                info.set_attribute_boolean(a, w);
            }
        }
        eintrag::Eintrag::aus_info(&self.ordner, &info)
    }
}

/// What the search thread reports.
#[derive(Debug)]
pub enum Meldung {
    /// A batch of hits.
    Funde(Vec<Fund>),
    /// Done (or stopped): entries looked at, folders read, folders that
    /// could not be read, milliseconds.
    Ende {
        eintraege: u64,
        ordner: u64,
        fehler: u64,
        ms: u128,
        abgebrochen: bool,
        /// Stopped at [`Grenzen`] (hits or depth).
        begrenzt: bool,
        /// Files whose text was searched.
        inhalte: u64,
    },
}

/// Limits of a search: folders deeper than `tiefe` below the start are
/// not read, after `treffer` hits it stops.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Grenzen {
    pub tiefe: u32,
    pub treffer: usize,
    /// Larger files are not searched for their text.
    pub datei_bytes: u64,
}

impl Default for Grenzen {
    /// 64 levels, 50,000 hits (a home folder of half a million files
    /// stays answerable; the list of hits stays below ~200 MB), texts up
    /// to 10 MiB.
    fn default() -> Self {
        Self {
            tiefe: 64,
            treffer: 50_000,
            datei_bytes: 10 * 1024 * 1024,
        }
    }
}

/// A batch goes out after this many hits or this time.
const BATCH: usize = 500;
const BATCH_ZEIT: Duration = Duration::from_millis(150);

/// Number of the last search started; a running search whose number is
/// older stops (only one search per window runs; the number is per
/// process, the window checks its own).
static NUMMER: AtomicU64 = AtomicU64::new(0);

pub fn neue_nummer() -> u64 {
    NUMMER.fetch_add(1, Ordering::Relaxed) + 1
}

/// Searches below `wurzel` for `worte` (in the names; with `inhalt` also
/// in the texts); `melden` gets the batches (on the search thread). Stops
/// when `abbruch` is cancelled.
#[allow(clippy::too_many_arguments)]
pub fn suchen(
    wurzel: &gio::File,
    worte: &[String],
    verborgene: bool,
    inhalt: bool,
    grenzen: Grenzen,
    abbruch: &gio::Cancellable,
    melden: &dyn Fn(Meldung),
) {
    let start = Instant::now();
    let attribute = format!("{},id::file", eintrag::attribute_fuer(wurzel));
    let mut warteschlange = VecDeque::from([(wurzel.clone(), 0u32)]);
    let mut gesehen: HashSet<String> = HashSet::new();
    // the start folder counts as seen (a bind mount back to it is a loop)
    if let Some(id) = wurzel
        .query_info("id::file", gio::FileQueryInfoFlags::NONE, Some(abbruch))
        .ok()
        .and_then(|i| i.attribute_string("id::file"))
    {
        gesehen.insert(id.to_string());
    }
    let mut funde = Vec::new();
    let mut zuletzt = Instant::now();
    let (mut eintraege, mut ordner, mut fehler, mut treffer) = (0u64, 0u64, 0u64, 0usize);
    let mut begrenzt = false;
    let mut inhalte = 0u64;
    'ordner: while let Some((dir, tiefe)) = warteschlange.pop_front() {
        if abbruch.is_cancelled() {
            break;
        }
        let liste = match dir.enumerate_children(
            &attribute,
            gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
            Some(abbruch),
        ) {
            Ok(l) => l,
            Err(_) => {
                fehler += 1;
                continue;
            }
        };
        ordner += 1;
        loop {
            let info = match liste.next_file(Some(abbruch)) {
                Ok(Some(i)) => i,
                Ok(None) => break,
                Err(_) => {
                    fehler += 1;
                    break;
                }
            };
            eintraege += 1;
            if !verborgene && (info.is_hidden() || info.is_backup()) {
                continue;
            }
            let mut fund = None;
            if passt(worte, &info.display_name()) {
                fund = Some(Fund::aus(&dir, &info));
            } else if inhalt
                && info.file_type() == gio::FileType::Regular
                && u64::try_from(info.size()).is_ok_and(|g| g <= grenzen.datei_bytes)
            {
                inhalte += 1;
                if let Some(stelle) =
                    im_inhalt(&dir.child(info.name()), worte, grenzen.datei_bytes, abbruch)
                {
                    let mut f = Fund::aus(&dir, &info);
                    f.fundstelle = Some(stelle);
                    fund = Some(f);
                }
            }
            if let Some(f) = fund {
                funde.push(f);
                treffer += 1;
                if treffer >= grenzen.treffer {
                    begrenzt = true;
                    let _ = liste.close(gio::Cancellable::NONE);
                    break 'ordner;
                }
            }
            // symbolic links are never entered (NOFOLLOW: their type is
            // SymbolicLink), a folder reached twice is read once
            if info.file_type() == gio::FileType::Directory {
                if tiefe + 1 > grenzen.tiefe {
                    begrenzt = true;
                    continue;
                }
                let id = info
                    .attribute_string("id::file")
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                if id.is_empty() || gesehen.insert(id) {
                    warteschlange.push_back((dir.child(info.name()), tiefe + 1));
                }
            }
            if funde.len() >= BATCH || (!funde.is_empty() && zuletzt.elapsed() >= BATCH_ZEIT) {
                melden(Meldung::Funde(std::mem::take(&mut funde)));
                zuletzt = Instant::now();
            }
        }
        let _ = liste.close(gio::Cancellable::NONE);
    }
    if !funde.is_empty() {
        melden(Meldung::Funde(funde));
    }
    melden(Meldung::Ende {
        eintraege,
        ordner,
        fehler,
        ms: start.elapsed().as_millis(),
        abgebrochen: abbruch.is_cancelled(),
        begrenzt,
        inhalte,
    });
}

/// Starts a search thread; its reports reach `empfang` on the main thread
/// (GLib's default context) together with the search's number.
pub fn starten(
    nummer: u64,
    wurzel: gio::File,
    worte: Vec<String>,
    verborgene: bool,
    inhalt: bool,
    abbruch: gio::Cancellable,
    empfang: fn(u64, Meldung),
) -> std::io::Result<()> {
    let ab = Arc::new(abbruch);
    std::thread::Builder::new()
        .name("qd-suche".to_owned())
        .spawn(move || {
            suchen(
                &wurzel,
                &worte,
                verborgene,
                inhalt,
                Grenzen::default(),
                &ab,
                &|m| {
                    glib::idle_add_once(move || empfang(nummer, m));
                },
            );
        })
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[test]
    fn every_word_must_be_in_the_name() {
        let w = worte("  Rechnung  2026 ");
        assert_eq!(w, ["rechnung", "2026"]);
        assert!(passt(&w, "Rechnung-März-2026.pdf"));
        assert!(passt(&w, "2026 RECHNUNG.odt"));
        assert!(!passt(&w, "Rechnung 2025.pdf"));
        assert!(!passt(&[], "egal"), "leere Suche findet nichts");
        // accents: as Nautilus 2.30 (NFD), "cafe" finds "Café", "café" too
        assert!(passt(&worte("cafe"), "Café.txt"));
        assert!(passt(&worte("café"), "CAFÉ.txt"));
        assert!(passt(&worte("ü"), "Übersicht"));
    }

    #[test]
    fn the_tree_is_searched_breadth_first_without_hidden_files() {
        let d = std::env::temp_dir().join(format!("qd-suche-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        for p in ["a/b/c", ".versteckt", "x"] {
            std::fs::create_dir_all(d.join(p)).unwrap();
        }
        for p in [
            "rechnung-1.txt",
            "a/Rechnung 2.pdf",
            "a/b/c/rechnung3",
            ".versteckt/rechnung4",
            ".rechnung5",
            "x/anderes",
            "a/b/rechnung6~",
        ] {
            std::fs::write(d.join(p), b"%PDF-1.4").unwrap();
        }
        std::os::unix::fs::symlink(d.join("a"), d.join("x/schleife")).unwrap();
        let namen = |verborgene: bool| {
            let funde = RefCell::new(Vec::new());
            let ende = RefCell::new(None);
            suchen(
                &gio::File::for_path(&d),
                &worte("rechnung"),
                verborgene,
                false,
                Grenzen::default(),
                &gio::Cancellable::new(),
                &|m| match m {
                    Meldung::Funde(f) => funde.borrow_mut().extend(f),
                    Meldung::Ende {
                        eintraege, ordner, ..
                    } => *ende.borrow_mut() = Some((eintraege, ordner)),
                },
            );
            let mut n: Vec<String> = funde.borrow().iter().map(|f| f.name.clone()).collect();
            n.sort();
            (n, ende.borrow().unwrap())
        };
        let (n, (eintraege, ordner)) = namen(false);
        assert_eq!(n, ["Rechnung 2.pdf", "rechnung-1.txt", "rechnung3"]);
        // the link to "a" is not followed: a, a/b, a/b/c, x and the root
        assert_eq!(ordner, 5);
        assert!(eintraege >= 10);
        let (n, _) = namen(true);
        assert_eq!(n.len(), 6, "{n:?}");
        // a stopped search reports the end
        let c = gio::Cancellable::new();
        c.cancel();
        let ende = RefCell::new(false);
        suchen(
            &gio::File::for_path(&d),
            &worte("r"),
            false,
            false,
            Grenzen::default(),
            &c,
            &|m| {
                if let Meldung::Ende { abgebrochen, .. } = m {
                    *ende.borrow_mut() = abgebrochen;
                }
            },
        );
        assert!(*ende.borrow());
        // a hit becomes an entry of its own folder
        let f = Fund::aus(
            &gio::File::for_path(d.join("a")),
            &gio::File::for_path(d.join("a/Rechnung 2.pdf"))
                .query_info(
                    &format!("{},id::file", eintrag::ATTRIBUTE),
                    gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
                    gio::Cancellable::NONE,
                )
                .unwrap(),
        );
        let e = f.eintrag();
        assert_eq!(e.name(), "Rechnung 2.pdf");
        assert_eq!(e.datei().path().unwrap(), d.join("a/Rechnung 2.pdf"));
        assert_eq!(e.daten().typ, "application/pdf");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn limits_stop_the_search_and_link_loops_are_not_entered() {
        let d = std::env::temp_dir().join(format!("qd-suche-grenzen-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let tief = d.join("e1/e2/e3/e4");
        std::fs::create_dir_all(&tief).unwrap();
        for (i, p) in ["", "e1", "e1/e2", "e1/e2/e3", "e1/e2/e3/e4"]
            .iter()
            .enumerate()
        {
            for k in 0..3 {
                std::fs::write(d.join(p).join(format!("treffer-{i}-{k}")), b"x").unwrap();
            }
        }
        // links back to the start and to itself: never entered
        std::os::unix::fs::symlink(&d, tief.join("zurueck")).unwrap();
        std::os::unix::fs::symlink(d.join("e1/schleife"), d.join("e1/schleife")).unwrap();
        let lauf = |g: Grenzen| {
            let n = RefCell::new(0usize);
            let ende = RefCell::new((0u64, false));
            suchen(
                &gio::File::for_path(&d),
                &worte("treffer"),
                false,
                false,
                g,
                &gio::Cancellable::new(),
                &|m| match m {
                    Meldung::Funde(f) => *n.borrow_mut() += f.len(),
                    Meldung::Ende {
                        ordner, begrenzt, ..
                    } => *ende.borrow_mut() = (ordner, begrenzt),
                },
            );
            (n.into_inner(), ende.into_inner())
        };
        assert_eq!(
            lauf(Grenzen::default()),
            (15, (5, false)),
            "alle, ohne Schleife"
        );
        let (n, (ordner, begrenzt)) = lauf(Grenzen {
            tiefe: 2,
            treffer: 1000,
            ..Grenzen::default()
        });
        assert_eq!(
            (n, ordner, begrenzt),
            (9, 3, true),
            "bis zwei Ebenen unter dem Start"
        );
        let (n, (_, begrenzt)) = lauf(Grenzen {
            tiefe: 64,
            treffer: 4,
            ..Grenzen::default()
        });
        assert_eq!((n, begrenzt), (4, true), "nach 4 Treffern Schluss");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn contents_are_searched_only_with_the_switch_and_only_in_texts() {
        let d = std::env::temp_dir().join(format!("qd-suche-inhalt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("unter")).unwrap();
        std::fs::write(
            d.join("brief.txt"),
            "Hallo,\nanbei die Rechnung für März.\nGruß\n",
        )
        .unwrap();
        std::fs::write(
            d.join("unter/notiz.md"),
            "# Notiz\n\nRECHNUNG bezahlt, März 2026\n",
        )
        .unwrap();
        std::fs::write(d.join("nur-rechnung.txt"), "Rechnung ohne Monat\n").unwrap();
        // a zero byte: binary, not searched
        std::fs::write(d.join("bild.bin"), b"Rechnung M\xc3\xa4rz\0\x01").unwrap();
        std::fs::write(d.join("gross.txt"), "Rechnung März ".repeat(100)).unwrap();
        std::fs::write(d.join("März-Rechnung.odt"), b"PK\x03\x04").unwrap();
        let lauf = |inhalt: bool, max: u64| {
            let funde = RefCell::new(Vec::new());
            let gelesen = RefCell::new(0);
            suchen(
                &gio::File::for_path(&d),
                &worte("rechnung märz"),
                false,
                inhalt,
                Grenzen {
                    datei_bytes: max,
                    ..Grenzen::default()
                },
                &gio::Cancellable::new(),
                &|m| match m {
                    Meldung::Funde(f) => funde.borrow_mut().extend(f),
                    Meldung::Ende { inhalte, .. } => *gelesen.borrow_mut() = inhalte,
                },
            );
            let mut f: Vec<(String, Option<u32>)> = funde
                .borrow()
                .iter()
                .map(|f| (f.name.clone(), f.fundstelle.as_ref().map(|s| s.zeile)))
                .collect();
            f.sort();
            (f, gelesen.into_inner())
        };
        // without the switch: names only
        assert_eq!(
            lauf(false, 1000),
            (vec![("März-Rechnung.odt".to_owned(), None)], 0)
        );
        // with it: every word somewhere in the text, the line of the first
        let (f, gelesen) = lauf(true, 1000);
        assert_eq!(
            f,
            vec![
                ("März-Rechnung.odt".to_owned(), None),
                ("brief.txt".to_owned(), Some(2)),
                ("notiz.md".to_owned(), Some(3)),
            ]
        );
        // read: brief, notiz, nur-rechnung, bild (gross.txt is too large)
        assert_eq!(gelesen, 4);
        let (f, _) = lauf(true, 10 * 1024 * 1024);
        assert!(f.iter().any(|(n, z)| n == "gross.txt" && *z == Some(1)));
        let stelle = im_inhalt(
            &gio::File::for_path(d.join("brief.txt")),
            &worte("märz"),
            1000,
            &gio::Cancellable::new(),
        )
        .unwrap();
        assert_eq!(stelle.text, "anbei die Rechnung für März.");
        assert!(
            im_inhalt(
                &gio::File::for_path(d.join("brief.txt")),
                &worte("januar"),
                1000,
                &gio::Cancellable::new()
            )
            .is_none()
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_long_line_is_cut_around_the_word() {
        assert_eq!(ausschnitt("  kurz \t und   gut ", "gut"), "kurz und gut");
        let lang = format!("{} Treffer {}", "a".repeat(150), "b".repeat(150));
        let a = ausschnitt(&lang, "treffer");
        assert!(a.starts_with('…') && a.ends_with('…'), "{a}");
        assert!(a.contains("Treffer"));
        assert_eq!(a.chars().count(), AUSSCHNITT + 2);
        let vorn = ausschnitt(&format!("Treffer {}", "c".repeat(200)), "treffer");
        assert!(vorn.starts_with("Treffer") && vorn.ends_with('…'));
    }
}
