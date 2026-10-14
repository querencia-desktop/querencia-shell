//! The window list in the bottom panel: one button per toplevel window,
//! from `zwlr_foreign_toplevel_manager_v1` on a Wayland connection of its
//! own. The connection's fd is a GLib source in the GTK main context, so
//! events are dispatched there; there is no thread and no polling.
//!
//! The panel only shows what the compositor reports: a click sends
//! `activate` and the highlight changes when labwc reports the new state.

use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::Hash;
use std::os::fd::AsRawFd;
use std::rc::{Rc, Weak};

use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Button, Label, Orientation, glib, pango};
use wayland_client::backend::WaylandError;
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::{wl_registry::WlRegistry, wl_seat::WlSeat};
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle, event_created_child};
use wayland_protocols_wlr::foreign_toplevel::v1::client::{
    zwlr_foreign_toplevel_handle_v1::{self as handle, ZwlrForeignToplevelHandleV1},
    zwlr_foreign_toplevel_manager_v1::{self as manager, ZwlrForeignToplevelManagerV1},
};

/// Identity of a window for the widgets. Never reused while the panel runs,
/// unlike Wayland protocol ids, which the compositor reuses after `closed`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct WindowKey(u64);

#[derive(Clone, Debug, Default, PartialEq)]
struct Info {
    title: String,
    app_id: String,
    activated: bool,
    minimized: bool,
}

struct Entry {
    key: WindowKey,
    pending: Info,
    /// Last state completed by `done`; `None` until the first `done`.
    current: Option<Info>,
}

/// Window state as reported by the compositor, keyed by the protocol handle
/// (`H`; a plain number in the tests). Events only touch `pending`; `done`
/// completes it. `changed` collects completed changes until the widgets
/// have been brought up to date.
struct Windows<H> {
    entries: HashMap<H, Entry>,
    next_key: u64,
    changed: bool,
}

impl<H: Clone + Eq + Hash> Windows<H> {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
            next_key: 0,
            changed: false,
        }
    }

    fn added(&mut self, handle: H) {
        self.next_key += 1;
        let entry = Entry {
            key: WindowKey(self.next_key),
            pending: Info::default(),
            current: None,
        };
        self.entries.insert(handle, entry);
    }

    fn title(&mut self, handle: &H, title: String) {
        if let Some(entry) = self.entries.get_mut(handle) {
            entry.pending.title = title;
        }
    }

    fn app_id(&mut self, handle: &H, app_id: String) {
        if let Some(entry) = self.entries.get_mut(handle) {
            entry.pending.app_id = app_id;
        }
    }

    fn state(&mut self, handle: &H, raw: &[u8]) {
        if let Some(entry) = self.entries.get_mut(handle) {
            (entry.pending.activated, entry.pending.minimized) = parse_state(raw);
        }
    }

    fn done(&mut self, handle: &H) {
        if let Some(entry) = self.entries.get_mut(handle)
            && entry.current.as_ref() != Some(&entry.pending)
        {
            entry.current = Some(entry.pending.clone());
            self.changed = true;
        }
    }

    fn closed(&mut self, handle: &H) {
        if let Some(entry) = self.entries.remove(handle) {
            self.changed |= entry.current.is_some();
        }
    }

    /// The handle of a window that is still open.
    fn handle(&self, key: WindowKey) -> Option<&H> {
        self.entries
            .iter()
            .find(|(_, e)| e.key == key)
            .map(|(h, _)| h)
    }

    /// Completed windows in the order they appeared.
    fn snapshot(&self) -> Vec<(WindowKey, Info)> {
        let mut list: Vec<_> = self
            .entries
            .values()
            .filter_map(|e| e.current.clone().map(|info| (e.key, info)))
            .collect();
        list.sort_by_key(|(key, _)| *key);
        list
    }

    fn take_changed(&mut self) -> bool {
        std::mem::take(&mut self.changed)
    }
}

/// `activated` and `minimized` from the protocol's state array of native
/// endian u32 values; maximized (0) and fullscreen (3) are not shown.
fn parse_state(raw: &[u8]) -> (bool, bool) {
    let mut flags = (false, false);
    for value in raw.chunks_exact(4) {
        match u32::from_ne_bytes([value[0], value[1], value[2], value[3]]) {
            v if v == handle::State::Activated as u32 => flags.0 = true,
            v if v == handle::State::Minimized as u32 => flags.1 = true,
            _ => {}
        }
    }
    flags
}

/// Button text: the title, in brackets when minimized as in GNOME 2.
fn button_text(info: &Info) -> String {
    let title = match (info.title.is_empty(), info.app_id.is_empty()) {
        (false, _) => info.title.as_str(),
        (true, false) => info.app_id.as_str(),
        (true, true) => "Unbenanntes Fenster",
    };
    if info.minimized {
        format!("[{title}]")
    } else {
        title.to_owned()
    }
}

/// Dispatch target of the event queue.
struct Protocol {
    windows: Windows<ZwlrForeignToplevelHandleV1>,
    finished: bool,
}

impl Dispatch<WlRegistry, GlobalListContents> for Protocol {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: <WlRegistry as Proxy>::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlSeat, ()> for Protocol {
    fn event(
        _: &mut Self,
        _: &WlSeat,
        _: <WlSeat as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwlrForeignToplevelManagerV1, ()> for Protocol {
    fn event(
        state: &mut Self,
        _: &ZwlrForeignToplevelManagerV1,
        event: manager::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            manager::Event::Toplevel { toplevel } => state.windows.added(toplevel),
            manager::Event::Finished => state.finished = true,
            _ => {}
        }
    }

    event_created_child!(Protocol, ZwlrForeignToplevelManagerV1, [
        manager::EVT_TOPLEVEL_OPCODE => (ZwlrForeignToplevelHandleV1, ()),
    ]);
}

impl Dispatch<ZwlrForeignToplevelHandleV1, ()> for Protocol {
    fn event(
        state: &mut Self,
        proxy: &ZwlrForeignToplevelHandleV1,
        event: handle::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let windows = &mut state.windows;
        match event {
            handle::Event::Title { title } => windows.title(proxy, title),
            handle::Event::AppId { app_id } => windows.app_id(proxy, app_id),
            handle::Event::State { state } => windows.state(proxy, &state),
            handle::Event::Done => windows.done(proxy),
            handle::Event::Closed => {
                windows.closed(proxy);
                proxy.destroy();
            }
            _ => {}
        }
    }
}

/// Everything that lives only as long as the connection.
struct Session {
    conn: Connection,
    queue: EventQueue<Protocol>,
    protocol: Protocol,
    seat: WlSeat,
}

struct WindowList {
    /// Weak, so the list never keeps the panel's widgets alive.
    container: glib::WeakRef<GtkBox>,
    /// `None` once the connection has failed or was never there.
    session: Option<Session>,
    buttons: HashMap<WindowKey, Button>,
    /// The fd source while it is attached. Its closure holds the only
    /// strong reference to the list.
    source: Option<glib::SourceId>,
}

type Shared = Rc<RefCell<WindowList>>;

/// The window list for the bottom panel. Without a compositor that offers
/// the protocol it stays empty.
pub fn widget() -> GtkBox {
    let container = GtkBox::new(Orientation::Horizontal, 2);
    let list = Rc::new(RefCell::new(WindowList {
        container: container.downgrade(),
        session: None,
        buttons: HashMap::new(),
        source: None,
    }));
    match connect() {
        Ok(session) => {
            let fd = session.conn.backend().poll_fd().as_raw_fd();
            list.borrow_mut().session = Some(session);
            let source = glib_unix::unix_fd_add_local(
                fd,
                glib::IOCondition::IN | glib::IOCondition::HUP | glib::IOCondition::ERR,
                {
                    let list = list.clone();
                    move |_, condition| on_readable(&list, condition)
                },
            );
            list.borrow_mut().source = Some(source);
            // The list ends with the bottom panel: removing the source drops
            // its closure, then the list and with it the Wayland connection.
            // A source that already ended with `Break` has given up its id.
            let list = Rc::downgrade(&list);
            container.connect_destroy(move |_| {
                let source = list
                    .upgrade()
                    .and_then(|list| list.borrow_mut().source.take());
                if let Some(source) = source {
                    source.remove();
                }
            });
        }
        Err(err) => eprintln!("querencia-panel: Fensterliste nicht verfügbar: {err}"),
    }
    container
}

fn connect() -> Result<Session, String> {
    let conn = Connection::connect_to_env().map_err(|e| e.to_string())?;
    let (globals, queue) = registry_queue_init::<Protocol>(&conn).map_err(|e| e.to_string())?;
    let qh = queue.handle();
    let manager: ZwlrForeignToplevelManagerV1 = globals
        .bind(&qh, 1..=3, ())
        .map_err(|_| "kein zwlr_foreign_toplevel_manager_v1".to_owned())?;
    let seat: WlSeat = globals
        .bind(&qh, 1..=1, ())
        .map_err(|_| "kein wl_seat".to_owned())?;
    conn.flush().map_err(|e| e.to_string())?;
    eprintln!(
        "querencia-panel: Fensterliste über zwlr_foreign_toplevel_manager_v1 v{}",
        manager.version()
    );
    let protocol = Protocol {
        windows: Windows::new(),
        finished: false,
    };
    Ok(Session {
        conn,
        queue,
        protocol,
        seat,
    })
}

/// Reads and dispatches everything the compositor sent, then brings the
/// buttons up to date once for the whole batch. The protocol makes the
/// changes of one window consistent only up to its own `done`; it does not
/// promise that the `done` of two windows arrive together. labwc 0.9.8 was
/// observed to send a focus change (old and new window) in one batch, so
/// the panel normally never shows the intermediate state. If a batch ends
/// between the two, the next one completes it; no change is lost.
/// Returning `Break` makes GLib remove the source.
fn on_readable(list: &Shared, condition: glib::IOCondition) -> glib::ControlFlow {
    let result = {
        let mut list = list.borrow_mut();
        match list.session.as_mut() {
            Some(session) => dispatch(session, condition),
            None => Err("keine Sitzung".to_owned()),
        }
    };
    match result {
        Ok(()) => {
            sync(list);
            glib::ControlFlow::Continue
        }
        Err(err) => {
            eprintln!("querencia-panel: Fensterliste beendet: {err}");
            // Dropping the session releases the connection and all handles;
            // no stale window may stay in the panel. GLib removes the source
            // after `Break`, so its id is dropped here and never removed again.
            {
                let mut list = list.borrow_mut();
                list.session = None;
                list.source = None;
            }
            sync(list);
            glib::ControlFlow::Break
        }
    }
}

fn dispatch(session: &mut Session, condition: glib::IOCondition) -> Result<(), String> {
    if let Some(guard) = session.queue.prepare_read() {
        match guard.read() {
            Ok(_) => {}
            Err(WaylandError::Io(e)) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(err) => return Err(format!("Lesen: {err}")),
        }
    }
    session
        .queue
        .dispatch_pending(&mut session.protocol)
        .map_err(|e| format!("Verarbeiten: {e}"))?;
    flush(&session.conn)?;
    if session.protocol.finished {
        return Err("der Compositor hat die Fensterliste beendet".to_owned());
    }
    if condition.intersects(glib::IOCondition::HUP | glib::IOCondition::ERR) {
        return Err(format!("Verbindung getrennt ({condition:?})"));
    }
    Ok(())
}

fn flush(conn: &Connection) -> Result<(), String> {
    match conn.flush() {
        Ok(()) => Ok(()),
        // The socket is full; wayland-client keeps the requests and sends
        // them with the next flush. Known limitation: there is no watch for
        // the socket becoming writable, so that next flush only comes with
        // the next compositor event or click.
        Err(WaylandError::Io(e)) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(()),
        Err(err) => Err(format!("Senden: {err}")),
    }
}

/// Makes the buttons match the completed window state.
fn sync(list: &Shared) {
    let (snapshot, container) = {
        let mut list = list.borrow_mut();
        let snapshot = match list.session.as_mut() {
            Some(session) => {
                let windows = &mut session.protocol.windows;
                windows.take_changed().then(|| windows.snapshot())
            }
            None => Some(Vec::new()),
        };
        (snapshot, list.container.upgrade())
    };
    let (Some(snapshot), Some(container)) = (snapshot, container) else {
        return;
    };

    let mut stale: HashMap<_, _> = std::mem::take(&mut list.borrow_mut().buttons);
    let mut buttons = HashMap::new();
    for (key, info) in snapshot {
        let button = stale.remove(&key).unwrap_or_else(|| {
            let button = new_button(Rc::downgrade(list), key);
            container.append(&button);
            button
        });
        update_button(&button, &info);
        buttons.insert(key, button);
    }
    for button in stale.into_values() {
        container.remove(&button);
    }
    list.borrow_mut().buttons = buttons;
}

fn new_button(list: Weak<RefCell<WindowList>>, key: WindowKey) -> Button {
    let label = Label::builder()
        .ellipsize(pango::EllipsizeMode::End)
        .max_width_chars(24)
        .xalign(0.0)
        .build();
    let button = Button::builder()
        .child(&label)
        .has_frame(false)
        .css_classes(["window-button"])
        .build();
    button.connect_clicked(move |_| {
        if let Some(list) = list.upgrade() {
            activate(&list.borrow(), key);
        }
    });
    button
}

fn update_button(button: &Button, info: &Info) {
    if let Some(label) = button.child().and_downcast::<Label>() {
        label.set_label(&button_text(info));
    }
    button.set_tooltip_text((!info.title.is_empty()).then_some(info.title.as_str()));
    if info.activated {
        button.add_css_class("active");
    } else {
        button.remove_css_class("active");
    }
}

/// Asks the compositor to activate the window. The highlight follows only
/// the state labwc reports back; a closed window's key finds nothing.
fn activate(list: &WindowList, key: WindowKey) {
    let Some(session) = &list.session else { return };
    let Some(handle) = session.protocol.windows.handle(key) else {
        return;
    };
    handle.activate(&session.seat);
    match flush(&session.conn) {
        Ok(()) => eprintln!(
            "querencia-panel: Aktivierung von Fenster {} angefragt",
            key.0
        ),
        Err(err) => eprintln!("querencia-panel: Aktivierung nicht gesendet: {err}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(activated: bool, minimized: bool) -> Vec<u8> {
        let mut raw = Vec::new();
        if minimized {
            raw.extend((handle::State::Minimized as u32).to_ne_bytes());
        }
        if activated {
            raw.extend((handle::State::Activated as u32).to_ne_bytes());
        }
        raw
    }

    fn open(windows: &mut Windows<u32>, id: u32, title: &str, app_id: &str) -> WindowKey {
        windows.added(id);
        windows.title(&id, title.to_owned());
        windows.app_id(&id, app_id.to_owned());
        windows.done(&id);
        windows.snapshot().last().expect("window is listed").0
    }

    fn titles(windows: &Windows<u32>) -> Vec<String> {
        windows
            .snapshot()
            .iter()
            .map(|(_, info)| button_text(info))
            .collect()
    }

    #[test]
    fn windows_appear_after_done_and_disappear_when_closed() {
        let mut windows = Windows::new();
        windows.added(1);
        windows.title(&1, "Terminal".into());
        assert!(
            windows.snapshot().is_empty(),
            "nothing before the first done"
        );
        assert!(!windows.take_changed());

        windows.done(&1);
        assert_eq!(titles(&windows), ["Terminal"]);
        assert!(windows.take_changed());

        windows.closed(&1);
        assert!(windows.snapshot().is_empty());
        assert!(windows.take_changed());

        // A window closed before its first done was never shown.
        windows.added(2);
        windows.closed(&2);
        assert!(!windows.take_changed());
    }

    #[test]
    fn title_and_state_change_only_with_done() {
        let mut windows = Windows::new();
        open(&mut windows, 1, "alt", "foot");
        windows.take_changed();

        windows.title(&1, "neu".into());
        windows.state(&1, &state(true, false));
        assert_eq!(titles(&windows), ["alt"]);
        windows.done(&1);
        assert!(windows.take_changed());
        let (_, info) = &windows.snapshot()[0];
        assert_eq!((info.title.as_str(), info.activated), ("neu", true));

        windows.state(&1, &state(false, true));
        windows.done(&1);
        assert_eq!(titles(&windows), ["[neu]"]);

        // A done without a difference is no change.
        windows.take_changed();
        windows.done(&1);
        assert!(!windows.take_changed());
    }

    #[test]
    fn focus_change_across_two_windows_is_one_batch() {
        let mut windows = Windows::new();
        open(&mut windows, 1, "a", "foot");
        open(&mut windows, 2, "b", "foot");
        windows.state(&1, &state(true, false));
        windows.done(&1);
        windows.take_changed();

        // labwc reports the new window first, then the old one, or the
        // other way round; the widgets only see the result.
        windows.state(&2, &state(true, false));
        windows.done(&2);
        windows.state(&1, &state(false, false));
        windows.done(&1);
        assert!(windows.take_changed());
        let active: Vec<_> = windows
            .snapshot()
            .iter()
            .map(|(_, i)| i.activated)
            .collect();
        assert_eq!(active, [false, true]);
    }

    #[test]
    fn two_windows_of_the_same_application_stay_separate() {
        let mut windows = Windows::new();
        let a = open(&mut windows, 1, "c2-probe-a", "c2probe");
        let b = open(&mut windows, 2, "c2-probe-b", "c2probe");
        assert_ne!(a, b);
        assert_eq!(titles(&windows), ["c2-probe-a", "c2-probe-b"]);
        assert_eq!(windows.handle(a), Some(&1));
        assert_eq!(windows.handle(b), Some(&2));

        windows.closed(&1);
        assert_eq!(titles(&windows), ["c2-probe-b"]);
        assert_eq!(windows.handle(b), Some(&2));
    }

    #[test]
    fn stale_key_never_reaches_a_window_with_a_reused_protocol_id() {
        let mut windows = Windows::new();
        let old = open(&mut windows, 7, "erstes", "foot");
        windows.closed(&7);
        // Late events for the closed handle are ignored.
        windows.title(&7, "verspätet".into());
        windows.done(&7);
        assert!(windows.snapshot().is_empty());

        // The compositor hands out protocol id 7 again.
        let new = open(&mut windows, 7, "zweites", "foot");
        assert_ne!(old, new);
        assert_eq!(
            windows.handle(old),
            None,
            "a button of the closed window activates nothing"
        );
        assert_eq!(windows.handle(new), Some(&7));
        assert_eq!(titles(&windows), ["zweites"]);
    }

    #[test]
    fn state_array_and_button_text() {
        assert_eq!(parse_state(&[]), (false, false));
        assert_eq!(parse_state(&state(true, true)), (true, true));
        let maximized_fullscreen: Vec<u8> =
            [0u32, 3].iter().flat_map(|v| v.to_ne_bytes()).collect();
        assert_eq!(parse_state(&maximized_fullscreen), (false, false));

        let mut info = Info {
            title: "Dokument".into(),
            app_id: "editor".into(),
            ..Info::default()
        };
        assert_eq!(button_text(&info), "Dokument");
        info.minimized = true;
        assert_eq!(button_text(&info), "[Dokument]");
        info.title.clear();
        assert_eq!(button_text(&info), "[editor]");
        info.app_id.clear();
        info.minimized = false;
        assert_eq!(button_text(&info), "Unbenanntes Fenster");
    }
}
