//! Which windows the window list shows (G2-AK, B11): as in GNOME 2 only
//! those of the current workspace, or with `[window-list]
//! alle_arbeitsflaechen=ja` in `panel.ini` those of all workspaces.
//!
//! labwc 0.9.8 tells no client on which workspace a window is. The window
//! list therefore learns it
//! from what it sees on its own connection, where window and workspace
//! events arrive in the order labwc sent them ([`Zuordnung`]):
//!
//! - labwc opens a new window on the current workspace, so a window that
//!   appears while the list runs belongs there;
//! - labwc only activates windows of the current workspace (or windows on
//!   all workspaces), so an activated window belongs there too;
//! - when the workspace changes and the active window keeps the focus,
//!   it went along (Strg+Alt+Umschalt+←/→, `SendToDesktop follow="yes"`);
//!   if it loses the focus soon after, it stayed behind. Which of the two
//!   it was is only settled [`NACHZIEHEN_MS`] after the change.
//!
//! What the list cannot see: windows that were open before it started are
//! shown on every workspace until they are first activated; a window sent
//! to another workspace without following it (`follow="no"`, not bound in
//! Querencia) stays listed where it was until it is activated again; a
//! window on all workspaces is listed where it was activated last.
//! Without `ext_workspace_manager_v1` every window is shown.

use std::cell::Cell;
use std::collections::HashMap;
use std::hash::Hash;

/// How long after a workspace change the active window may still lose the
/// focus (then it stayed behind) before it counts as moved along.
pub const NACHZIEHEN_MS: u32 = 300;

thread_local! {
    /// The setting for window lists built from now on (`panel.ini`).
    static ALLE_ARBEITSFLAECHEN: Cell<bool> = const { Cell::new(
        crate::layout::WindowListSettings::DEFAULT_ALL_WORKSPACES,
    ) };
}

/// Sets what window lists built from now on show (from the layout, before
/// the panels are built).
pub fn set_alle_arbeitsflaechen(alle: bool) {
    ALLE_ARBEITSFLAECHEN.with(|a| a.set(alle));
}

/// Whether new window lists show the windows of all workspaces.
pub fn alle_arbeitsflaechen() -> bool {
    ALLE_ARBEITSFLAECHEN.with(Cell::get)
}

/// What happened, in the order the compositor reported it. `K` is the
/// window list's key of a window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ereignis<K> {
    /// A window was completed for the first time.
    Erschienen(K),
    /// A window became the active one.
    Aktiviert(K),
    /// A window stopped being the active one.
    Deaktiviert(K),
    Geschlossen(K),
    /// The current workspace (its number for the list, see [`Flaechen`]),
    /// `None` without one.
    Flaeche(Option<u64>),
    /// Everything the compositor had to report when the list connected has
    /// arrived; windows from now on are new.
    Bereit,
}

/// Workspace of each window as far as the list knows it.
#[derive(Debug)]
pub struct Zuordnung<K> {
    flaeche: HashMap<K, Option<u64>>,
    aktuelle: Option<u64>,
    aktiv: Option<K>,
    bereit: bool,
    /// Window taken along by the last workspace change, with the workspace
    /// it had before, until [`Zuordnung::festschreiben`].
    mitgenommen: Option<(K, Option<u64>)>,
}

/// What an [`Ereignis`] asks of the list.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Wirkung {
    /// What is shown may have changed.
    pub neu_filtern: bool,
    /// The workspace changed: call [`Zuordnung::festschreiben`] after
    /// [`NACHZIEHEN_MS`].
    pub festschreiben_planen: bool,
}

impl<K: Copy + Eq + Hash> Default for Zuordnung<K> {
    fn default() -> Self {
        Self {
            flaeche: HashMap::new(),
            aktuelle: None,
            aktiv: None,
            bereit: false,
            mitgenommen: None,
        }
    }
}

impl<K: Copy + Eq + Hash> Zuordnung<K> {
    pub fn anwenden(&mut self, ereignis: Ereignis<K>) -> Wirkung {
        let filtern = Wirkung {
            neu_filtern: true,
            festschreiben_planen: false,
        };
        match ereignis {
            Ereignis::Erschienen(key) => {
                let flaeche = if self.bereit { self.aktuelle } else { None };
                self.flaeche.insert(key, flaeche);
                filtern
            }
            Ereignis::Aktiviert(key) => {
                // another window took the focus: the one taken along by a
                // workspace change stayed behind
                self.zuruecknehmen(|k| k != key);
                self.aktiv = Some(key);
                self.hierher(key);
                filtern
            }
            Ereignis::Deaktiviert(key) => {
                self.zuruecknehmen(|k| k == key);
                if self.aktiv == Some(key) {
                    self.aktiv = None;
                }
                filtern
            }
            Ereignis::Geschlossen(key) => {
                self.flaeche.remove(&key);
                if self.aktiv == Some(key) {
                    self.aktiv = None;
                }
                if self.mitgenommen.is_some_and(|(k, _)| k == key) {
                    self.mitgenommen = None;
                }
                filtern
            }
            Ereignis::Flaeche(neu) => {
                if neu == self.aktuelle {
                    return Wirkung::default();
                }
                let erste = self.aktuelle.is_none();
                self.aktuelle = neu;
                self.mitgenommen = None;
                if let Some(key) = self.aktiv {
                    if erste {
                        self.hierher(key);
                    } else if neu.is_some() {
                        let vorher = self.flaeche.get(&key).copied().flatten();
                        self.mitgenommen = Some((key, vorher));
                        self.hierher(key);
                    }
                }
                Wirkung {
                    neu_filtern: true,
                    festschreiben_planen: !erste,
                }
            }
            Ereignis::Bereit => {
                self.bereit = true;
                if let Some(key) = self.aktiv {
                    self.hierher(key);
                }
                filtern
            }
        }
    }

    /// The window taken along by the last workspace change kept the focus
    /// long enough: it stays on the new workspace.
    pub fn festschreiben(&mut self) {
        self.mitgenommen = None;
    }

    /// Whether the list shows `key`: with `alle` every window; otherwise
    /// windows of the current workspace and those whose workspace is not
    /// known (never hide what cannot be placed).
    pub fn sichtbar(&self, key: K, alle: bool) -> bool {
        if alle {
            return true;
        }
        match (self.aktuelle, self.flaeche.get(&key).copied().flatten()) {
            (Some(aktuelle), Some(flaeche)) => aktuelle == flaeche,
            _ => true,
        }
    }

    /// The current workspace, `None` while unknown or without the protocol.
    pub fn aktuelle(&self) -> Option<u64> {
        self.aktuelle
    }

    fn hierher(&mut self, key: K) {
        if let (Some(aktuelle), Some(flaeche)) = (self.aktuelle, self.flaeche.get_mut(&key)) {
            *flaeche = Some(aktuelle);
        }
    }

    fn zuruecknehmen(&mut self, betrifft: impl Fn(K) -> bool) {
        if let Some((key, vorher)) = self.mitgenommen
            && betrifft(key)
        {
            self.mitgenommen = None;
            if let Some(flaeche) = self.flaeche.get_mut(&key) {
                *flaeche = vorher;
            }
        }
    }
}

/// The workspaces as far as the window list needs them: which one is
/// active. `W` is the protocol handle (a number in the tests); every
/// workspace gets a number of its own, never reused.
#[derive(Debug)]
pub struct Flaechen<W> {
    eintraege: HashMap<W, Flaeche>,
    naechste: u64,
    aktiv: Option<u64>,
}

#[derive(Debug)]
struct Flaeche {
    nummer: u64,
    name: String,
    aktiv: bool,
}

impl<W: Eq + Hash> Default for Flaechen<W> {
    fn default() -> Self {
        Self {
            eintraege: HashMap::new(),
            naechste: 0,
            aktiv: None,
        }
    }
}

impl<W: Eq + Hash> Flaechen<W> {
    pub fn neu(&mut self, handle: W) {
        self.naechste += 1;
        let flaeche = Flaeche {
            nummer: self.naechste,
            name: String::new(),
            aktiv: false,
        };
        self.eintraege.insert(handle, flaeche);
    }

    pub fn name(&mut self, handle: &W, name: String) {
        if let Some(f) = self.eintraege.get_mut(handle) {
            f.name = name;
        }
    }

    pub fn aktiv(&mut self, handle: &W, aktiv: bool) {
        if let Some(f) = self.eintraege.get_mut(handle) {
            f.aktiv = aktiv;
        }
    }

    pub fn entfernt(&mut self, handle: &W) {
        self.eintraege.remove(handle);
    }

    /// The manager's `done`: the active workspace if it changed (the one
    /// first created if, against the protocol's intent for one group,
    /// several are active).
    pub fn fertig(&mut self) -> Option<Option<u64>> {
        let aktiv = self
            .eintraege
            .values()
            .filter(|f| f.aktiv)
            .map(|f| f.nummer)
            .min();
        (aktiv != self.aktiv).then(|| {
            self.aktiv = aktiv;
            aktiv
        })
    }

    /// The name labwc gives workspace `nummer` (for the log).
    pub fn name_von(&self, nummer: u64) -> Option<&str> {
        self.eintraege
            .values()
            .find(|f| f.nummer == nummer)
            .map(|f| f.name.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::Ereignis::*;
    use super::*;

    /// Windows 1..: numbers; workspaces 10, 20.
    fn zuordnung(ereignisse: &[Ereignis<u32>]) -> Zuordnung<u32> {
        let mut z = Zuordnung::default();
        for e in ereignisse {
            z.anwenden(*e);
        }
        z
    }

    fn sichtbare(z: &Zuordnung<u32>, fenster: &[u32]) -> Vec<u32> {
        fenster
            .iter()
            .copied()
            .filter(|&k| z.sichtbar(k, false))
            .collect()
    }

    /// Start: one workspace, ready, then windows open on it.
    const START: [Ereignis<u32>; 2] = [Flaeche(Some(10)), Bereit];

    #[test]
    fn new_windows_belong_to_the_current_workspace() {
        let mut z = zuordnung(&START);
        z.anwenden(Erschienen(1));
        z.anwenden(Aktiviert(1));
        assert!(z.anwenden(Flaeche(Some(20))).festschreiben_planen);
        // 1 kept the focus for now: taken along until it loses it
        z.anwenden(Deaktiviert(1));
        z.festschreiben();
        z.anwenden(Erschienen(2));
        z.anwenden(Aktiviert(2));
        assert_eq!(sichtbare(&z, &[1, 2]), [2]);
        z.anwenden(Flaeche(Some(10)));
        z.anwenden(Deaktiviert(2));
        z.anwenden(Aktiviert(1));
        z.festschreiben();
        assert_eq!(sichtbare(&z, &[1, 2]), [1]);
        // with alle_arbeitsflaechen=ja every window
        assert!(z.sichtbar(2, true));
    }

    #[test]
    fn switching_to_an_empty_workspace_leaves_the_window_behind() {
        // labwc: workspace done, then the old window loses the focus
        let mut z = zuordnung(&START);
        z.anwenden(Erschienen(1));
        z.anwenden(Aktiviert(1));
        z.anwenden(Flaeche(Some(20)));
        assert_eq!(sichtbare(&z, &[1]), [1], "vorläufig mitgenommen");
        z.anwenden(Deaktiviert(1));
        assert_eq!(sichtbare(&z, &[1]), Vec::<u32>::new());
        z.festschreiben();
        z.anwenden(Flaeche(Some(10)));
        z.festschreiben();
        assert_eq!(sichtbare(&z, &[1]), [1]);
    }

    #[test]
    fn switching_to_a_workspace_with_windows_moves_the_focus() {
        let mut z = zuordnung(&START);
        z.anwenden(Erschienen(1));
        z.anwenden(Aktiviert(1));
        z.anwenden(Flaeche(Some(20)));
        z.anwenden(Deaktiviert(1));
        z.festschreiben();
        z.anwenden(Erschienen(2));
        z.anwenden(Aktiviert(2));
        z.anwenden(Flaeche(Some(10)));
        // labwc focuses 1, the topmost window there; 2 stays on 20
        z.anwenden(Aktiviert(1));
        z.anwenden(Deaktiviert(2));
        z.festschreiben();
        assert_eq!(sichtbare(&z, &[1, 2]), [1]);
        z.anwenden(Flaeche(Some(20)));
        z.anwenden(Aktiviert(2));
        z.anwenden(Deaktiviert(1));
        z.festschreiben();
        assert_eq!(sichtbare(&z, &[1, 2]), [2]);
    }

    #[test]
    fn events_in_either_order_give_the_same_result() {
        // the protocol does not order window and workspace events; labwc
        // 0.9.8 may report the new focus before the new workspace
        for erst_fenster in [false, true] {
            let mut z = zuordnung(&START);
            z.anwenden(Erschienen(1));
            z.anwenden(Aktiviert(1));
            z.anwenden(Flaeche(Some(20)));
            z.anwenden(Deaktiviert(1));
            z.festschreiben();
            z.anwenden(Erschienen(2));
            z.anwenden(Aktiviert(2));
            let mut schritte = vec![Flaeche(Some(10)), Aktiviert(1), Deaktiviert(2)];
            if erst_fenster {
                schritte.rotate_left(1);
            }
            for s in schritte {
                z.anwenden(s);
            }
            z.festschreiben();
            assert_eq!(
                sichtbare(&z, &[1, 2]),
                [1],
                "Fenster zuerst: {erst_fenster}"
            );
        }
    }

    #[test]
    fn a_window_taken_along_stays_on_the_new_workspace() {
        // Strg+Alt+Umschalt+→: the workspace changes, the window keeps the focus
        let mut z = zuordnung(&START);
        z.anwenden(Erschienen(1));
        z.anwenden(Erschienen(2));
        z.anwenden(Aktiviert(2));
        assert!(z.anwenden(Flaeche(Some(20))).festschreiben_planen);
        assert_eq!(sichtbare(&z, &[1, 2]), [2], "ohne Flackern sofort dort");
        z.festschreiben();
        // a later focus change does not take it back
        z.anwenden(Deaktiviert(2));
        assert_eq!(sichtbare(&z, &[1, 2]), [2]);
        z.anwenden(Flaeche(Some(10)));
        z.festschreiben();
        assert_eq!(sichtbare(&z, &[1, 2]), [1]);
    }

    #[test]
    fn windows_from_before_the_start_are_shown_until_placed() {
        // the panel (re)starts with windows open on 10 and 20
        let mut z = Zuordnung::default();
        for e in [
            Erschienen(1),
            Erschienen(2),
            Aktiviert(1),
            Flaeche(Some(10)),
            Bereit,
        ] {
            z.anwenden(e);
        }
        assert_eq!(sichtbare(&z, &[1, 2]), [1, 2]);
        z.anwenden(Flaeche(Some(20)));
        z.anwenden(Aktiviert(2));
        z.anwenden(Deaktiviert(1));
        z.festschreiben();
        // 1 was active on 10, 2 now on 20
        assert_eq!(sichtbare(&z, &[1, 2]), [2]);
        z.anwenden(Flaeche(Some(10)));
        z.anwenden(Aktiviert(1));
        z.anwenden(Deaktiviert(2));
        z.festschreiben();
        assert_eq!(sichtbare(&z, &[1, 2]), [1]);
    }

    #[test]
    fn window_opened_before_ready_is_not_guessed() {
        // windows reported in the first burst could be anywhere
        let z = zuordnung(&[Flaeche(Some(10)), Erschienen(1), Bereit, Flaeche(Some(20))]);
        assert_eq!(sichtbare(&z, &[1]), [1]);
    }

    #[test]
    fn without_workspaces_everything_is_shown() {
        let mut z = zuordnung(&[Bereit]);
        z.anwenden(Erschienen(1));
        z.anwenden(Aktiviert(1));
        assert!(z.sichtbar(1, false));
        assert_eq!(z.aktuelle(), None);
        // the protocol goes away: no workspace, show all again
        let mut z = zuordnung(&START);
        z.anwenden(Erschienen(1));
        z.anwenden(Flaeche(Some(20)));
        z.festschreiben();
        assert!(!z.sichtbar(1, false));
        z.anwenden(Flaeche(None));
        assert!(z.sichtbar(1, false));
    }

    #[test]
    fn closed_windows_are_forgotten() {
        let mut z = zuordnung(&START);
        z.anwenden(Erschienen(1));
        z.anwenden(Aktiviert(1));
        z.anwenden(Flaeche(Some(20)));
        z.anwenden(Geschlossen(1));
        z.festschreiben();
        assert!(z.flaeche.is_empty());
        assert_eq!(z.aktiv, None);
        // an unchanged workspace changes nothing
        assert_eq!(z.anwenden(Flaeche(Some(20))), Wirkung::default());
    }

    #[test]
    fn workspaces_report_the_active_one_on_done() {
        let mut f = Flaechen::default();
        f.neu(7);
        f.neu(8);
        f.name(&7, "Arbeitsfläche 1".into());
        f.aktiv(&7, true);
        assert_eq!(f.fertig(), Some(Some(1)));
        assert_eq!(f.fertig(), None, "unverändert");
        f.aktiv(&7, false);
        assert_eq!(f.fertig(), Some(None), "zwischen zwei Meldungen");
        f.aktiv(&8, true);
        assert_eq!(f.fertig(), Some(Some(2)));
        assert_eq!(f.name_von(1), Some("Arbeitsfläche 1"));
        f.entfernt(&8);
        assert_eq!(f.fertig(), Some(None));
        // a new workspace never gets an old number
        f.neu(8);
        f.aktiv(&8, true);
        assert_eq!(f.fertig(), Some(Some(3)));
    }

    #[test]
    fn standard_is_the_current_workspace_only() {
        assert!(!alle_arbeitsflaechen());
        set_alle_arbeitsflaechen(true);
        assert!(alle_arbeitsflaechen());
        set_alle_arbeitsflaechen(false);
    }
}
