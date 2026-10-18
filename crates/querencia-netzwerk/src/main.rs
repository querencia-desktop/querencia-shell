//! `querencia-netzwerk`: diagnosis of the network applet's data without a
//! window.
//!
//! - `--zustand`: prints the state once as the panel sees it (backend,
//!   icon, tooltip, devices with addresses, WLANs) and exits;
//! - `--folgen`: prints it again on every change until interrupted.
//!
//! First check on a machine with NetworkManager:
//! `querencia-netzwerk --zustand`.

use std::time::Duration;

use gtk4::glib;
use querencia_netzwerk::model::{self, State};
use querencia_netzwerk::service;

/// The state as text, one fact per line.
pub fn describe(state: &State) -> String {
    let mut out = Vec::new();
    out.push(format!("Quelle: {:?}", state.backend));
    out.push(format!("Symbol: {:?}", model::icon(state)));
    out.push(format!(
        "WLAN: {}, Hardware {}, Flugmodus {}",
        match state.wireless_enabled {
            Some(true) => "an",
            Some(false) => "aus",
            None => "unbekannt",
        },
        if state.wireless_hardware_enabled {
            "frei"
        } else {
            "gesperrt"
        },
        match state.airplane() {
            Some(true) => "an",
            Some(false) => "aus",
            None => "nicht schaltbar",
        }
    ));
    out.push(format!(
        "Tooltip: {}",
        model::tooltip(state).replace('\n', " | ")
    ));
    for device in &state.devices {
        out.push(format!(
            "Gerät {} ({}): {}{}",
            device.interface,
            match device.kind {
                model::Kind::Wired => "Kabel",
                model::Kind::Wireless => "WLAN",
            },
            device.state_text(),
            if device.primary { ", Standard" } else { "" }
        ));
        for (group, rows) in model::info(device, &state.backend) {
            for (label, value) in rows {
                out.push(format!("  {group} {label}: {value}"));
            }
        }
        for ap in model::networks(device) {
            out.push(format!(
                "  Netz „{}“ {} % {}{}{}",
                ap.ssid,
                ap.strength,
                ap.security.label(),
                if ap.known.is_some() {
                    ", gespeichert"
                } else {
                    ""
                },
                if ap.active { ", verbunden" } else { "" }
            ));
        }
    }
    out.join("\n")
}

fn main() -> glib::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let follow = match args.first().map(String::as_str) {
        Some("--zustand") => false,
        Some("--folgen") => true,
        _ => {
            eprintln!("Aufruf: querencia-netzwerk --zustand | --folgen");
            return glib::ExitCode::from(2);
        }
    };
    let main_loop = glib::MainLoop::new(None, false);
    let service = service::get("querencia-netzwerk");
    let shown = std::rc::Rc::new(std::cell::Cell::new(false));
    let looper = main_loop.clone();
    let seen = shown.clone();
    service.subscribe(move |state| {
        if state.backend == model::Backend::Starting {
            return;
        }
        if follow || !seen.get() {
            println!("{}\n", describe(state));
        }
        seen.set(true);
        if !follow {
            // Let a second report of the same start arrive, then end.
            let looper = looper.clone();
            glib::timeout_add_local_once(Duration::from_millis(300), move || looper.quit());
        }
    });
    if !follow {
        let looper = main_loop.clone();
        glib::timeout_add_local_once(Duration::from_secs(10), move || {
            querencia_netzwerk::say!("querencia-netzwerk: keine Antwort nach 10 s");
            looper.quit()
        });
    }
    main_loop.run();
    if shown.get() {
        glib::ExitCode::SUCCESS
    } else {
        glib::ExitCode::FAILURE
    }
}
