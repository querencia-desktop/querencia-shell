//! Rules of the tabs without a display (tested in `cargo test`): where a
//! new tab goes, which tabs Strg+Bild↑/↓ and Umschalt+Strg+Bild↑/↓ reach,
//! when the tab bar and the menu „Reiter“ show, which tab Alt+1 … Alt+0
//! means. Nautilus 2.30 (`nautilus-navigation-window-menus.c`,
//! `nautilus-notebook.c`): no wrap-around at either end, new tabs open
//! beside the current one, the tabs show from the second one on.

/// The tab bar and the menu „Reiter“ show from two tabs on.
pub fn leiste_sichtbar(anzahl: usize) -> bool {
    anzahl >= 2
}

/// Position of a new tab: right of the current one.
pub fn neue_position(aktiv: Option<usize>, anzahl: usize) -> usize {
    aktiv.map_or(anzahl, |a| (a + 1).min(anzahl))
}

/// Strg+Bild↑ (`schritt` −1) and Strg+Bild↓ (+1): the tab to show, `None`
/// at the ends (the menu entry is insensitive there).
pub fn nachbar(aktiv: usize, anzahl: usize, schritt: isize) -> Option<usize> {
    let ziel = aktiv.checked_add_signed(schritt)?;
    (ziel < anzahl && ziel != aktiv).then_some(ziel)
}

/// Alt+1 … Alt+9, Alt+0: tabs 1 to 10 (index 0 to 9).
pub fn alt_ziffer(index: usize) -> Option<char> {
    match index {
        0..=8 => char::from_digit(index as u32 + 1, 10),
        9 => Some('0'),
        _ => None,
    }
}

/// The index Alt+`ziffer` stands for, if that tab exists.
pub fn index_fuer_ziffer(ziffer: char, anzahl: usize) -> Option<usize> {
    let i = match ziffer.to_digit(10)? {
        0 => 9,
        n => n as usize - 1,
    };
    (i < anzahl).then_some(i)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_shows_from_the_second_tab() {
        assert!(!leiste_sichtbar(0));
        assert!(!leiste_sichtbar(1));
        assert!(leiste_sichtbar(2));
        assert!(leiste_sichtbar(7));
    }

    #[test]
    fn new_tabs_open_beside_the_current_one() {
        assert_eq!(neue_position(None, 0), 0);
        assert_eq!(neue_position(Some(0), 1), 1);
        assert_eq!(neue_position(Some(0), 3), 1);
        assert_eq!(neue_position(Some(2), 3), 3);
        // a stale index never points past the end
        assert_eq!(neue_position(Some(9), 3), 3);
    }

    #[test]
    fn previous_and_next_stop_at_the_ends() {
        assert_eq!(nachbar(0, 3, -1), None);
        assert_eq!(nachbar(0, 3, 1), Some(1));
        assert_eq!(nachbar(1, 3, -1), Some(0));
        assert_eq!(nachbar(2, 3, 1), None);
        assert_eq!(nachbar(0, 1, 1), None);
        assert_eq!(nachbar(0, 1, -1), None);
    }

    #[test]
    fn alt_digits_reach_the_first_ten_tabs() {
        assert_eq!(alt_ziffer(0), Some('1'));
        assert_eq!(alt_ziffer(8), Some('9'));
        assert_eq!(alt_ziffer(9), Some('0'));
        assert_eq!(alt_ziffer(10), None);
        assert_eq!(index_fuer_ziffer('1', 3), Some(0));
        assert_eq!(index_fuer_ziffer('3', 3), Some(2));
        assert_eq!(index_fuer_ziffer('4', 3), None);
        assert_eq!(index_fuer_ziffer('0', 10), Some(9));
        assert_eq!(index_fuer_ziffer('0', 9), None);
        assert_eq!(index_fuer_ziffer('x', 9), None);
        for i in 0..10 {
            assert_eq!(index_fuer_ziffer(alt_ziffer(i).unwrap(), 10), Some(i));
        }
    }
}
