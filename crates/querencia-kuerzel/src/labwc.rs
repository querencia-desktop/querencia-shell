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

use crate::i18n::{gettext, gettext_f, pgettext};
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

    /// The name printed on the key, in the language of the session.
    pub fn label(self) -> String {
        match self {
            Self::Ctrl => pgettext("key", "Ctrl"),
            Self::Alt => "Alt".to_owned(),
            Self::Shift => pgettext("key", "Shift"),
            Self::Super => "Super".to_owned(),
            Self::Hyper => "Hyper".to_owned(),
            Self::Mod5 => "AltGr".to_owned(),
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

    /// "Ctrl", "Alt", "←" … one entry per key to press together. A
    /// symbol that is always typed with Shift ("?" on German and US
    /// keyboards) is shown without it: W-S-question is "Super+?".
    pub fn keys(&self) -> Vec<String> {
        let shifted = SHIFTED_SYMBOLS.contains(&normal_key(&self.key).as_str());
        let mut keys: Vec<String> = self
            .modifiers
            .iter()
            .filter(|m| !(shifted && **m == Modifier::Shift))
            .map(|m| m.label())
            .collect();
        keys.push(key_label(&self.key));
        keys
    }

    pub fn keys_text(&self) -> String {
        self.keys().join("+")
    }
}

/// Keysyms that need Shift on German and US layouts alike.
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

/// The name of a key as printed on keyboards of the session's language
/// (German keyboards: Eingabe, Entf, Pos1, Bild↑ …; context "key").
pub fn key_label(key: &str) -> String {
    let translated = match normal_key(key).as_str() {
        "return" => pgettext("key", "Enter"),
        "kp_enter" => pgettext("key", "Enter (keypad)"),
        "delete" => pgettext("key", "Delete"),
        "insert" => pgettext("key", "Insert"),
        "print" => pgettext("key", "Print"),
        "space" => pgettext("key", "Space"),
        "escape" => pgettext("key", "Esc"),
        "backspace" => pgettext("key", "Backspace"),
        "home" => pgettext("key", "Home"),
        "end" => pgettext("key", "End"),
        "page_up" | "prior" => pgettext("key", "Page Up"),
        "page_down" | "next" => pgettext("key", "Page Down"),
        "menu" => pgettext("key", "Menu"),
        "xf86audioraisevolume" => pgettext("key", "Volume Up key"),
        "xf86audiolowervolume" => pgettext("key", "Volume Down key"),
        "xf86audiomute" => pgettext("key", "Mute key"),
        "xf86monbrightnessup" => pgettext("key", "Brightness Up key"),
        "xf86monbrightnessdown" => pgettext("key", "Brightness Down key"),
        _ => String::new(),
    };
    if !translated.is_empty() {
        return translated;
    }
    let named = match normal_key(key).as_str() {
        "left" => "←",
        "right" => "→",
        "up" => "↑",
        "down" => "↓",
        "tab" => "Tab",
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

    pub fn title(self) -> String {
        match self {
            Group::Windows => gettext("Windows"),
            Group::Workspaces => gettext("Workspaces"),
            Group::Screenshot => gettext("Screenshot"),
            Group::Session => gettext("Session"),
            Group::Other => gettext("Other"),
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
        // TRANSLATORS: joins the actions of one shortcut ("close the
        // window, then switch to the next window")
        let mut text = parts.join(&gettext(", then "));
        if let Some(first) = text.chars().next() {
            text = first.to_uppercase().chain(text.chars().skip(1)).collect();
        }
        text
    }
}

/// What GoToDesktop and SendToDesktop do, as whole sentences (the target
/// changes article and preposition in German, Italian, Spanish and French).
#[derive(Clone, Copy)]
enum Move {
    Go,
    Take,
    Send,
}

fn workspace_move(how: Move, to: Option<&str>) -> String {
    let to = to.unwrap_or("");
    let lower = to.to_ascii_lowercase();
    match how {
        Move::Go => match lower.as_str() {
            "left" => gettext("switch to the left workspace"),
            "right" => gettext("switch to the right workspace"),
            "up" => gettext("switch to the workspace above"),
            "down" => gettext("switch to the workspace below"),
            "next" => gettext("switch to the next workspace"),
            "previous" => gettext("switch to the previous workspace"),
            "last" => gettext("switch to the last used workspace"),
            "current" => gettext("switch to the current workspace"),
            "" => gettext("switch to another workspace"),
            _ if to.chars().all(|c| c.is_ascii_digit()) => {
                gettext_f("switch to workspace {number}", &[("number", to)])
            }
            _ => gettext_f("switch to workspace “{name}”", &[("name", to)]),
        },
        Move::Take => match lower.as_str() {
            "left" => gettext("take the window to the left workspace"),
            "right" => gettext("take the window to the right workspace"),
            "up" => gettext("take the window to the workspace above"),
            "down" => gettext("take the window to the workspace below"),
            "next" => gettext("take the window to the next workspace"),
            "previous" => gettext("take the window to the previous workspace"),
            "last" => gettext("take the window to the last used workspace"),
            "current" => gettext("take the window to the current workspace"),
            "" => gettext("take the window to another workspace"),
            _ if to.chars().all(|c| c.is_ascii_digit()) => {
                gettext_f("take the window to workspace {number}", &[("number", to)])
            }
            _ => gettext_f("take the window to workspace “{name}”", &[("name", to)]),
        },
        Move::Send => match lower.as_str() {
            "left" => gettext("send the window to the left workspace (without following it)"),
            "right" => gettext("send the window to the right workspace (without following it)"),
            "up" => gettext("send the window to the workspace above (without following it)"),
            "down" => gettext("send the window to the workspace below (without following it)"),
            "next" => gettext("send the window to the next workspace (without following it)"),
            "previous" => {
                gettext("send the window to the previous workspace (without following it)")
            }
            "last" => gettext("send the window to the last used workspace (without following it)"),
            "current" => gettext("send the window to the current workspace (without following it)"),
            "" => gettext("send the window to another workspace (without following it)"),
            _ if to.chars().all(|c| c.is_ascii_digit()) => gettext_f(
                "send the window to workspace {number} (without following it)",
                &[("number", to)],
            ),
            _ => gettext_f(
                "send the window to workspace “{name}” (without following it)",
                &[("name", to)],
            ),
        },
    }
}

fn describe(action: &Action) -> String {
    let dir = action
        .arg("direction")
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    match action.name.to_ascii_lowercase().as_str() {
        "nextwindow" => gettext("switch to the next window"),
        "previouswindow" => gettext("switch to the previous window"),
        "close" => gettext("close the window"),
        "kill" => gettext("quit the window's program"),
        "iconify" => gettext("minimize the window"),
        "togglemaximize" => gettext("maximize or restore the window"),
        "maximize" => gettext("maximize the window"),
        "unmaximize" => gettext("unmaximize the window"),
        "togglefullscreen" => gettext("fullscreen on or off"),
        "snaptoedge" => match dir.as_str() {
            "left" => gettext("snap the window to the left half of the screen"),
            "right" => gettext("snap the window to the right half of the screen"),
            "up" => gettext("snap the window to the top half of the screen"),
            "down" => gettext("snap the window to the bottom half of the screen"),
            _ => gettext("snap the window to the screen edge"),
        },
        "movetoedge" => match dir.as_str() {
            "left" => gettext("move the window to the left edge"),
            "right" => gettext("move the window to the right edge"),
            "up" => gettext("move the window to the top edge"),
            "down" => gettext("move the window to the bottom edge"),
            _ => gettext("move the window to the edge"),
        },
        "growtoedge" => match dir.as_str() {
            "left" => gettext("grow the window to the left edge"),
            "right" => gettext("grow the window to the right edge"),
            "up" => gettext("grow the window to the top edge"),
            "down" => gettext("grow the window to the bottom edge"),
            _ => gettext("grow the window to the edge"),
        },
        "shrinktoedge" => gettext("shrink the window from the edge"),
        "showmenu" => match action.arg("menu") {
            Some("client-menu") => gettext("open the window menu"),
            Some("root-menu") => gettext("open labwc's desktop menu"),
            Some(menu) => gettext_f("open the labwc menu “{menu}”", &[("menu", menu)]),
            None => gettext("open a menu"),
        },
        "move" => gettext("move the window"),
        "resize" => gettext("resize the window"),
        "raise" => gettext("raise the window"),
        "lower" => gettext("lower the window"),
        "focus" => gettext("activate the window"),
        "togglealwaysontop" => gettext("“Always on top” on or off"),
        "togglealwaysonbottom" => gettext("“Always on bottom” on or off"),
        "toggledecorations" => gettext("window frame on or off"),
        "toggleshade" => gettext("shade or unshade the window"),
        "toggleomnipresent" => gettext("“On all workspaces” on or off"),
        "toggleshowdesktop" | "showdesktop" => gettext("show the desktop (all windows away)"),
        "togglekeybinds" => gettext("labwc's keyboard shortcuts on or off"),
        "gotodesktop" => workspace_move(Move::Go, action.arg("to")),
        "sendtodesktop" => {
            let stay = action
                .arg("follow")
                .is_some_and(|f| f.eq_ignore_ascii_case("no"));
            workspace_move(if stay { Move::Send } else { Move::Take }, action.arg("to"))
        }
        "adddesktop" => gettext("add a workspace"),
        "removedesktop" => gettext("remove a workspace"),
        "exit" => gettext("log out (quit labwc)"),
        "reconfigure" => gettext("reload labwc's settings"),
        "none" => gettext("nothing (shortcut turned off)"),
        "execute" => describe_command(command_of(action)),
        _ => gettext_f("labwc action “{action}”", &[("action", &action.name)]),
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
        gettext("Show desktop: minimize or restore all windows")
    } else if program.contains("screenshot") {
        if has("--window") {
            gettext("Screenshot of a window")
        } else if has("--area") || has("--bereich") {
            gettext("Screenshot of an area")
        } else {
            gettext("Take a screenshot (choose screen, window or area)")
        }
    } else if program == "querencia-sitzung" {
        if has("--sperren") {
            gettext("lock the screen")
        } else {
            gettext("end the session: log out, lock, restart, shut down")
        }
    } else if program == "querencia-kuerzel" {
        gettext("this overview of the keyboard shortcuts")
    } else if program == "lab-sensible-terminal" {
        gettext("open a terminal")
    } else if program == "amixer" || program == "wpctl" || program == "pactl" {
        if command.contains("toggle") {
            gettext("sound off or on")
        } else if command.contains('+') {
            gettext("louder")
        } else if command.contains('-') {
            gettext("quieter")
        } else {
            gettext("volume")
        }
    } else if program == "brightnessctl" {
        if command.contains('+') {
            gettext("screen brighter")
        } else {
            gettext("screen darker")
        }
    } else if program.is_empty() {
        gettext("command without a program")
    } else {
        gettext_f("starts “{program}”", &[("program", program)])
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
            Source::Argument(rc) => {
                gettext_f("Source: {path}", &[("path", &rc.display().to_string())])
            }
            Source::Session { pid, rc } => gettext_f(
                "Source: {path} (running session, labwc {pid})",
                &[
                    ("path", &rc.display().to_string()),
                    ("pid", &pid.to_string()),
                ],
            ),
            Source::Search { rc, pid } => match pid {
                Some(pid) => gettext_f(
                    "Source: {path} (labwc {pid}, default path)",
                    &[
                        ("path", &rc.display().to_string()),
                        ("pid", &pid.to_string()),
                    ],
                ),
                None => gettext_f(
                    "Source: {path} (default path; no running labwc session found)",
                    &[("path", &rc.display().to_string())],
                ),
            },
            Source::None { .. } => gettext("No rc.xml found: labwc's default bindings"),
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
        assert_eq!(b.keys_text(), "Ctrl+Alt+Shift+←");
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
            "Print"
        );
        assert_eq!(
            Keybind::parse("C-A-l", vec![], false).unwrap().keys_text(),
            "Ctrl+Alt+L"
        );
        assert_eq!(
            Keybind::parse("Mod4-Return", vec![], false)
                .unwrap()
                .keys_text(),
            "Super+Enter"
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
            "Switch to the next window"
        );
        assert_eq!(find(&list, "Alt+F4").unwrap().group(), Group::Windows);
        assert_eq!(find(&list, "Volume Up key").unwrap().group(), Group::Other);
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
            "Minimize the window"
        );
        assert!(find(&list, "Super+A").is_none());
        let mute: Vec<_> = list
            .iter()
            .filter(|b| b.keys_text() == "Mute key")
            .collect();
        assert_eq!(mute.len(), 1);
        assert!(!mute[0].default);
        assert_eq!(mute[0].description(), "Sound off or on");
        assert_eq!(list.len(), DEFAULTS.len() - 1);
    }

    #[test]
    fn querencia_runtime_rc_xml() {
        let text = include_str!("../../../session/querencia/labwc/rc.xml");
        let list = rc(text);
        let print = find(&list, "Print").unwrap();
        assert_eq!(print.group(), Group::Screenshot);
        assert!(print.description().starts_with("Take a screenshot"));
        assert_eq!(
            find(&list, "Alt+Print").unwrap().description(),
            "Screenshot of a window"
        );
        let left = find(&list, "Ctrl+Alt+←").unwrap();
        assert_eq!(
            (left.group(), left.description().as_str()),
            (Group::Workspaces, "Switch to the left workspace")
        );
        assert_eq!(
            find(&list, "Ctrl+Alt+Shift+→").unwrap().description(),
            "Take the window to the right workspace"
        );
        assert_eq!(find(&list, "Ctrl+Alt+L").unwrap().group(), Group::Session);
        assert_eq!(
            find(&list, "Ctrl+Alt+L").unwrap().description(),
            "Lock the screen"
        );
        assert_eq!(
            find(&list, "Ctrl+Alt+Delete").unwrap().group(),
            Group::Session
        );
        assert_eq!(
            find(&list, "Super+←").unwrap().description(),
            "Snap the window to the left half of the screen"
        );
        assert_eq!(
            find(&list, "Alt+Space").unwrap().description(),
            "Open the window menu"
        );
        // G2-AL: Super+D through the panel
        let show = find(&list, "Super+D").unwrap();
        assert_eq!(
            (show.group(), show.description().as_str()),
            (
                Group::Windows,
                "Show desktop: minimize or restore all windows"
            )
        );
        // QD-065 (G2-AK): fullscreen on Super+F and Alt+F11, not on F11
        for keys in ["Super+F", "Alt+F11"] {
            let full = find(&list, keys).unwrap();
            assert_eq!(
                (full.group(), full.description().as_str()),
                (Group::Windows, "Fullscreen on or off"),
                "{keys}"
            );
        }
        assert!(find(&list, "F11").is_none());
        // another gdbus call is just a program
        assert_eq!(
            describe_command("gdbus call --session --dest x --method y.Z"),
            "starts “gdbus”"
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
        assert_eq!(
            help.description(),
            "This overview of the keyboard shortcuts"
        );
    }

    #[test]
    fn child_element_arguments() {
        let list = rc(
            r#"<labwc_config><keyboard><keybind key="W-1"><action name="GoToDesktop"><to>1</to></action></keybind>
            <keybind key="W-S-1"><action name="SendToDesktop"><to>1</to><follow>no</follow></action></keybind></keyboard></labwc_config>"#,
        );
        assert_eq!(
            find(&list, "Super+1").unwrap().description(),
            "Switch to workspace 1"
        );
        assert_eq!(
            find(&list, "Shift+Super+1").unwrap().description(),
            "Send the window to workspace 1 (without following it)"
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
