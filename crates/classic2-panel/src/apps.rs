//! Application catalog for the applications menu.
//!
//! Reads the installed applications through GIO, keeps the ones GIO says
//! should be shown, gives each one exactly one menu group and sorts them by
//! name for the current locale. No widgets here; the GTK menu is built on
//! top of this from M1.1b-1b on. Details and test results:
//! `docs/m1-1b1a-anwendungskatalog-test.md`.

use std::collections::HashSet;

use gio_unix::DesktopAppInfo;
use gtk4::gio;
use gtk4::glib::CollationKey;
use gtk4::prelude::*;

/// Submenus of "Anwendungen"; declaration order is menu order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
    pub const ALL: [Self; 9] = [
        Self::Accessories,
        Self::Graphics,
        Self::Internet,
        Self::Office,
        Self::Programming,
        Self::Games,
        Self::Multimedia,
        Self::SystemTools,
        Self::Other,
    ];

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
mod tests {
    use std::collections::BTreeMap;
    use std::ffi::{c_char, c_int};
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use gtk4::glib;

    use super::*;

    // ---- Classification without GIO -------------------------------------

    #[test]
    fn every_rule_maps_its_categories() {
        for (group, names) in RULES {
            for name in names {
                assert_eq!(menu_group(&[*name]), group, "Kategorie {name}");
            }
        }
    }

    #[test]
    fn missing_or_unmapped_categories_go_to_other() {
        assert_eq!(menu_group::<&str>(&[]), MenuGroup::Other);
        assert_eq!(menu_group(&["Education", "Science"]), MenuGroup::Other);
        assert_eq!(
            menu_group(&["Settings", "DesktopSettings"]),
            MenuGroup::Other
        );
        // Categories are case-sensitive in the desktop entry specification.
        assert_eq!(menu_group(&["game"]), MenuGroup::Other);
    }

    #[test]
    fn rule_priority_not_file_order_decides() {
        assert_eq!(
            menu_group(&["Office", "Graphics", "Viewer"]),
            MenuGroup::Graphics
        );
        assert_eq!(menu_group(&["Graphics", "Office"]), MenuGroup::Graphics);
        assert_eq!(
            menu_group(&["Network", "AudioVideo"]),
            MenuGroup::Multimedia
        );
        assert_eq!(menu_group(&["Development", "Game"]), MenuGroup::Games);
        assert_eq!(
            menu_group(&["Utility", "System", "TerminalEmulator"]),
            MenuGroup::SystemTools
        );
        assert_eq!(
            menu_group(&["Settings", "System", "HardwareSettings"]),
            MenuGroup::SystemTools
        );
    }

    #[test]
    fn groups_are_listed_in_menu_order() {
        let labels: Vec<_> = MenuGroup::ALL.iter().map(|g| g.label()).collect();
        assert_eq!(
            labels,
            [
                "Zubehör",
                "Grafik",
                "Internet",
                "Büro",
                "Programmierung",
                "Spiele",
                "Multimedia",
                "Systemwerkzeuge",
                "Sonstiges"
            ]
        );
        assert!(MenuGroup::ALL.is_sorted());
    }

    #[test]
    fn settings_stay_in_the_catalog_and_are_marked() {
        let entry = |categories: &[&str]| AppEntry {
            id: "x.desktop".into(),
            name: "X".into(),
            categories: categories.iter().map(|c| c.to_string()).collect(),
            icon: None,
            group: menu_group(categories),
        };
        assert!(entry(&["Settings", "DesktopSettings"]).is_settings());
        assert!(!entry(&["Utility"]).is_settings());
    }

    #[test]
    fn groups_split_the_sorted_entries() {
        let entry = |id: &str, group| AppEntry {
            id: id.into(),
            name: id.into(),
            categories: Vec::new(),
            icon: None,
            group,
        };
        let catalog = Catalog {
            entries: vec![
                entry("a", MenuGroup::Accessories),
                entry("b", MenuGroup::Accessories),
                entry("c", MenuGroup::SystemTools),
                entry("d", MenuGroup::Other),
            ],
            excluded: Vec::new(),
        };
        let groups: Vec<_> = catalog
            .groups()
            .map(|(group, entries)| {
                (
                    group,
                    entries.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(),
                )
            })
            .collect();
        assert_eq!(
            groups,
            [
                (MenuGroup::Accessories, vec!["a", "b"]),
                (MenuGroup::SystemTools, vec!["c"]),
                (MenuGroup::Other, vec!["d"]),
            ]
        );
    }

    // ---- GIO in an isolated child process -------------------------------
    //
    // GLib reads XDG_DATA_DIRS, XDG_CURRENT_DESKTOP and the language once per
    // process. The tests below therefore never touch their own environment:
    // they start this test binary again with a cleared, explicit environment
    // and only the test `catalog_child_process`, which prints the catalog as
    // tab-separated lines prefixed with `C2`.

    const CHILD_MARKER: &str = "CLASSIC2_CATALOG_CHILD";
    const CHILD_TEST: &str = "apps::tests::catalog_child_process";

    fn fixtures() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/app-catalog")
    }

    /// Runs in the child process only; in a normal test run it does nothing.
    #[test]
    fn catalog_child_process() {
        if std::env::var_os(CHILD_MARKER).is_none() {
            return;
        }
        // libtest has already written "test ... " without a line break.
        println!("\nC2\tlocale\t{}", set_locale_from_environment());
        let catalog = Catalog::load();
        for e in &catalog.entries {
            println!(
                "C2\tentry\t{}\t{}\t{}\t{}\t{}\t{}",
                e.id,
                e.group.label(),
                e.name,
                e.icon.as_deref().unwrap_or("-"),
                e.categories.join(";"),
                e.is_settings()
            );
        }
        for (id, reason) in &catalog.excluded {
            println!("C2\texcluded\t{id}\t{reason:?}");
        }
        report_files_gio_dropped(&catalog);

        // `AppInfo::all` never repeats an ID; feed every one twice to check
        // the duplicate guard.
        let all = gio::AppInfo::all();
        let doubled = Catalog::from_app_infos(all.iter().chain(&all).cloned());
        let duplicates = doubled
            .excluded
            .iter()
            .filter(|(_, r)| *r == Exclusion::DuplicateId)
            .count();
        println!(
            "C2\tdoubled\t{}\t{duplicates}\t{}",
            doubled.entries == catalog.entries,
            all.len()
        );
    }

    /// GTK calls `setlocale(LC_ALL, "")` in `gtk4::init`; the child has no
    /// display, so it makes the same call itself. `LC_ALL` is 6 in glibc.
    fn set_locale_from_environment() -> bool {
        unsafe extern "C" {
            fn setlocale(category: c_int, locale: *const c_char) -> *mut c_char;
        }
        const LC_ALL: c_int = 6;
        !unsafe { setlocale(LC_ALL, c"".as_ptr()) }.is_null()
    }

    /// Diagnostics only: desktop files in the data directories that GIO did
    /// not list at all, with GIO's own reading of them. The menu code never
    /// looks at files itself.
    fn report_files_gio_dropped(catalog: &Catalog) {
        let listed: HashSet<&str> = catalog
            .entries
            .iter()
            .map(|e| e.id.as_str())
            .chain(catalog.excluded.iter().map(|(id, _)| id.as_str()))
            .collect();
        let mut seen = HashSet::new();
        let dirs = std::iter::once(glib::user_data_dir()).chain(glib::system_data_dirs());
        for dir in dirs.map(|d| d.join("applications")) {
            let mut files = Vec::new();
            desktop_files(&dir, &mut files);
            files.sort();
            for file in files {
                let id = file
                    .strip_prefix(&dir)
                    .expect("found below dir")
                    .to_string_lossy()
                    .replace('/', "-");
                let path = file.display();
                if !seen.insert(id.clone()) {
                    println!("C2\tshadowed\t{id}\t{path}");
                } else if !listed.contains(id.as_str()) {
                    let reason = match DesktopAppInfo::from_filename(&file) {
                        None => "nicht ladbar (Type, TryExec oder Exec)",
                        Some(info) if info.is_hidden() => "Hidden",
                        Some(_) => "unbekannt",
                    };
                    println!("C2\tdropped\t{id}\t{reason}\t{path}");
                }
            }
        }
    }

    fn desktop_files(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(read) = std::fs::read_dir(dir) else {
            return;
        };
        for item in read.flatten() {
            let path = item.path();
            if path.is_dir() {
                desktop_files(&path, out);
            } else if path.extension().is_some_and(|e| e == "desktop") {
                out.push(path);
            }
        }
    }

    /// Records the child printed, keyed by kind (`entry`, `excluded`, ...).
    struct ChildRun {
        stdout: String,
    }

    impl ChildRun {
        fn records(&self, kind: &str) -> Vec<Vec<&str>> {
            self.stdout
                .lines()
                .filter_map(|line| line.strip_prefix("C2\t"))
                .map(|line| line.split('\t').collect::<Vec<_>>())
                .filter(|fields| fields[0] == kind)
                .map(|fields| fields[1..].to_vec())
                .collect()
        }

        /// Desktop ID → remaining fields.
        fn by_id(&self, kind: &str) -> BTreeMap<&str, Vec<&str>> {
            self.records(kind)
                .into_iter()
                .map(|fields| (fields[0], fields[1..].to_vec()))
                .collect()
        }

        fn entry_ids(&self) -> Vec<&str> {
            self.records("entry").iter().map(|f| f[0]).collect()
        }
    }

    fn run_child(env: &[(&str, String)]) -> ChildRun {
        let output = Command::new(std::env::current_exe().expect("test binary path"))
            .args(["--exact", CHILD_TEST, "--nocapture", "--test-threads=1"])
            .env_clear()
            .env(CHILD_MARKER, "1")
            .envs(env.iter().map(|(k, v)| (k, v)))
            .output()
            .expect("child test process starts");
        let stdout = String::from_utf8(output.stdout).expect("UTF-8 output");
        assert!(
            output.status.success(),
            "Kindprozess fehlgeschlagen:\n{stdout}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        ChildRun { stdout }
    }

    /// Only the fixture directories, German names, English collation (the
    /// VM has `en_US.UTF-8` but no German locale), the given desktop.
    fn fixture_run(desktop: Option<&str>) -> ChildRun {
        let base = fixtures();
        let mut env = vec![
            ("PATH", "/usr/bin:/bin".to_string()),
            ("HOME", base.join("kein-home").display().to_string()),
            (
                "XDG_DATA_HOME",
                base.join("daten-home").display().to_string(),
            ),
            (
                "XDG_DATA_DIRS",
                base.join("daten-system").display().to_string(),
            ),
            ("LANG", "en_US.UTF-8".to_string()),
            ("LANGUAGE", "de".to_string()),
        ];
        if let Some(desktop) = desktop {
            env.push(("XDG_CURRENT_DESKTOP", desktop.to_string()));
        }
        run_child(&env)
    }

    fn sorted(mut ids: Vec<&str>) -> Vec<&str> {
        ids.sort();
        ids
    }

    #[test]
    fn fixtures_in_classic2_session() {
        let run = fixture_run(Some("X-Classic2"));
        assert_eq!(run.records("locale"), [["true"]], "en_US.UTF-8 fehlt");

        assert_eq!(
            sorted(run.entry_ids()),
            [
                "c2-basis.desktop",
                "c2-doppelt.desktop",
                "c2-einstellung.desktop",
                "c2-icon-fehlt.desktop",
                "c2-mehrfach-buero-grafik.desktop",
                "c2-mehrfach-netz-medien.desktop",
                "c2-normal.desktop",
                "c2-ohne-kategorie.desktop",
                "c2-onlyshowin-classic2.desktop",
                "c2-systemeinstellung.desktop",
                "c2-tryexec-vorhanden.desktop",
                "c2-umlaut.desktop",
                "c2-zebra.desktop",
                "c2unter-eintrag.desktop",
            ]
        );

        // Fields after the ID: group, name, icon, categories, settings.
        let entries = run.by_id("entry");
        let field = |id: &str, i: usize| entries[id][i];
        assert_eq!(
            entries["c2-normal.desktop"],
            [
                "Zubehör",
                "Normal",
                "utilities-terminal",
                "Utility",
                "false"
            ]
        );
        assert_eq!(field("c2-ohne-kategorie.desktop", 0), "Sonstiges");
        assert_eq!(field("c2-ohne-kategorie.desktop", 2), "-");
        assert_eq!(field("c2-ohne-kategorie.desktop", 3), "");
        assert_eq!(field("c2-mehrfach-buero-grafik.desktop", 0), "Grafik");
        assert_eq!(field("c2-mehrfach-netz-medien.desktop", 0), "Multimedia");
        assert_eq!(field("c2-basis.desktop", 0), "Programmierung");
        assert_eq!(field("c2-umlaut.desktop", 1), "Ölwechsel");
        assert_eq!(field("c2-doppelt.desktop", 1), "Doppelt aus daten-home");
        assert_eq!(
            field("c2-icon-fehlt.desktop", 2),
            "classic2-nicht-vorhandenes-icon"
        );
        assert_eq!(field("c2-einstellung.desktop", 0), "Sonstiges");
        assert_eq!(field("c2-einstellung.desktop", 4), "true");
        assert_eq!(field("c2-systemeinstellung.desktop", 0), "Systemwerkzeuge");
        assert_eq!(field("c2-systemeinstellung.desktop", 4), "true");

        let excluded = run.by_id("excluded");
        let reason = |id: &str| excluded[id][0];
        assert_eq!(excluded.len(), 3);
        assert_eq!(reason("c2-nodisplay.desktop"), "NoDisplay");
        assert_eq!(reason("c2-onlyshowin-gnome.desktop"), "NotShownInDesktop");
        assert_eq!(reason("c2-notshowin-classic2.desktop"), "NotShownInDesktop");

        let dropped = run.by_id("dropped");
        assert_eq!(dropped.len(), 3);
        assert_eq!(dropped["c2-hidden.desktop"][0], "Hidden");
        assert_eq!(
            dropped["c2-tryexec-fehlt.desktop"][0],
            "nicht ladbar (Type, TryExec oder Exec)"
        );
        assert_eq!(
            dropped["c2-exec-fehlt.desktop"][0],
            "nicht ladbar (Type, TryExec oder Exec)"
        );

        // AppInfo::all lists entries and exclusions, not what GIO dropped.
        let listed = (entries.len() + excluded.len()).to_string();
        assert_eq!(
            run.records("doubled"),
            [["true", listed.as_str(), listed.as_str()]],
            "zweites Vorkommen jeder ID muss als DuplicateId herausfallen"
        );

        let shadowed = run.by_id("shadowed");
        assert_eq!(
            shadowed.keys().copied().collect::<Vec<_>>(),
            ["c2-doppelt.desktop"]
        );
        assert!(shadowed["c2-doppelt.desktop"][0].contains("daten-system"));
    }

    #[test]
    fn entries_are_grouped_and_sorted_for_the_locale() {
        let run = fixture_run(Some("X-Classic2"));
        let entries = run.records("entry");

        let rank = |label: &str| MenuGroup::ALL.iter().position(|g| g.label() == label);
        let ranks: Vec<_> = entries
            .iter()
            .map(|f| rank(f[1]).expect("known group"))
            .collect();
        assert!(
            ranks.is_sorted(),
            "Gruppen nicht in Menüreihenfolge: {ranks:?}"
        );

        // en_US collation puts "Ö" next to "O"; byte order would put it last.
        let accessories: Vec<_> = entries
            .iter()
            .filter(|f| f[1] == "Zubehör")
            .map(|f| f[2])
            .collect();
        assert_eq!(
            accessories,
            [
                "Doppelt aus daten-home",
                "Icon fehlt",
                "Normal",
                "Nur in Classic2",
                "Ölwechsel",
                "TryExec vorhanden",
                "Unterordner",
                "Zebra",
            ]
        );
    }

    #[test]
    fn show_in_rules_follow_xdg_current_desktop() {
        let shown = |run: &ChildRun, id: &str| run.entry_ids().contains(&id);
        let only_classic2 = "c2-onlyshowin-classic2.desktop";
        let only_gnome = "c2-onlyshowin-gnome.desktop";
        let not_classic2 = "c2-notshowin-classic2.desktop";

        let classic2 = fixture_run(Some("X-Classic2"));
        assert!(shown(&classic2, only_classic2));
        assert!(!shown(&classic2, only_gnome));
        assert!(!shown(&classic2, not_classic2));

        let gnome = fixture_run(Some("GNOME"));
        assert!(!shown(&gnome, only_classic2));
        assert!(shown(&gnome, only_gnome));
        assert!(shown(&gnome, not_classic2));

        let unset = fixture_run(None);
        assert!(!shown(&unset, only_classic2));
        assert!(!shown(&unset, only_gnome));
        assert!(shown(&unset, not_classic2));
    }

    #[test]
    fn catalog_is_deterministic() {
        let first = fixture_run(Some("X-Classic2"));
        let second = fixture_run(Some("X-Classic2"));
        assert_eq!(first.stdout, second.stdout);
        assert!(!first.records("entry").is_empty());
    }

    /// The applications installed on this machine, read like the Classic2
    /// session will: real data directories, `XDG_CURRENT_DESKTOP=X-Classic2`.
    /// Asserts only invariants; `--nocapture` prints the full report.
    #[test]
    fn installed_applications_report() {
        let mut env = vec![
            ("XDG_CURRENT_DESKTOP", "X-Classic2".to_string()),
            ("LANG", "en_US.UTF-8".to_string()),
            ("LANGUAGE", "de".to_string()),
        ];
        for key in ["PATH", "HOME", "XDG_DATA_HOME", "XDG_DATA_DIRS"] {
            if let Ok(value) = std::env::var(key) {
                env.push((key, value));
            }
        }
        let run = run_child(&env);
        assert_eq!(run.stdout, run_child(&env).stdout, "zwei Läufe weichen ab");

        let ids = run.entry_ids();
        let unique: HashSet<_> = ids.iter().collect();
        assert_eq!(unique.len(), ids.len(), "Desktop-ID doppelt im Katalog");
        for id in run.by_id("excluded").keys() {
            assert!(
                !unique.contains(id),
                "{id} zugleich erkannt und ausgeschlossen"
            );
        }

        println!(
            "Anwendungskatalog, XDG_CURRENT_DESKTOP=X-Classic2, LANG=en_US.UTF-8, LANGUAGE=de"
        );
        println!("Locale gesetzt: {}", run.records("locale")[0][0]);
        let entries = run.records("entry");
        println!("\nErkannt: {}", entries.len());
        for f in &entries {
            let settings = if f[5] == "true" {
                "  [Einstellung]"
            } else {
                ""
            };
            println!(
                "  {:<16} {:<28} {:<36} Icon: {:<22} Kategorien: {}{settings}",
                f[1], f[2], f[0], f[3], f[4]
            );
        }
        for (kind, title) in [
            ("excluded", "Von Classic2 ausgeschlossen (should_show)"),
            ("dropped", "Von GIO verworfen"),
            ("shadowed", "Verdeckt durch gleiche Desktop-ID weiter vorn"),
        ] {
            let records = run.records(kind);
            println!("\n{title}: {}", records.len());
            for f in records {
                println!("  {}", f.join("  "));
            }
        }
    }
}
