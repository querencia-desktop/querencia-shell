//! „Reiter beim Start wiederherstellen“ (0.3.0, Vorgabe an): when a window
//! closes, its tabs (the folders, in order) and the current one are noted
//! in `~/.local/state/querencia/dateien-reiter`; the next start without a
//! folder opens them again in one window. With several windows the one
//! closed last counts. Places that are not reachable then (folder gone, a
//! network place not mounted) are left out; if none is left, the start is
//! as before (home folder). Nautilus 2.30 had no such thing; the file is
//! state, not a setting, so it is not in `dateien.ini`.

use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{gio, glib};

use crate::app::App;
use crate::fenster::Fenster;

/// The tabs of a window as noted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gemerkt {
    /// Folder URIs, in the order of the tabs.
    pub orte: Vec<String>,
    /// Index of the current tab.
    pub aktiv: usize,
}

/// `$XDG_STATE_HOME/querencia/dateien-reiter`, without the variable
/// `~/.local/state/…` (`glib::user_state_dir` needs a newer glib feature).
pub fn pfad() -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| glib::home_dir().join(".local").join("state"))
        .join("querencia")
        .join("dateien-reiter")
}

impl Gemerkt {
    /// The file text: a comment, `Aktiv=n`, one URI per line.
    pub fn als_text(&self) -> String {
        let mut s = String::from(
            "# Querencia Dateien – zuletzt offene Reiter (beim Schließen eines Fensters)\n",
        );
        s += &format!("Aktiv={}\n", self.aktiv);
        for o in &self.orte {
            s += o;
            s.push('\n');
        }
        s
    }

    /// Reads the file text; lines that are no URI are skipped. `None`
    /// without any place.
    pub fn aus_text(text: &str) -> Option<Self> {
        let mut aktiv = 0;
        let mut orte = Vec::new();
        for zeile in text.lines().map(str::trim) {
            if zeile.is_empty() || zeile.starts_with('#') {
                continue;
            }
            if let Some(n) = zeile.strip_prefix("Aktiv=") {
                aktiv = n.trim().parse().unwrap_or(0);
            } else if glib::Uri::is_valid(zeile, glib::UriFlags::NONE).is_ok() {
                orte.push(zeile.to_owned());
            }
        }
        if orte.is_empty() {
            return None;
        }
        let aktiv = aktiv.min(orte.len() - 1);
        Some(Self { orte, aktiv })
    }

    /// Only the places `erreichbar` accepts; the current tab stays the same
    /// place, or the one before it when it is left out. `None` when no
    /// place is left.
    pub fn nur_erreichbare(&self, erreichbar: impl Fn(&str) -> bool) -> Option<Self> {
        let mut orte = Vec::new();
        let mut aktiv = 0;
        for (i, o) in self.orte.iter().enumerate() {
            if erreichbar(o) {
                orte.push(o.clone());
            }
            if i == self.aktiv {
                aktiv = orte.len().saturating_sub(1);
            }
        }
        (!orte.is_empty()).then_some(Self { orte, aktiv })
    }
}

/// Whether a place can be shown now without asking anybody: a local
/// folder that exists, or a folder of a mounted remote place. Special
/// places of GVfs (`trash:`, `network:` …) count when GIO knows the scheme.
/// Nothing is mounted here and nothing waits for the network.
pub fn erreichbar(uri: &str) -> bool {
    let datei = gio::File::for_uri(uri);
    if datei.is_native() {
        return datei.query_file_type(gio::FileQueryInfoFlags::NONE, gio::Cancellable::NONE)
            == gio::FileType::Directory;
    }
    let schema = datei
        .uri_scheme()
        .map(|s| s.to_string())
        .unwrap_or_default();
    if matches!(schema.as_str(), "trash" | "network" | "computer" | "recent") {
        return gio::Vfs::default()
            .supported_uri_schemes()
            .iter()
            .any(|s| s.as_str() == schema);
    }
    datei.find_enclosing_mount(gio::Cancellable::NONE).is_ok()
}

fn schreiben(pfad: &Path, g: &Gemerkt) -> std::io::Result<()> {
    if let Some(dir) = pfad.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = pfad.with_extension("neu");
    std::fs::write(&tmp, g.als_text())?;
    std::fs::rename(&tmp, pfad)
}

/// Notes the tabs of `fenster` (it is closing). Only while the setting is
/// on; a window without a place writes nothing.
pub fn merken(fenster: &Fenster, app: &App) {
    if !app.konfig().werte().reiter_wiederherstellen {
        return;
    }
    let liste = fenster.reiterliste();
    let orte: Vec<String> = liste
        .alle()
        .iter()
        .filter_map(|r| r.ort())
        .map(|o| o.uri().to_string())
        .collect();
    if orte.is_empty() {
        return;
    }
    let aktiv = liste.aktiv_index().unwrap_or(0).min(orte.len() - 1);
    let g = Gemerkt { orte, aktiv };
    match schreiben(&pfad(), &g) {
        Ok(()) => crate::log(&format!(
            "Reiter gemerkt: {} (aktiv {})",
            g.orte.len(),
            g.aktiv + 1
        )),
        Err(e) => crate::log(&format!("Reiter nicht gemerkt: {e}")),
    }
}

/// The start without a folder: a window with the noted tabs, if the
/// setting is on and any of them is reachable; else `None`.
pub fn wiederherstellen(app: &Rc<App>) -> Option<Rc<Fenster>> {
    if !app.konfig().werte().reiter_wiederherstellen {
        return None;
    }
    let text = std::fs::read_to_string(pfad()).ok()?;
    let gemerkt = Gemerkt::aus_text(&text)?;
    let g = gemerkt.nur_erreichbare(|o| {
        let ok = erreichbar(o);
        if !ok {
            crate::log(&format!("Reiter übersprungen (nicht erreichbar): {o}"));
        }
        ok
    })?;
    let erster = gio::File::for_uri(&g.orte[0]);
    let fenster = app.neues_fenster(Some(&erster));
    for o in &g.orte[1..] {
        fenster.reiter_oeffnen(Some(&gio::File::for_uri(o)), true, None);
    }
    fenster.reiterliste().zeigen(g.aktiv);
    crate::log(&format!(
        "Reiter wiederhergestellt: {} von {} (aktiv {})",
        g.orte.len(),
        gemerkt.orte.len(),
        g.aktiv + 1
    ));
    Some(fenster)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g(orte: &[&str], aktiv: usize) -> Gemerkt {
        Gemerkt {
            orte: orte.iter().map(|o| (*o).to_owned()).collect(),
            aktiv,
        }
    }

    #[test]
    fn the_file_keeps_order_and_current_tab() {
        let a = g(&["file:///h/a%20b", "sftp://server/srv", "trash:///"], 1);
        assert_eq!(Gemerkt::aus_text(&a.als_text()), Some(a.clone()));
        // broken lines are skipped, a too large index is cut
        let b = Gemerkt::aus_text("# x\nAktiv=9\nkein uri\nfile:///h\n\n").unwrap();
        assert_eq!(b, g(&["file:///h"], 0));
        assert_eq!(Gemerkt::aus_text("Aktiv=0\n"), None);
        assert_eq!(Gemerkt::aus_text(""), None);
    }

    #[test]
    fn unreachable_places_are_left_out() {
        let a = g(&["file:///a", "file:///b", "file:///c", "file:///d"], 2);
        let ohne = |weg: &'static [&'static str]| move |o: &str| !weg.contains(&o);
        // the current one stays current
        assert_eq!(
            a.nur_erreichbare(ohne(&["file:///b"])),
            Some(g(&["file:///a", "file:///c", "file:///d"], 1))
        );
        // the current one gone: the one before it
        assert_eq!(
            a.nur_erreichbare(ohne(&["file:///c"])),
            Some(g(&["file:///a", "file:///b", "file:///d"], 1))
        );
        // the first and current one gone: the next
        let b = g(&["file:///a", "file:///b"], 0);
        assert_eq!(
            b.nur_erreichbare(ohne(&["file:///a"])),
            Some(g(&["file:///b"], 0))
        );
        assert_eq!(b.nur_erreichbare(|_| false), None);
    }

    #[test]
    fn reachable_means_an_existing_local_folder_or_a_mounted_place() {
        let d = std::env::temp_dir().join(format!("qd-reiter-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("datei"), b"x").unwrap();
        let uri = |p: &Path| gio::File::for_path(p).uri().to_string();
        assert!(erreichbar(&uri(&d)));
        assert!(!erreichbar(&uri(&d.join("datei"))), "keine Datei");
        assert!(!erreichbar(&uri(&d.join("fehlt"))));
        // remote places: in the GUI run (no GVfs of the session in tests)
        let _ = std::fs::remove_dir_all(&d);
    }
}
