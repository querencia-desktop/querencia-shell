//! Application icons for the window list (HMP-06): from a window's Wayland
//! app ID to the icon of its desktop entry, as GNOME's window trackers do.
//! The desktop entries come from GIO (the same catalog as the Anwendungen
//! menu) and the icon is drawn by GTK's icon theme, which the panel has set
//! to GNOME Human; there is no icon handling of its own.
//!
//! An app ID names the desktop entry only by convention (`foot` and
//! `foot.desktop`, `io.github.lgse.Strata` and `io.github.lgse.Strata.desktop`);
//! some applications only match through `StartupWMClass` (the Firefox snap:
//! `firefox` and `firefox_firefox.desktop`). The rules, strongest first:
//!
//! 1. the desktop ID is `<app_id>.desktop`;
//! 2. the same, ignoring case;
//! 3. `StartupWMClass` equals the app ID, ignoring case;
//! 4. a snap entry `<snap>_<app_id>.desktop`;
//! 5. the last dot-separated part of the desktop ID equals the app ID
//!    (`org.gnome.Calculator.desktop` for `calculator`), ignoring case;
//! 6. the program of `Exec` is named like the app ID.
//!
//! The title is never used: it is the document, not the application.

use std::cell::RefCell;
use std::collections::HashMap;

use gtk4::prelude::*;
use gtk4::{gio, glib};

/// What the matching needs to know about one desktop entry.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Candidate {
    /// Desktop ID, e.g. `foot.desktop`.
    pub id: String,
    pub wm_class: Option<String>,
    /// File name of the program in `Exec`, e.g. `foot`.
    pub program: Option<String>,
}

/// Generic icon when no desktop entry matches or it has no icon.
pub const FALLBACK_ICON: &str = "application-x-executable";

/// Index of the desktop entry for `app_id`, by the rules of the module.
pub fn best_match(app_id: &str, candidates: &[Candidate]) -> Option<usize> {
    if app_id.is_empty() {
        return None;
    }
    let wanted = app_id.to_lowercase();
    let stem = |c: &Candidate| c.id.strip_suffix(".desktop").unwrap_or(&c.id).to_owned();
    let rules: [&dyn Fn(&Candidate) -> bool; 6] = [
        &|c| stem(c) == app_id,
        &|c| stem(c).to_lowercase() == wanted,
        &|c| {
            c.wm_class
                .as_ref()
                .is_some_and(|w| w.to_lowercase() == wanted)
        },
        &|c| {
            stem(c)
                .split_once('_')
                .is_some_and(|(_, app)| app.to_lowercase() == wanted)
        },
        &|c| {
            stem(c)
                .rsplit('.')
                .next()
                .is_some_and(|last| last.to_lowercase() == wanted)
        },
        &|c| {
            c.program
                .as_ref()
                .is_some_and(|p| p.to_lowercase() == wanted)
        },
    ];
    rules
        .iter()
        .find_map(|rule| candidates.iter().position(rule))
}

/// The program name of an `Exec` command line: the file name of its first
/// word, `env` and its assignments skipped.
pub fn program_of(commandline: &str) -> Option<String> {
    let words = glib::shell_parse_argv(commandline.trim()).ok()?;
    let mut words = words.iter().map(|w| w.to_string_lossy().into_owned());
    let mut first = words.next()?;
    if first == "env" {
        first = words.find(|w| !w.contains('='))?;
    }
    std::path::Path::new(&first)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
}

thread_local! {
    /// Icons by app ID; emptied when the installed applications change.
    static CACHE: RefCell<Option<HashMap<String, Option<gio::Icon>>>> = const { RefCell::new(None) };
}

/// The icon for a window with `app_id`; the generic program icon if there
/// is no desktop entry for it.
pub fn icon_for(app_id: &str) -> gio::Icon {
    let found = CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        let cache = cache.get_or_insert_with(|| {
            // forget everything when applications are (un)installed
            gio::AppInfoMonitor::get().connect_changed(|_| {
                CACHE.with(|c| c.borrow_mut().take());
            });
            HashMap::new()
        });
        cache
            .entry(app_id.to_owned())
            .or_insert_with(|| lookup(app_id))
            .clone()
    });
    found.unwrap_or_else(|| gio::ThemedIcon::new(FALLBACK_ICON).upcast())
}

fn lookup(app_id: &str) -> Option<gio::Icon> {
    if app_id.is_empty() {
        return None;
    }
    let apps: Vec<gio_unix::DesktopAppInfo> = gio::AppInfo::all()
        .into_iter()
        .filter_map(|app| app.downcast::<gio_unix::DesktopAppInfo>().ok())
        .collect();
    let candidates: Vec<Candidate> = apps
        .iter()
        .map(|app| Candidate {
            id: app.id().map(|id| id.to_string()).unwrap_or_default(),
            wm_class: app.startup_wm_class().map(|c| c.to_string()),
            program: app
                .commandline()
                .and_then(|c| program_of(&c.to_string_lossy())),
        })
        .collect();
    let index = best_match(app_id, &candidates)?;
    apps[index].icon()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, wm_class: Option<&str>, program: Option<&str>) -> Candidate {
        Candidate {
            id: id.into(),
            wm_class: wm_class.map(Into::into),
            program: program.map(Into::into),
        }
    }

    fn catalog() -> Vec<Candidate> {
        vec![
            entry("firefox_firefox.desktop", Some("firefox"), Some("firefox")),
            entry("foot.desktop", None, Some("foot")),
            entry("footclient.desktop", None, Some("footclient")),
            entry(
                "io.github.lgse.Strata.desktop",
                Some("io.github.lgse.Strata"),
                Some("strata"),
            ),
            entry(
                "org.example.Querencia.Terminal.desktop",
                Some("org.example.Querencia.Terminal"),
                Some("querencia-terminal"),
            ),
            entry(
                "org.querencia.Screenshot.desktop",
                Some("org.querencia.Screenshot"),
                Some("querencia-screenshot"),
            ),
            entry(
                "org.gnome.Calculator.desktop",
                None,
                Some("gnome-calculator"),
            ),
            entry("htop.desktop", None, Some("htop")),
            entry("vim.desktop", None, Some("vim")),
        ]
    }

    fn found(app_id: &str) -> Option<String> {
        let list = catalog();
        best_match(app_id, &list).map(|i| list[i].id.clone())
    }

    #[test]
    fn app_ids_of_the_querencia_session_find_their_entries() {
        assert_eq!(found("foot").as_deref(), Some("foot.desktop"));
        assert_eq!(found("firefox").as_deref(), Some("firefox_firefox.desktop"));
        assert_eq!(
            found("io.github.lgse.Strata").as_deref(),
            Some("io.github.lgse.Strata.desktop")
        );
        assert_eq!(
            found("org.example.Querencia.Terminal").as_deref(),
            Some("org.example.Querencia.Terminal.desktop")
        );
        assert_eq!(
            found("org.querencia.Screenshot").as_deref(),
            Some("org.querencia.Screenshot.desktop")
        );
    }

    #[test]
    fn weaker_rules_only_when_stronger_ones_find_nothing() {
        // case only: GIMP's entry is found by nothing else
        let gimp = vec![entry("GIMP.desktop", None, Some("gimp-2.10"))];
        assert_eq!(best_match("gimp", &gimp), Some(0));
        // the desktop ID (any case) beats another entry's StartupWMClass
        let list = vec![
            entry("bar.desktop", Some("Foo"), None),
            entry("foo.desktop", None, None),
        ];
        assert_eq!(best_match("Foo", &list), Some(1));
        // StartupWMClass alone: neither ID nor program name says it
        let idea = vec![entry(
            "jetbrains-idea.desktop",
            Some("jetbrains-idea-ce"),
            Some("idea.sh"),
        )];
        assert_eq!(best_match("jetbrains-idea-ce", &idea), Some(0));
        assert_eq!(found("FOOT").as_deref(), Some("foot.desktop"));
        assert_eq!(found("Firefox").as_deref(), Some("firefox_firefox.desktop"));
        // last part of a reverse-DNS ID
        assert_eq!(
            found("calculator").as_deref(),
            Some("org.gnome.Calculator.desktop")
        );
        // program name
        assert_eq!(
            found("gnome-calculator").as_deref(),
            Some("org.gnome.Calculator.desktop")
        );
        assert_eq!(
            found("strata").as_deref(),
            Some("io.github.lgse.Strata.desktop")
        );
        // footclient is its own entry, never foot's by a prefix
        assert_eq!(found("footclient").as_deref(), Some("footclient.desktop"));
        // an exact ID beats an earlier entry matching by program
        let list = vec![
            entry("other.desktop", None, Some("vim")),
            entry("vim.desktop", None, None),
        ];
        assert_eq!(best_match("vim", &list), Some(1));
    }

    #[test]
    fn unknown_or_missing_app_ids_match_nothing() {
        assert_eq!(found(""), None);
        assert_eq!(found("unbekannt.app"), None);
        assert_eq!(found("desktop"), None, "no match on the file suffix");
        assert_eq!(found("org"), None, "no match on a reverse-DNS prefix");
        assert_eq!(best_match("foot", &[]), None);
    }

    #[test]
    fn program_names_from_exec_lines() {
        assert_eq!(program_of("foot").as_deref(), Some("foot"));
        assert_eq!(program_of("/usr/bin/htop %U").as_deref(), Some("htop"));
        assert_eq!(
            program_of("env BAMF_DESKTOP_FILE_HINT=/x.desktop /snap/bin/firefox %u").as_deref(),
            Some("firefox")
        );
        assert_eq!(
            program_of("\"/opt/my app/run\" --x").as_deref(),
            Some("run")
        );
        assert_eq!(program_of("   ").as_deref(), None);
        assert_eq!(program_of("env").as_deref(), None);
    }
}
