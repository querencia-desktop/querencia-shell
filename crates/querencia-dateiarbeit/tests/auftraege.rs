//! Table tests of the jobs on scratch trees (one local file system; the
//! file-system boundary is `tests/namensraum.rs`).

mod gemeinsam;

use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::Path;
use std::time::{Duration, Instant};

use gemeinsam::*;
use querencia_dateiarbeit::auftraege::{
    Answer, Conflict, Event, Job, Op, Outcome, Queue, ausfuehren,
};

#[test]
fn copy_takes_folders_with_their_content_and_links_as_links() {
    let base = scratch("kopie");
    let (from, to) = (base.join("quelle"), base.join("ziel"));
    write(&from.join("Ordner/innen/a.txt"), "a");
    write(&from.join("b.txt"), "b");
    symlink(base.join("draussen"), from.join("Ordner/verweis")).expect("toter Verweis");
    symlink("schleife", from.join("Ordner/schleife")).expect("Schleife");
    write(&to.join("b.txt"), "alt");
    // b.txt exists: the question, answered with a free name (the old rule
    // of the desktop as an answer)
    let (outcome, cancelled, skript) = run(
        job(
            Op::Copy,
            &[&from.join("Ordner"), &from.join("b.txt")],
            Some(&to),
        ),
        [once(Answer::Rename(String::new()))],
    );
    assert!(!cancelled);
    assert_eq!(outcome.failed, vec![]);
    assert_eq!(outcome.arrived, ["Ordner", "b (copy).txt"]);
    assert!(matches!(
        &skript.questions[..],
        [Conflict::Exists { vorschlag, .. }] if vorschlag == "b (copy).txt"
    ));
    assert_eq!(read(&to.join("Ordner/innen/a.txt")), "a");
    assert_eq!(read(&to.join("b.txt")), "alt");
    assert_eq!(read(&to.join("b (copy).txt")), "b");
    for link in ["Ordner/verweis", "Ordner/schleife"] {
        let meta = to.join(link).symlink_metadata().expect("Verweis");
        assert!(meta.file_type().is_symlink(), "{link}");
    }
    assert_eq!(
        std::fs::read_link(to.join("Ordner/verweis")).expect("Ziel"),
        base.join("draussen")
    );
    assert!(from.join("Ordner/innen/a.txt").exists() && from.join("b.txt").exists());
    assert!(partials(&base).is_empty());
    // totals: Ordner, innen, a.txt, verweis, schleife, b.txt
    assert_eq!(skript.started(), (Some(6), Some(2 + read_link_len(&from))));
    remove(&base);
}

/// Bytes GIO counts for the two links (their target's length).
fn read_link_len(from: &Path) -> u64 {
    ["Ordner/verweis", "Ordner/schleife"]
        .iter()
        .map(|l| from.join(l).symlink_metadata().map_or(0, |m| m.len()))
        .sum()
}

#[test]
fn a_copy_into_its_own_folder_is_a_duplicate_and_never_into_itself() {
    let base = scratch("selbst");
    std::fs::create_dir_all(base.join("Ordner/unter")).expect("fixture");
    write(&base.join("notiz.txt"), "n");
    let (outcome, _, _) = run(job(Op::Copy, &[&base.join("notiz.txt")], Some(&base)), []);
    assert_eq!(outcome.arrived, ["notiz (copy).txt"]);
    for (op, target) in [
        (Op::Copy, base.join("Ordner")),
        (Op::Copy, base.join("Ordner/unter")),
        (Op::Move, base.join("Ordner/unter")),
    ] {
        let (outcome, _, _) = run(job(op.clone(), &[&base.join("Ordner")], Some(&target)), []);
        assert_eq!(outcome.done, 0);
        assert!(
            outcome.failed[0].1.contains("into itself"),
            "{op:?} {outcome:?}"
        );
    }
    assert!(base.join("Ordner/unter").is_dir());
    remove(&base);
}

/// One row of the conflict table.
struct Fall {
    name: &'static str,
    op: Op,
    /// Sets up `quelle/` and `ziel/` below the base.
    setup: fn(&Path),
    sources: &'static [&'static str],
    answers: Vec<querencia_dateiarbeit::auftraege::Answered>,
    check: fn(&Path, &Outcome, bool),
}

fn file_onto_file(base: &Path) {
    write(&base.join("quelle/a.txt"), "neu");
    write(&base.join("ziel/a.txt"), "alt");
}

fn dir_onto_dir(base: &Path) {
    write(&base.join("quelle/Ordner/x.txt"), "x neu");
    write(&base.join("quelle/Ordner/nur-neu.txt"), "n");
    write(&base.join("ziel/Ordner/x.txt"), "x alt");
    write(&base.join("ziel/Ordner/nur-alt.txt"), "o");
}

fn three_files(base: &Path) {
    for n in ["a", "b", "c"] {
        write(&base.join(format!("quelle/{n}.txt")), &format!("{n} neu"));
        write(&base.join(format!("ziel/{n}.txt")), &format!("{n} alt"));
    }
}

#[test]
fn conflicts_are_answered_as_the_table_says() {
    let faelle = vec![
        Fall {
            name: "ersetzen Datei",
            op: Op::Copy,
            setup: file_onto_file,
            sources: &["a.txt"],
            answers: vec![once(Answer::Replace)],
            check: |b, o, c| {
                assert!(!c && o.done == 1);
                assert_eq!(read(&b.join("ziel/a.txt")), "neu");
                assert_eq!(read(&b.join("quelle/a.txt")), "neu");
            },
        },
        Fall {
            name: "überspringen",
            op: Op::Copy,
            setup: file_onto_file,
            sources: &["a.txt"],
            answers: vec![once(Answer::Skip)],
            check: |b, o, c| {
                assert!(!c && o.done == 0 && o.skipped == 1);
                assert_eq!(read(&b.join("ziel/a.txt")), "alt");
            },
        },
        Fall {
            name: "umbenennen mit Namen",
            op: Op::Copy,
            setup: file_onto_file,
            sources: &["a.txt"],
            answers: vec![once(Answer::Rename("b.txt".into()))],
            check: |b, o, _| {
                assert_eq!(o.arrived, ["b.txt"]);
                assert_eq!(read(&b.join("ziel/a.txt")), "alt");
                assert_eq!(read(&b.join("ziel/b.txt")), "neu");
            },
        },
        Fall {
            name: "umbenennen ungültig",
            op: Op::Copy,
            setup: file_onto_file,
            sources: &["a.txt"],
            answers: vec![once(Answer::Rename("x/y".into()))],
            check: |b, o, _| {
                assert_eq!(o.failed.len(), 1);
                assert!(o.failed[0].1.contains("“/”"), "{o:?}");
                assert_eq!(names(&b.join("ziel")), ["a.txt"]);
            },
        },
        Fall {
            name: "abbrechen",
            op: Op::Copy,
            setup: three_files,
            sources: &["a.txt", "b.txt", "c.txt"],
            answers: vec![once(Answer::Cancel)],
            check: |b, o, c| {
                assert!(c && o.done == 0 && o.failed.is_empty());
                assert_eq!(read(&b.join("ziel/a.txt")), "a alt");
            },
        },
        Fall {
            name: "für alle ersetzen",
            op: Op::Copy,
            setup: three_files,
            sources: &["a.txt", "b.txt", "c.txt"],
            answers: vec![all(Answer::Replace)],
            check: |b, o, _| {
                assert_eq!(o.done, 3);
                for n in ["a", "b", "c"] {
                    assert_eq!(read(&b.join(format!("ziel/{n}.txt"))), format!("{n} neu"));
                }
            },
        },
        Fall {
            name: "für alle überspringen",
            op: Op::Move,
            setup: three_files,
            sources: &["a.txt", "b.txt", "c.txt"],
            answers: vec![all(Answer::Skip)],
            check: |b, o, _| {
                assert_eq!((o.done, o.skipped), (0, 3));
                assert_eq!(names(&b.join("quelle")), ["a.txt", "b.txt", "c.txt"]);
                assert_eq!(read(&b.join("ziel/b.txt")), "b alt");
            },
        },
        Fall {
            name: "für alle umbenennen",
            op: Op::Copy,
            setup: three_files,
            sources: &["a.txt", "b.txt", "c.txt"],
            answers: vec![all(Answer::Rename("a-neu.txt".into()))],
            check: |b, o, _| {
                // the first gets the given name, the others a free name each
                assert_eq!(o.arrived, ["a-neu.txt", "b (copy).txt", "c (copy).txt"]);
                assert_eq!(read(&b.join("ziel/c (copy).txt")), "c neu");
                assert_eq!(read(&b.join("ziel/c.txt")), "c alt");
            },
        },
        Fall {
            name: "Ordner zusammenführen, darin ersetzen",
            op: Op::Copy,
            setup: dir_onto_dir,
            sources: &["Ordner"],
            answers: vec![once(Answer::Merge), once(Answer::Replace)],
            check: |b, o, _| {
                assert_eq!(o.arrived, ["Ordner"]);
                assert_eq!(
                    names(&b.join("ziel/Ordner")),
                    ["nur-alt.txt", "nur-neu.txt", "x.txt"]
                );
                assert_eq!(read(&b.join("ziel/Ordner/x.txt")), "x neu");
            },
        },
        Fall {
            name: "Ordner ersetzen",
            op: Op::Copy,
            setup: dir_onto_dir,
            sources: &["Ordner"],
            answers: vec![once(Answer::Replace)],
            check: |b, _, _| {
                assert_eq!(names(&b.join("ziel/Ordner")), ["nur-neu.txt", "x.txt"]);
                assert_eq!(read(&b.join("ziel/Ordner/x.txt")), "x neu");
                assert_eq!(names(&b.join("ziel")), ["Ordner"]);
            },
        },
        Fall {
            name: "Datei ersetzt Ordner",
            op: Op::Copy,
            setup: |b| {
                write(&b.join("quelle/x"), "Datei");
                write(&b.join("ziel/x/innen.txt"), "i");
            },
            sources: &["x"],
            answers: vec![once(Answer::Replace)],
            check: |b, _, _| assert_eq!(read(&b.join("ziel/x")), "Datei"),
        },
        Fall {
            name: "verschieben ersetzen",
            op: Op::Move,
            setup: file_onto_file,
            sources: &["a.txt"],
            answers: vec![once(Answer::Replace)],
            check: |b, o, _| {
                assert_eq!(o.done, 1);
                assert_eq!(read(&b.join("ziel/a.txt")), "neu");
                assert!(names(&b.join("quelle")).is_empty());
            },
        },
        Fall {
            name: "verschieben Ordner ersetzen",
            op: Op::Move,
            setup: dir_onto_dir,
            sources: &["Ordner"],
            answers: vec![once(Answer::Replace)],
            check: |b, _, _| {
                assert_eq!(names(&b.join("ziel/Ordner")), ["nur-neu.txt", "x.txt"]);
                assert!(names(&b.join("quelle")).is_empty());
            },
        },
        Fall {
            name: "verschieben zusammenführen",
            op: Op::Move,
            setup: dir_onto_dir,
            sources: &["Ordner"],
            answers: vec![once(Answer::Merge), once(Answer::Skip)],
            check: |b, _, _| {
                assert_eq!(
                    names(&b.join("ziel/Ordner")),
                    ["nur-alt.txt", "nur-neu.txt", "x.txt"]
                );
                assert_eq!(read(&b.join("ziel/Ordner/x.txt")), "x alt");
                // the skipped file stays in the source folder, which stays
                assert_eq!(names(&b.join("quelle/Ordner")), ["x.txt"]);
            },
        },
    ];
    for fall in faelle {
        let base = scratch("konflikt");
        (fall.setup)(&base);
        let sources: Vec<std::path::PathBuf> = fall
            .sources
            .iter()
            .map(|s| base.join("quelle").join(s))
            .collect();
        let refs: Vec<&Path> = sources.iter().map(|p| p.as_path()).collect();
        let (outcome, cancelled, skript) = run(
            job(fall.op.clone(), &refs, Some(&base.join("ziel"))),
            fall.answers,
        );
        eprintln!("Fall „{}“: {outcome:?}", fall.name);
        (fall.check)(&base, &outcome, cancelled);
        assert!(partials(&base).is_empty(), "{}: Teilobjekte", fall.name);
        assert!(
            skript
                .questions
                .iter()
                .all(|q| !matches!(q, Conflict::Fehler { .. })),
            "{}",
            fall.name
        );
        remove(&base);
    }
}

#[test]
fn a_move_never_overwrites_silently_and_into_the_same_folder_changes_nothing() {
    let base = scratch("verschieben");
    let (from, to) = (base.join("quelle"), base.join("ziel"));
    write(&from.join("Ordner/x.txt"), "x");
    write(&from.join("a.txt"), "neu");
    write(&from.join("c.txt"), "c");
    write(&to.join("a.txt"), "alt");
    let (outcome, _, skript) = run(
        job(
            Op::Move,
            &[
                &from.join("Ordner"),
                &from.join("a.txt"),
                &from.join("c.txt"),
                &from.join("fehlt.txt"),
            ],
            Some(&to),
        ),
        [once(Answer::Skip)],
    );
    assert_eq!(skript.questions.len(), 1);
    assert_eq!((outcome.done, outcome.skipped), (2, 1));
    assert_eq!(outcome.arrived, ["Ordner", "c.txt"]);
    assert_eq!(
        outcome.failed,
        [("fehlt.txt".to_owned(), "no longer there".to_owned())]
    );
    assert_eq!(read(&to.join("a.txt")), "alt");
    assert_eq!(read(&from.join("a.txt")), "neu");
    assert!(to.join("Ordner/x.txt").exists() && !from.join("Ordner").exists());
    // same file system: counted as one object each, no bytes
    assert_eq!(skript.started(), (Some(4), None));
    let (outcome, _, _) = run(job(Op::Move, &[&to.join("c.txt")], Some(&to)), []);
    assert_eq!(
        (outcome.done, outcome.arrived),
        (1, vec!["c.txt".to_owned()])
    );
    assert!(to.join("c.txt").exists());
    remove(&base);
}

#[test]
fn a_cancel_in_the_middle_of_a_tree_leaves_no_half_tree() {
    let base = scratch("abbruch");
    let block = "x".repeat(64 * 1024);
    for d in 0..10 {
        for f in 0..20 {
            write(&base.join(format!("quelle/Baum/d{d}/f{f}.bin")), &block);
        }
    }
    // one big file, cancelled inside the file
    let big = vec![7u8; 48 * 1024 * 1024];
    std::fs::write(base.join("quelle/gross.bin"), &big).expect("gross");
    std::fs::create_dir_all(base.join("ziel")).expect("ziel");
    for (source, limit) in [("Baum", 3 * 1024 * 1024), ("gross.bin", 20 * 1024 * 1024)] {
        let mut skript = Skript::new([]);
        let cancellable = gio::Cancellable::new();
        skript.cancel_after_bytes = Some((limit, cancellable.clone()));
        let mut job = job(
            Op::Copy,
            &[&base.join("quelle").join(source)],
            Some(&base.join("ziel")),
        );
        job.fortschritt = Duration::ZERO;
        let (outcome, cancelled) = ausfuehren(7, &job, &cancellable, &mut skript);
        assert!(cancelled, "{source}");
        assert_eq!(outcome.done, 0, "{source}");
        assert!(
            names(&base.join("ziel")).is_empty(),
            "{source}: {:?}",
            names(&base.join("ziel"))
        );
        assert!(partials(&base).is_empty(), "{source}");
        let reached = skript.progress().iter().map(|p| p.1).max().unwrap_or(0);
        assert!(reached >= limit, "{source}: {reached}");
        eprintln!(
            "MESSWERT Abbruch {source}: abgebrochen bei {reached} Bytes, {} Fortschrittsereignisse",
            skript.progress().len()
        );
    }
    // the source is untouched
    assert_eq!(names(&base.join("quelle/Baum")).len(), 10);
    assert_eq!(
        std::fs::metadata(base.join("quelle/gross.bin"))
            .expect("da")
            .len(),
        48 << 20
    );
    // a move cancelled during its question leaves both sides as they were
    write(&base.join("ziel/gross.bin"), "alt");
    let (outcome, cancelled, _) = run(
        job(
            Op::Move,
            &[&base.join("quelle/gross.bin")],
            Some(&base.join("ziel")),
        ),
        [once(Answer::Cancel)],
    );
    assert!(cancelled && outcome.done == 0);
    assert_eq!(read(&base.join("ziel/gross.bin")), "alt");
    remove(&base);
}

#[test]
fn ten_thousand_files_with_progress_events() {
    let base = scratch("gross");
    let tree = base.join("quelle/Zehntausend");
    let mut bytes = 0u64;
    for d in 0..100 {
        let dir = tree.join(format!("ordner-{d:03}"));
        std::fs::create_dir_all(&dir).expect("ordner");
        for f in 0..100 {
            let text = format!("Datei {d}/{f}\n");
            bytes += text.len() as u64;
            std::fs::write(dir.join(format!("datei-{f:03}.txt")), text).expect("datei");
        }
    }
    std::fs::create_dir_all(base.join("ziel")).expect("ziel");
    let begun = Instant::now();
    let (outcome, cancelled, skript) = run(job(Op::Copy, &[&tree], Some(&base.join("ziel"))), []);
    let took = begun.elapsed();
    assert!(!cancelled && outcome.failed.is_empty());
    let items = 10_000 + 100 + 1;
    assert_eq!(skript.started(), (Some(items), Some(bytes)));
    let progress = skript.progress();
    assert!(
        progress.windows(2).all(|w| w[0].0 <= w[1].0),
        "Objekte steigen"
    );
    assert_eq!(*progress.last().expect("Fortschritt"), (items, bytes));
    // at most one event per 100 ms plus the forced ones (one per source)
    let limit = (took.as_millis() / 100) as usize + 3;
    assert!(
        progress.len() <= limit,
        "{} Ereignisse in {took:?}",
        progress.len()
    );
    let copied = walk_count(&base.join("ziel/Zehntausend"));
    assert_eq!(copied, items - 1);
    eprintln!(
        "MESSWERT 10.000 Dateien kopieren: {took:?}, {} Fortschrittsereignisse",
        progress.len()
    );
    let begun = Instant::now();
    let (outcome, _, skript) = run(job(Op::Delete, &[&base.join("ziel/Zehntausend")], None), []);
    assert_eq!(outcome.done, 1);
    assert_eq!(skript.progress().last().map(|p| p.0), Some(items));
    assert!(!base.join("ziel/Zehntausend").exists());
    eprintln!("MESSWERT 10.000 Dateien löschen: {:?}", begun.elapsed());
    remove(&base);
}

fn walk_count(dir: &Path) -> u64 {
    let mut n = 0;
    for entry in std::fs::read_dir(dir).expect("lesbar").flatten() {
        n += 1;
        if entry.file_type().expect("Typ").is_dir() {
            n += walk_count(&entry.path());
        }
    }
    n
}

#[test]
fn an_unreadable_file_is_reported_skipped_or_cancels() {
    let base = scratch("rechte");
    write(&base.join("quelle/Ordner/lesbar.txt"), "l");
    write(&base.join("quelle/Ordner/geheim.txt"), "g");
    std::fs::set_permissions(
        base.join("quelle/Ordner/geheim.txt"),
        std::fs::Permissions::from_mode(0o000),
    )
    .expect("chmod");
    std::fs::create_dir_all(base.join("ziel")).expect("ziel");
    let source = base.join("quelle/Ordner");
    // reported, the rest arrives
    let (outcome, _, _) = run(job(Op::Copy, &[&source], Some(&base.join("ziel"))), []);
    assert_eq!(outcome.arrived, ["Ordner"]);
    assert_eq!(
        outcome.failed,
        [(
            "Ordner/geheim.txt".to_owned(),
            "permission denied".to_owned()
        )]
    );
    assert_eq!(names(&base.join("ziel/Ordner")), ["lesbar.txt"]);
    // with questions: a skip lets the rest of the folder arrive, a cancel
    // takes the partial folder away again
    for (answer, arrived) in [(Answer::Skip, true), (Answer::Cancel, false)] {
        std::fs::remove_dir_all(base.join("ziel/Ordner")).ok();
        let mut job = job(Op::Copy, &[&source], Some(&base.join("ziel")));
        job.fehler_fragen = true;
        let (outcome, cancelled, skript) = run(job, [once(answer.clone())]);
        assert!(
            matches!(&skript.questions[..], [Conflict::Fehler { why, .. }] if why == "permission denied")
        );
        assert_eq!(outcome.failed, vec![]);
        assert_eq!(base.join("ziel/Ordner").exists(), arrived, "{answer:?}");
        assert_eq!(cancelled, !arrived);
        assert!(partials(&base).is_empty());
    }
    remove(&base);
}

#[test]
fn delete_removes_folders_but_never_what_a_link_points_to() {
    let base = scratch("loeschen");
    write(&base.join("bleibt/wichtig.txt"), "w");
    write(&base.join("weg/innen/a.txt"), "a");
    symlink(base.join("bleibt"), base.join("weg/verweis")).expect("Verweis");
    symlink(base.join("bleibt/wichtig.txt"), base.join("verweis-allein")).expect("Verweis");
    let (outcome, _, _) = run(
        job(
            Op::Delete,
            &[&base.join("weg"), &base.join("verweis-allein")],
            None,
        ),
        [],
    );
    assert_eq!((outcome.done, outcome.failed), (2, vec![]));
    assert!(!base.join("weg").exists() && !base.join("verweis-allein").exists());
    assert_eq!(read(&base.join("bleibt/wichtig.txt")), "w");
    remove(&base);
}

#[test]
fn links_new_objects_and_renames() {
    let base = scratch("neu");
    write(&base.join("a.txt"), "a");
    write(&base.join("b.txt"), "b");
    write(&base.join("Vorlagen/Brief.txt"), "Sehr geehrte");
    let dir = base.join("ziel");
    std::fs::create_dir_all(&dir).expect("ziel");
    let (outcome, _, _) = run(
        job(
            Op::Link,
            &[&base.join("a.txt"), &base.join("a.txt")],
            Some(&dir),
        ),
        [],
    );
    assert_eq!(outcome.arrived, ["Link to a.txt", "Link to a 2.txt"]);
    assert_eq!(
        std::fs::read_link(dir.join("Link to a.txt")).expect("Verweis"),
        base.join("a.txt")
    );
    for expected in ["Neuer Ordner", "Neuer Ordner 2"] {
        let (outcome, _, _) = run(
            job(
                Op::NewFolder {
                    name: "Neuer Ordner".into(),
                },
                &[],
                Some(&dir),
            ),
            [],
        );
        assert_eq!(outcome.arrived, [expected]);
    }
    let (outcome, _, _) = run(
        job(
            Op::NewFile {
                name: "Brief.txt".into(),
                template: Some(uri(&base.join("Vorlagen/Brief.txt"))),
            },
            &[],
            Some(&dir),
        ),
        [],
    );
    assert_eq!(outcome.arrived, ["Brief.txt"]);
    assert_eq!(read(&dir.join("Brief.txt")), "Sehr geehrte");
    let (outcome, _, _) = run(
        job(
            Op::NewFile {
                name: "Brief.txt".into(),
                template: None,
            },
            &[],
            Some(&dir),
        ),
        [],
    );
    assert_eq!(outcome.arrived, ["Brief 2.txt"]);
    assert_eq!(read(&dir.join("Brief 2.txt")), "");
    // rename: never onto an existing name, never an invalid one
    let (outcome, _, _) = run(
        job(
            Op::Rename {
                new: "b.txt".into(),
            },
            &[&base.join("a.txt")],
            None,
        ),
        [],
    );
    assert_eq!(
        outcome.failed[0].1,
        "There is already an item with this name at the target."
    );
    assert_eq!(
        (read(&base.join("a.txt")), read(&base.join("b.txt"))),
        ("a".into(), "b".into())
    );
    let (outcome, _, _) = run(
        job(
            Op::Rename { new: "..".into() },
            &[&base.join("a.txt")],
            None,
        ),
        [],
    );
    assert_eq!(outcome.failed.len(), 1);
    let (outcome, _, _) = run(
        job(
            Op::Rename {
                new: "c.txt".into(),
            },
            &[&base.join("a.txt")],
            None,
        ),
        [],
    );
    assert_eq!(outcome.arrived, ["c.txt"]);
    assert_eq!(read(&base.join("c.txt")), "a");
    assert!(partials(&base).is_empty());
    remove(&base);
}

#[test]
fn the_queue_delivers_events_and_takes_answers_on_the_main_context() {
    let base = scratch("warteschlange");
    write(&base.join("quelle/a.txt"), "neu");
    write(&base.join("quelle/b.txt"), "b");
    write(&base.join("ziel/a.txt"), "alt");
    let context = glib::MainContext::new();
    context
        .with_thread_default(|| {
            let queue = Queue::new();
            let seen: std::rc::Rc<std::cell::RefCell<Vec<Event>>> = Default::default();
            let log = seen.clone();
            let main_thread = std::thread::current().id();
            queue.subscribe(move |event| {
                assert_eq!(std::thread::current().id(), main_thread, "Hauptkontext");
                if let Event::Ask { reply, .. } = event {
                    reply.send(once(Answer::Replace));
                }
                log.borrow_mut().push(event.clone());
            });
            let id = queue.start(job(
                Op::Copy,
                &[&base.join("quelle/a.txt"), &base.join("quelle/b.txt")],
                Some(&base.join("ziel")),
            ));
            assert_eq!(queue.running(), [(id, Op::Copy)]);
            iterate_until(&context, || {
                seen.borrow()
                    .iter()
                    .any(|e| matches!(e, Event::Finished { .. }))
            });
            assert!(queue.running().is_empty());
            let events = seen.borrow();
            assert!(matches!(events.first(), Some(Event::Started { id: i, .. }) if *i == id));
            assert_eq!(
                events
                    .iter()
                    .filter(|e| matches!(e, Event::Ask { .. }))
                    .count(),
                1
            );
            assert!(matches!(
                events.last(),
                Some(Event::Finished { outcome, cancelled: false, .. }) if outcome.done == 2
            ));
            assert_eq!(read(&base.join("ziel/a.txt")), "neu");

            // a cancel while the job waits for an answer ends it at once
            write(&base.join("quelle/a.txt"), "noch neuer");
            let waiting = Queue::new();
            let ended: std::rc::Rc<std::cell::Cell<Option<bool>>> = Default::default();
            let (flag, q) = (ended.clone(), waiting.clone());
            waiting.subscribe(move |event| match event {
                Event::Ask { id, .. } => q.cancel(*id),
                Event::Finished { cancelled, .. } => flag.set(Some(*cancelled)),
                _ => {}
            });
            let begun = Instant::now();
            waiting.start(job(
                Op::Copy,
                &[&base.join("quelle/a.txt")],
                Some(&base.join("ziel")),
            ));
            iterate_until(&context, || ended.get().is_some());
            assert_eq!(ended.get(), Some(true));
            assert!(begun.elapsed() < Duration::from_secs(5));
            assert_eq!(read(&base.join("ziel/a.txt")), "neu");
        })
        .expect("eigener Hauptkontext");
    assert!(partials(&base).is_empty());
    remove(&base);
}

#[test]
fn jobs_without_target_or_source_fail_cleanly() {
    let (outcome, _, _) = run(
        Job::new(Op::Copy, vec!["file:///nicht/da".into()], None),
        [],
    );
    assert_eq!(outcome.failed[0].1, "no target");
    let (outcome, _, _) = run(Job::new(Op::Rename { new: "x".into() }, vec![], None), []);
    assert_eq!(outcome, Outcome::default());
}

#[test]
fn a_folder_is_never_replaced_by_something_inside_it() {
    let base = scratch("vorfahr");
    write(&base.join("P/Q/Q/innen.txt"), "i");
    write(&base.join("P/Q/bleibt.txt"), "b");
    for op in [Op::Copy, Op::Move] {
        let (outcome, _, skript) = run(
            job(op.clone(), &[&base.join("P/Q/Q")], Some(&base.join("P"))),
            [once(Answer::Replace)],
        );
        assert!(matches!(
            &skript.questions[..],
            [Conflict::FolderMerge { .. }]
        ));
        assert_eq!(
            outcome.failed[0].1, "A folder cannot be replaced by an item it contains.",
            "{op:?}"
        );
        assert_eq!(read(&base.join("P/Q/Q/innen.txt")), "i");
        assert_eq!(read(&base.join("P/Q/bleibt.txt")), "b");
        assert!(partials(&base).is_empty());
    }
    remove(&base);
}

#[test]
fn undo_lists_only_objects_that_can_simply_go_back() {
    let base = scratch("undo");
    let (from, to) = (base.join("quelle"), base.join("ziel"));
    for n in ["a.txt", "b.txt", "c.txt", "d.txt"] {
        write(&from.join(n), n);
    }
    std::fs::create_dir_all(from.join("Ordner")).expect("fixture");
    write(&from.join("Ordner/innen.txt"), "i");
    write(&to.join("b.txt"), "alt");
    write(&to.join("c.txt"), "alt");
    write(&to.join("d.txt"), "alt");
    std::fs::create_dir_all(to.join("Ordner")).expect("fixture");
    // a moves cleanly; b replaces, c keeps both under a new name, d is
    // skipped, the folder is merged: only a can go back
    let (outcome, cancelled, _) = run(
        job(
            Op::Move,
            &[
                &from.join("a.txt"),
                &from.join("b.txt"),
                &from.join("c.txt"),
                &from.join("d.txt"),
                &from.join("Ordner"),
            ],
            Some(&to),
        ),
        [
            once(Answer::Replace),
            once(Answer::Rename(String::new())),
            once(Answer::Skip),
            once(Answer::Merge),
        ],
    );
    assert!(!cancelled);
    assert_eq!(outcome.done, 4);
    assert_eq!(
        outcome.undo,
        vec![(uri(&from.join("a.txt")), "a.txt".to_owned())]
    );
    // a copy with a new name can go back (the copy is deleted); a move
    // into its own folder changes nothing and is not listed
    write(&from.join("e.txt"), "e");
    write(&to.join("e.txt"), "alt");
    let (outcome, _, _) = run(
        job(Op::Copy, &[&from.join("e.txt")], Some(&to)),
        [once(Answer::Rename(String::new()))],
    );
    assert_eq!(
        outcome.undo,
        vec![(uri(&from.join("e.txt")), "e (copy).txt".to_owned())]
    );
    let (outcome, _, _) = run(job(Op::Move, &[&to.join("e.txt")], Some(&to)), []);
    assert_eq!(outcome.done, 1);
    assert!(outcome.undo.is_empty());
    // rename: the new name
    let (outcome, _, _) = run(
        job(
            Op::Rename {
                new: "f.txt".to_owned(),
            },
            &[&to.join("e.txt")],
            None,
        ),
        [],
    );
    assert_eq!(
        outcome.undo,
        vec![(uri(&to.join("e.txt")), "f.txt".to_owned())]
    );
    remove(&base);
}
