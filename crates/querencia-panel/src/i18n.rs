//! Übersetzungen über GNU gettext (Textdomäne `querencia-panel`; Muster wie
//! Querencia Desktop 0.2.5 und Editor 0.2.0, `docs/i18n-muster.md` im Editor).
//!
//! Quellsprache im Code ist Englisch (msgid englisch, wie bei GNOME und
//! Debian); Deutsch kommt aus `po/de.po`. Ohne passenden Katalog zeigt das
//! Programm die englischen Texte. Die Sprache wählt gettext wie üblich über
//! `LANGUAGE`, `LC_ALL`, `LC_MESSAGES` und `LANG`.
//!
//! Die Kataloge (`<lang>/LC_MESSAGES/querencia-panel.mo`) sucht das Programm
//! zuerst neben der Binary (`<ablage>/share/locale`, unveränderliche Ablage
//! unter `~/opt/querencia/panel/<sha16>/`), dann im Präfix der Binary
//! (`/usr/bin/../share/locale`), zuletzt in `/usr/share/locale`.
//! `QUERENCIA_PANEL_LOCALEDIR` setzt das Verzeichnis ausdrücklich (Tests).
//!
//! Die Applets Lautstärke und Netzwerk zeigen Texte aus den Bibliotheken
//! `querencia-lautstaerke` und `querencia-netzwerk`, die Menüs die
//! Tastennamen der Kürzel aus `querencia-oberflaeche`; die haben eigene
//! Textdomänen gleichen Namens, deren Kataloge das Panel im selben
//! Verzeichnis mitliefert und hier mit bindet.
//!
//! Texte mit Platzhaltern stehen als ein Satz im Katalog („Saved “{name}”.“),
//! nie zusammengesetzt; [`gettext_f`] und [`ngettext_f`] setzen die Werte erst
//! nach der Übersetzung ein. Gleiche englische Wörter mit verschiedenen
//! Zugriffstasten im Deutschen („_Schließen“ im Dialog, „S_chließen“ im Menü)
//! trennt ein Kontext (`pgettext`).

use std::path::{Path, PathBuf};

use gettextrs::{bind_textdomain_codeset, bindtextdomain, textdomain};
pub use gettextrs::{gettext, ngettext};

/// Textdomäne = Name der .mo-Dateien.
pub const DOMAIN: &str = "querencia-panel";

/// Textdomäne binden; vor `Application::run` aufrufen. Die Locale aus der
/// Umgebung setzt GTK selbst beim Start (`gtk_init` ruft `setlocale(LC_ALL,
/// "")`); alle Texte entstehen erst danach (startup/activate). Ein eigenes
/// `setlocale` wäre in gettext-rs 0.8 `unsafe`.
pub fn init() {
    let dir = locale_dir();
    if let Err(err) = bindtextdomain(DOMAIN, dir.as_path()) {
        eprintln!(
            "querencia-panel: Übersetzungen nicht gebunden ({}): {err}",
            dir.display()
        );
    }
    let _ = bind_textdomain_codeset(DOMAIN, "UTF-8");
    let _ = textdomain(DOMAIN);
    querencia_lautstaerke::i18n::bind(dir.as_path());
    querencia_netzwerk::i18n::bind(dir.as_path());
    querencia_oberflaeche::i18n::bind(dir.as_path());
}

/// Verzeichnis der Kataloge (siehe Modulbeschreibung).
pub fn locale_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("QUERENCIA_PANEL_LOCALEDIR").filter(|d| !d.is_empty()) {
        return PathBuf::from(dir);
    }
    // current_exe() löst den Link ~/opt/querencia/bin/querencia-panel auf.
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.canonicalize().ok())
        .and_then(|exe| exe.parent().map(Path::to_path_buf));
    locale_dir_for(exe_dir.as_deref())
}

fn locale_dir_for(exe_dir: Option<&Path>) -> PathBuf {
    if let Some(exe_dir) = exe_dir {
        let beside = exe_dir.join("share/locale");
        if beside.is_dir() {
            return beside;
        }
        if let Some(prefix) = exe_dir.parent() {
            let prefixed = prefix.join("share/locale");
            if prefixed.is_dir() {
                return prefixed;
            }
        }
    }
    PathBuf::from("/usr/share/locale")
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

/// The German text of `msgid` from `po/de.po` (without context), for tests
/// that keep the German wording; panics if it is missing.
#[cfg(test)]
pub fn german(msgid: &str) -> String {
    german_in(include_str!("../po/de.po"), None, msgid)
}

#[cfg(test)]
fn german_in(po: &str, ctxt: Option<&str>, msgid: &str) -> String {
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
                && value(block, "msgctxt").as_deref() == ctxt
        })
        .and_then(|block| value(block, "msgstr").or_else(|| value(block, "msgstr[0]")))
        .unwrap_or_else(|| panic!("de.po: {ctxt:?} {msgid:?} fehlt"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholders_are_filled_once() {
        assert_eq!(
            fill("Saved “{name}”.", &[("name", "a.txt")]),
            "Saved “a.txt”."
        );
        // Ein Wert mit Klammern wird nicht noch einmal ersetzt.
        assert_eq!(
            fill("{a} und {b}", &[("a", "{b}"), ("b", "x")]),
            "{b} und x"
        );
        assert_eq!(fill("{unbekannt} {", &[("a", "x")]), "{unbekannt} {");
    }

    #[test]
    fn untranslated_text_falls_back_to_the_english_msgid() {
        // Ohne gebundenen Katalog liefert gettext die msgid.
        assert_eq!(
            ngettext_f(
                "{n} unread notification",
                "{n} unread notifications",
                3,
                &[]
            ),
            "3 unread notifications"
        );
        assert_eq!(
            gettext_f("Workspace {number}", &[("number", "2")]),
            "Workspace 2"
        );
    }

    /// Der deutsche Wortlaut bleibt der bisherige (Stichproben; vollständig
    /// prüft `deutsch-gleich.py` gegen panel/0.2.5).
    #[test]
    fn german_wording_is_the_old_one() {
        assert_eq!(german("Applications"), "Anwendungen");
        assert_eq!(german("Accessories"), "Zubehör");
        assert_eq!(german("Log Out…"), "Abmelden …");
        assert_eq!(german("%a %b %e, %l:%M %p"), "%d.%m.%Y %H:%M");
        // Die Tastennamen ("keyboard key") stehen seit 0.3.1 in der Domäne
        // querencia-oberflaeche (Test dort).
    }

    /// Deutsch hängt an `po/de.po`: jede msgid der Vorlage muss dort
    /// übersetzt sein (sonst sähe die deutsche Sitzung Englisch).
    #[test]
    fn german_catalog_is_complete() {
        let pot = include_str!("../po/querencia-panel.pot");
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
        assert!(pot_ids.len() > 50, "Vorlage zu klein: {}", pot_ids.len());
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

    #[test]
    fn catalogs_are_searched_beside_the_binary_then_in_the_prefix() {
        let tmp = std::env::temp_dir().join(format!("qp-i18n-{}", std::process::id()));
        let ablage = tmp.join("ablage");
        std::fs::create_dir_all(ablage.join("share/locale")).unwrap();
        assert_eq!(locale_dir_for(Some(&ablage)), ablage.join("share/locale"));
        let bin = tmp.join("usr/bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::create_dir_all(tmp.join("usr/share/locale")).unwrap();
        assert_eq!(locale_dir_for(Some(&bin)), tmp.join("usr/share/locale"));
        let leer = tmp.join("leer/bin");
        std::fs::create_dir_all(&leer).unwrap();
        assert_eq!(
            locale_dir_for(Some(&leer)),
            PathBuf::from("/usr/share/locale")
        );
        assert_eq!(locale_dir_for(None), PathBuf::from("/usr/share/locale"));
        std::fs::remove_dir_all(&tmp).unwrap();
    }
}
