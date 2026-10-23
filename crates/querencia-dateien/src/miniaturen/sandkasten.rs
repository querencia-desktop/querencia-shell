//! Foreign thumbnailers run in a bubblewrap sandbox, as GNOME runs them
//! (`gnome-desktop-thumbnail-script.c`): no network and no other
//! namespaces of the session (`--unshare-all`), an empty environment,
//! `/usr` read-only, the file to read bound read-only to
//! `/tmp/eingabe.<ext>`, a fresh folder of the job as `/tmp` for the
//! output, `--die-with-parent`, and the caller's time limit.
//!
//! Without a working bwrap no foreign thumbnailer runs. The one exception
//! is the check frame (`werkzeug/dateien-pruefrahmen/huelle.sh`: own user,
//! PID and network namespace with only `lo`), where the kernel allows no
//! nested user namespace; there the thumbnailer runs in the frame's own
//! sandbox (recognised by `QD_HUELLE` and `XDG_RUNTIME_DIR=/tmp/qd-*`).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

/// Where a thumbnailer runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Huelle {
    /// bwrap works: every thumbnailer runs in its own sandbox.
    Bwrap(PathBuf),
    /// Inside the check frame (no nested sandbox possible).
    Pruefrahmen,
    /// No sandbox: foreign thumbnailers stay off.
    Keine,
}

/// The paths of one job as the thumbnailer sees them.
pub struct Pfade {
    pub eingabe: PathBuf,
    pub uri: String,
    pub ausgabe: PathBuf,
}

/// Whether this process runs in the check frame.
pub fn im_pruefrahmen() -> bool {
    std::env::var_os("QD_HUELLE").is_some()
        && std::env::var("XDG_RUNTIME_DIR").is_ok_and(|r| r.starts_with("/tmp/qd-"))
}

/// The arguments that give a thumbnailer the system without the session:
/// `/usr` and the links of merged `/usr`, the few files of `/etc` programs
/// need to start, `/proc`, `/dev`, the job folder as `/tmp`.
fn grundlage(auftrag: &Path) -> Vec<String> {
    let mut a: Vec<String> = ["--ro-bind", "/usr", "/usr"].map(str::to_owned).to_vec();
    for d in ["bin", "sbin", "lib", "lib64", "lib32"] {
        let p = Path::new("/").join(d);
        match std::fs::read_link(&p) {
            Ok(ziel) => a.extend([
                "--symlink".to_owned(),
                ziel.to_string_lossy().into_owned(),
                p.to_string_lossy().into_owned(),
            ]),
            Err(_) if p.is_dir() => a.extend([
                "--ro-bind".to_owned(),
                p.to_string_lossy().into_owned(),
                p.to_string_lossy().into_owned(),
            ]),
            Err(_) => {}
        }
    }
    for p in [
        "/etc/ld.so.cache",
        "/etc/alternatives",
        "/etc/fonts",
        "/var/cache/fontconfig",
    ] {
        a.extend(["--ro-bind-try", p, p].map(str::to_owned));
    }
    a.extend(["--proc", "/proc", "--dev", "/dev", "--bind"].map(str::to_owned));
    a.push(auftrag.to_string_lossy().into_owned());
    a.push("/tmp".to_owned());
    a
}

const ENDE: [&str; 15] = [
    "--chdir",
    "/tmp",
    "--clearenv",
    "--setenv",
    "PATH",
    "/usr/bin:/bin",
    "--setenv",
    "HOME",
    "/tmp",
    "--setenv",
    "GIO_USE_VFS",
    "local",
    "--unshare-all",
    "--die-with-parent",
    "--new-session",
];

/// Whether bwrap starts a sandbox here (once per process, ~5 ms).
fn bwrap_geht(bwrap: &Path) -> bool {
    let d = std::env::temp_dir();
    let mut a = grundlage(&d);
    a.extend(ENDE.map(str::to_owned));
    a.extend(["--", "/usr/bin/true"].map(str::to_owned));
    Command::new(bwrap)
        .args(&a)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Where thumbnailers run in this process (decided once).
pub fn huelle() -> &'static Huelle {
    static HUELLE: OnceLock<Huelle> = OnceLock::new();
    HUELLE.get_or_init(|| {
        let bwrap = gtk4::glib::find_program_in_path("bwrap");
        match bwrap {
            Some(b) if bwrap_geht(&b) => Huelle::Bwrap(b),
            _ if im_pruefrahmen() => Huelle::Pruefrahmen,
            _ => Huelle::Keine,
        }
    })
}

/// The paths of a job: in the sandbox `/tmp/eingabe<ext>` and
/// `/tmp/ausgabe.png`, in the check frame the real ones.
pub fn pfade(huelle: &Huelle, auftrag: &Path, eingabe: &Path, uri: &str, endung: &str) -> Pfade {
    match huelle {
        Huelle::Bwrap(_) => {
            let innen = PathBuf::from(format!("/tmp/eingabe{endung}"));
            Pfade {
                uri: format!("file://{}", innen.display()),
                eingabe: innen,
                ausgabe: PathBuf::from("/tmp/ausgabe.png"),
            }
        }
        _ => Pfade {
            eingabe: eingabe.to_owned(),
            uri: uri.to_owned(),
            ausgabe: auftrag.join("ausgabe.png"),
        },
    }
}

/// The command for `programm args` of a job in `auftrag` (host folder)
/// reading `eingabe` (host path). `None` without a sandbox.
pub fn befehl(
    huelle: &Huelle,
    auftrag: &Path,
    eingabe: &Path,
    endung: &str,
    programm: &Path,
    args: &[String],
) -> Option<Command> {
    match huelle {
        Huelle::Keine => None,
        Huelle::Pruefrahmen => {
            let mut c = Command::new(programm);
            c.args(args);
            Some(c)
        }
        Huelle::Bwrap(bwrap) => {
            let mut a = grundlage(auftrag);
            // the input over the job folder (bound after it), unless it is
            // already in there (files read through GIO)
            if !eingabe.starts_with(auftrag) {
                a.extend([
                    "--ro-bind".to_owned(),
                    eingabe.to_string_lossy().into_owned(),
                    format!("/tmp/eingabe{endung}"),
                ]);
            }
            // a thumbnailer outside /usr (e.g. ~/.local/bin) is bound itself
            if !programm.starts_with("/usr") {
                let p = programm.to_string_lossy().into_owned();
                a.extend(["--ro-bind".to_owned(), p.clone(), p]);
            }
            a.extend(ENDE.map(str::to_owned));
            a.push("--".to_owned());
            a.push(programm.to_string_lossy().into_owned());
            a.extend(args.iter().cloned());
            let mut c = Command::new(bwrap);
            c.args(a);
            Some(c)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sandbox_shares_nothing_but_usr_and_the_job() {
        let h = Huelle::Bwrap(PathBuf::from("/usr/bin/bwrap"));
        let p = pfade(
            &h,
            Path::new("/x/1"),
            Path::new("/home/a/b.pdf"),
            "file:///home/a/b.pdf",
            ".pdf",
        );
        assert_eq!(p.eingabe, Path::new("/tmp/eingabe.pdf"));
        assert_eq!(p.uri, "file:///tmp/eingabe.pdf");
        assert_eq!(p.ausgabe, Path::new("/tmp/ausgabe.png"));
        let c = befehl(
            &h,
            Path::new("/x/1"),
            Path::new("/home/a/b.pdf"),
            ".pdf",
            Path::new("/home/a/.local/bin/t"),
            &["/tmp/eingabe.pdf".to_owned()],
        )
        .unwrap();
        let a: Vec<String> = c
            .get_args()
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        let text = a.join(" ");
        for muss in [
            "--unshare-all",
            "--die-with-parent",
            "--clearenv",
            "--ro-bind /usr /usr",
            "--bind /x/1 /tmp",
            "--ro-bind /home/a/b.pdf /tmp/eingabe.pdf",
            "--ro-bind /home/a/.local/bin/t /home/a/.local/bin/t",
            "-- /home/a/.local/bin/t /tmp/eingabe.pdf",
        ] {
            assert!(text.contains(muss), "{muss} fehlt in {text}");
        }
        assert!(!text.contains("--bind /home"), "kein schreibbares HOME");
        assert!(!text.contains("--share-net"));
        // the job folder comes before the input bound into it
        let tmp = a.iter().position(|x| x == "/tmp").unwrap();
        let ein = a.iter().position(|x| x == "/tmp/eingabe.pdf").unwrap();
        assert!(tmp < ein);
        assert!(
            befehl(
                &Huelle::Keine,
                Path::new("/x"),
                Path::new("/y"),
                "",
                Path::new("/usr/bin/t"),
                &[]
            )
            .is_none()
        );
        let pr = pfade(
            &Huelle::Pruefrahmen,
            Path::new("/x/1"),
            Path::new("/a.pdf"),
            "file:///a.pdf",
            ".pdf",
        );
        assert_eq!(pr.ausgabe, Path::new("/x/1/ausgabe.png"));
        assert_eq!(pr.uri, "file:///a.pdf");
    }
}
