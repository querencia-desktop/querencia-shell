//! Mounting network places through GIO (`File::mount_enclosing_volume`
//! with [`Anmeldung`]): when a window goes to a server that is not mounted
//! (a bookmark, an address typed, a computer in "Netzwerk", the dialog
//! "Mit Server verbinden …"), the server is mounted and the folder read
//! again, as Nautilus 2.30 did.
//!
//! - One attempt per server in the whole process: a second window or a
//!   second click waits for the first attempt instead of asking for the
//!   password twice.
//! - "Stopp" (Esc, toolbar) cancels; without an open question an attempt
//!   ends after [`ZEITGRENZE`] with a message.
//! - A failure is told once, in a dialog with a readable text
//!   (`gvfs_texte`); a cancel says nothing. Afterwards the window goes back
//!   to the folder it came from.
//! - After an accepted SFTP host key the saved key is compared with the
//!   one shown; a different key disconnects at once.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};
use std::time::Duration;

use gtk4::prelude::*;
use gtk4::{gio, glib};

use super::adresse::{self, NETZSCHEMEN};
use super::anmeldung::{self, Anmeldung};
use super::gvfs_texte;
use crate::fenster::Fenster;
use crate::i18n::gettext_f;
use crate::reiter::Reiter;

/// Without an open question, an attempt ends after this time.
pub const ZEITGRENZE: Duration = Duration::from_secs(60);

/// [`ZEITGRENZE`], or `QUERENCIA_DATEIEN_NETZ_ZEITGRENZE` seconds (tests).
fn zeitgrenze() -> Duration {
    std::env::var("QUERENCIA_DATEIEN_NETZ_ZEITGRENZE")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .filter(|s| *s > 0)
        .map_or(ZEITGRENZE, Duration::from_secs)
}

/// What follows a successful mount.
pub struct Nachher {
    /// Go there in the window (from the dialog); else read the shown
    /// folder again if it is still `ort`.
    pub hingehen: bool,
    /// Add a bookmark with this name (`Some("")`: without a name).
    pub lesezeichen: Option<String>,
}

type Warter = Box<dyn FnOnce(bool)>;

thread_local! {
    /// The running attempts by the server's key, with those waiting.
    static LAUFEND: RefCell<HashMap<String, Vec<Warter>>> = RefCell::default();
    /// Per tab (by its address, Welle C3): the last folder that was read.
    static GELESEN: RefCell<HashMap<usize, (Weak<Reiter>, gio::File)>> = RefCell::default();
}

fn reiter_schluessel(reiter: &Rc<Reiter>) -> usize {
    Rc::as_ptr(reiter) as usize
}

/// Where the shown tab of a window goes after a failed attempt: the last
/// folder it read if that needs no connection (local, or still mounted),
/// else the home folder. Never another server that would ask again.
fn sicherer_ort(fenster: &Fenster) -> gio::File {
    let schluessel = fenster
        .reiterliste()
        .aktiv_wenn()
        .map(|r| reiter_schluessel(&r));
    GELESEN
        .with(|g| schluessel.and_then(|k| g.borrow().get(&k).map(|(_, o)| o.clone())))
        .filter(|o| o.is_native() || o.find_enclosing_mount(gio::Cancellable::NONE).is_ok())
        .unwrap_or_else(|| gio::File::for_path(glib::home_dir()))
}

/// The key of a server: scheme, user, host and port (GIO normalises URIs,
/// G2-N trap 6, so compare parts, not text).
pub fn server_schluessel(ort: &gio::File) -> String {
    let uri = ort.uri();
    match glib::Uri::parse(&uri, glib::UriFlags::NONE) {
        Ok(u) => format!(
            "{}://{}@{}:{}",
            u.scheme().to_ascii_lowercase(),
            u.user().unwrap_or_default(),
            u.host().unwrap_or_default().to_lowercase(),
            u.port()
        ),
        Err(_) => uri.to_string(),
    }
}

/// Whether `ort` is on a network protocol GVfs mounts on demand (not the
/// automounted `network:///`, `dns-sd://`, `trash:///`, `computer:///`).
pub fn netzort(ort: &gio::File) -> bool {
    !ort.is_native()
        && ort
            .uri_scheme()
            .is_some_and(|s| NETZSCHEMEN.contains(&s.to_ascii_lowercase().as_str()))
}

/// Whether the server of `ort` is being mounted right now.
pub fn laeuft(ort: &gio::File) -> bool {
    let schluessel = server_schluessel(ort);
    LAUFEND.with(|l| l.borrow().contains_key(&schluessel))
}

/// Each tab remembers the last folder it read (Welle C3: per tab).
pub fn reiter_beobachten(reiter: &Rc<Reiter>) {
    let schluessel = reiter_schluessel(reiter);
    let weak = Rc::downgrade(reiter);
    reiter.modell().bei_zustand(move |z| {
        if let (crate::modell::Zustand::Geladen(_), Some(r)) = (z, weak.upgrade())
            && let Some(ort) = r.ort()
        {
            GELESEN.with(|g| {
                let mut g = g.borrow_mut();
                // closed tabs leave the map
                g.retain(|_, (w, _)| w.strong_count() > 0);
                g.insert(schluessel, (Rc::downgrade(&r), ort));
            });
        }
    });
}

/// Every change of folder: a network place that is not mounted is
/// mounted first.
pub fn beobachten(fenster: &Rc<Fenster>) {
    let weak = Rc::downgrade(fenster);
    fenster.bei_ortswechsel(move |ort| {
        if let Some(f) = weak.upgrade() {
            bei_bedarf(&f, ort);
        }
    });
    // a window opened on a server (`querencia-dateien sftp://…`, "In neuem
    // Fenster öffnen") read its first folder before this hook ran
    if let Some(ort) = fenster.ort() {
        bei_bedarf(fenster, &ort);
    }
}

/// Mounts the server of `ort` if it is a network place that is not mounted.
fn bei_bedarf(fenster: &Rc<Fenster>, ort: &gio::File) {
    // GVfs answers from its mount tracker (no network traffic)
    if !netzort(ort) || ort.find_enclosing_mount(gio::Cancellable::NONE).is_ok() {
        return;
    }
    starten(
        fenster,
        ort,
        Nachher {
            hingehen: false,
            lesezeichen: None,
        },
    );
}

/// Mounts the server of `ort` for `fenster`, then does `nachher`.
pub fn starten(fenster: &Rc<Fenster>, ort: &gio::File, nachher: Nachher) {
    let schluessel = server_schluessel(ort);
    let weak = Rc::downgrade(fenster);
    let ziel = ort.clone();
    let warten: Warter = Box::new(move |ok| {
        if let Some(f) = weak.upgrade() {
            nach_einhaengen(&f, &ziel, ok, &nachher);
        }
    });
    let schon = LAUFEND.with(|l| {
        let mut l = l.borrow_mut();
        match l.get_mut(&schluessel) {
            Some(liste) => {
                liste.push(warten);
                None
            }
            None => {
                l.insert(schluessel.clone(), Vec::new());
                Some(warten)
            }
        }
    });
    let Some(warten) = schon else {
        crate::log(&format!(
            "Netzwerk: wartet auf laufenden Versuch für {schluessel}"
        ));
        return;
    };
    versuch(fenster, ort, schluessel, warten);
}

fn versuch(fenster: &Rc<Fenster>, ort: &gio::File, schluessel: String, eigen: Warter) {
    let uri = ort.uri().to_string();
    let server = adresse::servername(&uri);
    let window: gtk4::Window = fenster.window().clone().upcast();
    let anmeldung = Anmeldung::neu(&window, &uri);
    let abbruch = gio::Cancellable::new();
    let zeit_um = Rc::new(Cell::new(false));
    fenster.melden(&gettext_f(
        "Connecting to “{server}” …",
        &[("server", &server)],
    ));
    crate::log(&format!("Netzwerk: einhängen {}", ohne_benutzer(&uri)));
    let beginn = std::time::Instant::now();

    // time limit, paused while a question is open
    let uhr: Rc<RefCell<Option<glib::SourceId>>> = Rc::default();
    let uhr_stellen = {
        let uhr = uhr.clone();
        let abbruch = abbruch.clone();
        let zeit_um = zeit_um.clone();
        Rc::new(move |laufen: bool| {
            if let Some(id) = uhr.borrow_mut().take() {
                id.remove();
            }
            if laufen {
                let abbruch = abbruch.clone();
                let zeit_um = zeit_um.clone();
                let uhr2 = uhr.clone();
                let id = glib::timeout_add_local_once(zeitgrenze(), move || {
                    uhr2.borrow_mut().take();
                    zeit_um.set(true);
                    crate::log("Netzwerk: Zeitgrenze erreicht");
                    abbruch.cancel();
                });
                uhr.replace(Some(id));
            }
        })
    };
    {
        let uhr_stellen = uhr_stellen.clone();
        anmeldung.bei_dialog(move |offen| uhr_stellen(!offen));
    }
    uhr_stellen(true);

    // "Stopp" cancels the attempt
    let stopp = fenster
        .window()
        .lookup_action("stopp")
        .and_downcast::<gio::SimpleAction>();
    let stopp_id = stopp.as_ref().map(|a| {
        a.set_enabled(true);
        let abbruch = abbruch.clone();
        a.connect_activate(move |_, _| {
            crate::log("Netzwerk: Stopp");
            abbruch.cancel();
        })
    });

    let fw = Rc::downgrade(fenster);
    let a2 = anmeldung.clone();
    ort.mount_enclosing_volume(
        gio::MountMountFlags::NONE,
        Some(anmeldung.operation()),
        Some(&abbruch),
        move |ergebnis| {
            uhr_stellen(false);
            a2.beenden();
            if let (Some(a), Some(id)) = (stopp, stopp_id) {
                a.disconnect(id);
                if let Some(f) = fw.upgrade() {
                    a.set_enabled(f.modell().laedt());
                }
            }
            let ort = gio::File::for_uri(&uri);
            glib::spawn_future_local(async move {
                let ergebnis = auswerten(ergebnis, &a2, &ort, zeit_um.get()).await;
                let ok = ergebnis.is_ok();
                if let Err(Some(text)) = &ergebnis
                    && let Some(f) = fw.upgrade()
                {
                    crate::log(&format!(
                        "Netzwerk: Meldung: {}",
                        text.lines().next().unwrap_or_default()
                    ));
                    super::fehler_zeigen(
                        f.window().upcast_ref(),
                        &gettext_f("Could not connect to “{server}”.", &[("server", &server)]),
                        text,
                    );
                }
                crate::log(&format!(
                    "Netzwerk: einhängen {} ({} ms)",
                    match &ergebnis {
                        Ok(()) => "fertig",
                        Err(None) => "abgebrochen",
                        Err(Some(_)) => "gescheitert",
                    },
                    beginn.elapsed().as_millis()
                ));
                let andere =
                    LAUFEND.with(|l| l.borrow_mut().remove(&schluessel).unwrap_or_default());
                eigen(ok);
                for w in andere {
                    w(ok);
                }
            });
        },
    );
}

/// The result of an attempt: `Ok`, or `Err` with the text to show (`None`:
/// cancelled, say nothing).
async fn auswerten(
    ergebnis: Result<(), glib::Error>,
    anmeldung: &Anmeldung,
    ort: &gio::File,
    zeit_um: bool,
) -> Result<(), Option<String>> {
    if let Some(fehler) = anmeldung.falscher_schluessel() {
        return Err(Some(fehler.message().to_owned()));
    }
    match ergebnis {
        Ok(()) => {}
        Err(e) if e.matches(gio::IOErrorEnum::AlreadyMounted) => return Ok(()),
        Err(e) if zeit_um && e.matches(gio::IOErrorEnum::Cancelled) => {
            return Err(Some(gettext_f(
                "The server did not answer within {seconds} seconds. Check that it is switched \
                 on and reachable, then try again.",
                &[("seconds", &zeitgrenze().as_secs().to_string())],
            )));
        }
        Err(e) if anmeldung.abgelehnt() => {
            crate::log(&format!("Netzwerk: abgelehnt ({})", e.message()));
            return Err(None);
        }
        Err(e) if gvfs_texte::allgemein(&e) && anmeldung.kennwort_gesendet() => {
            // gvfsd-sftp after the last wrong password: the login was refused
            return Err(gvfs_texte::fehlertext_mit(
                &e,
                Some(gio::IOErrorEnum::PermissionDenied),
            ));
        }
        Err(e) if gvfs_texte::allgemein(&e) => {
            // GVfs' sftp backend only says "Connection failed": find the
            // cause without logging in
            let beginn = std::time::Instant::now();
            let ursache = ursache_ermitteln(&ort.uri()).await;
            crate::log(&format!(
                "Netzwerk: Ursache ermittelt: {ursache:?} ({} ms)",
                beginn.elapsed().as_millis()
            ));
            return Err(gvfs_texte::fehlertext_mit(&e, ursache));
        }
        Err(e) => return Err(gvfs_texte::fehlertext(&e)),
    }
    // the key ssh saved must be the one the person confirmed
    if let Some(gezeigt) = anmeldung.akzeptiert() {
        let pruefung = anmeldung::known_hosts_pruefen(&ort.uri(), &gezeigt).await;
        if let Some(fehler) = anmeldung::schluessel_fehler(&pruefung, &gezeigt) {
            if let Ok(mount) = ort.find_enclosing_mount(gio::Cancellable::NONE) {
                let _ = mount
                    .unmount_with_operation_future(
                        gio::MountUnmountFlags::FORCE,
                        gio::MountOperation::NONE,
                    )
                    .await;
            }
            return Err(Some(fehler.message().to_owned()));
        }
    }
    Ok(())
}

/// The default port of a network protocol (for the diagnosis).
fn standardport(schema: &str) -> u16 {
    match schema {
        "sftp" | "ssh" => 22,
        "ftp" | "ftpis" => 21,
        "ftps" => 990,
        "dav" => 80,
        "davs" => 443,
        "smb" => 445,
        "afp" => 548,
        "nfs" => 2049,
        _ => 0,
    }
}

/// Why a server cannot be reached: the name is not known, the port refuses,
/// nobody answers. `None` when a connection to the port works (then the
/// service itself refused, the generic text stays). No data is sent.
async fn ursache_ermitteln(uri: &str) -> Option<gio::IOErrorEnum> {
    let u = glib::Uri::parse(uri, glib::UriFlags::NONE).ok()?;
    let host = u.host()?.to_string();
    let port = u16::try_from(u.port())
        .ok()
        .filter(|p| *p > 0)
        .unwrap_or_else(|| standardport(&u.scheme().to_ascii_lowercase()));
    if port == 0 {
        return None;
    }
    let host = host
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_owned();
    if gio::Resolver::default()
        .lookup_by_name_future(&host)
        .await
        .is_err()
    {
        return Some(gio::IOErrorEnum::HostNotFound);
    }
    let client = gio::SocketClient::new();
    client.set_timeout(5);
    match client.connect_to_host_future(&host, port).await {
        Ok(verbindung) => {
            let _ = verbindung.close_future(glib::Priority::DEFAULT).await;
            None
        }
        Err(e) => e
            .kind::<gio::IOErrorEnum>()
            .or(Some(gio::IOErrorEnum::Failed)),
    }
}

fn nach_einhaengen(fenster: &Rc<Fenster>, ort: &gio::File, ok: bool, nachher: &Nachher) {
    let hier = fenster.ort().is_some_and(|o| o.equal(ort));
    if !ok {
        // back to a folder that needs no connection, as Nautilus 2.30
        // stayed where it was after a cancelled password
        if hier {
            fenster.gehe_zu(&sicherer_ort(fenster));
        } else {
            fenster.melden(&gettext_f(
                "Not connected to “{server}”.",
                &[("server", &adresse::servername(&ort.uri()))],
            ));
        }
        return;
    }
    if let Some(name) = &nachher.lesezeichen {
        super::lesezeichen_merken(fenster, ort, name);
    }
    if nachher.hingehen && !hier {
        fenster.gehe_zu(ort);
    } else if hier {
        fenster.modell().neu_laden();
    }
}

/// The URI without the user part, for the log.
fn ohne_benutzer(uri: &str) -> String {
    match glib::Uri::parse(uri, glib::UriFlags::NONE) {
        Ok(u) => format!(
            "{}://{}{}",
            u.scheme(),
            u.host().unwrap_or_default(),
            if u.port() > 0 {
                format!(":{}", u.port())
            } else {
                String::new()
            }
        ),
        Err(_) => "?".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_key_per_server_whatever_the_path() {
        let a = gio::File::for_uri("sftp://anna@Srv.Example:2222/a/b");
        let b = gio::File::for_uri("sftp://anna@srv.example:2222/c");
        let c = gio::File::for_uri("sftp://bert@srv.example:2222/c");
        assert_eq!(server_schluessel(&a), server_schluessel(&b));
        assert_ne!(server_schluessel(&b), server_schluessel(&c));
    }

    #[test]
    fn only_network_protocols_are_mounted_on_demand() {
        assert!(netzort(&gio::File::for_uri("sftp://h/x")));
        assert!(netzort(&gio::File::for_uri("smb://h/s")));
        assert!(!netzort(&gio::File::for_uri("network:///")));
        assert!(!netzort(&gio::File::for_uri("trash:///")));
        assert!(!netzort(&gio::File::for_path("/tmp")));
    }

    #[test]
    fn the_log_has_no_user() {
        assert_eq!(ohne_benutzer("sftp://anna@h:2222/x"), "sftp://h:2222");
        assert_eq!(ohne_benutzer("smb://BUERO;anna@nas/D"), "smb://nas");
    }
}
