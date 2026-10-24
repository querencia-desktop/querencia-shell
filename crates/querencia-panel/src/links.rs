//! The web addresses of Querencia the panel offers, and how it opens them:
//! "Online-Hilfe" and "Fehler melden" in the System menu, the link to the
//! web site in "Informationen über Querencia".
//!
//! Addresses open with `xdg-open` ([`open`]), looked up like every program
//! the panel starts (`sitzung_start.rs`: `XDG_OPEN`, `PATH`,
//! `~/opt/querencia/bin`, `/usr/bin`), so the user's browser takes them
//! whatever the desktop's MIME setup says.

/// The web site of Querencia.
pub const HOME_URL: &str = "https://querencialinux.org";
/// Where problems are reported: the issue form chooser of the main
/// repository.
pub const BUG_URL: &str = "https://github.com/querencia-desktop/querencia/issues/new/choose";
/// The documentation, in German and in English.
const HELP_DE: &str = "https://querencialinux.org/de/docs/";
const HELP_EN: &str = "https://querencialinux.org/en/docs/";
const OPENER: &str = "xdg-open";

/// The documentation in the interface language: German for a German
/// interface, else English. `languages` are the interface languages in
/// order of preference, as `g_get_language_names()` gives them (gettext's
/// rules: `LANGUAGE`, then `LC_ALL`, `LC_MESSAGES`, `LANG`; a C locale
/// last).
pub fn help_url<S: AsRef<str>>(languages: &[S]) -> &'static str {
    let first = languages
        .iter()
        .map(AsRef::as_ref)
        .find(|name| !name.is_empty())
        .unwrap_or("C");
    match first.split(['_', '.', '@']).next() {
        Some("de") => HELP_DE,
        _ => HELP_EN,
    }
}

/// [`help_url`] for the languages of this process.
pub fn help_url_here() -> &'static str {
    let languages: Vec<String> = gtk4::glib::language_names()
        .iter()
        .map(|name| name.to_string())
        .collect();
    help_url(&languages)
}

/// Opens `url` with `xdg-open`; start and exit status go to the log, a
/// missing `xdg-open` only to the log.
pub fn open(url: &str) {
    match crate::sitzung_start::suchen(OPENER) {
        Ok(opener) => crate::sitzung_start::starten(&opener, &[url]),
        Err(err) => eprintln!("querencia-panel: {url} nicht geöffnet: {err}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_follows_the_interface_language() {
        // The Querencia session: LANG=en_US.UTF-8 with LANGUAGE=de.
        assert_eq!(
            help_url(&["de", "en_US.UTF-8", "en_US", "en", "C"]),
            HELP_DE
        );
        assert_eq!(help_url(&["de_AT.UTF-8", "de_AT", "de", "C"]), HELP_DE);
        assert_eq!(help_url(&["de@euro", "C"]), HELP_DE);
        assert_eq!(help_url(&["en_GB.UTF-8", "en_GB", "en", "C"]), HELP_EN);
        // Other languages have no documentation yet: English.
        assert_eq!(help_url(&["fr_FR.UTF-8", "fr", "C"]), HELP_EN);
        assert_eq!(help_url(&["C"]), HELP_EN);
        assert_eq!(help_url::<&str>(&[]), HELP_EN);
        // "deu" is not German in gettext's sense of a language code.
        assert_eq!(help_url(&["deu", "C"]), HELP_EN);
    }

    #[test]
    fn addresses_are_the_public_ones() {
        assert_eq!(HOME_URL, "https://querencialinux.org");
        assert_eq!(
            BUG_URL,
            "https://github.com/querencia-desktop/querencia/issues/new/choose"
        );
        assert_eq!(HELP_DE, "https://querencialinux.org/de/docs/");
        assert_eq!(HELP_EN, "https://querencialinux.org/en/docs/");
    }
}
