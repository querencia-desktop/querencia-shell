//! The questions while connecting to a server (`gtk4::MountOperation`
//! with the program's own dialogs instead of GTK's):
//!
//! - **Password** (`ask-password`): user name, password and domain as GVfs
//!   asks for them, "Anonym verbinden" where the server allows it, and
//!   "Kennwort merken" only where GVfs can store it. Off by default: GVfs
//!   saves the password in the session's keyring (libsecret) only when the
//!   person ticks it (`PasswordSave::Permanently`), otherwise never.
//!   A second question for the same connection says that the login failed.
//! - **Question** (`ask-question`): the SFTP host key with a readable
//!   fingerprint. GVfs 1.60 sends "???" with OpenSSH 10 (QD-008), so the
//!   program reads the key itself (`querencia_dateiarbeit::netz`, QF-0o-2)
//!   before it shows the question. The safe choice comes first and has the
//!   focus; "Trotzdem verbinden" is marked as dangerous.
//! - After the key was accepted, the key ssh saved in known_hosts is
//!   compared with the one shown, before the password is sent and once
//!   more after the connection (the probe was a separate connection).
//!
//! Double questions (QD-036's sibling in the dialogs): a new question of
//! the same connection closes the previous one, a late probe result for a
//! question that is no longer pending is dropped (generation counter), and
//! GVfs' `aborted` closes what is open.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::time::Duration;

use gtk4::prelude::*;
use gtk4::{gio, glib};
use querencia_dateiarbeit::netz::{self, HostKey, KnownHostsCheck};

use super::{gvfs_texte, rahmen};
use crate::i18n::{gettext, gettext_f};

/// The questions of one connection attempt.
#[derive(Clone)]
pub struct Anmeldung(Rc<Inner>);

struct Inner {
    op: gtk4::MountOperation,
    uri: String,
    sftp: bool,
    parent: glib::WeakRef<gtk4::Window>,
    dialog: RefCell<Option<gtk4::Window>>,
    /// Raised per question and at the end: late probes are dropped.
    generation: Cell<u64>,
    passwort_runden: Cell<u32>,
    /// The fingerprint shown when the person chose "Trotzdem verbinden".
    akzeptiert: RefCell<Option<String>>,
    known_hosts_vorher: Cell<bool>,
    abgelehnt: Cell<bool>,
    falscher_schluessel: RefCell<Option<glib::Error>>,
    /// Runs when a dialog opens or closes (the caller's time limit waits
    /// while the person decides).
    bei_dialog: RefCell<Option<DialogHaken>>,
}

/// Told `true` when a dialog opens, `false` when it closes.
type DialogHaken = Box<dyn Fn(bool)>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, glib::ErrorDomain)]
#[error_domain(name = "querencia-dateien-known-hosts")]
pub enum SchluesselFehler {
    Anders,
}

impl Anmeldung {
    pub fn neu(parent: &gtk4::Window, uri: &str) -> Self {
        let op = gtk4::MountOperation::new(Some(parent));
        let sftp = matches!(glib::Uri::peek_scheme(uri).as_deref(), Some("sftp" | "ssh"));
        let a = Self(Rc::new(Inner {
            op,
            uri: uri.to_owned(),
            sftp,
            parent: parent.downgrade(),
            dialog: RefCell::default(),
            generation: Cell::new(0),
            passwort_runden: Cell::new(0),
            akzeptiert: RefCell::default(),
            known_hosts_vorher: Cell::new(false),
            abgelehnt: Cell::new(false),
            falscher_schluessel: RefCell::default(),
            bei_dialog: RefCell::default(),
        }));
        a.signale();
        a
    }

    pub fn operation(&self) -> &gtk4::MountOperation {
        &self.0.op
    }

    pub fn bei_dialog(&self, f: impl Fn(bool) + 'static) {
        self.0.bei_dialog.replace(Some(Box::new(f)));
    }

    /// The person declined a question or cancelled the password.
    pub fn abgelehnt(&self) -> bool {
        self.0.abgelehnt.get()
    }

    /// Whether a password was sent in this attempt (then a bare
    /// "Connection failed" of gvfsd-sftp means the login was refused).
    pub fn kennwort_gesendet(&self) -> bool {
        self.0.passwort_runden.get() > 0
    }

    /// The fingerprint the person accepted, if any.
    pub fn akzeptiert(&self) -> Option<String> {
        self.0.akzeptiert.borrow().clone()
    }

    /// The failure of a known_hosts check before the password, if any.
    pub fn falscher_schluessel(&self) -> Option<glib::Error> {
        self.0.falscher_schluessel.borrow_mut().take()
    }

    /// The attempt is over: close what is open, drop late probes.
    pub fn beenden(&self) {
        self.0.generation.set(self.0.generation.get() + 1);
        self.0.schliessen();
    }

    fn signale(&self) {
        let weak = Rc::downgrade(&self.0);
        self.0
            .op
            .connect_ask_password(move |op, nachricht, benutzer, domaene, flags| {
                op.stop_signal_emission_by_name("ask-password");
                let Some(inner) = weak.upgrade() else {
                    op.reply(gio::MountOperationResult::Aborted);
                    return;
                };
                inner.passwort(nachricht, benutzer, domaene, flags);
            });
        let weak = Rc::downgrade(&self.0);
        self.0
            .op
            .connect_local("ask-question", false, move |werte| {
                let op = werte.first()?.get::<gtk4::MountOperation>().ok()?;
                op.stop_signal_emission_by_name("ask-question");
                let nachricht = werte.get(1)?.get::<String>().ok()?;
                let wahlen: Vec<String> = werte
                    .get(2)?
                    .get::<glib::StrV>()
                    .ok()?
                    .iter()
                    .map(ToString::to_string)
                    .collect();
                match weak.upgrade() {
                    Some(inner) if !wahlen.is_empty() => inner.frage(nachricht, wahlen),
                    _ => op.reply(gio::MountOperationResult::Unhandled),
                }
                None
            });
        let weak = Rc::downgrade(&self.0);
        self.0.op.connect_aborted(move |_| {
            if let Some(inner) = weak.upgrade() {
                crate::log("Netzwerk: Anmeldung von GVfs abgebrochen");
                inner.generation.set(inner.generation.get() + 1);
                inner.schliessen();
            }
        });
    }
}

impl Inner {
    fn schliessen(&self) {
        let alt = self.dialog.borrow_mut().take();
        if let Some(w) = alt {
            w.destroy();
            self.dialog_gemeldet(false);
        }
    }

    fn dialog_gemeldet(&self, offen: bool) {
        if let Some(f) = self.bei_dialog.borrow().as_ref() {
            f(offen);
        }
    }

    fn zeigen(&self, window: &gtk4::Window) {
        self.schliessen();
        self.dialog.replace(Some(window.clone()));
        self.dialog_gemeldet(true);
        window.present();
    }

    /// Closes `window` if it is still the open dialog.
    fn fertig(&self, window: &gtk4::Window) {
        let ist = self.dialog.borrow().as_ref() == Some(window);
        if ist {
            self.dialog.replace(None);
            self.dialog_gemeldet(false);
        }
        window.destroy();
    }

    // ------------------------------------------------------------ password

    fn passwort(
        self: Rc<Self>,
        nachricht: &str,
        benutzer: &str,
        domaene: &str,
        flags: gio::AskPasswordFlags,
    ) {
        let akzeptiert = self.akzeptiert.borrow().clone();
        if let (true, Some(gezeigt), false) = (self.sftp, akzeptiert, self.known_hosts_vorher.get())
        {
            // ssh saved the accepted key before asking for the password:
            // compare it first, a different key never gets the password
            self.known_hosts_vorher.set(true);
            let (nachricht, benutzer, domaene) = (
                nachricht.to_owned(),
                benutzer.to_owned(),
                domaene.to_owned(),
            );
            let generation = self.generation.get();
            glib::spawn_future_local(async move {
                let pruefung = known_hosts_pruefen(&self.uri, &gezeigt).await;
                if self.generation.get() != generation {
                    return;
                }
                if let Some(fehler) = schluessel_fehler(&pruefung, &gezeigt) {
                    self.falscher_schluessel.replace(Some(fehler));
                    self.op.reply(gio::MountOperationResult::Aborted);
                    return;
                }
                self.passwort_zeigen(&nachricht, &benutzer, &domaene, flags);
            });
            return;
        }
        self.passwort_zeigen(nachricht, benutzer, domaene, flags);
    }

    fn passwort_zeigen(
        self: &Rc<Self>,
        nachricht: &str,
        benutzer: &str,
        domaene: &str,
        flags: gio::AskPasswordFlags,
    ) {
        let runde = self.passwort_runden.get();
        self.passwort_runden.set(runde + 1);
        crate::log(&format!(
            "Netzwerk: Kennwortfrage {} ({:?})",
            runde + 1,
            flags
        ));
        let (erst, zweit) = match gvfs_texte::passwort_text(nachricht) {
            Some(text) => (text, String::new()),
            None => gvfs_texte::rohtext(nachricht),
        };
        let mut hinweise: Vec<String> = Vec::new();
        if !zweit.is_empty() {
            hinweise.push(zweit);
        }
        // the user of the address, when GVfs does not ask for it
        let adressbenutzer = glib::Uri::parse(&self.uri, glib::UriFlags::NONE)
            .ok()
            .and_then(|u| u.user())
            .map(|u| u.split(';').next_back().unwrap_or_default().to_owned())
            .filter(|u| !u.is_empty());
        if let Some(b) = adressbenutzer
            .as_ref()
            .filter(|_| !flags.contains(gio::AskPasswordFlags::NEED_USERNAME))
        {
            hinweise.push(gettext_f("You log in as “{user}”.", &[("user", b)]));
        }
        if runde > 0 {
            hinweise.push(gettext(
                "The login failed. Check the user name and the password.",
            ));
        }
        let zweit = hinweise.join("\n");
        let parent = self.parent.upgrade();
        let r = rahmen::neu(
            parent.as_ref(),
            &gettext("Password Required"),
            "dialog-password",
            &erst,
            &zweit,
        );
        let window = r.window.clone();

        let anonym_moeglich = flags.contains(gio::AskPasswordFlags::ANONYMOUS_SUPPORTED);
        let anonym = gtk4::CheckButton::with_mnemonic(&gettext("Connect _anonymously"));
        let als_benutzer = gtk4::CheckButton::with_mnemonic(&gettext("Connect as u_ser:"));
        als_benutzer.set_group(Some(&anonym));
        als_benutzer.set_active(true);
        if anonym_moeglich {
            r.inhalt.append(&anonym);
            r.inhalt.append(&als_benutzer);
        }
        let gitter = gtk4::Grid::builder()
            .row_spacing(6)
            .column_spacing(12)
            .build();
        if anonym_moeglich {
            gitter.set_margin_start(24);
        }
        let mut reihe = 0;
        let benutzer_feld = gtk4::Entry::new();
        let vorgabe = if benutzer.is_empty() {
            adressbenutzer
                .clone()
                .unwrap_or_else(|| glib::user_name().to_string_lossy().into_owned())
        } else {
            benutzer.to_owned()
        };
        benutzer_feld.set_text(&vorgabe);
        benutzer_feld.set_activates_default(true);
        if flags.contains(gio::AskPasswordFlags::NEED_USERNAME) {
            rahmen::zeile(&gitter, reihe, &gettext("_User name:"), &benutzer_feld);
            reihe += 1;
        }
        let domaene_feld = gtk4::Entry::new();
        domaene_feld.set_text(domaene);
        domaene_feld.set_activates_default(true);
        if flags.contains(gio::AskPasswordFlags::NEED_DOMAIN) {
            rahmen::zeile(&gitter, reihe, &gettext("_Domain:"), &domaene_feld);
            reihe += 1;
        }
        let kennwort_feld = gtk4::PasswordEntry::new();
        kennwort_feld.set_show_peek_icon(true);
        kennwort_feld.set_activates_default(true);
        if flags.contains(gio::AskPasswordFlags::NEED_PASSWORD) {
            rahmen::zeile(&gitter, reihe, &gettext("_Password:"), &kennwort_feld);
        }
        r.inhalt.append(&gitter);
        let merken = gtk4::CheckButton::with_mnemonic(&gettext("_Remember password"));
        merken.set_active(false);
        if flags.contains(gio::AskPasswordFlags::SAVING_SUPPORTED) {
            merken.set_tooltip_text(Some(&gettext(
                "The password is kept in the keyring of your session and used for this server \
                 next time.",
            )));
            r.inhalt.append(&merken);
        }
        {
            let gitter = gitter.clone();
            anonym.connect_toggled(move |a| gitter.set_sensitive(!a.is_active()));
        }

        let antwort: Rc<Cell<bool>> = Rc::default();
        let abbrechen = gtk4::Button::with_mnemonic(&gettext("_Cancel"));
        let verbinden = gtk4::Button::with_mnemonic(&gettext("C_onnect"));
        verbinden.add_css_class("suggested-action");
        r.knoepfe.append(&abbrechen);
        r.knoepfe.append(&verbinden);
        window.set_default_widget(Some(&verbinden));

        let weg = {
            let weak: Weak<Inner> = Rc::downgrade(self);
            let antwort = antwort.clone();
            let window = window.clone();
            Rc::new(
                move |ergebnis: Option<(bool, String, String, String, bool)>| {
                    if antwort.replace(true) {
                        return;
                    }
                    let Some(inner) = weak.upgrade() else { return };
                    match ergebnis {
                        Some((anonym, benutzer, domaene, kennwort, merken)) => {
                            let op = &inner.op;
                            op.set_anonymous(anonym);
                            if !anonym {
                                op.set_username(Some(&benutzer));
                                op.set_domain(Some(&domaene));
                                op.set_password(Some(&kennwort));
                            }
                            op.set_password_save(if merken {
                                gio::PasswordSave::Permanently
                            } else {
                                gio::PasswordSave::Never
                            });
                            crate::log(&format!(
                                "Netzwerk: Kennwort gesendet (anonym {anonym}, merken {merken})"
                            ));
                            op.reply(gio::MountOperationResult::Handled);
                        }
                        None => {
                            crate::log("Netzwerk: Kennwortfrage abgebrochen");
                            // some backends (dav) answer a cancel with "permission denied":
                            // the person cancelled, nothing to report
                            inner.abgelehnt.set(true);
                            inner.op.reply(gio::MountOperationResult::Aborted);
                        }
                    }
                    inner.fertig(&window);
                },
            )
        };
        {
            let weg = weg.clone();
            abbrechen.connect_clicked(move |_| weg(None));
        }
        {
            let weg = weg.clone();
            let (anonym, benutzer_feld, domaene_feld, kennwort_feld, merken) = (
                anonym.clone(),
                benutzer_feld.clone(),
                domaene_feld.clone(),
                kennwort_feld.clone(),
                merken.clone(),
            );
            let merken_moeglich = flags.contains(gio::AskPasswordFlags::SAVING_SUPPORTED);
            verbinden.connect_clicked(move |_| {
                weg(Some((
                    anonym_moeglich && anonym.is_active(),
                    benutzer_feld.text().to_string(),
                    domaene_feld.text().to_string(),
                    kennwort_feld.text().to_string(),
                    merken_moeglich && merken.is_active(),
                )));
            });
        }
        {
            let weg = weg.clone();
            rahmen::bei_abbruch(&window, move || weg(None));
        }
        self.zeigen(&window);
        if flags.contains(gio::AskPasswordFlags::NEED_USERNAME) && benutzer_feld.text().is_empty() {
            benutzer_feld.grab_focus();
        } else if flags.contains(gio::AskPasswordFlags::NEED_PASSWORD) {
            kennwort_feld.grab_focus();
        }
    }

    // ------------------------------------------------------------ question

    fn frage(self: Rc<Self>, nachricht: String, wahlen: Vec<String>) {
        let generation = self.generation.get() + 1;
        self.generation.set(generation);
        // a new question closes the previous one (no double dialogs)
        self.schliessen();
        crate::log(&format!("Netzwerk: Frage mit {} Antworten", wahlen.len()));
        let sonde = (self.sftp && netz::gvfs_question_lacks_fingerprint(&nachricht))
            .then(|| netz::host_key_probe_argv(&self.uri))
            .flatten();
        let Some(argv) = sonde else {
            self.frage_zeigen(&nachricht, None, &wahlen);
            return;
        };
        glib::spawn_future_local(async move {
            let beginn = std::time::Instant::now();
            let gelesen = gio::spawn_blocking(move || netz::probe_host_key(&argv))
                .await
                .ok()
                .flatten();
            crate::log(&format!(
                "Netzwerk: Fingerabdruck selbst gelesen: {} ({} ms)",
                gelesen.as_ref().map_or("nein", |_| "ja"),
                beginn.elapsed().as_millis()
            ));
            if self.generation.get() == generation {
                self.frage_zeigen(&nachricht, gelesen.as_ref(), &wahlen);
            }
        });
    }

    fn frage_zeigen(
        self: &Rc<Self>,
        nachricht: &str,
        gelesen: Option<&HostKey>,
        wahlen: &[String],
    ) {
        let schluessel = netz::gvfs_question_parts(nachricht).is_some()
            || netz::gvfs_question_lacks_fingerprint(nachricht);
        let (erst, zweit) = gvfs_texte::frage(nachricht, gelesen);
        let parent = self.parent.upgrade();
        let (titel, symbol) = if schluessel {
            (gettext("Verify Server Identity"), "dialog-warning")
        } else {
            (gettext("Connect to Server"), "dialog-question")
        };
        let r = rahmen::neu(parent.as_ref(), &titel, symbol, &erst, &zweit);
        let window = r.window.clone();
        let gezeigt = netz::shown_fingerprint(nachricht, gelesen);
        if let Some(fp) = &gezeigt {
            // a fingerprint is no secret; the isolated test compares it
            crate::log(&format!("Netzwerk: Frage zeigt Fingerabdruck {fp}"));
        }
        let antwort: Rc<Cell<bool>> = Rc::default();
        let antworten = {
            let weak: Weak<Inner> = Rc::downgrade(self);
            let antwort = antwort.clone();
            let window = window.clone();
            Rc::new(move |wahl: Option<(usize, bool)>| {
                if antwort.replace(true) {
                    return;
                }
                let Some(inner) = weak.upgrade() else { return };
                match wahl {
                    Some((index, ablehnen)) => {
                        if ablehnen {
                            inner.abgelehnt.set(true);
                        } else if schluessel {
                            inner.akzeptiert.replace(gezeigt.clone());
                        }
                        crate::log(&format!(
                            "Netzwerk: Antwort {index} ({})",
                            if ablehnen { "ablehnen" } else { "annehmen" }
                        ));
                        inner.op.set_choice(i32::try_from(index).unwrap_or(0));
                        inner.op.reply(gio::MountOperationResult::Handled);
                    }
                    None => {
                        inner.abgelehnt.set(true);
                        crate::log("Netzwerk: Frage geschlossen");
                        inner.op.reply(gio::MountOperationResult::Aborted);
                    }
                }
                inner.fertig(&window);
            })
        };
        let mut erster: Option<gtk4::Button> = None;
        for w in gvfs_texte::wahlen(wahlen) {
            let knopf = gtk4::Button::with_mnemonic(&w.text);
            if !w.ablehnen && schluessel {
                knopf.add_css_class("destructive-action");
            }
            let antworten = antworten.clone();
            let (index, ablehnen) = (w.index, w.ablehnen);
            knopf.connect_clicked(move |_| antworten(Some((index, ablehnen))));
            r.knoepfe.append(&knopf);
            erster.get_or_insert(knopf);
        }
        {
            let antworten = antworten.clone();
            rahmen::bei_abbruch(&window, move || antworten(None));
        }
        self.zeigen(&window);
        if let Some(k) = erster {
            window.set_default_widget(Some(&k));
            k.grab_focus();
        }
    }
}

/// After "Trotzdem verbinden": does the entry ssh saved match the shown
/// fingerprint? Runs `ssh -G` and `ssh-keygen -F` off the main thread.
pub async fn known_hosts_pruefen(uri: &str, gezeigt: &str) -> KnownHostsCheck {
    let (uri, gezeigt) = (uri.to_owned(), gezeigt.to_owned());
    gio::spawn_blocking(move || {
        let grenze = Duration::from_secs(5);
        let Some(argv) = netz::ssh_config_argv(&uri) else {
            return KnownHostsCheck::NotSaved;
        };
        let Some(ziel) = netz::command_output(&argv, grenze, true)
            .as_deref()
            .and_then(netz::known_hosts_target)
        else {
            return KnownHostsCheck::NotSaved;
        };
        let mut gespeichert = Vec::new();
        for datei in ziel.files {
            if !std::path::Path::new(&datei).is_file() {
                continue;
            }
            let liste = netz::command_output(
                &netz::known_hosts_lookup_argv(&ziel.name, &datei),
                grenze,
                true,
            );
            gespeichert.push((
                datei,
                netz::known_hosts_fingerprints(liste.as_deref().unwrap_or("")),
            ));
        }
        netz::compare_known_hosts(&gezeigt, &ziel.name, &gespeichert)
    })
    .await
    .unwrap_or(KnownHostsCheck::NotSaved)
}

/// The failure for a known_hosts entry that differs from the shown key,
/// with the command that removes it; `None` when it matches or is missing.
pub fn schluessel_fehler(pruefung: &KnownHostsCheck, gezeigt: &str) -> Option<glib::Error> {
    let KnownHostsCheck::Differs { name, file } = pruefung else {
        crate::log(&format!("Netzwerk: known_hosts {pruefung:?}"));
        return None;
    };
    crate::log("Netzwerk: known_hosts weicht vom bestätigten Schlüssel ab");
    let befehl = format!("ssh-keygen -R {} -f {}", shell_wort(name), shell_wort(file));
    Some(glib::Error::new(
        SchluesselFehler::Anders,
        &gettext_f(
            "The connection was stopped: while logging in, the server showed a different key \
             than the fingerprint you confirmed ({fingerprint}). ssh has saved that other key. \
             Remove it with “{command}” and ask the administrator of the server for the right \
             fingerprint before you connect again.",
            &[("fingerprint", gezeigt), ("command", &befehl)],
        ),
    ))
}

fn shell_wort(wert: &str) -> String {
    if wert
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "-_./:@%+=,[]".contains(c))
    {
        wert.to_owned()
    } else {
        format!("'{}'", wert.replace('\'', "'\\''"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_different_key_stops_with_the_remove_command() {
        let f = schluessel_fehler(
            &KnownHostsCheck::Differs {
                name: "[127.0.0.1]:2222".into(),
                file: "/home/u/.ssh/known hosts".into(),
            },
            "SHA256:x",
        )
        .unwrap();
        assert!(f.matches(SchluesselFehler::Anders));
        assert!(
            f.message()
                .contains("ssh-keygen -R [127.0.0.1]:2222 -f '/home/u/.ssh/known hosts'"),
            "{}",
            f.message()
        );
        assert!(schluessel_fehler(&KnownHostsCheck::Matches, "x").is_none());
        assert!(schluessel_fehler(&KnownHostsCheck::NotSaved, "x").is_none());
    }
}
