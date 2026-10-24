//! The settings portal at start (QD-062, G2-AI; panel and desktop first).
//!
//! GTK 4.22 reads theme, font and cursor only from the settings portal
//! (`org.freedesktop.portal.Settings`). While opening the display it asks
//! the portal synchronously with D-Bus's default timeout of 25 s, twice
//! (version, then all values). A portal left over from an earlier session
//! that no longer answers therefore kept panel and desktop from appearing
//! for about a minute after a new login (20.10.2026, black screen).
//! Without a portal GTK does not fall back to GSettings either: it keeps its
//! built-in defaults ("Sans 10", theme "Default") and follows no change.
//!
//! So before GTK starts, [`prepare`] asks the portal itself with a short
//! timeout. If it answers with a value, GTK uses it as usual. If not, GTK is
//! started with `GDK_DEBUG=no-portals` (no waiting at all), and
//! [`follow_gsettings`] sets the same values the GTK portal backend would
//! deliver, read directly from GSettings, and follows their changes. The
//! result looks the same; GTK has no way to switch to the portal later, so
//! the values keep coming from GSettings for the life of the process (the
//! portal backend reads the very same keys). The icon theme is left to
//! `icon_theme.rs`.

use std::cell::RefCell;
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use gtk4::prelude::*;
use gtk4::{gio, glib};

/// How long [`prepare`] waits for the portal.
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(2);
const GDK_DEBUG: &str = "GDK_DEBUG";
const NO_PORTALS: &str = "no-portals";
/// What [`prepare`] decided.
static MODE: OnceLock<Mode> = OnceLock::new();

/// How GTK reads its settings in this process.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Through the settings portal, as usual.
    Portal,
    /// Without the portal; `external` if `GDK_DEBUG=no-portals` came from
    /// the environment (session starter), not from [`prepare`].
    WithoutPortal { external: bool },
}

/// Whether the `GDK_DEBUG` value `value` already turns the portals off.
/// GDK accepts `,`, `:`, `;` and spaces between flags and `all`.
pub fn has_no_portals(value: Option<&str>) -> bool {
    value.is_some_and(|v| {
        v.split([',', ':', ';', ' ', '\t'])
            .any(|flag| flag.eq_ignore_ascii_case(NO_PORTALS))
    })
}

/// `value` with `no-portals` added.
pub fn with_no_portals(value: Option<&str>) -> String {
    match value.map(str::trim) {
        Some(v) if !v.is_empty() => format!("{v},{NO_PORTALS}"),
        _ => NO_PORTALS.to_string(),
    }
}

/// The `busctl` call that reads one value from the portal; it starts the
/// portal through D-Bus activation if it is not running.
fn probe_command(timeout: Duration) -> Command {
    let mut command = Command::new("busctl");
    command
        .arg("--user")
        .arg(format!("--timeout={}", timeout.as_secs().max(1)))
        .args([
            "call",
            "org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.Settings",
            "ReadOne",
            "ss",
            "org.gnome.desktop.interface",
            "gtk-theme",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    command
}

/// Asks the portal for the GTK theme; `Ok` with the time it took if a value
/// came back, else the reason. An error answer (e.g. no GTK backend behind
/// the portal) counts as no answer: GTK would get no theme from it either.
fn probe(timeout: Duration) -> Result<Duration, String> {
    let start = Instant::now();
    let mut child = probe_command(timeout)
        .spawn()
        .map_err(|e| format!("busctl nicht startbar: {e}"))?;
    // busctl stops itself after `timeout`; this is only the safety net.
    let deadline = start + timeout + Duration::from_millis(500);
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(start.elapsed()),
            Ok(Some(_)) => {
                let mut text = String::new();
                if let Some(mut err) = child.stderr.take() {
                    let _ = std::io::Read::read_to_string(&mut err, &mut text);
                }
                let line = text.lines().next().unwrap_or("ohne Meldung").trim();
                return Err(line.to_string());
            }
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("keine Antwort".to_string());
            }
            Err(e) => return Err(e.to_string()),
        }
    }
}

/// Decides how GTK reads its settings; call first thing in `main`, before
/// GTK or any other thread starts (it may change the environment).
/// `program` prefixes the log lines.
pub fn prepare(program: &str) -> Mode {
    let mode = decide(program);
    let _ = MODE.set(mode);
    mode
}

fn decide(program: &str) -> Mode {
    let before = std::env::var(GDK_DEBUG).ok();
    if has_no_portals(before.as_deref()) {
        eprintln!(
            "{program}: GDK_DEBUG=no-portals aus der Umgebung: GTK ohne Settings-Portal, \
             vom Benutzer gesetzte gsettings-Werte gelten"
        );
        return Mode::WithoutPortal { external: true };
    }
    match probe(PROBE_TIMEOUT) {
        Ok(took) => {
            eprintln!(
                "{program}: Settings-Portal antwortet ({} ms)",
                took.as_millis()
            );
            Mode::Portal
        }
        Err(reason) => {
            // SAFETY: called at the very start of main, before GTK, GLib or
            // any thread of this process exists; nothing reads the
            // environment concurrently.
            unsafe { std::env::set_var(GDK_DEBUG, with_no_portals(before.as_deref())) };
            eprintln!(
                "{program}: Settings-Portal antwortet nicht in {} s ({reason}): GTK startet \
                 ohne Portal, Thema, Schrift und Zeiger aus gsettings",
                PROBE_TIMEOUT.as_secs()
            );
            Mode::WithoutPortal { external: false }
        }
    }
}

/// What programs started from here get for `GDK_DEBUG` instead of this
/// process's value: in [`Mode::WithoutPortal`] the same flags without
/// `no-portals` (`Some(None)`: unset), else `None` (nothing to change).
/// `no-portals` only serves this process, whether [`prepare`] or the
/// session starter set it; a program started later should find the portal
/// running by then. The environment of this process cannot be put back
/// once GTK runs (Pango's fontconfig thread reads it), so every start of
/// another program goes through [`for_children`] or
/// [`launcher_for_children`].
fn child_value() -> Option<Option<String>> {
    child_value_for(
        MODE.get().copied(),
        std::env::var(GDK_DEBUG).ok().as_deref(),
    )
}

/// [`child_value`] for decision `mode` and this process's value `current`.
fn child_value_for(mode: Option<Mode>, current: Option<&str>) -> Option<Option<String>> {
    if !matches!(mode, Some(Mode::WithoutPortal { .. })) {
        return None;
    }
    let rest: Vec<&str> = current
        .unwrap_or_default()
        .split([',', ':', ';', ' ', '\t'])
        .filter(|flag| !flag.is_empty() && !flag.eq_ignore_ascii_case(NO_PORTALS))
        .collect();
    Some((!rest.is_empty()).then(|| rest.join(",")))
}

/// Gives programs launched with `context` the portal again (see
/// [`child_value`]).
pub fn for_children(context: &impl IsA<gio::AppLaunchContext>) {
    match child_value() {
        Some(Some(value)) => context.setenv(GDK_DEBUG, value),
        Some(None) => context.unsetenv(GDK_DEBUG),
        None => {}
    }
}

/// Gives programs spawned with `launcher` the portal again (see
/// [`child_value`]).
pub fn launcher_for_children(launcher: &gio::SubprocessLauncher) {
    match child_value() {
        Some(Some(value)) => launcher.setenv(GDK_DEBUG, value, true),
        Some(None) => launcher.unsetenv(GDK_DEBUG),
        None => {}
    }
}

/// A GTK setting with its value from GSettings.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Text(String),
    Number(i32),
    Flag(bool),
}

/// The GSettings keys the GTK portal backend passes on (except the icon
/// theme), as (schema, key).
pub const KEYS: &[(&str, &str)] = &[
    ("org.gnome.desktop.interface", "gtk-theme"),
    ("org.gnome.desktop.interface", "font-name"),
    ("org.gnome.desktop.interface", "cursor-theme"),
    ("org.gnome.desktop.interface", "cursor-size"),
    ("org.gnome.desktop.interface", "color-scheme"),
    ("org.gnome.desktop.interface", "enable-animations"),
    ("org.gnome.desktop.interface", "cursor-blink"),
    ("org.gnome.desktop.interface", "font-antialiasing"),
    ("org.gnome.desktop.interface", "font-hinting"),
    ("org.gnome.desktop.interface", "font-rgba-order"),
    ("org.gnome.desktop.peripherals.mouse", "double-click"),
];

/// The `GtkSettings` properties for GSettings key `key` with value `value`
/// (the same translation GDK applies to portal values); empty for unknown
/// keys or values of the wrong type.
pub fn translate(key: &str, value: &glib::Variant) -> Vec<(&'static str, Value)> {
    let text = || value.str().map(str::to_string);
    let number = || {
        value
            .get::<i32>()
            .or_else(|| value.get::<u32>().map(|n| n as i32))
    };
    match key {
        "gtk-theme" => text().map(|t| vec![("gtk-theme-name", Value::Text(t))]),
        "font-name" => text().map(|t| vec![("gtk-font-name", Value::Text(t))]),
        "cursor-theme" => text().map(|t| vec![("gtk-cursor-theme-name", Value::Text(t))]),
        "cursor-size" => number().map(|n| vec![("gtk-cursor-theme-size", Value::Number(n))]),
        "color-scheme" => text().map(|t| {
            vec![(
                "gtk-application-prefer-dark-theme",
                Value::Flag(t == "prefer-dark"),
            )]
        }),
        "enable-animations" => value
            .get::<bool>()
            .map(|b| vec![("gtk-enable-animations", Value::Flag(b))]),
        "cursor-blink" => value
            .get::<bool>()
            .map(|b| vec![("gtk-cursor-blink", Value::Flag(b))]),
        "font-antialiasing" => {
            text().map(|t| vec![("gtk-xft-antialias", Value::Number(i32::from(t != "none")))])
        }
        "font-hinting" => text().map(|t| {
            let style = match t.as_str() {
                "none" => "hintnone",
                "slight" => "hintslight",
                "medium" => "hintmedium",
                _ => "hintfull",
            };
            vec![
                ("gtk-xft-hinting", Value::Number(i32::from(t != "none"))),
                ("gtk-xft-hintstyle", Value::Text(style.to_string())),
            ]
        }),
        "font-rgba-order" => text().map(|t| vec![("gtk-xft-rgba", Value::Text(t))]),
        "double-click" => number().map(|n| vec![("gtk-double-click-time", Value::Number(n))]),
        _ => None,
    }
    .unwrap_or_default()
}

thread_local! {
    /// The watched settings objects; they must stay alive for their signals.
    static WATCHED: RefCell<Vec<gio::Settings>> = const { RefCell::new(Vec::new()) };
}

/// Sets the GTK properties of `key` from `settings`.
/// With `only_user`, only a value the user has set counts; without one,
/// the property goes back to what GTK has from elsewhere (`settings.ini`,
/// built-in default).
fn apply(gtk_settings: &gtk4::Settings, settings: &gio::Settings, key: &str, only_user: bool) {
    let user = settings.user_value(key);
    for (property, value) in translate(key, &user.clone().unwrap_or_else(|| settings.value(key))) {
        if gtk_settings.find_property(property).is_none() {
            continue;
        }
        if only_user && user.is_none() {
            gtk_settings.reset_property(property);
            continue;
        }
        match value {
            Value::Text(t) => gtk_settings.set_property(property, t),
            Value::Number(n) => gtk_settings.set_property(property, n),
            Value::Flag(b) => gtk_settings.set_property(property, b),
        }
    }
}

/// In [`Mode::WithoutPortal`]: sets the values of [`KEYS`] from GSettings
/// on this process's `GtkSettings` and follows their changes. Call after
/// GTK is initialised. Nothing in [`Mode::Portal`].
///
/// If `no-portals` came from outside (`external`), only values the user has
/// set count: test harnesses set `GDK_DEBUG=no-portals` together with their
/// own `settings.ini` and an empty dconf, whose schema defaults must not
/// override it; in the session the values that matter (theme, font,
/// cursor) are the user's choices anyway.
pub fn follow_gsettings(program: &'static str, mode: Mode) {
    let only_user = match mode {
        Mode::Portal => return,
        Mode::WithoutPortal { external } => external,
    };
    let Some(gtk_settings) = gtk4::Settings::default() else {
        return;
    };
    let Some(source) = gio::SettingsSchemaSource::default() else {
        eprintln!("{program}: keine GSettings-Schemas, GTK bleibt bei seinen Vorgaben");
        return;
    };
    let mut schemas: Vec<&str> = KEYS.iter().map(|(schema, _)| *schema).collect();
    schemas.dedup();
    for schema_id in schemas {
        let Some(schema) = source.lookup(schema_id, true) else {
            continue;
        };
        let settings = gio::Settings::new(schema_id);
        let keys: Vec<&'static str> = KEYS
            .iter()
            .filter(|(s, k)| *s == schema_id && schema.has_key(k))
            .map(|(_, k)| *k)
            .collect();
        for key in &keys {
            apply(&gtk_settings, &settings, key, only_user);
        }
        let watched_keys = keys.clone();
        settings.connect_changed(None, move |settings, key| {
            if watched_keys.contains(&key)
                && let Some(gtk_settings) = gtk4::Settings::default()
            {
                apply(&gtk_settings, settings, key, only_user);
                eprintln!(
                    "{program}: gsettings {key} = {} übernommen",
                    settings.value(key)
                );
            }
        });
        WATCHED.with(|w| w.borrow_mut().push(settings));
    }
    eprintln!(
        "{program}: ohne Portal aus gsettings{}: Thema {}, Schrift {}, Zeiger {} {} (folgt Änderungen)",
        if only_user {
            " (nur Benutzerwerte)"
        } else {
            ""
        },
        gtk_settings.gtk_theme_name().unwrap_or_default(),
        gtk_settings.gtk_font_name().unwrap_or_default(),
        gtk_settings.gtk_cursor_theme_name().unwrap_or_default(),
        gtk_settings.gtk_cursor_theme_size(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_portals_is_found_in_any_list() {
        assert!(!has_no_portals(None));
        assert!(!has_no_portals(Some("")));
        assert!(!has_no_portals(Some("portals")));
        assert!(!has_no_portals(Some("no-portals-x")));
        for v in [
            "no-portals",
            "events,no-portals",
            "no-portals:misc",
            "a; no-portals",
            "NO-PORTALS",
        ] {
            assert!(has_no_portals(Some(v)), "{v}");
        }
    }

    #[test]
    fn no_portals_is_added_to_existing_flags() {
        assert_eq!(with_no_portals(None), "no-portals");
        assert_eq!(with_no_portals(Some(" ")), "no-portals");
        assert_eq!(with_no_portals(Some("events")), "events,no-portals");
        assert!(has_no_portals(Some(&with_no_portals(Some("misc")))));
    }

    #[test]
    fn children_get_the_flags_without_no_portals() {
        for mode in [
            Mode::WithoutPortal { external: false },
            Mode::WithoutPortal { external: true },
        ] {
            assert_eq!(child_value_for(Some(mode), Some("no-portals")), Some(None));
            assert_eq!(child_value_for(Some(mode), None), Some(None));
            assert_eq!(
                child_value_for(Some(mode), Some("events,no-portals")),
                Some(Some("events".to_string()))
            );
            assert_eq!(
                child_value_for(Some(mode), Some("misc:NO-PORTALS events")),
                Some(Some("misc,events".to_string()))
            );
        }
        assert_eq!(child_value_for(Some(Mode::Portal), Some("events")), None);
        assert_eq!(child_value_for(None, Some("no-portals")), None);
    }

    #[test]
    fn probe_asks_the_settings_portal_for_the_theme() {
        let command = probe_command(Duration::from_secs(2));
        assert_eq!(command.get_program(), "busctl");
        let args: Vec<_> = command
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args[0], "--user");
        assert_eq!(args[1], "--timeout=2");
        assert!(args.contains(&"org.freedesktop.portal.Settings".to_string()));
        assert_eq!(args.last().map(String::as_str), Some("gtk-theme"));
    }

    #[test]
    fn gsettings_values_become_gtk_properties() {
        let s = |t: &str| t.to_variant();
        assert_eq!(
            translate("gtk-theme", &s("Querencia-Klassisch")),
            vec![(
                "gtk-theme-name",
                Value::Text("Querencia-Klassisch".to_string())
            )]
        );
        assert_eq!(
            translate("font-name", &s("DejaVu Sans 10")),
            vec![("gtk-font-name", Value::Text("DejaVu Sans 10".to_string()))]
        );
        assert_eq!(
            translate("cursor-size", &24i32.to_variant()),
            vec![("gtk-cursor-theme-size", Value::Number(24))]
        );
        assert_eq!(
            translate("color-scheme", &s("prefer-dark")),
            vec![("gtk-application-prefer-dark-theme", Value::Flag(true))]
        );
        assert_eq!(
            translate("color-scheme", &s("default")),
            vec![("gtk-application-prefer-dark-theme", Value::Flag(false))]
        );
        assert_eq!(
            translate("font-hinting", &s("slight")),
            vec![
                ("gtk-xft-hinting", Value::Number(1)),
                ("gtk-xft-hintstyle", Value::Text("hintslight".to_string()))
            ]
        );
        assert_eq!(
            translate("font-antialiasing", &s("none")),
            vec![("gtk-xft-antialias", Value::Number(0))]
        );
        assert_eq!(
            translate("double-click", &400i32.to_variant()),
            vec![("gtk-double-click-time", Value::Number(400))]
        );
        assert_eq!(
            translate("enable-animations", &false.to_variant()),
            vec![("gtk-enable-animations", Value::Flag(false))]
        );
    }

    #[test]
    fn unknown_keys_and_wrong_types_change_nothing() {
        assert!(translate("icon-theme", &"Querencia".to_variant()).is_empty());
        assert!(translate("gtk-theme", &5i32.to_variant()).is_empty());
        assert!(translate("cursor-size", &"24".to_variant()).is_empty());
    }

    #[test]
    fn every_key_has_a_translation() {
        let sample = |key: &str| match key {
            "cursor-size" | "double-click" => 1i32.to_variant(),
            "enable-animations" | "cursor-blink" => true.to_variant(),
            _ => "x".to_variant(),
        };
        for (_, key) in KEYS {
            assert!(!translate(key, &sample(key)).is_empty(), "{key}");
        }
    }
}
