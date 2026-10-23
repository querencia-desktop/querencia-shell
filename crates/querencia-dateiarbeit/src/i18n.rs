//! Übersetzungen der Bibliothek über GNU gettext, Textdomäne
//! `querencia-dateiarbeit` (Muster wie querencia-netzwerk und
//! querencia-lautstaerke: msgid englisch, Deutsch aus `po/de.po`).
//!
//! Die Texte zeigen die Programme, die das Crate nutzen (Desktop,
//! Dateimanager). Jedes bindet die Domäne mit [`bind`] an sein eigenes
//! Katalogverzeichnis, meist dasselbe wie für seine Hauptdomäne. Ohne
//! Katalog erscheinen die englischen Texte.
//!
//! Übersetzt wird mit `dgettext`/`dngettext` über die eigene Domäne, damit
//! die Texte unabhängig von der Hauptdomäne des Programms sind. Kontexte
//! (`pgettext`) baut [`pgettext`] selbst nach der GNU-Regel
//! „Kontext\x04msgid“, weil gettext-rs dafür nur die Hauptdomäne kennt.

use std::path::Path;

/// Textdomäne = Name der .mo-Dateien.
pub const DOMAIN: &str = "querencia-dateiarbeit";

/// Bindet die Domäne an `dir` (`<dir>/<lang>/LC_MESSAGES/querencia-dateiarbeit.mo`).
pub fn bind(dir: &Path) {
    if let Err(err) = gettextrs::bindtextdomain(DOMAIN, dir) {
        crate::log_line(format_args!(
            "{DOMAIN}: Übersetzungen nicht gebunden ({}): {err}",
            dir.display()
        ));
    }
    let _ = gettextrs::bind_textdomain_codeset(DOMAIN, "UTF-8");
}

/// Übersetzt `msgid` in der Domäne der Bibliothek.
pub fn gettext(msgid: &str) -> String {
    gettextrs::dgettext(DOMAIN, msgid)
}

/// Übersetzt `msgid` mit Pluralform für `n`.
pub fn ngettext(msgid: &str, msgid_plural: &str, n: u32) -> String {
    gettextrs::dngettext(DOMAIN, msgid, msgid_plural, n)
}

/// Übersetzt `msgid` im Kontext `context`; ohne Übersetzung `msgid`.
pub fn pgettext(context: &str, msgid: &str) -> String {
    let key = format!("{context}\u{4}{msgid}");
    let text = gettextrs::dgettext(DOMAIN, key.as_str());
    if text == key { msgid.to_owned() } else { text }
}

/// Übersetzt `msgid` und setzt danach benannte Platzhalter `{name}` ein.
pub fn gettext_f(msgid: &str, args: &[(&str, &str)]) -> String {
    fill(&gettext(msgid), args)
}

/// Wie [`gettext_f`] mit Pluralform für `n`; `{n}` steht automatisch bereit.
pub fn ngettext_f(msgid: &str, msgid_plural: &str, n: u32, args: &[(&str, &str)]) -> String {
    let count = n.to_string();
    let mut all = vec![("n", count.as_str())];
    all.extend_from_slice(args);
    fill(&ngettext(msgid, msgid_plural, n), &all)
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
            fill("{n} of {total}", &[("n", "1"), ("total", "3")]),
            "1 of 3"
        );
        assert_eq!(fill("{a} {b}", &[("a", "{b}"), ("b", "x")]), "{b} x");
        assert_eq!(fill("{fehlt}", &[]), "{fehlt}");
    }

    #[test]
    fn without_a_catalog_the_english_text_and_plural_stay() {
        assert_eq!(pgettext("thousands separator", ","), ",");
        let (one, many) = ("{n} item", "{n} items");
        assert_eq!(ngettext_f(one, many, 3, &[]), "3 items");
        assert_eq!(ngettext_f(one, many, 1, &[]), "1 item");
    }

    /// Deutsch hängt an `po/de.po`: jede msgid der Vorlage muss dort
    /// übersetzt sein.
    #[test]
    fn german_catalog_is_complete() {
        let pot = include_str!("../po/querencia-dateiarbeit.pot");
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
        assert!(pot_ids.len() > 20, "Vorlage zu klein: {}", pot_ids.len());
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
