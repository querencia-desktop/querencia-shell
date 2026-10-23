//! Netzwerk (Welle C1): network places in Querencia Dateien after
//! Nautilus 2.30, everything over GVfs (no protocol of its own).
//!
//! - "Netzwerk" (`network:///`) in the side pane and under "Gehe zu"
//!   (`win.netzwerk`): the computers and shares GVfs finds (Avahi/DNS-SD,
//!   WS-Discovery, the Windows network), each device once
//!   ([`netzansicht`]).
//! - Opening a server mounts it through GIO with the program's own
//!   password and question dialogs ([`einhaengen`], [`anmeldung`]):
//!   user name, password, domain, "Kennwort merken" (libsecret through
//!   GVfs, only when ticked), SFTP host key with a readable fingerprint
//!   (QD-008).
//! - "Datei → Mit Server verbinden …" and the side pane's context menu of
//!   "Netzwerk" (`win.mit-server-verbinden`, [`verbinden_dialog`]).
//! - Mounted servers appear in the side pane as volumes with ⏏ (B1's
//!   `seitenleiste.rs` over `querencia_dateiarbeit::laufwerke`).
//! - Failures in plain words ([`gvfs_texte`], adapted from Querencia Files,
//!   MIT).
//!
//! Registered in `main.rs` (`netzwerk::registrieren`); `docs/dateien.md`
//! §7.

mod adresse;
mod anmeldung;
mod einhaengen;
mod gvfs_texte;
mod netzansicht;
mod rahmen;
mod verbinden_dialog;

use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{gio, glib};

use crate::app::Register;
use crate::fenster::Fenster;
use crate::i18n::gettext_f;
use crate::lesezeichen::{self, Marke};

pub const NETZWERK_URI: &str = "network:///";

pub fn registrieren(register: &mut Register) {
    register.fenster(|fenster| {
        aktionen(fenster);
        einhaengen::beobachten(fenster);
    });
    // Welle C3: what hangs on a folder model, per tab
    register.reiter(|fenster, reiter| {
        einhaengen::reiter_beobachten(reiter);
        netzansicht::beobachten(fenster, reiter);
    });
}

fn aktionen(fenster: &Rc<Fenster>) {
    let verbinden = gio::SimpleAction::new("mit-server-verbinden", None);
    let weak = Rc::downgrade(fenster);
    verbinden.connect_activate(move |_, _| {
        if let Some(f) = weak.upgrade() {
            verbinden_dialog::zeigen(&f);
        }
    });
    fenster.window().add_action(&verbinden);

    let netzwerk = gio::SimpleAction::new("netzwerk", None);
    netzwerk.set_enabled(crate::orte::schema_verfuegbar("network"));
    let weak = Rc::downgrade(fenster);
    netzwerk.connect_activate(move |_, _| {
        if let Some(f) = weak.upgrade() {
            f.gehe_zu(&gio::File::for_uri(NETZWERK_URI));
        }
    });
    fenster.window().add_action(&netzwerk);
}

/// The status line while a server of the shown folder is being mounted
/// (instead of GIO's "not mounted" of the folder model).
fn verbindung_melden(fenster: &Rc<Fenster>, ort: &gio::File) {
    let server = adresse::servername(&ort.uri());
    if einhaengen::laeuft(ort) {
        fenster.melden(&gettext_f(
            "Connecting to “{server}” …",
            &[("server", &server)],
        ));
    } else {
        fenster.melden(&gettext_f(
            "Not connected to “{server}”.",
            &[("server", &server)],
        ));
    }
}

fn fehler_zeigen(parent: &gtk4::Window, titel: &str, text: &str) {
    crate::seitenleiste::fehler_zeigen(parent, titel, text);
}

/// Adds a bookmark for `ort` with `name` (empty: no own name) unless it is
/// there; the file is written atomically, the bookmarks of the process
/// follow through their file monitor. Never with a password (the URIs of
/// this module have none).
fn lesezeichen_merken(fenster: &Rc<Fenster>, ort: &gio::File, name: &str) {
    let Some(app) = fenster.app() else { return };
    let uri = ort.uri().to_string();
    let mut liste = app.lesezeichen().liste();
    if liste.iter().any(|m| m.uri == uri) {
        return;
    }
    let name: String = name
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let name = name.trim();
    liste.push(Marke {
        uri,
        name: (!name.is_empty()).then(|| name.to_owned()),
    });
    let pfad = lesezeichen::pfad();
    let ergebnis = (|| {
        if let Some(dir) = pfad.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = pfad.with_extension("querencia-netzwerk");
        std::fs::write(&tmp, lesezeichen::als_text(&liste))?;
        std::fs::rename(&tmp, &pfad)
    })();
    match ergebnis {
        Ok(()) => crate::log("Netzwerk: Lesezeichen angelegt"),
        Err(e) => fenster.melden(&gettext_f(
            "The bookmark could not be saved: {error}",
            &[("error", &e.to_string())],
        )),
    }
}

/// For the isolated GUI test: the GVfs schemes this process can open.
fn schemen() -> Vec<String> {
    gio::Vfs::default()
        .supported_uri_schemes()
        .iter()
        .map(glib::GString::to_string)
        .collect()
}
