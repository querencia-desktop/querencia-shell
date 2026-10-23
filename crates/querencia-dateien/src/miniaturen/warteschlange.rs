//! The worker threads of the thumbnails: a queue in arrival order (the
//! views bind their visible cells from the top), a few threads that read
//! the cache or make a thumbnail, and the delivery of the finished picture
//! to the main thread. A job nobody waits for any more (its cell was
//! unbound, the folder changed) is taken out of the queue; a running
//! thumbnailer process is stopped.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};

use gtk4::{gdk, glib};

use super::bild::{Bild, RAHMEN_LINKS, RAHMEN_RECHTS};
use super::cache::{self, Eimer};
use super::erzeuger::{self, Fehler, Quelle, Weg};

/// What a view asks for: the thumbnail of a file in one state at one size.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Anfrage {
    pub uri: String,
    pub mtime: i64,
    /// Long edge of the picture in the view (frame included).
    pub px: u32,
}

/// One job of the queue.
pub struct Auftrag {
    pub anfrage: Anfrage,
    pub quelle: Quelle,
}

/// Where a picture came from (statistics, log of the checks).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Herkunft {
    Cache,
    Erzeugt(Weg),
    /// A failure is noted in `fail/` (now or before).
    Fehl,
    KeinWeg,
    Abgebrochen,
}

pub struct Ergebnis {
    pub anfrage: Anfrage,
    pub textur: Option<gdk::Texture>,
    pub herkunft: Herkunft,
}

/// Frames from this size on (Nautilus 2.30 left small icons unframed).
pub const RAHMEN_AB: u32 = 48;

/// The picture for the view: scaled to `px` and framed when it is opaque
/// and big enough.
pub fn anzeigen(bild: &Bild, px: u32) -> Bild {
    let rahmen = px >= RAHMEN_AB && !bild.hat_transparenz();
    if rahmen {
        bild.verkleinert(px - RAHMEN_LINKS - RAHMEN_RECHTS)
            .gerahmt()
    } else {
        bild.verkleinert(px)
    }
}

/// Cache first (the bucket of the size, for `normal` also a valid `large`
/// one, as Querencia Files writes them), else a noted failure, else make
/// it and write it to the cache.
pub fn bearbeiten(wurzel: &Path, auftrag: &Auftrag, abbruch: &AtomicBool) -> Ergebnis {
    let a = &auftrag.anfrage;
    let q = &auftrag.quelle;
    let eimer = Eimer::fuer(a.px);
    let mut eimer_folge = vec![eimer];
    if eimer == Eimer::Normal {
        eimer_folge.push(Eimer::Gross);
    }
    let mut herkunft = Herkunft::Cache;
    let mut png = eimer_folge
        .iter()
        .find_map(|e| cache::lesen(&cache::pfad(wurzel, *e, &q.uri), &q.uri, q.mtime));
    if png.is_none() {
        if cache::fehlgeschlagen(wurzel, &q.uri, q.mtime) {
            return leer(a, Herkunft::Fehl);
        }
        match erzeuger::erzeugen(q, eimer, abbruch) {
            Ok((neu, weg)) => {
                if let Err(e) = cache::schreiben(&cache::pfad(wurzel, eimer, &q.uri), &neu) {
                    crate::log(&format!("Miniatur nicht gespeichert: {e}"));
                }
                herkunft = Herkunft::Erzeugt(weg);
                png = Some(neu);
            }
            Err(Fehler::KeinWeg) => return leer(a, Herkunft::KeinWeg),
            Err(Fehler::Abgebrochen) => return leer(a, Herkunft::Abgebrochen),
            Err(Fehler::Fehlgeschlagen(grund)) => {
                crate::ansichtskern::protokoll(&format!("Miniatur fehlgeschlagen: {grund}"));
                if let Some(fehl) = erzeuger::fehl_png(q) {
                    let _ = cache::schreiben(&cache::fehl_pfad(wurzel, &q.uri), &fehl);
                }
                return leer(a, Herkunft::Fehl);
            }
        }
    }
    let Some(png) = png else {
        return leer(a, Herkunft::Fehl);
    };
    let bild = gdk::Texture::from_bytes(&glib::Bytes::from_owned(png))
        .ok()
        .and_then(|t| Bild::aus_textur(&t));
    match bild {
        Some(b) => Ergebnis {
            anfrage: a.clone(),
            textur: Some(anzeigen(&b, a.px).textur()),
            herkunft,
        },
        None => leer(a, Herkunft::Fehl),
    }
}

fn leer(a: &Anfrage, herkunft: Herkunft) -> Ergebnis {
    Ergebnis {
        anfrage: a.clone(),
        textur: None,
        herkunft,
    }
}

type Ausliefern = Arc<dyn Fn(Ergebnis) + Send + Sync>;

#[derive(Default)]
struct Schlange {
    wartend: VecDeque<Auftrag>,
    laufend: HashMap<Anfrage, Arc<AtomicBool>>,
    ende: bool,
}

struct Geteilt {
    schlange: Mutex<Schlange>,
    signal: Condvar,
}

/// The worker threads (started on first use).
pub struct Pool {
    geteilt: Arc<Geteilt>,
    faeden: Vec<std::thread::JoinHandle<()>>,
}

fn sperren(g: &Geteilt) -> MutexGuard<'_, Schlange> {
    g.schlange.lock().unwrap_or_else(|e| e.into_inner())
}

impl Pool {
    /// `anzahl` threads; `ausliefern` gets every result on the worker
    /// thread (it passes them on to the main thread).
    pub fn new(anzahl: usize, wurzel: PathBuf, ausliefern: Ausliefern) -> Self {
        let geteilt = Arc::new(Geteilt {
            schlange: Mutex::new(Schlange::default()),
            signal: Condvar::new(),
        });
        let faeden = (0..anzahl.max(1))
            .map(|i| {
                let g = geteilt.clone();
                let wurzel = wurzel.clone();
                let aus = ausliefern.clone();
                std::thread::Builder::new()
                    .name(format!("qd-miniatur-{i}"))
                    .spawn(move || arbeiten(&g, &wurzel, &aus))
                    .expect("Faden für Miniaturen")
            })
            .collect();
        Self { geteilt, faeden }
    }

    pub fn einreihen(&self, auftrag: Auftrag) {
        sperren(&self.geteilt).wartend.push_back(auftrag);
        self.geteilt.signal.notify_one();
    }

    /// Withdraws `anfrage`: out of the queue, or stopped while it runs.
    pub fn zuruecknehmen(&self, anfrage: &Anfrage) {
        let mut s = sperren(&self.geteilt);
        s.wartend.retain(|a| &a.anfrage != anfrage);
        if let Some(abbruch) = s.laufend.get(anfrage) {
            abbruch.store(true, Ordering::Relaxed);
        }
    }

    /// Keeps only the queued jobs `behalten` agrees to; the number dropped.
    pub fn sieben(&self, behalten: impl Fn(&Anfrage) -> bool) -> usize {
        let mut s = sperren(&self.geteilt);
        let vorher = s.wartend.len();
        s.wartend.retain(|a| behalten(&a.anfrage));
        vorher - s.wartend.len()
    }

    /// Jobs waiting and running.
    pub fn offen(&self) -> (usize, usize) {
        let s = sperren(&self.geteilt);
        (s.wartend.len(), s.laufend.len())
    }
}

impl Drop for Pool {
    fn drop(&mut self) {
        {
            let mut s = sperren(&self.geteilt);
            s.ende = true;
            s.wartend.clear();
            for a in s.laufend.values() {
                a.store(true, Ordering::Relaxed);
            }
        }
        self.geteilt.signal.notify_all();
        for f in self.faeden.drain(..) {
            let _ = f.join();
        }
    }
}

fn arbeiten(g: &Geteilt, wurzel: &Path, ausliefern: &Ausliefern) {
    loop {
        let (auftrag, abbruch) = {
            let mut s = sperren(g);
            loop {
                if s.ende {
                    return;
                }
                if let Some(a) = s.wartend.pop_front() {
                    let abbruch = Arc::new(AtomicBool::new(false));
                    s.laufend.insert(a.anfrage.clone(), abbruch.clone());
                    break (a, abbruch);
                }
                s = g.signal.wait(s).unwrap_or_else(|e| e.into_inner());
            }
        };
        let ergebnis = bearbeiten(wurzel, &auftrag, &abbruch);
        sperren(g).laufend.remove(&auftrag.anfrage);
        ausliefern(ergebnis);
    }
}

/// Hands every result to the main thread (GLib's default context).
pub fn an_hauptfaden(f: fn(Ergebnis)) -> Ausliefern {
    Arc::new(move |e: Ergebnis| {
        glib::idle_add_once(move || f(e));
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use gtk4::prelude::*;
    use std::sync::mpsc;

    fn auftrag(uri: &str, typ: &str) -> Auftrag {
        Auftrag {
            anfrage: Anfrage {
                uri: uri.to_owned(),
                mtime: 1,
                px: 64,
            },
            quelle: Quelle {
                uri: uri.to_owned(),
                pfad: None,
                typ: typ.to_owned(),
                mtime: 1,
                groesse: 1,
            },
        }
    }

    #[test]
    fn opaque_pictures_get_a_frame_from_48_px() {
        let mut b = Bild::leer(400, 300);
        b.px.chunks_exact_mut(4).for_each(|p| p.fill(255));
        let g = anzeigen(&b, 64);
        assert_eq!(g.breite, 64);
        assert_eq!(g.hoehe, 44 + RAHMEN_LINKS + RAHMEN_RECHTS);
        let k = anzeigen(&b, 24);
        assert_eq!((k.breite, k.hoehe), (24, 18), "ohne Rahmen");
        let durchsichtig = Bild::leer(400, 300);
        assert_eq!(anzeigen(&durchsichtig, 64).breite, 64);
        assert_eq!(anzeigen(&durchsichtig, 64).hoehe, 48, "ohne Rahmen");
    }

    #[test]
    fn the_pool_delivers_withdraws_and_stops() {
        let (tx, rx) = mpsc::channel();
        let tx = Mutex::new(tx);
        let wurzel =
            std::env::temp_dir().join(format!("qd-miniaturen-pool-{}", std::process::id()));
        let pool = Pool::new(
            2,
            wurzel.clone(),
            Arc::new(move |e: Ergebnis| {
                let _ = tx.lock().unwrap().send((e.anfrage.uri, e.herkunft));
            }),
        );
        // no way for text: delivered as KeinWeg, nothing in the cache
        pool.einreihen(auftrag("file:///nirgends/a.txt", "text/plain"));
        let (uri, herkunft) = rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
        assert_eq!(uri, "file:///nirgends/a.txt");
        assert_eq!(herkunft, Herkunft::KeinWeg);
        assert!(!wurzel.join("fail").exists());
        assert_eq!(pool.sieben(|_| false), 0);
        assert_eq!(pool.offen(), (0, 0));
        drop(pool);
        let _ = std::fs::remove_dir_all(&wurzel);
    }

    #[test]
    fn pictures_are_made_cached_and_failures_noted() {
        let d = std::env::temp_dir().join(format!("qd-miniaturen-erzeugen-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let wurzel = d.join("thumbnails");
        let mut b = Bild::leer(300, 200);
        b.px.chunks_exact_mut(4).for_each(|p| p.fill(200));
        let bild = d.join("foto.png");
        std::fs::write(&bild, b.png()).unwrap();
        let uri = gtk4::gio::File::for_path(&bild).uri().to_string();
        let mut a = auftrag(&uri, "image/png");
        a.quelle.pfad = Some(bild.clone());
        a.quelle.mtime = 1234;
        a.anfrage.mtime = 1234;
        let nein = AtomicBool::new(false);
        let e = bearbeiten(&wurzel, &a, &nein);
        assert_eq!(e.herkunft, Herkunft::Erzeugt(Weg::Gtk));
        let t = e.textur.unwrap();
        assert_eq!(t.width(), 64);
        let png = std::fs::read(cache::pfad(&wurzel, Eimer::Normal, &uri)).unwrap();
        assert_eq!(cache::png_masse(&png), Some((128, 85)));
        assert_eq!(
            cache::thumb_tags(&png),
            (Some(uri.clone()), Some("1234".to_owned()))
        );
        assert_eq!(bearbeiten(&wurzel, &a, &nein).herkunft, Herkunft::Cache);
        // a changed file (other mtime) is made again
        a.quelle.mtime = 1235;
        a.anfrage.mtime = 1235;
        assert_eq!(
            bearbeiten(&wurzel, &a, &nein).herkunft,
            Herkunft::Erzeugt(Weg::Gtk)
        );
        // a valid `large` thumbnail (as Querencia Files writes them) serves
        // a normal request
        let anderes = d.join("anderes.png");
        std::fs::write(&anderes, b.png()).unwrap();
        let uri2 = gtk4::gio::File::for_path(&anderes).uri().to_string();
        let gross = cache::mit_tags(
            &b.verkleinert(256).png(),
            &cache::tags(&uri2, "7", "1", "image/png"),
        )
        .unwrap();
        cache::schreiben(&cache::pfad(&wurzel, Eimer::Gross, &uri2), &gross).unwrap();
        let mut a2 = auftrag(&uri2, "image/png");
        a2.quelle.mtime = 7;
        a2.anfrage.mtime = 7;
        assert_eq!(bearbeiten(&wurzel, &a2, &nein).herkunft, Herkunft::Cache);
        // a broken picture: failure noted, not tried again
        let kaputt = d.join("kaputt.png");
        std::fs::write(&kaputt, b"kein Bild").unwrap();
        let uri3 = gtk4::gio::File::for_path(&kaputt).uri().to_string();
        let mut a3 = auftrag(&uri3, "image/png");
        a3.quelle.pfad = Some(kaputt);
        let e = bearbeiten(&wurzel, &a3, &nein);
        assert_eq!(e.herkunft, Herkunft::Fehl);
        assert!(cache::fehlgeschlagen(&wurzel, &uri3, 1));
        assert_eq!(bearbeiten(&wurzel, &a3, &nein).herkunft, Herkunft::Fehl);
        let _ = std::fs::remove_dir_all(&d);
    }
}
