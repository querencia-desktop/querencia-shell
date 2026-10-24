//! The address of "Mit Server verbinden …" (Nautilus 2.30
//! `nautilus-connect-server-dialog.c`): service type, server, port, share,
//! folder, user and domain become one GVfs URI; a custom address is
//! checked and normalised (`\\server\share` → `smb://`, `ssh://` →
//! `sftp://`, as Querencia Files QF-0m). No password ever goes into the
//! URI: GVfs asks for it while mounting (`anmeldung.rs`).

use gtk4::glib;

use crate::i18n::{gettext, gettext_f};

/// The service types of the dialog, in its order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dienst {
    Ssh,
    Smb,
    Ftp,
    WebDav,
    WebDavSicher,
    Eigene,
}

impl Dienst {
    pub const ALLE: [Dienst; 6] = [
        Dienst::Ssh,
        Dienst::Smb,
        Dienst::Ftp,
        Dienst::WebDav,
        Dienst::WebDavSicher,
        Dienst::Eigene,
    ];

    /// The name in the drop-down.
    pub fn name(self) -> String {
        match self {
            Dienst::Ssh => gettext("SSH (SFTP)"),
            Dienst::Smb => gettext("Windows share (SMB)"),
            Dienst::Ftp => gettext("FTP (with login)"),
            Dienst::WebDav => gettext("WebDAV (HTTP)"),
            Dienst::WebDavSicher => gettext("Secure WebDAV (HTTPS)"),
            Dienst::Eigene => gettext("Custom location"),
        }
    }

    /// The URI scheme GVfs mounts (`None`: the custom address names it).
    pub fn schema(self) -> Option<&'static str> {
        match self {
            Dienst::Ssh => Some("sftp"),
            Dienst::Smb => Some("smb"),
            Dienst::Ftp => Some("ftp"),
            Dienst::WebDav => Some("dav"),
            Dienst::WebDavSicher => Some("davs"),
            Dienst::Eigene => None,
        }
    }

    /// Only Windows shares have a share name and a domain.
    pub fn hat_freigabe(self) -> bool {
        self == Dienst::Smb
    }

    /// SMB does not take a port in the dialog (Nautilus 2.30: 445/139 are
    /// tried by the client itself).
    pub fn hat_port(self) -> bool {
        !matches!(self, Dienst::Smb | Dienst::Eigene)
    }
}

/// What the person typed into the dialog.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Angaben {
    pub server: String,
    pub port: String,
    pub freigabe: String,
    pub ordner: String,
    pub benutzer: String,
    pub domaene: String,
    /// Only for [`Dienst::Eigene`].
    pub adresse: String,
}

/// Why an address cannot be used; [`Adressfehler::text`] says it in words
/// for the dialog.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Adressfehler {
    KeinServer,
    ServerUngueltig,
    Port,
    KeineAdresse,
    /// The custom address has no `scheme://`; the suggestion has one.
    KeinProtokoll(String),
    Ungueltig,
    KeinRechner,
    /// A scheme that is no network protocol (`file`, `http`, …).
    KeinNetzprotokoll(String),
    /// GVfs on this system has no backend for the scheme.
    NichtUnterstuetzt(String),
    KennwortInAdresse,
}

impl Adressfehler {
    pub fn text(&self) -> String {
        match self {
            Adressfehler::KeinServer => gettext("Enter the name or the address of the server."),
            Adressfehler::ServerUngueltig => gettext(
                "The server name must not contain spaces, slashes or “@”. \
                 Enter the user name in its own field.",
            ),
            Adressfehler::Port => gettext("The port must be a number from 1 to 65535."),
            Adressfehler::KeineAdresse => gettext("Enter the address of the server."),
            Adressfehler::KeinProtokoll(vorschlag) => gettext_f(
                "The address needs a protocol at the start, for example “{example}”.",
                &[("example", vorschlag)],
            ),
            Adressfehler::Ungueltig => gettext("That is not a valid server address."),
            Adressfehler::KeinRechner => gettext("The address does not name a server."),
            Adressfehler::KeinNetzprotokoll(schema) => gettext_f(
                "“{scheme}” is not a network protocol. Use smb://, sftp://, ftp:// or dav://.",
                &[("scheme", schema)],
            ),
            Adressfehler::NichtUnterstuetzt(schema) => gettext_f(
                "This system cannot connect to “{scheme}” servers. The package \
                 “gvfs-backends” provides them.",
                &[("scheme", schema)],
            ),
            Adressfehler::KennwortInAdresse => gettext(
                "Do not write the password into the address. It is asked for when connecting.",
            ),
        }
    }
}

/// The schemes "Mit Server verbinden" accepts in a custom address.
pub const NETZSCHEMEN: [&str; 10] = [
    "smb", "sftp", "ssh", "ftp", "ftps", "ftpis", "dav", "davs", "afp", "nfs",
];

/// Characters kept as they are in a path segment of the URI (RFC 3986
/// pchar without the percent sign); everything else is escaped, UTF-8 too.
const PFAD_ERLAUBT: &str = "!$&'()*+,;=:@";

fn port_pruefen(port: &str) -> Result<Option<u16>, Adressfehler> {
    let port = port.trim();
    if port.is_empty() {
        return Ok(None);
    }
    match port.parse::<u16>() {
        Ok(p) if p > 0 => Ok(Some(p)),
        _ => Err(Adressfehler::Port),
    }
}

/// The host part: a name, an IPv4 address or an IPv6 address (in brackets
/// in the URI). A trailing `:port` typed into the server field is taken as
/// the port when the port field is empty.
fn rechner(server: &str, port: Option<u16>) -> Result<(String, Option<u16>), Adressfehler> {
    let server = server.trim();
    if server.is_empty() {
        return Err(Adressfehler::KeinServer);
    }
    if server
        .chars()
        .any(|c| c.is_whitespace() || c.is_control() || matches!(c, '/' | '\\' | '@' | '?' | '#'))
    {
        return Err(Adressfehler::ServerUngueltig);
    }
    if let Some(innen) = server.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        return Ok((format!("[{innen}]"), port));
    }
    match server.matches(':').count() {
        0 => Ok((server.to_owned(), port)),
        1 => {
            let (name, p) = server.split_once(':').unwrap_or((server, ""));
            let p = port_pruefen(p)?.ok_or(Adressfehler::Port)?;
            if name.is_empty() {
                return Err(Adressfehler::KeinServer);
            }
            Ok((name.to_owned(), port.or(Some(p))))
        }
        // an IPv6 address
        _ => Ok((format!("[{server}]"), port)),
    }
}

fn segment(text: &str) -> String {
    glib::Uri::escape_string(text, Some(PFAD_ERLAUBT), false).to_string()
}

/// The URI for the dialog's fields (`supported`: GVfs' schemes, from
/// `gio::Vfs::supported_uri_schemes`).
pub fn uri(
    dienst: Dienst,
    angaben: &Angaben,
    supported: &[String],
) -> Result<String, Adressfehler> {
    let Some(schema) = dienst.schema() else {
        return eigene_adresse(&angaben.adresse, supported);
    };
    let port = if dienst.hat_port() {
        port_pruefen(&angaben.port)?
    } else {
        None
    };
    let (host, port) = rechner(&angaben.server, port)?;
    let mut benutzer = angaben.benutzer.trim().to_owned();
    if dienst.hat_freigabe() && !angaben.domaene.trim().is_empty() && !benutzer.is_empty() {
        // GVfs' smb backend reads "DOMAIN;user" (Nautilus 2.30)
        benutzer = format!("{};{benutzer}", angaben.domaene.trim());
    }
    let mut uri = format!("{schema}://");
    if !benutzer.is_empty() {
        uri.push_str(&glib::Uri::escape_string(
            &benutzer,
            Some("!$&'()*+,;="),
            false,
        ));
        uri.push('@');
    }
    uri.push_str(&host);
    if let Some(p) = port {
        uri.push_str(&format!(":{p}"));
    }
    uri.push('/');
    let mut teile: Vec<String> = Vec::new();
    if dienst.hat_freigabe() {
        let freigabe = angaben.freigabe.trim().trim_matches('/');
        if !freigabe.is_empty() {
            teile.push(segment(freigabe));
        }
    }
    teile.extend(
        angaben
            .ordner
            .split('/')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(segment),
    );
    uri.push_str(&teile.join("/"));
    if !teile.is_empty() && angaben.ordner.trim_end().ends_with('/') {
        uri.push('/');
    }
    if !supported.iter().any(|s| s == schema) {
        return Err(Adressfehler::NichtUnterstuetzt(schema.to_owned()));
    }
    glib::Uri::parse(&uri, glib::UriFlags::ENCODED).map_err(|_| Adressfehler::Ungueltig)?;
    Ok(uri)
}

/// A typed address (custom location, also from the location bar):
/// `\\server\share` becomes `smb://server/share`, `ssh://` becomes
/// `sftp://`; it must name a network protocol and a server.
pub fn eigene_adresse(eingabe: &str, supported: &[String]) -> Result<String, Adressfehler> {
    let eingabe = eingabe.trim();
    if eingabe.is_empty() {
        return Err(Adressfehler::KeineAdresse);
    }
    let adresse = match eingabe.strip_prefix("\\\\") {
        Some(unc) => format!("smb://{}", unc.replace('\\', "/")),
        None if !eingabe.contains("://") => {
            return Err(Adressfehler::KeinProtokoll(format!(
                "smb://{}",
                eingabe.trim_start_matches('/')
            )));
        }
        None => eingabe.to_owned(),
    };
    let (schema, rest) = adresse.split_once("://").ok_or(Adressfehler::Ungueltig)?;
    let schema = match schema.to_ascii_lowercase().as_str() {
        "ssh" => "sftp".to_owned(),
        anderes => anderes.to_owned(),
    };
    if !NETZSCHEMEN.contains(&schema.as_str()) {
        return Err(Adressfehler::KeinNetzprotokoll(schema));
    }
    let adresse = format!("{schema}://{rest}");
    let geparst = glib::Uri::parse(&adresse, glib::UriFlags::HAS_PASSWORD)
        .or_else(|_| {
            glib::Uri::parse(
                &adresse,
                glib::UriFlags::HAS_PASSWORD | glib::UriFlags::ENCODED,
            )
        })
        .map_err(|_| Adressfehler::Ungueltig)?;
    if geparst.host().is_none_or(|h| h.is_empty()) {
        return Err(Adressfehler::KeinRechner);
    }
    if geparst.password().is_some_and(|p| !p.is_empty()) {
        return Err(Adressfehler::KennwortInAdresse);
    }
    if !supported.contains(&schema) {
        return Err(Adressfehler::NichtUnterstuetzt(schema));
    }
    Ok(adresse)
}

/// The server's name for titles and messages ("host" of the URI, else the
/// whole address).
pub fn servername(uri: &str) -> String {
    glib::Uri::parse(uri, glib::UriFlags::NONE)
        .ok()
        .and_then(|u| u.host())
        .map(|h| h.to_string())
        .filter(|h| !h.is_empty())
        .unwrap_or_else(|| uri.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alle() -> Vec<String> {
        NETZSCHEMEN.iter().map(|s| (*s).to_owned()).collect()
    }

    fn angaben(server: &str) -> Angaben {
        Angaben {
            server: server.into(),
            ..Angaben::default()
        }
    }

    #[test]
    fn the_fields_become_a_uri_per_service() {
        let mut a = angaben("srv.example");
        a.benutzer = "anna".into();
        a.ordner = "/projekte/2026".into();
        assert_eq!(
            uri(Dienst::Ssh, &a, &alle()).unwrap(),
            "sftp://anna@srv.example/projekte/2026"
        );
        a.port = "2222".into();
        assert_eq!(
            uri(Dienst::Ssh, &a, &alle()).unwrap(),
            "sftp://anna@srv.example:2222/projekte/2026"
        );
        assert_eq!(
            uri(Dienst::Ftp, &angaben("192.0.2.7"), &alle()).unwrap(),
            "ftp://192.0.2.7/"
        );
        assert_eq!(
            uri(Dienst::WebDavSicher, &angaben("dav.example"), &alle()).unwrap(),
            "davs://dav.example/"
        );
    }

    #[test]
    fn a_windows_share_takes_share_and_domain_but_no_port() {
        let a = Angaben {
            server: "nas-test".into(),
            port: "4450".into(),
            freigabe: "Daten".into(),
            ordner: "Fotos".into(),
            benutzer: "anna".into(),
            domaene: "BUERO".into(),
            ..Angaben::default()
        };
        assert_eq!(
            uri(Dienst::Smb, &a, &alle()).unwrap(),
            "smb://BUERO;anna@nas-test/Daten/Fotos"
        );
        // a domain without a user is left out (GVfs asks for both)
        let ohne = Angaben {
            benutzer: String::new(),
            ..a
        };
        assert_eq!(
            uri(Dienst::Smb, &ohne, &alle()).unwrap(),
            "smb://nas-test/Daten/Fotos"
        );
    }

    #[test]
    fn umlauts_and_spaces_in_folders_are_escaped() {
        let mut a = angaben("srv");
        a.ordner = "Größe Ä/neu & alt/".into();
        assert_eq!(
            uri(Dienst::WebDav, &a, &alle()).unwrap(),
            "dav://srv/Gr%C3%B6%C3%9Fe%20%C3%84/neu%20&%20alt/"
        );
    }

    #[test]
    fn server_field_accepts_ipv6_and_host_with_port() {
        assert_eq!(
            uri(Dienst::Ssh, &angaben("2001:db8::7"), &alle()).unwrap(),
            "sftp://[2001:db8::7]/"
        );
        assert_eq!(
            uri(Dienst::Ssh, &angaben("[2001:db8::7]"), &alle()).unwrap(),
            "sftp://[2001:db8::7]/"
        );
        assert_eq!(
            uri(Dienst::Ssh, &angaben("127.0.0.1:2222"), &alle()).unwrap(),
            "sftp://127.0.0.1:2222/"
        );
    }

    #[test]
    fn wrong_fields_are_named() {
        assert_eq!(
            uri(Dienst::Ssh, &angaben(" "), &alle()),
            Err(Adressfehler::KeinServer)
        );
        assert_eq!(
            uri(Dienst::Ssh, &angaben("anna@srv"), &alle()),
            Err(Adressfehler::ServerUngueltig)
        );
        assert_eq!(
            uri(Dienst::Ssh, &angaben("a b"), &alle()),
            Err(Adressfehler::ServerUngueltig)
        );
        let mut a = angaben("srv");
        a.port = "70000".into();
        assert_eq!(uri(Dienst::Ssh, &a, &alle()), Err(Adressfehler::Port));
        a.port = "0".into();
        assert_eq!(uri(Dienst::Ftp, &a, &alle()), Err(Adressfehler::Port));
        assert_eq!(
            uri(Dienst::Ssh, &angaben("srv"), &["file".to_owned()]),
            Err(Adressfehler::NichtUnterstuetzt("sftp".into()))
        );
    }

    #[test]
    fn custom_addresses_are_normalised_like_files() {
        assert_eq!(
            eigene_adresse("\\\\nas-test\\Daten\\Fotos", &alle()).unwrap(),
            "smb://nas-test/Daten/Fotos"
        );
        assert_eq!(
            eigene_adresse("ssh://anna@srv:22/home", &alle()).unwrap(),
            "sftp://anna@srv:22/home"
        );
        assert_eq!(
            eigene_adresse("nas-test/Daten", &alle()),
            Err(Adressfehler::KeinProtokoll("smb://nas-test/Daten".into()))
        );
        assert_eq!(
            eigene_adresse("file:///tmp", &alle()),
            Err(Adressfehler::KeinNetzprotokoll("file".into()))
        );
        assert_eq!(
            eigene_adresse("smb:///", &alle()),
            Err(Adressfehler::KeinRechner)
        );
        assert_eq!(
            eigene_adresse("sftp://anna:geheim@srv/", &alle()),
            Err(Adressfehler::KennwortInAdresse)
        );
        assert_eq!(eigene_adresse("", &alle()), Err(Adressfehler::KeineAdresse));
    }

    #[test]
    fn every_error_has_a_text() {
        for f in [
            Adressfehler::KeinServer,
            Adressfehler::ServerUngueltig,
            Adressfehler::Port,
            Adressfehler::KeineAdresse,
            Adressfehler::KeinProtokoll("smb://x".into()),
            Adressfehler::Ungueltig,
            Adressfehler::KeinRechner,
            Adressfehler::KeinNetzprotokoll("file".into()),
            Adressfehler::NichtUnterstuetzt("afp".into()),
            Adressfehler::KennwortInAdresse,
        ] {
            assert!(!f.text().is_empty());
            assert!(!f.text().contains('{'), "{}", f.text());
        }
    }

    #[test]
    fn the_server_name_for_messages() {
        assert_eq!(servername("sftp://anna@srv.example:2222/x"), "srv.example");
        assert_eq!(servername("smb://BUERO;anna@nas/Daten"), "nas");
        assert_eq!(servername("kaputt"), "kaputt");
    }
}
