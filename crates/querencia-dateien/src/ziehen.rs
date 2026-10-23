//! Ziehen und Ablegen as Nautilus 2.30 did it.
//!
//! - **Source:** a `gtk::DragSource` on a view; the files under the pointer
//!   (the whole selection when the pointer is on it) leave as a
//!   `GdkFileList` and `text/uri-list` – what the desktop, Querencia Files
//!   and file dialogs take.
//! - **Target:** a `gtk::DropTargetAsync` on views (folder under the
//!   pointer, else the shown folder), sidebar rows and path buttons.
//! - **Which action** ([`entscheiden`]): Strg copies, Umschalt moves,
//!   Strg+Umschalt links; without a key the same file system moves and
//!   another one copies (`id::filesystem` of source and target). Out of the
//!   trash only moving; onto the trash only "In den Papierkorb". Into the
//!   folder the files are in already, or a folder into itself: nothing.
//! - **Wayland (labwc 0.9.8):** the compositor (wlroots) picks the action
//!   from what source and target offer, the target's preferred one first; it
//!   sends no modifier state of its own. So the source fixes Strg/Umschalt
//!   when the drag begins (as Desktop 0.2.3), the target reads its own
//!   modifier state while it has the keyboard and prefers the action of the
//!   rule, and the drop is finished with the action really done (a source
//!   that deletes after MOVE must not lose a copied file).
//! - The target does the work (the job queue); a source never deletes
//!   after a drag, as GTK expects.

use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{gdk, gio, glib};
use querencia_dateiarbeit::papierkorb;

use crate::ansicht::Ansicht;
use crate::eintrag::Eintrag;
use crate::fenster::Fenster;
use crate::operationen::{Operationen, debugging, protokoll};
use crate::zwischenablage;

/// What a drop does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ablage {
    Kopieren,
    Verschieben,
    Verknuepfen,
    Papierkorb,
}

/// Where a drop goes: a folder (the trash is `trash:///` or the home
/// trash's `files` folder, [`im_papierkorb`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ziel {
    Ordner(gio::File),
}

/// The actions a source offers (`gdk::DragAction`, without GTK for tests).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Angebot {
    pub kopieren: bool,
    pub verschieben: bool,
    pub verknuepfen: bool,
}

impl Angebot {
    #[cfg(test)]
    pub const ALLE: Angebot = Angebot {
        kopieren: true,
        verschieben: true,
        verknuepfen: true,
    };

    pub fn aus(actions: gdk::DragAction) -> Self {
        Self {
            kopieren: actions.contains(gdk::DragAction::COPY),
            verschieben: actions.contains(gdk::DragAction::MOVE),
            verknuepfen: actions.contains(gdk::DragAction::LINK),
        }
    }

    fn hat(self, ablage: Ablage) -> bool {
        match ablage {
            Ablage::Kopieren => self.kopieren,
            Ablage::Verschieben | Ablage::Papierkorb => self.verschieben,
            Ablage::Verknuepfen => self.verknuepfen,
        }
    }
}

/// Everything [`entscheiden`] looks at.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Lage {
    pub strg: bool,
    pub umschalt: bool,
    /// Every source is on the target's file system.
    pub gleiches_dateisystem: bool,
    /// A source lies in the trash.
    pub aus_papierkorb: bool,
    pub ziel_papierkorb: bool,
    /// Every source is in the target folder already.
    pub schon_dort: bool,
    /// The target is one of the sources (or inside one).
    pub in_sich_selbst: bool,
    pub angebot: Angebot,
}

/// The Nautilus 2.30 rule (module text); `None`: no drop here.
pub fn entscheiden(lage: Lage) -> Option<Ablage> {
    let erlaubt = |ablage: Ablage| lage.angebot.hat(ablage).then_some(ablage);
    if lage.ziel_papierkorb {
        return if lage.aus_papierkorb {
            None
        } else {
            erlaubt(Ablage::Papierkorb)
        };
    }
    if lage.in_sich_selbst {
        return None;
    }
    if lage.aus_papierkorb {
        // QD-113: out of the trash only moving, whatever the keys say
        return if lage.schon_dort {
            None
        } else {
            erlaubt(Ablage::Verschieben)
        };
    }
    let gewollt = match (lage.strg, lage.umschalt) {
        (true, true) => Some(Ablage::Verknuepfen),
        (true, false) => Some(Ablage::Kopieren),
        (false, true) => Some(Ablage::Verschieben),
        (false, false) => None,
    };
    if let Some(gewollt) = gewollt {
        // a move into the same folder changes nothing
        if gewollt == Ablage::Verschieben && lage.schon_dort {
            return None;
        }
        return erlaubt(gewollt);
    }
    if lage.schon_dort {
        return None;
    }
    let (erst, dann) = if lage.gleiches_dateisystem {
        (Ablage::Verschieben, Ablage::Kopieren)
    } else {
        (Ablage::Kopieren, Ablage::Verschieben)
    };
    // a source that offers only one action (Strg/Umschalt there) decides
    erlaubt(erst).or_else(|| erlaubt(dann))
}

/// What a source offers for the keys held when the drag begins.
pub fn quell_aktionen(strg: bool, umschalt: bool, aus_papierkorb: bool) -> gdk::DragAction {
    if aus_papierkorb {
        return gdk::DragAction::MOVE;
    }
    match (strg, umschalt) {
        (true, true) => gdk::DragAction::LINK,
        (true, false) => gdk::DragAction::COPY,
        (false, true) => gdk::DragAction::MOVE,
        (false, false) => gdk::DragAction::COPY | gdk::DragAction::MOVE | gdk::DragAction::LINK,
    }
}

pub fn als_action(ablage: Ablage) -> gdk::DragAction {
    match ablage {
        Ablage::Kopieren => gdk::DragAction::COPY,
        Ablage::Verschieben | Ablage::Papierkorb => gdk::DragAction::MOVE,
        Ablage::Verknuepfen => gdk::DragAction::LINK,
    }
}

/// Whether `file` is the trash or in it: `trash:///…`, or without GVfs the
/// home trash's `files` folder and what is in it.
pub fn im_papierkorb(file: &gio::File) -> bool {
    if file.uri_scheme().is_some_and(|s| s == "trash") {
        return true;
    }
    let files = gio::File::for_path(papierkorb::home_trash().join("files"));
    file.equal(&files) || file.has_prefix(&files)
}

/// `id::filesystem` of a local file (for GVfs places `None`: asking could
/// block, and they count as another file system). A source is asked about
/// itself (a link is moved as a link), a target folder through its links.
fn dateisystem(file: &gio::File, flags: gio::FileQueryInfoFlags) -> Option<String> {
    if !file.is_native() {
        return None;
    }
    file.query_info("id::filesystem", flags, gio::Cancellable::NONE)
        .ok()
        .and_then(|info| info.attribute_string("id::filesystem"))
        .map(|s| s.to_string())
}

/// The situation of `quellen` dropped on `ziel`.
pub fn lage_fuer(quellen: &[gio::File], ziel: &Ziel, angebot: Angebot, keys: (bool, bool)) -> Lage {
    let aus_papierkorb = quellen.iter().any(im_papierkorb);
    let mut lage = Lage {
        strg: keys.0,
        umschalt: keys.1,
        aus_papierkorb,
        angebot,
        ..Lage::default()
    };
    match ziel {
        Ziel::Ordner(ordner) => {
            lage.ziel_papierkorb = im_papierkorb(ordner) && !aus_papierkorb;
            lage.schon_dort = !quellen.is_empty()
                && quellen
                    .iter()
                    .all(|q| q.parent().is_some_and(|p| p.equal(ordner)));
            lage.in_sich_selbst = quellen
                .iter()
                .any(|q| q.equal(ordner) || ordner.has_prefix(q));
            let ziel_fs = dateisystem(ordner, gio::FileQueryInfoFlags::NONE);
            lage.gleiches_dateisystem = ziel_fs.is_some()
                && !quellen.is_empty()
                && quellen
                    .iter()
                    .all(|q| dateisystem(q, gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS) == ziel_fs);
        }
    }
    lage
}

/// The drag this process runs, while it runs (the target knows its files
/// without reading them).
#[derive(Default)]
struct Laufend {
    dateien: Vec<gio::File>,
    angebot: Angebot,
}

thread_local! {
    static LAUFEND: RefCell<Option<Laufend>> = const { RefCell::new(None) };
}

fn eigene_dateien() -> Option<Vec<gio::File>> {
    LAUFEND.with(|l| l.borrow().as_ref().map(|l| l.dateien.clone()))
}

fn tasten(state: gdk::ModifierType) -> (bool, bool) {
    (
        state.contains(gdk::ModifierType::CONTROL_MASK),
        state.contains(gdk::ModifierType::SHIFT_MASK),
    )
}

/// A drag source on `widget`: `dateien_bei(x, y)` gives the files that leave
/// (empty: no drag, e.g. on free space for the rubber band).
pub fn quelle(
    widget: &impl IsA<gtk4::Widget>,
    dateien_bei: impl Fn(f64, f64) -> Vec<gio::File> + 'static,
) -> gtk4::DragSource {
    let source = gtk4::DragSource::new();
    source.set_button(gdk::BUTTON_PRIMARY);
    // before the views' own gestures (rubber band, clicks on the inner
    // list): it claims the press only once a drag on an entry begins; on
    // free space `prepare` refuses and the rubber band goes on (measured
    // with B2's views, which took every press in the bubble phase)
    source.set_propagation_phase(gtk4::PropagationPhase::Capture);
    source.connect_prepare(move |source, x, y| {
        let dateien = dateien_bei(x, y);
        if dateien.is_empty() {
            return None;
        }
        let (strg, umschalt) = tasten(source.current_event_state());
        let aus_papierkorb = dateien.iter().any(im_papierkorb);
        let aktionen = quell_aktionen(strg, umschalt, aus_papierkorb);
        source.set_actions(aktionen);
        if let Some(widget) = source.widget()
            && let Some(paintable) = zieh_bild(&widget, &dateien[0])
        {
            source.set_icon(Some(&paintable), 16, 16);
        }
        let inhalt = zwischenablage::inhalt(&dateien, None);
        LAUFEND.with(|l| {
            *l.borrow_mut() = Some(Laufend {
                dateien: dateien.clone(),
                angebot: Angebot::aus(aktionen),
            })
        });
        protokoll(&format!(
            "Ziehen beginnt: {} (Strg {strg}, Umschalt {umschalt}, Zustand {:?}, angeboten {aktionen:?})",
            namen(&dateien),
            source.current_event_state()
        ));
        Some(inhalt)
    });
    source.connect_drag_end(|_, drag, _| {
        LAUFEND.with(|l| l.borrow_mut().take());
        protokoll(&format!(
            "Ziehen beendet: {}",
            aktion_name(drag.selected_action())
        ));
    });
    source.connect_drag_cancel(|_, _, reason| {
        LAUFEND.with(|l| l.borrow_mut().take());
        protokoll(&format!("Ziehen abgebrochen: {reason:?}"));
        false
    });
    widget.add_controller(source.clone());
    source
}

fn zieh_bild(widget: &gtk4::Widget, file: &gio::File) -> Option<gdk::Paintable> {
    let info = file
        .query_info(
            "standard::icon",
            gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
            gio::Cancellable::NONE,
        )
        .ok()?;
    let icon = info.icon()?;
    let theme = gtk4::IconTheme::for_display(&widget.display());
    let paintable = theme.lookup_by_gicon(
        &icon,
        32,
        widget.scale_factor(),
        gtk4::TextDirection::None,
        gtk4::IconLookupFlags::empty(),
    );
    Some(paintable.upcast())
}

fn aktion_name(action: gdk::DragAction) -> &'static str {
    if action.contains(gdk::DragAction::MOVE) {
        "verschieben"
    } else if action.contains(gdk::DragAction::COPY) {
        "kopieren"
    } else if action.contains(gdk::DragAction::LINK) {
        "verknüpfen"
    } else {
        "nichts"
    }
}

fn namen(dateien: &[gio::File]) -> String {
    dateien
        .iter()
        .map(|f| querencia_dateiarbeit::uri::name_of(&f.uri()))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Drag and drop of a view (hook `neue_ansicht`): the entries under the
/// pointer leave (the whole selection when the entry is part of it, else
/// the entry, which becomes the selection, as Nautilus); a drop goes into
/// the folder under the pointer, else into the shown folder.
pub fn an_ansicht(fenster: &Rc<Fenster>, ansicht: &Rc<dyn Ansicht>, operationen: &Rc<Operationen>) {
    let widget = ansicht.widget();
    let a = Rc::downgrade(ansicht);
    let f = Rc::downgrade(fenster);
    quelle(&widget, move |x, y| {
        let (Some(a), Some(f)) = (a.upgrade(), f.upgrade()) else {
            return Vec::new();
        };
        let Some(eintrag) = a.eintrag_bei(x, y) else {
            return Vec::new();
        };
        let gewaehlt = f.modell().gewaehlte();
        if gewaehlt.iter().any(|g| g.datei().equal(eintrag.datei())) {
            gewaehlt.iter().map(|g| g.datei().clone()).collect()
        } else {
            f.modell().waehlen(std::slice::from_ref(eintrag.datei()));
            vec![eintrag.datei().clone()]
        }
    });
    let a = Rc::downgrade(ansicht);
    let f = Rc::downgrade(fenster);
    let a2 = a.clone();
    ziel(
        &widget,
        operationen,
        move |x, y| {
            let a = a.upgrade()?;
            match a.eintrag_bei(x, y).filter(Eintrag::ist_ordner) {
                Some(ordner) => Some(Ziel::Ordner(ordner.ziel())),
                None => f.upgrade()?.ort().map(Ziel::Ordner),
            }
        },
        move |punkt| {
            let Some(a) = a2.upgrade() else { return };
            let ordner = punkt
                .and_then(|(x, y)| a.eintrag_bei(x, y))
                .filter(Eintrag::ist_ordner);
            a.ablage_markieren(ordner.as_ref());
        },
    );
}

/// Files of a foreign drop, read once per drop (motion needs them for the
/// file system rule).
#[derive(Default)]
struct Fremd {
    drop: Option<gdk::Drop>,
    dateien: Option<Vec<gio::File>>,
    /// What the source offered when it entered (later GDK reports only the
    /// action the compositor chose).
    angebot: Option<Angebot>,
}

/// What the source of `drop` offers: our own drag knows it; a foreign one
/// as seen on entering. Under Wayland `gdk::Drop::actions` turns into the
/// one action the compositor selected after the first status, which would
/// make the first guess stick (measured in labwc 0.9.8).
fn angebot_fuer(drop: &gdk::Drop, fremd: &Fremd) -> Angebot {
    if drop.drag().is_some()
        && let Some(angebot) = LAUFEND.with(|l| l.borrow().as_ref().map(|l| l.angebot))
    {
        return angebot;
    }
    match (&fremd.drop, fremd.angebot) {
        (Some(d), Some(angebot)) if d == drop => angebot,
        _ => Angebot::aus(drop.actions()),
    }
}

async fn lesen(drop: &gdk::Drop) -> Vec<gio::File> {
    if let Ok(value) = drop
        .read_value_future(gdk::FileList::static_type(), glib::Priority::DEFAULT)
        .await
        && let Ok(list) = value.get::<gdk::FileList>()
    {
        return list.files();
    }
    Vec::new()
}

/// A drop target on `widget`. `ziel_bei(x, y)` says where a drop there goes
/// (`None`: nowhere); `markieren` gets the point a drop would go to (the
/// caller marks the folder cell there), `None` when there is none or the
/// pointer leaves.
pub fn ziel(
    widget: &impl IsA<gtk4::Widget>,
    operationen: &Rc<Operationen>,
    ziel_bei: impl Fn(f64, f64) -> Option<Ziel> + 'static,
    markieren: impl Fn(Option<(f64, f64)>) + 'static,
) -> gtk4::DropTargetAsync {
    let formats = gdk::ContentFormatsBuilder::new()
        .add_type(gdk::FileList::static_type())
        .add_mime_type(querencia_dateiarbeit::uri::URI_LIST)
        .build();
    let target = gtk4::DropTargetAsync::new(
        Some(formats),
        gdk::DragAction::COPY | gdk::DragAction::MOVE | gdk::DragAction::LINK,
    );
    let fremd: Rc<RefCell<Fremd>> = Rc::default();
    let ziel_bei = Rc::new(ziel_bei);
    let markieren = Rc::new(markieren);

    // the files, the target and the action of a drop at (x, y)
    let abwaegen = {
        let fremd = fremd.clone();
        let ziel_bei = ziel_bei.clone();
        Rc::new(
            move |target: &gtk4::DropTargetAsync, drop: &gdk::Drop, x: f64, y: f64| {
                let ziel = ziel_bei(x, y)?;
                let dateien = match eigene_dateien() {
                    Some(own) if drop.drag().is_some() => own,
                    _ => fremd.borrow().dateien.clone().unwrap_or_default(),
                };
                let keys = tasten(target.current_event_state());
                let angebot = angebot_fuer(drop, &fremd.borrow());
                let lage = lage_fuer(&dateien, &ziel, angebot, keys);
                let ablage = if dateien.is_empty() {
                    // not read yet: what the source offers first, as a guess
                    entscheiden(Lage {
                        gleiches_dateisystem: false,
                        schon_dort: false,
                        in_sich_selbst: false,
                        ..lage
                    })
                } else {
                    entscheiden(lage)
                };
                if debugging() {
                    protokoll(&format!(
                        "Ziehen über {:.0},{:.0}: GDK-Aktionen {:?}, Angebot {angebot:?}, Tasten {keys:?} → {ablage:?}",
                        x,
                        y,
                        drop.actions(),
                    ));
                }
                Some((ziel, dateien, ablage?))
            },
        )
    };

    let bewegt = {
        let abwaegen = abwaegen.clone();
        let markieren = markieren.clone();
        move |target: &gtk4::DropTargetAsync, drop: &gdk::Drop, x: f64, y: f64| match abwaegen(
            target, drop, x, y,
        ) {
            Some((_, _, ablage)) => {
                markieren(Some((x, y)));
                als_action(ablage)
            }
            None => {
                markieren(None);
                gdk::DragAction::empty()
            }
        }
    };

    let fremd_enter = fremd.clone();
    let bewegt_enter = bewegt.clone();
    target.connect_drag_enter(move |target, drop, x, y| {
        let eigen = drop.drag().is_some() && eigene_dateien().is_some();
        if !eigen {
            let neu = fremd_enter.borrow().drop.as_ref() != Some(drop);
            if neu {
                *fremd_enter.borrow_mut() = Fremd {
                    drop: Some(drop.clone()),
                    dateien: None,
                    angebot: Some(Angebot::aus(drop.actions())),
                };
                let fremd = fremd_enter.clone();
                let drop = drop.clone();
                glib::MainContext::default().spawn_local(async move {
                    let dateien = lesen(&drop).await;
                    let mut fremd = fremd.borrow_mut();
                    if fremd.drop.as_ref() == Some(&drop) {
                        fremd.dateien = Some(dateien);
                    }
                });
            }
        }
        bewegt_enter(target, drop, x, y)
    });
    target.connect_drag_motion(bewegt);
    let markieren_leave = markieren.clone();
    target.connect_drag_leave(move |_, _| markieren_leave(None));

    let weak = Rc::downgrade(operationen);
    target.connect_drop(move |target, drop, x, y| {
        markieren(None);
        let Some(operationen) = weak.upgrade() else {
            return false;
        };
        let Some(ziel) = ziel_bei(x, y) else {
            return false;
        };
        let keys = tasten(target.current_event_state());
        let angebot = angebot_fuer(drop, &fremd.borrow());
        let eigene = eigene_dateien().filter(|_| drop.drag().is_some());
        let drop = drop.clone();
        let fremd = fremd.clone();
        glib::MainContext::default().spawn_local(async move {
            let dateien = match eigene {
                Some(own) => own,
                None => lesen(&drop).await,
            };
            fremd.borrow_mut().drop.take();
            let lage = lage_fuer(&dateien, &ziel, angebot, keys);
            let ablage = (!dateien.is_empty()).then(|| entscheiden(lage)).flatten();
            protokoll(&format!(
                "Ablage: {} auf {} – angeboten {:?}, Tasten {keys:?}, gleiches Dateisystem {} → {ablage:?}",
                namen(&dateien),
                match &ziel {
                    Ziel::Ordner(o) => o.uri().to_string(),
                },
                angebot,
                lage.gleiches_dateisystem,
            ));
            match ablage {
                Some(ablage) => {
                    operationen.ablegen(ablage, &dateien, &ziel);
                    drop.finish(als_action(ablage));
                }
                None => drop.finish(gdk::DragAction::empty()),
            }
        });
        true
    });
    widget.add_controller(target.clone());
    target
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lage() -> Lage {
        Lage {
            angebot: Angebot::ALLE,
            ..Lage::default()
        }
    }

    #[test]
    fn the_same_file_system_moves_another_copies() {
        let gleich = Lage {
            gleiches_dateisystem: true,
            ..lage()
        };
        assert_eq!(entscheiden(gleich), Some(Ablage::Verschieben));
        assert_eq!(entscheiden(lage()), Some(Ablage::Kopieren));
    }

    #[test]
    fn keys_force_the_action() {
        let strg = Lage {
            strg: true,
            gleiches_dateisystem: true,
            ..lage()
        };
        assert_eq!(entscheiden(strg), Some(Ablage::Kopieren));
        let umschalt = Lage {
            umschalt: true,
            ..lage()
        };
        assert_eq!(entscheiden(umschalt), Some(Ablage::Verschieben));
        let beide = Lage {
            strg: true,
            umschalt: true,
            ..lage()
        };
        assert_eq!(entscheiden(beide), Some(Ablage::Verknuepfen));
    }

    #[test]
    fn a_source_offering_one_action_decides() {
        // Desktop/Files with Umschalt held offer only MOVE
        let nur_verschieben = Lage {
            angebot: Angebot {
                verschieben: true,
                ..Angebot::default()
            },
            ..lage()
        };
        assert_eq!(entscheiden(nur_verschieben), Some(Ablage::Verschieben));
        let nur_kopieren = Lage {
            gleiches_dateisystem: true,
            angebot: Angebot {
                kopieren: true,
                ..Angebot::default()
            },
            ..lage()
        };
        assert_eq!(entscheiden(nur_kopieren), Some(Ablage::Kopieren));
        // Strg here, but the source only moves: no drop
        let gegen = Lage {
            strg: true,
            ..nur_verschieben
        };
        assert_eq!(entscheiden(gegen), None);
    }

    #[test]
    fn out_of_the_trash_only_moving() {
        let raus = Lage {
            aus_papierkorb: true,
            strg: true,
            ..lage()
        };
        assert_eq!(entscheiden(raus), Some(Ablage::Verschieben));
        let ohne_verschieben = Lage {
            angebot: Angebot {
                kopieren: true,
                ..Angebot::default()
            },
            ..raus
        };
        assert_eq!(entscheiden(ohne_verschieben), None);
        assert_eq!(quell_aktionen(true, false, true), gdk::DragAction::MOVE);
    }

    #[test]
    fn onto_the_trash_is_trashing() {
        let hinein = Lage {
            ziel_papierkorb: true,
            strg: true,
            ..lage()
        };
        assert_eq!(entscheiden(hinein), Some(Ablage::Papierkorb));
        let zurueck = Lage {
            aus_papierkorb: true,
            ..hinein
        };
        assert_eq!(entscheiden(zurueck), None);
    }

    #[test]
    fn nothing_into_the_same_folder_or_into_itself() {
        let dort = Lage {
            schon_dort: true,
            gleiches_dateisystem: true,
            ..lage()
        };
        assert_eq!(entscheiden(dort), None);
        assert_eq!(
            entscheiden(Lage {
                umschalt: true,
                ..dort
            }),
            None
        );
        // Strg duplicates in place, as Nautilus ("x (Kopie)")
        assert_eq!(
            entscheiden(Lage { strg: true, ..dort }),
            Some(Ablage::Kopieren)
        );
        let selbst = Lage {
            in_sich_selbst: true,
            strg: true,
            ..lage()
        };
        assert_eq!(entscheiden(selbst), None);
    }

    #[test]
    fn the_source_fixes_keys_at_the_start() {
        assert_eq!(quell_aktionen(true, false, false), gdk::DragAction::COPY);
        assert_eq!(quell_aktionen(false, true, false), gdk::DragAction::MOVE);
        assert_eq!(quell_aktionen(true, true, false), gdk::DragAction::LINK);
        assert_eq!(
            quell_aktionen(false, false, false),
            gdk::DragAction::COPY | gdk::DragAction::MOVE | gdk::DragAction::LINK
        );
    }

    #[test]
    fn the_situation_of_real_files() {
        let basis = std::env::current_dir()
            .unwrap()
            .join("target/tmp/ziehen-lage");
        let _ = std::fs::remove_dir_all(&basis);
        std::fs::create_dir_all(basis.join("a/innen")).unwrap();
        std::fs::create_dir_all(basis.join("b")).unwrap();
        std::fs::write(basis.join("a/x.txt"), b"x").unwrap();
        let x = gio::File::for_path(basis.join("a/x.txt"));
        let a = gio::File::for_path(basis.join("a"));
        let b = gio::File::for_path(basis.join("b"));
        let innen = gio::File::for_path(basis.join("a/innen"));
        let l = lage_fuer(
            std::slice::from_ref(&x),
            &Ziel::Ordner(b.clone()),
            Angebot::ALLE,
            (false, false),
        );
        assert!(l.gleiches_dateisystem && !l.schon_dort && !l.in_sich_selbst);
        assert_eq!(entscheiden(l), Some(Ablage::Verschieben));
        let l = lage_fuer(
            std::slice::from_ref(&x),
            &Ziel::Ordner(a.clone()),
            Angebot::ALLE,
            (false, false),
        );
        assert!(l.schon_dort);
        let l = lage_fuer(
            std::slice::from_ref(&a),
            &Ziel::Ordner(innen),
            Angebot::ALLE,
            (true, false),
        );
        assert!(l.in_sich_selbst);
        assert_eq!(entscheiden(l), None);
        // a GVfs-like place counts as another file system
        let files = gio::File::for_path(papierkorb::home_trash().join("files"));
        assert!(im_papierkorb(&files) && im_papierkorb(&files.child("x.txt")));
        assert!(im_papierkorb(&gio::File::for_uri("trash:///")));
        assert!(!im_papierkorb(&a));
        let fern = gio::File::for_uri("sftp://127.0.0.1/ablage");
        let l = lage_fuer(&[x], &Ziel::Ordner(fern), Angebot::ALLE, (false, false));
        assert!(!l.gleiches_dateisystem);
        assert_eq!(entscheiden(l), Some(Ablage::Kopieren));
        let _ = std::fs::remove_dir_all(&basis);
    }
}
