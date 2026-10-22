//! `querencia-panel --ueberwacht`: the panel under a small watchdog. This
//! process stays without GTK and without a display connection; it starts
//! the panel (the same program, without `--ueberwacht`) as its child and
//! starts it again when it crashed, at most [`MAX_NEUSTARTS`] times within
//! [`FENSTER`]. Every start and every end goes to the log (standard error,
//! which the session writes to `panel.log`).
//!
//! What counts as a crash ([`Ende::absturz`]): death by a signal of a
//! program error (SIGSEGV, SIGABRT, SIGBUS, SIGILL, SIGFPE, SIGTRAP,
//! SIGSYS) or exit status 101, Rust's status after a panic. Every other end
//! is the panel's own decision (no layer shell, a second instance, the
//! compositor gone, SIGTERM from the session) and ends the watchdog with
//! the same status. Nor is the panel started again when the session is
//! over: SIGTERM, SIGINT or SIGHUP to the watchdog are passed on to the
//! panel, and without a Wayland socket or with labwc (`LABWC_PID`) gone
//! there is nothing to start into.
//!
//! The session's starter records the PID of what it started; with
//! `--ueberwacht` that is this process, and ending it ends the panel.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk4::glib;

/// The option that starts the watchdog.
pub const OPTION: &str = "--ueberwacht";
/// Restarts allowed within [`FENSTER`].
pub const MAX_NEUSTARTS: usize = 3;
/// The window of [`MAX_NEUSTARTS`].
pub const FENSTER: Duration = Duration::from_secs(10 * 60);
/// Pause before a restart, so a panel that crashes at once does not spin.
const PAUSE: Duration = Duration::from_secs(1);
/// How long the panel gets to end after SIGTERM before SIGKILL.
const ENDE_FRIST: Duration = Duration::from_secs(5);
const LOG: &str = "querencia-panel: Überwachung:";

const SIGHUP: i32 = 1;
const SIGINT: i32 = 2;
const SIGKILL: i32 = 9;
const SIGTERM: i32 = 15;

unsafe extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
}

/// How the panel ended, from the wait status of `waitpid(2)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ende {
    Status(i32),
    Signal(i32),
}

impl Ende {
    pub fn aus_wait_status(status: i32) -> Ende {
        match status & 0x7f {
            0 => Ende::Status((status >> 8) & 0xff),
            signal => Ende::Signal(signal),
        }
    }

    /// A program error rather than the panel's own decision.
    pub fn absturz(self) -> bool {
        match self {
            // SIGILL, SIGTRAP, SIGABRT, SIGBUS, SIGFPE, SIGSEGV, SIGSYS
            Ende::Signal(signal) => matches!(signal, 4 | 5 | 6 | 7 | 8 | 11 | 31),
            Ende::Status(status) => status == 101,
        }
    }

    /// The watchdog's own exit status for this end, as a shell reports it.
    pub fn exit_status(self) -> u8 {
        match self {
            Ende::Status(status) => status as u8,
            Ende::Signal(signal) => (128 + signal) as u8,
        }
    }

    fn beschreibung(self) -> String {
        match self {
            Ende::Status(status) => format!("mit Status {status}"),
            Ende::Signal(signal) => format!("durch Signal {signal} ({})", signal_name(signal)),
        }
    }
}

fn signal_name(signal: i32) -> &'static str {
    match signal {
        1 => "SIGHUP",
        2 => "SIGINT",
        4 => "SIGILL",
        5 => "SIGTRAP",
        6 => "SIGABRT",
        7 => "SIGBUS",
        8 => "SIGFPE",
        9 => "SIGKILL",
        11 => "SIGSEGV",
        13 => "SIGPIPE",
        15 => "SIGTERM",
        31 => "SIGSYS",
        _ => "?",
    }
}

/// The restarts of the last [`FENSTER`].
#[derive(Debug, Default)]
pub struct Neustarts {
    zeiten: Vec<Instant>,
}

impl Neustarts {
    /// Takes a restart at `jetzt` if fewer than [`MAX_NEUSTARTS`] fall
    /// into the window before it; returns its number (1 …), else `None`.
    pub fn erlauben(&mut self, jetzt: Instant) -> Option<usize> {
        self.zeiten
            .retain(|zeit| jetzt.saturating_duration_since(*zeit) < FENSTER);
        if self.zeiten.len() >= MAX_NEUSTARTS {
            return None;
        }
        self.zeiten.push(jetzt);
        Some(self.zeiten.len())
    }
}

/// What the watchdog does after the panel ended.
#[derive(Debug, PartialEq, Eq)]
pub enum Entscheidung {
    /// Start again; the number of the restart in the window.
    Neustart(usize),
    /// End with the panel's status; why, for the log.
    Ende(&'static str),
}

/// The rules of the module documentation. `sitzung_aus`: the watchdog was
/// asked to end; `sitzung_da`: Wayland socket and labwc still there.
pub fn entscheiden(
    ende: Ende,
    sitzung_aus: bool,
    sitzung_da: bool,
    neustarts: &mut Neustarts,
    jetzt: Instant,
) -> Entscheidung {
    if sitzung_aus {
        Entscheidung::Ende("die Überwachung wurde beendet")
    } else if !ende.absturz() {
        Entscheidung::Ende("kein Absturz")
    } else if !sitzung_da {
        Entscheidung::Ende("die Sitzung ist vorbei (kein Wayland oder kein labwc)")
    } else {
        match neustarts.erlauben(jetzt) {
            Some(nummer) => Entscheidung::Neustart(nummer),
            None => Entscheidung::Ende("Grenze erreicht"),
        }
    }
}

/// The Wayland socket of `wayland_display` (absolute, or below
/// `runtime_dir`) exists, and the labwc of `labwc_pid`, if named, runs.
pub fn sitzung_da(
    runtime_dir: Option<&Path>,
    wayland_display: Option<&str>,
    labwc_pid: Option<&str>,
    proc: &Path,
) -> bool {
    let display = wayland_display.unwrap_or("wayland-0");
    let socket = if Path::new(display).is_absolute() {
        Some(PathBuf::from(display))
    } else {
        runtime_dir.map(|dir| dir.join(display))
    };
    let socket_da = socket.is_some_and(|socket| socket.exists());
    let labwc_da = match labwc_pid.and_then(|pid| pid.trim().parse::<u32>().ok()) {
        Some(pid) => proc.join(pid.to_string()).exists(),
        None => true,
    };
    socket_da && labwc_da
}

fn sitzung_da_hier() -> bool {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from);
    sitzung_da(
        runtime.as_deref(),
        std::env::var("WAYLAND_DISPLAY").ok().as_deref(),
        std::env::var("LABWC_PID").ok().as_deref(),
        Path::new("/proc"),
    )
}

/// The arguments for the panel: `args` without [`OPTION`].
pub fn kind_argumente(args: &[String]) -> Vec<String> {
    args.iter().filter(|a| *a != OPTION).cloned().collect()
}

struct Zustand {
    programm: PathBuf,
    args: Vec<String>,
    kind: Cell<Option<(u32, Instant)>>,
    sitzung_aus: Cell<bool>,
    neustarts: RefCell<Neustarts>,
    status: Cell<u8>,
    main_loop: glib::MainLoop,
}

/// Runs the watchdog until the panel ends for good; returns its status.
pub fn laufen(args: &[String]) -> glib::ExitCode {
    let programm = match std::env::current_exe() {
        Ok(programm) => programm,
        Err(err) => {
            eprintln!("{LOG} eigenes Programm nicht gefunden: {err}");
            return glib::ExitCode::FAILURE;
        }
    };
    let zustand = Rc::new(Zustand {
        programm,
        args: kind_argumente(args),
        kind: Cell::new(None),
        sitzung_aus: Cell::new(false),
        neustarts: RefCell::new(Neustarts::default()),
        status: Cell::new(0),
        main_loop: glib::MainLoop::new(None, false),
    });
    for signal in [SIGTERM, SIGINT, SIGHUP] {
        let zustand = zustand.clone();
        glib_unix::unix_signal_add_local(signal, move || {
            beenden(&zustand, signal);
            glib::ControlFlow::Continue
        });
    }
    eprintln!(
        "{LOG} aktiv (PID {}, höchstens {MAX_NEUSTARTS} Neustarts in {} Minuten)",
        std::process::id(),
        FENSTER.as_secs() / 60
    );
    if !starten(&zustand) {
        return glib::ExitCode::FAILURE;
    }
    zustand.main_loop.run();
    glib::ExitCode::from(zustand.status.get())
}

/// Starts the panel and watches it; false if it could not be started.
fn starten(zustand: &Rc<Zustand>) -> bool {
    let kind = match std::process::Command::new(&zustand.programm)
        .args(&zustand.args)
        .spawn()
    {
        Ok(kind) => kind,
        Err(err) => {
            eprintln!(
                "{LOG} Panel nicht gestartet ({}): {err}",
                zustand.programm.display()
            );
            return false;
        }
    };
    let pid = kind.id();
    // GLib's child watch reaps it; the handle is not needed any more.
    drop(kind);
    zustand.kind.set(Some((pid, Instant::now())));
    eprintln!("{LOG} Panel gestartet (PID {pid})");
    let zustand = zustand.clone();
    glib::child_watch_add_local(glib::Pid(pid as i32), move |_, status| {
        beendet(&zustand, pid, Ende::aus_wait_status(status));
    });
    true
}

fn beendet(zustand: &Rc<Zustand>, pid: u32, ende: Ende) {
    let laufzeit = zustand
        .kind
        .take()
        .map(|(_, seit)| seit.elapsed().as_secs())
        .unwrap_or_default();
    let entscheidung = entscheiden(
        ende,
        zustand.sitzung_aus.get(),
        sitzung_da_hier(),
        &mut zustand.neustarts.borrow_mut(),
        Instant::now(),
    );
    let wie = ende.beschreibung();
    let was = if ende.absturz() {
        "abgestürzt"
    } else {
        "beendet"
    };
    match entscheidung {
        Entscheidung::Neustart(nummer) => {
            eprintln!(
                "{LOG} Panel PID {pid} {was} {wie} nach {laufzeit} s – Neustart {nummer} von {MAX_NEUSTARTS} in {} Minuten",
                FENSTER.as_secs() / 60
            );
            let zustand = zustand.clone();
            glib::timeout_add_local_once(PAUSE, move || {
                if zustand.sitzung_aus.get() || !starten(&zustand) {
                    zustand.status.set(ende.exit_status());
                    zustand.main_loop.quit();
                }
            });
        }
        Entscheidung::Ende(grund) => {
            let zusatz = if grund == "Grenze erreicht" {
                format!(
                    "{MAX_NEUSTARTS} Neustarts in {} Minuten erreicht, kein weiterer Neustart",
                    FENSTER.as_secs() / 60
                )
            } else {
                format!("{grund}, kein Neustart")
            };
            eprintln!("{LOG} Panel PID {pid} {was} {wie} nach {laufzeit} s – {zusatz}");
            zustand.status.set(ende.exit_status());
            zustand.main_loop.quit();
        }
    }
}

/// SIGTERM, SIGINT or SIGHUP to the watchdog: pass it on to the panel, and
/// end it for good after [`ENDE_FRIST`].
fn beenden(zustand: &Rc<Zustand>, signal: i32) {
    if zustand.sitzung_aus.replace(true) {
        return;
    }
    let Some((pid, _)) = zustand.kind.get() else {
        eprintln!("{LOG} {} erhalten, kein Panel läuft", signal_name(signal));
        zustand.status.set(128 + signal as u8);
        zustand.main_loop.quit();
        return;
    };
    eprintln!(
        "{LOG} {} erhalten, an Panel PID {pid} weitergegeben",
        signal_name(signal)
    );
    // SAFETY: kill(2) with a child PID this process started and not yet
    // reaped (the child watch clears `kind` when it is).
    unsafe { kill(pid as i32, signal) };
    let zustand = zustand.clone();
    glib::timeout_add_local_once(ENDE_FRIST, move || {
        if let Some((pid_jetzt, _)) = zustand.kind.get()
            && pid_jetzt == pid
        {
            eprintln!("{LOG} Panel PID {pid} endet nicht, SIGKILL");
            // SAFETY: as above, still our unreaped child.
            unsafe { kill(pid as i32, SIGKILL) };
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wait_status_decodes_exit_and_signal() {
        assert_eq!(Ende::aus_wait_status(0), Ende::Status(0));
        assert_eq!(Ende::aus_wait_status(1 << 8), Ende::Status(1));
        assert_eq!(Ende::aus_wait_status(101 << 8), Ende::Status(101));
        assert_eq!(Ende::aus_wait_status(11), Ende::Signal(11));
        // with the core-dump bit 0x80
        assert_eq!(Ende::aus_wait_status(0x80 | 6), Ende::Signal(6));
        assert_eq!(Ende::aus_wait_status(15), Ende::Signal(15));
    }

    #[test]
    fn only_program_errors_are_crashes() {
        for signal in [4, 5, 6, 7, 8, 11, 31] {
            assert!(Ende::Signal(signal).absturz(), "{signal}");
        }
        for signal in [1, 2, 9, 13, 15] {
            assert!(!Ende::Signal(signal).absturz(), "{signal}");
        }
        assert!(Ende::Status(101).absturz());
        for status in [0, 1, 2, 255] {
            assert!(!Ende::Status(status).absturz(), "{status}");
        }
        assert_eq!(Ende::Signal(11).exit_status(), 139);
        assert_eq!(Ende::Status(1).exit_status(), 1);
    }

    #[test]
    fn three_restarts_per_ten_minutes() {
        let mut neustarts = Neustarts::default();
        let t0 = Instant::now();
        let min = |m: u64| t0 + Duration::from_secs(m * 60);
        assert_eq!(neustarts.erlauben(t0), Some(1));
        assert_eq!(neustarts.erlauben(min(1)), Some(2));
        assert_eq!(neustarts.erlauben(min(2)), Some(3));
        assert_eq!(neustarts.erlauben(min(3)), None);
        assert_eq!(neustarts.erlauben(min(9)), None);
        // the first one leaves the window after ten minutes
        assert_eq!(neustarts.erlauben(min(10)), Some(3));
        assert_eq!(neustarts.erlauben(min(10)), None);
        assert_eq!(
            neustarts.erlauben(min(12) + Duration::from_secs(1)),
            Some(2)
        );
    }

    #[test]
    fn decisions() {
        let jetzt = Instant::now();
        let mut n = Neustarts::default();
        let segv = Ende::Signal(11);
        assert_eq!(
            entscheiden(segv, false, true, &mut n, jetzt),
            Entscheidung::Neustart(1)
        );
        assert_eq!(
            entscheiden(Ende::Status(101), false, true, &mut n, jetzt),
            Entscheidung::Neustart(2)
        );
        // the panel's own end, the session's SIGTERM, a gone compositor
        assert!(matches!(
            entscheiden(Ende::Status(1), false, true, &mut n, jetzt),
            Entscheidung::Ende(_)
        ));
        assert!(matches!(
            entscheiden(Ende::Signal(15), false, true, &mut n, jetzt),
            Entscheidung::Ende(_)
        ));
        assert!(matches!(
            entscheiden(segv, true, true, &mut n, jetzt),
            Entscheidung::Ende(_)
        ));
        assert!(matches!(
            entscheiden(segv, false, false, &mut n, jetzt),
            Entscheidung::Ende(_)
        ));
        // none of these used a restart
        assert_eq!(
            entscheiden(segv, false, true, &mut n, jetzt),
            Entscheidung::Neustart(3)
        );
        assert_eq!(
            entscheiden(segv, false, true, &mut n, jetzt),
            Entscheidung::Ende("Grenze erreicht")
        );
    }

    #[test]
    fn session_is_there_while_socket_and_labwc_are() {
        let dir =
            std::env::temp_dir().join(format!("querencia-ueberwachung-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("proc/4242")).unwrap();
        std::fs::write(dir.join("wayland-1"), "").unwrap();
        let proc = dir.join("proc");
        let da = |display: Option<&str>, labwc: Option<&str>| {
            sitzung_da(Some(&dir), display, labwc, &proc)
        };
        assert!(da(Some("wayland-1"), Some("4242")));
        assert!(da(Some("wayland-1"), None));
        assert!(!da(Some("wayland-1"), Some("4243")));
        assert!(!da(Some("wayland-2"), Some("4242")));
        // WAYLAND_DISPLAY unset means wayland-0
        assert!(!da(None, None));
        let absolut = dir.join("wayland-1");
        assert!(da(absolut.to_str(), Some("4242")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_panel_gets_the_arguments_without_the_option() {
        let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(kind_argumente(&args(&[OPTION])), Vec::<String>::new());
        assert_eq!(
            kind_argumente(&args(&["--x", OPTION, "--y"])),
            args(&["--x", "--y"])
        );
    }
}
