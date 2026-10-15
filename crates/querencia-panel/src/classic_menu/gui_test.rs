//! Isolated GUI test of the classic menus (HMP-04).
//!
//! Runs only inside the isolated session of `gui-test/isoliert.py` (own
//! headless labwc, own D-Bus, own XDG profile), never in the running desktop
//! session: it refuses to start unless `QUERENCIA_ISOLATED_GUI=1` and the
//! runtime directory is one of those throw-away directories. There is no
//! input device in that compositor, so pointer and keys are fed in through
//! the same methods the event controllers call; GTK itself computes the
//! style, places the popovers and the compositor maps them.
//!
//! What it checks, with the real panel buttons, style sheet, icon theme and
//! the desktop entries of `session/querencia/applications`:
//! - text colours of normal, highlighted and insensitive rows, also in a
//!   submenu opened from a highlighted row (the HMP-04 bug);
//! - submenus open to the right, their first row level with the row;
//! - keyboard: Down, Right into a submenu, Left out of it, Left/Right
//!   between the main menus; pointer onto another main menu switches;
//! - activating an entry closes every menu and runs its action;
//! - Querencia Files, Querencia Terminal and "Bildschirmfoto aufnehmen" are
//!   in Zubehör exactly once and start from the menu;
//! - Orte offers its places now that a file manager is installed;
//! - HMP-05: no submenu grabs, the keyboard focus stays in the top level, a
//!   pending open of a row the pointer has left never fires, and after a
//!   dwell of 0.5 s or 2 s on an open submenu the next row takes over and
//!   the old submenu closes. (Real pointer and keyboard with the popup
//!   grab: `tools/isolierte-gui-pruefung/pruefen.sh`.)
//!
//! The System menu is opened, but nothing in it is ever activated.
//! Screenshots go to `$HMP04_SHOT_DIR`.

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Label, MenuButton, Orientation, Window, gdk, gio, glib};

use super::*;

fn pump(ms: u64) {
    let context = glib::MainContext::default();
    let end = Instant::now() + Duration::from_millis(ms);
    while Instant::now() < end {
        while context.iteration(false) {}
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn shot(name: &str) {
    if let Ok(dir) = std::env::var("HMP04_SHOT_DIR") {
        pump(300);
        let path = format!("{dir}/{name}.png");
        let ok = std::process::Command::new("grim")
            .arg(&path)
            .status()
            .is_ok_and(|s| s.success());
        println!("BILD {path} {}", if ok { "ok" } else { "FEHLER" });
    }
}

/// The open top level of `button`'s classic menu.
fn level(button: &MenuButton) -> Rc<Level> {
    MAIN_MENUS.with(|menus| {
        menus
            .borrow()
            .iter()
            .find(|(b, _)| b.upgrade().as_ref() == Some(button))
            .and_then(|(_, current)| current.upgrade())
            .and_then(|current| current.borrow().clone())
            .expect("menu is open")
    })
}

fn child(level: &Level) -> Rc<Level> {
    level
        .child
        .borrow()
        .as_ref()
        .map(|(_, _, child)| child.clone())
        .expect("submenu is open")
}

fn labels(level: &Level) -> Vec<String> {
    level
        .rows
        .iter()
        .map(|row| match row {
            Row::Separator => "---".into(),
            Row::Entry { label, .. } | Row::Submenu { label, .. } => label.clone(),
        })
        .collect()
}

fn index_of(level: &Level, label: &str) -> usize {
    labels(level)
        .iter()
        .position(|l| l == label)
        .unwrap_or_else(|| panic!("{label} fehlt in {:?}", labels(level)))
}

/// Colour of the label in row `index`.
fn text_colour(level: &Level, index: usize) -> gdk::RGBA {
    let mut child = level.widgets[index].first_child();
    while let Some(widget) = child {
        if let Some(label) = widget.downcast_ref::<Label>() {
            return label.color();
        }
        child = widget.next_sibling();
    }
    panic!("row {index} has no label")
}

fn rgb(c: &gdk::RGBA) -> String {
    format!(
        "#{:02x}{:02x}{:02x}",
        (c.red() * 255.0).round() as u8,
        (c.green() * 255.0).round() as u8,
        (c.blue() * 255.0).round() as u8
    )
}

/// Relative luminance (WCAG) and contrast ratio.
fn luminance(c: &gdk::RGBA) -> f64 {
    let lin = |v: f32| {
        let v = f64::from(v);
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c.red()) + 0.7152 * lin(c.green()) + 0.0722 * lin(c.blue())
}

fn contrast(a: &gdk::RGBA, b: &gdk::RGBA) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// Menu background (`@theme_bg_color` of GTK's default theme) and the
/// Ambiance highlight, as in `PANEL_CSS`.
fn menu_background() -> gdk::RGBA {
    gdk::RGBA::parse("#f5f4f2").unwrap()
}
/// Middle of the highlight gradient #f8875d..#ea703d (HMP-05), as in the
/// GNOME 2 reference; white text on it has a contrast of about 2.7:1, the
/// same as in the reference.
fn highlight() -> gdk::RGBA {
    gdk::RGBA::parse("#f17c4d").unwrap()
}

/// Surface rectangle of an open popover, relative to its parent surface.
fn popup_rect(popover: &gtk4::Popover) -> Rect {
    let surface = popover.surface().expect("mapped");
    let popup = surface.downcast_ref::<gdk::Popup>().expect("popup");
    Rect {
        x: f64::from(popup.position_x()),
        y: f64::from(popup.position_y()),
        width: f64::from(surface.width()),
        height: f64::from(surface.height()),
    }
}

/// Top of row `index` in its menu's surface coordinates.
fn row_top(level: &Level, index: usize) -> f64 {
    let popover = level.popover.upgrade().unwrap();
    let bounds = level.widgets[index].compute_bounds(&popover).unwrap();
    let (_, dy) = popover.surface_transform();
    f64::from(bounds.y()) + dy
}

/// Middle of row `index` in its menu's content coordinates (as the motion
/// controller reports them).
fn row_mid(level: &Level, index: usize) -> (f64, f64) {
    let bounds = level.widgets[index].compute_bounds(&level.content).unwrap();
    (
        f64::from(bounds.x() + bounds.width() / 3.0),
        f64::from(bounds.y() + bounds.height() / 2.0),
    )
}

fn is_open(button: &MenuButton) -> bool {
    super::is_open(button)
}

#[test]
#[ignore = "nur in der isolierten GUI-Umgebung (gui-test/isoliert.py)"]
fn isolated_gui_menus() {
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_default();
    assert!(
        std::env::var("QUERENCIA_ISOLATED_GUI").as_deref() == Ok("1")
            && (runtime.contains("/hmp04-runtime-") || runtime.contains("/hmp05-runtime-")),
        "Abbruch: nur in der isolierten Sitzung von isoliert.py (XDG_RUNTIME_DIR={runtime})"
    );
    gtk4::init().expect("GTK in the isolated session");
    crate::install_panel_style();
    // No input device here, so no input serial for a popup grab: the top
    // levels must not grab in this test (submenus never grab, HMP-05). Real
    // pointer and keyboard with grabs: tools/isolierte-gui-pruefung.
    let no_grab = |button: &MenuButton| button.popover().unwrap().set_autohide(false);

    // A panel like the real one: the three main menus in a .querencia-panel.
    // At the top and only as high as its buttons, like the real panel: a
    // button as high as the window would put its menu off-screen, and GTK
    // closes a popover that does not fit.
    let panel = GtkBox::new(Orientation::Horizontal, 0);
    panel.add_css_class("querencia-panel");
    panel.set_valign(gtk4::Align::Start);
    panel.set_halign(gtk4::Align::Start);
    let apps = crate::app_menu::button();
    let places = crate::places_menu::button();
    let system = crate::system_menu::button();
    for button in [&apps, &places, &system] {
        no_grab(button);
        panel.append(button);
    }
    let window = Window::builder()
        .title("HMP-04 Menüprobe")
        .default_width(900)
        .default_height(600)
        .child(&panel)
        .build();
    window.present();
    pump(1500);

    // ---- Applications > Zubehör: the three Querencia applications ----
    apps.popup();
    pump(500);
    let root = level(&apps);
    println!("Anwendungen: {:?}", labels(&root));
    let accessories = index_of(&root, "Zubehör");
    root.select(Some(accessories), true);
    root.open_child(accessories, true);
    pump(800);
    let zub = child(&root);
    let entries = labels(&zub);
    println!("Zubehör: {entries:?}");
    for name in [
        "Querencia Files",
        "Querencia Terminal",
        "Bildschirmfoto aufnehmen",
    ] {
        assert_eq!(
            entries.iter().filter(|e| *e == name).count(),
            1,
            "{name} genau einmal"
        );
    }
    // no application twice anywhere in the applications menu
    let mut all = Vec::new();
    for (i, row) in root.rows.iter().enumerate() {
        if let Row::Submenu { model, .. } = row {
            for r in super::rows(model) {
                if let Row::Entry { label, .. } = r {
                    all.push((root.rows.len(), i, label));
                }
            }
        }
    }
    let mut names: Vec<&String> = all.iter().map(|(_, _, l)| l).collect();
    names.sort();
    let before = names.len();
    names.dedup();
    assert_eq!(before, names.len(), "doppelte Programmeinträge");

    // submenu to the right of the menu, first row level with "Zubehör"
    let (_, sub_popover, _) = root.child.borrow().clone().unwrap();
    // GTK's own classes survive: `background` resets the text colour of
    // every popover, `popup` marks the menu button (HMP-04 cause)
    for popover in [&apps.popover().unwrap(), &sub_popover] {
        assert!(
            popover.has_css_class("background"),
            "{:?}",
            popover.css_classes()
        );
        assert!(popover.has_css_class(MENU_CLASS));
    }
    assert!(apps.has_css_class("popup") && apps.has_css_class("querencia-main-menu"));
    // HMP-05 cause: a submenu must never grab (gtk_grab_add would hand it
    // every pointer event over the menu above)
    assert!(!sub_popover.is_autohide(), "Untermenü greift");
    // and keyboard focus stays in the top level
    let focus: Option<gtk4::Widget> = gtk4::prelude::GtkWindowExt::focus(&window);
    assert!(
        focus
            .as_ref()
            .is_none_or(|f: &gtk4::Widget| !f.is_ancestor(&sub_popover)),
        "Tastaturfokus im Untermenü: {focus:?}"
    );
    let rect = popup_rect(&sub_popover);
    let root_width = f64::from(root.popover.upgrade().unwrap().surface().unwrap().width());
    println!("Untermenü {rect:?}, Menübreite {root_width}");
    assert!(
        rect.x >= root_width - 4.0,
        "Untermenü nicht rechts daneben: {rect:?}"
    );
    let first = zub.rows.iter().position(Row::selectable).unwrap();
    let dy = (rect.y + row_top(&zub, first)) - row_top(&root, accessories);
    assert!(
        dy.abs() <= 3.0,
        "erste Zeile nicht auf Höhe von Zubehör: {dy}"
    );

    // colours: highlighted row white on orange, the others dark on light
    let active = zub
        .active
        .get()
        .expect("first entry highlighted by keyboard");
    zub.on_key(gdk::Key::Down);
    zub.on_key(gdk::Key::Down);
    pump(200);
    let active = zub
        .active
        .get()
        .filter(|a| *a != active)
        .expect("Down moves");
    let white = text_colour(&zub, active);
    println!("Zubehör hervorgehoben: {}", rgb(&white));
    assert!(contrast(&white, &highlight()) >= 2.5, "{}", rgb(&white));
    assert!(luminance(&white) > 0.9);
    for (i, row) in zub.rows.iter().enumerate() {
        if i == active || !row.selectable() {
            continue;
        }
        let c = text_colour(&zub, i);
        let ratio = contrast(&c, &menu_background());
        println!(
            "Zubehör {:?}: {} Kontrast {ratio:.1}",
            labels(&zub)[i],
            rgb(&c)
        );
        assert!(ratio >= 7.0, "unlesbar: {:?} {}", labels(&zub)[i], rgb(&c));
    }
    // the parent row of the open submenu stays highlighted; root rows dark
    for (i, row) in root.rows.iter().enumerate() {
        if i != accessories && row.selectable() {
            assert!(contrast(&text_colour(&root, i), &menu_background()) >= 7.0);
        }
    }
    shot("01-anwendungen-zubehoer");

    // Left closes the submenu, Left in the top level goes to System (wraps)
    zub.on_key(gdk::Key::Left);
    pump(400);
    assert!(root.child_row().is_none(), "Left schließt das Untermenü");
    root.on_key(gdk::Key::Left);
    pump(600);
    assert!(
        !is_open(&apps) && is_open(&system),
        "Left: Anwendungen -> System"
    );
    let sys = level(&system);
    println!("System: {:?}", labels(&sys));
    shot("02-system");
    // Right on an entry without submenu: next main menu (Anwendungen)
    let first_plain = sys
        .rows
        .iter()
        .position(|r| {
            matches!(
                r,
                Row::Entry {
                    action: Some(_),
                    ..
                }
            )
        })
        .unwrap();
    sys.select(Some(first_plain), true);
    sys.on_key(gdk::Key::Right);
    pump(600);
    assert!(
        is_open(&apps) && !is_open(&system),
        "Right: System -> Anwendungen"
    );

    // ---- pointer and timers (HMP-05), state of the menu itself ----
    // (The grab cause needs a real pointer: tools/isolierte-gui-pruefung.)
    let root = level(&apps);
    let acc = index_of(&root, "Zubehör");
    let net = index_of(&root, "Internet");
    let tools = index_of(&root, "Systemwerkzeuge");
    let point = |index: usize| {
        let (x, y) = row_mid(&root, index);
        root.on_motion(x, y);
    };
    // a pending open of Zubehör must not fire once the pointer is on Internet
    point(acc);
    pump(SUBMENU_DELAY_MS / 2);
    point(net);
    // the Zubehör timer would have fired by now, Internet's not yet
    pump(SUBMENU_DELAY_MS / 2 + 60);
    assert_eq!(
        root.child_row(),
        None,
        "veralteter Zeitgeber hat ein Untermenü geöffnet"
    );
    pump(SUBMENU_DELAY_MS + AIM_TIMEOUT_MS);
    assert_eq!(root.active.get(), Some(net), "Markierung auf Internet");
    assert_eq!(
        root.child_row(),
        Some(net),
        "veralteter Zeitgeber öffnete Zubehör"
    );
    // dwell on Systemwerkzeuge with its submenu open, then switch away and back
    for dwell in [500, 2000] {
        point(tools);
        pump(SUBMENU_DELAY_MS + 150);
        assert_eq!(root.child_row(), Some(tools), "Systemwerkzeuge offen");
        let old = child(&root).popover.upgrade().unwrap();
        pump(dwell);
        point(acc);
        pump(SUBMENU_DELAY_MS + AIM_TIMEOUT_MS + 100);
        assert_eq!(root.active.get(), Some(acc), "nach {dwell} ms: Markierung");
        assert_eq!(root.child_row(), Some(acc), "nach {dwell} ms: Untermenü");
        assert!(!old.is_visible(), "nach {dwell} ms: altes Untermenü offen");
        println!("Verweilen {dwell} ms auf Systemwerkzeuge, Wechsel zu Zubehör: ok");
    }

    // pointer onto Orte while Anwendungen is open switches over
    super::switch_on_hover(&places);
    pump(600);
    assert!(
        is_open(&places) && !is_open(&apps),
        "Überfahren: Anwendungen -> Orte"
    );
    let orte = level(&places);
    println!("Orte: {:?}", labels(&orte));
    assert!(labels(&orte).contains(&"Persönlicher Ordner".to_string()));
    assert!(
        !labels(&orte)
            .iter()
            .any(|l| l.contains("Kein Dateimanager"))
    );
    for i in 0..orte.rows.len() {
        if orte.rows[i].selectable() {
            assert!(contrast(&text_colour(&orte, i), &menu_background()) >= 7.0);
        }
    }
    shot("03-orte");
    // pointer onto a closed menu while none is open does nothing
    places.popdown();
    pump(300);
    super::switch_on_hover(&system);
    pump(300);
    assert!(
        !is_open(&system),
        "ohne offenes Menü öffnet Überfahren nichts"
    );

    // ---- colours of an insensitive row and activation, own menu ----
    let ran = Rc::new(Cell::new(0));
    let probe = gio::Menu::new();
    let sub = gio::Menu::new();
    sub.append(Some("Normal"), Some("probe.ran"));
    sub.append(Some("Deaktiviert"), None);
    sub.append(Some("Ausführen"), Some("probe.ran"));
    probe.append_submenu(Some("Prüfmenü"), &sub);
    let probe_button = crate::app_menu::menu_button("Probe", None, &probe);
    no_grab(&probe_button);
    let action = gio::SimpleAction::new("ran", None);
    action.connect_activate(glib::clone!(
        #[strong]
        ran,
        move |_, _| ran.set(ran.get() + 1)
    ));
    let group = gio::SimpleActionGroup::new();
    group.add_action(&action);
    probe_button.insert_action_group("probe", Some(&group));
    probe_button.set_valign(gtk4::Align::Start);
    probe_button.set_halign(gtk4::Align::Start);
    let probe_window = Window::builder()
        .title("Probe")
        .default_width(400)
        .default_height(300)
        .child(&probe_button)
        .build();
    probe_window.present();
    pump(800);
    probe_button.popup();
    pump(400);
    let top = level(&probe_button);
    top.select(Some(0), true);
    top.open_child(0, true);
    pump(600);
    let sub_level = child(&top);
    let grey = text_colour(&sub_level, 1);
    println!("deaktiviert: {}", rgb(&grey));
    let normal = text_colour(&sub_level, 2);
    assert!(
        contrast(&normal, &menu_background()) >= 7.0,
        "{}",
        rgb(&normal)
    );
    let grey_ratio = contrast(&grey, &menu_background());
    assert!(
        (1.8..4.5).contains(&grey_ratio),
        "grau, aber erkennbar: {} {grey_ratio:.1}",
        rgb(&grey)
    );
    assert!(!sub_level.widgets[1].is_sensitive());
    shot("04-deaktiviert");
    sub_level.select(Some(2), true);
    sub_level.on_key(gdk::Key::Return);
    pump(500);
    assert_eq!(ran.get(), 1, "Aktion ausgeführt");
    assert!(!is_open(&probe_button), "alle Menüs geschlossen");
    assert!(top.child_row().is_none());
    probe_window.close();

    // ---- start the three applications from Zubehör, as a user would ----
    for name in [
        "Querencia Terminal",
        "Bildschirmfoto aufnehmen",
        "Querencia Files",
    ] {
        apps.popup();
        pump(400);
        let root = level(&apps);
        let accessories = index_of(&root, "Zubehör");
        root.select(Some(accessories), true);
        root.open_child(accessories, true);
        pump(500);
        let zub = child(&root);
        let index = index_of(&zub, name);
        zub.select(Some(index), true);
        zub.on_key(gdk::Key::Return);
        pump(300);
        assert!(!is_open(&apps), "{name}: Menü nach dem Start geschlossen");
        println!("GESTARTET {name}");
        pump(5000);
        shot(&format!("05-start-{}", name.replace(' ', "-")));
    }
    window.close();
    pump(300);
}
