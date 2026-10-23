//! Rename in place (F2, „Umbenennen …“, a slow second click on the name):
//! the rules of the field, the rename lease that belongs to the object
//! instead of a cell, and the rename itself through GIO.
//!
//! Recycled cells (Recherche C §9, prototype report §5 A7): GTK takes the
//! focus away from a field whose cell scrolls out of view and gives the
//! cell to another object. The lease keeps text and selection; the views
//! park the field when the cell goes and put it back when the object's
//! cell comes again.

use gtk4::prelude::*;
use gtk4::{gio, glib};

use querencia_dateiarbeit::namen::{self, Problem};

use crate::ansichtskern::Eintrag;
use crate::i18n::{gettext, gettext_f, pgettext};

/// The characters the field selects at first, as (start, end): a file's
/// name without its extension („bericht.final“ of „bericht.final.odt“),
/// the whole name of a folder (Nautilus 2.30, Desktop `selected_chars`).
pub fn markierung(name: &str, ist_datei: bool) -> (i32, i32) {
    (0, namen::selected_chars(name, ist_datei) as i32)
}

/// Why a new name was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Abgelehnt {
    Ungueltig(Problem),
    Vorhanden,
}

impl Abgelehnt {
    /// The explanation (GNOME 2's wording; the invalid names from the
    /// shared crate, as the desktop shows them).
    pub fn text(self, neu: &str) -> String {
        match self {
            Abgelehnt::Ungueltig(p) => p.text(neu),
            Abgelehnt::Vorhanden => gettext_f(
                "The name “{name}” is already used in this folder. Please use a different name.",
                &[("name", neu)],
            ),
        }
    }
}

/// The new name for `alt`, `Ok(None)` when nothing changes. `vorhanden`
/// tells whether another object of the folder has a name already.
pub fn pruefen(
    alt: &str,
    neu: &str,
    vorhanden: impl Fn(&str) -> bool,
) -> Result<Option<String>, Abgelehnt> {
    // A line break cannot come from the keyboard (Enter ends the field)
    // but from pasting; GNOME 2 dropped it as well.
    let neu: String = neu.chars().filter(|c| *c != '\n' && *c != '\r').collect();
    if neu == alt {
        return Ok(None);
    }
    namen::check_file_name(&neu).map_err(Abgelehnt::Ungueltig)?;
    if vorhanden(&neu) {
        return Err(Abgelehnt::Vorhanden);
    }
    Ok(Some(neu))
}

/// The editing widget: several centred lines under the icon (a one-line
/// entry scrolls long names sideways, prototype §5 A7), one line in the
/// list.
#[derive(Clone)]
pub enum Feld {
    Mehrzeilig(gtk4::TextView),
    Einzeilig(gtk4::Text),
}

impl PartialEq for Feld {
    fn eq(&self, other: &Self) -> bool {
        self.widget() == other.widget()
    }
}

impl Feld {
    pub fn mehrzeilig(breite: i32) -> Self {
        let feld = gtk4::TextView::new();
        feld.set_wrap_mode(gtk4::WrapMode::WordChar);
        feld.set_justification(gtk4::Justification::Center);
        feld.set_accepts_tab(false);
        feld.set_width_request(breite);
        feld.add_css_class("umbenennen");
        Feld::Mehrzeilig(feld)
    }

    pub fn einzeilig() -> Self {
        let feld = gtk4::Text::new();
        feld.set_hexpand(true);
        feld.add_css_class("umbenennen");
        Feld::Einzeilig(feld)
    }

    pub fn widget(&self) -> gtk4::Widget {
        match self {
            Feld::Mehrzeilig(f) => f.clone().upcast(),
            Feld::Einzeilig(f) => f.clone().upcast(),
        }
    }

    pub fn text(&self) -> String {
        match self {
            Feld::Mehrzeilig(f) => {
                let p = f.buffer();
                p.text(&p.start_iter(), &p.end_iter(), false).to_string()
            }
            Feld::Einzeilig(f) => f.text().to_string(),
        }
    }

    pub fn markierung(&self) -> Option<(i32, i32)> {
        match self {
            Feld::Mehrzeilig(f) => f
                .buffer()
                .selection_bounds()
                .map(|(a, b)| (a.offset(), b.offset())),
            Feld::Einzeilig(f) => f.selection_bounds(),
        }
    }

    pub fn setzen(&self, text: &str, markierung: Option<(i32, i32)>) {
        match self {
            Feld::Mehrzeilig(f) => {
                let p = f.buffer();
                p.set_text(text);
                match markierung {
                    Some((von, bis)) => {
                        p.select_range(&p.iter_at_offset(von), &p.iter_at_offset(bis))
                    }
                    None => p.place_cursor(&p.end_iter()),
                }
            }
            Feld::Einzeilig(f) => {
                f.set_text(text);
                match markierung {
                    Some((von, bis)) => f.select_region(von, bis),
                    None => f.set_position(-1),
                }
            }
        }
    }

    /// A key that arrived while the field had no focus: delete the
    /// selection (or one character for Backspace) and insert `zeichen`.
    pub fn eingeben(&self, rueck: bool, zeichen: Option<char>) {
        match self {
            Feld::Mehrzeilig(f) => {
                let p = f.buffer();
                if !p.delete_selection(true, true) && rueck {
                    let mut stelle = p.iter_at_mark(&p.get_insert());
                    p.backspace(&mut stelle, true, true);
                }
                if let Some(c) = zeichen {
                    p.insert_at_cursor(&c.to_string());
                }
            }
            Feld::Einzeilig(f) => {
                let geloescht = f.selection_bounds().is_some();
                if geloescht {
                    f.delete_selection();
                } else if rueck {
                    let pos = f.position();
                    if pos > 0 {
                        f.delete_text(pos - 1, pos);
                    }
                }
                if let Some(c) = zeichen {
                    let mut pos = f.position();
                    f.insert_text(&c.to_string(), &mut pos);
                    f.set_position(pos);
                }
            }
        }
    }
}

/// The rename lease: the object, the text typed so far and its selection,
/// and the field with the cell it sits in while the cell shows the object.
pub struct Mietvertrag {
    pub eintrag: Eintrag,
    pub text: String,
    pub markierung: Option<(i32, i32)>,
    pub feld: Option<(Feld, gtk4::Box)>,
}

impl Mietvertrag {
    /// `ganz`: the whole name selected (Umschalt+F2), else the name
    /// without its extension.
    pub fn neu(eintrag: &Eintrag, ganz: bool) -> Self {
        let d = eintrag.daten();
        let markiert = if ganz {
            (0, d.anzeige.chars().count() as i32)
        } else {
            // a starter's shown name has no extension
            markierung(&d.anzeige, !d.ist_ordner && !d.starter)
        };
        Self {
            eintrag: eintrag.clone(),
            text: d.anzeige.clone(),
            markierung: Some(markiert),
            feld: None,
        }
    }

    /// Takes the field out of its cell, keeping text and selection.
    pub fn parken(&mut self) -> Option<gtk4::Box> {
        let (feld, zelle) = self.feld.take()?;
        self.text = feld.text();
        self.markierung = feld.markierung();
        zelle.remove(&feld.widget());
        Some(zelle)
    }

    /// The text as it stands now (field or lease).
    pub fn aktueller_text(&self) -> String {
        self.feld
            .as_ref()
            .map_or_else(|| self.text.clone(), |(f, _)| f.text())
    }
}

/// Sets the display name of `datei`; the result is the renamed file.
pub async fn ausfuehren(datei: gio::File, neu: String) -> Result<gio::File, glib::Error> {
    datei
        .set_display_name_future(&neu, glib::Priority::DEFAULT)
        .await
}

/// GNOME 2's message for a rename that did not work.
pub fn fehler_zeigen(fenster: Option<&gtk4::Window>, grund: &str) {
    let dialog = gtk4::AlertDialog::builder()
        .modal(true)
        .message(gettext("The item could not be renamed."))
        .detail(grund)
        .buttons([pgettext("dialog", "_Close").replace('_', "")])
        .build();
    dialog.show(fenster);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_leaves_the_extension_out() {
        assert_eq!(markierung("bericht.final.odt", true), (0, 13));
        assert_eq!(markierung(".bashrc", true), (0, 7));
        assert_eq!(markierung("README", true), (0, 6));
        assert_eq!(markierung("Ordner.alt", false), (0, 10));
        assert_eq!(
            markierung("Übersicht.pdf", true),
            (0, 9),
            "characters, not bytes"
        );
    }

    #[test]
    fn invalid_and_taken_names_are_refused() {
        let da = |n: &str| n == "vorhanden.txt";
        assert_eq!(pruefen("a.txt", "a.txt", da), Ok(None));
        assert_eq!(pruefen("a.txt", "b.txt", da), Ok(Some("b.txt".into())));
        assert_eq!(
            pruefen("a.txt", "", da),
            Err(Abgelehnt::Ungueltig(Problem::Empty))
        );
        assert_eq!(
            pruefen("a.txt", "   ", da),
            Err(Abgelehnt::Ungueltig(Problem::Empty))
        );
        assert_eq!(
            pruefen("a.txt", "x/y", da),
            Err(Abgelehnt::Ungueltig(Problem::Slash))
        );
        assert_eq!(
            pruefen("a.txt", "..", da),
            Err(Abgelehnt::Ungueltig(Problem::Dots))
        );
        assert_eq!(
            pruefen("a.txt", "vorhanden.txt", da),
            Err(Abgelehnt::Vorhanden)
        );
        assert_eq!(
            pruefen("a.txt", "zwei\nzeilen", da),
            Ok(Some("zweizeilen".into())),
            "pasted line breaks go"
        );
        assert_eq!(pruefen("a.txt", "a.txt\n", da), Ok(None));
    }

    #[test]
    fn refusals_explain_themselves() {
        assert!(Abgelehnt::Vorhanden.text("b.txt").contains("“b.txt”"));
        assert!(
            Abgelehnt::Ungueltig(Problem::Slash)
                .text("x/y")
                .contains("“/”")
        );
    }
}
