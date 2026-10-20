//! Reading `panel.ini` again while the panel runs (G2-AK, QD-046 ⑥), so
//! that Querencia Einstellungen can change the panels without a restart.
//!
//! Two ways in, one rule:
//! - a file monitor on `panel.ini`; changes are collected for
//!   [`ENTPRELLEN_MS`] (an editor or a settings program may write in
//!   several steps), then the file is read once;
//! - the D-Bus method `org.querencia.Panel.NeuLaden` ([`INTERFACE`]) on the
//!   panel's bus name and object path (`org.example.Querencia.Panel`,
//!   `/org/example/Querencia/Panel`), which reads at once and answers
//!   `(b geladen, s meldung)`.
//!
//! Reading again gives what a restart would show ([`Editor::load`]): no
//! file is the standard layout, single bad entries are dropped with a log
//! line. Only an unusable file (not readable, not a key file, no panels) is
//! refused: the panels keep the layout they show and the log says why. A
//! file that holds exactly what the panel last read or wrote (its own
//! "Zum Panel hinzufügen") changes nothing.
//!
//! A panel whose description is unchanged stays as it is, with its open
//! menus; a changed, new or removed panel is built again or taken away
//! ([`plan`]). A changed window list setting rebuilds the panels that hold
//! a window list. One log line per reload says what was done.

use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, gio, glib};

use crate::layout::{Layout, PanelSpec};
use crate::layout_editor::{self, Editor};
use crate::layout_store::{Disk, Source};
use crate::{fensterliste, panel};

/// D-Bus interface of the reload method, on the panel's object path.
pub const INTERFACE: &str = "org.querencia.Panel";
pub const METHOD: &str = "NeuLaden";
/// Quiet time after the last change of `panel.ini` before it is read.
pub const ENTPRELLEN_MS: u32 = 250;

const INTROSPECTION: &str = r#"<node>
  <interface name="org.querencia.Panel">
    <method name="NeuLaden">
      <arg type="b" name="geladen" direction="out"/>
      <arg type="s" name="meldung" direction="out"/>
    </method>
  </interface>
</node>"#;

/// Applet type of the window list, whose panels follow its setting.
const WINDOW_LIST: &str = "window-list";

/// What reading the file again found.
#[derive(Debug, PartialEq, Eq)]
pub enum Pruefung {
    /// The file holds what the panel last read or wrote.
    Unveraendert,
    /// A layout to show.
    Neu,
    /// Unusable; the panels stay as they are.
    Fehler(String),
}

/// Compares the file just read (`neu`, with its `source` and `problems`)
/// with what the panel last read or wrote (`alt`).
pub fn pruefen(alt: &Disk, neu: &Disk, source: &Source, problems: &[String]) -> Pruefung {
    if neu == alt {
        return Pruefung::Unveraendert;
    }
    match (source, neu) {
        (Source::Fallback, _) => Pruefung::Fehler(if problems.is_empty() {
            "panel.ini unbrauchbar".to_owned()
        } else {
            problems.join("; ")
        }),
        _ => Pruefung::Neu,
    }
}

/// Which panels a new layout keeps, builds again or takes away; panel IDs.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Plan {
    /// Shown and unchanged.
    pub behalten: Vec<String>,
    /// Shown and changed or gone: their windows go.
    pub weg: Vec<String>,
    /// To build, in the new layout's order (changed and new ones).
    pub bauen: Vec<String>,
}

/// The [`Plan`] from the shown layout `alt` to `neu`: a panel stays if its
/// description is the same and, if it holds a window list, the window
/// list setting is too.
pub fn plan(alt: &Layout, neu: &Layout) -> Plan {
    let window_list_changed = alt.window_list.all_workspaces() != neu.window_list.all_workspaces();
    let follows = |spec: &PanelSpec| spec.instances().any(|i| i.applet == WINDOW_LIST);
    let mut result = Plan::default();
    for old in &alt.panels {
        let same = neu
            .panels
            .iter()
            .any(|new| new == old && !(window_list_changed && follows(old)));
        if same {
            result.behalten.push(old.id.clone());
        } else {
            result.weg.push(old.id.clone());
        }
    }
    result.bauen = neu
        .panels
        .iter()
        .filter(|new| !result.behalten.contains(&new.id))
        .map(|new| new.id.clone())
        .collect();
    result
}

/// The panels on screen and how to change them.
struct Panels {
    app: glib::WeakRef<Application>,
    editor: Rc<RefCell<Editor>>,
    windows: Vec<(String, glib::WeakRef<ApplicationWindow>)>,
    /// Keeps the file monitor alive.
    monitor: Option<gio::FileMonitor>,
    pending: Option<glib::SourceId>,
}

type Shared = Rc<RefCell<Panels>>;

/// Shows the panels of `editor`'s layout and from now on follows changes
/// of `panel.ini` (file monitor and D-Bus).
pub fn start(app: &Application, editor: Rc<RefCell<Editor>>) {
    let layout = editor.borrow().layout().clone();
    fensterliste::set_alle_arbeitsflaechen(layout.window_list.all_workspaces());
    let mut windows = Vec::new();
    for spec in &layout.panels {
        let window = panel::build(app, spec, &editor);
        window.present();
        windows.push((spec.id.clone(), window.downgrade()));
    }
    let path = editor.borrow().path().to_path_buf();
    let panels = Rc::new(RefCell::new(Panels {
        app: app.downgrade(),
        editor,
        windows,
        monitor: None,
        pending: None,
    }));
    watch(&panels, &path);
    export(app, &panels);
}

/// Reloads [`ENTPRELLEN_MS`] after the last change of `path`.
fn watch(panels: &Shared, path: &std::path::Path) {
    let file = gio::File::for_path(path);
    let monitor = match file.monitor_file(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE) {
        Ok(monitor) => monitor,
        Err(err) => {
            eprintln!(
                "querencia-panel: Neuladen: {} wird nicht beobachtet: {err}",
                path.display()
            );
            return;
        }
    };
    let weak = Rc::downgrade(panels);
    monitor.connect_changed(move |_, _, _, event| {
        if matches!(
            event,
            gio::FileMonitorEvent::AttributeChanged
                | gio::FileMonitorEvent::PreUnmount
                | gio::FileMonitorEvent::Unmounted
        ) {
            return;
        }
        let Some(panels) = weak.upgrade() else { return };
        let mut state = panels.borrow_mut();
        if let Some(old) = state.pending.take() {
            old.remove();
        }
        let weak = Rc::downgrade(&panels);
        state.pending = Some(glib::timeout_add_local_once(
            std::time::Duration::from_millis(ENTPRELLEN_MS.into()),
            move || {
                if let Some(panels) = weak.upgrade() {
                    panels.borrow_mut().pending = None;
                    reload(&panels, "Dateiänderung");
                }
            },
        ));
    });
    eprintln!("querencia-panel: Neuladen: beobachte {}", path.display());
    panels.borrow_mut().monitor = Some(monitor);
}

/// Offers [`METHOD`] on the application's D-Bus connection.
fn export(app: &Application, panels: &Shared) {
    let (Some(connection), Some(object_path)) = (app.dbus_connection(), app.dbus_object_path())
    else {
        eprintln!("querencia-panel: Neuladen: keine D-Bus-Verbindung, nur Dateibeobachtung");
        return;
    };
    let info = match gio::DBusNodeInfo::for_xml(INTROSPECTION) {
        Ok(node) => node.lookup_interface(INTERFACE),
        Err(err) => {
            eprintln!("querencia-panel: Neuladen: Schnittstelle ungültig: {err}");
            return;
        }
    };
    let Some(info) = info else { return };
    let weak = Rc::downgrade(panels);
    let registered = connection
        .register_object(&object_path, &info)
        .method_call(move |_, sender, _, _, method, _, invocation| {
            let Some(panels) = weak.upgrade().filter(|_| method == METHOD) else {
                invocation.return_error(
                    gio::DBusError::UnknownMethod,
                    &format!("{method} gibt es nicht"),
                );
                return;
            };
            if let Some(pending) = panels.borrow_mut().pending.take() {
                pending.remove();
            }
            let why = format!("D-Bus {}", sender.unwrap_or("?"));
            let (loaded, message) = reload(&panels, &why);
            invocation.return_value(Some(&(loaded, message).to_variant()));
        })
        .build();
    match registered {
        Ok(_) => {
            eprintln!("querencia-panel: Neuladen: D-Bus {INTERFACE}.{METHOD} auf {object_path}")
        }
        Err(err) => eprintln!("querencia-panel: Neuladen: D-Bus nicht angeboten: {err}"),
    }
}

/// Reads `panel.ini` again and shows its layout; `why` names the trigger
/// for the log. Returns whether the panels show the file now, and what was
/// done (the D-Bus answer).
fn reload(panels: &Shared, why: &str) -> (bool, String) {
    let (path, alt, alt_disk) = {
        let state = panels.borrow();
        let editor = state.editor.borrow();
        (
            editor.path().to_path_buf(),
            editor.layout().clone(),
            editor.disk().clone(),
        )
    };
    let (editor, problems, source) = Editor::load(path.clone(), layout_editor::registry);
    let (loaded, message) = match pruefen(&alt_disk, editor.disk(), &source, &problems) {
        Pruefung::Unveraendert => (true, "panel.ini unverändert".to_owned()),
        Pruefung::Fehler(err) => (
            false,
            format!("panel.ini fehlerhaft, die Panels bleiben wie sie sind: {err}"),
        ),
        Pruefung::Neu => {
            for problem in &problems {
                eprintln!("querencia-panel: {}: {problem}", path.display());
            }
            let neu = editor.layout().clone();
            let plan = plan(&alt, &neu);
            apply(panels, editor, &neu, &plan);
            (true, describe(&source, &neu, &plan))
        }
    };
    eprintln!("querencia-panel: Neuladen ({why}): {message}");
    (loaded, message)
}

/// Puts `editor` in place and changes the panels by `plan`.
fn apply(panels: &Shared, editor: Editor, neu: &Layout, plan: &Plan) {
    let (app, shared_editor) = {
        let state = panels.borrow();
        (state.app.upgrade(), state.editor.clone())
    };
    // every reference to the editor (panel menus, moving) sees the new one
    *shared_editor.borrow_mut() = editor;
    fensterliste::set_alle_arbeitsflaechen(neu.window_list.all_workspaces());
    let Some(app) = app else { return };
    // the application must not end while it has no panel window
    let _hold = app.hold();
    let mut state = panels.borrow_mut();
    state.windows.retain(|(id, window)| {
        if plan.behalten.contains(id) {
            return true;
        }
        if let Some(window) = window.upgrade() {
            window.destroy();
        }
        false
    });
    for id in &plan.bauen {
        let Some(spec) = neu.panels.iter().find(|p| &p.id == id) else {
            continue;
        };
        let window = panel::build(&app, spec, &shared_editor);
        window.present();
        state.windows.push((id.clone(), window.downgrade()));
    }
}

/// The log line and D-Bus answer for a layout put on screen.
fn describe(source: &Source, neu: &Layout, plan: &Plan) -> String {
    let list = |ids: &[String]| {
        if ids.is_empty() {
            "–".to_owned()
        } else {
            ids.join(", ")
        }
    };
    let removed: Vec<String> = plan
        .weg
        .iter()
        .filter(|id| !plan.bauen.contains(id))
        .cloned()
        .collect();
    format!(
        "Layout {source}, {} Panel(s); neu aufgebaut: {}; unverändert: {}; entfernt: {}; \
         Fensterliste {}",
        neu.panels.len(),
        list(&plan.bauen),
        list(&plan.behalten),
        list(&removed),
        if neu.window_list.all_workspaces() {
            "alle Arbeitsflächen"
        } else {
            "nur aktuelle Arbeitsfläche"
        }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{Edge, Instance, Region, WindowListSettings};

    fn bytes(text: &str) -> Disk {
        Disk::Content(text.as_bytes().to_vec())
    }

    #[test]
    fn own_writes_change_nothing_and_unusable_files_are_refused() {
        let alt = bytes("[Layout]\nVersion=1\n");
        assert_eq!(
            pruefen(&alt, &alt.clone(), &Source::File, &[]),
            Pruefung::Unveraendert
        );
        assert_eq!(
            pruefen(&alt, &bytes("anders"), &Source::File, &[]),
            Pruefung::Neu
        );
        // single dropped entries: shown like after a restart
        assert_eq!(
            pruefen(&alt, &bytes("anders"), &Source::File, &["x".into()]),
            Pruefung::Neu
        );
        // unusable: the old layout stays, with the reasons
        assert_eq!(
            pruefen(
                &alt,
                &bytes("kaputt"),
                &Source::Fallback,
                &["nicht lesbar".into(), "kein Panel".into()]
            ),
            Pruefung::Fehler("nicht lesbar; kein Panel".into())
        );
        assert_eq!(
            pruefen(&alt, &Disk::Unreadable("x".into()), &Source::Fallback, &[]),
            Pruefung::Fehler("panel.ini unbrauchbar".into())
        );
        // removed: the standard layout, as after a restart
        assert_eq!(
            pruefen(&alt, &Disk::Missing, &Source::Standard, &[]),
            Pruefung::Neu
        );
        assert_eq!(
            pruefen(&Disk::Missing, &Disk::Missing, &Source::Standard, &[]),
            Pruefung::Unveraendert
        );
    }

    fn ids(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn only_changed_panels_are_built_again() {
        let alt = Layout::standard();
        assert_eq!(
            plan(&alt, &alt.clone()),
            Plan {
                behalten: ids(&["top", "bottom"]),
                weg: vec![],
                bauen: vec![],
            }
        );
        // one applet more on top
        let mut neu = alt.clone();
        neu.panels[0].push(Region::End, Instance::new("places-2", "places"));
        assert_eq!(
            plan(&alt, &neu),
            Plan {
                behalten: ids(&["bottom"]),
                weg: ids(&["top"]),
                bauen: ids(&["top"]),
            }
        );
        // the bottom panel replaced by another one on the same edge
        let mut neu = alt.clone();
        neu.panels.remove(1);
        let mut side = PanelSpec::new("unten2", Edge::Bottom, 24);
        side.push(Region::Start, Instance::new("clock-2", "clock"));
        neu.panels.push(side);
        assert_eq!(
            plan(&alt, &neu),
            Plan {
                behalten: ids(&["top"]),
                weg: ids(&["bottom"]),
                bauen: ids(&["unten2"]),
            }
        );
        // another height is another panel
        let mut neu = alt.clone();
        neu.panels[1].height = 24;
        assert_eq!(plan(&alt, &neu).bauen, ids(&["bottom"]));
    }

    #[test]
    fn the_window_list_setting_rebuilds_its_panel_only() {
        let alt = Layout::standard();
        let mut neu = alt.clone();
        neu.window_list = WindowListSettings {
            all_workspaces: Some(true),
        };
        assert_eq!(
            plan(&alt, &neu),
            Plan {
                behalten: ids(&["top"]),
                weg: ids(&["bottom"]),
                bauen: ids(&["bottom"]),
            }
        );
        // written down as the standard: nothing to do
        neu.window_list.all_workspaces = Some(false);
        assert_eq!(plan(&alt, &neu).bauen, Vec::<String>::new());
    }

    #[test]
    fn the_log_line_says_what_was_done() {
        let alt = Layout::standard();
        let mut neu = alt.clone();
        neu.panels.remove(0);
        let line = describe(&Source::File, &neu, &plan(&alt, &neu));
        assert_eq!(
            line,
            "Layout aus panel.ini, 1 Panel(s); neu aufgebaut: –; unverändert: bottom; \
             entfernt: top; Fensterliste nur aktuelle Arbeitsfläche"
        );
    }

    #[test]
    fn interface_matches_the_documented_method() {
        let node = gio::DBusNodeInfo::for_xml(INTROSPECTION).expect("XML");
        let iface = node.lookup_interface(INTERFACE).expect("Schnittstelle");
        assert!(iface.lookup_method(METHOD).is_some());
        // the answer (b geladen, s meldung), no arguments
        let args: Vec<&str> = INTROSPECTION
            .lines()
            .map(str::trim)
            .filter(|l| l.starts_with("<arg "))
            .collect();
        assert_eq!(
            args,
            [
                r#"<arg type="b" name="geladen" direction="out"/>"#,
                r#"<arg type="s" name="meldung" direction="out"/>"#
            ]
        );
    }
}
