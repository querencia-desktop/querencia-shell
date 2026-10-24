//! Foreign thumbnailers (`*.thumbnailer`) parse files from anywhere, so
//! they only ever run in a bubblewrap sandbox (Recherche C §2, condition of
//! the status session, 23.10.2026):
//!
//! - the system read-only (`--ro-bind / /`), fresh `/dev` and `/proc`;
//! - `/tmp`, `/run` and the real home folder as empty tmpfs: no files of
//!   the user, no session bus, Wayland or GVfs sockets (a network
//!   namespace does not hide sockets in the file system, so `/run` is
//!   covered too);
//! - then only the input file read-only and the job's output folder
//!   writable, both at their real paths (`%i`, `%u`, `%o` stay as they
//!   are), and the thumbnailer itself if it lives below a hidden folder;
//! - an empty environment, `--unshare-all` (no network, own PID, IPC, UTS,
//!   user namespace), `--die-with-parent`, `--new-session`; the caller
//!   keeps the time limit.
//!
//! If bwrap is missing or cannot start a sandbox (checked once per
//! process with `/usr/bin/true`), thumbnailers count as not available:
//! never without a sandbox. One log line per run says so. GdkPixbuf/glycin
//! in the process stay as they are (glycin has its own sandbox).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

/// Whether foreign thumbnailers can run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Huelle {
    /// bwrap starts a sandbox: every thumbnailer runs in its own.
    Bwrap(PathBuf),
    /// No sandbox possible: thumbnailers stay off.
    Keine,
}

fn s(x: &Path) -> String {
    x.to_string_lossy().into_owned()
}

/// The sandbox's arguments in front of the input, output and program
/// binds: the system read-only, `/tmp`, `/run` and `heim` empty.
fn grundlage(heim: &Path) -> Vec<String> {
    let mut a: Vec<String> = [
        "--ro-bind",
        "/",
        "/",
        "--dev",
        "/dev",
        "--proc",
        "/proc",
        "--tmpfs",
        "/tmp",
        "--tmpfs",
        "/run",
    ]
    .map(str::to_owned)
    .to_vec();
    // a home folder below /tmp or /run is already empty
    if heim != Path::new("/") && !heim.starts_with("/tmp") && !heim.starts_with("/run") {
        a.extend(["--tmpfs".to_owned(), s(heim)]);
    }
    a
}

/// The arguments after the binds: environment and namespaces.
fn ende(heim: &Path, ordner: &Path) -> Vec<String> {
    let mut a: Vec<String> = [
        "--clearenv",
        "--setenv",
        "PATH",
        "/usr/bin:/bin",
        "--setenv",
        "HOME",
    ]
    .map(str::to_owned)
    .to_vec();
    a.push(s(heim));
    a.extend(
        [
            "--setenv",
            "TMPDIR",
            "/tmp",
            "--setenv",
            "GIO_USE_VFS",
            "local",
            "--unshare-all",
            "--die-with-parent",
            "--new-session",
            "--chdir",
        ]
        .map(str::to_owned),
    );
    a.push(s(ordner));
    a
}

/// Whether bwrap starts this sandbox here (~5 ms).
fn bwrap_geht(bwrap: &Path) -> bool {
    let heim = gtk4::glib::home_dir();
    let mut a = grundlage(&heim);
    a.extend(ende(&heim, Path::new("/")));
    a.extend(["--", "/usr/bin/true"].map(str::to_owned));
    Command::new(bwrap)
        .args(&a)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|st| st.success())
}

/// Whether thumbnailers can run in this process (decided once; one log
/// line when not).
pub fn huelle() -> &'static Huelle {
    static HUELLE: OnceLock<Huelle> = OnceLock::new();
    HUELLE.get_or_init(|| match gtk4::glib::find_program_in_path("bwrap") {
        Some(b) if bwrap_geht(&b) => Huelle::Bwrap(b),
        b => {
            crate::log(&format!(
                "Thumbnailer nicht verfügbar: {} (ohne Sandkasten startet keiner)",
                if b.is_some() {
                    "bwrap startet keinen Sandkasten"
                } else {
                    "bwrap fehlt"
                }
            ));
            Huelle::Keine
        }
    })
}

/// The command for `programm args` reading `eingabe` and writing into
/// `ordner` (the job folder), with `heim` hidden. `None` without a
/// sandbox.
pub fn befehl(
    huelle: &Huelle,
    heim: &Path,
    ordner: &Path,
    eingabe: &Path,
    programm: &Path,
    args: &[String],
) -> Option<Command> {
    let Huelle::Bwrap(bwrap) = huelle else {
        return None;
    };
    let mut a = grundlage(heim);
    a.extend(["--bind".to_owned(), s(ordner), s(ordner)]);
    a.extend(["--ro-bind".to_owned(), s(eingabe), s(eingabe)]);
    // a thumbnailer below a hidden folder (e.g. ~/.local/bin) is bound
    // itself, read-only
    let versteckt = [heim, Path::new("/tmp"), Path::new("/run")];
    if versteckt.iter().any(|v| programm.starts_with(v)) {
        a.extend(["--ro-bind".to_owned(), s(programm), s(programm)]);
    }
    a.extend(ende(heim, ordner));
    a.push("--".to_owned());
    a.push(s(programm));
    a.extend(args.iter().cloned());
    let mut c = Command::new(bwrap);
    c.args(a);
    Some(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(c: &Command) -> Vec<String> {
        c.get_args()
            .map(|x| x.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn the_sandbox_follows_the_conditions() {
        let h = Huelle::Bwrap(PathBuf::from("/usr/bin/bwrap"));
        let c = befehl(
            &h,
            Path::new("/home/a"),
            Path::new("/tmp/job-1"),
            Path::new("/home/a/Dokumente/b.pdf"),
            Path::new("/home/a/.local/bin/t"),
            &[
                "/home/a/Dokumente/b.pdf".to_owned(),
                "/tmp/job-1/ausgabe.png".to_owned(),
            ],
        )
        .unwrap();
        let a = args(&c);
        let text = a.join(" ");
        for muss in [
            "--ro-bind / /",
            "--dev /dev",
            "--proc /proc",
            "--tmpfs /tmp",
            "--tmpfs /run",
            "--tmpfs /home/a",
            "--bind /tmp/job-1 /tmp/job-1",
            "--ro-bind /home/a/Dokumente/b.pdf /home/a/Dokumente/b.pdf",
            "--ro-bind /home/a/.local/bin/t /home/a/.local/bin/t",
            "--clearenv",
            "--unshare-all",
            "--die-with-parent",
            "--new-session",
            "-- /home/a/.local/bin/t /home/a/Dokumente/b.pdf /tmp/job-1/ausgabe.png",
        ] {
            assert!(text.contains(muss), "{muss} fehlt in {text}");
        }
        assert!(!text.contains("--share-net"));
        assert_eq!(
            a.iter().filter(|x| *x == "--bind").count(),
            1,
            "nur der Ausgabeordner schreibbar"
        );
        // the empty folders come before the binds into them
        let pos = |x: &str| a.iter().position(|y| y == x).unwrap();
        assert!(pos("/home/a") < pos("/home/a/Dokumente/b.pdf"));
        assert!(pos("/tmp") < pos("/tmp/job-1"));
        // a program in /usr is not bound again
        let c = befehl(
            &h,
            Path::new("/home/a"),
            Path::new("/tmp/j"),
            Path::new("/e"),
            Path::new("/usr/bin/t"),
            &[],
        )
        .unwrap();
        assert!(!args(&c).join(" ").contains("--ro-bind /usr/bin/t"));
        assert!(
            befehl(
                &Huelle::Keine,
                Path::new("/h"),
                Path::new("/t"),
                Path::new("/e"),
                Path::new("/usr/bin/t"),
                &[]
            )
            .is_none()
        );
    }
}
