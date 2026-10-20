//! The active labwc key bindings of the running session: where its rc.xml
//! is, which bindings it holds (with labwc 0.9.8's defaults where rc.xml
//! asks for them), and what they do in words.
//!
//! labwc 0.9.8 semantics (src/config/rcxml.c, include/config/default-bindings.h):
//! - `<keyboard><default />` adds the default bindings at that place;
//!   without any binding at all the defaults apply as well.
//! - A later binding of the same keys replaces an earlier one.
//! - A binding without actions (`<action name="None" />`) is removed.

use std::path::{Path, PathBuf};

use crate::xml::{self, Element};

/// labwc 0.9.8's default bindings (`include/config/default-bindings.h`).
/// (keys, action, arguments) of one default binding.
type DefaultBinding = (
    &'static str,
    &'static str,
    &'static [(&'static str, &'static str)],
);

const DEFAULTS: &[DefaultBinding] = &[
    ("A-Tab", "NextWindow", &[]),
    ("A-S-Tab", "PreviousWindow", &[]),
    (
        "W-Return",
        "Execute",
        &[("command", "lab-sensible-terminal")],
    ),
    ("A-F4", "Close", &[]),
    ("W-a", "ToggleMaximize", &[]),
    (
        "W-Left",
        "SnapToEdge",
        &[("direction", "left"), ("combine", "yes")],
    ),
    (
        "W-Right",
        "SnapToEdge",
        &[("direction", "right"), ("combine", "yes")],
    ),
    (
        "W-Up",
        "SnapToEdge",
        &[("direction", "up"), ("combine", "yes")],
    ),
    (
        "W-Down",
        "SnapToEdge",
        &[("direction", "down"), ("combine", "yes")],
    ),
    (
        "A-Space",
        "ShowMenu",
        &[("menu", "client-menu"), ("atCursor", "no")],
    ),
    (
        "XF86_AudioLowerVolume",
        "Execute",
        &[("command", "amixer sset Master 5%-")],
    ),
    (
        "XF86_AudioRaiseVolume",
        "Execute",
        &[("command", "amixer sset Master 5%+")],
    ),
    (
        "XF86_AudioMute",
        "Execute",
        &[("command", "amixer sset Master toggle")],
    ),
    (
        "XF86_MonBrightnessUp",
        "Execute",
        &[("command", "brightnessctl set +10%")],
    ),
    (
        "XF86_MonBrightnessDown",
        "Execute",
        &[("command", "brightnessctl set 10%-")],
    ),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Modifier {
    Ctrl,
    Alt,
    Shift,
    Super,
    Hyper,
    Mod5,
}

impl Modifier {
    fn parse(token: &str) -> Option<Self> {
        Some(match token {
            "S" => Self::Shift,
            "C" => Self::Ctrl,
            "A" | "Mod1" => Self::Alt,
            "W" | "Mod4" => Self::Super,
            "H" | "Mod3" => Self::Hyper,
            "M" | "Mod5" => Self::Mod5,
            _ => return None,
        })
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Ctrl => "Strg",
            Self::Alt => "Alt",
            Self::Shift => "Umschalt",
            Self::Super => "Super",
            Self::Hyper => "Hyper",
            Self::Mod5 => "AltGr",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Action {
    pub name: String,
    pub args: Vec<(String, String)>,
}

impl Action {
    pub fn arg(&self, name: &str) -> Option<&str> {
        self.args
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Keybind {
    /// As written in rc.xml, e.g. `C-A-Left`.
    pub binding: String,
    /// Sorted, without duplicates.
    pub modifiers: Vec<Modifier>,
    pub key: String,
    pub actions: Vec<Action>,
    /// One of labwc's built-in defaults.
    pub default: bool,
}

impl Keybind {
    /// labwc's key string: modifiers and key joined by "-"; "W--" is
    /// Super with the minus key.
    pub fn parse(binding: &str, actions: Vec<Action>, default: bool) -> Option<Self> {
        let binding = binding.trim();
        let (mods, key) = if let Some(prefix) = binding.strip_suffix("--") {
            (prefix, "-")
        } else {
            match binding.rsplit_once('-') {
                Some((mods, key)) => (mods, key),
                None => ("", binding),
            }
        };
        if key.is_empty() {
            return None;
        }
        let mut modifiers = Vec::new();
        for token in mods.split('-').filter(|t| !t.is_empty()) {
            modifiers.push(Modifier::parse(token)?);
        }
        modifiers.sort();
        modifiers.dedup();
        Some(Self {
            binding: binding.to_owned(),
            modifiers,
            key: key.to_owned(),
            actions,
            default,
        })
    }

    /// labwc's `keybind_the_same`: same modifiers, same keysym (case and
    /// the "XF86_" spelling do not matter).
    fn same_keys(&self, other: &Self) -> bool {
        self.modifiers == other.modifiers && normal_key(&self.key) == normal_key(&other.key)
    }

    /// "Strg", "Alt", "←" … one entry per key to press together. A
    /// symbol that is always typed with Umschalt ("?" on German and US
    /// keyboards) is shown without it: W-S-question is "Super+?".
    pub fn keys(&self) -> Vec<String> {
        let shifted = SHIFTED_SYMBOLS.contains(&normal_key(&self.key).as_str());
        let mut keys: Vec<String> = self
            .modifiers
            .iter()
            .filter(|m| !(shifted && **m == Modifier::Shift))
            .map(|m| m.label().to_owned())
            .collect();
        keys.push(key_label(&self.key));
        keys
    }

    pub fn keys_text(&self) -> String {
        self.keys().join("+")
    }
}

/// Keysyms that need Umschalt on German and US layouts alike.
const SHIFTED_SYMBOLS: &[&str] = &[
    "question",
    "exclam",
    "quotedbl",
    "dollar",
    "percent",
    "ampersand",
    "parenleft",
    "parenright",
    "asterisk",
    "colon",
    "underscore",
    "greater",
];

fn normal_key(key: &str) -> String {
    let lower = key.to_ascii_lowercase();
    match lower.strip_prefix("xf86_") {
        Some(rest) => format!("xf86{rest}"),
        None => lower,
    }
}

/// The German name of a key (as printed on German keyboards).
pub fn key_label(key: &str) -> String {
    let named = match normal_key(key).as_str() {
        "left" => "←",
        "right" => "→",
        "up" => "↑",
        "down" => "↓",
        "return" => "Eingabe",
        "kp_enter" => "Eingabe (Ziffernblock)",
        "delete" => "Entf",
        "insert" => "Einfg",
        "print" => "Druck",
        "tab" => "Tab",
        "space" => "Leertaste",
        "escape" => "Esc",
        "backspace" => "Rücktaste",
        "home" => "Pos1",
        "end" => "Ende",
        "page_up" | "prior" => "Bild↑",
        "page_down" | "next" => "Bild↓",
        "menu" => "Menü",
        "question" => "?",
        "exclam" => "!",
        "quotedbl" => "\"",
        "dollar" => "$",
        "percent" => "%",
        "ampersand" => "&",
        "parenleft" => "(",
        "parenright" => ")",
        "asterisk" => "*",
        "colon" => ":",
        "underscore" => "_",
        "greater" => ">",
        "comma" => ",",
        "period" => ".",
        "minus" | "-" => "-",
        "plus" => "+",
        "super_l" | "super_r" => "Super",
        "xf86audioraisevolume" => "Lauter-Taste",
        "xf86audiolowervolume" => "Leiser-Taste",
        "xf86audiomute" => "Stumm-Taste",
        "xf86monbrightnessup" => "Heller-Taste",
        "xf86monbrightnessdown" => "Dunkler-Taste",
        _ => "",
    };
    if !named.is_empty() {
        return named.to_owned();
    }
    let mut chars = key.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => c.to_uppercase().collect(),
        _ => key.to_owned(),
    }
}

/// The section a binding is listed in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Group {
    Windows,
    Workspaces,
    Screenshot,
    Session,
    Other,
}

impl Group {
    pub const ALL: [Group; 5] = [
        Group::Windows,
        Group::Workspaces,
        Group::Screenshot,
        Group::Session,
        Group::Other,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Group::Windows => "Fenster",
            Group::Workspaces => "Arbeitsflächen",
            Group::Screenshot => "Bildschirmfoto",
            Group::Session => "Sitzung",
            Group::Other => "Weitere",
        }
    }
}

fn command_of(action: &Action) -> &str {
    action
        .arg("command")
        .or(action.arg("execute"))
        .unwrap_or("")
}

fn program_of(command: &str) -> &str {
    let first = command.split_whitespace().next().unwrap_or("");
    first.rsplit('/').next().unwrap_or(first)
}

fn action_group(action: &Action) -> Group {
    match action.name.to_ascii_lowercase().as_str() {
        "gotodesktop" | "sendtodesktop" | "adddesktop" | "removedesktop" => Group::Workspaces,
        "exit" | "reconfigure" => Group::Session,
        "execute" => {
            let command = command_of(action);
            let program = program_of(command);
            if program.contains("screenshot") || program == "grim" {
                Group::Screenshot
            } else if is_show_desktop(command) {
                Group::Windows
            } else if program.contains("querencia-sitzung")
                || program.contains("lock")
                || command.contains("labwc --exit")
            {
                Group::Session
            } else {
                Group::Other
            }
        }
        "showmenu" if action.arg("menu") != Some("client-menu") => Group::Other,
        "none" => Group::Other,
        _ => Group::Windows,
    }
}

impl Keybind {
    pub fn group(&self) -> Group {
        self.actions
            .first()
            .map(action_group)
            .unwrap_or(Group::Other)
    }

    pub fn description(&self) -> String {
        let parts: Vec<String> = self.actions.iter().map(describe).collect();
        let mut text = parts.join(", dann ");
        if let Some(first) = text.chars().next() {
            text = first.to_uppercase().chain(text.chars().skip(1)).collect();
        }
        text
    }
}

fn direction(word: &str) -> Option<(&'static str, &'static str)> {
    // (adjective for "Bildschirmhälfte"/"Arbeitsfläche", edge)
    Some(match word.to_ascii_lowercase().as_str() {
        "left" => ("linke", "linken"),
        "right" => ("rechte", "rechten"),
        "up" => ("obere", "oberen"),
        "down" => ("untere", "unteren"),
        _ => return None,
    })
}

fn desktop_target(to: Option<&str>) -> String {
    let to = to.unwrap_or("");
    match to.to_ascii_lowercase().as_str() {
        "left" => "die linke Arbeitsfläche".to_owned(),
        "right" => "die rechte Arbeitsfläche".to_owned(),
        "up" => "die obere Arbeitsfläche".to_owned(),
        "down" => "die untere Arbeitsfläche".to_owned(),
        "next" => "die nächste Arbeitsfläche".to_owned(),
        "previous" => "die vorige Arbeitsfläche".to_owned(),
        "last" => "die zuletzt benutzte Arbeitsfläche".to_owned(),
        "current" => "die aktuelle Arbeitsfläche".to_owned(),
        "" => "eine andere Arbeitsfläche".to_owned(),
        _ if to.chars().all(|c| c.is_ascii_digit()) => format!("Arbeitsfläche {to}"),
        _ => format!("Arbeitsfläche „{to}“"),
    }
}

fn describe(action: &Action) -> String {
    let dir = action.arg("direction").and_then(direction);
    match action.name.to_ascii_lowercase().as_str() {
        "nextwindow" => "zum nächsten Fenster wechseln".to_owned(),
        "previouswindow" => "zum vorigen Fenster wechseln".to_owned(),
        "close" => "Fenster schließen".to_owned(),
        "kill" => "Programm des Fensters beenden".to_owned(),
        "iconify" => "Fenster minimieren".to_owned(),
        "togglemaximize" => "Fenster maximieren oder wiederherstellen".to_owned(),
        "maximize" => "Fenster maximieren".to_owned(),
        "unmaximize" => "Maximierung aufheben".to_owned(),
        "togglefullscreen" => "Vollbild ein oder aus".to_owned(),
        "snaptoedge" => match dir {
            Some((half, _)) => format!("Fenster auf die {half} Bildschirmhälfte"),
            None => "Fenster an den Bildschirmrand".to_owned(),
        },
        "movetoedge" => match dir {
            Some((_, edge)) => format!("Fenster an den {edge} Rand schieben"),
            None => "Fenster an den Rand schieben".to_owned(),
        },
        "growtoedge" => match dir {
            Some((_, edge)) => format!("Fenster bis zum {edge} Rand vergrößern"),
            None => "Fenster bis zum Rand vergrößern".to_owned(),
        },
        "shrinktoedge" => "Fenster vom Rand her verkleinern".to_owned(),
        "showmenu" => match action.arg("menu") {
            Some("client-menu") => "Fenstermenü öffnen".to_owned(),
            Some("root-menu") => "Arbeitsflächenmenü von labwc öffnen".to_owned(),
            Some(menu) => format!("labwc-Menü „{menu}“ öffnen"),
            None => "Menü öffnen".to_owned(),
        },
        "move" => "Fenster verschieben".to_owned(),
        "resize" => "Fenstergröße ändern".to_owned(),
        "raise" => "Fenster nach vorne holen".to_owned(),
        "lower" => "Fenster nach hinten stellen".to_owned(),
        "focus" => "Fenster aktivieren".to_owned(),
        "togglealwaysontop" => "„Immer im Vordergrund“ ein oder aus".to_owned(),
        "togglealwaysonbottom" => "„Immer im Hintergrund“ ein oder aus".to_owned(),
        "toggledecorations" => "Fensterrahmen ein oder aus".to_owned(),
        "toggleshade" => "Fenster einrollen oder ausrollen".to_owned(),
        "toggleomnipresent" => "„Auf allen Arbeitsflächen“ ein oder aus".to_owned(),
        "toggleshowdesktop" | "showdesktop" => {
            "Schreibtisch anzeigen (alle Fenster weg)".to_owned()
        }
        "togglekeybinds" => "Tastenkürzel von labwc ein oder aus".to_owned(),
        "gotodesktop" => format!("auf {} wechseln", desktop_target(action.arg("to"))),
        "sendtodesktop" => {
            let target = desktop_target(action.arg("to"));
            if action
                .arg("follow")
                .is_some_and(|f| f.eq_ignore_ascii_case("no"))
            {
                format!("Fenster auf {target} schicken (ohne mitzugehen)")
            } else {
                format!("Fenster auf {target} mitnehmen")
            }
        }
        "adddesktop" => "Arbeitsfläche hinzufügen".to_owned(),
        "removedesktop" => "Arbeitsfläche entfernen".to_owned(),
        "exit" => "abmelden (labwc beenden)".to_owned(),
        "reconfigure" => "Einstellungen von labwc neu laden".to_owned(),
        "none" => "nichts (Kürzel abgeschaltet)".to_owned(),
        "execute" => describe_command(command_of(action)),
        _ => format!("labwc-Aktion „{}“", action.name),
    }
}

/// G2-AL: the panel's „Arbeitsfläche anzeigen“ over D-Bus (Super+D).
fn is_show_desktop(command: &str) -> bool {
    program_of(command) == "gdbus"
        && command
            .split_whitespace()
            .any(|w| w == "org.querencia.Panel.ArbeitsflaecheUmschalten")
}

fn describe_command(command: &str) -> String {
    let program = program_of(command);
    let has = |word: &str| command.split_whitespace().any(|w| w == word);
    if is_show_desktop(command) {
        "Arbeitsfläche anzeigen: alle Fenster minimieren oder wiederherstellen".to_owned()
    } else if program.contains("screenshot") {
        if has("--window") {
            "Bildschirmfoto eines Fensters".to_owned()
        } else if has("--area") || has("--bereich") {
            "Bildschirmfoto eines Bereichs".to_owned()
        } else {
            "Bildschirmfoto aufnehmen (Bildschirm, Fenster oder Bereich wählen)".to_owned()
        }
    } else if program == "querencia-sitzung" {
        if has("--sperren") {
            "Bildschirm sperren".to_owned()
        } else {
            "Sitzung beenden: Abmelden, Sperren, Neu starten, Herunterfahren".to_owned()
        }
    } else if program == "querencia-kuerzel" {
        "diese Übersicht der Tastenkürzel".to_owned()
    } else if program == "lab-sensible-terminal" {
        "Terminal öffnen".to_owned()
    } else if program == "amixer" || program == "wpctl" || program == "pactl" {
        if command.contains("toggle") {
            "Ton aus oder an".to_owned()
        } else if command.contains('+') {
            "lauter".to_owned()
        } else if command.contains('-') {
            "leiser".to_owned()
        } else {
            "Lautstärke".to_owned()
        }
    } else if program == "brightnessctl" {
        if command.contains('+') {
            "Bildschirm heller".to_owned()
        } else {
            "Bildschirm dunkler".to_owned()
        }
    } else if program.is_empty() {
        "Befehl ohne Programm".to_owned()
    } else {
        format!("startet „{program}“")
    }
}

fn read_action(element: &Element) -> Option<Action> {
    let name = element.attribute("name")?.to_owned();
    let mut args: Vec<(String, String)> = element
        .attributes
        .iter()
        .filter(|(key, _)| !key.eq_ignore_ascii_case("name"))
        .cloned()
        .collect();
    // <action name="GoToDesktop"><to>left</to></action>
    for child in &element.children {
        if child.children.is_empty() {
            args.push((child.name.clone(), child.text.clone()));
        }
    }
    Some(Action { name, args })
}

fn defaults() -> Vec<Keybind> {
    DEFAULTS
        .iter()
        .filter_map(|(binding, action, args)| {
            let action = Action {
                name: (*action).to_owned(),
                args: args
                    .iter()
                    .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                    .collect(),
            };
            Keybind::parse(binding, vec![action], true)
        })
        .collect()
}

/// The active bindings of an rc.xml (`None`: no rc.xml, labwc's defaults).
pub fn keybinds(rc: Option<&Element>) -> Vec<Keybind> {
    let mut list = Vec::new();
    if let Some(rc) = rc {
        for keyboard in rc.children_named("keyboard") {
            for child in &keyboard.children {
                if child.name.eq_ignore_ascii_case("default") {
                    list.extend(defaults());
                } else if child.name.eq_ignore_ascii_case("keybind")
                    && let Some(key) = child.attribute("key")
                {
                    let actions: Vec<Action> = child
                        .children_named("action")
                        .filter_map(read_action)
                        .collect();
                    if let Some(bind) = Keybind::parse(key, actions, false) {
                        list.push(bind);
                    }
                }
            }
        }
    }
    if list.is_empty() {
        list = defaults();
    }
    // a later binding of the same keys replaces an earlier one
    let mut active: Vec<Keybind> = Vec::new();
    for (i, bind) in list.iter().enumerate() {
        if !list[i + 1..].iter().any(|later| later.same_keys(bind)) {
            active.push(bind.clone());
        }
    }
    active.retain(|bind| {
        bind.actions
            .iter()
            .any(|action| !action.name.eq_ignore_ascii_case("none"))
    });
    active
}

/// Where the rc.xml came from (shown under the list).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    /// `--rc <datei>`
    Argument(PathBuf),
    /// The running labwc (LABWC_PID) with its -C/-c option.
    Session { pid: u32, rc: PathBuf },
    /// The XDG search path (labwc without -C/-c, or no LABWC_PID).
    Search { rc: PathBuf, pid: Option<u32> },
    /// No rc.xml found: labwc's defaults.
    None { pid: Option<u32> },
}

impl Source {
    pub fn path(&self) -> Option<&Path> {
        match self {
            Source::Argument(rc) | Source::Session { rc, .. } | Source::Search { rc, .. } => {
                Some(rc)
            }
            Source::None { .. } => None,
        }
    }

    pub fn describe(&self) -> String {
        match self {
            Source::Argument(rc) => format!("Quelle: {}", rc.display()),
            Source::Session { pid, rc } => {
                format!("Quelle: {} (laufende Sitzung, labwc {pid})", rc.display())
            }
            Source::Search { rc, pid } => match pid {
                Some(pid) => format!("Quelle: {} (labwc {pid}, Standardpfad)", rc.display()),
                None => format!(
                    "Quelle: {} (Standardpfad; keine laufende labwc-Sitzung erkannt)",
                    rc.display()
                ),
            },
            Source::None { .. } => "Keine rc.xml gefunden: labwc-Standardbelegung".to_owned(),
        }
    }
}

/// The options of a labwc command line that name its configuration:
/// (-C/--config-dir, -c/--config).
fn config_options(args: &[String]) -> (Option<PathBuf>, Option<PathBuf>) {
    let (mut dir, mut file) = (None, None);
    let mut it = args.iter().skip(1);
    while let Some(arg) = it.next() {
        if let Some(v) = arg.strip_prefix("--config-dir=") {
            dir = Some(PathBuf::from(v));
        } else if let Some(v) = arg.strip_prefix("--config=") {
            file = Some(PathBuf::from(v));
        } else if arg == "-C" || arg == "--config-dir" {
            dir = it.next().map(PathBuf::from);
        } else if arg == "-c" || arg == "--config" {
            file = it.next().map(PathBuf::from);
        } else if let Some(v) = arg.strip_prefix("-C").filter(|v| !v.is_empty()) {
            dir = Some(PathBuf::from(v));
        } else if let Some(v) = arg.strip_prefix("-c").filter(|v| !v.is_empty()) {
            file = Some(PathBuf::from(v));
        }
    }
    (dir, file)
}

/// Everything `find_rc` reads from the system (a test passes its own).
pub struct Environment {
    pub labwc_pid: Option<String>,
    pub proc_root: PathBuf,
    pub xdg_config_home: Option<PathBuf>,
    pub home: PathBuf,
    pub xdg_config_dirs: Option<String>,
}

impl Environment {
    pub fn current() -> Self {
        Self {
            labwc_pid: std::env::var("LABWC_PID").ok(),
            proc_root: PathBuf::from("/proc"),
            xdg_config_home: std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .filter(|p| p.is_absolute()),
            home: std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("/")),
            xdg_config_dirs: std::env::var("XDG_CONFIG_DIRS").ok(),
        }
    }
}

/// The rc.xml the running labwc read: its -c file, or rc.xml in its -C
/// directory; without those labwc searches $XDG_CONFIG_HOME/labwc, then
/// $XDG_CONFIG_DIRS/labwc (first file found).
pub fn find_rc(env: &Environment) -> Source {
    let pid = env
        .labwc_pid
        .as_deref()
        .and_then(|p| p.trim().parse::<u32>().ok());
    if let Some(pid) = pid {
        let cmdline = std::fs::read(env.proc_root.join(pid.to_string()).join("cmdline"));
        if let Ok(cmdline) = cmdline {
            let args: Vec<String> = cmdline
                .split(|b| *b == 0)
                .filter(|a| !a.is_empty())
                .map(|a| String::from_utf8_lossy(a).into_owned())
                .collect();
            let is_labwc = args
                .first()
                .is_some_and(|a| a.rsplit('/').next() == Some("labwc"));
            if is_labwc {
                let (dir, file) = config_options(&args);
                let rc = file.or_else(|| dir.map(|d| d.join("rc.xml")));
                if let Some(rc) = rc {
                    return if rc.is_file() {
                        Source::Session { pid, rc }
                    } else {
                        Source::None { pid: Some(pid) }
                    };
                }
            }
        }
    }
    let mut dirs = vec![
        env.xdg_config_home
            .clone()
            .unwrap_or_else(|| env.home.join(".config")),
    ];
    let system = env
        .xdg_config_dirs
        .clone()
        .filter(|d| !d.is_empty())
        .unwrap_or_else(|| "/etc/xdg".to_owned());
    dirs.extend(
        system
            .split(':')
            .filter(|d| !d.is_empty())
            .map(PathBuf::from),
    );
    for dir in dirs {
        let rc = dir.join("labwc").join("rc.xml");
        if rc.is_file() {
            return Source::Search { rc, pid };
        }
    }
    Source::None { pid }
}

/// Reads and parses an rc.xml.
pub fn read_rc(path: &Path) -> Result<Element, String> {
    let text = std::fs::read_to_string(path).map_err(|err| format!("{}: {err}", path.display()))?;
    xml::parse(&text).map_err(|err| format!("{}: {err}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rc(text: &str) -> Vec<Keybind> {
        keybinds(Some(&xml::parse(text).unwrap()))
    }

    fn find<'a>(list: &'a [Keybind], keys: &str) -> Option<&'a Keybind> {
        list.iter().find(|b| b.keys_text() == keys)
    }

    #[test]
    fn key_strings() {
        let b = Keybind::parse("C-A-S-Left", vec![], false).unwrap();
        assert_eq!(b.keys_text(), "Strg+Alt+Umschalt+←");
        assert_eq!(
            Keybind::parse("W--", vec![], false).unwrap().keys_text(),
            "Super+-"
        );
        assert_eq!(
            Keybind::parse("W-S-question", vec![], false)
                .unwrap()
                .keys_text(),
            "Super+?"
        );
        assert_eq!(
            Keybind::parse("Print", vec![], false).unwrap().keys_text(),
            "Druck"
        );
        assert_eq!(
            Keybind::parse("C-A-l", vec![], false).unwrap().keys_text(),
            "Strg+Alt+L"
        );
        assert_eq!(
            Keybind::parse("Mod4-Return", vec![], false)
                .unwrap()
                .keys_text(),
            "Super+Eingabe"
        );
        assert!(Keybind::parse("Q-x", vec![], false).is_none());
        assert!(Keybind::parse("", vec![], false).is_none());
    }

    #[test]
    fn without_rc_the_defaults() {
        let list = keybinds(None);
        assert_eq!(list.len(), DEFAULTS.len());
        assert!(list.iter().all(|b| b.default));
        assert_eq!(
            find(&list, "Alt+Tab").unwrap().description(),
            "Zum nächsten Fenster wechseln"
        );
        assert_eq!(find(&list, "Alt+F4").unwrap().group(), Group::Windows);
        assert_eq!(find(&list, "Lauter-Taste").unwrap().group(), Group::Other);
    }

    #[test]
    fn own_bindings_without_default_replace_them() {
        let list = rc(
            "<labwc_config><keyboard><keybind key=\"A-Tab\"><action name=\"NextWindow\"/></keybind></keyboard></labwc_config>",
        );
        assert_eq!(list.len(), 1);
        assert!(!list[0].default);
    }

    #[test]
    fn later_binding_replaces_and_none_removes() {
        let list = rc(r#"<labwc_config><keyboard><default/>
            <keybind key="A-F4"><action name="Iconify"/></keybind>
            <keybind key="W-a"><action name="None"/></keybind>
            <keybind key="XF86AudioMute"><action name="Execute" command="wpctl set-mute @DEFAULT_SINK@ toggle"/></keybind>
            </keyboard></labwc_config>"#);
        assert_eq!(
            find(&list, "Alt+F4").unwrap().description(),
            "Fenster minimieren"
        );
        assert!(find(&list, "Super+A").is_none());
        let mute: Vec<_> = list
            .iter()
            .filter(|b| b.keys_text() == "Stumm-Taste")
            .collect();
        assert_eq!(mute.len(), 1);
        assert!(!mute[0].default);
        assert_eq!(mute[0].description(), "Ton aus oder an");
        assert_eq!(list.len(), DEFAULTS.len() - 1);
    }

    #[test]
    fn querencia_runtime_rc_xml() {
        let text = include_str!("../../../session/querencia/labwc/rc.xml");
        let list = rc(text);
        let print = find(&list, "Druck").unwrap();
        assert_eq!(print.group(), Group::Screenshot);
        assert!(print.description().starts_with("Bildschirmfoto aufnehmen"));
        assert_eq!(
            find(&list, "Alt+Druck").unwrap().description(),
            "Bildschirmfoto eines Fensters"
        );
        let left = find(&list, "Strg+Alt+←").unwrap();
        assert_eq!(
            (left.group(), left.description().as_str()),
            (Group::Workspaces, "Auf die linke Arbeitsfläche wechseln")
        );
        assert_eq!(
            find(&list, "Strg+Alt+Umschalt+→").unwrap().description(),
            "Fenster auf die rechte Arbeitsfläche mitnehmen"
        );
        assert_eq!(find(&list, "Strg+Alt+L").unwrap().group(), Group::Session);
        assert_eq!(
            find(&list, "Strg+Alt+L").unwrap().description(),
            "Bildschirm sperren"
        );
        assert_eq!(
            find(&list, "Strg+Alt+Entf").unwrap().group(),
            Group::Session
        );
        assert_eq!(
            find(&list, "Super+←").unwrap().description(),
            "Fenster auf die linke Bildschirmhälfte"
        );
        assert_eq!(
            find(&list, "Alt+Leertaste").unwrap().description(),
            "Fenstermenü öffnen"
        );
        // G2-AL: Super+D through the panel
        let show = find(&list, "Super+D").unwrap();
        assert_eq!(
            (show.group(), show.description().as_str()),
            (
                Group::Windows,
                "Arbeitsfläche anzeigen: alle Fenster minimieren oder wiederherstellen"
            )
        );
        // another gdbus call is just a program
        assert_eq!(
            describe_command("gdbus call --session --dest x --method y.Z"),
            "startet „gdbus“"
        );
        for group in Group::ALL {
            if group != Group::Other {
                assert!(list.iter().any(|b| b.group() == group), "{group:?}");
            }
        }
    }

    #[test]
    fn help_key_of_the_runtime() {
        let list = rc(include_str!("../../../session/querencia/labwc/rc.xml"));
        let help = find(&list, "Super+?").unwrap();
        assert_eq!(help.binding, "W-S-question");
        assert_eq!(help.description(), "Diese Übersicht der Tastenkürzel");
    }

    #[test]
    fn child_element_arguments() {
        let list = rc(
            r#"<labwc_config><keyboard><keybind key="W-1"><action name="GoToDesktop"><to>1</to></action></keybind>
            <keybind key="W-S-1"><action name="SendToDesktop"><to>1</to><follow>no</follow></action></keybind></keyboard></labwc_config>"#,
        );
        assert_eq!(
            find(&list, "Super+1").unwrap().description(),
            "Auf Arbeitsfläche 1 wechseln"
        );
        assert_eq!(
            find(&list, "Umschalt+Super+1").unwrap().description(),
            "Fenster auf Arbeitsfläche 1 schicken (ohne mitzugehen)"
        );
    }

    fn fake_proc(dir: &Path, pid: u32, args: &[&str]) {
        let p = dir.join("proc").join(pid.to_string());
        std::fs::create_dir_all(&p).unwrap();
        let mut cmd = Vec::new();
        for a in args {
            cmd.extend_from_slice(a.as_bytes());
            cmd.push(0);
        }
        std::fs::write(p.join("cmdline"), cmd).unwrap();
    }

    #[test]
    fn rc_of_the_running_session() {
        let dir = std::env::temp_dir().join(format!("qkuerzel-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let cfg = dir.join("sitzung");
        std::fs::create_dir_all(&cfg).unwrap();
        std::fs::write(cfg.join("rc.xml"), "<labwc_config/>").unwrap();
        fake_proc(
            &dir,
            42,
            &["/opt/labwc/bin/labwc", "-C", cfg.to_str().unwrap()],
        );
        fake_proc(&dir, 43, &["/usr/bin/bash", "-C", cfg.to_str().unwrap()]);
        let home = dir.join("home");
        std::fs::create_dir_all(home.join(".config/labwc")).unwrap();
        std::fs::write(home.join(".config/labwc/rc.xml"), "<labwc_config/>").unwrap();
        let env = |pid: Option<&str>| Environment {
            labwc_pid: pid.map(str::to_owned),
            proc_root: dir.join("proc"),
            xdg_config_home: None,
            home: home.clone(),
            xdg_config_dirs: Some(dir.join("etc").to_string_lossy().into_owned()),
        };
        assert_eq!(
            find_rc(&env(Some("42"))),
            Source::Session {
                pid: 42,
                rc: cfg.join("rc.xml")
            }
        );
        // not a labwc process: the search path
        assert_eq!(
            find_rc(&env(Some("43"))),
            Source::Search {
                rc: home.join(".config/labwc/rc.xml"),
                pid: Some(43)
            }
        );
        assert_eq!(
            find_rc(&env(None)),
            Source::Search {
                rc: home.join(".config/labwc/rc.xml"),
                pid: None
            }
        );
        fake_proc(&dir, 44, &["labwc", "--config-dir=/gibt/es/nicht"]);
        assert_eq!(find_rc(&env(Some("44"))), Source::None { pid: Some(44) });
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
