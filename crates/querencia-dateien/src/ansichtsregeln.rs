//! The rules of the icon and list views without a display: zoom levels and
//! cell widths, the extra lines under icon names, type-ahead matching, the
//! list columns with their texts and the mapping between a click on a
//! column header and the sort order. Tested in `cargo test`.

use crate::ansichtskern::{Kriterium, Sortierung};
use crate::i18n::gettext;
/// What an icon shows under its name (Nautilus `icon_view/captions`), the
/// setting of the frame (Reiter „Anzeige“).
pub use crate::konfig::Zusatz;

/// The zoom levels of both views in percent: Nautilus 2.30's smaller,
/// small, standard, large and larger (33 % and 400 % stay out, B §5.1 D10).
pub const STUFEN: [u32; 5] = [50, 66, 100, 150, 200];
/// Normal size of the icon view (Strg+0, Nautilus `default_zoom_level`).
pub const ZOOM_SYMBOLE: u32 = 100;
/// Normal size of the list view (Nautilus list view: `smaller`, 50 %).
pub const ZOOM_LISTE: u32 = 50;

/// The icon size at `prozent`, as Nautilus 2.30 (`nautilus-icon-info.h`):
/// 16, 24, 32, 48, 72, 96, 192 px.
pub fn symbolgroesse(prozent: u32) -> i32 {
    match prozent {
        0..=33 => 16,
        34..=50 => 24,
        51..=66 => 32,
        67..=100 => 48,
        101..=150 => 72,
        151..=200 => 96,
        _ => 192,
    }
}

/// The level `schritt` steps (+1 larger, -1 smaller) from `prozent`,
/// within [`STUFEN`]; a value between two levels counts as the next one up.
pub fn zoom_schritt(prozent: u32, schritt: i32) -> u32 {
    let jetzt = STUFEN
        .iter()
        .position(|p| *p >= prozent)
        .unwrap_or(STUFEN.len() - 1) as i64;
    let neu = (jetzt + i64::from(schritt)).clamp(0, STUFEN.len() as i64 - 1);
    STUFEN[neu as usize]
}

/// The nearest level of [`STUFEN`] for `prozent` (settings may hold 33 or
/// 400 %).
pub fn zoom_stufe(prozent: u32) -> u32 {
    zoom_schritt(prozent, 0)
}

/// Width of an icon view cell at icon size `symbol`: room for about twelve
/// characters at 100 %, as Nautilus 2.30's text width, never narrower than
/// a short word.
pub fn zellbreite(symbol: i32) -> i32 {
    match symbol {
        s if s <= 24 => 84,
        s if s <= 32 => 96,
        s if s <= 48 => 112,
        s if s <= 72 => 140,
        _ => 172,
    }
}

/// Columns of the icon view that fit into `breite` px at icon size `symbol`
/// (at least one). GtkGridView keeps `max_columns` × about 30 rows of
/// cells alive; a fixed high value made every resort build 1,600 cells
/// (prototype report §5 A1).
pub fn spalten(breite: i32, symbol: i32) -> u32 {
    (breite / zellbreite(symbol)).max(1) as u32
}

/// The extra lines under the name at zoom `prozent`: the first setting from
/// 100 %, the second in addition from 150 % (B §3.6, Reiter „Anzeige“;
/// Nautilus 2.30 showed one caption at standard size, two at large). The
/// defaults are [`Zusatz::Nichts`] and [`Zusatz::Groesse`].
pub fn zusatzzeilen(prozent: u32, einstellung: [Zusatz; 2]) -> Vec<Zusatz> {
    let mut zeilen = Vec::new();
    if prozent >= 100 {
        zeilen.push(einstellung[0]);
    }
    if prozent >= 150 {
        zeilen.push(einstellung[1]);
    }
    zeilen.retain(|z| *z != Zusatz::Nichts);
    zeilen.dedup();
    zeilen
}

pub const ZUSATZ_VORGABE: [Zusatz; 2] = [Zusatz::Nichts, Zusatz::Groesse];

/// Lines of a name that is not selected (Nautilus 2.30: three).
pub const ZEILEN: i32 = 3;

/// The form of a name the type-ahead compares (case-insensitive).
pub fn suchform(name: &str) -> String {
    name.to_lowercase()
}

/// The next position from `start` (included) whose name begins with
/// `eingabe`, forwards or backwards, wrapping around; `name(i)` is the
/// search form of the name at `i`.
pub fn treffer(
    anzahl: u32,
    start: u32,
    vorwaerts: bool,
    eingabe: &str,
    name: impl Fn(u32) -> String,
) -> Option<u32> {
    if anzahl == 0 || eingabe.is_empty() {
        return None;
    }
    let eingabe = suchform(eingabe);
    let start = start.min(anzahl - 1);
    (0..anzahl)
        .map(|schritt| {
            if vorwaerts {
                (start + schritt) % anzahl
            } else {
                (start + anzahl - schritt) % anzahl
            }
        })
        .find(|&i| name(i).starts_with(&eingabe))
}

/// A column of the list view.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Spalte {
    Name,
    Groesse,
    Typ,
    Datum,
    Rechte,
    Eigentuemer,
    Gruppe,
}

impl Spalte {
    /// In the order the list shows them (Nautilus 2.30 default order).
    pub const ALLE: [Spalte; 7] = [
        Spalte::Name,
        Spalte::Groesse,
        Spalte::Typ,
        Spalte::Datum,
        Spalte::Rechte,
        Spalte::Eigentuemer,
        Spalte::Gruppe,
    ];

    /// Shown unless the settings say otherwise (B §3.6 „Listenspalten“).
    pub const VORGABE: [Spalte; 4] = [Spalte::Name, Spalte::Groesse, Spalte::Typ, Spalte::Datum];

    /// The key in `dateien.ini`.
    pub fn kennung(self) -> &'static str {
        match self {
            Spalte::Name => "name",
            Spalte::Groesse => "groesse",
            Spalte::Typ => "typ",
            Spalte::Datum => "datum",
            Spalte::Rechte => "rechte",
            Spalte::Eigentuemer => "eigentuemer",
            Spalte::Gruppe => "gruppe",
        }
    }

    #[cfg(test)]
    pub fn aus_kennung(kennung: &str) -> Option<Self> {
        Self::ALLE.into_iter().find(|s| s.kennung() == kennung)
    }

    /// The header text (Nautilus 2.30 wording).
    pub fn titel(self) -> String {
        match self {
            Spalte::Name => gettext("Name"),
            Spalte::Groesse => gettext("Size"),
            Spalte::Typ => gettext("Type"),
            Spalte::Datum => gettext("Date Modified"),
            Spalte::Rechte => gettext("Permissions"),
            Spalte::Eigentuemer => gettext("Owner"),
            Spalte::Gruppe => gettext("Group"),
        }
    }

    /// The sort criterion a click on the header chooses; the columns
    /// without one are not sortable in 0.1.
    pub fn kriterium(self) -> Option<Kriterium> {
        match self {
            Spalte::Name => Some(Kriterium::Name),
            Spalte::Groesse => Some(Kriterium::Groesse),
            Spalte::Typ => Some(Kriterium::Typ),
            Spalte::Datum => Some(Kriterium::Datum),
            _ => None,
        }
    }

    pub fn fuer_kriterium(k: Kriterium) -> Self {
        match k {
            Kriterium::Name => Spalte::Name,
            Kriterium::Groesse => Spalte::Groesse,
            Kriterium::Typ => Spalte::Typ,
            Kriterium::Datum => Spalte::Datum,
        }
    }
}

/// The columns to show for the setting `gewaehlt`: Name always and first,
/// the others in the fixed order (no reordering, B §3.6).
pub fn sichtbare_spalten(gewaehlt: &[Spalte]) -> Vec<Spalte> {
    Spalte::ALLE
        .into_iter()
        .filter(|s| *s == Spalte::Name || gewaehlt.contains(s))
        .collect()
}

/// The sort order after a click on the header of `spalte` (GTK reports
/// the column and the direction): another column sorts ascending by it,
/// the same column turns the direction. Folders first stays as it is.
pub fn sortierung_nach_klick(
    alt: Sortierung,
    spalte: Spalte,
    absteigend: bool,
) -> Option<Sortierung> {
    let kriterium = spalte.kriterium()?;
    Some(Sortierung {
        kriterium,
        umgekehrt: absteigend,
        ..alt
    })
}

/// `ls -l` style permissions of `modus` (st_mode), with the type letter.
pub fn rechte_text(modus: u32) -> String {
    let art = match modus & 0o170000 {
        0o040000 => 'd',
        0o120000 => 'l',
        0o020000 => 'c',
        0o060000 => 'b',
        0o010000 => 'p',
        0o140000 => 's',
        _ => '-',
    };
    let mut text = String::with_capacity(10);
    text.push(art);
    let bits = [
        (0o400, 'r'),
        (0o200, 'w'),
        (0o100, 'x'),
        (0o040, 'r'),
        (0o020, 'w'),
        (0o010, 'x'),
        (0o004, 'r'),
        (0o002, 'w'),
        (0o001, 'x'),
    ];
    for (i, (bit, zeichen)) in bits.into_iter().enumerate() {
        let gesetzt = modus & bit != 0;
        // setuid, setgid and sticky replace the x of their group
        let besonders = match i {
            2 => modus & 0o4000 != 0,
            5 => modus & 0o2000 != 0,
            8 => modus & 0o1000 != 0,
            _ => false,
        };
        let c = match (besonders, gesetzt) {
            (true, true) if i == 8 => 't',
            (true, false) if i == 8 => 'T',
            (true, true) => 's',
            (true, false) => 'S',
            (false, true) => zeichen,
            (false, false) => '-',
        };
        text.push(c);
    }
    text
}

/// The size column: files with GLib's size text, folders empty (counting
/// items comes with the „Vorschau“ setting in the Beta).
pub fn groesse_text(ist_ordner: bool, groesse: u64, format: impl Fn(u64) -> String) -> String {
    if ist_ordner {
        String::new()
    } else {
        format(groesse)
    }
}

/// strftime pattern of the date column: the region's date and the time
/// without seconds („23.10.2026 01:15“), or ISO 8601.
pub fn datumsmuster(iso: bool) -> &'static str {
    if iso { "%Y-%m-%d %H:%M" } else { "%x %H:%M" }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_levels_are_nautilus_sizes_and_steps_stay_inside() {
        assert_eq!(symbolgroesse(ZOOM_SYMBOLE), 48);
        assert_eq!(symbolgroesse(ZOOM_LISTE), 24);
        let groessen: Vec<i32> = STUFEN.iter().map(|p| symbolgroesse(*p)).collect();
        assert_eq!(groessen, [24, 32, 48, 72, 96]);
        assert_eq!(zoom_schritt(100, 1), 150);
        assert_eq!(zoom_schritt(100, -1), 66);
        assert_eq!(zoom_schritt(200, 1), 200);
        assert_eq!(zoom_schritt(50, -1), 50);
        assert_eq!(
            zoom_schritt(75, 0),
            100,
            "between two levels: the next one up"
        );
        assert_eq!(zoom_stufe(33), 50);
        assert_eq!(zoom_stufe(400), 200);
        assert_eq!(symbolgroesse(400), 192);
        let breiten: Vec<i32> = STUFEN
            .iter()
            .map(|p| zellbreite(symbolgroesse(*p)))
            .collect();
        assert!(breiten.windows(2).all(|w| w[0] < w[1]), "{breiten:?}");
        assert!(
            STUFEN
                .iter()
                .all(|p| zellbreite(symbolgroesse(*p)) > symbolgroesse(*p))
        );
    }

    #[test]
    fn columns_fit_the_width() {
        assert_eq!(spalten(800, 48), 7);
        assert_eq!(spalten(1920, 48), 17);
        assert_eq!(spalten(50, 96), 1, "at least one column");
        assert_eq!(spalten(0, 24), 1);
        assert!(spalten(1000, 24) > spalten(1000, 96));
    }

    #[test]
    fn extra_lines_follow_zoom_and_setting() {
        let v = ZUSATZ_VORGABE;
        assert!(zusatzzeilen(50, v).is_empty());
        assert!(zusatzzeilen(100, v).is_empty());
        assert_eq!(zusatzzeilen(150, v), [Zusatz::Groesse]);
        assert_eq!(zusatzzeilen(200, v), [Zusatz::Groesse]);
        let beide = [Zusatz::Datum, Zusatz::Typ];
        assert_eq!(zusatzzeilen(66, beide), []);
        assert_eq!(zusatzzeilen(100, beide), [Zusatz::Datum]);
        assert_eq!(zusatzzeilen(150, beide), [Zusatz::Datum, Zusatz::Typ]);
        let doppelt = [Zusatz::Groesse, Zusatz::Groesse];
        assert_eq!(zusatzzeilen(150, doppelt), [Zusatz::Groesse]);
        for z in Zusatz::ALLE {
            assert_eq!(Zusatz::aus_kennung(z.kennung()), Some(z));
        }
    }

    #[test]
    fn type_ahead_finds_prefixes_case_insensitive_and_wraps() {
        let namen = ["Ablage", "bericht.odt", "Bilder", "brief.txt", "Übersicht"];
        let name = |i: u32| suchform(namen[i as usize]);
        assert_eq!(treffer(5, 0, true, "b", name), Some(1));
        assert_eq!(treffer(5, 0, true, "BR", name), Some(3));
        assert_eq!(treffer(5, 2, true, "b", name), Some(2));
        assert_eq!(treffer(5, 4, true, "b", name), Some(1), "wraps around");
        assert_eq!(treffer(5, 0, false, "b", name), Some(3), "backwards");
        assert_eq!(treffer(5, 0, true, "ü", name), Some(4));
        assert_eq!(treffer(5, 0, true, "x", name), None);
        assert_eq!(treffer(0, 0, true, "a", name), None);
        assert_eq!(treffer(5, 0, true, "", name), None);
    }

    #[test]
    fn columns_name_first_fixed_order() {
        assert_eq!(sichtbare_spalten(&Spalte::VORGABE), Spalte::VORGABE);
        assert_eq!(
            sichtbare_spalten(&[Spalte::Gruppe, Spalte::Groesse]),
            [Spalte::Name, Spalte::Groesse, Spalte::Gruppe]
        );
        assert_eq!(sichtbare_spalten(&[]), [Spalte::Name]);
        for s in Spalte::ALLE {
            assert_eq!(Spalte::aus_kennung(s.kennung()), Some(s));
            if let Some(k) = s.kriterium() {
                assert_eq!(Spalte::fuer_kriterium(k), s);
            }
        }
        assert_eq!(Spalte::Datum.titel(), "Date Modified");
    }

    #[test]
    fn header_click_sorts_and_turns_direction() {
        let alt = Sortierung::default();
        let nach_groesse = sortierung_nach_klick(alt, Spalte::Groesse, false).unwrap();
        assert_eq!(nach_groesse.kriterium, Kriterium::Groesse);
        assert!(!nach_groesse.umgekehrt);
        assert!(nach_groesse.ordner_zuerst, "folders first stays");
        let zurueck = sortierung_nach_klick(nach_groesse, Spalte::Groesse, true).unwrap();
        assert!(zurueck.umgekehrt);
        assert_eq!(sortierung_nach_klick(alt, Spalte::Rechte, false), None);
        let ohne = Sortierung {
            ordner_zuerst: false,
            ..alt
        };
        assert!(
            !sortierung_nach_klick(ohne, Spalte::Typ, false)
                .unwrap()
                .ordner_zuerst
        );
    }

    #[test]
    fn permissions_as_ls_shows_them() {
        assert_eq!(rechte_text(0o100644), "-rw-r--r--");
        assert_eq!(rechte_text(0o040755), "drwxr-xr-x");
        assert_eq!(rechte_text(0o120777), "lrwxrwxrwx");
        assert_eq!(rechte_text(0o104755), "-rwsr-xr-x");
        assert_eq!(rechte_text(0o041777), "drwxrwxrwt");
        assert_eq!(rechte_text(0o041776), "drwxrwxrwT");
        assert_eq!(rechte_text(0o102640), "-rw-r-S---");
        assert_eq!(rechte_text(0o100000), "----------");
    }

    #[test]
    fn size_and_date_texts() {
        let f = |n: u64| format!("{n} B");
        assert_eq!(groesse_text(true, 4096, f), "");
        assert_eq!(groesse_text(false, 12, f), "12 B");
        assert_eq!(datumsmuster(true), "%Y-%m-%d %H:%M");
        assert!(datumsmuster(false).starts_with("%x"));
    }
}
