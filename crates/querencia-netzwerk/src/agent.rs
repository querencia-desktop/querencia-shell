//! A small NetworkManager secret agent: when NetworkManager needs the key
//! of a WLAN (a saved profile whose key lives with an agent, or a wrong
//! key), it calls `GetSecrets` here, and the panel asks the user in a
//! simple password dialog ([`Prompt`]). Only WLAN keys (`psk`) are
//! answered; everything else (802.1X, VPN) is refused with `NoSecrets`, as
//! the applet has no dialog for it. Nothing is stored here: the key goes
//! back to NetworkManager, which keeps it with the profile.
//!
//! Only NetworkManager itself (its unique bus name) may call the agent.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gtk4::gio::{self, DBusConnection, DBusMethodInvocation};
use gtk4::glib::{self, Variant, prelude::*};

use crate::model::Security;
use crate::props;

pub const AGENT_PATH: &str = "/org/freedesktop/NetworkManager/SecretAgent";
const AGENT_MANAGER_PATH: &str = "/org/freedesktop/NetworkManager/AgentManager";
const AGENT_MANAGER: &str = "org.freedesktop.NetworkManager.AgentManager";
/// Identifier this agent registers with.
pub const IDENTIFIER: &str = "org.querencia.Panel.Netzwerk";
const WIRELESS_SECURITY: &str = "802-11-wireless-security";

const NO_SECRETS: &str = "org.freedesktop.NetworkManager.SecretAgent.NoSecrets";
const USER_CANCELED: &str = "org.freedesktop.NetworkManager.SecretAgent.UserCanceled";
const AGENT_CANCELED: &str = "org.freedesktop.NetworkManager.SecretAgent.AgentCanceled";
const NOT_AUTHORIZED: &str = "org.freedesktop.NetworkManager.SecretAgent.PermissionDenied";

/// `NMSecretAgentGetSecretsFlags`.
pub const ALLOW_INTERACTION: u32 = 0x1;
pub const REQUEST_NEW: u32 = 0x2;

const XML: &str = r#"<node>
  <interface name="org.freedesktop.NetworkManager.SecretAgent">
    <method name="GetSecrets">
      <arg name="connection" type="a{sa{sv}}" direction="in"/>
      <arg name="connection_path" type="o" direction="in"/>
      <arg name="setting_name" type="s" direction="in"/>
      <arg name="hints" type="as" direction="in"/>
      <arg name="flags" type="u" direction="in"/>
      <arg name="secrets" type="a{sa{sv}}" direction="out"/>
    </method>
    <method name="CancelGetSecrets">
      <arg name="connection_path" type="o" direction="in"/>
      <arg name="setting_name" type="s" direction="in"/>
    </method>
    <method name="SaveSecrets">
      <arg name="connection" type="a{sa{sv}}" direction="in"/>
      <arg name="connection_path" type="o" direction="in"/>
    </method>
    <method name="DeleteSecrets">
      <arg name="connection" type="a{sa{sv}}" direction="in"/>
      <arg name="connection_path" type="o" direction="in"/>
    </method>
  </interface>
</node>"#;

/// What the user is asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prompt {
    /// Network name.
    pub ssid: String,
    pub security: Security,
    /// NetworkManager asks again: the last key was wrong.
    pub retry: bool,
}

/// Answer to a prompt: the key, or `None` when the user cancelled.
pub type Answer = Box<dyn FnOnce(Option<String>)>;
/// Closes an open prompt (NetworkManager withdrew the request).
pub type Close = Box<dyn FnOnce()>;
/// Shows a prompt; calls the answer exactly once; returns how to close the
/// dialog if NetworkManager withdraws.
pub type PromptFn = Rc<dyn Fn(Prompt, Answer) -> Close>;

/// What `GetSecrets` should do with a request, before asking anyone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    Ask(Prompt),
    Refuse(&'static str),
}

/// Decides on a `GetSecrets` call for `setting` with `flags` on the
/// connection settings `connection` (`a{sa{sv}}`).
pub fn decide(connection: &Variant, setting: &str, flags: u32, can_ask: bool) -> Decision {
    if setting != WIRELESS_SECURITY {
        return Decision::Refuse(NO_SECRETS);
    }
    if flags & ALLOW_INTERACTION == 0 || !can_ask {
        return Decision::Refuse(NO_SECRETS);
    }
    let ssid =
        crate::model::ssid_text(&props::interface(connection, "802-11-wireless").bytes("ssid"));
    let key_mgmt = props::interface(connection, WIRELESS_SECURITY).string("key-mgmt");
    let security = match key_mgmt.as_deref() {
        Some("wpa-psk") => Security::WpaPsk,
        Some("sae") => Security::Sae,
        _ => return Decision::Refuse(NO_SECRETS),
    };
    Decision::Ask(Prompt {
        ssid,
        security,
        retry: flags & REQUEST_NEW != 0,
    })
}

/// The `GetSecrets` reply for key `psk`.
pub fn secrets(psk: &str) -> Variant {
    let mut setting: HashMap<String, Variant> = HashMap::new();
    setting.insert("psk".into(), psk.to_variant());
    let all: HashMap<String, HashMap<String, Variant>> =
        HashMap::from([(WIRELESS_SECURITY.to_string(), setting)]);
    (all,).to_variant()
}

type Key = (String, String);
/// Open `GetSecrets` calls by profile and setting, with the dialog's closer.
type Pending = Rc<RefCell<HashMap<Key, (DBusMethodInvocation, Option<Close>)>>>;

/// A registered agent; unregisters itself when dropped.
pub struct Agent {
    connection: DBusConnection,
    registration: Option<gio::RegistrationId>,
    pending: Pending,
}

impl Agent {
    /// Exports the agent on `connection` and registers it with
    /// NetworkManager (unique name `owner`). `prompt` returns the dialog
    /// shower, if the panel has one.
    pub fn start(
        connection: &DBusConnection,
        owner: &str,
        prompt: Rc<dyn Fn() -> Option<PromptFn>>,
        log: &str,
    ) -> Result<Agent, String> {
        let info = gio::DBusNodeInfo::for_xml(XML)
            .map_err(|e| e.to_string())?
            .lookup_interface("org.freedesktop.NetworkManager.SecretAgent")
            .ok_or("Schnittstelle fehlt")?;
        let pending: Pending = Rc::default();
        let owner = owner.to_string();
        let prefix = log.to_string();
        let log = prefix.clone();
        let calls = pending.clone();
        let registration = connection
            .register_object(AGENT_PATH, &info)
            .method_call(move |_, sender, _, _, method, args, invocation| {
                if sender != Some(owner.as_str()) {
                    invocation.return_dbus_error(NOT_AUTHORIZED, "Nur NetworkManager darf fragen");
                    return;
                }
                match method {
                    "GetSecrets" => get_secrets(&calls, &prompt, &log, &args, invocation),
                    "CancelGetSecrets" => {
                        let key = (
                            args.child_value(0).str().unwrap_or_default().to_string(),
                            args.child_value(1).str().unwrap_or_default().to_string(),
                        );
                        crate::say!("{log}: Netzwerk: Passwortabfrage zurückgezogen ({})", key.0);
                        if let Some((call, close)) = calls.borrow_mut().remove(&key) {
                            call.return_dbus_error(AGENT_CANCELED, "Zurückgezogen");
                            if let Some(close) = close {
                                close();
                            }
                        }
                        invocation.return_value(None);
                    }
                    // Nothing is stored by this agent.
                    _ => invocation.return_value(None),
                }
            })
            .build()
            .map_err(|e| e.to_string())?;
        let agent = Agent {
            connection: connection.clone(),
            registration: Some(registration),
            pending,
        };
        let register = connection.clone();
        let log = prefix;
        glib::MainContext::default().spawn_local(async move {
            let result = register
                .call_future(
                    Some(crate::nm::BUS_NAME),
                    AGENT_MANAGER_PATH,
                    AGENT_MANAGER,
                    "Register",
                    Some(&(IDENTIFIER,).to_variant()),
                    None,
                    gio::DBusCallFlags::NONE,
                    10_000,
                )
                .await;
            match result {
                Ok(_) => crate::say!("{log}: Netzwerk: Passwortagent angemeldet"),
                Err(err) => crate::say!(
                    "{log}: Netzwerk: Passwortagent nicht angemeldet: {}",
                    crate::nm::error_text(&err)
                ),
            }
        });
        Ok(agent)
    }
}

fn get_secrets(
    calls: &Pending,
    prompt: &Rc<dyn Fn() -> Option<PromptFn>>,
    log: &str,
    args: &Variant,
    invocation: DBusMethodInvocation,
) {
    let connection = args.child_value(0);
    let path = args.child_value(1).str().unwrap_or_default().to_string();
    let setting = args.child_value(2).str().unwrap_or_default().to_string();
    let flags = args.child_value(4).get::<u32>().unwrap_or(0);
    let shower = prompt();
    match decide(&connection, &setting, flags, shower.is_some()) {
        Decision::Refuse(error) => {
            crate::say!("{log}: Netzwerk: Geheimnis „{setting}“ nicht abgefragt (Flags {flags})");
            invocation.return_dbus_error(error, "Keine Abfrage möglich");
        }
        Decision::Ask(question) => {
            let Some(shower) = shower else { return };
            crate::say!(
                "{log}: Netzwerk: Passwortabfrage für „{}“{}",
                question.ssid,
                if question.retry { " (erneut)" } else { "" }
            );
            let key: Key = (path, setting);
            // A second request for the same profile replaces the first.
            if let Some((old, close)) = calls.borrow_mut().remove(&key) {
                old.return_dbus_error(AGENT_CANCELED, "Ersetzt");
                if let Some(close) = close {
                    close();
                }
            }
            calls.borrow_mut().insert(key.clone(), (invocation, None));
            let answered = calls.clone();
            let answer_key = key.clone();
            let log = log.to_string();
            let close = shower(
                question,
                Box::new(move |answer| {
                    let Some((call, _)) = answered.borrow_mut().remove(&answer_key) else {
                        return;
                    };
                    match answer {
                        Some(psk) => {
                            crate::say!("{log}: Netzwerk: Passwort an NetworkManager übergeben");
                            call.return_value(Some(&secrets(&psk)));
                        }
                        None => {
                            crate::say!("{log}: Netzwerk: Passwortabfrage abgebrochen");
                            call.return_dbus_error(USER_CANCELED, "Abgebrochen");
                        }
                    }
                }),
            );
            if let Some(entry) = calls.borrow_mut().get_mut(&key) {
                entry.1 = Some(close);
            }
        }
    }
}

impl Drop for Agent {
    fn drop(&mut self) {
        for (_, (call, close)) in self.pending.borrow_mut().drain() {
            call.return_dbus_error(AGENT_CANCELED, "Panel beendet den Agenten");
            if let Some(close) = close {
                close();
            }
        }
        if let Some(id) = self.registration.take() {
            let _ = self.connection.unregister_object(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn connection(key_mgmt: Option<&str>) -> Variant {
        let mut wireless: HashMap<String, Variant> = HashMap::new();
        wireless.insert("ssid".into(), b"Heimnetz".to_vec().to_variant());
        let mut all: HashMap<String, HashMap<String, Variant>> =
            HashMap::from([("802-11-wireless".to_string(), wireless)]);
        if let Some(k) = key_mgmt {
            all.insert(
                WIRELESS_SECURITY.into(),
                HashMap::from([("key-mgmt".to_string(), k.to_variant())]),
            );
        }
        all.to_variant()
    }

    #[test]
    fn asks_only_for_wlan_keys_with_interaction_allowed() {
        let c = connection(Some("wpa-psk"));
        assert_eq!(
            decide(&c, WIRELESS_SECURITY, ALLOW_INTERACTION, true),
            Decision::Ask(Prompt {
                ssid: "Heimnetz".into(),
                security: Security::WpaPsk,
                retry: false
            })
        );
        assert_eq!(
            decide(&c, WIRELESS_SECURITY, ALLOW_INTERACTION | REQUEST_NEW, true),
            Decision::Ask(Prompt {
                ssid: "Heimnetz".into(),
                security: Security::WpaPsk,
                retry: true
            })
        );
        assert_eq!(
            decide(&connection(Some("sae")), WIRELESS_SECURITY, 1, true),
            Decision::Ask(Prompt {
                ssid: "Heimnetz".into(),
                security: Security::Sae,
                retry: false
            })
        );
        assert_eq!(
            decide(&c, WIRELESS_SECURITY, 0, true),
            Decision::Refuse(NO_SECRETS)
        );
        assert_eq!(
            decide(&c, WIRELESS_SECURITY, 1, false),
            Decision::Refuse(NO_SECRETS)
        );
        assert_eq!(decide(&c, "802-1x", 1, true), Decision::Refuse(NO_SECRETS));
        assert_eq!(
            decide(&connection(Some("wpa-eap")), WIRELESS_SECURITY, 1, true),
            Decision::Refuse(NO_SECRETS)
        );
        assert_eq!(
            decide(&connection(None), WIRELESS_SECURITY, 1, true),
            Decision::Refuse(NO_SECRETS)
        );
    }

    #[test]
    fn secrets_reply_has_the_psk() {
        let reply = secrets("geheim123");
        assert_eq!(reply.type_().as_str(), "(a{sa{sv}})");
        let sec = props::interface(&reply.child_value(0), WIRELESS_SECURITY);
        assert_eq!(sec.string("psk").as_deref(), Some("geheim123"));
    }

    #[test]
    fn introspection_is_valid() {
        let info = gio::DBusNodeInfo::for_xml(XML).unwrap();
        assert!(
            info.lookup_interface("org.freedesktop.NetworkManager.SecretAgent")
                .is_some()
        );
    }
}
