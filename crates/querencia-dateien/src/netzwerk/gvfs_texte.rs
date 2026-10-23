// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 LGSE Ltd., Querencia Desktop contributors
//
// Adapted from Querencia Files (Strata fork, branch `querencia` at
// 3dfdb25f): `src/ui/browser/network.rs` (QF-0m readable GVfs failures,
// password prompts, host-key question and the order of its choices;
// QF-0o-2 fingerprint read by the program) and the failure classes of
// `src/ui/browser/location.rs`. MIT License as the whole fork
// (`LICENSES/MIT-Strata.txt` in this crate). Changes: gettext instead of
// rust-i18n, "Querencia Dateien" in the texts, plain functions without
// Strata's `Location`, one function for every failure (title and text are
// built by the caller), the probe itself lives in
// `querencia_dateiarbeit::netz`.

//! GVfs speaks English (or German, with the language pack) and technically.
//! These functions turn its failures, questions and password prompts into
//! short sentences in the language of the program, without jargon, and
//! order the choices of a question so that the safe one comes first.

use gtk4::{gio, glib};
use querencia_dateiarbeit::netz::{self, HostKey};

use crate::i18n::{gettext, gettext_f};

/// What to tell about a failed connection: `None` when nothing is to be
/// said (the person cancelled, or GVfs already showed the problem).
pub fn fehlertext(fehler: &glib::Error) -> Option<String> {
    fehlertext_mit(fehler, None)
}

/// As [`fehlertext`]; `ursache` is the kind of failure the program found
/// itself (name lookup, connection to the port) when GVfs only said
/// "Connection failed".
pub fn fehlertext_mit(fehler: &glib::Error, ursache: Option<gio::IOErrorEnum>) -> Option<String> {
    if fehler.matches(gio::IOErrorEnum::Cancelled)
        || fehler.matches(gio::IOErrorEnum::FailedHandled)
    {
        return None;
    }
    let erklaerung = match ursache {
        Some(art) => erklaerung(&glib::Error::new(art, "")),
        None => erklaerung(fehler),
    };
    let original = fehler.message().trim();
    if original.is_empty() || erklaerung.contains(original) {
        return Some(erklaerung);
    }
    Some(format!(
        "{erklaerung}\n\n{}",
        gettext_f(
            "Message from the system: {message}",
            &[("message", &ohne_steuerzeichen(original))]
        )
    ))
}

/// Whether GVfs said no more than that the connection failed (gvfsd-sftp
/// for a refused port, an unknown name and a broken connection alike).
pub fn allgemein(fehler: &glib::Error) -> bool {
    if !fehler.matches(gio::IOErrorEnum::Failed) {
        return false;
    }
    let nachricht = fehler.message().trim().to_lowercase();
    matches!(
        nachricht.trim_end_matches('.'),
        "connection failed" | "verbindung fehlgeschlagen"
    )
}

/// One readable sentence for a failure, by its kind first, else by the
/// wording of GVfs' message (English or German).
fn erklaerung(fehler: &glib::Error) -> String {
    let nachricht = fehler.message().to_lowercase();
    let hat = |teile: &[&str]| teile.iter().any(|t| nachricht.contains(t));
    if schluessel_abgelehnt(&nachricht) {
        return gettext(
            "The identity of the server could not be confirmed, so no connection was made. \
             Ask the administrator of the server for its key fingerprint before you try again.",
        );
    }
    if fehler.matches(gio::IOErrorEnum::PermissionDenied) || anmeldung_abgelehnt(&nachricht) {
        return gettext(
            "The server did not accept the user name or the password. Check both and try again.",
        );
    }
    if fehler.matches(gio::IOErrorEnum::NotSupported) {
        return gettext(
            "This system cannot open this kind of server address. The package “gvfs-backends” \
             provides the network protocols.",
        );
    }
    if fehler.matches(gio::IOErrorEnum::HostNotFound)
        || hat(&[
            "name or service not known",
            "temporary failure in name resolution",
            "could not resolve",
            "unknown host",
            "no address associated",
            "name oder der dienst ist nicht bekannt",
            "fehler bei der namensauflösung",
            "fehler beim auflösen",
            "gehört keine adresse",
        ])
    {
        gettext("The server could not be found. Check its name or address.")
    } else if fehler.matches(gio::IOErrorEnum::ConnectionRefused)
        || hat(&[
            "connection refused",
            "verbindungsaufbau abgelehnt",
            "vom server verweigert",
        ])
    {
        gettext(
            "The server refused the connection. Check the type of service and the port, and whether the service runs on the server.",
        )
    } else if fehler.matches(gio::IOErrorEnum::TimedOut)
        || hat(&[
            "timed out",
            "timeout",
            "wartezeit für die verbindung ist abgelaufen",
            "zeitüberschreitung",
        ])
    {
        gettext("The server did not answer in time. Check that it is switched on and reachable.")
    } else if fehler.matches(gio::IOErrorEnum::HostUnreachable)
        || fehler.matches(gio::IOErrorEnum::NetworkUnreachable)
        || hat(&[
            "no route to host",
            "network is unreachable",
            "host is unreachable",
            "keine route zum zielrechner",
            "netzwerk ist nicht erreichbar",
            "rechner ist nicht erreichbar",
        ])
    {
        gettext("The server cannot be reached. Check the network connection of this computer.")
    } else if hat(&[
        "failed to retrieve share list",
        "empfangen der freigabenliste vom server ist gescheitert",
    ]) {
        gettext(
            "The server did not provide a list of its shared folders. Enter the name of the share in the dialog “Connect to Server”.",
        )
    } else if fehler.matches(gio::IOErrorEnum::NotFound)
        || hat(&[
            "no such file or directory",
            "bad network name",
            "not found",
            "nicht gefunden",
            "datei oder verzeichnis nicht gefunden",
        ])
    {
        gettext(
            "The shared folder or the folder does not exist on this server. Check the share and the folder.",
        )
    } else if fehler.matches(gio::IOErrorEnum::NotMounted) {
        gettext("The connection to the server was closed. Connect again.")
    } else if matches!(
        nachricht.trim(),
        "connection failed" | "verbindung fehlgeschlagen"
    ) || hat(&[
        "connection reset",
        "connection closed",
        "vom kommunikationspartner zurückgesetzt",
        "von gegenstelle getrennt",
        "unerwartet getrennt",
    ]) {
        gettext(
            "The connection to the server broke off. Check the address, the port and whether the service runs on the server.",
        )
    } else {
        gettext("The connection to the server failed.")
    }
}

fn anmeldung_abgelehnt(nachricht: &str) -> bool {
    [
        "permission denied",
        "authentication failed",
        "authentication required",
        "auth fail",
        "too many authentication failures",
        "logon failure",
        "invalid credentials",
        "zugriff verweigert",
        "keine berechtigung",
        "authentifizierung fehlgeschlagen",
    ]
    .iter()
    .any(|t| nachricht.contains(t))
}

fn schluessel_abgelehnt(nachricht: &str) -> bool {
    [
        "host key",
        "host identification",
        "fingerprint",
        "known_hosts",
        "rechnerschlüssel",
        "identität",
    ]
    .iter()
    .any(|t| nachricht.contains(t))
}

/// GVfs' messages may hold control characters (terminal output of ssh).
fn ohne_steuerzeichen(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() && c != '\n' { ' ' } else { c })
        .collect::<String>()
        .trim()
        .to_owned()
}

/// GVfs' SFTP host-key question: (first line in bold, explanation). The
/// host and the fingerprint come from GVfs' English or German wording;
/// where GVfs could not read the fingerprint ("???", QD-008) `gelesen`
/// supplies the one the program read itself, or a note says it is missing.
/// Other questions (certificates of WebDAV servers, SMB) are shown as GVfs
/// asks them: the first line bold, the rest below.
pub fn frage(nachricht: &str, gelesen: Option<&HostKey>) -> (String, String) {
    if let Some((rechner, identitaet)) = netz::gvfs_question_parts(nachricht) {
        let erst = gettext_f(
            "The identity of the server “{host}” is not known yet.",
            &[("host", &rechner)],
        );
        let fingerabdruck = if identitaet.trim_matches('?').trim().is_empty() {
            fehlender_fingerabdruck(gelesen)
        } else {
            gettext_f(
                "The server sent this key fingerprint:\n{fingerprint}\nCompare it with the \
                 fingerprint you know from the administrator of the server.",
                &[("fingerprint", &identitaet)],
            )
        };
        let zweit = format!(
            "{}\n\n{fingerabdruck}",
            gettext(
                "This is normal when you connect to this server for the first time. If you \
                 connected before, the server may have been replaced, or someone may be \
                 pretending to be the server."
            )
        );
        return (erst, zweit);
    }
    let sauber = ohne_steuerzeichen(nachricht);
    let (erst, rest) = sauber.split_once('\n').unwrap_or((&sauber, ""));
    let mut zweit = rest.trim().to_owned();
    if netz::gvfs_question_lacks_fingerprint(nachricht) {
        if !zweit.is_empty() {
            zweit.push_str("\n\n");
        }
        zweit.push_str(&fehlender_fingerabdruck(gelesen));
    }
    (erst.trim().to_owned(), zweit)
}

fn fehlender_fingerabdruck(gelesen: Option<&HostKey>) -> String {
    match gelesen {
        Some(key) => gettext_f(
            "Querencia Dateien read the key of the server over a separate connection. \
             Fingerprint of the {key_type} key:\n{fingerprint}\nCompare it with the \
             fingerprint you know from the administrator of the server.",
            &[
                ("key_type", &key.key_type),
                ("fingerprint", &key.fingerprint),
            ],
        ),
        None => gettext(
            "The fingerprint of the server key could not be read, so it cannot be compared \
             here. Confirm it with the administrator of the server before you connect.",
        ),
    }
}

/// The password prompts of GVfs 1.60 (gvfsd-sftp, -smb, -ftp, -dav) in
/// English and in the German of the language pack: template with `%s`,
/// what they ask for.
const PASSWORT_VORLAGEN: &[(&str, Frage)] = &[
    ("Enter password for share “%s” on “%s”:", Frage::Freigabe),
    (
        "Bitte geben Sie das Passwort für die Freigabe »%s« auf »%s« ein:",
        Frage::Freigabe,
    ),
    (
        "Enter user and password for share “%s” on “%s”:",
        Frage::FreigabeMitBenutzer,
    ),
    (
        "Bitte geben Sie Benutzer und Passwort für die Freigabe »%s« auf »%s« ein:",
        Frage::FreigabeMitBenutzer,
    ),
    ("Enter user and password for “%s”:", Frage::MitBenutzer),
    (
        "Bitte geben Sie Benutzer und Passwort für »%s« ein:",
        Frage::MitBenutzer,
    ),
    ("Enter password for “%s” on “%s”:", Frage::BenutzerAuf),
    (
        "Bitte geben Sie das Passwort für »%s« auf »%s« ein:",
        Frage::BenutzerAuf,
    ),
    ("Enter password for “%s”:", Frage::Fuer),
    ("Bitte geben Sie das Passwort für »%s« ein:", Frage::Fuer),
    ("Enter proxy password:", Frage::Proxy),
    ("Bitte geben Sie das Proxy-Passwort ein:", Frage::Proxy),
];

#[derive(Clone, Copy)]
enum Frage {
    Freigabe,
    FreigabeMitBenutzer,
    MitBenutzer,
    BenutzerAuf,
    Fuer,
    Proxy,
}

/// The values of `%s` when `text` has the shape of `vorlage`.
fn passt(text: &str, vorlage: &str) -> Option<Vec<String>> {
    let teile: Vec<&str> = vorlage.split("%s").collect();
    let mut rest = text.strip_prefix(teile[0])?;
    let mut werte = Vec::new();
    for (i, teil) in teile.iter().enumerate().skip(1) {
        let ende = if i + 1 == teile.len() && teil.is_empty() {
            rest.len()
        } else {
            rest.find(teil)?
        };
        werte.push(rest[..ende].to_owned());
        rest = &rest[ende + teil.len()..];
    }
    rest.trim().is_empty().then_some(werte)
}

/// A GVfs password prompt in the program's language: the sentence for the
/// bold line, or `None` for a wording that is not known (then the caller
/// shows GVfs' own text). GVfs puts "Authentication Required" in front.
pub fn passwort_text(nachricht: &str) -> Option<String> {
    let zeile = nachricht.lines().map(str::trim).rfind(|z| !z.is_empty())?;
    PASSWORT_VORLAGEN.iter().find_map(|(vorlage, frage)| {
        let w = passt(zeile, vorlage)?;
        let wert = |i: usize| w.get(i).map_or("", String::as_str);
        Some(match frage {
            Frage::Freigabe => gettext_f(
                "Enter the password for the shared folder “{share}” on the server “{host}”.",
                &[("share", wert(0)), ("host", wert(1))],
            ),
            Frage::FreigabeMitBenutzer => gettext_f(
                "Enter user name and password for the shared folder “{share}” on the server \
                 “{host}”.",
                &[("share", wert(0)), ("host", wert(1))],
            ),
            Frage::MitBenutzer => gettext_f(
                "Enter user name and password for “{name}”.",
                &[("name", wert(0))],
            ),
            Frage::BenutzerAuf => gettext_f(
                "Enter the password of “{user}” on the server “{host}”.",
                &[("user", wert(0)), ("host", wert(1))],
            ),
            Frage::Fuer => gettext_f("Enter the password for “{name}”.", &[("name", wert(0))]),
            Frage::Proxy => gettext("Enter the password of the proxy server."),
        })
    })
}

/// GVfs' own text split for the dialog: the first line in bold (without
/// "Authentication Required"), the rest below.
pub fn rohtext(nachricht: &str) -> (String, String) {
    let sauber = ohne_steuerzeichen(nachricht);
    let mut zeilen: Vec<&str> = sauber
        .lines()
        .map(str::trim)
        .filter(|z| !z.is_empty())
        .collect();
    if zeilen.len() > 1 {
        zeilen.remove(0);
    }
    let erst = zeilen.first().copied().unwrap_or_default().to_owned();
    (erst, zeilen.get(1..).unwrap_or_default().join("\n"))
}

/// Marks a msgid for the catalog tool (`werkzeug/i18n.py` knows `N_`);
/// translated when shown.
#[allow(non_snake_case)]
const fn N_(msgid: &'static str) -> &'static str {
    msgid
}

/// Labels GVfs gives the host-key question's choices in English and German,
/// with the English text shown in the program's language.
const WAHL_TEXTE: &[(&str, &str)] = &[
    ("Log In Anyway", N_("Connect _Anyway")),
    ("Cancel Login", N_("_Cancel Connection")),
    ("Dennoch anmelden", N_("Connect _Anyway")),
    ("Anmelden abbrechen", N_("_Cancel Connection")),
];

fn beschriftung(wahl: &str) -> String {
    WAHL_TEXTE
        .iter()
        .find(|(gvfs, _)| *gvfs == wahl)
        .map_or_else(|| wahl.to_owned(), |(_, msgid)| gettext(*msgid))
}

fn lehnt_ab(wahl: &str) -> bool {
    let wahl = wahl.to_lowercase();
    let englisch = [
        "cancel",
        "reject",
        "deny",
        "disconnect",
        "abort",
        "do not",
        "don't",
        "no",
    ]
    .iter()
    .any(|wort| wahl == *wort || wahl.starts_with(&format!("{wort} ")));
    let deutsch = wahl.split_whitespace().any(|wort| {
        matches!(
            wort,
            "abbrechen" | "nein" | "ablehnen" | "trennen" | "nicht"
        )
    });
    englisch || deutsch
}

/// A choice of a GVfs question as the dialog shows it; `index` is the reply.
#[derive(Debug, PartialEq, Eq)]
pub struct Wahl {
    pub index: usize,
    pub text: String,
    pub ablehnen: bool,
}

/// The choices in the order of the dialog: safe ones first. GVfs lists the
/// safe choice last ("Cancel Login", "No", "Cancel"), so when no label is
/// known, as in other languages, that position marks it.
pub fn wahlen(choices: &[String]) -> Vec<Wahl> {
    let erkannt: Vec<bool> = choices.iter().map(|c| lehnt_ab(c)).collect();
    let nach_stelle = choices.len() > 1 && !erkannt.contains(&true);
    let mut liste: Vec<Wahl> = choices
        .iter()
        .zip(erkannt)
        .enumerate()
        .map(|(index, (wahl, erkannt))| Wahl {
            index,
            text: beschriftung(wahl),
            ablehnen: erkannt || (nach_stelle && index + 1 == choices.len()),
        })
        .collect();
    liste.sort_by_key(|w| !w.ablehnen);
    liste
}

#[cfg(test)]
mod tests {
    use super::*;

    const FP: &str = "SHA256:abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQ";

    fn fehler(art: gio::IOErrorEnum, text: &str) -> glib::Error {
        glib::Error::new(art, text)
    }

    #[test]
    fn cancelled_mounts_say_nothing() {
        assert!(fehlertext(&fehler(gio::IOErrorEnum::Cancelled, "x")).is_none());
        assert!(fehlertext(&fehler(gio::IOErrorEnum::FailedHandled, "x")).is_none());
    }

    #[test]
    fn failures_are_explained_by_kind_and_by_gvfs_wording() {
        let t = fehlertext(&fehler(gio::IOErrorEnum::Failed, "Connection refused")).unwrap();
        assert!(t.starts_with("The server refused the connection."), "{t}");
        assert!(
            t.contains("Message from the system: Connection refused"),
            "{t}"
        );
        let t = fehlertext(&fehler(gio::IOErrorEnum::HostNotFound, "x")).unwrap();
        assert!(t.starts_with("The server could not be found."));
        let t = fehlertext(&fehler(
            gio::IOErrorEnum::Failed,
            "Fehler beim Auflösen von »nas-test«: Name oder Dienst nicht bekannt",
        ))
        .unwrap();
        assert!(t.starts_with("The server could not be found."), "{t}");
        let t = fehlertext(&fehler(
            gio::IOErrorEnum::PermissionDenied,
            "Permission denied",
        ))
        .unwrap();
        assert!(t.starts_with("The server did not accept"), "{t}");
        let t = fehlertext(&fehler(gio::IOErrorEnum::Failed, "Login dialog cancelled")).unwrap();
        assert!(t.starts_with("The connection to the server failed."), "{t}");
        let t = fehlertext(&fehler(
            gio::IOErrorEnum::Failed,
            "Host key verification failed",
        ))
        .unwrap();
        assert!(
            t.starts_with("The identity of the server could not be confirmed"),
            "{t}"
        );
        let t = fehlertext(&fehler(gio::IOErrorEnum::NotSupported, "")).unwrap();
        assert!(
            t.contains("gvfs-backends") && !t.contains("Message from"),
            "{t}"
        );
        let t = fehlertext(&fehler(
            gio::IOErrorEnum::Failed,
            "Failed to retrieve share list from server: Connection timed out",
        ))
        .unwrap();
        assert!(t.starts_with("The server did not answer in time."), "{t}");
    }

    #[test]
    fn a_bare_connection_failed_is_explained_by_the_cause_found() {
        let e = fehler(gio::IOErrorEnum::Failed, "Verbindung fehlgeschlagen");
        assert!(allgemein(&e));
        assert!(!allgemein(&fehler(
            gio::IOErrorEnum::Failed,
            "Connection refused"
        )));
        let t = fehlertext_mit(&e, Some(gio::IOErrorEnum::ConnectionRefused)).unwrap();
        assert!(t.starts_with("The server refused the connection."), "{t}");
        assert!(t.contains("Verbindung fehlgeschlagen"), "{t}");
        let t = fehlertext_mit(&e, Some(gio::IOErrorEnum::HostNotFound)).unwrap();
        assert!(t.starts_with("The server could not be found."), "{t}");
        let t = fehlertext_mit(&e, None).unwrap();
        assert!(
            t.starts_with("The connection to the server broke off."),
            "{t}"
        );
    }

    #[test]
    fn control_characters_of_ssh_do_not_reach_the_dialog() {
        let t = fehlertext(&fehler(
            gio::IOErrorEnum::Failed,
            "Connection failed\r\u{1b}[0m",
        ))
        .unwrap();
        assert!(!t.chars().any(|c| c.is_control() && c != '\n'), "{t:?}");
    }

    #[test]
    fn the_host_key_question_shows_the_probed_fingerprint_for_question_marks() {
        let gvfs = "Verifying the identity of “127.0.0.1” failed.\nThe identity sent by the remote computer is “???”. If you want to be absolutely sure it is safe to continue, contact the system administrator.";
        let key = HostKey {
            key_type: "ED25519".into(),
            fingerprint: FP.into(),
        };
        let (erst, zweit) = frage(gvfs, Some(&key));
        assert!(erst.contains("127.0.0.1"));
        assert!(zweit.contains(FP) && zweit.contains("ED25519"), "{zweit}");
        let (_, zweit) = frage(gvfs, None);
        assert!(zweit.contains("could not be read"));
        let deutsch = format!(
            "Die Identität von »srv« konnte nicht überprüft werden.\nDie vom entfernten Rechner übermittelte Identität lautet »{FP}«."
        );
        let (erst, zweit) = frage(&deutsch, None);
        assert!(erst.contains("“srv”"));
        assert!(zweit.contains(FP));
    }

    #[test]
    fn other_questions_keep_gvfs_wording() {
        let (erst, zweit) = frage(
            "The site’s identity can’t be verified:\nThe signing certificate authority is not known.",
            None,
        );
        assert_eq!(erst, "The site’s identity can’t be verified:");
        assert!(zweit.starts_with("The signing certificate"));
    }

    #[test]
    fn password_prompts_of_gvfs_1_60_are_rewritten() {
        assert_eq!(
            passwort_text("Authentication Required\nEnter password for “anna” on “127.0.0.1”:")
                .unwrap(),
            "Enter the password of “anna” on the server “127.0.0.1”."
        );
        assert_eq!(
            passwort_text(
                "Anmeldung erforderlich\nBitte geben Sie das Passwort für »QD-Test« ein:"
            )
            .unwrap(),
            "Enter the password for “QD-Test”."
        );
        assert!(
            passwort_text(
                "Authentication Required\nEnter user and password for share “Daten” on “nas”:"
            )
            .unwrap()
            .contains("“Daten” on the server “nas”")
        );
        assert!(
            passwort_text(
                "Anmeldung erforderlich\nBitte geben Sie das Passwort für die Freigabe »D« auf »n« ein:"
            )
            .unwrap()
            .starts_with("Enter the password for the shared folder “D”")
        );
        assert!(passwort_text("Authentication Required\nEnter proxy password:").is_some());
        assert!(passwort_text("Something else").is_none());
        assert_eq!(
            rohtext("Authentication Required\nSomething else\nmore"),
            ("Something else".to_owned(), "more".to_owned())
        );
    }

    #[test]
    fn the_safe_choice_comes_first() {
        let w = wahlen(&["Log In Anyway".into(), "Cancel Login".into()]);
        assert_eq!(w[0].index, 1);
        assert!(w[0].ablehnen && !w[1].ablehnen);
        assert_eq!(w[0].text, "_Cancel Connection");
        // unknown language: the last one is the safe one (GVfs' order)
        let w = wahlen(&["Ja".into(), "Weiter".into()]);
        assert_eq!(w[0].index, 1);
        assert!(w[0].ablehnen);
        let w = wahlen(&["Dennoch anmelden".into(), "Anmelden abbrechen".into()]);
        assert_eq!(w[0].index, 1);
    }
}
