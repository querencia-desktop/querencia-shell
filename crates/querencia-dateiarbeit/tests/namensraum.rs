//! Tests that need mounts: a file-system boundary (tmpfs next to the ext4
//! scratch tree), a full target, a place without trash, and volume dummies
//! for GIO's unix volume monitor. The test runs itself again inside
//! `werkzeug/namensraum.sh` (own user and mount namespace, no sudo); the
//! mounts end with the child. Without user namespaces the test fails,
//! unless `QUERENCIA_DATEIARBEIT_OHNE_NAMENSRAUM=1` says to skip it.

mod gemeinsam;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use gemeinsam::*;
use gio::prelude::*;
use querencia_dateiarbeit::auftraege::{Answer, Conflict, Op, ausfuehren};
use querencia_dateiarbeit::laufwerke::{self, OhneRueckfrage, Quelle, Removal};

const KIND: &str = "QUERENCIA_DATEIARBEIT_NAMENSRAUM";

#[test]
fn namensraum() {
    if std::env::var_os(KIND).is_some() {
        return;
    }
    if std::env::var("QUERENCIA_DATEIARBEIT_OHNE_NAMENSRAUM").as_deref() == Ok("1") {
        eprintln!(
            "NICHT GEMESSEN: Namensraum-Tests übersprungen (QUERENCIA_DATEIARBEIT_OHNE_NAMENSRAUM=1)"
        );
        return;
    }
    let base = scratch("namensraum");
    let huelle = Path::new(env!("CARGO_MANIFEST_DIR")).join("werkzeug/namensraum.sh");
    let mut child = Command::new("/bin/sh");
    child
        .arg(&huelle)
        .arg(&base)
        .arg(std::env::current_exe().expect("Testprogramm"));
    child.args([
        "--exact",
        "namensraum_kind",
        "--nocapture",
        "--test-threads=1",
    ]);
    isolated(&mut child, &base);
    child.env(KIND, &base);
    let output = child.output().expect("Hülle");
    let text = String::from_utf8_lossy(&output.stdout).into_owned()
        + &String::from_utf8_lossy(&output.stderr);
    eprintln!("{text}");
    assert!(
        output.status.success(),
        "Namensraum-Lauf: {}",
        output.status
    );
    assert!(text.contains("1 passed"), "Kindtest lief nicht");
    remove(&base);
}

fn kind_base() -> Option<PathBuf> {
    std::env::var_os(KIND).map(PathBuf::from)
}

#[test]
fn namensraum_kind() {
    let Some(base) = kind_base() else { return };
    // inside: other file systems below base, our own trash
    assert!(glib::user_data_dir().starts_with(&base));
    let dev = |p: &Path| std::os::unix::fs::MetadataExt::dev(&std::fs::metadata(p).expect("da"));
    assert_ne!(dev(&base), dev(&base.join("grenze")), "tmpfs eingehängt");
    grenze(&base);
    voll(&base);
    ohne_papierkorb(&base);
    laufwerke_attrappen();
}

/// Moves across the file-system boundary: copy, then delete the original;
/// a cancel during the copy leaves the original whole and no half target.
fn grenze(base: &Path) {
    let quelle = base.join("quelle");
    let block = "q".repeat(256 * 1024);
    for n in 0..24 {
        write(&quelle.join(format!("Baum/teil-{n:02}.bin")), &block);
    }
    write(&quelle.join("Baum/innen/klein.txt"), "k");
    let ziel = base.join("grenze");
    // cancelled in the middle: nothing arrives, the source is whole
    let mut auftrag = job(Op::Move, &[&quelle.join("Baum")], Some(&ziel));
    auftrag.fortschritt = Duration::ZERO;
    let mut skript = Skript::new([]);
    let cancellable = gio::Cancellable::new();
    skript.cancel_after_bytes = Some((2 * 1024 * 1024, cancellable.clone()));
    let (outcome, cancelled) = ausfuehren(1, &auftrag, &cancellable, &mut skript);
    assert!(cancelled && outcome.done == 0);
    assert!(names(&ziel).is_empty(), "{:?}", names(&ziel));
    assert_eq!(names(&quelle.join("Baum")).len(), 25);
    let (items, bytes) = skript.started();
    assert_eq!((items, bytes), (Some(27), Some(24 * 256 * 1024 + 1)));
    eprintln!(
        "MESSWERT Grenze Abbruch: {} Fortschrittsereignisse bis zum Abbruch",
        skript.progress().len()
    );
    // the whole move
    let (outcome, cancelled, skript) = run(auftrag.clone(), []);
    assert!(!cancelled && outcome.failed.is_empty(), "{outcome:?}");
    assert!(!quelle.join("Baum").exists());
    assert_eq!(names(&ziel.join("Baum")).len(), 25);
    assert_eq!(read(&ziel.join("Baum/innen/klein.txt")), "k");
    assert_eq!(
        *skript.progress().last().expect("Fortschritt"),
        (27, 24 * 256 * 1024 + 1)
    );
    // replace across the boundary: built completely, swapped in
    write(&quelle.join("Baum/neu.txt"), "neu");
    let (outcome, _, _) = run(
        job(Op::Move, &[&quelle.join("Baum")], Some(&ziel)),
        [once(Answer::Replace)],
    );
    assert_eq!(outcome.done, 1);
    assert_eq!(names(&ziel.join("Baum")), ["neu.txt"]);
    assert!(!quelle.join("Baum").exists());
    // a big file across the boundary reports progress inside the file
    std::fs::write(base.join("gross.bin"), vec![1u8; 32 << 20]).expect("gross");
    let mut gross = job(Op::Copy, &[&base.join("gross.bin")], Some(&ziel));
    gross.fortschritt = Duration::ZERO;
    let (_, _, skript) = run(gross, []);
    eprintln!(
        "MESSWERT 32 MiB über die Grenze: {} Fortschrittsereignisse",
        skript.progress().len()
    );
    assert!(
        skript.progress().len() > 2,
        "Fortschritt innerhalb der Datei"
    );
    assert!(partials(base).is_empty());
}

/// A full target: the object fails, no partial copy stays.
fn voll(base: &Path) {
    std::fs::write(base.join("zwei-mb.bin"), vec![2u8; 2 << 20]).expect("Datei");
    write(&base.join("baum-voll/a.txt"), "a");
    std::fs::write(base.join("baum-voll/b.bin"), vec![3u8; 2 << 20]).expect("Datei");
    let (outcome, _, _) = run(
        job(
            Op::Copy,
            &[&base.join("zwei-mb.bin"), &base.join("baum-voll")],
            Some(&base.join("voll")),
        ),
        [],
    );
    assert_eq!(outcome.done, 0, "{outcome:?}");
    assert_eq!(outcome.failed.len(), 2);
    assert!(
        outcome
            .failed
            .iter()
            .all(|(_, why)| why == "There is no space left at the target.")
    );
    assert!(
        names(&base.join("voll")).is_empty(),
        "{:?}",
        names(&base.join("voll"))
    );
}

/// No trash on the place: the question, Skip keeps, Delete deletes.
fn ohne_papierkorb(base: &Path) {
    let place = base.join("ohne-papierkorb");
    write(&place.join("a.txt"), "a");
    write(&place.join("b.txt"), "b");
    let (outcome, _, skript) = run(
        job(Op::Trash, &[&place.join("a.txt")], None),
        [once(Answer::Skip)],
    );
    assert!(matches!(&skript.questions[..], [Conflict::NoTrash { file }] if file.name == "a.txt"));
    assert_eq!(outcome.skipped, 1);
    assert!(place.join("a.txt").exists());
    let (outcome, _, _) = run(
        job(
            Op::Trash,
            &[&place.join("a.txt"), &place.join("b.txt")],
            None,
        ),
        [all(Answer::Delete)],
    );
    assert_eq!(outcome.done, 2);
    assert_eq!(names(&place), [".Trash-0"]);
}

/// The unix volume monitor sees QDTEST mounted (rbind of /boot) and
/// QDFSTAB not mounted (a tmpfs line); QDFSTAB is mounted and unmounted
/// through GIO.
fn laufwerke_attrappen() {
    assert_eq!(
        std::env::var("GIO_USE_VOLUME_MONITOR").as_deref(),
        Ok("unix")
    );
    // GLib's unix volumes run mount(8) and read its output on the global
    // default main context (measured: on a context of its own the mount
    // never finishes), as programs with GTK have it anyway
    let context = glib::MainContext::default();
    let _owner = context.acquire().expect("Standardkontext");
    {
        {
            let monitor = gio::VolumeMonitor::get();
            let alle = laufwerke::alle(&monitor);
            let shown: Vec<String> = alle
                .iter()
                .map(|l| {
                    format!(
                        "{} {:?} eingehängt={} {:?}",
                        l.name, l.typ, l.mounted, l.removal
                    )
                })
                .collect();
            eprintln!("MESSWERT Laufwerke im Namensraum: {shown:?}");
            let test = alle.iter().find(|l| l.name == "QDTEST").expect("QDTEST");
            assert!(test.mounted);
            assert_eq!(test.uri.as_deref(), Some("file:///media/QDTEST"));
            assert_eq!(test.removal, Some(Removal::Unmount));
            let fstab = alle.iter().find(|l| l.name == "QDFSTAB").expect("QDFSTAB");
            assert!(!fstab.mounted && fstab.uri.is_none());
            let Quelle::Volume(volume) = &fstab.quelle else {
                panic!("QDFSTAB ist kein Volume")
            };
            // mount through GIO (mount(8) with LIBMOUNT_FSTAB, in the namespace)
            let volume = volume.clone();
            let result: std::rc::Rc<std::cell::RefCell<Option<Result<gio::Mount, glib::Error>>>> =
                Default::default();
            let slot = result.clone();
            context.spawn_local(async move {
                let mounted = laufwerke::einhaengen(&volume, &OhneRueckfrage).await;
                *slot.borrow_mut() = Some(mounted);
            });
            iterate_until(&context, || result.borrow().is_some());
            let mount = match result.borrow_mut().take().expect("Ergebnis") {
                Ok(mount) => mount,
                Err(err) => panic!("einhängen: {err}"),
            };
            assert_eq!(mount.root().uri(), "file:///media/QDFSTAB");
            let dev =
                |p: &str| std::os::unix::fs::MetadataExt::dev(&std::fs::metadata(p).expect("da"));
            assert_ne!(dev("/media/QDFSTAB"), dev("/media"), "eingehängt");
            let alle = laufwerke::alle(&monitor);
            assert!(alle.iter().any(|l| l.name == "QDFSTAB" && l.mounted));
            let done: std::rc::Rc<std::cell::RefCell<Option<Result<(), glib::Error>>>> =
                Default::default();
            let slot = done.clone();
            context.spawn_local(async move {
                let removed = laufwerke::entfernen(&mount, Removal::Unmount, &OhneRueckfrage).await;
                *slot.borrow_mut() = Some(removed);
            });
            iterate_until(&context, || done.borrow().is_some());
            if let Some(Err(err)) = done.borrow_mut().take() {
                panic!("aushängen: {err}");
            }
            iterate_until(&context, || {
                laufwerke::alle(&monitor)
                    .iter()
                    .any(|l| l.name == "QDFSTAB" && !l.mounted)
            });
            eprintln!("MESSWERT Einhängen/Aushängen QDFSTAB im Namensraum: bestanden");
        }
    }
}
