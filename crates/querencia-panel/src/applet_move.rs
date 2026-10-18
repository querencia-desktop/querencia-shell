//! Moving applets by dragging (Teil 5), as on the GNOME 2 panel: press the
//! middle button on an applet (or the left button while Alt is held) and
//! drag; the applet moves along live, in its panel, across the regions. On
//! release the new place is saved (`panel.ini`, through the shared
//! [`Editor`]); if saving fails, the applet goes back.
//!
//! The target follows the pointer: the third of the panel it is in gives
//! the region ([`Region::at`]), the applets of that region whose middle is
//! left of the pointer give the index ([`slot`]). The dragged applet is not
//! counted, so it never chases itself.
//!
//! A middle click without dragging stays with the applet (the volume
//! applet mutes on it): the drag only takes the click over once the
//! pointer has moved [`THRESHOLD`] pixels, and [`dragged_recently`] tells
//! the applet that the release ended a drag. Alt with the left button
//! takes the click at once, so a menu button does not open. But a panel
//! keeps `KeyboardMode::None`, never has the keyboard focus, and Wayland
//! sends the modifier state only to the focused client: on labwc GTK does
//! not learn that Alt is held, so Alt with the left button works only
//! where a compositor tells the panel (measured, see the G2-M report).
//!
//! For the left button alone there is GNOME 2's „Verschieben“ entry in
//! every applet menu ([`Mover::follow`]): the applet then follows the
//! pointer over its panel until a left click places it (saved); a right
//! click puts it back.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk4::prelude::*;
use gtk4::{
    ApplicationWindow, CenterBox, EventSequenceState, GestureDrag, PickFlags, PropagationPhase,
    Widget, gdk, glib,
};

use crate::layout::{Move, Region};
use crate::layout_editor::Editor;
use crate::{panel, panel_menu};

/// Pixels the pointer moves before a press becomes a drag.
pub const THRESHOLD: f64 = 8.0;
/// After a drag ended, a release within this time belongs to it.
const AFTER_DRAG: Duration = Duration::from_millis(400);

thread_local! {
    static LAST_DRAG: Cell<Option<Instant>> = const { Cell::new(None) };
}

/// Whether a drag of an applet is running or just ended: a middle-button
/// release then is not a click.
pub fn dragged_recently() -> bool {
    LAST_DRAG.with(|last| last.get().is_some_and(|t| t.elapsed() < AFTER_DRAG))
}

fn mark_drag() {
    LAST_DRAG.with(|last| last.set(Some(Instant::now())));
}

/// The index in a region for a pointer at `x`, given the middles of the
/// region's other applets from left to right: the number of them left of
/// the pointer.
pub fn slot(middles: &[f64], x: f64) -> usize {
    middles.iter().filter(|&&m| m < x).count()
}

/// Whether a press of `button` with `state` starts a drag: the middle
/// button, or the left button with Alt.
pub fn starts_drag(button: u32, state: gdk::ModifierType) -> bool {
    button == gdk::BUTTON_MIDDLE
        || (button == gdk::BUTTON_PRIMARY && state.contains(gdk::ModifierType::ALT_MASK))
}

/// A drag in progress.
struct Drag {
    id: String,
    /// Where the applet stood (shown position) when the drag began.
    from: (Region, usize),
    /// Press point in window coordinates.
    start: (f64, f64),
    moving: bool,
    button: u32,
}

/// „Verschieben“ from the menu: the applet `id` follows the pointer.
struct Follow {
    id: String,
    from: (Region, usize),
}

/// Moving by pointer for one panel window: the drag gesture and the
/// follow mode of „Verschieben“.
pub struct Mover {
    window: glib::WeakRef<ApplicationWindow>,
    bar: glib::WeakRef<CenterBox>,
    editor: Rc<RefCell<Editor>>,
    follow: RefCell<Option<Follow>>,
}

impl Mover {
    /// Starts „Verschieben“ for the applet `id`.
    pub fn follow(&self, id: &str) {
        let (Some(window), Some(bar)) = (self.window.upgrade(), self.bar.upgrade()) else {
            return;
        };
        let Some(from) = panel::shown_position(&bar, id) else {
            return;
        };
        window.set_cursor_from_name(Some("move"));
        eprintln!(
            "querencia-panel: Verschieben von {id} beginnt ({} {}): Linksklick setzt, Rechtsklick bricht ab",
            from.0.label(),
            from.1 + 1
        );
        self.follow.replace(Some(Follow {
            id: id.to_string(),
            from,
        }));
    }

    fn following(&self) -> bool {
        self.follow.borrow().is_some()
    }

    /// The pointer moved to `x` (window coordinates) in follow mode.
    fn follow_to(&self, x: f64, y: f64) {
        let (Some(window), Some(bar)) = (self.window.upgrade(), self.bar.upgrade()) else {
            return;
        };
        let follow = self.follow.borrow();
        let Some(follow) = follow.as_ref() else {
            return;
        };
        let Some((x, _)) = window.translate_coordinates(&bar, x, y) else {
            return;
        };
        let to = target(&bar, &follow.id, x);
        if panel::shown_position(&bar, &follow.id) != Some(to) {
            panel::move_applet(&bar, &follow.id, to.0, to.1);
        }
    }

    /// Ends follow mode: `place` saves where the applet stands now, else it
    /// goes back.
    fn end_follow(&self, place: bool) {
        let Some(follow) = self.follow.take() else {
            return;
        };
        let (Some(window), Some(bar)) = (self.window.upgrade(), self.bar.upgrade()) else {
            return;
        };
        window.set_cursor(None::<&gdk::Cursor>);
        let shown = panel::shown_position(&bar, &follow.id);
        if !place {
            panel::move_applet(&bar, &follow.id, follow.from.0, follow.from.1);
            eprintln!("querencia-panel: Verschieben von {} abgebrochen", follow.id);
            return;
        }
        match shown {
            Some(shown) if shown != follow.from => {
                save(&self.editor, &bar, &follow.id, shown, "Verschieben")
            }
            _ => eprintln!(
                "querencia-panel: {} bleibt, wo es ist (Verschieben)",
                follow.id
            ),
        }
    }
}

/// Saves the shown place `shown` of `id`; on failure the applet goes back
/// to where the layout has it.
fn save(
    editor: &Rc<RefCell<Editor>>,
    bar: &CenterBox,
    id: &str,
    shown: (Region, usize),
    how: &str,
) {
    let saved = editor
        .borrow_mut()
        .move_applet(id, Move::To(shown.0, shown.1));
    match saved {
        Ok(Some(moved)) => eprintln!(
            "querencia-panel: {id} verschoben ({how}): {} {} → {} {}; gespeichert in {}",
            moved.from.0.label(),
            moved.from.1 + 1,
            moved.to.0.label(),
            moved.to.1 + 1,
            editor.borrow().path().display()
        ),
        Ok(None) => {}
        Err(err) => {
            eprintln!("querencia-panel: {id} nicht verschoben: {err}");
            if let Some(at) = editor.borrow().position(id) {
                panel::move_applet(bar, id, at.0, at.1);
            }
        }
    }
}

/// Lets the applets of `window` (panel bar `bar`) be moved by pointer:
/// dragging, and „Verschieben“ through the returned [`Mover`].
pub fn attach(
    window: &ApplicationWindow,
    bar: &CenterBox,
    editor: &Rc<RefCell<Editor>>,
) -> Rc<Mover> {
    let mover = Rc::new(Mover {
        window: window.downgrade(),
        bar: bar.downgrade(),
        editor: editor.clone(),
        follow: RefCell::new(None),
    });
    // Follow mode: motion moves, a left click places, a right click
    // cancels; the click goes to nothing else.
    let motion = gtk4::EventControllerMotion::new();
    let weak = Rc::downgrade(&mover);
    motion.connect_motion(move |_, x, y| {
        if let Some(mover) = weak.upgrade()
            && mover.following()
        {
            mover.follow_to(x, y);
        }
    });
    window.add_controller(motion);
    let place = gtk4::GestureClick::new();
    place.set_button(0);
    place.set_propagation_phase(PropagationPhase::Capture);
    let weak = Rc::downgrade(&mover);
    place.connect_pressed(move |gesture, _, x, y| {
        let Some(mover) = weak.upgrade() else { return };
        if !mover.following() {
            gesture.set_state(EventSequenceState::Denied);
            return;
        }
        gesture.set_state(EventSequenceState::Claimed);
        if gesture.current_button() == gdk::BUTTON_PRIMARY {
            mover.follow_to(x, y);
            mover.end_follow(true);
        } else {
            mover.end_follow(false);
        }
    });
    window.add_controller(place);

    let drag = GestureDrag::new();
    drag.set_button(0);
    // Before the applets: a press with Alt must not reach a menu button.
    drag.set_propagation_phase(PropagationPhase::Capture);
    let current: Rc<RefCell<Option<Drag>>> = Rc::default();
    let follow_active = Rc::downgrade(&mover);

    drag.connect_drag_begin(glib::clone!(
        #[strong]
        follow_active,
        #[weak]
        window,
        #[weak]
        bar,
        #[strong]
        current,
        move |gesture, x, y| {
            if follow_active.upgrade().is_some_and(|m| m.following()) {
                gesture.set_state(EventSequenceState::Denied);
                return;
            }
            let button = gesture.current_button();
            let state = gesture.current_event_state();
            let applet = applet_at(&window, x, y);
            let begin = starts_drag(button, state)
                .then_some(applet)
                .flatten()
                .and_then(|applet| {
                    let id = applet.widget_name().to_string();
                    panel::shown_position(&bar, &id).map(|from| (id, from))
                });
            let Some((id, from)) = begin else {
                gesture.set_state(EventSequenceState::Denied);
                return;
            };
            if button == gdk::BUTTON_PRIMARY {
                gesture.set_state(EventSequenceState::Claimed);
            }
            current.replace(Some(Drag {
                id,
                from,
                start: (x, y),
                moving: false,
                button,
            }));
        }
    ));

    drag.connect_drag_update(glib::clone!(
        #[weak]
        window,
        #[weak]
        bar,
        #[strong]
        current,
        move |gesture, dx, dy| {
            let mut slot_ = current.borrow_mut();
            let Some(drag) = slot_.as_mut() else { return };
            if !drag.moving {
                if dx.abs().max(dy.abs()) < THRESHOLD {
                    return;
                }
                drag.moving = true;
                gesture.set_state(EventSequenceState::Claimed);
                window.set_cursor_from_name(Some("grabbing"));
                eprintln!(
                    "querencia-panel: Ziehen von {} beginnt ({}, {} {})",
                    drag.id,
                    if drag.button == gdk::BUTTON_MIDDLE {
                        "mittlere Taste"
                    } else {
                        "Alt + links"
                    },
                    drag.from.0.label(),
                    drag.from.1 + 1
                );
            }
            mark_drag();
            let Some((x, _)) = window.translate_coordinates(&bar, drag.start.0 + dx, drag.start.1)
            else {
                return;
            };
            let target = target(&bar, &drag.id, x);
            if panel::shown_position(&bar, &drag.id) != Some(target) {
                panel::move_applet(&bar, &drag.id, target.0, target.1);
            }
        }
    ));

    let finish = Rc::new(glib::clone!(
        #[weak]
        window,
        #[weak]
        bar,
        #[strong]
        current,
        #[strong]
        editor,
        move |keep: bool| {
            let Some(drag) = current.take() else { return };
            window.set_cursor(None::<&gdk::Cursor>);
            if !drag.moving {
                return;
            }
            mark_drag();
            let Some(shown) = panel::shown_position(&bar, &drag.id) else {
                return;
            };
            if !keep {
                panel::move_applet(&bar, &drag.id, drag.from.0, drag.from.1);
                eprintln!("querencia-panel: Ziehen von {} abgebrochen", drag.id);
                return;
            }
            if shown == drag.from {
                eprintln!("querencia-panel: {} bleibt, wo es ist (gezogen)", drag.id);
                return;
            }
            save(&editor, &bar, &drag.id, shown, "gezogen");
        }
    ));
    let end = finish.clone();
    drag.connect_drag_end(move |_, _, _| end(true));
    drag.connect_cancel(move |_, _| finish(false));
    window.add_controller(drag);
    mover
}

/// Region and index (counted without the dragged applet `id`) for the
/// pointer at `x` in the bar's coordinates.
fn target(bar: &CenterBox, id: &str, x: f64) -> (Region, usize) {
    let region = Region::at(x, f64::from(bar.width()));
    let middles: Vec<f64> = panel::region_applets(bar, region)
        .iter()
        .filter(|w| w.widget_name() != id)
        .filter_map(|w| {
            let bounds = w.compute_bounds(bar)?;
            Some(f64::from(bounds.x() + bounds.width() / 2.0))
        })
        .collect();
    (region, slot(&middles, x))
}

/// The applet under the point (window coordinates), also on an applet's
/// own background that counts as free space for the panel menu (the
/// window list).
fn applet_at(window: &ApplicationWindow, x: f64, y: f64) -> Option<Widget> {
    let mut hit = window.pick(x, y, PickFlags::DEFAULT);
    while let Some(widget) = hit {
        if widget.has_css_class(panel_menu::APPLET_CLASS) {
            return Some(widget);
        }
        hit = widget.parent();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_counts_the_applets_left_of_the_pointer() {
        let middles = [20.0, 60.0, 100.0];
        assert_eq!(slot(&middles, 5.0), 0);
        assert_eq!(slot(&middles, 40.0), 1);
        assert_eq!(slot(&middles, 99.0), 2);
        assert_eq!(slot(&middles, 300.0), 3);
        assert_eq!(slot(&[], 300.0), 0);
    }

    #[test]
    fn middle_button_or_alt_with_left_starts_a_drag() {
        use gdk::ModifierType as M;
        assert!(starts_drag(gdk::BUTTON_MIDDLE, M::empty()));
        assert!(starts_drag(gdk::BUTTON_PRIMARY, M::ALT_MASK));
        assert!(!starts_drag(gdk::BUTTON_PRIMARY, M::empty()));
        assert!(!starts_drag(gdk::BUTTON_PRIMARY, M::CONTROL_MASK));
        assert!(!starts_drag(gdk::BUTTON_SECONDARY, M::ALT_MASK));
    }

    #[test]
    fn no_drag_seen_at_first() {
        assert!(!dragged_recently());
        mark_drag();
        assert!(dragged_recently());
    }
}
