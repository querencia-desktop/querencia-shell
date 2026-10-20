//! Where the panel finds the Querencia programs it starts, and "Bildschirm
//! sperren" in the System menu (G2-AK, QD-069).
//!
//! A program is looked up in this order ([`programm`]):
//! 1. the environment variable named after it ([`variable`]:
//!    `querencia-sitzung` → `QUERENCIA_SITZUNG`), an absolute path. Once
//!    set, it alone counts: whoever names a program (a test, a user) never
//!    gets another one in its place;
//! 2. the absolute entries of `PATH`, in order;
//! 3. `~/opt/querencia/bin`, where the integration scripts link the
//!    programs on the development machine;
//! 4. `/usr/bin`, where the packages install them, for a `PATH` without it.
//!
//! Found is an executable regular file, as `which` would take it. The panel
//! of the package (Pflichtprobe B10) knew only the fixed path of step 3 and
//! fell back to its own logout question; that question stays as the
//! fallback for a session without Querencia Sitzung (`session.rs`).
//!
//! "Bildschirm sperren" runs `querencia-sitzung --sperren --sofort`, the
//! same command as Strg+Alt+L in the labwc configuration; without Querencia
//! Sitzung the entry is not offered.

use std::ffi::{OsStr, OsString};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use gtk4::{gio, glib};

/// Arguments for locking at once, without a question.
pub const SPERREN: [&str; 2] = ["--sperren", "--sofort"];

/// The shortcut the labwc configuration binds to [`SPERREN`]
/// (`session/querencia/labwc/rc.xml`), in GTK's accelerator notation; the
/// menu shows it next to the entry.
pub const SPERREN_KUERZEL: &str = "<Control><Alt>l";

/// Directory of the development links, below the home directory.
const OPT_BIN: &str = "opt/querencia/bin";
/// Directory of the packaged programs.
const USR_BIN: &str = "/usr/bin";

/// Name of the environment variable that names program `name`: upper case,
/// `-` as `_` (`querencia-lautstaerke` → `QUERENCIA_LAUTSTAERKE`).
pub fn variable(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '-' => '_',
            c => c.to_ascii_uppercase(),
        })
        .collect()
}

/// Where program `name` is, by the order of the module documentation:
/// `named` is the value of its [`variable`], `path` that of `PATH`; `usable`
/// tells whether a file is an executable program. The error says why
/// nothing was found.
pub fn programm(
    name: &str,
    named: Option<&OsStr>,
    path: Option<&OsStr>,
    home: &Path,
    usable: impl Fn(&Path) -> bool,
) -> Result<PathBuf, String> {
    if let Some(named) = named.filter(|v| !v.is_empty()) {
        let named = Path::new(named);
        let var = variable(name);
        return if !named.is_absolute() {
            Err(format!("{var}={}: kein absoluter Pfad", named.display()))
        } else if usable(named) {
            Ok(named.to_path_buf())
        } else {
            Err(format!(
                "{var}={}: kein ausführbares Programm",
                named.display()
            ))
        };
    }
    path.into_iter()
        .flat_map(std::env::split_paths)
        .filter(|dir| dir.is_absolute())
        .chain([home.join(OPT_BIN), PathBuf::from(USR_BIN)])
        .map(|dir| dir.join(name))
        .find(|candidate| usable(candidate))
        .ok_or_else(|| format!("{name} nicht gefunden (PATH, ~/{OPT_BIN}, {USR_BIN})"))
}

/// [`programm`] in this process's environment.
pub fn suchen(name: &str) -> Result<PathBuf, String> {
    programm(
        name,
        std::env::var_os(variable(name)).as_deref(),
        std::env::var_os("PATH").as_deref(),
        &glib::home_dir(),
        ausfuehrbar,
    )
}

/// An executable regular file (after following links).
fn ausfuehrbar(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

/// Locks the screen through Querencia Sitzung ([`SPERREN`]); without it
/// only a log line.
pub fn sperren() {
    match suchen(crate::session::SITZUNG_PROGRAM) {
        Ok(sitzung) => starten(&sitzung, &SPERREN),
        Err(err) => eprintln!("querencia-panel: Bildschirm sperren nicht möglich: {err}"),
    }
}

/// Starts `programm` with `args` as a child that GIO reaps; start and exit
/// status go to the log.
fn starten(programm: &Path, args: &[&str]) {
    let command = std::iter::once(programm.display().to_string())
        .chain(args.iter().map(|a| (*a).to_owned()))
        .collect::<Vec<_>>()
        .join(" ");
    let argv: Vec<OsString> = std::iter::once(programm.as_os_str().to_owned())
        .chain(args.iter().map(OsString::from))
        .collect();
    let argv: Vec<&OsStr> = argv.iter().map(OsString::as_os_str).collect();
    let launcher = gio::SubprocessLauncher::new(gio::SubprocessFlags::STDIN_INHERIT);
    match launcher.spawn(&argv) {
        Ok(child) => {
            eprintln!("querencia-panel: {command} gestartet");
            child.wait_check_async(gio::Cancellable::NONE, move |result| match result {
                Ok(()) => eprintln!("querencia-panel: {command} beendet"),
                Err(err) => eprintln!("querencia-panel: {command}: {err}"),
            });
        }
        Err(err) => eprintln!("querencia-panel: {command} fehlgeschlagen: {err}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOME: &str = "/home/x";
    const SITZUNG: &str = "querencia-sitzung";

    fn finde(
        named: Option<&str>,
        path: Option<&str>,
        usable: impl Fn(&Path) -> bool,
    ) -> Result<PathBuf, String> {
        programm(
            SITZUNG,
            named.map(OsStr::new),
            path.map(OsStr::new),
            Path::new(HOME),
            usable,
        )
    }

    fn nur<'a>(paths: &'a [&'a str]) -> impl Fn(&Path) -> bool + 'a {
        move |p| paths.iter().any(|q| p == Path::new(q))
    }

    #[test]
    fn variable_names_follow_the_program_names() {
        assert_eq!(variable("querencia-sitzung"), "QUERENCIA_SITZUNG");
        assert_eq!(variable("querencia-lautstaerke"), "QUERENCIA_LAUTSTAERKE");
        assert_eq!(variable("querencia-polkit-agent"), "QUERENCIA_POLKIT_AGENT");
    }

    #[test]
    fn order_variable_path_opt_then_usr_bin() {
        let opt = "/home/x/opt/querencia/bin/querencia-sitzung";
        let usr = "/usr/bin/querencia-sitzung";
        let local = "/usr/local/bin/querencia-sitzung";
        let test = "/tmp/attrappe/querencia-sitzung";
        let all = [opt, usr, local, test];
        let path = Some("/usr/local/bin:/usr/bin");
        // 1. the variable before everything else
        assert_eq!(finde(Some(test), path, nur(&all)), Ok(PathBuf::from(test)));
        // 2. PATH in its order
        assert_eq!(finde(None, path, nur(&all)), Ok(PathBuf::from(local)));
        assert_eq!(
            finde(None, Some("/usr/bin:/usr/local/bin"), nur(&all)),
            Ok(PathBuf::from(usr))
        );
        // 3. ~/opt/querencia/bin when PATH has none
        assert_eq!(
            finde(None, Some("/bin:/sbin"), nur(&[opt, usr])),
            Ok(PathBuf::from(opt))
        );
        // 4. /usr/bin last, also without PATH (the package)
        assert_eq!(
            finde(None, Some("/bin"), nur(&[usr])),
            Ok(PathBuf::from(usr))
        );
        assert_eq!(finde(None, None, nur(&[usr])), Ok(PathBuf::from(usr)));
        assert_eq!(finde(None, None, nur(&[opt])), Ok(PathBuf::from(opt)));
    }

    #[test]
    fn package_case_of_the_pflichtprobe() {
        // B10: only /usr/bin/querencia-sitzung, nothing under ~/opt
        let usr = "/usr/bin/querencia-sitzung";
        let path = Some("/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin");
        assert_eq!(finde(None, path, nur(&[usr])), Ok(PathBuf::from(usr)));
    }

    #[test]
    fn a_set_variable_alone_counts() {
        let usr = "/usr/bin/querencia-sitzung";
        // set but unusable: no other program in its place
        let err = finde(Some("/fehlt/querencia-sitzung"), None, nur(&[usr])).unwrap_err();
        assert!(err.starts_with("QUERENCIA_SITZUNG=/fehlt/"), "{err}");
        let err = finde(Some("relativ/querencia-sitzung"), None, |_| true).unwrap_err();
        assert!(err.contains("kein absoluter Pfad"), "{err}");
        // set but empty: as if unset
        assert_eq!(finde(Some(""), None, nur(&[usr])), Ok(PathBuf::from(usr)));
    }

    #[test]
    fn relative_path_entries_are_never_used() {
        let relative = "bin/querencia-sitzung";
        assert!(finde(None, Some("bin:./bin"), nur(&[relative])).is_err());
        let err = finde(None, Some("bin"), |_| false).unwrap_err();
        assert!(err.contains("nicht gefunden"), "{err}");
    }

    #[test]
    fn only_executable_regular_files_are_programs() {
        let dir = std::env::temp_dir().join(format!("g2ak-programm-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("ordner")).unwrap();
        let datei = dir.join("datei");
        let programm = dir.join("programm");
        std::fs::write(&datei, "").unwrap();
        std::fs::write(&programm, "").unwrap();
        std::fs::set_permissions(&datei, std::fs::Permissions::from_mode(0o644)).unwrap();
        std::fs::set_permissions(&programm, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(ausfuehrbar(&programm));
        assert!(!ausfuehrbar(&datei));
        assert!(!ausfuehrbar(&dir.join("ordner")));
        assert!(!ausfuehrbar(&dir.join("fehlt")));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn locking_is_the_command_of_strg_alt_l() {
        assert_eq!(SPERREN, ["--sperren", "--sofort"]);
        let rc = include_str!("../../../session/querencia/labwc/rc.xml");
        let bind = rc
            .split("<keybind ")
            .find(|b| b.starts_with("key=\"C-A-l\""))
            .expect("C-A-l in rc.xml");
        assert!(bind.contains("querencia-sitzung --sperren --sofort"), "{bind}");
        assert_eq!(SPERREN_KUERZEL, "<Control><Alt>l");
    }
}
