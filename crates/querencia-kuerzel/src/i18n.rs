//! Translations through GNU gettext (text domain `querencia-kuerzel`),
//! as in querencia-sitzung: msgid English, German from `po/de.po`, also
//! `it.po`, `es.po`, `fr.po`. Without a matching catalog the English texts
//! appear. The language comes from `LANGUAGE`, `LC_ALL`, `LC_MESSAGES` and
//! `LANG` as usual.
//!
//! Key names and key sequences use the context "key" ([`key`]): German
//! keyboards print „Strg“, „Umschalt“, „Bild↑“, French ones „Maj“,
//! „Entrée“. The lines of `data/programme.txt` are msgids as well; the
//! catalog tool reads them from there (`werkzeug/i18n.py`).
//!
//! [`init`] sets the locale itself, because `--liste` prints without GTK.
//! Catalogs (`<lang>/LC_MESSAGES/querencia-kuerzel.mo`) are searched beside
//! the binary (`share/locale`), then in the prefix of the binary
//! (`/usr/bin/../share/locale`), last in `/usr/share/locale`.

use std::path::{Path, PathBuf};

pub use gettextrs::gettext;

/// Text domain = name of the .mo files.
pub const DOMAIN: &str = "querencia-kuerzel";

/// Context of key names and key sequences in the catalog.
const KEY_CONTEXT: &str = "key";

/// Sets the locale from the environment and binds the text domain; first
/// thing in `main`, before threads exist.
pub fn init() {
    // SAFETY: setlocale is not thread-safe; init runs first in main, before
    // this process starts threads or reads locale data.
    let _ = unsafe { gettextrs::setlocale(gettextrs::LocaleCategory::LcAll, "") };
    let dir = locale_dir_for(
        std::env::current_exe()
            .ok()
            .and_then(|exe| exe.canonicalize().ok())
            .and_then(|exe| exe.parent().map(Path::to_path_buf))
            .as_deref(),
    );
    if let Err(err) = gettextrs::bindtextdomain(DOMAIN, dir.as_path()) {
        eprintln!(
            "querencia-kuerzel: Übersetzungen nicht gebunden ({}): {err}",
            dir.display()
        );
    }
    let _ = gettextrs::bind_textdomain_codeset(DOMAIN, "UTF-8");
    let _ = gettextrs::textdomain(DOMAIN);
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

/// For the extraction by `werkzeug/i18n.py` the call sites name the context
/// as a literal (`pgettext("key", "Ctrl")`).
pub fn pgettext(context: &str, msgid: &str) -> String {
    gettextrs::pgettext(context, msgid)
}

/// A key name or key sequence („Ctrl+Shift+T“) as printed on keyboards of
/// the session's language.
pub fn key(keys: &str) -> String {
    gettextrs::pgettext(KEY_CONTEXT, keys)
}

/// Translates `msgid` and then fills in named placeholders `{name}`.
pub fn gettext_f(msgid: &str, args: &[(&str, &str)]) -> String {
    fill(&gettext(msgid), args)
}

/// Replaces `{key}` in one pass; inserted values are not searched again,
/// unknown placeholders stay.
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
            fill("{a} and {b}", &[("a", "{b}"), ("b", "x")]),
            "{b} and x"
        );
        assert_eq!(fill("{unknown} {", &[("a", "x")]), "{unknown} {");
    }

    /// German comes from `po/de.po`: every msgid of the template must be
    /// translated there (otherwise a German session would see English).
    #[test]
    fn german_catalog_is_complete() {
        let pot = include_str!("../po/querencia-kuerzel.pot");
        let de = include_str!("../po/de.po");
        assert!(!de.contains("#, fuzzy"), "fuzzy entries in de.po");
        let ids = |text: &str| -> Vec<String> {
            text.split("\n\n")
                .filter_map(|block| {
                    let id: Vec<&str> = block
                        .lines()
                        .skip_while(|l| !l.starts_with("msgid "))
                        .take_while(|l| !l.starts_with("msgstr"))
                        .collect();
                    let ctxt = block.lines().find(|l| l.starts_with("msgctxt "));
                    (!id.is_empty()).then(|| format!("{ctxt:?}{}", id.join("")))
                })
                .collect()
        };
        let de_ids = ids(de);
        let pot_ids = ids(pot);
        assert!(pot_ids.len() > 150, "template too small: {}", pot_ids.len());
        for id in &pot_ids {
            assert!(de_ids.contains(id), "missing in de.po: {id}");
        }
    }

    #[test]
    fn catalogs_are_searched_beside_the_binary_then_in_the_prefix() {
        let tmp = std::env::temp_dir().join(format!("qk-i18n-{}", std::process::id()));
        let store = tmp.join("ablage");
        std::fs::create_dir_all(store.join("share/locale")).expect("test folder");
        assert_eq!(locale_dir_for(Some(&store)), store.join("share/locale"));
        let bin = tmp.join("usr/bin");
        std::fs::create_dir_all(&bin).expect("test folder");
        std::fs::create_dir_all(tmp.join("usr/share/locale")).expect("test folder");
        assert_eq!(locale_dir_for(Some(&bin)), tmp.join("usr/share/locale"));
        assert_eq!(locale_dir_for(None), PathBuf::from("/usr/share/locale"));
        std::fs::remove_dir_all(&tmp).expect("test folder");
    }
}
