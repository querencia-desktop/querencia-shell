//! The window list in the bottom panel: one button per toplevel window,
//! from `zwlr_foreign_toplevel_manager_v1` on a Wayland connection of its
//! own. The connection's fd is a GLib source in the GTK main context, so
//! events are dispatched there; there is no thread and no polling.
//!
//! The panel only shows what the compositor reports: a click sends a
//! request and the look changes when labwc reports the new state.
//!
//! HMP-06, the classic GNOME 2 window list: every button is
//! `[16 px application icon] [title…]` (icon from the window's desktop entry,
//! `window_icons`), all buttons equally wide (the available width shared
//! out, at most [`MAX_BUTTON_WIDTH`], see `button_width`) and as high as the
//! panel allows. The active window's button looks pressed, a minimized
//! window's button is dimmed (instead of GNOME 2's brackets around the
//! title), the full title is in the tooltip. A click minimizes the active
//! window, restores a minimized one and activates any other, as in GNOME 2.
//!
//! G2-AK (B11): as in GNOME 2 the list shows only the windows of the
//! current workspace, unless `panel.ini` says `[window-list]
//! alle_arbeitsflaechen=ja`. The same connection binds
//! `ext_workspace_manager_v1`, so window and workspace events arrive in the
//! order labwc sent them; `fensterliste.rs` places the windows from them.

use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::Hash;
use std::os::fd::AsRawFd;
use std::rc::{Rc, Weak};

use gtk4::prelude::*;
use gtk4::subclass::prelude::*;
use gtk4::{Box as GtkBox, Button, Image, Label, Orientation, glib, pango};
use wayland_client::backend::WaylandError;
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::{
    wl_callback::{self, WlCallback},
    wl_registry::WlRegistry,
    wl_seat::WlSeat,
};
use wayland_client::{
    Connection, Dispatch, EventQueue, Proxy, QueueHandle, WEnum, event_created_child,
};
use wayland_protocols::ext::workspace::v1::client::{
    ext_workspace_group_handle_v1::{self as workspace_group, ExtWorkspaceGroupHandleV1},
    ext_workspace_handle_v1::{self as workspace, ExtWorkspaceHandleV1},
    ext_workspace_manager_v1::{self as workspace_manager, ExtWorkspaceManagerV1},
};
use wayland_protocols_wlr::foreign_toplevel::v1::client::{
    zwlr_foreign_toplevel_handle_v1::{self as handle, ZwlrForeignToplevelHandleV1},
    zwlr_foreign_toplevel_manager_v1::{self as manager, ZwlrForeignToplevelManagerV1},
};

use crate::fensterliste::{self, Ereignis, Flaechen, Zuordnung};

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
    /// Appearing, focus and closing in the order of their `done` (G2-AK),
    /// for [`Zuordnung`].
    events: Vec<Ereignis<WindowKey>>,
}

impl<H: Clone + Eq + Hash> Windows<H> {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
            next_key: 0,
            changed: false,
            events: Vec::new(),
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
            let was = entry.current.as_ref().map(|info| info.activated);
            let now = entry.pending.activated;
            if was.is_none() {
                self.events.push(Ereignis::Erschienen(entry.key));
            }
            match (was.unwrap_or(false), now) {
                (false, true) => self.events.push(Ereignis::Aktiviert(entry.key)),
                (true, false) => self.events.push(Ereignis::Deaktiviert(entry.key)),
                _ => {}
            }
            entry.current = Some(entry.pending.clone());
            self.changed = true;
        }
    }

    fn closed(&mut self, handle: &H) {
        if let Some(entry) = self.entries.remove(handle) {
            if entry.current.is_some() {
                self.events.push(Ereignis::Geschlossen(entry.key));
            }
            self.changed |= entry.current.is_some();
        }
    }

    /// The completed state of a window that is still open.
    fn info(&self, key: WindowKey) -> Option<&Info> {
        self.entries
            .values()
            .find(|e| e.key == key)
            .and_then(|e| e.current.as_ref())
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

/// Button text: the title (the app ID for an untitled window). GNOME 2's
/// libwnck put a minimized window's title in brackets; the owner prefers
/// the state shown by the button's look instead (HMP-06, class
/// `minimized`), so the text is the title alone.
fn button_text(info: &Info) -> String {
    match (info.title.is_empty(), info.app_id.is_empty()) {
        (false, _) => info.title.clone(),
        (true, false) => info.app_id.clone(),
        (true, true) => "Unbenanntes Fenster".to_owned(),
    }
}

/// Tooltip: the full title, and the state where the look alone shows it.
fn tooltip_text(info: &Info) -> String {
    let text = button_text(info);
    if info.minimized {
        format!("{text} (minimiert)")
    } else {
        text
    }
}

/// Width of a button in the window list (HMP-06): as in GNOME 2 all
/// buttons are equally wide, the available width shared out, but never
/// wider than [`MAX_BUTTON_WIDTH`]. Below [`MIN_BUTTON_WIDTH`] per button
/// they still shrink to fit (the title ellipsized down to the icon) rather
/// than run out of the panel.
fn button_width(available: i32, count: usize, spacing: i32) -> i32 {
    let Ok(count) = i32::try_from(count) else {
        return 1;
    };
    if count == 0 {
        return 0;
    }
    let share = (available - spacing * (count - 1)) / count;
    share.clamp(1, MAX_BUTTON_WIDTH)
}

/// Widest button, the width of a window button in the GNOME 2 reference
/// (~185 px on a 1366 px panel) rounded up.
const MAX_BUTTON_WIDTH: i32 = 200;
/// Narrowest useful button (icon and a little of the title): what the list
/// asks for at least while it has windows, so it never disappears.
const MIN_BUTTON_WIDTH: i32 = 32;
/// Gap between two buttons, as in the reference.
const BUTTON_SPACING: i32 = 2;

/// What a click on a window's button does, as in GNOME 2: the active
/// window is minimized, a minimized one restored and activated, any other
/// activated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Click {
    Activate,
    Restore,
    Minimize,
}

fn click_action(info: &Info) -> Click {
    if info.minimized {
        Click::Restore
    } else if info.activated {
        Click::Minimize
    } else {
        Click::Activate
    }
}

/// Dispatch target of the event queue.
struct Protocol {
    windows: Windows<ZwlrForeignToplevelHandleV1>,
    finished: bool,
    /// The workspaces (G2-AK); their changes go into `windows.events`.
    workspaces: Flaechen<ExtWorkspaceHandleV1>,
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

/// The answer to the `sync` sent after binding: the windows reported so far
/// were open before the list started.
impl Dispatch<WlCallback, ()> for Protocol {
    fn event(
        state: &mut Self,
        _: &WlCallback,
        event: wl_callback::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_callback::Event::Done { .. } = event {
            state.windows.events.push(Ereignis::Bereit);
        }
    }
}

impl Dispatch<ExtWorkspaceManagerV1, ()> for Protocol {
    fn event(
        state: &mut Self,
        _: &ExtWorkspaceManagerV1,
        event: workspace_manager::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            workspace_manager::Event::Workspace { workspace } => state.workspaces.neu(workspace),
            workspace_manager::Event::Done => {
                if let Some(active) = state.workspaces.fertig() {
                    state.windows.events.push(Ereignis::Flaeche(active));
                }
            }
            // without workspaces every window is shown again
            workspace_manager::Event::Finished => {
                state.windows.events.push(Ereignis::Flaeche(None));
            }
            _ => {}
        }
    }

    event_created_child!(Protocol, ExtWorkspaceManagerV1, [
        workspace_manager::EVT_WORKSPACE_GROUP_OPCODE => (ExtWorkspaceGroupHandleV1, ()),
        workspace_manager::EVT_WORKSPACE_OPCODE => (ExtWorkspaceHandleV1, ()),
    ]);
}

impl Dispatch<ExtWorkspaceGroupHandleV1, ()> for Protocol {
    fn event(
        _: &mut Self,
        proxy: &ExtWorkspaceGroupHandleV1,
        event: workspace_group::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let workspace_group::Event::Removed = event {
            proxy.destroy();
        }
    }
}

impl Dispatch<ExtWorkspaceHandleV1, ()> for Protocol {
    fn event(
        state: &mut Self,
        proxy: &ExtWorkspaceHandleV1,
        event: workspace::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            workspace::Event::Name { name } => state.workspaces.name(proxy, name),
            workspace::Event::State { state: bits } => {
                let bits = match bits {
                    WEnum::Value(value) => value.bits(),
                    WEnum::Unknown(raw) => raw,
                };
                state
                    .workspaces
                    .aktiv(proxy, bits & workspace::State::Active.bits() != 0);
            }
            workspace::Event::Removed => {
                state.workspaces.entfernt(proxy);
                proxy.destroy();
            }
            _ => {}
        }
    }
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
    container: glib::WeakRef<WindowRow>,
    /// `None` once the connection has failed or was never there.
    session: Option<Session>,
    buttons: HashMap<WindowKey, Button>,
    /// The fd source while it is attached. Its closure holds the only
    /// strong reference to the list.
    source: Option<glib::SourceId>,
    /// G2-AK: which workspace each window is on, and whether to show all.
    placement: Zuordnung<WindowKey>,
    all_workspaces: bool,
    /// What is shown may have changed although no window did.
    refilter: bool,
    /// Pending [`Zuordnung::festschreiben`] after a workspace change.
    settle: Option<glib::SourceId>,
}

type Shared = Rc<RefCell<WindowList>>;

/// The window list for the bottom panel. Without a compositor that offers
/// the protocol it stays empty.
pub fn widget() -> WindowRow {
    let container: WindowRow = glib::Object::new();
    container.set_hexpand(true);
    container.set_overflow(gtk4::Overflow::Hidden);
    // Its background is free panel space for the panel menu (QREL-04d):
    // the list fills the bottom panel, which otherwise had almost none.
    container.add_css_class(crate::panel_menu::FREE_SPACE_CLASS);
    // a little air before the next applet (workspace switcher), as in GNOME 2
    container.set_margin_end(6);
    let all_workspaces = fensterliste::alle_arbeitsflaechen();
    let list = Rc::new(RefCell::new(WindowList {
        container: container.downgrade(),
        session: None,
        buttons: HashMap::new(),
        source: None,
        placement: Zuordnung::default(),
        all_workspaces,
        refilter: false,
        settle: None,
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
                let (source, settle) = list
                    .upgrade()
                    .map(|list| {
                        let mut list = list.borrow_mut();
                        (list.source.take(), list.settle.take())
                    })
                    .unwrap_or_default();
                if let Some(source) = source {
                    source.remove();
                }
                if let Some(settle) = settle {
                    settle.remove();
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
    let workspaces: Option<ExtWorkspaceManagerV1> = globals.bind(&qh, 1..=1, ()).ok();
    // answered after everything labwc reports on binding (G2-AK)
    conn.display().sync(&qh, ());
    conn.flush().map_err(|e| e.to_string())?;
    eprintln!(
        "querencia-panel: Fensterliste über zwlr_foreign_toplevel_manager_v1 v{}, {}",
        manager.version(),
        match (&workspaces, fensterliste::alle_arbeitsflaechen()) {
            (_, true) => "Fenster aller Arbeitsflächen (alle_arbeitsflaechen=ja)",
            (Some(_), false) => "nur Fenster der aktuellen Arbeitsfläche",
            (None, false) => "kein ext_workspace_manager_v1, Fenster aller Arbeitsflächen",
        }
    );
    let protocol = Protocol {
        windows: Windows::new(),
        finished: false,
        workspaces: Flaechen::default(),
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
            place(list);
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

/// Applies the window and workspace events of the last batch to the
/// placement (G2-AK) and, after a workspace change, settles it later.
fn place(list: &Shared) {
    let mut guard = list.borrow_mut();
    let state = &mut *guard;
    let Some(session) = state.session.as_mut() else {
        return;
    };
    let mut settle = false;
    let mut changed_workspace = None;
    for event in std::mem::take(&mut session.protocol.windows.events) {
        let effect = state.placement.anwenden(event);
        state.refilter |= effect.neu_filtern;
        settle |= effect.festschreiben_planen;
        if let Ereignis::Flaeche(Some(number)) = event {
            changed_workspace = Some(number);
        }
    }
    if let Some(number) = changed_workspace {
        let name = session.protocol.workspaces.name_von(number).unwrap_or("");
        eprintln!("querencia-panel: Fensterliste: Arbeitsfläche „{name}“");
    }
    if settle {
        if let Some(old) = state.settle.take() {
            old.remove();
        }
        let weak = Rc::downgrade(list);
        state.settle = Some(glib::timeout_add_local_once(
            std::time::Duration::from_millis(fensterliste::NACHZIEHEN_MS.into()),
            move || {
                if let Some(list) = weak.upgrade() {
                    {
                        let mut list = list.borrow_mut();
                        list.settle = None;
                        list.placement.festschreiben();
                        list.refilter = true;
                    }
                    sync(&list);
                }
            },
        ));
    }
}

/// Makes the buttons match the completed window state, showing the
/// windows the placement lets through (G2-AK).
fn sync(list: &Shared) {
    let (snapshot, container) = {
        let mut guard = list.borrow_mut();
        let state = &mut *guard;
        let refilter = std::mem::take(&mut state.refilter);
        let snapshot = match state.session.as_mut() {
            Some(session) => {
                let windows = &mut session.protocol.windows;
                (windows.take_changed() || refilter).then(|| windows.snapshot())
            }
            None => Some(Vec::new()),
        };
        let snapshot = snapshot.map(|all| {
            let total = all.len();
            let shown: Vec<_> = all
                .into_iter()
                .filter(|(key, _)| state.placement.sichtbar(*key, state.all_workspaces))
                .collect();
            (total, shown)
        });
        (snapshot, state.container.upgrade())
    };
    let (Some((total, snapshot)), Some(container)) = (snapshot, container) else {
        return;
    };
    log_shown(&list.borrow(), total, &snapshot);
    let mut stale: HashMap<_, _> = std::mem::take(&mut list.borrow_mut().buttons);
    let mut buttons = HashMap::new();
    let mut previous: Option<Button> = None;
    for (key, info) in snapshot {
        let button = stale.remove(&key).unwrap_or_else(|| {
            let button = new_button(Rc::downgrade(list), key);
            button.set_parent(&container);
            button
        });
        // in the order of the windows' appearance, also for a button shown
        // again after a workspace change
        button.insert_after(&container, previous.as_ref());
        update_button(&button, &info);
        buttons.insert(key, button.clone());
        previous = Some(button);
    }
    for button in stale.into_values() {
        button.unparent();
    }
    container.queue_resize();
    list.borrow_mut().buttons = buttons;
}

/// Logs which windows are shown when that set changes and the list filters
/// by workspace; the app ID, never the title (it may be private).
fn log_shown(list: &WindowList, total: usize, shown: &[(WindowKey, Info)]) {
    if list.all_workspaces || list.placement.aktuelle().is_none() {
        return;
    }
    let mut before: Vec<WindowKey> = list.buttons.keys().copied().collect();
    before.sort();
    let now: Vec<WindowKey> = shown.iter().map(|(key, _)| *key).collect();
    if before == now {
        return;
    }
    let ids: Vec<String> = shown
        .iter()
        .map(|(key, info)| format!("{} {}", key.0, info.app_id))
        .collect();
    eprintln!(
        "querencia-panel: Fensterliste: {} von {total} Fenstern sichtbar ({})",
        shown.len(),
        ids.join(", ")
    );
}

/// Icon size in a window button, as in GNOME 2.
const ICON_SIZE: i32 = 16;

fn new_button(list: Weak<RefCell<WindowList>>, key: WindowKey) -> Button {
    let icon = Image::builder().pixel_size(ICON_SIZE).build();
    icon.add_css_class("window-icon");
    let label = Label::builder()
        .ellipsize(pango::EllipsizeMode::End)
        .xalign(0.0)
        .hexpand(true)
        .build();
    let content = GtkBox::new(Orientation::Horizontal, 4);
    content.append(&icon);
    content.append(&label);
    let button = Button::builder().child(&content).build();
    // added, not set: GTK's own classes of the button stay (HMP-04 lesson)
    button.add_css_class("window-button");
    button.connect_clicked(move |_| {
        if let Some(list) = list.upgrade() {
            click(&list.borrow(), key);
        }
    });
    button
}

/// The icon and label of a button made by [`new_button`].
fn parts(button: &Button) -> Option<(Image, Label)> {
    let content = button.child()?;
    let icon = content.first_child()?.downcast::<Image>().ok()?;
    let label = content.last_child()?.downcast::<Label>().ok()?;
    Some((icon, label))
}

fn update_button(button: &Button, info: &Info) {
    if let Some((icon, label)) = parts(button) {
        label.set_label(&button_text(info));
        icon.set_from_gicon(&crate::window_icons::icon_for(&info.app_id));
    }
    button.set_tooltip_text(Some(&tooltip_text(info)));
    for (class, on) in [
        ("active", info.activated && !info.minimized),
        ("minimized", info.minimized),
    ] {
        if on {
            button.add_css_class(class);
        } else {
            button.remove_css_class(class);
        }
    }
}

/// Sends what a click on the window's button asks for ([`click_action`]).
/// The look follows only the state labwc reports back; a closed window's
/// key finds nothing.
fn click(list: &WindowList, key: WindowKey) {
    let Some(session) = &list.session else { return };
    let windows = &session.protocol.windows;
    let (Some(handle), Some(info)) = (windows.handle(key), windows.info(key)) else {
        return;
    };
    let action = click_action(info);
    match action {
        Click::Activate => handle.activate(&session.seat),
        Click::Restore => {
            handle.unset_minimized();
            handle.activate(&session.seat);
        }
        Click::Minimize => handle.set_minimized(),
    }
    match flush(&session.conn) {
        Ok(()) => eprintln!(
            "querencia-panel: Fenster {}: {} angefragt",
            key.0,
            match action {
                Click::Activate => "Aktivierung",
                Click::Restore => "Wiederherstellen",
                Click::Minimize => "Minimieren",
            }
        ),
        Err(err) => eprintln!("querencia-panel: Anfrage nicht gesendet: {err}"),
    }
}

/// The row of window buttons: lays its children out side by side, all
/// equally wide ([`button_width`]) and as high as the row.
mod row {
    use super::*;

    #[derive(Default)]
    pub struct WindowRow;

    #[glib::object_subclass]
    impl ObjectSubclass for WindowRow {
        const NAME: &'static str = "QuerenciaWindowRow";
        type Type = super::WindowRow;
        type ParentType = gtk4::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("windowlist");
        }
    }

    impl ObjectImpl for WindowRow {
        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for WindowRow {
        fn measure(&self, orientation: Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            let children = children(&self.obj());
            if orientation == Orientation::Horizontal {
                // as wide as the classic width of all buttons, if there is
                // room; at least one narrow button, never forcing the panel
                let n = i32::try_from(children.len()).unwrap_or(i32::MAX / MAX_BUTTON_WIDTH);
                let natural = n * MAX_BUTTON_WIDTH + (n - 1).max(0) * BUTTON_SPACING;
                (MIN_BUTTON_WIDTH.min(natural), natural, -1, -1)
            } else {
                let (min, nat) = children.iter().fold((0, 0), |(min, nat), child| {
                    let (m, n, _, _) = child.measure(orientation, for_size);
                    (min.max(m), nat.max(n))
                });
                (min, nat, -1, -1)
            }
        }

        fn size_allocate(&self, width: i32, height: i32, _baseline: i32) {
            let children = children(&self.obj());
            let share = button_width(width, children.len(), BUTTON_SPACING);
            let mut x = 0;
            for child in children {
                let (min, _, _, _) = child.measure(Orientation::Horizontal, height);
                // never below what the button needs (icon and padding):
                // with too many windows the row is clipped at the end
                let w = share.max(min);
                child.size_allocate(&gtk4::Allocation::new(x, 0, w, height), -1);
                x += w + BUTTON_SPACING;
            }
        }
    }

    fn children(row: &super::WindowRow) -> Vec<gtk4::Widget> {
        let mut list = Vec::new();
        let mut child = row.first_child();
        while let Some(widget) = child {
            child = widget.next_sibling();
            if widget.should_layout() {
                list.push(widget);
            }
        }
        list
    }
}

glib::wrapper! {
    /// The window list's row of buttons (`windowlist` in CSS).
    pub struct WindowRow(ObjectSubclass<row::WindowRow>)
        @extends gtk4::Widget,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget;
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

    /// G2-AK: `done` reports appearing, focus and closing for the
    /// workspace placement, in order and only on a change.
    #[test]
    fn done_reports_appearing_focus_and_closing() {
        use crate::fensterliste::Ereignis::*;
        let mut windows = Windows::new();
        windows.added(1);
        windows.state(&1, &state(true, false));
        windows.done(&1);
        let key = windows.snapshot()[0].0;
        windows.added(2);
        windows.done(&2);
        let other = windows.snapshot()[1].0;
        // title only: no event
        windows.title(&1, "neu".into());
        windows.done(&1);
        windows.state(&1, &state(false, false));
        windows.done(&1);
        windows.state(&2, &state(true, false));
        windows.done(&2);
        windows.closed(&2);
        // closed before its first done: never shown, never reported
        windows.added(3);
        windows.closed(&3);
        assert_eq!(
            std::mem::take(&mut windows.events),
            [
                Erschienen(key),
                Aktiviert(key),
                Erschienen(other),
                Deaktiviert(key),
                Aktiviert(other),
                Geschlossen(other),
            ]
        );
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
        // minimized: the title stays as it is (HMP-06), the state is kept
        assert_eq!(titles(&windows), ["neu"]);
        assert!(windows.snapshot()[0].1.minimized);

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
        // HMP-06: no brackets; minimized is shown by the look (class)
        info.minimized = true;
        assert_eq!(button_text(&info), "Dokument");
        info.title.clear();
        assert_eq!(button_text(&info), "editor");
        info.app_id.clear();
        info.minimized = false;
        assert_eq!(button_text(&info), "Unbenanntes Fenster");
    }

    #[test]
    fn tooltip_has_the_full_title_and_the_hidden_state() {
        let mut info = Info {
            title: "R1 Abschlussreview austauschen und den Bericht prüfen".into(),
            app_id: "foot".into(),
            ..Info::default()
        };
        assert_eq!(tooltip_text(&info), info.title);
        info.minimized = true;
        assert_eq!(
            tooltip_text(&info),
            "R1 Abschlussreview austauschen und den Bericht prüfen (minimiert)"
        );
        info.title.clear();
        assert_eq!(tooltip_text(&info), "foot (minimiert)");
    }

    #[test]
    fn buttons_share_the_width_up_to_the_classic_maximum() {
        let w = |available, count| button_width(available, count, BUTTON_SPACING);
        // few windows: the classic width, never wider
        assert_eq!(w(1100, 1), MAX_BUTTON_WIDTH);
        assert_eq!(w(1100, 2), MAX_BUTTON_WIDTH);
        assert_eq!(w(1100, 5), MAX_BUTTON_WIDTH);
        // many windows: equal shares that fit, spacing included
        let ten = w(1100, 10);
        assert!((MIN_BUTTON_WIDTH..MAX_BUTTON_WIDTH).contains(&ten));
        assert!(10 * ten + 9 * BUTTON_SPACING <= 1100, "{ten}");
        assert!(
            10 * (ten + 1) + 9 * BUTTON_SPACING > 1100,
            "uses the space: {ten}"
        );
        // narrow panel: still all in, smaller than the minimum
        let tight = w(200, 10);
        assert!(10 * tight + 9 * BUTTON_SPACING <= 200, "{tight}");
        assert!(tight >= 1);
        // nothing to share
        assert_eq!(w(1100, 0), 0);
        assert_eq!(w(0, 3), 1, "a button is never zero wide");
    }

    #[test]
    fn click_minimizes_the_active_window_and_restores_minimized_ones() {
        let mut info = Info {
            title: "x".into(),
            ..Info::default()
        };
        assert_eq!(click_action(&info), Click::Activate);
        info.activated = true;
        assert_eq!(click_action(&info), Click::Minimize);
        info.minimized = true;
        assert_eq!(
            click_action(&info),
            Click::Restore,
            "minimized wins over activated"
        );
        info.activated = false;
        assert_eq!(click_action(&info), Click::Restore);
    }
}
