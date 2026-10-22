//! The workspace switcher in the bottom panel: one button per visible
//! workspace, from `ext_workspace_manager_v1` on a Wayland connection of its
//! own. As in `window_list`, the connection's fd is a GLib source in the
//! GTK main context; there is no thread and no polling.
//!
//! The panel only shows what the compositor reports: a click sends
//! `activate` and `commit`, and the highlight changes when labwc reports
//! the new state.
//!
//! The GNOME 2 pager look (HMP-07, colours HMP-11b): small cells side by
//! side, miniatures of the screen ([`cell_size`]) without numbers, light
//! grey raised rectangles in a sunken frame, the active one in Querencia
//! orange (style in `main.rs`). The name is in the tooltip; the mouse wheel
//! steps through the workspaces.
//!
//! GNOME 2 drew the windows into the cells. labwc 0.9.8 gives a client
//! neither which workspace a window is on (`zwlr_foreign_toplevel_manager_v1`
//! v3, `ext_foreign_toplevel_list_v1` and `ext_workspace_manager_v1` v1 do
//! not link them) nor where a window is or how large (no protocol reports
//! window geometry; `set_rectangle` only goes from client to compositor).
//! So the cells stay empty instead of showing invented windows; evidence in
//!  and the HMP-11b report.

use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::Hash;
use std::os::fd::AsRawFd;
use std::rc::{Rc, Weak};

use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Button, Orientation, glib};
use wayland_client::backend::WaylandError;
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::{
    Connection, Dispatch, EventQueue, Proxy, QueueHandle, WEnum, event_created_child,
};
use wayland_protocols::ext::workspace::v1::client::{
    ext_workspace_group_handle_v1::{self as group, ExtWorkspaceGroupHandleV1},
    ext_workspace_handle_v1::{self as workspace, ExtWorkspaceHandleV1},
    ext_workspace_manager_v1::{self as manager, ExtWorkspaceManagerV1},
};

use crate::i18n::gettext_f;

const ACTIVE: u32 = workspace::State::Active.bits();
const HIDDEN: u32 = workspace::State::Hidden.bits();
const CAN_ACTIVATE: u32 = workspace::WorkspaceCapabilities::Activate.bits();

/// Identity of a workspace for the widgets, in creation order. Never reused
/// while the panel runs, unlike Wayland protocol ids.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct WorkspaceKey(u64);

struct Entry<G> {
    key: WorkspaceKey,
    name: String,
    coordinates: Vec<u32>,
    state: u32,
    capabilities: u32,
    group: Option<G>,
}

/// A workspace as the buttons show it.
#[derive(Clone, Debug, PartialEq)]
struct Shown {
    key: WorkspaceKey,
    name: String,
    active: bool,
    activatable: bool,
}

/// Workspaces and groups as reported by the compositor, keyed by protocol
/// handle (`W`, `G`; plain numbers in the tests). Events change the entries
/// directly; only `done` turns them into the list the buttons show, because
/// the protocol makes all changes up to the manager's `done` one atomic
/// update, whatever order the single events came in.
struct Workspaces<W, G> {
    entries: HashMap<W, Entry<G>>,
    /// In creation order; only the first one is shown.
    groups: Vec<G>,
    next_key: u64,
    shown: Vec<Shown>,
    changed: bool,
    /// Group count last reported as unsupported, to log each change once.
    logged_groups: usize,
}

impl<W: Clone + Eq + Hash, G: Clone + Eq> Workspaces<W, G> {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
            groups: Vec::new(),
            next_key: 0,
            shown: Vec::new(),
            changed: false,
            logged_groups: 1,
        }
    }

    fn added(&mut self, handle: W) {
        self.next_key += 1;
        let entry = Entry {
            key: WorkspaceKey(self.next_key),
            name: String::new(),
            coordinates: Vec::new(),
            state: 0,
            capabilities: 0,
            group: None,
        };
        self.entries.insert(handle, entry);
    }

    fn name(&mut self, handle: &W, name: String) {
        if let Some(entry) = self.entries.get_mut(handle) {
            entry.name = name;
        }
    }

    fn coordinates(&mut self, handle: &W, raw: &[u8]) {
        if let Some(entry) = self.entries.get_mut(handle) {
            entry.coordinates = raw
                .chunks_exact(4)
                .map(|c| u32::from_ne_bytes([c[0], c[1], c[2], c[3]]))
                .collect();
        }
    }

    fn state(&mut self, handle: &W, state: u32) {
        if let Some(entry) = self.entries.get_mut(handle) {
            entry.state = state;
        }
    }

    fn capabilities(&mut self, handle: &W, capabilities: u32) {
        if let Some(entry) = self.entries.get_mut(handle) {
            entry.capabilities = capabilities;
        }
    }

    fn removed(&mut self, handle: &W) {
        self.entries.remove(handle);
    }

    fn group_added(&mut self, group: G) {
        self.groups.push(group);
    }

    fn entered(&mut self, group: &G, handle: &W) {
        if let Some(entry) = self.entries.get_mut(handle) {
            entry.group = Some(group.clone());
        }
    }

    fn left(&mut self, group: &G, handle: &W) {
        if let Some(entry) = self.entries.get_mut(handle)
            && entry.group.as_ref() == Some(group)
        {
            entry.group = None;
        }
    }

    fn group_removed(&mut self, group: &G) {
        self.groups.retain(|g| g != group);
        // The compositor sends workspace_leave first; this only guards
        // against a compositor that does not.
        for entry in self.entries.values_mut() {
            if entry.group.as_ref() == Some(group) {
                entry.group = None;
            }
        }
    }

    /// The visible workspaces of the first group, ordered by coordinates if
    /// every one of them has some, otherwise in creation order.
    fn done(&mut self) {
        if self.groups.len() != self.logged_groups {
            self.logged_groups = self.groups.len();
            if self.groups.len() > 1 {
                eprintln!(
                    "querencia-panel: {} Arbeitsflächengruppen gemeldet; nicht unterstützt, \
                     nur die erste wird gezeigt",
                    self.groups.len()
                );
            }
        }
        let first = self.groups.first();
        let mut visible: Vec<&Entry<G>> = self
            .entries
            .values()
            .filter(|e| first.is_some() && e.group.as_ref() == first && e.state & HIDDEN == 0)
            .collect();
        if visible.iter().all(|e| !e.coordinates.is_empty()) {
            visible.sort_by(|a, b| (&a.coordinates, a.key).cmp(&(&b.coordinates, b.key)));
        } else {
            visible.sort_by_key(|e| e.key);
        }
        let shown: Vec<Shown> = visible
            .into_iter()
            .map(|e| Shown {
                key: e.key,
                name: e.name.clone(),
                active: e.state & ACTIVE != 0,
                activatable: e.capabilities & CAN_ACTIVATE != 0,
            })
            .collect();
        if shown != self.shown {
            self.shown = shown;
            self.changed = true;
        }
    }

    /// The handle to activate for a button: the workspace must be shown as
    /// activatable and, by the events received since the last `done`, still
    /// be activatable, not hidden and in the shown group.
    fn activation(&self, key: WorkspaceKey) -> Option<&W> {
        self.shown.iter().find(|s| s.key == key && s.activatable)?;
        let first = self.groups.first()?;
        self.entries
            .iter()
            .find(|(_, e)| {
                e.key == key
                    && e.capabilities & CAN_ACTIVATE != 0
                    && e.state & HIDDEN == 0
                    && e.group.as_ref() == Some(first)
            })
            .map(|(h, _)| h)
    }

    fn take_changed(&mut self) -> bool {
        std::mem::take(&mut self.changed)
    }
}

/// Height of a workspace cell (HMP-07): the GNOME 2 pager drew 20 px cells
/// in a 24 px panel; 22 px fits the default 32 px panel and panels down to
/// 24 px.
const CELL_HEIGHT: i32 = 22;

/// Size of a workspace cell: a miniature of the screen, `height` high and
/// as wide as the monitor's aspect ratio makes it (16:9 if unknown), at
/// least square and at most three times as wide as high.
fn cell_size(height: i32, monitor: Option<(i32, i32)>) -> (i32, i32) {
    let aspect = match monitor {
        Some((w, h)) if w > 0 && h > 0 => f64::from(w) / f64::from(h),
        _ => 16.0 / 9.0,
    };
    let width = (f64::from(height) * aspect.clamp(1.0, 3.0)).round() as i32;
    (width, height)
}

/// Tooltip of a cell: "Arbeitsfläche N", or the workspace's own name if it
/// has a real one (labwc's default names are just the numbers).
fn cell_tooltip(number: usize, name: &str) -> String {
    if name.is_empty() || name.chars().all(|c| c.is_ascii_digit()) {
        gettext_f("Workspace {number}", &[("number", &number.to_string())])
    } else {
        name.to_owned()
    }
}

/// Where the mouse wheel goes from the active workspace, as in GNOME 2: one
/// workspace on per notch, down/right = next, up/left = previous, no
/// wrap-around, skipping workspaces that cannot be activated.
fn scroll_target(shown: &[Shown], forward: bool) -> Option<WorkspaceKey> {
    let active = shown.iter().position(|s| s.active)?;
    let candidates: Box<dyn Iterator<Item = &Shown>> = if forward {
        Box::new(shown[active + 1..].iter())
    } else {
        Box::new(shown[..active].iter().rev())
    };
    candidates
        .into_iter()
        .find(|s| s.activatable)
        .map(|s| s.key)
}

fn bits<T: Into<u32>>(value: WEnum<T>) -> u32 {
    match value {
        WEnum::Value(value) => value.into(),
        WEnum::Unknown(raw) => raw,
    }
}

/// Dispatch target of the event queue.
struct Protocol {
    workspaces: Workspaces<ExtWorkspaceHandleV1, ExtWorkspaceGroupHandleV1>,
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

impl Dispatch<ExtWorkspaceManagerV1, ()> for Protocol {
    fn event(
        state: &mut Self,
        _: &ExtWorkspaceManagerV1,
        event: manager::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            manager::Event::WorkspaceGroup { workspace_group } => {
                state.workspaces.group_added(workspace_group)
            }
            manager::Event::Workspace { workspace } => state.workspaces.added(workspace),
            manager::Event::Done => state.workspaces.done(),
            manager::Event::Finished => state.finished = true,
            _ => {}
        }
    }

    event_created_child!(Protocol, ExtWorkspaceManagerV1, [
        manager::EVT_WORKSPACE_GROUP_OPCODE => (ExtWorkspaceGroupHandleV1, ()),
        manager::EVT_WORKSPACE_OPCODE => (ExtWorkspaceHandleV1, ()),
    ]);
}

impl Dispatch<ExtWorkspaceGroupHandleV1, ()> for Protocol {
    fn event(
        state: &mut Self,
        proxy: &ExtWorkspaceGroupHandleV1,
        event: group::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let workspaces = &mut state.workspaces;
        match event {
            group::Event::WorkspaceEnter { workspace } => workspaces.entered(proxy, &workspace),
            group::Event::WorkspaceLeave { workspace } => workspaces.left(proxy, &workspace),
            group::Event::Removed => {
                workspaces.group_removed(proxy);
                proxy.destroy();
            }
            _ => {}
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
        let workspaces = &mut state.workspaces;
        match event {
            workspace::Event::Name { name } => workspaces.name(proxy, name),
            workspace::Event::Coordinates { coordinates } => {
                workspaces.coordinates(proxy, &coordinates)
            }
            workspace::Event::State { state } => workspaces.state(proxy, bits(state)),
            workspace::Event::Capabilities { capabilities } => {
                workspaces.capabilities(proxy, bits(capabilities))
            }
            workspace::Event::Removed => {
                workspaces.removed(proxy);
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
    manager: ExtWorkspaceManagerV1,
}

struct Switcher {
    /// Weak, so the switcher never keeps the panel's widgets alive.
    container: glib::WeakRef<GtkBox>,
    /// `None` once the connection has failed or was never there.
    session: Option<Session>,
    buttons: HashMap<WorkspaceKey, Button>,
    /// The fd source while it is attached. Its closure holds the only
    /// strong reference to the switcher.
    source: Option<glib::SourceId>,
}

type Shared = Rc<RefCell<Switcher>>;

/// The workspace switcher for the bottom panel. Without a compositor that
/// offers the protocol it stays empty.
pub fn widget() -> GtkBox {
    let container = GtkBox::new(Orientation::Horizontal, 0);
    container.add_css_class("workspace-pager");
    container.set_valign(gtk4::Align::Center);
    let switcher = Rc::new(RefCell::new(Switcher {
        container: container.downgrade(),
        session: None,
        buttons: HashMap::new(),
        source: None,
    }));
    match connect() {
        Ok(session) => {
            let fd = session.conn.backend().poll_fd().as_raw_fd();
            switcher.borrow_mut().session = Some(session);
            let source = glib_unix::unix_fd_add_local(
                fd,
                glib::IOCondition::IN | glib::IOCondition::HUP | glib::IOCondition::ERR,
                {
                    let switcher = switcher.clone();
                    move |_, condition| on_readable(&switcher, condition)
                },
            );
            switcher.borrow_mut().source = Some(source);
            // Mouse wheel; weak, the fd source holds the only strong reference.
            let scroll = gtk4::EventControllerScroll::new(
                gtk4::EventControllerScrollFlags::BOTH_AXES
                    | gtk4::EventControllerScrollFlags::DISCRETE,
            );
            let weak = Rc::downgrade(&switcher);
            scroll.connect_scroll(move |_, dx, dy| match weak.upgrade() {
                Some(switcher) => on_scroll(&switcher, dx, dy),
                None => glib::Propagation::Proceed,
            });
            container.add_controller(scroll);
            // The switcher ends with the bottom panel: removing the source
            // drops its closure, then the switcher and the connection. A
            // source that already ended with `Break` has given up its id.
            let switcher = Rc::downgrade(&switcher);
            container.connect_destroy(move |_| {
                let source = switcher
                    .upgrade()
                    .and_then(|switcher| switcher.borrow_mut().source.take());
                if let Some(source) = source {
                    source.remove();
                }
            });
        }
        Err(err) => eprintln!("querencia-panel: Arbeitsflächenumschalter nicht verfügbar: {err}"),
    }
    container
}

fn connect() -> Result<Session, String> {
    let conn = Connection::connect_to_env().map_err(|e| e.to_string())?;
    let (globals, queue) = registry_queue_init::<Protocol>(&conn).map_err(|e| e.to_string())?;
    let qh = queue.handle();
    let manager: ExtWorkspaceManagerV1 = globals
        .bind(&qh, 1..=1, ())
        .map_err(|_| "kein ext_workspace_manager_v1".to_owned())?;
    conn.flush().map_err(|e| e.to_string())?;
    eprintln!(
        "querencia-panel: Arbeitsflächen über ext_workspace_manager_v1 v{}",
        manager.version()
    );
    let protocol = Protocol {
        workspaces: Workspaces::new(),
        finished: false,
    };
    Ok(Session {
        conn,
        queue,
        protocol,
        manager,
    })
}

/// Reads and dispatches everything the compositor sent, then brings the
/// buttons up to date if a `done` completed a change. Returning `Break`
/// makes GLib remove the source.
fn on_readable(switcher: &Shared, condition: glib::IOCondition) -> glib::ControlFlow {
    let result = {
        let mut switcher = switcher.borrow_mut();
        match switcher.session.as_mut() {
            Some(session) => dispatch(session, condition),
            None => Err("keine Sitzung".to_owned()),
        }
    };
    match result {
        Ok(()) => {
            sync(switcher);
            glib::ControlFlow::Continue
        }
        Err(err) => {
            eprintln!("querencia-panel: Arbeitsflächenumschalter beendet: {err}");
            // Dropping the session releases the connection and all handles;
            // no stale workspace may stay in the panel. GLib removes the
            // source after `Break`, so its id is dropped and never removed again.
            {
                let mut switcher = switcher.borrow_mut();
                switcher.session = None;
                switcher.source = None;
            }
            sync(switcher);
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
        return Err("der Compositor hat die Arbeitsflächenliste beendet".to_owned());
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
        // them with the next flush. Known limitation, as in window_list:
        // there is no watch for the socket becoming writable.
        Err(WaylandError::Io(e)) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(()),
        Err(err) => Err(format!("Senden: {err}")),
    }
}

/// Makes the buttons match the list completed by the last `done`.
fn sync(switcher: &Shared) {
    let (shown, container) = {
        let mut switcher = switcher.borrow_mut();
        let shown = match switcher.session.as_mut() {
            Some(session) => {
                let workspaces = &mut session.protocol.workspaces;
                workspaces.take_changed().then(|| workspaces.shown.clone())
            }
            None => Some(Vec::new()),
        };
        (shown, switcher.container.upgrade())
    };
    let (Some(shown), Some(container)) = (shown, container) else {
        return;
    };

    let mut stale: HashMap<_, _> = std::mem::take(&mut switcher.borrow_mut().buttons);
    let mut buttons = HashMap::new();
    let mut previous: Option<Button> = None;
    for (index, workspace) in shown.iter().enumerate() {
        let button = stale.remove(&workspace.key).unwrap_or_else(|| {
            let button = new_button(Rc::downgrade(switcher), workspace.key);
            container.append(&button);
            button
        });
        // Keeps the children in list order when coordinates reorder them.
        container.reorder_child_after(&button, previous.as_ref());
        update_button(&button, index + 1, workspace);
        buttons.insert(workspace.key, button.clone());
        previous = Some(button);
    }
    for button in stale.into_values() {
        container.remove(&button);
    }
    switcher.borrow_mut().buttons = buttons;
}

/// Width and height of the first monitor, for the cells' aspect ratio.
fn monitor_size() -> Option<(i32, i32)> {
    let display = gtk4::gdk::Display::default()?;
    let monitor = display
        .monitors()
        .item(0)?
        .downcast::<gtk4::gdk::Monitor>()
        .ok()?;
    let geometry = monitor.geometry();
    Some((geometry.width(), geometry.height()))
}

fn new_button(switcher: Weak<RefCell<Switcher>>, key: WorkspaceKey) -> Button {
    let button = Button::new();
    // added, not set: GTK's own classes of the button stay (HMP-04 lesson)
    button.add_css_class("workspace-cell");
    let (width, height) = cell_size(CELL_HEIGHT, monitor_size());
    button.set_size_request(width, height);
    button.connect_clicked(move |_| {
        if let Some(switcher) = switcher.upgrade() {
            activate(&switcher.borrow(), key);
        }
    });
    button
}

/// The mouse wheel over the pager (see [`scroll_target`]).
fn on_scroll(switcher: &Shared, dx: f64, dy: f64) -> glib::Propagation {
    let forward = if dy != 0.0 { dy > 0.0 } else { dx > 0.0 };
    let target = {
        let switcher = switcher.borrow();
        switcher
            .session
            .as_ref()
            .and_then(|s| scroll_target(&s.protocol.workspaces.shown, forward))
    };
    if let Some(key) = target {
        activate(&switcher.borrow(), key);
    }
    glib::Propagation::Stop
}

fn update_button(button: &Button, number: usize, workspace: &Shown) {
    button.set_tooltip_text(Some(&cell_tooltip(number, &workspace.name)));
    button.set_sensitive(workspace.activatable);
    if workspace.active {
        button.add_css_class("active");
    } else {
        button.remove_css_class("active");
    }
}

/// Asks the compositor to activate the workspace. The highlight follows
/// only the state labwc reports back; a removed workspace's key finds
/// nothing, and an already active one may stay without any event.
fn activate(switcher: &Switcher, key: WorkspaceKey) {
    let Some(session) = &switcher.session else {
        return;
    };
    let Some(handle) = session.protocol.workspaces.activation(key) else {
        return;
    };
    handle.activate();
    session.manager.commit();
    match flush(&session.conn) {
        Ok(()) => eprintln!(
            "querencia-panel: Wechsel zu Arbeitsfläche {} angefragt",
            key.0
        ),
        Err(err) => eprintln!("querencia-panel: Arbeitsflächenwechsel nicht gesendet: {err}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GROUP: u32 = 100;

    fn shown(active: usize, activatable: &[bool]) -> Vec<Shown> {
        activatable
            .iter()
            .enumerate()
            .map(|(i, &activatable)| Shown {
                key: WorkspaceKey(i as u64 + 1),
                name: (i + 1).to_string(),
                active: i == active,
                activatable,
            })
            .collect()
    }

    #[test]
    fn cells_are_miniatures_of_the_screen() {
        assert_eq!(cell_size(22, Some((1920, 1080))), (39, 22));
        assert_eq!(cell_size(22, Some((1280, 720))), (39, 22));
        assert_eq!(cell_size(22, Some((1366, 768))), (39, 22));
        assert_eq!(cell_size(22, Some((1280, 1024))), (28, 22));
        assert_eq!(cell_size(22, None), (39, 22), "16:9 if unknown");
        // nonsense monitor data stays within square .. three times as wide
        assert_eq!(cell_size(22, Some((1080, 1920))), (22, 22));
        assert_eq!(cell_size(22, Some((10000, 100))), (66, 22));
        assert_eq!(cell_size(22, Some((0, 0))), (39, 22));
    }

    #[test]
    fn tooltip_names_the_workspace() {
        assert_eq!(cell_tooltip(1, "1"), "Workspace 1");
        assert_eq!(cell_tooltip(3, ""), "Workspace 3");
        assert_eq!(cell_tooltip(2, "Workspace 2"), "Workspace 2");
        assert_eq!(cell_tooltip(2, "Office"), "Office");
    }

    #[test]
    fn mouse_wheel_steps_through_workspaces_without_wrapping() {
        let four = shown(1, &[true; 4]);
        assert_eq!(scroll_target(&four, true), Some(WorkspaceKey(3)));
        assert_eq!(scroll_target(&four, false), Some(WorkspaceKey(1)));
        let last = shown(3, &[true; 4]);
        assert_eq!(scroll_target(&last, true), None, "no wrap at the end");
        let first = shown(0, &[true; 4]);
        assert_eq!(scroll_target(&first, false), None, "no wrap at the start");
        // a workspace that cannot be activated is skipped
        let gap = shown(0, &[true, false, true, true]);
        assert_eq!(scroll_target(&gap, true), Some(WorkspaceKey(3)));
        // nothing active (changing data) or a single workspace: nothing to do
        let none: Vec<Shown> = shown(9, &[true; 3]);
        assert_eq!(scroll_target(&none, true), None);
        assert_eq!(scroll_target(&shown(0, &[true]), true), None);
        assert_eq!(scroll_target(&[], true), None);
    }

    /// A workspace in GROUP with a name and state, completed by `done`.
    fn add(workspaces: &mut Workspaces<u32, u32>, id: u32, name: &str, state: u32) {
        workspaces.added(id);
        workspaces.capabilities(&id, CAN_ACTIVATE);
        workspaces.entered(&GROUP, &id);
        workspaces.name(&id, name.to_owned());
        workspaces.state(&id, state);
    }

    fn names(workspaces: &Workspaces<u32, u32>) -> Vec<&str> {
        workspaces.shown.iter().map(|s| s.name.as_str()).collect()
    }

    fn active(workspaces: &Workspaces<u32, u32>) -> Vec<bool> {
        workspaces.shown.iter().map(|s| s.active).collect()
    }

    #[test]
    fn events_count_only_with_manager_done() {
        let mut workspaces = Workspaces::new();
        workspaces.group_added(GROUP);
        // labwc sends workspace_enter before name and state.
        add(&mut workspaces, 1, "Workspace 1", ACTIVE);
        assert!(workspaces.shown.is_empty(), "nothing before done");
        assert!(!workspaces.take_changed());

        workspaces.done();
        assert!(workspaces.take_changed());
        assert_eq!(names(&workspaces), ["Workspace 1"]);
        assert_eq!(active(&workspaces), [true]);

        workspaces.name(&1, "Eins".into());
        assert_eq!(names(&workspaces), ["Workspace 1"]);
        workspaces.done();
        assert_eq!(names(&workspaces), ["Eins"]);

        // A done without a difference is no change.
        workspaces.take_changed();
        workspaces.done();
        assert!(!workspaces.take_changed());
    }

    #[test]
    fn switching_between_two_workspaces_shows_one_active() {
        let mut workspaces = Workspaces::new();
        workspaces.group_added(GROUP);
        add(&mut workspaces, 1, "Workspace 1", ACTIVE);
        add(&mut workspaces, 2, "Workspace 2", 0);
        workspaces.done();
        workspaces.take_changed();

        // As observed with labwc: both states, then one done.
        workspaces.state(&1, 0);
        assert_eq!(active(&workspaces), [true, false], "unchanged before done");
        workspaces.state(&2, ACTIVE);
        workspaces.done();
        assert!(workspaces.take_changed());
        assert_eq!(active(&workspaces), [false, true]);

        workspaces.state(&2, 0);
        workspaces.state(&1, ACTIVE);
        workspaces.done();
        assert_eq!(active(&workspaces), [true, false]);
    }

    #[test]
    fn hidden_group_membership_and_order() {
        let mut workspaces = Workspaces::new();
        workspaces.group_added(GROUP);
        workspaces.group_added(200);
        add(&mut workspaces, 3, "drei", 0);
        add(&mut workspaces, 1, "eins", ACTIVE);
        add(&mut workspaces, 2, "versteckt", HIDDEN);
        workspaces.added(4);
        workspaces.name(&4, "ohne Gruppe".into());
        workspaces.added(5);
        workspaces.entered(&200, &5);
        workspaces.name(&5, "zweite Gruppe".into());
        workspaces.done();
        // Creation order, not protocol id; only the first group.
        assert_eq!(names(&workspaces), ["drei", "eins"]);

        // Coordinates of every visible workspace decide the order.
        workspaces.coordinates(&3, &2u32.to_ne_bytes());
        workspaces.coordinates(&1, &1u32.to_ne_bytes());
        workspaces.done();
        assert_eq!(names(&workspaces), ["eins", "drei"]);

        // Leaving the group hides a workspace; the first group's removal
        // moves the second group to the front.
        workspaces.left(&GROUP, &3);
        workspaces.done();
        assert_eq!(names(&workspaces), ["eins"]);
        workspaces.left(&GROUP, &1);
        workspaces.group_removed(&GROUP);
        workspaces.done();
        assert_eq!(names(&workspaces), ["zweite Gruppe"]);
    }

    #[test]
    fn removed_workspace_and_stale_key_activate_nothing() {
        let mut workspaces = Workspaces::new();
        workspaces.group_added(GROUP);
        add(&mut workspaces, 1, "Workspace 1", ACTIVE);
        add(&mut workspaces, 2, "Workspace 2", 0);
        workspaces.done();
        let old = workspaces.shown[1].key;
        assert_eq!(workspaces.activation(old), Some(&2));

        workspaces.left(&GROUP, &2);
        workspaces.removed(&2);
        // Late events for the removed handle are ignored.
        workspaces.name(&2, "verspätet".into());
        workspaces.done();
        assert_eq!(names(&workspaces), ["Workspace 1"]);
        assert_eq!(workspaces.activation(old), None);

        // The compositor hands out protocol id 2 again.
        add(&mut workspaces, 2, "neu", 0);
        workspaces.done();
        let new = workspaces.shown[1].key;
        assert_ne!(old, new);
        assert_eq!(workspaces.activation(old), None);
        assert_eq!(workspaces.activation(new), Some(&2));

        // Changes before the next done already block a click: the button
        // still shows the old state, the model does not.
        workspaces.capabilities(&2, 0);
        assert!(workspaces.shown[1].activatable);
        assert_eq!(workspaces.activation(new), None);
        workspaces.capabilities(&2, CAN_ACTIVATE);
        workspaces.state(&2, HIDDEN);
        assert_eq!(workspaces.activation(new), None);
        workspaces.state(&2, 0);
        workspaces.left(&GROUP, &2);
        assert_eq!(workspaces.activation(new), None);
        workspaces.entered(&GROUP, &2);
        assert_eq!(workspaces.activation(new), Some(&2));

        // Without the activate capability a click sends nothing.
        workspaces.capabilities(&2, 0);
        workspaces.done();
        assert!(!workspaces.shown[1].activatable);
        assert_eq!(workspaces.activation(new), None);
    }
}
