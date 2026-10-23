//! `network:///` in the window: GVfs lists a device once per discovery
//! service (Avahi/DNS-SD and wsdd/WS-Discovery), the view shows it once
//! (QD-036, `querencia_dateiarbeit::netz::merge_network_entries`, the
//! DNS-SD entry wins). An empty network gets a hint in the status bar.
//!
//! The duplicates are hidden by a filter of this module chained below the
//! window model's sorted list (`Ordnermodell` stays as it is: positions of
//! the selection are those of the sorted list, the filter sits beneath it).

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::{Rc, Weak};

use gtk4::prelude::*;
use gtk4::{gio, glib};
use querencia_dateiarbeit::netz::{self, NetzEintrag};

use crate::eintrag::Eintrag;
use crate::fenster::Fenster;
use crate::i18n::gettext;
use crate::modell::Zustand;

/// Whether `ort` is the network view (or the Windows network below it).
pub fn ist_netzwerk(ort: &gio::File) -> bool {
    matches!(ort.uri_scheme().as_deref(), Some("network"))
}

/// The names of the entries that are duplicates of another one.
pub fn doppelte(eintraege: &[Eintrag]) -> HashSet<String> {
    let liste: Vec<NetzEintrag> = eintraege
        .iter()
        .map(|e| NetzEintrag {
            native_name: e.name().to_owned(),
            display_name: e.daten().anzeige.clone(),
            target: e
                .daten()
                .info
                .attribute_string("standard::target-uri")
                .map(|t| t.to_string())
                .unwrap_or_else(|| e.datei().uri().to_string()),
        })
        .collect();
    let (bleiben, _) = netz::merge_network_entries(liste);
    let bleiben: HashSet<String> = bleiben.into_iter().map(|e| e.native_name).collect();
    eintraege
        .iter()
        .map(|e| e.name().to_owned())
        .filter(|n| !bleiben.contains(n))
        .collect()
}

pub fn beobachten(fenster: &Rc<Fenster>) {
    filter_einsetzen(fenster);
    let weak = Rc::downgrade(fenster);
    fenster.modell().bei_zustand(move |z| {
        let Some(f) = weak.upgrade() else { return };
        let Some(ort) = f.ort() else { return };
        match z {
            Zustand::Geladen(0) if ist_netzwerk(&ort) => f.melden(&gettext(
                "No computers found in the network – File → Connect to Server…",
            )),
            Zustand::Fehler(_) if super::einhaengen::netzort(&ort) => {
                super::verbindung_melden(&f, &ort);
            }
            _ => {}
        }
    });
}

fn filter_einsetzen(fenster: &Rc<Fenster>) {
    let Some(sortiert) = fenster
        .modell()
        .auswahl()
        .model()
        .and_downcast::<gtk4::SortListModel>()
    else {
        crate::log("Netzwerk: Modellkette unbekannt, Doppel bleiben sichtbar");
        return;
    };
    let unten = sortiert.model();
    let doppelt: Rc<RefCell<HashSet<String>>> = Rc::default();
    let filter = {
        let doppelt = doppelt.clone();
        gtk4::CustomFilter::new(move |o| {
            o.downcast_ref::<Eintrag>()
                .is_none_or(|e| !doppelt.borrow().contains(e.name()))
        })
    };
    let gefiltert = gtk4::FilterListModel::new(unten.clone(), Some(filter.clone()));
    sortiert.set_model(Some(&gefiltert));
    let Some(unten) = unten else { return };
    let weak: Weak<Fenster> = Rc::downgrade(fenster);
    let geplant = Rc::new(std::cell::Cell::new(false));
    unten.connect_items_changed(move |liste, _, _, _| {
        if geplant.replace(true) {
            return;
        }
        let (liste, weak, doppelt, filter, geplant) = (
            liste.clone(),
            weak.clone(),
            doppelt.clone(),
            filter.clone(),
            geplant.clone(),
        );
        // once per main loop turn, after the block of the model is in
        glib::idle_add_local_once(move || {
            geplant.set(false);
            let netz = weak
                .upgrade()
                .and_then(|f| f.ort())
                .is_some_and(|o| ist_netzwerk(&o));
            let neu = if netz {
                let eintraege: Vec<Eintrag> = (0..liste.n_items())
                    .filter_map(|i| liste.item(i).and_downcast::<Eintrag>())
                    .collect();
                doppelte(&eintraege)
            } else {
                HashSet::new()
            };
            if *doppelt.borrow() != neu {
                if !neu.is_empty() {
                    crate::log(&format!("Netzwerk: {} Doppel ausgeblendet", neu.len()));
                }
                doppelt.replace(neu);
                filter.changed(gtk4::FilterChange::Different);
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eintrag(name: &str, anzeige: &str, ziel: &str) -> Eintrag {
        let info = gio::FileInfo::new();
        info.set_name(name);
        info.set_display_name(anzeige);
        info.set_file_type(gio::FileType::Shortcut);
        info.set_attribute_string("standard::target-uri", ziel);
        Eintrag::aus_info(&gio::File::for_uri("network:///"), &info)
    }

    #[test]
    fn a_device_from_avahi_and_wsdd_is_shown_once() {
        let liste = [
            eintrag("wsdd-server-9ac3", "NAS-TEST", "smb://NAS-TEST.local/"),
            eintrag(
                "dnssd-server-nas-test._smb._tcp",
                "nas-test",
                "smb://192.0.2.12:445/",
            ),
            eintrag("wsdd-server-544b", "PC-TEST", "smb://PC-TEST.local/"),
            eintrag("smb-root", "Windows-Netzwerk", "smb:///"),
        ];
        let d = doppelte(&liste);
        assert_eq!(d, HashSet::from(["wsdd-server-9ac3".to_owned()]));
    }

    #[test]
    fn computers_of_the_network_open_like_folders() {
        let e = eintrag("wsdd-urn:uuid:1", "QD-PC", "smb://127.0.0.1/");
        assert!(e.ist_ordner());
        assert_eq!(e.ziel().uri(), "smb://127.0.0.1/");
    }

    #[test]
    fn same_name_on_two_protocols_stays() {
        let liste = [
            eintrag(
                "dnssd-server-srv._sftp-ssh._tcp",
                "srv",
                "sftp://srv.local/",
            ),
            eintrag("wsdd-server-1", "SRV", "smb://SRV.local/"),
        ];
        assert!(doppelte(&liste).is_empty());
        assert!(ist_netzwerk(&gio::File::for_uri("network:///")));
        assert!(!ist_netzwerk(&gio::File::for_uri("smb:///")));
    }
}
