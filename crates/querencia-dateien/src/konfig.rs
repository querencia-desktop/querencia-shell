//! Settings of Querencia Dateien in `~/.config/querencia/dateien.ini`
//! (format as `dateisuche.ini`: sections, `Schluessel=Wert`). Every change
//! is written at once and reaches every open window (GNOME 2: no "Apply").
//!
//! The first start without `dateien.ini` takes six values once from
//! Querencia Files (`~/.config/querencia-files/settings.toml`, migration
//! M6 in B §4.2) and notes `AusFiles=1`; the Files file is only read, never
//! changed or removed.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk4::glib;

use crate::regeln::{Ansichtsart, Kriterium, Zoom};

/// What a double click on an executable text file does (B §3.6, Verhalten).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AusfuehrbareTexte {
    Nachfragen,
    Anzeigen,
    Ausfuehren,
}

impl AusfuehrbareTexte {
    pub const ALLE: [AusfuehrbareTexte; 3] = [Self::Nachfragen, Self::Anzeigen, Self::Ausfuehren];
    pub fn kennung(self) -> &'static str {
        match self {
            Self::Nachfragen => "nachfragen",
            Self::Anzeigen => "anzeigen",
            Self::Ausfuehren => "ausfuehren",
        }
    }
    pub fn aus_kennung(k: &str) -> Option<Self> {
        Self::ALLE.into_iter().find(|a| a.kennung() == k)
    }
}

/// Extra line under the name in the icon view (B §3.6, Anzeige; Beta).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zusatz {
    Nichts,
    Groesse,
    Datum,
    Typ,
}

impl Zusatz {
    pub const ALLE: [Zusatz; 4] = [Self::Nichts, Self::Groesse, Self::Datum, Self::Typ];
    pub fn kennung(self) -> &'static str {
        match self {
            Self::Nichts => "nichts",
            Self::Groesse => "groesse",
            Self::Datum => "datum",
            Self::Typ => "typ",
        }
    }
    pub fn aus_kennung(k: &str) -> Option<Self> {
        Self::ALLE.into_iter().find(|a| a.kennung() == k)
    }
}

/// Where a preview is made (B §3.6, Vorschau; Beta).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wo {
    Immer,
    NurLokal,
    Nie,
}

impl Wo {
    pub const ALLE: [Wo; 3] = [Self::Immer, Self::NurLokal, Self::Nie];
    pub fn kennung(self) -> &'static str {
        match self {
            Self::Immer => "immer",
            Self::NurLokal => "lokal",
            Self::Nie => "nie",
        }
    }
    pub fn aus_kennung(k: &str) -> Option<Self> {
        Self::ALLE.into_iter().find(|a| a.kennung() == k)
    }
}

/// All settings with Nautilus 2.30's defaults (B §3.7).
#[derive(Clone, Debug, PartialEq)]
pub struct Werte {
    // Ansichten
    pub ansicht: Ansichtsart,
    pub anordnen: Kriterium,
    pub ordner_zuerst: bool,
    pub verborgene: bool,
    pub zoom_symbole: Zoom,
    pub zoom_liste: Zoom,
    // Verhalten
    pub einfacher_klick: bool,
    pub ausfuehrbare_texte: AusfuehrbareTexte,
    pub nachfragen: bool,
    pub loeschen_anbieten: bool,
    pub adresse_als_text: bool,
    // Anzeige (Beta)
    pub zusatz: Zusatz,
    pub zusatz_gross: Zusatz,
    pub datum_iso: bool,
    // Listenspalten (Beta)
    pub spalte_groesse: bool,
    pub spalte_typ: bool,
    pub spalte_datum: bool,
    pub spalte_rechte: bool,
    pub spalte_eigentuemer: bool,
    pub spalte_gruppe: bool,
    // Vorschau (Beta)
    pub vorschaubilder: Wo,
    pub vorschau_max_mb: u32,
    pub objekte_zaehlen: Wo,
    // Fenster (no dialog: remembered from the window and the View menu)
    pub breite: i32,
    pub hoehe: i32,
    pub werkzeugleiste: bool,
    pub seitenleiste: bool,
    pub adressleiste: bool,
    pub statusleiste: bool,
    pub seitenleiste_breite: i32,
    // Migration M6
    pub aus_files: bool,
}

impl Default for Werte {
    fn default() -> Self {
        Self {
            ansicht: Ansichtsart::Symbole,
            anordnen: Kriterium::Name,
            ordner_zuerst: true,
            verborgene: false,
            zoom_symbole: Zoom::NORMAL,
            zoom_liste: Zoom::LISTE,
            einfacher_klick: false,
            ausfuehrbare_texte: AusfuehrbareTexte::Nachfragen,
            nachfragen: true,
            loeschen_anbieten: false,
            adresse_als_text: false,
            zusatz: Zusatz::Nichts,
            zusatz_gross: Zusatz::Groesse,
            datum_iso: false,
            spalte_groesse: true,
            spalte_typ: true,
            spalte_datum: true,
            spalte_rechte: false,
            spalte_eigentuemer: false,
            spalte_gruppe: false,
            vorschaubilder: Wo::NurLokal,
            vorschau_max_mb: 10,
            objekte_zaehlen: Wo::NurLokal,
            breite: 800,
            hoehe: 550,
            werkzeugleiste: true,
            seitenleiste: true,
            adressleiste: true,
            statusleiste: true,
            seitenleiste_breite: 150,
            aus_files: false,
        }
    }
}

impl Werte {
    /// The zoom a new view of `art` starts with.
    pub fn zoom_fuer(&self, art: Ansichtsart) -> Zoom {
        match art {
            Ansichtsart::Symbole => self.zoom_symbole,
            Ansichtsart::Liste | Ansichtsart::Kompakt => self.zoom_liste,
        }
    }

    /// Reads `text` (missing or unknown values keep their default).
    pub fn aus_ini(text: &str) -> Self {
        let mut w = Self::default();
        let mut abschnitt = String::new();
        for zeile in text.lines().map(str::trim) {
            if zeile.is_empty() || zeile.starts_with('#') || zeile.starts_with(';') {
                continue;
            }
            if let Some(name) = zeile.strip_prefix('[').and_then(|z| z.strip_suffix(']')) {
                abschnitt = name.trim().to_owned();
                continue;
            }
            let Some((k, v)) = zeile.split_once('=') else {
                continue;
            };
            w.setzen(&abschnitt, k.trim(), v.trim());
        }
        w
    }

    fn setzen(&mut self, abschnitt: &str, k: &str, v: &str) {
        let b = |v: &str| matches!(v, "true" | "1" | "ja");
        let zahl = |v: &str| v.parse::<i32>().ok();
        let zoom = |v: &str| {
            v.parse::<u32>()
                .ok()
                .filter(|p| crate::regeln::ZOOMSTUFEN.contains(p))
                .map(Zoom)
        };
        match (abschnitt, k) {
            ("Ansichten", "Ansicht") => {
                if let Some(a) = Ansichtsart::aus_kennung(v) {
                    self.ansicht = a;
                }
            }
            ("Ansichten", "Anordnen") => {
                if let Some(a) = Kriterium::aus_kennung(v) {
                    self.anordnen = a;
                }
            }
            ("Ansichten", "OrdnerZuerst") => self.ordner_zuerst = b(v),
            ("Ansichten", "Verborgene") => self.verborgene = b(v),
            ("Ansichten", "ZoomSymbole") => {
                self.zoom_symbole = zoom(v).unwrap_or(self.zoom_symbole)
            }
            ("Ansichten", "ZoomListe") => self.zoom_liste = zoom(v).unwrap_or(self.zoom_liste),
            ("Verhalten", "EinfacherKlick") => self.einfacher_klick = b(v),
            ("Verhalten", "AusfuehrbareTexte") => {
                if let Some(a) = AusfuehrbareTexte::aus_kennung(v) {
                    self.ausfuehrbare_texte = a;
                }
            }
            ("Verhalten", "Nachfragen") => self.nachfragen = b(v),
            ("Verhalten", "LoeschenAnbieten") => self.loeschen_anbieten = b(v),
            ("Verhalten", "AdresseAlsText") => self.adresse_als_text = b(v),
            ("Anzeige", "Zusatz") => {
                if let Some(z) = Zusatz::aus_kennung(v) {
                    self.zusatz = z;
                }
            }
            ("Anzeige", "ZusatzGross") => {
                if let Some(z) = Zusatz::aus_kennung(v) {
                    self.zusatz_gross = z;
                }
            }
            ("Anzeige", "Datum") => self.datum_iso = v == "iso",
            ("Listenspalten", "Groesse") => self.spalte_groesse = b(v),
            ("Listenspalten", "Typ") => self.spalte_typ = b(v),
            ("Listenspalten", "Datum") => self.spalte_datum = b(v),
            ("Listenspalten", "Rechte") => self.spalte_rechte = b(v),
            ("Listenspalten", "Eigentuemer") => self.spalte_eigentuemer = b(v),
            ("Listenspalten", "Gruppe") => self.spalte_gruppe = b(v),
            ("Vorschau", "Bilder") => {
                if let Some(x) = Wo::aus_kennung(v) {
                    self.vorschaubilder = x;
                }
            }
            ("Vorschau", "MaxMB") => {
                if let Ok(n) = v.parse::<u32>() {
                    self.vorschau_max_mb = n.clamp(1, 4096);
                }
            }
            ("Vorschau", "Zaehlen") => {
                if let Some(x) = Wo::aus_kennung(v) {
                    self.objekte_zaehlen = x;
                }
            }
            ("Fenster", "Breite") => self.breite = zahl(v).unwrap_or(self.breite).clamp(320, 16384),
            ("Fenster", "Hoehe") => self.hoehe = zahl(v).unwrap_or(self.hoehe).clamp(240, 16384),
            ("Fenster", "Werkzeugleiste") => self.werkzeugleiste = b(v),
            ("Fenster", "Seitenleiste") => self.seitenleiste = b(v),
            ("Fenster", "Adressleiste") => self.adressleiste = b(v),
            ("Fenster", "Statusleiste") => self.statusleiste = b(v),
            ("Fenster", "SeitenleisteBreite") => {
                self.seitenleiste_breite =
                    zahl(v).unwrap_or(self.seitenleiste_breite).clamp(80, 800);
            }
            ("Migration", "AusFiles") => self.aus_files = b(v),
            _ => {}
        }
    }

    /// The file text.
    pub fn als_ini(&self) -> String {
        let b = |x: bool| if x { "true" } else { "false" };
        let mut s = String::from(
            "# Querencia Dateien – Einstellungen (jede Änderung wird sofort geschrieben)\n",
        );
        s += "[Ansichten]\n";
        s += &format!("Ansicht={}\n", self.ansicht.kennung());
        s += &format!("Anordnen={}\n", self.anordnen.kennung());
        s += &format!("OrdnerZuerst={}\n", b(self.ordner_zuerst));
        s += &format!("Verborgene={}\n", b(self.verborgene));
        s += &format!("ZoomSymbole={}\n", self.zoom_symbole.0);
        s += &format!("ZoomListe={}\n", self.zoom_liste.0);
        s += "\n[Verhalten]\n";
        s += &format!("EinfacherKlick={}\n", b(self.einfacher_klick));
        s += &format!("AusfuehrbareTexte={}\n", self.ausfuehrbare_texte.kennung());
        s += &format!("Nachfragen={}\n", b(self.nachfragen));
        s += &format!("LoeschenAnbieten={}\n", b(self.loeschen_anbieten));
        s += &format!("AdresseAlsText={}\n", b(self.adresse_als_text));
        s += "\n[Anzeige]\n";
        s += &format!("Zusatz={}\n", self.zusatz.kennung());
        s += &format!("ZusatzGross={}\n", self.zusatz_gross.kennung());
        s += &format!("Datum={}\n", if self.datum_iso { "iso" } else { "region" });
        s += "\n[Listenspalten]\n";
        s += &format!("Groesse={}\n", b(self.spalte_groesse));
        s += &format!("Typ={}\n", b(self.spalte_typ));
        s += &format!("Datum={}\n", b(self.spalte_datum));
        s += &format!("Rechte={}\n", b(self.spalte_rechte));
        s += &format!("Eigentuemer={}\n", b(self.spalte_eigentuemer));
        s += &format!("Gruppe={}\n", b(self.spalte_gruppe));
        s += "\n[Vorschau]\n";
        s += &format!("Bilder={}\n", self.vorschaubilder.kennung());
        s += &format!("MaxMB={}\n", self.vorschau_max_mb);
        s += &format!("Zaehlen={}\n", self.objekte_zaehlen.kennung());
        s += "\n[Fenster]\n";
        s += &format!("Breite={}\n", self.breite);
        s += &format!("Hoehe={}\n", self.hoehe);
        s += &format!("Werkzeugleiste={}\n", b(self.werkzeugleiste));
        s += &format!("Seitenleiste={}\n", b(self.seitenleiste));
        s += &format!("Adressleiste={}\n", b(self.adressleiste));
        s += &format!("Statusleiste={}\n", b(self.statusleiste));
        s += &format!("SeitenleisteBreite={}\n", self.seitenleiste_breite);
        if self.aus_files {
            s += "\n[Migration]\nAusFiles=1\n";
        }
        s
    }

    /// Migration M6: the six values of a Querencia Files `settings.toml`
    /// (top-level `key = value` lines; other tables are skipped).
    pub fn aus_files(text: &str) -> Self {
        let mut w = Self::default();
        let mut oben = true;
        let mut klicks = (None::<i64>, None::<i64>);
        for zeile in text.lines().map(str::trim) {
            if zeile.starts_with('[') {
                oben = false;
                continue;
            }
            if !oben || zeile.starts_with('#') {
                continue;
            }
            let Some((k, v)) = zeile.split_once('=') else {
                continue;
            };
            let v = v.trim().trim_matches('"');
            match k.trim() {
                "show_hidden" => w.verborgene = v == "true",
                "browser_mode" => {
                    w.ansicht = match v {
                        "icons" | "grid" => Ansichtsart::Symbole,
                        // B §4.2 M6: columns becomes the list view
                        "list" | "columns" => Ansichtsart::Liste,
                        _ => w.ansicht,
                    }
                }
                "sort_key" => {
                    w.anordnen = match v {
                        "name" => Kriterium::Name,
                        "size" => Kriterium::Groesse,
                        "type" => Kriterium::Typ,
                        "modified" => Kriterium::Datum,
                        _ => w.anordnen,
                    }
                }
                "folders_first" => w.ordner_zuerst = v != "false",
                "grid_folder_clicks" => klicks.0 = v.parse().ok(),
                "list_folder_clicks" => klicks.1 = v.parse().ok(),
                "sidebar_expanded" => w.seitenleiste = v != "false",
                _ => {}
            }
        }
        w.einfacher_klick = klicks == (Some(1), Some(1));
        w.aus_files = true;
        w
    }
}

/// `~/.config/querencia/dateien.ini`.
pub fn pfad() -> PathBuf {
    glib::user_config_dir()
        .join("querencia")
        .join("dateien.ini")
}

/// `~/.config/querencia-files/settings.toml`.
pub fn files_pfad() -> PathBuf {
    glib::user_config_dir()
        .join("querencia-files")
        .join("settings.toml")
}

type Handler = Rc<dyn Fn(&Werte)>;

/// The settings of the running program (one per process).
pub struct Konfig {
    pfad: PathBuf,
    werte: RefCell<Werte>,
    handler: RefCell<Vec<Handler>>,
}

impl Konfig {
    /// Reads `dateien.ini`; without it migrates once from Files (and writes
    /// the result), else starts with the defaults (written on first change).
    pub fn laden() -> Rc<Self> {
        Self::laden_von(&pfad(), &files_pfad())
    }

    pub fn laden_von(pfad: &Path, files: &Path) -> Rc<Self> {
        let werte = match std::fs::read_to_string(pfad) {
            Ok(text) => Werte::aus_ini(&text),
            Err(_) => match std::fs::read_to_string(files) {
                Ok(text) => {
                    let w = Werte::aus_files(&text);
                    let _ = schreiben(pfad, &w);
                    eprintln!(
                        "querencia-dateien: Einstellungen einmalig aus {} übernommen",
                        files.display()
                    );
                    w
                }
                Err(_) => Werte::default(),
            },
        };
        Rc::new(Self {
            pfad: pfad.to_path_buf(),
            werte: RefCell::new(werte),
            handler: RefCell::default(),
        })
    }

    pub fn werte(&self) -> Werte {
        self.werte.borrow().clone()
    }

    /// Changes the settings, writes the file and tells every handler (only
    /// when something changed).
    pub fn aendern(&self, f: impl FnOnce(&mut Werte)) {
        let neu = {
            let mut w = self.werte.borrow().clone();
            f(&mut w);
            w
        };
        if neu == *self.werte.borrow() {
            return;
        }
        self.werte.replace(neu.clone());
        if let Err(err) = schreiben(&self.pfad, &neu) {
            eprintln!(
                "querencia-dateien: {} nicht geschrieben: {err}",
                self.pfad.display()
            );
        }
        let handler: Vec<Handler> = self.handler.borrow().clone();
        for h in handler {
            h(&neu);
        }
    }

    /// `f` gets the settings after every change.
    pub fn bei_aenderung(&self, f: impl Fn(&Werte) + 'static) {
        self.handler.borrow_mut().push(Rc::new(f));
    }
}

/// Writes atomically: a temporary file beside it, then rename.
fn schreiben(pfad: &Path, werte: &Werte) -> std::io::Result<()> {
    if let Some(dir) = pfad.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = pfad.with_extension("ini.neu");
    std::fs::write(&tmp, werte.als_ini())?;
    std::fs::rename(&tmp, pfad)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_nautilus_2_30() {
        let w = Werte::default();
        assert_eq!(w.ansicht, Ansichtsart::Symbole);
        assert_eq!(w.zoom_symbole, Zoom(100));
        assert_eq!(w.zoom_liste, Zoom(50));
        assert!(!w.einfacher_klick);
        assert!(w.ordner_zuerst && w.nachfragen && !w.verborgene && !w.loeschen_anbieten);
        assert_eq!(w.ausfuehrbare_texte, AusfuehrbareTexte::Nachfragen);
        assert_eq!((w.breite, w.hoehe), (800, 550));
    }

    #[test]
    fn ini_round_trip_and_unknown_values_keep_defaults() {
        let mut w = Werte {
            ansicht: Ansichtsart::Liste,
            anordnen: Kriterium::Datum,
            verborgene: true,
            einfacher_klick: true,
            zoom_symbole: Zoom(150),
            ausfuehrbare_texte: AusfuehrbareTexte::Anzeigen,
            seitenleiste: false,
            vorschaubilder: Wo::Nie,
            ..Werte::default()
        };
        assert_eq!(Werte::aus_ini(&w.als_ini()), w);
        w.aus_files = true;
        assert_eq!(Werte::aus_ini(&w.als_ini()), w);
        let kaputt = "[Ansichten]\nAnsicht=spalten\nZoomSymbole=77\n[Fenster]\nBreite=10\nfoo\n";
        let w = Werte::aus_ini(kaputt);
        assert_eq!(w.ansicht, Ansichtsart::Symbole);
        assert_eq!(w.zoom_symbole, Zoom(100));
        assert_eq!(w.breite, 320);
    }

    #[test]
    fn migration_m6_takes_six_values_from_files() {
        let toml = "browser_mode = \"columns\"\nshow_hidden = true\nsort_key = \"modified\"\n\
                    folders_first = false\ngrid_folder_clicks = 1\nlist_folder_clicks = 1\n\
                    sidebar_expanded = false\n[tabs]\nshow_hidden = false\n";
        let w = Werte::aus_files(toml);
        assert_eq!(w.ansicht, Ansichtsart::Liste);
        assert!(w.verborgene);
        assert_eq!(w.anordnen, Kriterium::Datum);
        assert!(!w.ordner_zuerst);
        assert!(w.einfacher_klick);
        assert!(!w.seitenleiste);
        assert!(w.aus_files);
        // one click value 2: double click stays
        let w = Werte::aus_files(
            "grid_folder_clicks = 2\nlist_folder_clicks = 1\nbrowser_mode = \"list\"\n",
        );
        assert!(!w.einfacher_klick);
        assert_eq!(w.ansicht, Ansichtsart::Liste);
    }

    #[test]
    fn konfig_migrates_once_writes_on_change_and_never_touches_files() {
        let dir = std::env::temp_dir().join(format!("qdat-konfig-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("querencia-files")).unwrap();
        let files = dir.join("querencia-files/settings.toml");
        let inhalt = "show_hidden = true\nbrowser_mode = \"icons\"\n";
        std::fs::write(&files, inhalt).unwrap();
        let ini = dir.join("querencia/dateien.ini");
        let k = Konfig::laden_von(&ini, &files);
        assert!(k.werte().verborgene && k.werte().aus_files);
        assert!(
            std::fs::read_to_string(&ini)
                .unwrap()
                .contains("AusFiles=1")
        );
        let gesehen = Rc::new(RefCell::new(0));
        let g = gesehen.clone();
        k.bei_aenderung(move |_| *g.borrow_mut() += 1);
        k.aendern(|w| w.einfacher_klick = true);
        k.aendern(|w| w.einfacher_klick = true);
        assert_eq!(*gesehen.borrow(), 1);
        assert!(
            std::fs::read_to_string(&ini)
                .unwrap()
                .contains("EinfacherKlick=true")
        );
        // second start reads the ini, no second migration
        std::fs::write(&files, "show_hidden = false\n").unwrap();
        let k = Konfig::laden_von(&ini, &files);
        assert!(k.werte().verborgene && k.werte().einfacher_klick);
        assert_eq!(
            std::fs::read_to_string(&files).unwrap(),
            "show_hidden = false\n"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
