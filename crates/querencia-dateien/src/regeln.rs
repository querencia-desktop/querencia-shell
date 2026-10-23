//! Rules without a display: zoom levels, sort order, type-ahead, status
//! bar texts, sizes and the buttons of the path bar. Everything here is
//! tested in `cargo test`; parts come from the icon view prototype
//! (`dateien/prototyp-2`, `regeln.rs`).

use std::cmp::Ordering;
use std::path::{Path, PathBuf};

use gtk4::glib;

use crate::i18n::{decimal_separator, gettext, gettext_f, ngettext_f};

/// Nautilus 2.30's zoom levels in percent (`nautilus-icon-info.h`); 100 % is
/// the 48 px icon of the icon view, the list view starts at 50 %.
pub const ZOOMSTUFEN: [u32; 7] = [33, 50, 66, 100, 150, 200, 400];

/// A zoom level of a view, in percent (one of [`ZOOMSTUFEN`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Zoom(pub u32);

impl Zoom {
    pub const NORMAL: Zoom = Zoom(100);
    /// Nautilus 2.30's standard zoom of the list view (`smaller`).
    pub const LISTE: Zoom = Zoom(50);

    /// Icon size in px at this level, as Nautilus 2.30: 16, 24, 32, 48, 72,
    /// 96, 192.
    pub fn symbolgroesse(self) -> i32 {
        match self.0 {
            0..=33 => 16,
            34..=50 => 24,
            51..=66 => 32,
            67..=100 => 48,
            101..=150 => 72,
            151..=200 => 96,
            _ => 192,
        }
    }

    /// The level `schritt` steps (+1 larger, −1 smaller) from this one,
    /// within `stufen` (ascending); a level outside snaps to the nearest.
    pub fn schritt(self, schritt: i32, stufen: &[u32]) -> Zoom {
        if stufen.is_empty() {
            return self;
        }
        let jetzt = stufen
            .iter()
            .position(|p| *p >= self.0)
            .unwrap_or(stufen.len() - 1) as i64;
        let neu = (jetzt + i64::from(schritt)).clamp(0, stufen.len() as i64 - 1);
        Zoom(stufen[neu as usize])
    }

    /// "100 %" (German with a narrow space as Nautilus' de.po would; the
    /// form comes from the catalog).
    pub fn text(self) -> String {
        // TRANSLATORS: zoom level in the toolbar, deutsch „{percent} %“
        gettext_f("{percent}%", &[("percent", &self.0.to_string())])
    }
}

/// The kinds of views the window switches between (View menu, toolbar).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Ansichtsart {
    Symbole,
    Liste,
    /// Beta (B §1.5).
    Kompakt,
}

impl Ansichtsart {
    pub const ALLE: [Ansichtsart; 3] = [
        Ansichtsart::Symbole,
        Ansichtsart::Liste,
        Ansichtsart::Kompakt,
    ];

    /// Target of the action `win.ansicht` and value in `dateien.ini`.
    pub fn kennung(self) -> &'static str {
        match self {
            Ansichtsart::Symbole => "symbole",
            Ansichtsart::Liste => "liste",
            Ansichtsart::Kompakt => "kompakt",
        }
    }

    pub fn aus_kennung(kennung: &str) -> Option<Self> {
        Self::ALLE.into_iter().find(|a| a.kennung() == kennung)
    }

    /// Name in the toolbar's drop-down ("Symbolansicht").
    pub fn name(self) -> String {
        match self {
            Ansichtsart::Symbole => gettext("Icon View"),
            Ansichtsart::Liste => gettext("List View"),
            Ansichtsart::Kompakt => gettext("Compact View"),
        }
    }

    /// The zoom a view of this kind starts with (settings may change it).
    pub fn vorgabe_zoom(self) -> Zoom {
        match self {
            Ansichtsart::Symbole => Zoom::NORMAL,
            Ansichtsart::Liste | Ansichtsart::Kompakt => Zoom::LISTE,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kriterium {
    Name,
    Groesse,
    Typ,
    Datum,
}

impl Kriterium {
    pub const ALLE: [Kriterium; 4] = [
        Kriterium::Name,
        Kriterium::Groesse,
        Kriterium::Typ,
        Kriterium::Datum,
    ];

    /// Target of the action `win.anordnen` and value in `dateien.ini`.
    pub fn kennung(self) -> &'static str {
        match self {
            Kriterium::Name => "name",
            Kriterium::Groesse => "groesse",
            Kriterium::Typ => "typ",
            Kriterium::Datum => "datum",
        }
    }

    pub fn aus_kennung(kennung: &str) -> Option<Self> {
        Self::ALLE.into_iter().find(|k| k.kennung() == kennung)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sortierung {
    pub kriterium: Kriterium,
    pub ordner_zuerst: bool,
    pub umgekehrt: bool,
}

impl Default for Sortierung {
    /// Nautilus 2.30's defaults: by name, ascending, folders first.
    fn default() -> Self {
        Self {
            kriterium: Kriterium::Name,
            ordner_zuerst: true,
            umgekehrt: false,
        }
    }
}

/// What the sorter compares of an entry; the keys are computed once when
/// the entry is made.
#[derive(Debug)]
pub struct Sortdaten {
    pub ist_ordner: bool,
    pub name: glib::FilenameCollationKey,
    pub typ: glib::CollationKey,
    pub groesse: u64,
    pub zeit: i64,
    /// The file name, to break ties the same way every time.
    pub dateiname: String,
}

/// Nautilus 2.30's order (`nautilus_file_compare_for_sort`): folders
/// first stay first when reversed; size larger first, date older first,
/// type by its description; ties by name, then by file name.
pub fn vergleiche(a: &Sortdaten, b: &Sortdaten, s: Sortierung) -> Ordering {
    if s.ordner_zuerst && a.ist_ordner != b.ist_ordner {
        return if a.ist_ordner {
            Ordering::Less
        } else {
            Ordering::Greater
        };
    }
    let nach_name = || {
        a.name
            .cmp(&b.name)
            .then_with(|| a.dateiname.cmp(&b.dateiname))
    };
    let ergebnis = match s.kriterium {
        Kriterium::Name => nach_name(),
        Kriterium::Groesse => {
            // Folders have no size here (Nautilus counted their items).
            if a.ist_ordner && b.ist_ordner {
                nach_name()
            } else {
                b.groesse.cmp(&a.groesse).then_with(nach_name)
            }
        }
        Kriterium::Typ => a.typ.cmp(&b.typ).then_with(nach_name),
        Kriterium::Datum => a.zeit.cmp(&b.zeit).then_with(nach_name),
    };
    if s.umgekehrt {
        ergebnis.reverse()
    } else {
        ergebnis
    }
}

/// The form of a name the type-ahead compares (case-insensitive).
pub fn suchform(name: &str) -> String {
    name.to_lowercase()
}

/// A size as Nautilus 2.30's status bar shows it ("12,3 kB", "3,4 GB",
/// "512 Bytes"): decimal units as GLib's `g_format_size`, one decimal, the
/// decimal separator of the interface language.
pub fn groesse(bytes: u64) -> String {
    if bytes < 1000 {
        let n = u32::try_from(bytes).unwrap_or(u32::MAX);
        return ngettext_f("{n} byte", "{n} bytes", n, &[]);
    }
    const EINHEITEN: [&str; 6] = ["kB", "MB", "GB", "TB", "PB", "EB"];
    let mut wert = bytes as f64 / 1000.0;
    let mut einheit = 0;
    while wert >= 999.95 && einheit + 1 < EINHEITEN.len() {
        wert /= 1000.0;
        einheit += 1;
    }
    let zahl = format!("{wert:.1}").replace('.', &decimal_separator());
    format!("{zahl} {}", EINHEITEN[einheit])
}

/// "12 items" as Nautilus 2.30 says it.
pub fn objekte(anzahl: u32) -> String {
    ngettext_f("{n} item", "{n} items", anzahl, &[])
}

/// Status bar without selection: "12 Objekte, freier Speicherplatz: 3,4 GB".
pub fn status_ohne_auswahl(anzahl: u32, frei: Option<u64>) -> String {
    match frei {
        Some(frei) => gettext_f(
            "{items}, Free space: {size}",
            &[("items", &objekte(anzahl)), ("size", &groesse(frei))],
        ),
        None => objekte(anzahl),
    }
}

/// One selected object for the status bar.
pub struct Gewaehlt<'a> {
    pub name: &'a str,
    pub ist_ordner: bool,
    pub groesse: u64,
}

/// Status bar with a selection, as Nautilus 2.30
/// (`fm_directory_view_display_selection_info`, without item counts of
/// folders): "»a.txt« ausgewählt (12,3 kB)", "3 Objekte ausgewählt (…)",
/// "2 Ordner ausgewählt, 3 andere Objekte ausgewählt (…)".
pub fn status_mit_auswahl(gewaehlt: &[Gewaehlt]) -> String {
    let ordner =
        u32::try_from(gewaehlt.iter().filter(|g| g.ist_ordner).count()).unwrap_or(u32::MAX);
    let andere = u32::try_from(gewaehlt.len()).unwrap_or(u32::MAX) - ordner;
    let summe: u64 = gewaehlt
        .iter()
        .filter(|g| !g.ist_ordner)
        .map(|g| g.groesse)
        .sum();
    if let [eins] = gewaehlt {
        return if eins.ist_ordner {
            gettext_f("“{name}” selected", &[("name", eins.name)])
        } else {
            gettext_f(
                "“{name}” selected ({size})",
                &[("name", eins.name), ("size", &groesse(eins.groesse))],
            )
        };
    }
    let ordner_text = ngettext_f("{n} folder selected", "{n} folders selected", ordner, &[]);
    match (ordner, andere) {
        (_, 0) => ordner_text,
        (0, n) => ngettext_f(
            "{n} item selected ({size})",
            "{n} items selected ({size})",
            n,
            &[("size", &groesse(summe))],
        ),
        (_, n) => {
            let andere_text = ngettext_f(
                "{n} other item selected ({size})",
                "{n} other items selected ({size})",
                n,
                &[("size", &groesse(summe))],
            );
            // TRANSLATORS: "2 folders selected, 3 other items selected (1.2 MB)"
            gettext_f(
                "{folders}, {others}",
                &[("folders", &ordner_text), ("others", &andere_text)],
            )
        }
    }
}

/// What a path button stands for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Knopfart {
    Dateisystem,
    Persoenlich,
    Ordner,
}

/// The buttons of the path bar for `pfad`: from the home folder when the
/// path is inside it (as Nautilus 2.30), else from the file system root.
pub fn knopfpfad(pfad: &Path, home: &Path) -> Vec<(String, PathBuf, Knopfart)> {
    let mut knoepfe = Vec::new();
    let (start, rest) = match pfad.strip_prefix(home) {
        Ok(rest) => {
            let name = home
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| gettext("Home Folder"));
            knoepfe.push((name, home.to_path_buf(), Knopfart::Persoenlich));
            (home.to_path_buf(), rest.to_path_buf())
        }
        Err(_) => {
            knoepfe.push((
                gettext("File System"),
                PathBuf::from("/"),
                Knopfart::Dateisystem,
            ));
            (
                PathBuf::from("/"),
                pfad.strip_prefix("/").unwrap_or(pfad).to_path_buf(),
            )
        }
    };
    let mut bisher = start;
    for teil in rest.components() {
        bisher.push(teil);
        knoepfe.push((
            teil.as_os_str().to_string_lossy().into_owned(),
            bisher.clone(),
            Knopfart::Ordner,
        ));
    }
    knoepfe
}

#[cfg(test)]
mod tests {
    use super::*;

    fn daten(name: &str, ordner: bool, groesse: u64, zeit: i64, typ: &str) -> Sortdaten {
        Sortdaten {
            ist_ordner: ordner,
            name: glib::FilenameCollationKey::from(name),
            typ: glib::CollationKey::from(typ),
            groesse,
            zeit,
            dateiname: name.to_owned(),
        }
    }

    fn sortiert(liste: &mut [Sortdaten], s: Sortierung) -> Vec<String> {
        liste.sort_by(|a, b| vergleiche(a, b, s));
        liste.iter().map(|d| d.dateiname.clone()).collect()
    }

    fn probe() -> Vec<Sortdaten> {
        vec![
            daten("datei10.txt", false, 10, 3, "Text"),
            daten("Briefe", true, 0, 5, "Ordner"),
            daten("datei2.txt", false, 300, 1, "Text"),
            daten("bild.png", false, 200, 2, "Bild"),
            daten("Archiv", true, 0, 4, "Ordner"),
        ]
    }

    #[test]
    fn folders_first_then_natural_name_order() {
        let mut l = probe();
        assert_eq!(
            sortiert(&mut l, Sortierung::default()),
            ["Archiv", "Briefe", "bild.png", "datei2.txt", "datei10.txt"]
        );
    }

    #[test]
    fn reversed_keeps_folders_first() {
        let mut l = probe();
        let s = Sortierung {
            umgekehrt: true,
            ..Sortierung::default()
        };
        assert_eq!(
            sortiert(&mut l, s),
            ["Briefe", "Archiv", "datei10.txt", "datei2.txt", "bild.png"]
        );
    }

    #[test]
    fn by_size_larger_first_and_by_date_older_first() {
        let mut l = probe();
        let s = Sortierung {
            kriterium: Kriterium::Groesse,
            ..Sortierung::default()
        };
        assert_eq!(
            sortiert(&mut l, s),
            ["Archiv", "Briefe", "datei2.txt", "bild.png", "datei10.txt"]
        );
        let s = Sortierung {
            kriterium: Kriterium::Datum,
            ordner_zuerst: false,
            umgekehrt: false,
        };
        assert_eq!(
            sortiert(&mut l, s),
            ["datei2.txt", "bild.png", "datei10.txt", "Archiv", "Briefe"]
        );
    }

    #[test]
    fn by_type_description() {
        let mut l = probe();
        let s = Sortierung {
            kriterium: Kriterium::Typ,
            ordner_zuerst: false,
            umgekehrt: false,
        };
        assert_eq!(
            sortiert(&mut l, s),
            ["bild.png", "Archiv", "Briefe", "datei2.txt", "datei10.txt"]
        );
    }

    #[test]
    fn zoom_steps_stay_within_the_levels() {
        let alle = &ZOOMSTUFEN[..];
        assert_eq!(Zoom(100).schritt(1, alle), Zoom(150));
        assert_eq!(Zoom(100).schritt(-1, alle), Zoom(66));
        assert_eq!(Zoom(400).schritt(1, alle), Zoom(400));
        assert_eq!(Zoom(33).schritt(-1, alle), Zoom(33));
        let mitte = &[50, 75, 100, 150, 200];
        assert_eq!(Zoom(66).schritt(0, mitte), Zoom(75));
        assert_eq!(Zoom(400).schritt(-1, mitte), Zoom(150));
        assert_eq!(Zoom(100).symbolgroesse(), 48);
        assert_eq!(Zoom(50).symbolgroesse(), 24);
        assert_eq!(Zoom(400).symbolgroesse(), 192);
    }

    #[test]
    fn kinds_and_criteria_round_trip_their_keys() {
        for a in Ansichtsart::ALLE {
            assert_eq!(Ansichtsart::aus_kennung(a.kennung()), Some(a));
        }
        for k in Kriterium::ALLE {
            assert_eq!(Kriterium::aus_kennung(k.kennung()), Some(k));
        }
        assert_eq!(Ansichtsart::aus_kennung("spalten"), None);
    }

    #[test]
    fn sizes_and_status_texts_in_english_without_catalog() {
        assert_eq!(groesse(512), "512 bytes");
        assert_eq!(groesse(1), "1 byte");
        assert_eq!(groesse(12_345), "12.3 kB");
        assert_eq!(groesse(3_400_000_000), "3.4 GB");
        assert_eq!(groesse(999_960), "1.0 MB");
        assert_eq!(status_ohne_auswahl(1, None), "1 item");
        assert_eq!(
            status_ohne_auswahl(12, Some(3_400_000_000)),
            "12 items, Free space: 3.4 GB"
        );
        let a = Gewaehlt {
            name: "a.txt",
            ist_ordner: false,
            groesse: 12_345,
        };
        let d = Gewaehlt {
            name: "Briefe",
            ist_ordner: true,
            groesse: 0,
        };
        assert_eq!(status_mit_auswahl(&[a]), "“a.txt” selected (12.3 kB)");
        let a = || Gewaehlt {
            name: "a.txt",
            ist_ordner: false,
            groesse: 1000,
        };
        assert_eq!(status_mit_auswahl(&[d]), "“Briefe” selected");
        let d = || Gewaehlt {
            name: "Briefe",
            ist_ordner: true,
            groesse: 0,
        };
        assert_eq!(
            status_mit_auswahl(&[a(), a(), a()]),
            "3 items selected (3.0 kB)"
        );
        assert_eq!(status_mit_auswahl(&[d(), d()]), "2 folders selected");
        assert_eq!(
            status_mit_auswahl(&[d(), d(), a()]),
            "2 folders selected, 1 other item selected (1.0 kB)"
        );
    }

    #[test]
    fn path_buttons_from_home_or_root() {
        let home = Path::new("/home/u");
        let k = knopfpfad(Path::new("/home/u/Dokumente/Rechnungen"), home);
        let namen: Vec<_> = k.iter().map(|(n, _, _)| n.as_str()).collect();
        assert_eq!(namen, ["u", "Dokumente", "Rechnungen"]);
        assert_eq!(k[0].2, Knopfart::Persoenlich);
        assert_eq!(k[2].1, PathBuf::from("/home/u/Dokumente/Rechnungen"));
        let k = knopfpfad(Path::new("/usr/share"), home);
        let namen: Vec<_> = k.iter().map(|(n, _, _)| n.as_str()).collect();
        assert_eq!(namen, ["File System", "usr", "share"]);
        assert_eq!(knopfpfad(Path::new("/"), home).len(), 1);
    }
}
