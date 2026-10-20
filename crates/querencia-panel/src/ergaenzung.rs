//! G2-AL: puts „Arbeitsfläche anzeigen“ and the notification bell into an
//! existing `panel.ini`, and takes them out again (fallback of the
//! integration). The standard layout has both; a stored layout made before
//! G2-AL gets them only through this step, because the panel never adds
//! applets to the user's file on its own.
//!
//! - `show-desktop-1` goes right before the window list (as in GNOME 2 at
//!   the left end of the bottom panel);
//! - `benachrichtigungen-1` right before the clock (`End=…;network-1;
//!   benachrichtigungen-1;clock-1`), without a clock at the end of the top
//!   panel;
//! - a type that is already there is left as it is, so the step can run
//!   twice; an ID already taken gets the next number.
//!
//! The file is changed as a GLib key file with its comments and every
//! other group and key kept, not rewritten from the layout.
//! `querencia-panel --panel-ini ergaenzen|zuruecknehmen|pruefen <datei>`
//! runs it before GTK starts; the integration script keeps a copy first.

use std::path::Path;

use gtk4::glib;

use crate::applets::{NOTIFICATIONS, SHOW_DESKTOP};

/// The result of a change: the new file and one line per decision.
#[derive(Debug, PartialEq, Eq)]
pub struct Ergebnis {
    pub text: String,
    pub geaendert: bool,
    pub meldungen: Vec<String>,
}

const REGIONS: [&str; 3] = ["Start", "Center", "End"];

fn split(list: &str) -> Vec<String> {
    list.split(';')
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The key file as text, ending with one line break as the panel's own
/// file does (GLib adds an empty line after the last group).
fn text_of(file: &glib::KeyFile) -> String {
    let mut text = file.to_data().to_string();
    while text.ends_with("\n\n") {
        text.pop();
    }
    text
}

fn read(text: &str) -> Result<glib::KeyFile, String> {
    let file = glib::KeyFile::new();
    file.load_from_data(
        text,
        glib::KeyFileFlags::KEEP_COMMENTS | glib::KeyFileFlags::KEEP_TRANSLATIONS,
    )
    .map_err(|e| format!("panel.ini nicht lesbar: {e}"))?;
    Ok(file)
}

/// The panels of `[Layout] Panels`, in order.
fn panels(file: &glib::KeyFile) -> Result<Vec<String>, String> {
    file.string("Layout", "Panels")
        .map(|list| split(&list))
        .map_err(|e| format!("[Layout] Panels fehlt: {e}"))
}

fn applet_type(file: &glib::KeyFile, id: &str) -> Option<String> {
    file.string(&format!("Applet {id}"), "Type")
        .ok()
        .map(|t| t.to_string())
}

/// One region of a panel: its group, its key and its instance IDs.
type Bereich = (String, &'static str, Vec<String>);

/// Every region of the layout's panels, in order.
fn regions(file: &glib::KeyFile) -> Result<Vec<Bereich>, String> {
    let mut list = Vec::new();
    for panel in panels(file)? {
        let group = format!("Panel {panel}");
        for key in REGIONS {
            let ids = file
                .string(&group, key)
                .map(|s| split(&s))
                .unwrap_or_default();
            list.push((group.clone(), key, ids));
        }
    }
    Ok(list)
}

/// Whether an instance of `kind` stands in a panel.
fn shown(file: &glib::KeyFile, kind: &str) -> Result<bool, String> {
    Ok(regions(file)?
        .iter()
        .flat_map(|(_, _, ids)| ids)
        .any(|id| applet_type(file, id).as_deref() == Some(kind)))
}

/// `<kind>-<n>` with the smallest `n` no instance or applet group uses.
fn free_id(file: &glib::KeyFile, kind: &str) -> Result<String, String> {
    let used: Vec<String> = regions(file)?
        .into_iter()
        .flat_map(|(_, _, ids)| ids)
        .collect();
    let groups: Vec<String> = file.groups().iter().map(|g| g.to_string()).collect();
    let id = (1..)
        .map(|n| format!("{kind}-{n}"))
        .find(|id| !used.contains(id) && !groups.contains(&format!("Applet {id}")))
        .expect("eine freie Nummer gibt es immer");
    Ok(id)
}

/// Puts a new instance of `kind` right before the first instance of
/// `before` (in the region holding it); `None` if there is none.
fn insert_before(
    file: &glib::KeyFile,
    kind: &str,
    before: &str,
) -> Result<Option<(String, String)>, String> {
    for (group, key, mut ids) in regions(file)? {
        let Some(position) = ids
            .iter()
            .position(|id| applet_type(file, id).as_deref() == Some(before))
        else {
            continue;
        };
        let id = free_id(file, kind)?;
        ids.insert(position, id.clone());
        file.set_string(&group, key, &ids.join(";"));
        file.set_string(&format!("Applet {id}"), "Type", kind);
        return Ok(Some((id, format!("[{group}] {key}={}", ids.join(";")))));
    }
    Ok(None)
}

/// Appends a new instance of `kind` to the end region of the first panel
/// at the top edge; `None` without one.
fn append_top_end(file: &glib::KeyFile, kind: &str) -> Result<Option<(String, String)>, String> {
    for panel in panels(file)? {
        let group = format!("Panel {panel}");
        if file.string(&group, "Edge").ok().as_deref() != Some("top") {
            continue;
        }
        let mut ids = file
            .string(&group, "End")
            .map(|s| split(&s))
            .unwrap_or_default();
        let id = free_id(file, kind)?;
        ids.push(id.clone());
        file.set_string(&group, "End", &ids.join(";"));
        file.set_string(&format!("Applet {id}"), "Type", kind);
        return Ok(Some((id, format!("[{group}] End={}", ids.join(";")))));
    }
    Ok(None)
}

/// Adds both applets where they are missing.
pub fn ergaenzen(text: &str) -> Result<Ergebnis, String> {
    let file = read(text)?;
    let mut meldungen = Vec::new();
    let mut geaendert = false;

    if shown(&file, SHOW_DESKTOP)? {
        meldungen.push(format!("{SHOW_DESKTOP}: schon im Panel, unverändert"));
    } else {
        match insert_before(&file, SHOW_DESKTOP, "window-list")? {
            Some((id, line)) => {
                geaendert = true;
                meldungen.push(format!("{id} vor der Fensterliste: {line}"));
            }
            None => meldungen.push(format!(
                "{SHOW_DESKTOP}: keine Fensterliste im Layout, nicht ergänzt"
            )),
        }
    }

    if shown(&file, NOTIFICATIONS)? {
        meldungen.push(format!("{NOTIFICATIONS}: schon im Panel, unverändert"));
    } else {
        let placed = match insert_before(&file, NOTIFICATIONS, "clock")? {
            Some((id, line)) => Some(format!("{id} vor der Uhr: {line}")),
            None => append_top_end(&file, NOTIFICATIONS)?
                .map(|(id, line)| format!("{id} am Ende des oberen Panels: {line}")),
        };
        match placed {
            Some(line) => {
                geaendert = true;
                meldungen.push(line);
            }
            None => meldungen.push(format!(
                "{NOTIFICATIONS}: weder Uhr noch oberes Panel, nicht ergänzt"
            )),
        }
    }

    Ok(Ergebnis {
        text: if geaendert {
            text_of(&file)
        } else {
            text.to_owned()
        },
        geaendert,
        meldungen,
    })
}

/// Takes every instance of both types out of the panels and drops their
/// applet groups (an older panel does not know them).
pub fn zuruecknehmen(text: &str) -> Result<Ergebnis, String> {
    let file = read(text)?;
    let mut meldungen = Vec::new();
    let mut weg = Vec::new();
    for (group, key, ids) in regions(&file)? {
        let (raus, bleiben): (Vec<String>, Vec<String>) = ids.into_iter().partition(|id| {
            matches!(
                applet_type(&file, id).as_deref(),
                Some(SHOW_DESKTOP | NOTIFICATIONS)
            )
        });
        if raus.is_empty() {
            continue;
        }
        file.set_string(&group, key, &bleiben.join(";"));
        meldungen.push(format!(
            "{} entfernt: [{group}] {key}={}",
            raus.join(", "),
            bleiben.join(";")
        ));
        weg.extend(raus);
    }
    for group in file.groups().iter().map(|g| g.to_string()) {
        let Some(id) = group.strip_prefix("Applet ") else {
            continue;
        };
        if matches!(
            applet_type(&file, id).as_deref(),
            Some(SHOW_DESKTOP | NOTIFICATIONS)
        ) {
            file.remove_group(&group)
                .map_err(|e| format!("[{group}]: {e}"))?;
            if !weg.contains(&id.to_owned()) {
                meldungen.push(format!("[{group}] ohne Panel entfernt"));
            }
            weg.push(id.to_owned());
        }
    }
    let geaendert = !weg.is_empty();
    if !geaendert {
        meldungen.push(format!(
            "weder {SHOW_DESKTOP} noch {NOTIFICATIONS}, unverändert"
        ));
    }
    Ok(Ergebnis {
        text: if geaendert {
            text_of(&file)
        } else {
            text.to_owned()
        },
        geaendert,
        meldungen,
    })
}

/// `--panel-ini <befehl> <datei>`: `ergaenzen` and `zuruecknehmen` change
/// the file atomically (only if needed), `pruefen` only reads; all three
/// then print what the panel would show. Exit 0 done, 1 error, 2 wrong use.
pub fn kommandozeile(args: &[String]) -> u8 {
    let (Some(befehl), Some(datei), None) = (args.first(), args.get(1), args.get(2)) else {
        eprintln!("Aufruf: querencia-panel --panel-ini ergaenzen|zuruecknehmen|pruefen <datei>");
        return 2;
    };
    let path = Path::new(datei);
    let aendern: fn(&str) -> Result<Ergebnis, String> = match befehl.as_str() {
        "ergaenzen" => ergaenzen,
        "zuruecknehmen" => zuruecknehmen,
        "pruefen" => |text| {
            Ok(Ergebnis {
                text: text.to_owned(),
                geaendert: false,
                meldungen: Vec::new(),
            })
        },
        _ => {
            eprintln!("unbekannter Befehl {befehl}");
            return 2;
        }
    };
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) => {
            eprintln!("{}: {err}", path.display());
            return 1;
        }
    };
    let ergebnis = match aendern(&text) {
        Ok(ergebnis) => ergebnis,
        Err(err) => {
            eprintln!("{}: {err}", path.display());
            return 1;
        }
    };
    for meldung in &ergebnis.meldungen {
        println!("{meldung}");
    }
    if ergebnis.geaendert {
        // atomically, as the panel saves (layout_store::save); an existing
        // file keeps its permissions
        if let Err(err) = glib::file_set_contents_full(
            path,
            ergebnis.text.as_bytes(),
            glib::FileSetContentsFlags::CONSISTENT | glib::FileSetContentsFlags::DURABLE,
            0o666,
        ) {
            eprintln!("{}: {err}", path.display());
            return 1;
        }
        println!("geschrieben: {}", path.display());
    }
    let loaded = crate::layout_store::load_stored(path, crate::layout_editor::registry);
    for problem in &loaded.problems {
        println!("Problem: {problem}");
    }
    for panel in &loaded.layout.panels {
        let ids = |region| {
            panel
                .region(region)
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>()
                .join(";")
        };
        println!(
            "Panel {}: Start={} Center={} End={}",
            panel.id,
            ids(crate::layout::Region::Start),
            ids(crate::layout::Region::Center),
            ids(crate::layout::Region::End)
        );
    }
    println!("Layout {}", loaded.source);
    u8::from(!loaded.problems.is_empty() || loaded.source == crate::layout_store::Source::Fallback)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The live `panel.ini` of 20.10.2026 (G2-M layout), with a comment.
    const LIVE: &str = "# eigene Notiz\n[Layout]\nVersion=1\nPanels=top;bottom\n\n[Panel top]\n\
        Edge=top\nHeight=32\nStart=applications-1;places-1;system-1\nCenter=\n\
        End=volume-1;network-1;clock-1\n\n[Applet applications-1]\nType=applications\n\n\
        [Applet places-1]\nType=places\n\n[Applet system-1]\nType=system\n\n\
        [Applet volume-1]\nType=volume\n\n[Applet network-1]\nType=network\n\n\
        [Applet clock-1]\nType=clock\n\n[Panel bottom]\nEdge=bottom\nHeight=32\n\
        Start=window-list-1\nCenter=\nEnd=workspace-switcher-1\n\n\
        [Applet window-list-1]\nType=window-list\n\n\
        [Applet workspace-switcher-1]\nType=workspace-switcher\n";

    fn get(text: &str, group: &str, key: &str) -> String {
        read(text).unwrap().string(group, key).unwrap().to_string()
    }

    fn load(text: &str) -> crate::layout_store::Loaded {
        let dir = std::env::temp_dir().join(format!(
            "g2al-ergaenzung-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("panel.ini");
        std::fs::write(&path, text).unwrap();
        let loaded = crate::layout_store::load_stored(&path, crate::layout_editor::registry);
        std::fs::remove_dir_all(&dir).unwrap();
        loaded
    }

    #[test]
    fn live_layout_gets_both_applets_in_their_places() {
        let neu = ergaenzen(LIVE).unwrap();
        assert!(neu.geaendert);
        assert_eq!(
            get(&neu.text, "Panel bottom", "Start"),
            "show-desktop-1;window-list-1"
        );
        assert_eq!(
            get(&neu.text, "Panel top", "End"),
            "volume-1;network-1;benachrichtigungen-1;clock-1"
        );
        assert_eq!(
            get(&neu.text, "Applet show-desktop-1", "Type"),
            "show-desktop"
        );
        assert_eq!(
            get(&neu.text, "Applet benachrichtigungen-1", "Type"),
            "benachrichtigungen"
        );
        // everything else stays, the comment too
        assert!(neu.text.starts_with("# eigene Notiz\n"), "{}", neu.text);
        assert_eq!(
            get(&neu.text, "Panel top", "Start"),
            "applications-1;places-1;system-1"
        );
        assert_eq!(
            get(&neu.text, "Panel bottom", "End"),
            "workspace-switcher-1"
        );
        assert_eq!(neu.meldungen.len(), 2, "{:?}", neu.meldungen);
        // the panel reads it without a problem
        let loaded = load(&neu.text);
        assert!(loaded.problems.is_empty(), "{:?}", loaded.problems);
        let types: Vec<_> = loaded.layout.panels[0]
            .region(crate::layout::Region::End)
            .iter()
            .map(|i| i.applet.as_str())
            .collect();
        assert_eq!(types, ["volume", "network", "benachrichtigungen", "clock"]);
    }

    #[test]
    fn a_second_run_changes_nothing() {
        let einmal = ergaenzen(LIVE).unwrap();
        let zweimal = ergaenzen(&einmal.text).unwrap();
        assert!(!zweimal.geaendert);
        assert_eq!(zweimal.text, einmal.text);
        assert!(
            zweimal
                .meldungen
                .iter()
                .all(|m| m.contains("schon im Panel")),
            "{:?}",
            zweimal.meldungen
        );
    }

    #[test]
    fn fallback_takes_out_exactly_what_was_added() {
        let neu = ergaenzen(LIVE).unwrap();
        let alt = zuruecknehmen(&neu.text).unwrap();
        assert!(alt.geaendert);
        assert_eq!(get(&alt.text, "Panel bottom", "Start"), "window-list-1");
        assert_eq!(
            get(&alt.text, "Panel top", "End"),
            "volume-1;network-1;clock-1"
        );
        assert!(!read(&alt.text).unwrap().has_group("Applet show-desktop-1"));
        assert!(
            !read(&alt.text)
                .unwrap()
                .has_group("Applet benachrichtigungen-1")
        );
        // the same file as before the change, byte for byte
        assert_eq!(alt.text, LIVE);
        assert_eq!(load(&alt.text).layout, load(LIVE).layout);
        // nothing left to take out
        let nochmal = zuruecknehmen(&alt.text).unwrap();
        assert!(!nochmal.geaendert);
        assert_eq!(nochmal.text, alt.text);
    }

    #[test]
    fn without_volume_and_network_the_bell_still_goes_before_the_clock() {
        let text = LIVE.replace("End=volume-1;network-1;clock-1", "End=clock-1");
        let neu = ergaenzen(&text).unwrap();
        assert_eq!(
            get(&neu.text, "Panel top", "End"),
            "benachrichtigungen-1;clock-1"
        );
    }

    #[test]
    fn without_a_clock_the_bell_goes_to_the_end_of_the_top_panel() {
        let text = LIVE.replace("End=volume-1;network-1;clock-1", "End=volume-1");
        let neu = ergaenzen(&text).unwrap();
        assert_eq!(
            get(&neu.text, "Panel top", "End"),
            "volume-1;benachrichtigungen-1"
        );
        // neither clock nor top panel: left out with a message
        let text = "[Layout]\nVersion=1\nPanels=bottom\n\n[Panel bottom]\nEdge=bottom\n\
            Start=window-list-1\n\n[Applet window-list-1]\nType=window-list\n";
        let neu = ergaenzen(text).unwrap();
        assert_eq!(
            get(&neu.text, "Panel bottom", "Start"),
            "show-desktop-1;window-list-1"
        );
        assert!(
            neu.meldungen.iter().any(|m| m.contains("nicht ergänzt")),
            "{:?}",
            neu.meldungen
        );
    }

    #[test]
    fn without_a_window_list_show_desktop_is_left_out() {
        let text = LIVE.replace("Start=window-list-1", "Start=");
        let neu = ergaenzen(&text).unwrap();
        assert!(!read(&neu.text).unwrap().has_group("Applet show-desktop-1"));
        assert!(
            neu.meldungen[0].contains("keine Fensterliste"),
            "{:?}",
            neu.meldungen
        );
    }

    #[test]
    fn taken_ids_get_the_next_number_and_a_moved_bell_stays() {
        // a stray group of the name, not in any panel
        let text = format!("{LIVE}\n[Applet show-desktop-1]\nType=launcher\n");
        let neu = ergaenzen(&text).unwrap();
        assert_eq!(
            get(&neu.text, "Panel bottom", "Start"),
            "show-desktop-2;window-list-1"
        );
        // the user already put the bell elsewhere: left there
        let text = LIVE.replace(
            "Start=applications-1;places-1;system-1",
            "Start=applications-1;places-1;system-1;b-7",
        ) + "\n[Applet b-7]\nType=benachrichtigungen\n";
        let neu = ergaenzen(&text).unwrap();
        assert_eq!(
            get(&neu.text, "Panel top", "End"),
            "volume-1;network-1;clock-1"
        );
        // and the fallback takes it out wherever it is
        let alt = zuruecknehmen(&neu.text).unwrap();
        assert_eq!(
            get(&alt.text, "Panel top", "Start"),
            "applications-1;places-1;system-1"
        );
    }

    #[test]
    fn unusable_files_are_refused() {
        assert!(ergaenzen("kein ini").is_err());
        assert!(ergaenzen("[Layout]\nVersion=1\n").is_err());
        assert!(zuruecknehmen("[x\n").is_err());
    }

    #[test]
    fn wrong_use_of_the_command_line() {
        assert_eq!(kommandozeile(&[]), 2);
        assert_eq!(kommandozeile(&["ergaenzen".into()]), 2);
        assert_eq!(kommandozeile(&["loeschen".into(), "/x".into()]), 2);
        assert_eq!(
            kommandozeile(&["pruefen".into(), "/nicht/da/panel.ini".into()]),
            1
        );
    }
}
