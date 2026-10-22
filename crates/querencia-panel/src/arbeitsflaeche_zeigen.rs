//! „Arbeitsfläche anzeigen“ (G2-AL, QD-078), after the GNOME 2 / MATE
//! show-desktop button at the left end of the bottom panel: a click
//! minimizes every window of the current workspace, a second click restores
//! exactly the windows the first one minimized, as long as nothing else was
//! activated in between. The same switch is offered on D-Bus
//! (`org.querencia.Panel.ArbeitsflaecheUmschalten`, `neuladen.rs`), which
//! the labwc default binds to Super+D.
//!
//! The windows and their requests are the window list's
//! (`window_list.rs`, wlr-foreign-toplevel on its own connection); this
//! module holds the rule ([`Merkliste`]) and the button. Without a window
//! list (none in `panel.ini`, or no foreign-toplevel protocol) the button
//! does nothing but say so in the log.
//!
//! labwc 0.9.8 (`src/view.c`, `view_minimize`): minimizing the active
//! window focuses the topmost one left, so while the windows of a click go
//! one after the other, each of them may be activated for a moment; an
//! activation of a window that is still to be minimized therefore does not
//! end the list. Restoring a window raises and focuses it, so the windows
//! come back in the order they were last active: the last one active ends
//! on top with the focus, as before the click.

use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::Hash;

use gtk4::prelude::*;
use gtk4::{ToggleButton, glib};

use crate::fensterliste::Ereignis;
use crate::i18n::gettext;
use crate::logo;

/// CSS class of the button (style in `main.rs`).
pub const CLASS: &str = "querencia-show-desktop";
/// Theme icon of the button, as in the GNOME 2 panel.
pub const ICON: &str = "user-desktop";

/// What a click asks of the window list, in this order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Schritt<K> {
    /// Minimize these windows.
    Minimieren(Vec<K>),
    /// Restore these windows, the last one active last.
    Wiederherstellen(Vec<K>),
    /// No window to minimize or restore.
    Nichts,
}

/// The windows the last click minimized, and when each open window was
/// last active. `K` is the window list's key of a window.
#[derive(Debug)]
pub struct Merkliste<K> {
    /// Minimized by the last click, with whether labwc has reported it
    /// minimized since; empty while windows are shown.
    fenster: Vec<(K, bool)>,
    /// Order of the last activation of each open window (higher: later).
    zuletzt: HashMap<K, u64>,
    zaehler: u64,
}

impl<K: Copy + Eq + Hash> Default for Merkliste<K> {
    fn default() -> Self {
        Self {
            fenster: Vec::new(),
            zuletzt: HashMap::new(),
            zaehler: 0,
        }
    }
}

impl<K: Copy + Eq + Hash> Merkliste<K> {
    /// Whether the windows of the last click are minimized and would come
    /// back with the next one.
    pub fn zeigt_arbeitsflaeche(&self) -> bool {
        !self.fenster.is_empty()
    }

    /// The click: `fenster` are the open windows of the current workspace
    /// with whether each is minimized. Restores the remembered windows that
    /// are still there and minimized; otherwise minimizes every window
    /// that is not.
    pub fn umschalten(&mut self, fenster: &[(K, bool)]) -> Schritt<K> {
        let minimiert = |key: &K| fenster.iter().any(|(k, m)| k == key && *m);
        let mut zurueck: Vec<K> = std::mem::take(&mut self.fenster)
            .into_iter()
            .map(|(key, _)| key)
            .filter(minimiert)
            .collect();
        if !zurueck.is_empty() {
            zurueck.sort_by_key(|key| self.zuletzt.get(key).copied().unwrap_or(0));
            return Schritt::Wiederherstellen(zurueck);
        }
        let weg: Vec<K> = fenster
            .iter()
            .filter(|(_, minimiert)| !minimiert)
            .map(|(key, _)| *key)
            .collect();
        if weg.is_empty() {
            return Schritt::Nichts;
        }
        self.fenster = weg.iter().map(|key| (*key, false)).collect();
        Schritt::Minimieren(weg)
    }

    /// Forgets the remembered windows: they stay as they are.
    pub fn vergessen(&mut self) {
        self.fenster.clear();
    }

    /// One event of the window list, in the order labwc sent them.
    /// Another window activated, or a change of workspace, ends the list.
    pub fn ereignis(&mut self, ereignis: Ereignis<K>) {
        match ereignis {
            Ereignis::Aktiviert(key) => {
                // labwc focuses the next window while minimizing the
                // active one (module comment): not yet minimized ones of
                // the list may be activated on their way; that neither
                // ends the list nor changes their order
                let unterwegs = self
                    .fenster
                    .iter()
                    .any(|(k, gemeldet)| *k == key && !gemeldet);
                if !unterwegs {
                    self.zaehler += 1;
                    self.zuletzt.insert(key, self.zaehler);
                    self.vergessen();
                }
            }
            Ereignis::Geschlossen(key) => {
                self.zuletzt.remove(&key);
                self.fenster.retain(|(k, _)| *k != key);
            }
            Ereignis::Flaeche(_) => self.vergessen(),
            Ereignis::Erschienen(_) | Ereignis::Deaktiviert(_) | Ereignis::Bereit => {}
        }
    }

    /// After a batch of events: `minimiert` tells whether an open window is
    /// minimized (`None`: closed). A remembered window that labwc reported
    /// minimized and that is shown again (restored elsewhere, e.g. in the
    /// window list) ends the list.
    pub fn abgleichen(&mut self, minimiert: impl Fn(K) -> Option<bool>) {
        let mut wieder_da = false;
        self.fenster
            .retain_mut(|(key, gemeldet)| match minimiert(*key) {
                None => false,
                Some(true) => {
                    *gemeldet = true;
                    true
                }
                Some(false) => {
                    wieder_da |= *gemeldet;
                    true
                }
            });
        if wieder_da {
            self.vergessen();
        }
    }
}

/// The tooltip for the state.
/// The tooltip for the state: "Show Desktop" while windows are shown,
/// "Restore Windows" while the windows of the last click are minimized.
pub fn tooltip(zeigt_arbeitsflaeche: bool) -> String {
    if zeigt_arbeitsflaeche {
        gettext("Restore Windows")
    } else {
        gettext("Show Desktop")
    }
}

thread_local! {
    /// The buttons of the running panel; [`melden`] shows the state on
    /// each. Weak, so a button removed from the panel goes.
    static KNOEPFE: RefCell<Vec<glib::WeakRef<ToggleButton>>> = const { RefCell::new(Vec::new()) };
}

/// Shows on every button whether the windows of the last click are
/// minimized (pressed, tooltip „Fenster wiederherstellen“).
pub fn melden(zeigt_arbeitsflaeche: bool) {
    KNOEPFE.with(|knoepfe| {
        knoepfe.borrow_mut().retain(|weak| {
            let Some(button) = weak.upgrade() else {
                return false;
            };
            zeigen(&button, zeigt_arbeitsflaeche);
            true
        });
    });
}

fn zeigen(button: &ToggleButton, zeigt_arbeitsflaeche: bool) {
    // set_active emits "toggled", not "clicked": no new switch
    button.set_active(zeigt_arbeitsflaeche);
    button.set_tooltip_text(Some(&tooltip(zeigt_arbeitsflaeche)));
}

/// The button for a panel `panel_height` pixels high.
pub fn widget(panel_height: i32) -> ToggleButton {
    let image = crate::symbole::image_named(ICON, logo::size_for_panel(panel_height));
    let button = ToggleButton::builder()
        .has_frame(false)
        .child(&image)
        .build();
    button.add_css_class(CLASS);
    zeigen(&button, crate::window_list::zeigt_arbeitsflaeche());
    // The window list's answer decides the look, not GTK's toggling.
    button.connect_clicked(|button| {
        let (_, meldung) = crate::window_list::arbeitsflaeche_umschalten("Knopf");
        eprintln!("querencia-panel: Arbeitsfläche anzeigen: {meldung}");
        zeigen(button, crate::window_list::zeigt_arbeitsflaeche());
    });
    KNOEPFE.with(|k| k.borrow_mut().push(button.downgrade()));
    button
}

#[cfg(test)]
mod tests {
    use super::Ereignis::*;
    use super::*;

    /// Windows 1, 2, 3 open, 3 active last, none minimized.
    fn drei() -> Merkliste<u32> {
        let mut m = Merkliste::default();
        for key in [1, 2, 3] {
            m.ereignis(Erschienen(key));
            m.ereignis(Aktiviert(key));
        }
        m
    }

    /// What labwc reports while minimizing 1, 2, 3 with 3 active: the
    /// focus moves to 2, then 1, each minimized right after.
    fn minimieren_melden(m: &mut Merkliste<u32>) {
        for (weg, naechstes) in [(3, Some(2)), (2, Some(1)), (1, None)] {
            m.ereignis(Deaktiviert(weg));
            if let Some(key) = naechstes {
                m.ereignis(Aktiviert(key));
            }
        }
        m.abgleichen(|_| Some(true));
    }

    #[test]
    fn first_click_minimizes_all_second_restores_them() {
        let mut m = drei();
        let offen = [(1, false), (2, false), (3, false)];
        assert_eq!(m.umschalten(&offen), Schritt::Minimieren(vec![1, 2, 3]));
        assert!(m.zeigt_arbeitsflaeche());
        minimieren_melden(&mut m);
        assert!(m.zeigt_arbeitsflaeche(), "labwc's focus moves end nothing");
        let minimiert = [(1, true), (2, true), (3, true)];
        // 3 was active last, it comes back last (on top, focused)
        assert_eq!(
            m.umschalten(&minimiert),
            Schritt::Wiederherstellen(vec![1, 2, 3])
        );
        assert!(!m.zeigt_arbeitsflaeche());
        // restoring activates them: nothing remembered, nothing ended
        for key in [1, 2, 3] {
            m.ereignis(Aktiviert(key));
        }
        assert!(!m.zeigt_arbeitsflaeche());
    }

    #[test]
    fn restore_order_follows_the_last_activation() {
        let mut m = drei();
        // 1 activated again after 3: 1 is the one on top
        m.ereignis(Aktiviert(1));
        m.umschalten(&[(1, false), (2, false), (3, false)]);
        m.abgleichen(|_| Some(true));
        assert_eq!(
            m.umschalten(&[(1, true), (2, true), (3, true)]),
            Schritt::Wiederherstellen(vec![2, 3, 1])
        );
    }

    #[test]
    fn windows_minimized_before_stay_minimized() {
        let mut m = drei();
        // 2 was minimized by the user before the click
        assert_eq!(
            m.umschalten(&[(1, false), (2, true), (3, false)]),
            Schritt::Minimieren(vec![1, 3])
        );
        m.abgleichen(|_| Some(true));
        assert_eq!(
            m.umschalten(&[(1, true), (2, true), (3, true)]),
            Schritt::Wiederherstellen(vec![1, 3])
        );
    }

    #[test]
    fn another_activation_ends_the_list_and_the_next_click_minimizes() {
        let mut m = drei();
        m.umschalten(&[(1, false), (2, false), (3, false)]);
        minimieren_melden(&mut m);
        // a new window opens and is activated
        m.ereignis(Erschienen(4));
        m.ereignis(Aktiviert(4));
        assert!(!m.zeigt_arbeitsflaeche());
        assert_eq!(
            m.umschalten(&[(1, true), (2, true), (3, true), (4, false)]),
            Schritt::Minimieren(vec![4])
        );
    }

    #[test]
    fn a_remembered_window_restored_elsewhere_ends_the_list() {
        let mut m = drei();
        m.umschalten(&[(1, false), (2, false), (3, false)]);
        m.abgleichen(|_| Some(true));
        // the window list restores 2 (unset_minimized, activate)
        m.ereignis(Aktiviert(2));
        assert!(!m.zeigt_arbeitsflaeche(), "2 was reported minimized");
        // even without its activation: shown again
        let mut m = drei();
        m.umschalten(&[(1, false), (2, false), (3, false)]);
        m.abgleichen(|_| Some(true));
        m.abgleichen(|key| Some(key != 2));
        assert!(!m.zeigt_arbeitsflaeche());
    }

    #[test]
    fn not_yet_reported_windows_are_not_restored_elsewhere() {
        let mut m = drei();
        m.umschalten(&[(1, false), (2, false), (3, false)]);
        // first batch: only 3 is minimized so far
        m.abgleichen(|key| Some(key == 3));
        assert!(m.zeigt_arbeitsflaeche());
        m.abgleichen(|_| Some(true));
        assert!(m.zeigt_arbeitsflaeche());
    }

    #[test]
    fn closed_windows_are_dropped_and_a_workspace_change_ends_the_list() {
        let mut m = drei();
        m.umschalten(&[(1, false), (2, false), (3, false)]);
        m.abgleichen(|_| Some(true));
        m.ereignis(Geschlossen(2));
        m.abgleichen(|key| (key != 2).then_some(true));
        assert_eq!(
            m.umschalten(&[(1, true), (3, true)]),
            Schritt::Wiederherstellen(vec![1, 3])
        );
        let mut m = drei();
        m.umschalten(&[(1, false), (2, false), (3, false)]);
        m.ereignis(Flaeche(Some(2)));
        assert!(!m.zeigt_arbeitsflaeche());
        // all remembered windows closed: nothing left to restore
        let mut m = drei();
        m.umschalten(&[(1, false)]);
        m.ereignis(Geschlossen(1));
        assert!(!m.zeigt_arbeitsflaeche());
    }

    #[test]
    fn nothing_to_do_without_windows_and_a_stale_list_minimizes_again() {
        let mut m: Merkliste<u32> = Merkliste::default();
        assert_eq!(m.umschalten(&[]), Schritt::Nichts);
        assert_eq!(m.umschalten(&[(1, true)]), Schritt::Nichts);
        assert!(!m.zeigt_arbeitsflaeche());
        // the remembered windows are no longer minimized (not reported yet,
        // restored before labwc answered): minimize again
        let mut m = drei();
        m.umschalten(&[(1, false), (2, false)]);
        assert_eq!(
            m.umschalten(&[(1, false), (2, false)]),
            Schritt::Minimieren(vec![1, 2])
        );
    }

    #[test]
    fn tooltip_follows_the_state() {
        assert_eq!(tooltip(false), "Show Desktop");
        assert_eq!(tooltip(true), "Restore Windows");
    }
}
