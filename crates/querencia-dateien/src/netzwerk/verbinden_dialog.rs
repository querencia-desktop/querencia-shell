//! "Mit Server verbinden …" after Nautilus 2.30
//! (`nautilus-connect-server-dialog.c`): service type, server; optional
//! port, share, folder, user name, domain; "Lesezeichen hinzufügen" with
//! a name; Hilfe, Abbrechen, Verbinden. The fields that do not apply to the
//! chosen type are hidden (share and domain only for Windows shares, no
//! port for them, one address field for "Eigene Adresse"). A wrong field
//! is named below the fields and gets the focus; the dialog stays open.
//! "Verbinden" closes it and mounts (`einhaengen::starten`), then the
//! window shows the server.

use std::cell::Cell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{gio, glib};

use super::adresse::{self, Adressfehler, Angaben, Dienst};
use super::einhaengen::{self, Nachher};
use super::rahmen;
use crate::fenster::Fenster;
use crate::i18n::{gettext, gettext_f};

thread_local! {
    /// The type chosen last (the dialog opens with it again).
    static ZULETZT: Cell<usize> = const { Cell::new(0) };
}

struct Felder {
    dienst: gtk4::DropDown,
    server: gtk4::Entry,
    adresse: gtk4::Entry,
    port: gtk4::Entry,
    freigabe: gtk4::Entry,
    ordner: gtk4::Entry,
    benutzer: gtk4::Entry,
    domaene: gtk4::Entry,
    merken: gtk4::CheckButton,
    name: gtk4::Entry,
    fehler: gtk4::Label,
}

impl Felder {
    fn dienst(&self) -> Dienst {
        Dienst::ALLE
            .get(self.dienst.selected() as usize)
            .copied()
            .unwrap_or(Dienst::Ssh)
    }

    fn angaben(&self) -> Angaben {
        Angaben {
            server: self.server.text().to_string(),
            port: self.port.text().to_string(),
            freigabe: self.freigabe.text().to_string(),
            ordner: self.ordner.text().to_string(),
            benutzer: self.benutzer.text().to_string(),
            domaene: self.domaene.text().to_string(),
            adresse: self.adresse.text().to_string(),
        }
    }

    /// The field a failure is about.
    fn feld_fuer(&self, fehler: &Adressfehler) -> &gtk4::Entry {
        match fehler {
            Adressfehler::KeinServer | Adressfehler::ServerUngueltig => &self.server,
            Adressfehler::Port => &self.port,
            _ if self.dienst() == Dienst::Eigene => &self.adresse,
            _ => &self.server,
        }
    }
}

pub fn zeigen(fenster: &Rc<Fenster>) {
    let parent: gtk4::Window = fenster.window().clone().upcast();
    let r = rahmen::neu(
        Some(&parent),
        &gettext("Connect to Server"),
        "network-server",
        &gettext("Connect to a server"),
        &gettext(
            "Choose the type of server and enter its name. The password is asked for when \
             connecting.",
        ),
    );
    let window = r.window.clone();
    window.set_modal(false);
    let gitter = gtk4::Grid::builder()
        .row_spacing(6)
        .column_spacing(12)
        .build();
    let namen: Vec<String> = Dienst::ALLE.iter().map(|d| d.name()).collect();
    let namen: Vec<&str> = namen.iter().map(String::as_str).collect();
    let felder = Rc::new(Felder {
        dienst: gtk4::DropDown::from_strings(&namen),
        server: gtk4::Entry::new(),
        adresse: gtk4::Entry::new(),
        port: gtk4::Entry::new(),
        freigabe: gtk4::Entry::new(),
        ordner: gtk4::Entry::new(),
        benutzer: gtk4::Entry::new(),
        domaene: gtk4::Entry::new(),
        merken: gtk4::CheckButton::with_mnemonic(&gettext("Add _bookmark")),
        name: gtk4::Entry::new(),
        fehler: gtk4::Label::new(None),
    });
    let f = &felder;
    f.dienst.set_selected(ZULETZT.with(Cell::get) as u32);
    f.server
        .set_placeholder_text(Some(&gettext("for example server.example or 192.0.2.7")));
    f.adresse.set_placeholder_text(Some(&gettext(
        "for example smb://server/share or \\\\server\\share",
    )));
    f.port.set_input_purpose(gtk4::InputPurpose::Digits);
    f.port.set_width_chars(6);
    f.port.set_hexpand(false);
    for e in [
        &f.server,
        &f.adresse,
        &f.port,
        &f.freigabe,
        &f.ordner,
        &f.benutzer,
        &f.domaene,
        &f.name,
    ] {
        e.set_activates_default(true);
        e.set_width_chars(28);
    }
    f.port.set_width_chars(6);
    let mut reihe = 0;
    let mut zeile = |text: &str, feld: &gtk4::Widget| -> (gtk4::Label, gtk4::Widget) {
        let l = rahmen::zeile(&gitter, reihe, text, feld);
        reihe += 1;
        (l, feld.clone())
    };
    zeile(&gettext("Service _type:"), f.dienst.upcast_ref());
    let z_server = zeile(&gettext("_Server:"), f.server.upcast_ref());
    let z_adresse = zeile(&gettext("_Location (URI):"), f.adresse.upcast_ref());
    let optional = gtk4::Label::new(None);
    optional.set_markup(&format!(
        "<b>{}</b>",
        glib::markup_escape_text(&gettext("Optional information:"))
    ));
    optional.set_xalign(0.0);
    optional.set_margin_top(6);
    gitter.attach(&optional, 0, reihe, 2, 1);
    reihe += 1;
    let mut zeile = |text: &str, feld: &gtk4::Widget| -> (gtk4::Label, gtk4::Widget) {
        let l = rahmen::zeile(&gitter, reihe, text, feld);
        reihe += 1;
        (l, feld.clone())
    };
    let z_port = zeile(&gettext("_Port:"), f.port.upcast_ref());
    let z_freigabe = zeile(&gettext("Sh_are:"), f.freigabe.upcast_ref());
    let z_ordner = zeile(&gettext("_Folder:"), f.ordner.upcast_ref());
    let z_benutzer = zeile(&gettext("_User name:"), f.benutzer.upcast_ref());
    let z_domaene = zeile(&gettext("_Domain name:"), f.domaene.upcast_ref());
    r.inhalt.append(&gitter);
    let lz = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    lz.append(&f.merken);
    r.inhalt.append(&lz);
    let namensgitter = gtk4::Grid::builder()
        .row_spacing(6)
        .column_spacing(12)
        .margin_start(24)
        .build();
    rahmen::zeile(&namensgitter, 0, &gettext("Bookmark _name:"), &f.name);
    f.name.set_sensitive(false);
    r.inhalt.append(&namensgitter);
    f.fehler.set_xalign(0.0);
    f.fehler.set_wrap(true);
    f.fehler.set_max_width_chars(60);
    f.fehler.add_css_class("error");
    f.fehler.set_visible(false);
    r.inhalt.append(&f.fehler);

    // fields per type
    let anpassen = {
        let felder = felder.clone();
        let zeilen = [
            (z_server.clone(), 0u8),
            (z_adresse.clone(), 1),
            (z_port.clone(), 2),
            (z_freigabe.clone(), 3),
            (z_ordner.clone(), 4),
            (z_benutzer.clone(), 4),
            (z_domaene.clone(), 3),
        ];
        let optional = optional.clone();
        move || {
            let d = felder.dienst();
            let eigene = d == Dienst::Eigene;
            for ((label, feld), art) in &zeilen {
                let zeigen = match art {
                    0 => !eigene,
                    1 => eigene,
                    2 => d.hat_port(),
                    3 => d.hat_freigabe(),
                    _ => !eigene,
                };
                label.set_visible(zeigen);
                feld.set_visible(zeigen);
            }
            optional.set_visible(!eigene);
            felder.fehler.set_visible(false);
        }
    };
    anpassen();
    {
        let anpassen = anpassen.clone();
        f.dienst.connect_selected_notify(move |_| anpassen());
    }
    {
        let name = f.name.clone();
        f.merken
            .connect_toggled(move |m| name.set_sensitive(m.is_active()));
    }

    let hilfe = gtk4::Button::with_mnemonic(&gettext("_Help"));
    let abbrechen = gtk4::Button::with_mnemonic(&gettext("_Cancel"));
    let verbinden = gtk4::Button::with_mnemonic(&gettext("C_onnect"));
    verbinden.add_css_class("suggested-action");
    // Hilfe on the left, as GNOME 2 placed it
    let knopfzeile = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    knopfzeile.set_hexpand(true);
    hilfe.set_halign(gtk4::Align::Start);
    hilfe.set_hexpand(true);
    knopfzeile.append(&hilfe);
    r.knoepfe.set_halign(gtk4::Align::Fill);
    r.knoepfe.append(&knopfzeile);
    r.knoepfe.append(&abbrechen);
    r.knoepfe.append(&verbinden);
    window.set_default_widget(Some(&verbinden));
    {
        let parent = parent.clone();
        hilfe.connect_clicked(move |_| crate::info::online_hilfe(&parent));
    }
    {
        let w = window.clone();
        abbrechen.connect_clicked(move |_| w.destroy());
    }
    rahmen::bei_abbruch(&window, || {
        crate::log("Netzwerk: Verbinden-Dialog geschlossen")
    });
    {
        let felder = felder.clone();
        let w = window.clone();
        let fw = Rc::downgrade(fenster);
        verbinden.connect_clicked(move |_| {
            let Some(fenster) = fw.upgrade() else { return };
            let dienst = felder.dienst();
            let angaben = felder.angaben();
            match adresse::uri(dienst, &angaben, &super::schemen()) {
                Ok(uri) => {
                    ZULETZT.with(|z| z.set(felder.dienst.selected() as usize));
                    let lesezeichen = felder.merken.is_active().then(|| {
                        let name = felder.name.text().trim().to_owned();
                        if name.is_empty() {
                            vorgabe_name(dienst, &angaben, &uri)
                        } else {
                            name
                        }
                    });
                    crate::log(&format!(
                        "Netzwerk: Verbinden-Dialog, Typ {dienst:?}, Lesezeichen {}",
                        lesezeichen.is_some()
                    ));
                    w.destroy();
                    einhaengen::starten(
                        &fenster,
                        &gio::File::for_uri(&uri),
                        Nachher {
                            hingehen: true,
                            lesezeichen,
                        },
                    );
                }
                Err(fehler) => {
                    crate::log(&format!("Netzwerk: Angaben ungültig: {fehler:?}"));
                    felder.fehler.set_text(&fehler.text());
                    felder.fehler.set_visible(true);
                    felder.feld_fuer(&fehler).grab_focus();
                }
            }
        });
    }
    window.present();
    crate::log("Netzwerk: Verbinden-Dialog offen");
    if f.dienst() == Dienst::Eigene {
        f.adresse.grab_focus();
    } else {
        f.server.grab_focus();
    }
}

/// The bookmark's name when none was typed: "Ordner auf Server",
/// "Freigabe auf Server" or the server.
fn vorgabe_name(dienst: Dienst, angaben: &Angaben, uri: &str) -> String {
    let server = adresse::servername(uri);
    let letzter = |s: &str| {
        s.trim()
            .trim_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or("")
            .to_owned()
    };
    let teil = Some(letzter(&angaben.ordner))
        .filter(|s| !s.is_empty())
        .or_else(|| {
            dienst
                .hat_freigabe()
                .then(|| letzter(&angaben.freigabe))
                .filter(|s| !s.is_empty())
        });
    match teil {
        Some(t) if dienst != Dienst::Eigene => gettext_f(
            "{folder} on {server}",
            &[("folder", &t), ("server", &server)],
        ),
        _ => server,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bookmark_names_say_what_and_where() {
        let a = Angaben {
            server: "nas".into(),
            freigabe: "Daten".into(),
            ..Angaben::default()
        };
        assert_eq!(
            vorgabe_name(Dienst::Smb, &a, "smb://nas/Daten"),
            "Daten on nas"
        );
        let a = Angaben {
            ordner: "/home/anna/".into(),
            ..Angaben::default()
        };
        assert_eq!(
            vorgabe_name(Dienst::Ssh, &a, "sftp://srv/home/anna/"),
            "anna on srv"
        );
        assert_eq!(
            vorgabe_name(Dienst::Ftp, &Angaben::default(), "ftp://srv/"),
            "srv"
        );
    }
}
