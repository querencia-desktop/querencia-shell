//! The search thread: walks the folder tree below the shown folder with
//! `g_file_enumerate_children` (breadth first, as Nautilus 2.30's simple
//! search engine), compares every display name with the words of the
//! query and hands the hits in batches to the main thread. No index, no
//! file contents (decision for Querencia Dateien: names only).
//!
//! Rules of Nautilus 2.30 (`nautilus-search-engine-simple.c`): name and
//! query are compared in NFD and lower case, a hit contains every word of
//! the query; hidden files and folders are left out (here: unless the
//! window shows hidden files); a folder reached twice (bind mounts, links)
//! is read once (`id::file`); symbolic links to folders are not followed.

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
    },
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

/// Searches below `wurzel` for `worte`; `melden` gets the batches (on the
/// search thread). Stops when `abbruch` is cancelled.
pub fn suchen(
    wurzel: &gio::File,
    worte: &[String],
    verborgene: bool,
    abbruch: &gio::Cancellable,
    melden: &dyn Fn(Meldung),
) {
    let start = Instant::now();
    let attribute = format!("{},id::file", eintrag::attribute_fuer(wurzel));
    let mut warteschlange = VecDeque::from([wurzel.clone()]);
    let mut gesehen: HashSet<String> = HashSet::new();
    let mut funde = Vec::new();
    let mut zuletzt = Instant::now();
    let (mut eintraege, mut ordner, mut fehler) = (0u64, 0u64, 0u64);
    while let Some(dir) = warteschlange.pop_front() {
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
            if passt(worte, &info.display_name()) {
                funde.push(Fund::aus(&dir, &info));
            }
            if info.file_type() == gio::FileType::Directory {
                let id = info
                    .attribute_string("id::file")
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                if id.is_empty() || gesehen.insert(id) {
                    warteschlange.push_back(dir.child(info.name()));
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
    });
}

/// Starts a search thread; its reports reach `empfang` on the main thread
/// (GLib's default context) together with the search's number.
pub fn starten(
    nummer: u64,
    wurzel: gio::File,
    worte: Vec<String>,
    verborgene: bool,
    abbruch: gio::Cancellable,
    empfang: fn(u64, Meldung),
) -> std::io::Result<()> {
    let ab = Arc::new(abbruch);
    std::thread::Builder::new()
        .name("qd-suche".to_owned())
        .spawn(move || {
            suchen(&wurzel, &worte, verborgene, &ab, &|m| {
                glib::idle_add_once(move || empfang(nummer, m));
            });
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
        suchen(&gio::File::for_path(&d), &worte("r"), false, &c, &|m| {
            if let Meldung::Ende { abgebrochen, .. } = m {
                *ende.borrow_mut() = abgebrochen;
            }
        });
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
}
