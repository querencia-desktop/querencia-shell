//! Folder monitors on scratch folders, on a main context of the test.

mod gemeinsam;

use std::cell::RefCell;
use std::rc::Rc;

use gemeinsam::*;
use gio::prelude::*;
use querencia_dateiarbeit::ueberwachung::{Aenderung, FolderWatch};

fn label(change: &Aenderung) -> String {
    let name = |f: &gio::File| {
        f.basename()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    match change {
        Aenderung::Neu(f) => format!("neu {}", name(f)),
        Aenderung::Weg(f) => format!("weg {}", name(f)),
        Aenderung::Geaendert(f) => format!("geändert {}", name(f)),
        Aenderung::Umbenannt(a, b) => format!("umbenannt {} {}", name(a), name(b)),
        Aenderung::OrdnerDa => "Ordner da".into(),
        Aenderung::OrdnerWeg => "Ordner weg".into(),
        Aenderung::NeuLaden => "neu laden".into(),
    }
}

type Seen = Rc<RefCell<Vec<Vec<String>>>>;

fn watch(dir: &std::path::Path, warten: bool) -> (FolderWatch, Seen) {
    let watch = FolderWatch::new(&gio::File::for_path(dir), warten);
    let seen: Seen = Rc::default();
    let log = seen.clone();
    watch.connect(move |changes| log.borrow_mut().push(changes.iter().map(label).collect()));
    (watch, seen)
}

fn flat(seen: &Seen) -> Vec<String> {
    seen.borrow().iter().flatten().cloned().collect()
}

#[test]
fn changes_come_bundled_with_renames_as_one() {
    let base = scratch("ueberwachung");
    let context = glib::MainContext::new();
    context
        .with_thread_default(|| {
            let (watch, seen) = watch(&base, false);
            assert!(watch.watches_folder());
            write(&base.join("a.txt"), "a");
            iterate_until(&context, || flat(&seen).contains(&"neu a.txt".to_owned()));
            std::fs::rename(base.join("a.txt"), base.join("b.txt")).expect("umbenennen");
            iterate_until(&context, || {
                flat(&seen).contains(&"umbenannt a.txt b.txt".to_owned())
            });
            std::fs::remove_file(base.join("b.txt")).expect("löschen");
            iterate_until(&context, || flat(&seen).contains(&"weg b.txt".to_owned()));
            // a burst of 50 files arrives in few bundles
            let before = seen.borrow().len();
            for n in 0..50 {
                write(&base.join(format!("viele-{n:02}.txt")), "v");
            }
            iterate_until(&context, || {
                flat(&seen)
                    .iter()
                    .filter(|l| l.starts_with("neu viele-"))
                    .count()
                    == 50
            });
            let bundles = seen.borrow().len() - before;
            eprintln!("MESSWERT 50 neue Dateien: {bundles} Bündel");
            assert!(bundles <= 10, "{bundles} Bündel");
        })
        .expect("Hauptkontext");
    remove(&base);
}

#[test]
fn a_missing_folder_is_awaited_and_followed() {
    let base = scratch("warten");
    let dir = base.join("x/Schreibtisch");
    let context = glib::MainContext::new();
    context
        .with_thread_default(|| {
            let (watch, seen) = watch(&dir, true);
            assert!(!watch.watches_folder(), "wartet im Elternordner");
            std::fs::create_dir_all(&dir).expect("anlegen");
            iterate_until(&context, || flat(&seen).contains(&"Ordner da".to_owned()));
            assert!(watch.watches_folder());
            write(&dir.join("notiz.txt"), "n");
            iterate_until(&context, || {
                flat(&seen).contains(&"neu notiz.txt".to_owned())
            });
            std::fs::remove_dir_all(&dir).expect("entfernen");
            iterate_until(&context, || flat(&seen).contains(&"Ordner weg".to_owned()));
            assert!(!watch.watches_folder());
            std::fs::create_dir_all(&dir).expect("wieder anlegen");
            iterate_until(&context, || {
                flat(&seen).iter().filter(|l| *l == "Ordner da").count() == 2
            });
            assert!(watch.watches_folder());
        })
        .expect("Hauptkontext");
    remove(&base);
}
