//! Übersetzungen der Bibliothek über GNU gettext, Textdomäne
//! `querencia-lautstaerke` (Muster wie querencia-panel, msgid englisch, Deutsch aus
//! `po/de.po`). Die Texte zeigt vor allem das Panel (Applet, Tooltip, Menü);
//! es bindet diese Domäne an sein Katalogverzeichnis ([`bind`]). Das
//! eigene Programm `querencia-lautstaerke` bindet sie mit [`init`] neben seiner
//! Binary bzw. im Präfix. Ohne Katalog erscheinen die englischen Texte.
//!
//! Übersetzt wird mit `dgettext` über die eigene Domäne, damit die Texte
//! unabhängig von der Hauptdomäne des Programms sind.

use std::path::{Path, PathBuf};

/// Textdomäne = Name der .mo-Dateien.
pub const DOMAIN: &str = "querencia-lautstaerke";

/// Bindet die Domäne an `dir` (`<dir>/<lang>/LC_MESSAGES/querencia-lautstaerke.mo`).
pub fn bind(dir: &Path) {
    if let Err(err) = gettextrs::bindtextdomain(DOMAIN, dir) {
        eprintln!(
            "{DOMAIN}: Übersetzungen nicht gebunden ({}): {err}",
            dir.display()
        );
    }
    let _ = gettextrs::bind_textdomain_codeset(DOMAIN, "UTF-8");
}

/// Für das eigene Programm: Kataloge neben der Binary (`share/locale`),
/// dann im Präfix der Binary, zuletzt `/usr/share/locale`.
pub fn init() {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.canonicalize().ok())
        .and_then(|exe| exe.parent().map(Path::to_path_buf));
    let dir = exe_dir
        .iter()
        .flat_map(|d| [d.join("share/locale"), d.join("../share/locale")])
        .find(|d| d.is_dir())
        .unwrap_or_else(|| PathBuf::from("/usr/share/locale"));
    bind(&dir);
}

/// Übersetzt `msgid` in der Domäne der Bibliothek.
pub fn gettext(msgid: &str) -> String {
    gettextrs::dgettext(DOMAIN, msgid)
}

/// Übersetzt `msgid` und setzt danach benannte Platzhalter `{name}` ein.
pub fn gettext_f(msgid: &str, args: &[(&str, &str)]) -> String {
    fill(&gettext(msgid), args)
}

/// Ersetzt `{schlüssel}` in einem Durchgang; eingesetzte Werte werden nicht
/// erneut durchsucht, unbekannte Platzhalter bleiben stehen.
pub fn fill(template: &str, args: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let value = after.find('}').and_then(|close| {
            let key = &after[..close];
            args.iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| (*v, close))
        });
        match value {
            Some((v, close)) => {
                out.push_str(v);
                rest = &after[close + 1..];
            }
            None => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholders_are_filled_once() {
        assert_eq!(
            fill("Volume: {percent} %", &[("percent", "50")]),
            "Volume: 50 %"
        );
        assert_eq!(fill("{a} {b}", &[("a", "{b}"), ("b", "x")]), "{b} x");
    }

    /// Deutsch hängt an `po/de.po`: jede msgid der Vorlage muss dort
    /// übersetzt sein.
    #[test]
    fn german_catalog_is_complete() {
        let pot = include_str!("../po/querencia-lautstaerke.pot");
        let de = include_str!("../po/de.po");
        assert!(!de.contains("#, fuzzy"), "de.po enthält fuzzy-Einträge");
        let ids = |text: &str| -> Vec<String> {
            text.split("\n\n")
                .filter_map(|block| {
                    let id: Vec<&str> = block
                        .lines()
                        .skip_while(|l| !l.starts_with("msgid "))
                        .take_while(|l| !l.starts_with("msgstr") && !l.starts_with("msgid_plural"))
                        .collect();
                    let ctxt = block.lines().find(|l| l.starts_with("msgctxt "));
                    (!id.is_empty()).then(|| format!("{ctxt:?}{}", id.join("")))
                })
                .collect()
        };
        let pot_ids = ids(pot);
        let de_ids = ids(de);
        assert!(pot_ids.len() > 3, "Vorlage zu klein: {}", pot_ids.len());
        for id in &pot_ids {
            assert!(de_ids.contains(id), "fehlt in de.po: {id}");
        }
        for block in de.split("\n\n").skip(1) {
            let empty = block
                .lines()
                .any(|l| l.starts_with("msgstr") && l.ends_with(" \"\""))
                && !block.lines().any(|l| l.starts_with('"'));
            assert!(!empty, "unübersetzt in de.po:\n{block}");
        }
    }
}
