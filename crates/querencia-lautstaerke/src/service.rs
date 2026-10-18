//! The audio service as the GTK main thread sees it: one connection per
//! process ([`get`]), shared by every widget that shows or changes volumes
//! (the panel applet, the mixer window). Widgets [`Service::subscribe`] and
//! are called with the new [`State`] whenever the audio system reports a
//! change; they [`Service::unsubscribe`] when destroyed.
//!
//! Without an audio service the state is [`Connection::Unavailable`]. The
//! service then watches the server socket (`$XDG_RUNTIME_DIR/pulse/native`,
//! GIO file monitor, inotify) and connects again as soon as it appears or
//! changes, so PipeWire or PulseAudio started later is found without a
//! restart and without polling.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;

use crate::model::{self, Connection, State};
use crate::pulse::Pulse;

type Listener = Rc<dyn Fn(&State)>;

pub struct Service {
    pulse: Option<Pulse>,
    state: RefCell<State>,
    listeners: RefCell<Vec<(u64, Listener)>>,
    next_id: Cell<u64>,
    /// Watches the server socket while there is no service.
    monitor: RefCell<Option<gio::FileMonitor>>,
    /// A reconnect is scheduled.
    reconnecting: Cell<bool>,
    /// Prefix of log lines, e.g. `querencia-panel`.
    log: String,
}

thread_local! {
    static SERVICE: RefCell<Option<Rc<Service>>> = const { RefCell::new(None) };
}

/// The service of this process, created and connected on first use. `log`
/// is the prefix of its log lines; only the first call's counts. Must be
/// called on the GTK main thread.
pub fn get(log: &str) -> Rc<Service> {
    SERVICE.with(|slot| {
        if let Some(service) = slot.borrow().as_ref() {
            return service.clone();
        }
        let service = Rc::new(Service::start(log));
        *slot.borrow_mut() = Some(service.clone());
        service.update();
        service
    })
}

/// Called on the main thread after libpulse's thread woke it.
fn on_wake() {
    let service = SERVICE.with(|slot| slot.borrow().clone());
    if let Some(service) = service {
        service.update();
    }
}

impl Service {
    fn start(log: &str) -> Service {
        let wake = || {
            glib::MainContext::default().invoke(on_wake);
        };
        // The first `update` compares with `Connecting`, so a service that
        // is missing right from the start is reported and watched for too.
        let (pulse, state) = match Pulse::start(wake) {
            Ok(pulse) => (
                Some(pulse),
                State {
                    connection: Connection::Connecting,
                    ..State::default()
                },
            ),
            Err(error) => {
                crate::say!("{log}: kein Audiodienst ({error})");
                (None, State::unavailable(&error))
            }
        };
        Service {
            pulse,
            state: RefCell::new(state),
            listeners: RefCell::new(Vec::new()),
            next_id: Cell::new(1),
            monitor: RefCell::new(None),
            reconnecting: Cell::new(false),
            log: log.to_string(),
        }
    }

    /// The state as last reported.
    pub fn state(&self) -> State {
        self.state.borrow().clone()
    }

    /// Calls `listener` now and after every change, until
    /// [`Service::unsubscribe`] with the returned ID.
    pub fn subscribe(&self, listener: impl Fn(&State) + 'static) -> u64 {
        let id = self.next_id.get();
        self.next_id.set(id + 1);
        let listener: Listener = Rc::new(listener);
        self.listeners.borrow_mut().push((id, listener.clone()));
        listener(&self.state.borrow());
        id
    }

    pub fn unsubscribe(&self, id: u64) {
        self.listeners.borrow_mut().retain(|(i, _)| *i != id);
    }

    /// Reads the state from libpulse and tells every listener.
    fn update(&self) {
        let Some(pulse) = &self.pulse else {
            // No libpulse: the state never changes.
            self.notify();
            return;
        };
        let new = pulse.state();
        let old = self.state.replace(new.clone());
        match watch_change(&old.connection, &new.connection) {
            Some(Watch::Stop) => {
                crate::say!("{}: Audiodienst verbunden ({})", self.log, new.server);
                self.monitor.replace(None);
            }
            Some(Watch::Start) => {
                if let Connection::Unavailable(reason) = &new.connection {
                    crate::say!("{}: kein Audiodienst ({reason})", self.log);
                }
                self.watch_socket();
            }
            None => {}
        }
        if old.output().map(|d| (&d.name, d.percent(), d.muted))
            != new.output().map(|d| (&d.name, d.percent(), d.muted))
        {
            match new.output() {
                Some(d) => crate::say!(
                    "{}: Ausgabe {} ({}): {} %{}",
                    self.log,
                    d.label(),
                    d.name,
                    d.percent(),
                    if d.muted { ", stumm" } else { "" }
                ),
                None if new.ready() => crate::say!("{}: kein Ausgabegerät", self.log),
                None => {}
            }
        }
        self.notify();
    }

    fn notify(&self) {
        let listeners: Vec<Listener> = self
            .listeners
            .borrow()
            .iter()
            .map(|(_, l)| l.clone())
            .collect();
        let state = self.state.borrow().clone();
        for listener in listeners {
            listener(&state);
        }
    }

    /// The server socket libpulse connects to by default, if it is a local
    /// one: `$PULSE_SERVER` as `unix:<path>`, else
    /// `$PULSE_RUNTIME_PATH/native` or `$XDG_RUNTIME_DIR/pulse/native`.
    fn socket_path() -> Option<PathBuf> {
        socket_path_from(|name| std::env::var(name).ok())
    }

    fn watch_socket(&self) {
        if self.pulse.is_none() || self.monitor.borrow().is_some() {
            return;
        }
        let Some(path) = Self::socket_path() else {
            crate::say!("{}: Audiodienst nicht lokal, keine Überwachung", self.log);
            return;
        };
        let file = gio::File::for_path(&path);
        match file.monitor_file(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE) {
            Ok(monitor) => {
                monitor.connect_changed(|_, _, _, event| {
                    if matches!(
                        event,
                        gio::FileMonitorEvent::Created
                            | gio::FileMonitorEvent::Changed
                            | gio::FileMonitorEvent::AttributeChanged
                    ) {
                        let service = SERVICE.with(|slot| slot.borrow().clone());
                        if let Some(service) = service {
                            service.schedule_reconnect();
                        }
                    }
                });
                crate::say!("{}: warte auf {}", self.log, path.display());
                self.monitor.replace(Some(monitor));
            }
            Err(err) => crate::say!("{}: {} nicht überwachbar: {err}", self.log, path.display()),
        }
    }

    /// Connects again shortly after the socket appeared (the server may
    /// need a moment to accept), once for a burst of events.
    fn schedule_reconnect(&self) {
        if self.reconnecting.replace(true) {
            return;
        }
        glib::timeout_add_local_once(Duration::from_millis(300), || {
            let service = SERVICE.with(|slot| slot.borrow().clone());
            if let Some(service) = service {
                service.reconnecting.set(false);
                if !service.state.borrow().ready()
                    && let Some(pulse) = &service.pulse
                {
                    crate::say!("{}: Audiodienst erschienen, verbinde", service.log);
                    pulse.reconnect();
                }
            }
        });
    }

    /// Sets the volume of the default output device to `percent` percent,
    /// keeping its balance. Returns whether a request went out.
    pub fn set_output_percent(&self, percent: u32) -> bool {
        let state = self.state.borrow();
        let Some(device) = state.output() else {
            return false;
        };
        let volume = model::scaled(&device.volume, percent);
        let name = device.name.clone();
        drop(state);
        self.request(|p| p.set_device_volume(true, &name, &volume))
    }

    /// Mutes or unmutes the default output device.
    pub fn set_output_muted(&self, muted: bool) -> bool {
        let Some(name) = self.state.borrow().output().map(|d| d.name.clone()) else {
            return false;
        };
        self.request(|p| p.set_device_muted(true, &name, muted))
    }

    /// Mutes the default output device if it is not, else unmutes it.
    pub fn toggle_output_mute(&self) -> bool {
        let Some(muted) = self.state.borrow().output().map(|d| d.muted) else {
            return false;
        };
        self.set_output_muted(!muted)
    }

    /// Sets the volume of the output (`sink`) or input device `name`.
    pub fn set_device_percent(&self, sink: bool, name: &str, percent: u32) -> bool {
        let state = self.state.borrow();
        let devices = if sink { &state.sinks } else { &state.sources };
        let Some(device) = devices.iter().find(|d| d.name == name) else {
            return false;
        };
        let volume = model::scaled(&device.volume, percent);
        drop(state);
        self.request(|p| p.set_device_volume(sink, name, &volume))
    }

    pub fn set_device_muted(&self, sink: bool, name: &str, muted: bool) -> bool {
        self.request(|p| p.set_device_muted(sink, name, muted))
    }

    /// Makes `name` the default output (`sink`) or input device.
    pub fn set_default(&self, sink: bool, name: &str) -> bool {
        crate::say!(
            "{}: Standard-{} → {name}",
            self.log,
            if sink { "Ausgabe" } else { "Eingabe" }
        );
        self.request(|p| p.set_default(sink, name))
    }

    /// Sets the volume of a playing (`playback`) or recording stream.
    pub fn set_stream_percent(&self, playback: bool, index: u32, percent: u32) -> bool {
        let state = self.state.borrow();
        let streams = if playback {
            &state.playback
        } else {
            &state.recording
        };
        let Some(stream) = streams.iter().find(|s| s.index == index) else {
            return false;
        };
        let volume = model::scaled(&stream.volume, percent);
        drop(state);
        self.request(|p| p.set_stream_volume(playback, index, &volume))
    }

    pub fn set_stream_muted(&self, playback: bool, index: u32, muted: bool) -> bool {
        self.request(|p| p.set_stream_muted(playback, index, muted))
    }

    fn request(&self, request: impl FnOnce(&Pulse) -> bool) -> bool {
        self.pulse.as_ref().is_some_and(request)
    }
}

/// What a change of the connection means for watching the server socket.
#[derive(Debug, PartialEq, Eq)]
enum Watch {
    /// Connected: stop watching.
    Stop,
    /// The service is (still or newly) missing: watch the socket.
    Start,
}

/// [`Watch`] for a change of the connection from `old` to `new`; `None`
/// if nothing changes (e.g. a reconnect that failed again keeps watching).
fn watch_change(old: &Connection, new: &Connection) -> Option<Watch> {
    match (old, new) {
        (Connection::Ready, Connection::Ready) => None,
        (_, Connection::Ready) => Some(Watch::Stop),
        (Connection::Unavailable(_), Connection::Unavailable(_)) => None,
        (_, Connection::Unavailable(_)) => Some(Watch::Start),
        (_, Connection::Connecting) => None,
    }
}

/// [`Service::socket_path`] with the environment given by `var`.
fn socket_path_from(var: impl Fn(&str) -> Option<String>) -> Option<PathBuf> {
    if let Some(server) = var("PULSE_SERVER").filter(|s| !s.trim().is_empty()) {
        // Only a single local socket can be watched.
        return server
            .strip_prefix("unix:")
            .filter(|p| p.starts_with('/'))
            .map(PathBuf::from);
    }
    if let Some(dir) = var("PULSE_RUNTIME_PATH").filter(|s| s.starts_with('/')) {
        return Some(PathBuf::from(dir).join("native"));
    }
    var("XDG_RUNTIME_DIR")
        .filter(|s| s.starts_with('/'))
        .map(|dir| PathBuf::from(dir).join("pulse").join("native"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |name| {
            pairs
                .iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| v.to_string())
        }
    }

    #[test]
    fn a_service_missing_from_the_start_is_watched_for() {
        let missing = || Connection::Unavailable("Connection refused".into());
        // the first update after start compares with Connecting
        assert_eq!(
            watch_change(&Connection::Connecting, &missing()),
            Some(Watch::Start)
        );
        assert_eq!(
            watch_change(&Connection::Connecting, &Connection::Ready),
            Some(Watch::Stop)
        );
        // lost later, found again
        assert_eq!(
            watch_change(&Connection::Ready, &missing()),
            Some(Watch::Start)
        );
        assert_eq!(
            watch_change(&missing(), &Connection::Ready),
            Some(Watch::Stop)
        );
        // a failed reconnect keeps the watch, no new log line
        assert_eq!(watch_change(&missing(), &missing()), None);
        assert_eq!(watch_change(&missing(), &Connection::Connecting), None);
        assert_eq!(watch_change(&Connection::Ready, &Connection::Ready), None);
    }

    #[test]
    fn the_socket_is_found_as_libpulse_finds_it() {
        assert_eq!(
            socket_path_from(env(&[("XDG_RUNTIME_DIR", "/run/user/1001")])),
            Some(PathBuf::from("/run/user/1001/pulse/native"))
        );
        assert_eq!(
            socket_path_from(env(&[
                ("XDG_RUNTIME_DIR", "/run/user/1001"),
                ("PULSE_RUNTIME_PATH", "/tmp/p")
            ])),
            Some(PathBuf::from("/tmp/p/native"))
        );
        assert_eq!(
            socket_path_from(env(&[
                ("XDG_RUNTIME_DIR", "/run/user/1001"),
                ("PULSE_SERVER", "unix:/tmp/x/native")
            ])),
            Some(PathBuf::from("/tmp/x/native"))
        );
        // remote or unusual servers cannot be watched
        assert_eq!(
            socket_path_from(env(&[
                ("XDG_RUNTIME_DIR", "/run/user/1001"),
                ("PULSE_SERVER", "tcp:host:4713")
            ])),
            None
        );
        assert_eq!(socket_path_from(env(&[("XDG_RUNTIME_DIR", "rel")])), None);
        assert_eq!(socket_path_from(env(&[])), None);
    }
}
