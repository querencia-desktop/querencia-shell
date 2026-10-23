// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Querencia Desktop contributors
//
// Adapted from Querencia Files (Strata fork, branch `querencia` at
// 3dfdb25f): `src/adapters/local_files/network_devices.rs` (QF-0o) and the
// host-key part of `src/ui/browser/network.rs` (QF-0o-2, follow-up check of
// known_hosts), MIT License as the whole fork (docs/querencia-lizenz.md
// there; `LICENSES/MIT-Strata.txt`). Changes: plain data instead of
// Strata's `FileEntry`/`Location`, URIs as text, the probe blocking with a
// time limit (std::process) instead of a GIO future, no texts (the dialog
// with its wording belongs to the program).

//! Network places without a display:
//!
//! - GVfs lists a device once per discovery service (DNS-SD from Avahi,
//!   WS-Discovery from wsdd), e.g. `nas-test` and `NAS-TEST`. The network
//!   view shows one entry per device and protocol; the DNS-SD target wins,
//!   it carries the resolved address ([`merge_network_entries`], QD-036).
//! - GVfs 1.60 cannot read OpenSSH 10's host-key fingerprint and asks with
//!   "???" (QD-008). The fingerprint is read over a separate `ssh -v` that
//!   stops at host-key verification and offers no credentials
//!   ([`host_key_probe_argv`], [`host_key_from_ssh_log`]); after the user
//!   accepted, the key ssh saved is compared with the one shown
//!   ([`compare_known_hosts`]).

use std::collections::HashMap;
use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiscoveryService {
    DnsSd,
    WsDiscovery,
}

/// One entry of `network:///`: GVfs' own name (`dnssd-…`, `wsdd-…`), the
/// shown name and the target URI (`standard::target-uri`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NetzEintrag {
    pub native_name: String,
    pub display_name: String,
    pub target: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NetworkSource {
    pub service: DiscoveryService,
    pub display_name: String,
    pub target: String,
}

fn discovery_service(native_name: &str) -> Option<DiscoveryService> {
    if native_name.starts_with("dnssd-") {
        Some(DiscoveryService::DnsSd)
    } else if native_name.starts_with("wsdd-") {
        Some(DiscoveryService::WsDiscovery)
    } else {
        None
    }
}

/// The device a shown name means: without case and `.local`.
pub fn device_key(display_name: &str) -> String {
    let lower = display_name.trim().trim_end_matches('.').to_lowercase();
    lower
        .strip_suffix(".local")
        .map(str::to_owned)
        .unwrap_or(lower)
}

fn scheme(target: &str) -> String {
    target
        .split_once(':')
        .map(|(scheme, _)| scheme.to_ascii_lowercase())
        .unwrap_or_default()
}

/// The merged entries and, per kept entry's target, every source that was
/// folded into it (for a tooltip).
pub fn merge_network_entries(
    entries: Vec<NetzEintrag>,
) -> (Vec<NetzEintrag>, HashMap<String, Vec<NetworkSource>>) {
    let mut kept: Vec<NetzEintrag> = Vec::with_capacity(entries.len());
    let mut groups: HashMap<(String, String), (usize, Vec<NetworkSource>)> = HashMap::new();
    for entry in entries {
        let Some(service) = discovery_service(&entry.native_name) else {
            kept.push(entry);
            continue;
        };
        let source = NetworkSource {
            service,
            display_name: entry.display_name.clone(),
            target: entry.target.clone(),
        };
        let key = (device_key(&entry.display_name), scheme(&entry.target));
        match groups.get_mut(&key) {
            Some((index, sources)) => {
                let replaces = service == DiscoveryService::DnsSd
                    && sources
                        .iter()
                        .all(|known| known.service != DiscoveryService::DnsSd);
                if replaces {
                    kept[*index] = entry;
                }
                sources.push(source);
            }
            None => {
                groups.insert(key, (kept.len(), vec![source]));
                kept.push(entry);
            }
        }
    }
    let merged = groups
        .into_values()
        .filter(|(_, sources)| sources.len() > 1)
        .map(|(index, mut sources)| {
            sources.sort_by_key(|source| source.service != DiscoveryService::DnsSd);
            (kept[index].target.clone(), sources)
        })
        .collect();
    (kept, merged)
}

/// A server host key as OpenSSH names and fingerprints it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostKey {
    pub key_type: String,
    pub fingerprint: String,
}

/// GVfs' placeholder when it could not read the server's key fingerprint.
const GVFS_UNKNOWN_FINGERPRINT: &str = "???";

/// Where GVfs' SFTP host-key question quotes the host and the fingerprint, in
/// each GVfs translation whose wording is known.
const GVFS_HOST_KEY_QUESTIONS: &[[(&str, &str); 2]] = &[
    [
        ("Verifying the identity of “", "” failed"),
        ("The identity sent by the remote computer is “", "”"),
    ],
    [
        ("Die Identität von »", "« konnte nicht überprüft werden"),
        (
            "Die vom entfernten Rechner übermittelte Identität lautet »",
            "«",
        ),
    ],
];

/// Whether GVfs' question lacks the fingerprint. GVfs 1.60 expects OpenSSH's
/// older "key fingerprint is SHA256:...." and sends "???" for OpenSSH 10's
/// "key fingerprint is: SHA256:...".
pub fn gvfs_question_lacks_fingerprint(message: &str) -> bool {
    message.contains(GVFS_UNKNOWN_FINGERPRINT)
}

/// The host and the identity GVfs quotes in a known wording of its question.
pub fn gvfs_question_parts(message: &str) -> Option<(String, String)> {
    let between = |(start, end): (&str, &str)| {
        let (_, rest) = message.split_once(start)?;
        rest.split_once(end)
            .map(|(value, _)| value.trim().to_owned())
    };
    GVFS_HOST_KEY_QUESTIONS
        .iter()
        .find_map(|[host, identity]| Some((between(*host)?, between(*identity)?)))
}

/// The SHA256 fingerprint the host-key question showed: the one GVfs quoted,
/// else the one read by the probe.
pub fn shown_fingerprint(message: &str, probed: Option<&HostKey>) -> Option<String> {
    gvfs_question_parts(message)
        .map(|(_, identity)| identity)
        .filter(|identity| is_sha256_fingerprint(identity))
        .or_else(|| probed.map(|key| key.fingerprint.clone()))
}

pub fn is_sha256_fingerprint(value: &str) -> bool {
    value.strip_prefix("SHA256:").is_some_and(|digest| {
        digest.len() == 43
            && digest
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/')
    })
}

/// `[-p port] [-l user] -- host` of an SFTP URI, or `None` when it has no
/// usable host.
fn ssh_target_argv(uri: &str) -> Option<Vec<String>> {
    let parsed = glib::Uri::parse(uri, glib::UriFlags::HAS_PASSWORD).ok()?;
    if parsed.scheme() != "sftp" && parsed.scheme() != "ssh" {
        return None;
    }
    let usable = |value: &str| {
        !value.is_empty()
            && !value.starts_with('-')
            && !value.chars().any(|c| c.is_whitespace() || c.is_control())
    };
    let host = parsed.host().filter(|host| usable(host))?;
    let mut argv = Vec::new();
    if parsed.port() > 0 {
        argv.extend(["-p".to_owned(), parsed.port().to_string()]);
    }
    if let Some(user) = parsed.user().filter(|user| usable(user)) {
        argv.extend(["-l".to_owned(), user.as_str().to_owned()]);
    }
    argv.extend(["--".to_owned(), host.as_str().to_owned()]);
    Some(argv)
}

/// `ssh` arguments that read the SFTP server's host key the way GVfs' own
/// `ssh` does (same user configuration, aliases and algorithm order) and stop
/// at host-key verification: no known_hosts file is read or written. Every
/// authentication method is disabled, so no credentials are offered even if
/// the user configuration would accept the key (NoHostAuthenticationForLocalhost,
/// VerifyHostKeyDNS). `None` when the URI has no usable host.
pub fn host_key_probe_argv(uri: &str) -> Option<Vec<String>> {
    let target = ssh_target_argv(uri)?;
    let mut argv: Vec<String> = [
        "ssh",
        "-v",
        "-oBatchMode=yes",
        "-oStrictHostKeyChecking=yes",
        "-oUserKnownHostsFile=/dev/null",
        "-oGlobalKnownHostsFile=/dev/null",
        "-oKnownHostsCommand=none",
        "-oUpdateHostKeys=no",
        "-oCheckHostIP=no",
        "-oFingerprintHash=sha256",
        "-oVisualHostKey=no",
        "-oConnectTimeout=5",
        "-oControlMaster=no",
        "-oControlPath=none",
        "-oForwardX11=no",
        "-oForwardAgent=no",
        "-oPermitLocalCommand=no",
        "-oClearAllForwardings=yes",
        "-oNoHostAuthenticationForLocalhost=no",
        "-oVerifyHostKeyDNS=no",
        "-oPubkeyAuthentication=no",
        "-oPasswordAuthentication=no",
        "-oKbdInteractiveAuthentication=no",
        "-oHostbasedAuthentication=no",
        "-oGSSAPIAuthentication=no",
    ]
    .map(str::to_owned)
    .into();
    argv.extend(target);
    argv.push("true".to_owned());
    Some(argv)
}

/// The host key from `ssh -v`'s "Server host key: <name> SHA256:<base64>".
pub fn host_key_from_ssh_log(log: &str) -> Option<HostKey> {
    let (name, fingerprint) = log.lines().find_map(|line| {
        let (_, rest) = line.split_once("Server host key: ")?;
        let mut parts = rest.split_whitespace();
        Some((parts.next()?, parts.next()?))
    })?;
    if !is_sha256_fingerprint(fingerprint) {
        return None;
    }
    let key_type = match name {
        "ssh-ed25519" => "ED25519",
        "sk-ssh-ed25519@openssh.com" => "ED25519-SK",
        "sk-ecdsa-sha2-nistp256@openssh.com" => "ECDSA-SK",
        "ssh-rsa" | "rsa-sha2-256" | "rsa-sha2-512" => "RSA",
        "ssh-dss" => "DSA",
        name if name.starts_with("ecdsa-sha2-") => "ECDSA",
        _ => return None,
    };
    Some(HostKey {
        key_type: key_type.to_owned(),
        fingerprint: fingerprint.to_owned(),
    })
}

pub const HOST_KEY_PROBE_TIMEOUT: Duration = Duration::from_secs(8);

/// Runs `argv` with empty input and gives what it wrote to `stderr`
/// (`stdout` too with `stdout`), or `None` when it cannot start or runs
/// longer than `limit` (then it is killed and reaped, no process is left).
/// Blocking: call it off the main thread.
pub fn command_output(argv: &[String], limit: Duration, stdout: bool) -> Option<String> {
    let (program, args) = argv.split_first()?;
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(if stdout {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stderr(if stdout {
            Stdio::null()
        } else {
            Stdio::piped()
        })
        .spawn()
        .inspect_err(|err| crate::say!("netz: {program} startet nicht: {err}"))
        .ok()?;
    let mut pipe: Box<dyn Read + Send> = if stdout {
        Box::new(child.stdout.take()?)
    } else {
        Box::new(child.stderr.take()?)
    };
    let reader = std::thread::spawn(move || {
        let mut text = Vec::new();
        let _ = pipe.read_to_end(&mut text);
        text
    });
    let begun = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if begun.elapsed() < limit => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                crate::say!("netz: {program} nach {} s beendet", limit.as_secs());
                let _ = reader.join();
                return None;
            }
        }
    }
    let text = reader.join().ok()?;
    Some(String::from_utf8_lossy(&text).into_owned())
}

/// Runs the host-key probe; `None` when `ssh` is missing, times out or
/// reports no plain host key (for example a host certificate).
pub fn probe_host_key(argv: &[String]) -> Option<HostKey> {
    let log = command_output(argv, HOST_KEY_PROBE_TIMEOUT, false)?;
    let key = host_key_from_ssh_log(&log);
    if key.is_none() {
        crate::say!("netz: kein Host-Schlüssel im ssh-Protokoll");
    }
    key
}

/// `ssh -G` for an SFTP URI: resolves the user configuration (aliases,
/// `HostKeyAlias`, `UserKnownHostsFile`) without connecting.
pub fn ssh_config_argv(uri: &str) -> Option<Vec<String>> {
    let mut argv = vec!["ssh".to_owned(), "-G".to_owned()];
    argv.extend(ssh_target_argv(uri)?);
    Some(argv)
}

/// Where `ssh` saves a host's key: its name in known_hosts and the user
/// known-hosts files, as `ssh -G` printed them.
#[derive(Debug, PartialEq, Eq)]
pub struct KnownHostsTarget {
    pub name: String,
    pub files: Vec<String>,
}

pub fn known_hosts_target(config: &str) -> Option<KnownHostsTarget> {
    let value = |key: &str| {
        config.lines().find_map(|line| {
            let (name, value) = line.split_once(' ')?;
            (name == key).then(|| value.trim())
        })
    };
    let name = match value("hostkeyalias").filter(|alias| !alias.is_empty() && *alias != "none") {
        Some(alias) => alias.to_owned(),
        None => {
            let host = value("hostname").filter(|host| !host.is_empty())?;
            match value("port") {
                Some(port) if port != "22" => format!("[{host}]:{port}"),
                _ => host.to_owned(),
            }
        }
    };
    let files = value("userknownhostsfile")?
        .split_whitespace()
        .filter(|file| *file != "none")
        .map(str::to_owned)
        .collect();
    Some(KnownHostsTarget { name, files })
}

pub fn known_hosts_lookup_argv(name: &str, file: &str) -> Vec<String> {
    ["ssh-keygen", "-l", "-F", name, "-f", file]
        .map(str::to_owned)
        .into()
}

/// The fingerprints in `ssh-keygen -l -F`'s listing, without marked
/// (`@revoked`, `@cert-authority`) entries.
pub fn known_hosts_fingerprints(listing: &str) -> Vec<String> {
    listing
        .lines()
        .filter(|line| !line.starts_with('#') && !line.starts_with('@'))
        .filter_map(|line| line.split_whitespace().last())
        .filter(|fingerprint| is_sha256_fingerprint(fingerprint))
        .map(str::to_owned)
        .collect()
}

#[derive(Debug, PartialEq, Eq)]
pub enum KnownHostsCheck {
    Matches,
    NotSaved,
    Differs { name: String, file: String },
}

/// Compares the fingerprint the question showed with what known_hosts holds
/// for the host after the user accepted it; `saved` lists each file's
/// fingerprints in ssh's order.
pub fn compare_known_hosts(
    shown: &str,
    name: &str,
    saved: &[(String, Vec<String>)],
) -> KnownHostsCheck {
    if saved
        .iter()
        .any(|(_, fingerprints)| fingerprints.iter().any(|saved| saved == shown))
    {
        return KnownHostsCheck::Matches;
    }
    saved
        .iter()
        .find(|(_, fingerprints)| !fingerprints.is_empty())
        .map_or(KnownHostsCheck::NotSaved, |(file, _)| {
            KnownHostsCheck::Differs {
                name: name.to_owned(),
                file: file.clone(),
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const FP: &str = "SHA256:abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQ";

    fn entry(native: &str, shown: &str, target: &str) -> NetzEintrag {
        NetzEintrag {
            native_name: native.into(),
            display_name: shown.into(),
            target: target.into(),
        }
    }

    #[test]
    fn a_device_found_by_avahi_and_wsdd_is_listed_once_with_the_dnssd_target() {
        let listing = vec![
            entry("wsdd-server-9ac3", "NAS-TEST", "smb://NAS-TEST.local/"),
            entry(
                "dnssd-server-nas-test._smb._tcp",
                "nas-test",
                "smb://192.0.2.12:445/",
            ),
            entry("wsdd-server-544b", "PC-TEST", "smb://PC-TEST.local/"),
            entry("smb-root", "Windows-Netzwerk", "smb:///"),
        ];
        let (entries, merged) = merge_network_entries(listing);
        let names: Vec<&str> = entries.iter().map(|e| e.display_name.as_str()).collect();
        assert_eq!(names, ["nas-test", "PC-TEST", "Windows-Netzwerk"]);
        assert_eq!(entries[0].target, "smb://192.0.2.12:445/");
        let sources = &merged["smb://192.0.2.12:445/"];
        assert_eq!(sources[0].service, DiscoveryService::DnsSd);
        assert_eq!(sources[1].display_name, "NAS-TEST");
        assert!(!merged.contains_key("smb://PC-TEST.local/"));
        assert_eq!(device_key(" NAS.Local. "), "nas");
        assert_ne!(device_key("nas2"), device_key("nas"));
    }

    #[test]
    fn the_host_key_comes_from_the_ssh_log_only_when_complete() {
        let log = format!("debug1: Connecting\ndebug1: Server host key: ssh-ed25519 {FP}\n");
        assert_eq!(
            host_key_from_ssh_log(&log),
            Some(HostKey {
                key_type: "ED25519".into(),
                fingerprint: FP.into()
            })
        );
        for line in [
            "debug1: Server host key: ssh-ed25519 MD5:12:34:56",
            "debug1: Server host key: ssh-ed25519 SHA256:short",
            "debug1: Server host key: ssh-ed25519",
        ] {
            assert_eq!(host_key_from_ssh_log(line), None, "{line}");
        }
        assert!(gvfs_question_lacks_fingerprint("… is “???”"));
        let german = format!(
            "Die Identität von »server« konnte nicht überprüft werden. Die vom entfernten Rechner übermittelte Identität lautet »{FP}«."
        );
        assert_eq!(
            gvfs_question_parts(&german),
            Some(("server".into(), FP.into()))
        );
        assert_eq!(shown_fingerprint(&german, None).as_deref(), Some(FP));
    }

    #[test]
    fn the_probe_targets_the_server_and_never_trusts_or_authenticates() {
        let argv = host_key_probe_argv("sftp://anna@server.test:2222/home").expect("argv");
        assert!(argv.ends_with(
            &["-p", "2222", "-l", "anna", "--", "server.test", "true"].map(str::to_owned)
        ));
        for option in [
            "-oBatchMode=yes",
            "-oStrictHostKeyChecking=yes",
            "-oUserKnownHostsFile=/dev/null",
            "-oPasswordAuthentication=no",
            "-oPubkeyAuthentication=no",
            "-oNoHostAuthenticationForLocalhost=no",
        ] {
            assert!(argv.iter().any(|a| a == option), "{option}");
        }
        assert_eq!(host_key_probe_argv("smb://server/share"), None);
        assert_eq!(host_key_probe_argv("sftp://-oProxyCommand=x/"), None);
    }

    #[test]
    fn the_probe_reads_the_key_from_the_process_and_tolerates_failures() {
        let script = format!(
            "echo 'debug1: Server host key: ssh-ed25519 {FP}' >&2; echo 'Host key verification failed.' >&2; exit 255"
        );
        let argv = vec!["sh".to_owned(), "-c".to_owned(), script];
        assert_eq!(
            probe_host_key(&argv).map(|k| k.fingerprint),
            Some(FP.to_owned())
        );
        let silent = vec!["sh".to_owned(), "-c".to_owned(), "exit 1".to_owned()];
        assert_eq!(probe_host_key(&silent), None);
        let missing = vec!["/nicht/da/ssh".to_owned()];
        assert_eq!(probe_host_key(&missing), None);
        let slow = vec!["sleep".to_owned(), "5".to_owned()];
        let begun = Instant::now();
        assert_eq!(
            command_output(&slow, Duration::from_millis(200), false),
            None
        );
        assert!(begun.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn the_accepted_key_is_compared_with_every_saved_fingerprint() {
        let config = "hostname server.test\nport 2222\nuserknownhostsfile ~/.ssh/known_hosts ~/.ssh/known_hosts2\n";
        assert_eq!(
            known_hosts_target(config),
            Some(KnownHostsTarget {
                name: "[server.test]:2222".into(),
                files: vec!["~/.ssh/known_hosts".into(), "~/.ssh/known_hosts2".into()],
            })
        );
        let listing =
            format!("# Host server found\n[server.test]:2222 ED25519 {FP}\n@revoked x {FP}\n");
        assert_eq!(known_hosts_fingerprints(&listing), [FP]);
        let other = "SHA256:ZZZdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQ";
        assert_eq!(
            compare_known_hosts(FP, "h", &[("f".into(), vec![FP.into()])]),
            KnownHostsCheck::Matches
        );
        assert_eq!(compare_known_hosts(FP, "h", &[]), KnownHostsCheck::NotSaved);
        assert_eq!(
            compare_known_hosts(FP, "h", &[("f".into(), vec![other.into()])]),
            KnownHostsCheck::Differs {
                name: "h".into(),
                file: "f".into()
            }
        );
        assert_eq!(
            known_hosts_lookup_argv("h", "f")[..3],
            ["ssh-keygen", "-l", "-F"]
        );
    }
}
