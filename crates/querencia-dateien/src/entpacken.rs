//! „Hier entpacken“ in the context menu of archives (0.3.0), as the
//! file-roller extension added it to Nautilus 2.30: Querencia Archiv
//! unpacks every selected archive beside itself
//! (`querencia-archiv --entpacken-hier DATEI…`, its own process with its
//! own progress, conflict, password and error windows; a folder named
//! after the archive unless everything is in one folder already).
//!
//! The entry shows only when the program is there and every selected
//! object is a local file of a type Querencia Archiv names in its desktop
//! file (`MimeType=`). Unpacking is not part of the undo stack.

use std::path::PathBuf;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{gio, glib};

use crate::app::Register;
use crate::eintrag::Eintrag;
use crate::fenster::Fenster;
use crate::i18n::{gettext, gettext_f};

/// The desktop file of Querencia Archiv.
pub const ARCHIV_ID: &str = "org.querencia.Archiv.desktop";

/// The program: `QUERENCIA_ARCHIV`, then PATH, then `~/opt/querencia/bin`
/// (the rule of the panel, QD-069).
pub fn programm() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("QUERENCIA_ARCHIV").map(PathBuf::from)
        && p.is_absolute()
        && p.is_file()
    {
        return Some(p);
    }
    glib::find_program_in_path("querencia-archiv").or_else(|| {
        let p = glib::home_dir().join("opt/querencia/bin/querencia-archiv");
        p.is_file().then_some(p)
    })
}

/// Whether `typ` is one of `typen` (the types Querencia Archiv names) or
/// a subtype of one.
pub fn passt(typ: &str, typen: &[String]) -> bool {
    !typ.is_empty()
        && typ != "inode/directory"
        && typen
            .iter()
            .any(|t| t == typ || gio::content_type_is_a(typ, t))
}

/// The types Querencia Archiv opens (its `MimeType=`), empty without it.
fn archivtypen() -> Vec<String> {
    gio::AppInfo::all()
        .into_iter()
        .find(|a| a.id().is_some_and(|id| id == ARCHIV_ID))
        .map(|a| a.supported_types().iter().map(|t| t.to_string()).collect())
        .unwrap_or_default()
}

/// Whether „Hier entpacken“ fits `eintraege`.
pub fn moeglich(eintraege: &[Eintrag]) -> bool {
    if eintraege.is_empty() || programm().is_none() {
        return false;
    }
    let typen = archivtypen();
    eintraege.iter().all(|e| {
        let d = e.daten();
        !d.ist_ordner && d.datei.path().is_some() && passt(&d.typ, &typen)
    })
}

fn entpacken(fenster: &Rc<Fenster>) {
    let gewaehlt = fenster.modell().gewaehlte();
    if !moeglich(&gewaehlt) {
        crate::log("Hier entpacken: Auswahl passt nicht");
        return;
    }
    let Some(programm) = programm() else { return };
    let mut args: Vec<std::ffi::OsString> =
        vec![programm.clone().into(), "--entpacken-hier".into()];
    args.extend(
        gewaehlt
            .iter()
            .filter_map(|e| e.datei().path())
            .map(Into::into),
    );
    crate::log(&format!(
        "Hier entpacken: {} {}",
        programm.display(),
        gewaehlt
            .iter()
            .map(Eintrag::name)
            .collect::<Vec<_>>()
            .join(", ")
    ));
    let args: Vec<&std::ffi::OsStr> = args.iter().map(|a| a.as_os_str()).collect();
    match gio::Subprocess::newv(&args, gio::SubprocessFlags::NONE) {
        Ok(p) => p.wait_check_async(gio::Cancellable::NONE, |r| match r {
            Ok(()) => crate::log("Hier entpacken: fertig"),
            // Querencia Archiv has shown its own message
            Err(e) => crate::log(&format!("Hier entpacken: Archiv meldet {}", e.message())),
        }),
        Err(e) => fenster.melden(&gettext_f(
            "The Archive Manager could not be started: {error}",
            &[("error", e.message())],
        )),
    }
}

/// The action `win.hier-entpacken` of every window.
pub fn registrieren(register: &mut Register) {
    register.fenster(|fenster| {
        let aktion = gio::SimpleAction::new("hier-entpacken", None);
        let weak = Rc::downgrade(fenster);
        aktion.connect_activate(move |_, _| {
            if let Some(f) = weak.upgrade() {
                entpacken(&f);
            }
        });
        fenster.window().add_action(&aktion);
    });
}

/// The menu text (for the context menu's table).
pub fn text() -> String {
    gettext("_Extract Here")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_types_of_querencia_archiv_and_their_subtypes() {
        let typen: Vec<String> = ["application/zip", "application/x-tar", "application/gzip"]
            .iter()
            .map(|t| (*t).to_owned())
            .collect();
        assert!(passt("application/zip", &typen));
        assert!(passt("application/x-tar", &typen));
        assert!(!passt("text/plain", &typen));
        assert!(!passt("inode/directory", &typen));
        assert!(!passt("", &typen));
        assert!(!passt("application/zip", &[]));
    }
}
