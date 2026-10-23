//! The trash, never the user's: the test runs itself again as a child with
//! its own HOME and XDG folders below cargo's target directory, without
//! GVfs (`GIO_USE_VFS=local`) and without a session bus. GLib reads HOME
//! and XDG_DATA_HOME once per process, so only a child can be sure of them.

mod gemeinsam;

use std::path::{Path, PathBuf};
use std::process::Command;

use gemeinsam::*;
use gio::prelude::*;
use querencia_dateiarbeit::auftraege::{Answer, Conflict, Op};
use querencia_dateiarbeit::papierkorb::{self, Papierkorb};

const KIND: &str = "QUERENCIA_DATEIARBEIT_PAPIERKORB";

#[test]
fn papierkorb() {
    if std::env::var_os(KIND).is_some() {
        return;
    }
    let base = scratch("papierkorb");
    let mut child = Command::new(std::env::current_exe().expect("Testprogramm"));
    child.args([
        "--exact",
        "papierkorb_kind",
        "--nocapture",
        "--test-threads=1",
    ]);
    isolated(&mut child, &base);
    child.env(KIND, &base);
    let output = child.output().expect("Kindprozess");
    let text = String::from_utf8_lossy(&output.stdout).into_owned()
        + &String::from_utf8_lossy(&output.stderr);
    eprintln!("{text}");
    assert!(output.status.success(), "Kindprozess: {}", output.status);
    assert!(text.contains("1 passed"), "Kindtest lief nicht");
    remove(&base);
}

fn trash_dir(base: &Path) -> PathBuf {
    base.join("home/.local/share/Trash")
}

#[test]
fn papierkorb_kind() {
    let Some(base) = std::env::var_os(KIND).map(PathBuf::from) else {
        return;
    };
    // the safety check first: never the real trash
    assert!(
        glib::user_data_dir().starts_with(&base),
        "XDG_DATA_HOME umgelenkt"
    );
    assert_eq!(papierkorb::home_trash(), trash_dir(&base));
    assert!(!papierkorb::gvfs_trash(), "ohne GVfs");
    let docs = base.join("home/Dokumente");
    write(&docs.join("a.txt"), "a");
    write(&docs.join("b.txt"), "b alt");
    write(&docs.join("Ordner/x.txt"), "x");
    write(&docs.join("c d.txt"), "c");
    let context = glib::MainContext::new();
    context
        .with_thread_default(|| {
            let trash = Papierkorb::new();
            let counts: std::rc::Rc<std::cell::RefCell<Vec<Option<u32>>>> = Default::default();
            let seen = counts.clone();
            trash.connect_changed(move |n| seen.borrow_mut().push(n));
            iterate_until(&context, || trash.anzahl().is_some());
            assert_eq!(trash.anzahl(), Some(0));

            let (outcome, _, _) = run(
                job(
                    Op::Trash,
                    &[
                        &docs.join("a.txt"),
                        &docs.join("b.txt"),
                        &docs.join("Ordner"),
                        &docs.join("c d.txt"),
                    ],
                    None,
                ),
                [],
            );
            assert_eq!((outcome.done, outcome.failed.len()), (4, 0));
            assert_eq!(papierkorb::count_now(), Some(4));
            assert!(names(&docs).is_empty());
            // the monitor counts without polling
            iterate_until(&context, || trash.anzahl() == Some(4));
            assert!(counts.borrow().contains(&Some(4)));
            let info = std::fs::read_to_string(trash_dir(&base).join("info/c d.txt.trashinfo"))
                .expect("trashinfo");
            assert!(info.contains("c%20d.txt"), "{info}");

            // restore to where it was
            let files = trash_dir(&base).join("files");
            let (outcome, _, _) = run(job(Op::Restore, &[&files.join("a.txt"), &files.join("c d.txt")], None), []);
            assert_eq!(outcome.failed, vec![]);
            assert_eq!(
                outcome.arrived,
                [docs.join("a.txt").display().to_string(), docs.join("c d.txt").display().to_string()]
            );
            assert_eq!(read(&docs.join("a.txt")), "a");
            assert!(!trash_dir(&base).join("info/a.txt.trashinfo").exists());

            // the place is taken: asked, restored under a free name
            write(&docs.join("b.txt"), "b neu");
            let (outcome, _, skript) = run(
                job(Op::Restore, &[&files.join("b.txt")], None),
                [once(Answer::Rename(String::new()))],
            );
            assert!(matches!(&skript.questions[..], [Conflict::Exists { vorschlag, .. }] if vorschlag == "b (copy).txt"));
            assert_eq!(outcome.done, 1);
            assert_eq!(read(&docs.join("b.txt")), "b neu");
            assert_eq!(read(&docs.join("b (copy).txt")), "b alt");

            // a single entry for good: its .trashinfo goes too
            let (outcome, _, _) = run(job(Op::Delete, &[&files.join("Ordner")], None), []);
            assert_eq!(outcome.done, 1);
            assert!(names(&trash_dir(&base).join("info")).is_empty());
            assert_eq!(papierkorb::count_now(), Some(0));

            // emptying
            write(&docs.join("e1.txt"), "1");
            write(&docs.join("e2/innen.txt"), "2");
            run(job(Op::Trash, &[&docs.join("e1.txt"), &docs.join("e2")], None), []);
            iterate_until(&context, || trash.anzahl() == Some(2));
            let (outcome, _, skript) = run(job(Op::EmptyTrash, &[], None), []);
            assert_eq!((outcome.done, outcome.failed.len()), (2, 0));
            assert_eq!(skript.started(), (Some(2), None));
            assert!(names(&files).is_empty() && names(&trash_dir(&base).join("info")).is_empty());
            iterate_until(&context, || trash.anzahl() == Some(0));
            assert!(trash.icon().to_string().is_some());
        })
        .expect("Hauptkontext");
}
