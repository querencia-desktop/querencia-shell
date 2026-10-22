//! The network as the GTK main thread sees it: one service per process
//! ([`get`]), shared by every widget that shows it. Widgets
//! [`Service::subscribe`] and are called with the new [`State`] on every
//! change; they [`Service::unsubscribe`] when destroyed.
//!
//! **Backend choice:** the service watches the bus name
//! `org.freedesktop.NetworkManager` on the system bus (`NameOwnerChanged`
//! through `g_bus_watch_name`). While it has an owner, NetworkManager is
//! read and can be changed (and the secret agent is registered with it);
//! while it has none, or NetworkManager's data cannot be read, the
//! read-only fallback runs. The switch happens at once in both directions,
//! without restarting the panel. Without a system bus at all the fallback
//! runs too.

use crate::i18n::gettext;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4::gio::{self, BusNameWatcherFlags, BusType, DBusConnection};
use gtk4::glib;

use crate::agent::{self, Agent, PromptFn};
use crate::fallback::Fallback;
use crate::model::{AccessPoint, Security, State};
use crate::nm::{self, Nm};

/// Which backend runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    NetworkManager,
    Fallback,
}

/// What happened on the system bus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// `org.freedesktop.NetworkManager` got an owner.
    Appeared,
    /// It lost its owner (or there is no system bus).
    Vanished,
    /// NetworkManager runs, but its state could not be read.
    ReadFailed,
}

/// The backend to run after `event`, if it changes from `current`: an
/// owner of the name means NetworkManager, anything else the fallback; a
/// NetworkManager that cannot be read is treated as missing until its
/// name appears again.
pub fn choose(current: Option<Choice>, event: Event) -> Option<Choice> {
    let next = match event {
        Event::Appeared => Choice::NetworkManager,
        Event::Vanished | Event::ReadFailed => Choice::Fallback,
    };
    (current != Some(next) || event == Event::Appeared).then_some(next)
}

type Listener = Rc<dyn Fn(&State)>;

enum Running {
    None,
    Nm { nm: Rc<Nm>, _agent: Option<Agent> },
    Fallback(Rc<Fallback>),
}

/// Result of an action, for the caller's message: `Err` holds the reason.
pub type Done = Box<dyn FnOnce(Result<(), String>)>;

pub struct Service {
    state: RefCell<State>,
    listeners: RefCell<Vec<(u64, Listener)>>,
    next_id: Cell<u64>,
    running: RefCell<Running>,
    choice: Cell<Option<Choice>>,
    prompt: RefCell<Option<PromptFn>>,
    log: String,
}

thread_local! {
    static SERVICE: RefCell<Option<Rc<Service>>> = const { RefCell::new(None) };
}

/// The service of this process, created and started on first use. `log`
/// is the prefix of its log lines. Must be called on the GTK main thread.
pub fn get(log: &str) -> Rc<Service> {
    SERVICE.with(|slot| {
        if let Some(service) = slot.borrow().as_ref() {
            return service.clone();
        }
        let service = Rc::new(Service {
            state: RefCell::new(State::starting()),
            listeners: RefCell::new(Vec::new()),
            next_id: Cell::new(1),
            running: RefCell::new(Running::None),
            choice: Cell::new(None),
            prompt: RefCell::new(None),
            log: log.to_string(),
        });
        *slot.borrow_mut() = Some(service.clone());
        service.watch();
        service
    })
}

impl Service {
    fn watch(self: &Rc<Self>) {
        let appeared = Rc::downgrade(self);
        let vanished = Rc::downgrade(self);
        // Watched for the life of the process (the service is never
        // dropped).
        let _ = gio::bus_watch_name(
            BusType::System,
            nm::BUS_NAME,
            BusNameWatcherFlags::NONE,
            move |connection, _, owner| {
                if let Some(service) = appeared.upgrade() {
                    service.event(Event::Appeared, Some((connection, owner.to_string())));
                }
            },
            move |_, _| {
                if let Some(service) = vanished.upgrade() {
                    service.event(Event::Vanished, None);
                }
            },
        );
    }

    fn event(self: &Rc<Self>, event: Event, nm: Option<(DBusConnection, String)>) {
        let Some(choice) = choose(self.choice.get(), event) else {
            return;
        };
        self.choice.set(Some(choice));
        // Stop what runs.
        match self.running.replace(Running::None) {
            Running::Nm { nm, .. } => nm.stop(),
            Running::Fallback(f) => f.stop(),
            Running::None => {}
        }
        match (choice, nm) {
            (Choice::NetworkManager, Some((connection, owner))) => {
                crate::say!("{}: Netzwerk: NetworkManager gefunden ({owner})", self.log);
                let weak = Rc::downgrade(self);
                let handle = Nm::start(&connection, &owner, move |result| {
                    let Some(service) = weak.upgrade() else {
                        return;
                    };
                    match result {
                        Ok(state) => service.publish(state),
                        Err(reason) => {
                            crate::say!(
                                "{}: Netzwerk: NetworkManager nicht lesbar ({reason}), nur Anzeige",
                                service.log
                            );
                            service.event(Event::ReadFailed, None);
                        }
                    }
                });
                let weak = Rc::downgrade(self);
                let prompt: Rc<dyn Fn() -> Option<PromptFn>> =
                    Rc::new(move || weak.upgrade().and_then(|s| s.prompt.borrow().clone()));
                let agent = match Agent::start(&connection, &owner, prompt, &self.log) {
                    Ok(agent) => Some(agent),
                    Err(err) => {
                        crate::say!("{}: Netzwerk: kein Passwortagent: {err}", self.log);
                        None
                    }
                };
                self.running.replace(Running::Nm {
                    nm: handle,
                    _agent: agent,
                });
            }
            _ => {
                crate::say!(
                    "{}: Netzwerk: kein NetworkManager, nur Anzeige (Kernel, systemd-networkd)",
                    self.log
                );
                let weak = Rc::downgrade(self);
                let fallback = Fallback::start(&self.log, move |state| {
                    if let Some(service) = weak.upgrade() {
                        service.publish(state);
                    }
                });
                self.running.replace(Running::Fallback(fallback));
            }
        }
    }

    fn publish(&self, state: State) {
        let old = self.state.replace(state.clone());
        if old == state {
            return;
        }
        if old.backend != state.backend {
            crate::say!("{}: Netzwerk: Quelle {:?}", self.log, state.backend);
        }
        let icon = crate::model::icon(&state);
        if crate::model::icon(&old) != icon {
            crate::say!("{}: Netzwerk: Symbol {icon:?}", self.log);
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

    /// Sets the password dialog the secret agent uses.
    pub fn set_prompt(&self, prompt: PromptFn) {
        self.prompt.replace(Some(prompt));
    }

    fn nm(&self) -> Option<Rc<Nm>> {
        match &*self.running.borrow() {
            Running::Nm { nm, .. } => Some(nm.clone()),
            _ => None,
        }
    }

    /// Runs `action` against NetworkManager on the main loop, logs it and
    /// hands the result to `done`. Without NetworkManager `done` gets the
    /// reason at once.
    fn run<F, Fut>(&self, what: String, done: Done, action: F)
    where
        F: FnOnce(Rc<Nm>) -> Fut + 'static,
        Fut: std::future::Future<Output = Result<(), String>> + 'static,
    {
        let Some(nm) = self.nm() else {
            done(Err(gettext(
                "Nothing can be changed without NetworkManager",
            )));
            return;
        };
        let log = self.log.clone();
        crate::say!("{log}: Netzwerk: {what}");
        glib::MainContext::default().spawn_local(async move {
            let result = action(nm.clone()).await;
            match &result {
                Ok(()) => crate::say!("{log}: Netzwerk: {what}: angenommen"),
                Err(reason) => crate::say!("{log}: Netzwerk: {what}: fehlgeschlagen: {reason}"),
            }
            nm.schedule();
            done(result);
        });
    }

    /// Connects WLAN device `device` with `ap`; `password` for a new
    /// secured network.
    pub fn connect_wifi(
        &self,
        device: &str,
        ap: &AccessPoint,
        password: Option<String>,
        done: Done,
    ) {
        let device = device.to_string();
        let ap = ap.clone();
        let what = format!(
            "verbinde mit WLAN „{}“{}",
            ap.ssid,
            if ap.known.is_some() {
                " (gespeichert)"
            } else {
                ""
            }
        );
        self.run(what, done, move |nm| async move {
            nm.connect_wifi(&device, &ap, password.as_deref()).await
        });
    }

    pub fn connect_hidden(
        &self,
        device: &str,
        ssid: &str,
        security: Security,
        password: Option<String>,
        done: Done,
    ) {
        let device = device.to_string();
        let ssid = ssid.to_string();
        let what = format!("verbinde mit verborgenem WLAN „{ssid}“");
        self.run(what, done, move |nm| async move {
            nm.connect_hidden(&device, &ssid, security, password.as_deref())
                .await
        });
    }

    pub fn connect_device(&self, device: &str, done: Done) {
        let device = device.to_string();
        self.run(format!("verbinde {device}"), done, move |nm| async move {
            nm.connect_device(&device).await
        });
    }

    pub fn disconnect(&self, device: &str, done: Done) {
        let device = device.to_string();
        self.run(format!("trenne {device}"), done, move |nm| async move {
            nm.disconnect(&device).await
        });
    }

    pub fn set_wireless(&self, enabled: bool, done: Done) {
        let what = format!("WLAN {}", if enabled { "an" } else { "aus" });
        self.run(what, done, move |nm| async move {
            nm.set_wireless(enabled).await
        });
    }

    pub fn set_airplane(&self, on: bool, done: Done) {
        let what = format!("Flugmodus {}", if on { "an" } else { "aus" });
        self.run(
            what,
            done,
            move |nm| async move { nm.set_airplane(on).await },
        );
    }

    /// Asks every WLAN device to scan (when the menu opens).
    pub fn request_scan(&self) {
        let Some(nm) = self.nm() else { return };
        let devices: Vec<String> = self
            .state
            .borrow()
            .devices
            .iter()
            .filter(|d| d.kind == crate::model::Kind::Wireless)
            .map(|d| d.id.clone())
            .collect();
        let log = self.log.clone();
        for device in devices {
            let nm = nm.clone();
            let log = log.clone();
            glib::MainContext::default().spawn_local(async move {
                if let Err(reason) = nm.request_scan(&device).await {
                    crate::say!("{log}: Netzwerk: Suche nach WLANs nicht gestartet: {reason}");
                }
            });
        }
    }
}

/// The question of [`agent::Prompt`] re-exported for the panel.
pub use agent::{Answer, Close, Prompt};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_follows_the_bus_name() {
        use Choice::*;
        // start: name has an owner, or none
        assert_eq!(choose(None, Event::Appeared), Some(NetworkManager));
        assert_eq!(choose(None, Event::Vanished), Some(Fallback));
        // NetworkManager stops: fallback; starts later: NetworkManager
        assert_eq!(
            choose(Some(NetworkManager), Event::Vanished),
            Some(Fallback)
        );
        assert_eq!(
            choose(Some(Fallback), Event::Appeared),
            Some(NetworkManager)
        );
        // nothing changes twice
        assert_eq!(choose(Some(Fallback), Event::Vanished), None);
        // a new owner (restart) starts NetworkManager anew
        assert_eq!(
            choose(Some(NetworkManager), Event::Appeared),
            Some(NetworkManager)
        );
        // unreadable NetworkManager: fallback until the name appears again
        assert_eq!(
            choose(Some(NetworkManager), Event::ReadFailed),
            Some(Fallback)
        );
        assert_eq!(choose(Some(Fallback), Event::ReadFailed), None);
    }
}
