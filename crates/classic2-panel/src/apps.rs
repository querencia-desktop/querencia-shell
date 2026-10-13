//! Application catalog for the applications menu.
//!
//! Reads the installed applications through GIO, keeps the ones GIO says
//! should be shown, gives each one exactly one menu group and sorts them by
//! name for the current locale. No widgets here; `app_menu.rs` builds the
//! GTK menu on top of this. Details and test results:
//! `docs/m1-1b1a-anwendungskatalog-test.md`.

use std::collections::HashSet;

use gio_unix::DesktopAppInfo;
use gtk4::gio;
use gtk4::glib::CollationKey;
use gtk4::prelude::*;

/// Submenus of "Anwendungen"; declaration order is menu order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MenuGroup {
    Accessories,
    Graphics,
    Internet,
    Office,
    Programming,
    Games,
    Multimedia,
    SystemTools,
    Other,
}

impl MenuGroup {
    pub fn label(self) -> &'static str {
        match self {
            Self::Accessories => "Zubehör",
            Self::Graphics => "Grafik",
            Self::Internet => "Internet",
            Self::Office => "Büro",
            Self::Programming => "Programmierung",
            Self::Games => "Spiele",
            Self::Multimedia => "Multimedia",
            Self::SystemTools => "Systemwerkzeuge",
            Self::Other => "Sonstiges",
        }
    }
}

/// Classification rules, highest priority first. The first rule naming one
/// of the entry's freedesktop categories wins, whatever their order in the
/// desktop file. Specific groups come before the broad `System` and
/// `Utility`. `Settings` is no rule: settings keep their group here and are
/// recognisable through [`AppEntry::is_settings`] for the later "System" menu.
const RULES: [(MenuGroup, &[&str]); 8] = [
    (MenuGroup::Games, &["Game"]),
    (MenuGroup::Programming, &["Development"]),
    (MenuGroup::Graphics, &["Graphics"]),
    (MenuGroup::Multimedia, &["AudioVideo", "Audio", "Video"]),
    (MenuGroup::Internet, &["Network"]),
    (MenuGroup::Office, &["Office"]),
    (MenuGroup::SystemTools, &["System"]),
    (MenuGroup::Accessories, &["Utility", "Accessibility"]),
];

/// Menu group for a list of freedesktop categories; anything no rule
/// covers, including no categories at all, goes to [`MenuGroup::Other`].
pub fn menu_group<S: AsRef<str>>(categories: &[S]) -> MenuGroup {
    RULES
        .iter()
        .find(|(_, names)| categories.iter().any(|c| names.contains(&c.as_ref())))
        .map_or(MenuGroup::Other, |(group, _)| *group)
}

/// One application in the menu, as plain data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppEntry {
    /// Desktop ID such as `foot.desktop`: unique in the catalog and the key
    /// to load the application again for launching (`DesktopAppInfo::new`).
    pub id: String,
    /// Localised `Name` of the desktop entry.
    pub name: String,
    /// `Categories` in desktop-file order.
    pub categories: Vec<String>,
    /// Serialised `GIcon` (theme name or file path), not resolved against an
    /// icon theme; `gio::Icon::for_string` turns it back into an icon.
    #[allow(dead_code, reason = "menu icons follow in a later step")]
    pub icon: Option<String>,
    pub group: MenuGroup,
}

impl AppEntry {
    pub fn is_settings(&self) -> bool {
        self.categories.iter().any(|c| c == "Settings")
    }
}

/// Why an application GIO listed is not in the catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exclusion {
    /// Only a guard: GLib 2.88 gives every desktop entry an ID, even one
    /// loaded by file name.
    NoDesktopId,
    NotADesktopEntry,
    DuplicateId,
    NoDisplay,
    /// `OnlyShowIn`/`NotShowIn` rule out the current `XDG_CURRENT_DESKTOP`.
    NotShownInDesktop,
}

#[derive(Debug, Default)]
pub struct Catalog {
    /// Sorted by menu group, then by name for the current locale, then by ID.
    pub entries: Vec<AppEntry>,
    /// Sorted by ID.
    pub excluded: Vec<(String, Exclusion)>,
}

impl Catalog {
    /// Reads all installed applications. GIO itself drops `Hidden` entries
    /// and entries whose `TryExec` or `Exec` program is missing, and resolves
    /// equal desktop IDs by `XDG_DATA_HOME`/`XDG_DATA_DIRS` precedence;
    /// `should_show` applies `NoDisplay`, `OnlyShowIn` and `NotShowIn` against
    /// `XDG_CURRENT_DESKTOP`. Sorting uses the process locale, which
    /// `gtk4::init` sets.
    pub fn load() -> Self {
        Self::from_app_infos(gio::AppInfo::all())
    }

    fn from_app_infos(infos: impl IntoIterator<Item = gio::AppInfo>) -> Self {
        let mut catalog = Self::default();
        let mut seen = HashSet::new();
        for info in infos {
            let Some(id) = info.id().map(|id| id.to_string()) else {
                catalog
                    .excluded
                    .push((info.name().to_string(), Exclusion::NoDesktopId));
                continue;
            };
            let Some(desktop) = info.downcast_ref::<DesktopAppInfo>() else {
                catalog.excluded.push((id, Exclusion::NotADesktopEntry));
                continue;
            };
            if !seen.insert(id.clone()) {
                catalog.excluded.push((id, Exclusion::DuplicateId));
                continue;
            }
            if !info.should_show() {
                let reason = if desktop.is_nodisplay() {
                    Exclusion::NoDisplay
                } else {
                    Exclusion::NotShownInDesktop
                };
                catalog.excluded.push((id, reason));
                continue;
            }

            let categories: Vec<String> = desktop
                .categories()
                .map(|list| {
                    list.split(';')
                        .filter(|c| !c.is_empty())
                        .map(String::from)
                        .collect()
                })
                .unwrap_or_default();
            catalog.entries.push(AppEntry {
                group: menu_group(&categories),
                name: info.name().to_string(),
                icon: info
                    .icon()
                    .and_then(|icon| IconExt::to_string(&icon))
                    .map(|icon| icon.to_string()),
                categories,
                id,
            });
        }

        catalog
            .entries
            .sort_by_cached_key(|e| (e.group, CollationKey::from(&e.name), e.id.clone()));
        catalog.excluded.sort_by(|a, b| a.0.cmp(&b.0));
        catalog
    }

    /// Non-empty groups in menu order with their entries.
    pub fn groups(&self) -> impl Iterator<Item = (MenuGroup, &[AppEntry])> {
        self.entries
            .chunk_by(|a, b| a.group == b.group)
            .map(|chunk| (chunk[0].group, chunk))
    }
}

#[cfg(test)]
mod tests;
