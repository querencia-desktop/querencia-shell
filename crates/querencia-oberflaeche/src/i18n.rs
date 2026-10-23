//! Übersetzungen der Bibliothek über GNU gettext, Textdomäne
//! `querencia-oberflaeche` (Muster wie querencia-lautstaerke und
//! querencia-netzwerk, msgid englisch, Deutsch aus `po/de.po`).
//!
//! Die Bausteine zeigen nur wenige eigene Texte, heute die Tastennamen der
//! Kürzel in den klassischen Menüs (`classic_menu::accel_label`). Jedes
//! Programm, das die Bibliothek nutzt, bindet die Domäne an sein
//! Katalogverzeichnis ([`bind`]) und liefert
//! `<lang>/LC_MESSAGES/querencia-oberflaeche.mo` dort mit. Ohne Katalog
//! erscheinen die englischen Texte.
//!
//! Übersetzt wird über die eigene Domäne (`dgettext`), damit die Texte
//! unabhängig von der Hauptdomäne des Programms sind.

use std::path::Path;

/// Textdomäne = Name der .mo-Dateien.
pub const DOMAIN: &str = "querencia-oberflaeche";

/// Bindet die Domäne an `dir` (`<dir>/<lang>/LC_MESSAGES/querencia-oberflaeche.mo`).
pub fn bind(dir: &Path) {
    if let Err(err) = gettextrs::bindtextdomain(DOMAIN, dir) {
        eprintln!(
            "{DOMAIN}: Übersetzungen nicht gebunden ({}): {err}",
            dir.display()
        );
    }
    let _ = gettextrs::bind_textdomain_codeset(DOMAIN, "UTF-8");
}

/// Übersetzt `msgid` mit dem Kontext `msgctxt` in der Domäne der
/// Bibliothek; ohne Übersetzung die msgid (wie `pgettext` von gettext-rs,
/// dort nur für die Hauptdomäne).
pub fn pgettext(msgctxt: &str, msgid: &str) -> String {
    let translation = gettextrs::dgettext(DOMAIN, format!("{msgctxt}\u{4}{msgid}"));
    if translation.contains('\u{4}') {
        msgid.to_owned()
    } else {
        translation
    }
}

/// The German text of `msgid` with the context `ctxt` from `po/de.po`, for
/// tests that keep the German wording; panics if it is missing.
#[cfg(test)]
pub fn german_ctx(ctxt: &str, msgid: &str) -> String {
    let po = include_str!("../po/de.po");
    // the value of `key "…" "…"` lines of a block, unescaped
    fn value(block: &str, key: &str) -> Option<String> {
        let mut lines = block
            .lines()
            .skip_while(|l| !l.starts_with(&format!("{key} ")));
        let first = lines.next()?;
        let mut out = unquote(&first[key.len() + 1..]);
        for l in lines.take_while(|l| l.starts_with('"')) {
            out.push_str(&unquote(l));
        }
        Some(out)
    }
    fn unquote(s: &str) -> String {
        s.trim()
            .trim_matches('"')
            .replace("\\n", "\n")
            .replace("\\\"", "\"")
            .replace("\\\\", "\\")
    }
    po.split("\n\n")
        .find(|block| {
            value(block, "msgid").as_deref() == Some(msgid)
                && value(block, "msgctxt").as_deref() == Some(ctxt)
        })
        .and_then(|block| value(block, "msgstr"))
        .unwrap_or_else(|| panic!("de.po: {ctxt:?} {msgid:?} fehlt"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn untranslated_text_falls_back_to_the_english_msgid() {
        // Ohne gebundenen Katalog liefert gettext die msgid (Kontext als
        // Variable, damit werkzeug/i18n.py den Test nicht in die Vorlage
        // übernimmt).
        let context = "keyboard key";
        assert_eq!(pgettext(context, "Ctrl"), "Ctrl");
        assert_eq!(pgettext(context, "Del"), "Del");
    }

    /// Der deutsche Wortlaut ist der bisherige des Panels (bis Panel 0.3.0
    /// in dessen Domäne `querencia-panel`).
    #[test]
    fn german_wording_is_the_old_one() {
        for (msgid, german) in [
            ("Ctrl", "Strg"),
            ("Shift", "Umschalt"),
            ("Del", "Entf"),
            ("Enter", "Eingabe"),
            ("Print", "Druck"),
        ] {
            assert_eq!(german_ctx("keyboard key", msgid), german, "{msgid}");
        }
    }

    /// Deutsch hängt an `po/de.po`: jede msgid der Vorlage muss dort
    /// übersetzt sein.
    #[test]
    fn german_catalog_is_complete() {
        let pot = include_str!("../po/querencia-oberflaeche.pot");
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
