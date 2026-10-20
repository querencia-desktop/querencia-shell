//! Session actions of the "System" menu, as in GNOME 2 "Abmelden …" and
//! "Ausschalten …" (G2-AD, QD-053/QD-054).
//!
//! With Querencia Sitzung installed ([`sitzung_command`]), both open its
//! question with the 60-second countdown (`querencia-sitzung --abmelden`,
//! `--herunterfahren`): "Abbrechen" stops it, otherwise the session ends or
//! the computer shuts down by itself when the countdown runs out. The
//! program decides and acts; the panel only starts it.
//!
//! Without Querencia Sitzung, "Abmelden …" falls back to the panel's own
//! question (without countdown) and labwc's `--exit`, which sends SIGTERM
//! to `$LABWC_PID`; `querencia-session` waits for labwc and cleans up after
//! it, and greetd returns to the login screen. The labwc binary is the one
//! that runs the session (`/proc/<pid>/exe`), never whatever `labwc` the
//! `PATH` finds. "Ausschalten …" has no fallback and is not offered then.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Image, KeyvalTrigger, Label, NamedAction, Orientation, Shortcut,
    ShortcutController, Window, gdk, gio, glib,
};

use crate::add_dialog;

pub const LOGOUT_TITLE: &str = "Abmelden";
pub const LOGOUT_QUESTION: &str = "Möchten Sie sich wirklich abmelden?";
pub const LOGOUT_DETAIL: &str = "Alle Anwendungen dieser Sitzung werden beendet. \
                                 Was dort nicht gespeichert ist, geht verloren.";

/// The labwc binary whose `--exit` ends this session: the program of the
/// process `labwc_pid` (the value of `LABWC_PID`), looked up with `exe`,
/// if it is a labwc. `None` outside a labwc session, for a process that is
/// gone or not ours (no access to its `exe`), or for another program.
pub fn labwc_exit_command(
    labwc_pid: Option<&str>,
    exe: impl Fn(u32) -> Option<PathBuf>,
) -> Option<PathBuf> {
    let pid: u32 = labwc_pid?.parse().ok().filter(|pid| *pid > 0)?;
    let path = exe(pid)?;
    let name = path.file_name()?.to_str()?;
    (name == "labwc" || name.starts_with("labwc-")).then_some(path)
}

/// [`labwc_exit_command`] for this process.
pub fn logout_command() -> Option<PathBuf> {
    labwc_exit_command(std::env::var("LABWC_PID").ok().as_deref(), |pid| {
        std::fs::read_link(format!("/proc/{pid}/exe")).ok()
    })
}

/// File name of the session program (G2-K), found by [`querencia_program`].
pub const SITZUNG_PROGRAM: &str = "querencia-sitzung";

/// Where the Querencia program `name` is, by the order of
/// [`crate::sitzung_start`] (its variable, `PATH`, `~/opt/querencia/bin`,
/// `/usr/bin`); `named` is the variable's value, `usable` tells whether a
/// file is an executable program.
pub fn querencia_program(
    name: &str,
    named: Option<&std::ffi::OsStr>,
    path: Option<&std::ffi::OsStr>,
    home: &Path,
    usable: impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
    crate::sitzung_start::programm(name, named, path, home, usable).ok()
}

/// Querencia Sitzung, if installed ([`querencia_program`]).
pub fn sitzung_command() -> Option<PathBuf> {
    crate::sitzung_start::suchen(SITZUNG_PROGRAM).ok()
}

/// Starts `sitzung` with `arg` (`--abmelden`, `--herunterfahren`); its
/// question and countdown run in its own process, which inherits
/// `LABWC_PID` (and `QS_TROCKENLAUF` in the tests). GIO reaps it and the
/// exit status is logged.
pub fn run_sitzung(sitzung: &Path, arg: &str) {
    let command = format!("{} {arg}", sitzung.display());
    let started = gio::Subprocess::newv(
        &[sitzung.as_os_str(), std::ffi::OsStr::new(arg)],
        gio::SubprocessFlags::STDIN_INHERIT,
    );
    match started {
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

/// The running panel application, for the panel's own windows: belonging
/// to it gives them the panel's app ID (`org.example.Querencia.Panel`)
/// instead of the program's file name, so the window list shows them with
/// the Querencia logo (QREL-04c). `None` outside a running application
/// (tests).
pub(crate) fn panel_application() -> Option<gtk4::Application> {
    gio::Application::default().and_downcast()
}

/// Asks, as GNOME 2 did, whether to log out; "Abbrechen" has the focus.
/// Raises the open question in `slot` instead of asking twice.
pub fn confirm_logout(slot: &glib::WeakRef<Window>, labwc: PathBuf) {
    if let Some(window) = slot.upgrade() {
        window.present();
        return;
    }
    let window = Window::builder()
        .title(LOGOUT_TITLE)
        .resizable(false)
        .build();
    window.set_application(panel_application().as_ref());

    let icon = Image::from_gicon(&gio::ThemedIcon::from_names(&[
        "system-log-out",
        "system-log-out-symbolic",
        "application-exit",
    ]));
    icon.set_pixel_size(48);
    icon.set_valign(Align::Start);
    let question = Label::builder()
        .label(format!(
            "<b><big>{}</big></b>",
            glib::markup_escape_text(LOGOUT_QUESTION)
        ))
        .use_markup(true)
        .xalign(0.0)
        .build();
    let detail = Label::builder()
        .label(LOGOUT_DETAIL)
        .xalign(0.0)
        .wrap(true)
        .max_width_chars(40)
        .build();
    let text = GtkBox::new(Orientation::Vertical, 12);
    text.append(&question);
    text.append(&detail);
    let top = GtkBox::new(Orientation::Horizontal, 12);
    top.append(&icon);
    top.append(&text);

    let cancel = add_dialog::stock_button("_Abbrechen", "process-stop");
    cancel.connect_clicked(glib::clone!(
        #[weak]
        window,
        move |_| window.close()
    ));
    let logout = add_dialog::stock_button("Ab_melden", "system-log-out");
    logout.connect_clicked(glib::clone!(
        #[weak]
        window,
        move |_| {
            window.close();
            end_session(&labwc);
        }
    ));
    let buttons = GtkBox::new(Orientation::Horizontal, 6);
    buttons.set_halign(Align::End);
    buttons.append(&cancel);
    buttons.append(&logout);

    let content = GtkBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(18)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();
    content.append(&top);
    content.append(&buttons);
    window.set_child(Some(&content));
    close_on_escape(&window);
    GtkWindowExt::set_focus(&window, Some(&cancel));
    slot.set(Some(&window));
    window.present();
    eprintln!("querencia-panel: Abmelden: Rückfrage geöffnet");
}

/// Runs `<labwc> --exit`; the child inherits `LABWC_PID` from the panel.
fn end_session(labwc: &Path) {
    match Command::new(labwc)
        .arg("--exit")
        .stdin(Stdio::null())
        .spawn()
    {
        Ok(mut child) => {
            eprintln!(
                "querencia-panel: Abmelden: {} --exit gestartet",
                labwc.display()
            );
            // Reaps the short-lived child; the panel itself ends with labwc.
            std::thread::spawn(move || child.wait());
        }
        Err(err) => eprintln!(
            "querencia-panel: Abmelden: {} --exit fehlgeschlagen: {err}",
            labwc.display()
        ),
    }
}

/// Escape closes `window`, as in GNOME 2 dialogs.
pub(crate) fn close_on_escape(window: &Window) {
    let shortcuts = ShortcutController::new();
    shortcuts.add_shortcut(Shortcut::new(
        Some(KeyvalTrigger::new(
            gdk::Key::Escape,
            gdk::ModifierType::empty(),
        )),
        Some(NamedAction::new("window.close")),
    ));
    window.add_controller(shortcuts);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exe_of(pid: u32) -> Option<PathBuf> {
        match pid {
            1816 => Some(PathBuf::from("~/opt/labwc-0.9.8/bin/labwc")),
            42 => Some(PathBuf::from("/usr/bin/labwc")),
            7 => Some(PathBuf::from("/usr/bin/bash")),
            _ => None,
        }
    }

    #[test]
    fn the_running_labwc_ends_the_session() {
        assert_eq!(
            labwc_exit_command(Some("1816"), exe_of).as_deref(),
            Some(Path::new("~/opt/labwc-0.9.8/bin/labwc"))
        );
        assert_eq!(
            labwc_exit_command(Some("42"), exe_of).as_deref(),
            Some(Path::new("/usr/bin/labwc"))
        );
    }

    #[test]
    fn no_logout_without_a_labwc_of_this_user() {
        // not in a labwc session
        assert_eq!(labwc_exit_command(None, exe_of), None);
        // not a process ID
        for bad in ["", "abc", "-1", "0", "18 16", "1816\n"] {
            assert_eq!(labwc_exit_command(Some(bad), exe_of), None, "{bad:?}");
        }
        // gone, or not ours (no access to /proc/<pid>/exe)
        assert_eq!(labwc_exit_command(Some("99"), exe_of), None);
        // some other program
        assert_eq!(labwc_exit_command(Some("7"), exe_of), None);
        // PID 0 never, even if a lookup claimed a labwc for it: labwc --exit
        // would send SIGTERM to kill(0), the whole process group
        let any_labwc = |_| Some(PathBuf::from("/usr/bin/labwc"));
        assert_eq!(labwc_exit_command(Some("0"), any_labwc), None);
        assert!(labwc_exit_command(Some("5"), any_labwc).is_some());
    }

    #[test]
    fn querencia_programs_from_path_then_opt_querencia() {
        let home = Path::new("/home/x");
        let path = std::ffi::OsString::from("relative/bin:/usr/local/bin:/usr/bin");
        let opt = Path::new("/home/x/opt/querencia/bin/querencia-sitzung");
        let find = |exists: &dyn Fn(&Path) -> bool| {
            querencia_program(SITZUNG_PROGRAM, None, Some(&path), home, exists)
        };
        assert_eq!(find(&|p| p == opt).as_deref(), Some(opt));
        assert_eq!(
            find(&|p| p == opt || p == Path::new("/usr/bin/querencia-sitzung")).as_deref(),
            Some(Path::new("/usr/bin/querencia-sitzung"))
        );
        // Relative PATH entries are never used.
        assert_eq!(
            find(&|p| p == Path::new("relative/bin/querencia-sitzung")),
            None
        );
        assert_eq!(find(&|_| false), None);
        assert_eq!(
            querencia_program(SITZUNG_PROGRAM, None, None, home, |p| p == opt).as_deref(),
            Some(opt)
        );
        // QD-069: the package's /usr/bin without PATH, the variable first
        let usr = Path::new("/usr/bin/querencia-sitzung");
        assert_eq!(
            querencia_program(SITZUNG_PROGRAM, None, None, home, |p| p == usr).as_deref(),
            Some(usr)
        );
        let test = std::ffi::OsString::from("/tmp/t/querencia-sitzung");
        assert_eq!(
            querencia_program(SITZUNG_PROGRAM, Some(&test), Some(&path), home, |_| true).as_deref(),
            Some(Path::new("/tmp/t/querencia-sitzung"))
        );
    }

    #[test]
    fn confirmation_texts_are_gnome2_like() {
        assert_eq!(LOGOUT_TITLE, "Abmelden");
        assert_eq!(LOGOUT_QUESTION, "Möchten Sie sich wirklich abmelden?");
        assert!(LOGOUT_DETAIL.contains("nicht gespeichert"));
    }
}
