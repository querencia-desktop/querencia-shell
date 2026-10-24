//! Tests of the application catalog and of the catalog monitor.

use std::cell::Cell;
use std::collections::BTreeMap;
use std::ffi::{c_char, c_int};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime};

use gtk4::glib;

use super::*;

/// Every group in menu order.
const ALL_GROUPS: [MenuGroup; 11] = [
    MenuGroup::Accessories,
    MenuGroup::Education,
    MenuGroup::Games,
    MenuGroup::Graphics,
    MenuGroup::Internet,
    MenuGroup::Office,
    MenuGroup::Programming,
    MenuGroup::Multimedia,
    MenuGroup::SystemTools,
    MenuGroup::Accessibility,
    MenuGroup::Other,
];

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
    assert_eq!(menu_group(&["Documentation", "Viewer"]), MenuGroup::Other);
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

/// GNOME 2 (`gnome-menus` `applications.menu`): "Zubehör" holds `Utility`
/// without `Accessibility` and without `System`; "Barrierefreiheit" holds
/// `Accessibility`; "Bildung" holds `Education` (Ubuntu also `Science`).
#[test]
fn gnome2_groups_for_education_and_accessibility() {
    assert_eq!(menu_group(&["Education", "Science"]), MenuGroup::Education);
    assert_eq!(menu_group(&["Science", "Math"]), MenuGroup::Education);
    assert_eq!(
        menu_group(&["Utility", "Accessibility"]),
        MenuGroup::Accessibility
    );
    assert_eq!(menu_group(&["Accessibility"]), MenuGroup::Accessibility);
    assert_eq!(
        menu_group(&["Utility", "TextEditor"]),
        MenuGroup::Accessories
    );
    // a learning game is a game, a programming course development
    assert_eq!(menu_group(&["Education", "Game"]), MenuGroup::Games);
    assert_eq!(
        menu_group(&["Education", "Development"]),
        MenuGroup::Programming
    );
}
/// HMP-04: the desktop entries of the three Querencia applications
/// (`session/querencia/applications`), plus the hidden bridge of the old
/// Files ID (QD-059). Read as key files, so the test does
/// not depend on the binaries being installed; GIO and the real binaries
/// are checked in the isolated GUI test.
#[test]
fn querencia_desktop_entries_land_in_accessories() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../session/querencia/applications");
    let mut ids: Vec<String> = std::fs::read_dir(&dir)
        .expect("session/querencia/applications")
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".desktop"))
        .collect();
    ids.sort();
    assert_eq!(
        ids,
        [
            "io.github.lgse.Strata.desktop",
            "org.example.Querencia.Terminal.desktop",
            "org.querencia.Files.desktop",
            "org.querencia.Screenshot.desktop",
        ]
    );
    let expected = [
        (
            "org.querencia.Files.desktop",
            "Querencia Files",
            "org.querencia.Files",
            "/target/debug/strata",
        ),
        (
            "org.example.Querencia.Terminal.desktop",
            "Querencia Terminal",
            "utilities-terminal",
            "/target/debug/querencia-terminal",
        ),
        (
            "org.querencia.Screenshot.desktop",
            "Bildschirmfoto aufnehmen",
            "applets-screenshooter",
            "/target/debug/querencia-screenshot",
        ),
    ];
    const GROUP: &str = "Desktop Entry";
    for (id, name, icon, binary) in expected {
        let file = glib::KeyFile::new();
        file.load_from_file(dir.join(id), glib::KeyFileFlags::NONE)
            .unwrap_or_else(|e| panic!("{id}: {e}"));
        let get = |key: &str| file.string(GROUP, key).map(|v| v.to_string()).ok();
        assert_eq!(get("Type").as_deref(), Some("Application"), "{id}");
        assert_eq!(get("Name").as_deref(), Some(name), "{id}");
        assert_eq!(get("Icon").as_deref(), Some(icon), "{id}");
        assert_eq!(get("NoDisplay"), None, "{id}");
        // the window's app ID is the desktop ID, so labwc and the window
        // list find the entry
        assert_eq!(
            get("StartupWMClass").as_deref(),
            id.strip_suffix(".desktop"),
            "{id}"
        );
        let categories: Vec<String> = get("Categories")
            .unwrap_or_default()
            .split(';')
            .filter(|c| !c.is_empty())
            .map(String::from)
            .collect();
        assert_eq!(menu_group(&categories), MenuGroup::Accessories, "{id}");
        // Exec starts the binary TryExec names, by absolute path
        let try_exec = get("TryExec").expect("TryExec");
        assert!(
            try_exec.starts_with('/') && try_exec.ends_with(binary),
            "{id}: {try_exec}"
        );
        let exec = get("Exec").expect("Exec");
        assert_eq!(exec.split(' ').next(), Some(try_exec.as_str()), "{id}");
        assert!(!exec.contains("grim"), "{id}: kein grim als Menüanwendung");
    }
    // only the file manager opens directories
    let files = glib::KeyFile::new();
    files
        .load_from_file(
            dir.join("org.querencia.Files.desktop"),
            glib::KeyFileFlags::NONE,
        )
        .unwrap();
    assert_eq!(
        files.string("Desktop Entry", "MimeType").unwrap(),
        "inode/directory;"
    );
    // the old ID stays as a hidden bridge: not in any menu, no MIME types,
    // starts the same binary with the new window ID
    let bridge = glib::KeyFile::new();
    bridge
        .load_from_file(
            dir.join("io.github.lgse.Strata.desktop"),
            glib::KeyFileFlags::NONE,
        )
        .unwrap();
    assert!(bridge.boolean("Desktop Entry", "NoDisplay").unwrap());
    assert!(bridge.string("Desktop Entry", "MimeType").is_err());
    assert_eq!(
        bridge.string("Desktop Entry", "Exec").unwrap(),
        files.string("Desktop Entry", "Exec").unwrap()
    );
    assert_eq!(
        bridge
            .string("Desktop Entry", "StartupWMClass")
            .unwrap()
            .as_str(),
        "org.querencia.Files"
    );
    assert!(
        files
            .string("Desktop Entry", "Exec")
            .unwrap()
            .ends_with(" %U"),
        "URIs (trash:///, computer:///) are passed on"
    );
}

#[test]
fn every_group_has_its_gnome2_icon() {
    let icons: Vec<_> = ALL_GROUPS.iter().map(|g| g.icon()).collect();
    assert_eq!(
        icons,
        [
            "applications-accessories",
            "applications-science",
            "applications-games",
            "applications-graphics",
            "applications-internet",
            "applications-office",
            "applications-development",
            "applications-multimedia",
            "applications-system",
            "preferences-desktop-accessibility",
            "applications-other",
        ]
    );
}

#[test]
fn groups_are_listed_in_menu_order() {
    let labels: Vec<_> = ALL_GROUPS.iter().map(|g| g.label()).collect();
    assert_eq!(
        labels,
        // Ubuntu 10.04 order (English names alphabetical), German labels
        // as in gnome-menus; "Other" last
        [
            "Accessories",
            "Education",
            "Games",
            "Graphics",
            "Internet",
            "Office",
            "Programming",
            "Sound & Video",
            "System Tools",
            "Universal Access",
            "Other"
        ]
    );
    assert!(ALL_GROUPS.is_sorted());
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
    for (section, entries) in catalog.system_sections() {
        for e in entries {
            println!("C2\tsystem\t{}\t{}\t{}", section.label(), e.id, e.name);
        }
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

    /// Every catalog line the child printed, in order, without libtest's
    /// own lines around them (`running 1 test`, `test result: … finished in
    /// 0.00s`); the run time there differs between otherwise equal runs.
    fn catalog(&self) -> Vec<&str> {
        self.stdout
            .lines()
            .filter(|line| line.starts_with("C2\t"))
            .collect()
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

/// QREL-04d: exactly the owner's list is left out of the menus, by desktop
/// ID; entries with similar names or IDs stay, and a group without entries
/// left (here "Systemwerkzeuge") is gone, as any empty group.
#[test]
fn owner_hidden_entries_leave_the_menus_and_nothing_else() {
    let base = std::env::temp_dir().join(format!("querencia-hidden-{}", std::process::id()));
    let apps = base.join("daten/applications");
    std::fs::create_dir_all(&apps).unwrap();
    let entry = |id: &str, name: &str, categories: &str| {
        std::fs::write(
            apps.join(id),
            format!(
                "[Desktop Entry]\nType=Application\nName={name}\nExec=sh\nCategories={categories}\n"
            ),
        )
        .unwrap();
    };
    for id in HIDDEN_IN_MENUS {
        entry(id, id, "System;Utility;");
    }
    entry(
        "org.example.Querencia.Terminal.desktop",
        "Querencia Terminal",
        "GTK;Utility;TerminalEmulator;",
    );
    entry("vimx.desktop", "Vim X", "Utility;TextEditor;");
    entry(
        "org.codeberg.dnkl.foot.desktop",
        "Foot (andere ID)",
        "Utility;",
    );
    let run = run_child(&[
        ("PATH", "/usr/bin:/bin".to_string()),
        ("HOME", base.join("home").display().to_string()),
        (
            "XDG_DATA_HOME",
            base.join("home-daten").display().to_string(),
        ),
        ("XDG_DATA_DIRS", base.join("daten").display().to_string()),
        ("LANG", "en_US.UTF-8".to_string()),
        ("LANGUAGE", "de".to_string()),
        ("XDG_CURRENT_DESKTOP", "X-Querencia".to_string()),
    ]);
    let _ = std::fs::remove_dir_all(&base);
    let hidden: Vec<&str> = run
        .records("excluded")
        .iter()
        .filter(|f| f[1] == "HiddenInQuerencia")
        .map(|f| f[0])
        .collect();
    assert_eq!(sorted(hidden), sorted(HIDDEN_IN_MENUS.to_vec()));
    assert_eq!(
        sorted(run.entry_ids()),
        [
            "org.codeberg.dnkl.foot.desktop",
            "org.example.Querencia.Terminal.desktop",
            "vimx.desktop"
        ]
    );
    let groups: Vec<&str> = run.records("entry").iter().map(|f| f[1]).collect();
    assert!(groups.iter().all(|g| *g == "Accessories"), "{groups:?}");
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
            "c2-einstellung-oel.desktop",
            "c2-einstellung-papier.desktop",
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
            "Accessories",
            "Normal",
            "utilities-terminal",
            "Utility",
            "false"
        ]
    );
    assert_eq!(field("c2-ohne-kategorie.desktop", 0), "Other");
    assert_eq!(field("c2-ohne-kategorie.desktop", 2), "-");
    assert_eq!(field("c2-ohne-kategorie.desktop", 3), "");
    assert_eq!(field("c2-mehrfach-buero-grafik.desktop", 0), "Graphics");
    assert_eq!(field("c2-mehrfach-netz-medien.desktop", 0), "Sound & Video");
    assert_eq!(field("c2-basis.desktop", 0), "Programming");
    // Utility;Accessibility: Barrierefreiheit, as in GNOME 2
    assert_eq!(field("c2-doppelt.desktop", 0), "Universal Access");
    assert_eq!(field("c2-umlaut.desktop", 1), "Ölwechsel");
    assert_eq!(field("c2-doppelt.desktop", 1), "Doppelt aus daten-home");
    assert_eq!(
        field("c2-icon-fehlt.desktop", 2),
        "classic2-nicht-vorhandenes-icon"
    );
    assert_eq!(field("c2-einstellung.desktop", 0), "Other");
    assert_eq!(field("c2-einstellung.desktop", 4), "true");
    assert_eq!(field("c2-systemeinstellung.desktop", 0), "System Tools");
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

    let rank = |label: &str| ALL_GROUPS.iter().position(|g| g.label() == label);
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
        .filter(|f| f[1] == "Accessories")
        .map(|f| f[2])
        .collect();
    assert_eq!(
        accessories,
        [
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
fn settings_are_split_for_the_system_menu() {
    // Fields: section, ID, name; in menu order. The settings sit in the
    // groups Sonstige and Systemwerkzeuge of the catalog; the system menu
    // sorts them by name across groups, "Ö" next to "O" as in en_US.
    let run = fixture_run(Some("X-Classic2"));
    assert_eq!(
        run.records("system"),
        [
            ["Preferences", "c2-einstellung.desktop", "Einstellung"],
            ["Preferences", "c2-einstellung-oel.desktop", "Ölfarben"],
            [
                "Preferences",
                "c2-einstellung-papier.desktop",
                "Papierformat"
            ],
            [
                "Administration",
                "c2-systemeinstellung.desktop",
                "Systemeinstellung"
            ],
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
    assert_eq!(first.catalog(), second.catalog());
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
    assert_eq!(
        run.catalog(),
        run_child(&env).catalog(),
        "zwei Läufe weichen ab"
    );

    let ids = run.entry_ids();
    let unique: HashSet<_> = ids.iter().collect();
    assert_eq!(unique.len(), ids.len(), "Desktop-ID doppelt im Katalog");
    for id in run.by_id("excluded").keys() {
        assert!(
            !unique.contains(id),
            "{id} zugleich erkannt und ausgeschlossen"
        );
    }

    println!("Anwendungskatalog, XDG_CURRENT_DESKTOP=X-Classic2, LANG=en_US.UTF-8, LANGUAGE=de");
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

// ---- AppInfoMonitor in an isolated child process --------------------
//
// The child gets its own XDG_DATA_HOME and XDG_DATA_DIRS, changes desktop
// files step by step like a package manager would (write elsewhere, then
// rename into place) and reports, per step, how many `changed` signals GIO
// sent and what `Catalog::load` reads afterwards.

const MONITOR_MARKER: &str = "CLASSIC2_MONITOR_CHILD";
const MONITOR_TEST: &str = "apps::tests::monitor_child_process";
/// Upper bound for the first signal; it usually arrives within a second.
const SIGNAL_TIMEOUT: Duration = Duration::from_secs(10);
/// Time to count further signals before the catalog is read again.
const SETTLE: Duration = Duration::from_millis(1500);

/// Runs in the child process only; in a normal test run it does nothing.
#[test]
fn monitor_child_process() {
    if std::env::var_os(MONITOR_MARKER).is_none() {
        return;
    }
    println!();
    let apps = PathBuf::from(std::env::var_os("XDG_DATA_HOME").expect("XDG_DATA_HOME"))
        .join("applications");
    let signals = Rc::new(Cell::new(0u32));
    let monitor = gio::AppInfoMonitor::get();
    monitor.connect_changed(glib::clone!(
        #[strong]
        signals,
        move |_| signals.set(signals.get() + 1)
    ));

    // Reading the catalog is what makes GIO watch the directories.
    report_step("start", 0, None, &Catalog::load());
    let utility = ["Name=Neu", "Categories=Utility;"];
    let renamed = ["Name=Aaa neu", "Categories=Utility;"];
    let steps: [(&str, &dyn Fn()); 11] = [
        ("hinzu", &|| write_entry(&apps, "c2-mon-neu", &utility)),
        ("umbenannt", &|| write_entry(&apps, "c2-mon-neu", &renamed)),
        ("kategorie", &|| {
            write_entry(
                &apps,
                "c2-mon-neu",
                &["Name=Aaa neu", "Categories=Development;"],
            )
        }),
        ("nodisplay", &|| {
            write_entry(&apps, "c2-mon-neu", &["Name=Aaa neu", "NoDisplay=true"])
        }),
        ("onlyshowin-fremd", &|| {
            write_entry(&apps, "c2-mon-neu", &["Name=Aaa neu", "OnlyShowIn=GNOME;"])
        }),
        ("onlyshowin-classic2", &|| {
            write_entry(
                &apps,
                "c2-mon-neu",
                &[
                    "Name=Aaa neu",
                    "Categories=Utility;",
                    "OnlyShowIn=X-Classic2;",
                ],
            )
        }),
        ("zwei", &|| {
            write_entry(&apps, "c2-mon-eins", &utility);
            std::thread::sleep(Duration::from_millis(300));
            write_entry(&apps, "c2-mon-zwei", &utility);
        }),
        ("unterordner", &|| {
            std::fs::create_dir_all(apps.join("c2sub")).expect("subdirectory");
            write_entry(&apps.join("c2sub"), "eintrag", &utility);
        }),
        ("einstellung", &|| {
            write_entry(
                &apps,
                "c2-mon-einst",
                &["Name=Drucker", "Categories=Settings;System;"],
            )
        }),
        ("entfernt", &|| remove_entry(&apps, "c2-mon-neu")),
        ("leer", &|| {
            std::fs::remove_dir_all(apps.join("c2sub")).expect("remove subdirectory");
            for name in ["c2-mon-basis", "c2-mon-eins", "c2-mon-zwei", "c2-mon-einst"] {
                remove_entry(&apps, name);
            }
        }),
    ];
    for (name, change) in steps {
        signals.set(0);
        let started = Instant::now();
        change();
        let first = wait_for(&signals, SIGNAL_TIMEOUT).then(|| started.elapsed());
        keep_running(SETTLE);
        report_step(name, signals.get(), first, &Catalog::load());
    }

    // Without reading in between: does GIO signal a second change?
    signals.set(0);
    write_entry(&apps, "c2-mon-ungelesen-1", &utility);
    let first = wait_for(&signals, SIGNAL_TIMEOUT);
    keep_running(SETTLE);
    let after_first = signals.get();
    write_entry(&apps, "c2-mon-ungelesen-2", &utility);
    keep_running(SETTLE * 2);
    println!(
        "C2\tungelesen\t{first}\t{after_first}\t{}",
        signals.get() - after_first
    );
    report_step("ungelesen-gelesen", signals.get(), None, &Catalog::load());
}

/// Iterates the default main context until a signal arrived or time is up.
fn wait_for(signals: &Cell<u32>, timeout: Duration) -> bool {
    let context = glib::MainContext::default();
    let deadline = Instant::now() + timeout;
    while signals.get() == 0 && Instant::now() < deadline {
        if !context.iteration(false) {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    signals.get() > 0
}

/// Keeps the main context running for `time`, so late signals get counted.
fn keep_running(time: Duration) {
    wait_for(&Cell::new(0), time);
}

fn report_step(name: &str, signals: u32, first: Option<Duration>, catalog: &Catalog) {
    let first = first.map_or("-".to_string(), |d| d.as_millis().to_string());
    let entries: Vec<String> = catalog
        .entries
        .iter()
        .map(|e| {
            let section = e
                .system_section()
                .map_or(String::new(), |s| format!("/{}", s.label()));
            format!("{}={}@{}{section}", e.id, e.name, e.group.label())
        })
        .collect();
    let excluded: Vec<String> = catalog
        .excluded
        .iter()
        .map(|(id, reason)| format!("{id}={reason:?}"))
        .collect();
    println!(
        "C2\tstep\t{name}\t{signals}\t{first}\t{}\t{}",
        entries.join(","),
        excluded.join(",")
    );
}

/// Writes next to the directory and renames into place, as package
/// managers do, so GIO never reads a half-written file.
fn write_entry(dir: &Path, name: &str, lines: &[&str]) {
    let staging = dir.parent().expect("parent").join(format!(".{name}.tmp"));
    let mut text = String::from("[Desktop Entry]\nType=Application\nExec=true\n");
    for line in lines {
        text.push_str(line);
        text.push('\n');
    }
    std::fs::write(&staging, text).expect("write desktop entry");
    std::fs::rename(&staging, dir.join(format!("{name}.desktop"))).expect("rename into place");
}

fn remove_entry(dir: &Path, name: &str) {
    std::fs::remove_file(dir.join(format!("{name}.desktop"))).expect("remove desktop entry");
}

/// A fresh, empty directory below the system temp directory.
fn scratch_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("clock after 1970")
        .as_nanos();
    let dir =
        std::env::temp_dir().join(format!("querencia-{label}-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch directory");
    dir
}

#[test]
fn monitor_reports_installed_removed_and_changed_applications() {
    let base = scratch_dir("monitor");
    let home = base.join("home");
    let system = base.join("system");
    std::fs::create_dir_all(home.join("applications")).expect("applications");
    std::fs::create_dir_all(&system).expect("system data dir");
    write_entry(
        &home.join("applications"),
        "c2-mon-basis",
        &["Name=Basis", "Categories=Utility;"],
    );

    let output = Command::new(std::env::current_exe().expect("test binary path"))
        .args(["--exact", MONITOR_TEST, "--nocapture", "--test-threads=1"])
        .env_clear()
        .env(MONITOR_MARKER, "1")
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", base.join("kein-home"))
        .env("XDG_DATA_HOME", &home)
        .env("XDG_DATA_DIRS", &system)
        .env("XDG_CURRENT_DESKTOP", "X-Classic2")
        .env("LANG", "en_US.UTF-8")
        .output()
        .expect("child test process starts");
    std::fs::remove_dir_all(&base).expect("remove scratch directory");
    let run = ChildRun {
        stdout: String::from_utf8(output.stdout).expect("UTF-8 output"),
    };
    assert!(
        output.status.success(),
        "Kindprozess fehlgeschlagen:\n{}\n{}",
        run.stdout,
        String::from_utf8_lossy(&output.stderr)
    );
    println!("{}", run.stdout);

    // Fields after the step name: signals, ms to the first, entries, excluded.
    let steps = run.by_id("step");
    let entries = |step: &str| steps[step][2];
    let excluded = |step: &str| steps[step][3];
    for (step, fields) in &steps {
        if !["start", "ungelesen-gelesen"].contains(step) {
            assert_ne!(fields[0], "0", "kein changed-Signal nach Schritt {step}");
        }
    }
    let basis = "c2-mon-basis.desktop=Basis@Accessories";
    let neu = "c2-mon-neu.desktop=Aaa neu@Accessories";
    assert_eq!(entries("start"), basis);
    assert_eq!(
        entries("hinzu"),
        format!("{basis},c2-mon-neu.desktop=Neu@Accessories")
    );
    // The new name sorts it before Basis.
    assert_eq!(entries("umbenannt"), format!("{neu},{basis}"));
    assert_eq!(
        entries("kategorie"),
        format!("{basis},c2-mon-neu.desktop=Aaa neu@Programming")
    );
    assert_eq!(entries("nodisplay"), basis);
    assert_eq!(excluded("nodisplay"), "c2-mon-neu.desktop=NoDisplay");
    assert_eq!(entries("onlyshowin-fremd"), basis);
    assert_eq!(
        excluded("onlyshowin-fremd"),
        "c2-mon-neu.desktop=NotShownInDesktop"
    );
    assert_eq!(entries("onlyshowin-classic2"), format!("{neu},{basis}"));
    assert_eq!(
        entries("zwei"),
        format!(
            "{neu},{basis},c2-mon-eins.desktop=Neu@Accessories,c2-mon-zwei.desktop=Neu@Accessories"
        )
    );
    assert!(entries("unterordner").contains("c2sub-eintrag.desktop=Neu@Accessories"));
    // A new setting reaches the catalog with its section of the system menu.
    assert!(
        entries("einstellung").contains("c2-mon-einst.desktop=Drucker@System Tools/Administration")
    );
    assert!(!entries("entfernt").contains("c2-mon-neu"));
    assert_eq!(entries("leer"), "");
    assert_eq!(excluded("leer"), "");
    // Both unread changes are in the catalog once it is read again.
    assert_eq!(
        entries("ungelesen-gelesen"),
        "c2-mon-ungelesen-1.desktop=Neu@Accessories,c2-mon-ungelesen-2.desktop=Neu@Accessories"
    );
}
