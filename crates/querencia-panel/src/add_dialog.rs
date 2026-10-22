//! The "Zum Panel hinzufügen" dialog of the GNOME 2 panel: a search field
//! and the applet types from the registry with icon, name and description,
//! sorted by name. "Hinzufügen" (or a double click, or Enter on a row) adds
//! the selected type to the panel the dialog belongs to, through
//! [`Hooks::add`]; the dialog stays open for more, as in GNOME 2. Types that
//! cannot be added once more (a single-instance type the layout already
//! has) are shown greyed out. If adding fails, the reason is shown above
//! the buttons and nothing changes.
//!
//! A type that needs an application (the launcher, HMP-11) is not added
//! directly: as in GNOME 2, "Hinzufügen" becomes "Vor" and leads to a second
//! page with the installed applications (`app_chooser.rs`); "Zurück" returns.
//!
//! The dialog is an ordinary toplevel window, not a layer surface, so labwc
//! gives it the keyboard like any other window. It has no transient parent:
//! the panels are layer surfaces, not xdg toplevels.

use std::cell::Cell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{
    Align, Application, Box as GtkBox, Button, KeyvalTrigger, Label, ListBox, ListBoxRow,
    NamedAction, Orientation, PolicyType, ScrolledWindow, SearchEntry, SelectionMode, Shortcut,
    ShortcutController, Stack, Window, gdk, gio, glib,
};

use crate::app_chooser;
use crate::applets::{self, AppletType};
use crate::layout::Region;
use crate::logo;

const TITLE: &str = "Zum Panel hinzufügen";
/// Shown when the theme has none of the applet's icons.
const FALLBACK_ICON: &str = "application-x-addon";
const ICON_SIZE: i32 = 32;
/// The choices of the region list, in order.
pub const REGIONS: [Region; 3] = [Region::Start, Region::Center, Region::End];

/// Name of a region in the dialog.
pub fn region_name(region: Region) -> &'static str {
    match region {
        Region::Start => "Links",
        Region::Center => "Mitte",
        Region::End => "Rechts",
    }
}

/// Tooltip of a type that cannot be added once more.
const UNAVAILABLE: &str = "Dieses Objekt gibt es nur einmal, und es ist bereits in einem Panel";

/// Adds an instance of an applet type, for a launcher with the desktop ID
/// of its application; `Err` carries the reason shown.
pub type AddFn = Box<dyn Fn(&str, Option<&str>) -> Result<(), String>>;

/// What the dialog needs from its panel.
pub struct Hooks {
    /// The region new applets go to (Teil 5): set from the third of the
    /// panel that was right-clicked, shown and chosen in the dialog.
    pub region: Rc<Cell<Region>>,
    /// Whether one more instance of the applet type may be added.
    pub available: Box<dyn Fn(&str) -> bool>,
    /// Adds an instance of the applet type (with the application's desktop
    /// ID for a launcher) to the panel; the error text is shown to the user.
    pub add: AddFn,
}

/// Raises the dialog in `slot` or, if there is none, opens a new one there.
pub fn present(app: &Application, slot: &glib::WeakRef<Window>, panel_id: &str, hooks: &Rc<Hooks>) {
    if let Some(window) = slot.upgrade() {
        window.present();
        return;
    }
    let window = build(app, panel_id, hooks);
    slot.set(Some(&window));
    window.present();
    eprintln!(
        "querencia-panel: Dialog „{TITLE}“ für Panel {panel_id} geöffnet (Bereich {})",
        hooks.region.get().label()
    );
}

/// All applet types, sorted by name for the user's locale.
pub fn choices() -> Vec<&'static AppletType> {
    let mut choices: Vec<_> = applets::APPLETS.iter().collect();
    choices.sort_by_cached_key(|applet| glib::CollationKey::from(applet.name));
    choices
}

/// Whether `applet` fits the search text: its name or description contains
/// it, ignoring case and surrounding space. An empty search fits all.
pub fn matches(applet: &AppletType, query: &str) -> bool {
    let query = query.trim().to_lowercase();
    query.is_empty()
        || applet.name.to_lowercase().contains(&query)
        || applet.description.to_lowercase().contains(&query)
}

/// The type "Hinzufügen" adds: the selected one, if it may be added.
fn add_target(selected: Option<&str>, available: impl Fn(&str) -> bool) -> Option<&str> {
    selected.filter(|id| !id.is_empty() && available(id))
}

/// Icon names tried in order: the GNOME 2 name, then the freedesktop name
/// and its symbolic variant (GTK 4 does not fall back to it by itself, and
/// Adwaita ships most names only as symbolic icons), then a generic one.
fn icon_names(applet: &AppletType) -> Vec<String> {
    applet
        .classic_icon
        .into_iter()
        .map(str::to_string)
        .chain([
            applet.icon.to_string(),
            format!("{}-symbolic", applet.icon),
            FALLBACK_ICON.to_string(),
        ])
        .collect()
}

fn build(app: &Application, panel_id: &str, hooks: &Rc<Hooks>) -> Window {
    let window = Window::builder()
        .application(app)
        .title(TITLE)
        .default_width(480)
        .default_height(440)
        .build();

    let search = SearchEntry::new();
    let prompt = Label::builder()
        .label("_Objekt suchen, das dem Panel hinzugefügt werden soll:")
        .use_underline(true)
        .mnemonic_widget(&search)
        .xalign(0.0)
        .build();

    // A single click only selects; a double click (or Enter) adds. GTK's
    // default would add on every single click.
    let list = ListBox::builder()
        .selection_mode(SelectionMode::Single)
        .activate_on_single_click(false)
        .build();
    let shown = choices();
    for applet in &shown {
        list.append(&row(applet));
    }
    list.set_filter_func(glib::clone!(
        #[weak]
        search,
        #[upgrade_or]
        true,
        move |row| {
            usize::try_from(row.index())
                .ok()
                .and_then(|index| shown.get(index))
                .is_some_and(|applet| matches(applet, &search.text()))
        }
    ));

    // Shown only after a failed attempt; the next change hides it again.
    let status = Label::builder()
        .xalign(0.0)
        .wrap(true)
        .visible(false)
        .css_classes(["error"])
        .build();

    let close = stock_button("_Schließen", "window-close");
    close.connect_clicked(glib::clone!(
        #[weak]
        window,
        move |_| window.close()
    ));
    let add = stock_button("_Hinzufügen", "list-add");
    add.set_sensitive(false);
    add.set_tooltip_text(Some("Das ausgewählte Objekt zum Panel hinzufügen"));
    // Instead of "Hinzufügen" while a type that needs an application is
    // selected, as in GNOME 2.
    let forward = stock_button("_Vor", "go-next");
    forward.set_visible(false);
    forward.set_tooltip_text(Some("Eine Anwendung für den Starter auswählen"));
    // Teil 5: where in the panel the new applet goes, preset from the
    // right-clicked third, as a choice left of the buttons.
    let names: Vec<&str> = REGIONS.iter().map(|r| region_name(*r)).collect();
    let place = gtk4::DropDown::from_strings(&names);
    place.set_valign(Align::Center);
    place.set_tooltip_text(Some("Bereich des Panels, in den das Objekt kommt"));
    let place_label = Label::builder()
        .label("_Bereich:")
        .use_underline(true)
        .mnemonic_widget(&place)
        .build();
    let sync_place = glib::clone!(
        #[weak]
        place,
        #[strong]
        hooks,
        move || {
            let index = REGIONS
                .iter()
                .position(|r| *r == hooks.region.get())
                .unwrap_or(0);
            if place.selected() as usize != index {
                place.set_selected(index as u32);
            }
        }
    );
    sync_place();
    place.connect_selected_notify(glib::clone!(
        #[strong]
        hooks,
        move |place| {
            if let Some(region) = REGIONS.get(place.selected() as usize) {
                if hooks.region.get() != *region {
                    eprintln!(
                        "querencia-panel: Dialog „{TITLE}“: Bereich {}",
                        region.label()
                    );
                }
                hooks.region.set(*region);
            }
        }
    ));
    let right = GtkBox::new(Orientation::Horizontal, 6);
    right.set_halign(Align::End);
    right.set_hexpand(true);
    right.append(&close);
    right.append(&add);
    right.append(&forward);
    let buttons = GtkBox::new(Orientation::Horizontal, 6);
    buttons.append(&place_label);
    buttons.append(&place);
    buttons.append(&right);

    // Page 2: the applications for a launcher.
    let stack = Stack::new();
    let panel_for_log = panel_id.to_string();
    let (apps, app_search) = app_chooser::page(app_chooser::Hooks {
        add: Box::new(glib::clone!(
            #[strong]
            hooks,
            move |app| (hooks.add)(crate::layout::LAUNCHER, Some(app))
        )),
        back: Box::new(glib::clone!(
            #[weak]
            stack,
            #[weak]
            list,
            move || {
                stack.set_visible_child_name(OBJECTS_PAGE);
                list.grab_focus();
            }
        )),
        close: Box::new(glib::clone!(
            #[weak]
            window,
            move || window.close()
        )),
    });

    // Greys out what cannot be added and keeps "Hinzufügen" in step. Runs
    // after every change and whenever the dialog becomes active, since the
    // dialog of the other panel may have added a single-instance type.
    let refresh = Rc::new(glib::clone!(
        #[weak]
        list,
        #[weak]
        add,
        #[weak]
        forward,
        #[strong]
        hooks,
        move || {
            let mut index = 0;
            while let Some(row) = list.row_at_index(index) {
                let available = (hooks.available)(&row.widget_name());
                row.set_sensitive(available);
                row.set_selectable(available);
                row.set_activatable(available);
                row.set_tooltip_text((!available).then_some(UNAVAILABLE));
                index += 1;
            }
            if list.selected_row().is_some_and(|row| !row.is_selectable()) {
                list.unselect_all();
            }
            show_buttons(&add, &forward, selected_target(&list, &hooks).as_deref());
        }
    ));
    refresh();

    let panel_id = panel_id.to_string();
    let add_selected = Rc::new(glib::clone!(
        #[weak]
        list,
        #[weak]
        status,
        #[weak]
        stack,
        #[weak]
        app_search,
        #[strong]
        hooks,
        #[strong]
        refresh,
        move || {
            let Some(applet) = selected_target(&list, &hooks) else {
                return;
            };
            if applets::find(&applet).is_some_and(|a| a.chooses_application) {
                stack.set_visible_child_name(APPS_PAGE);
                app_search.grab_focus();
                eprintln!(
                    "querencia-panel: Dialog „{TITLE}“ (Panel {panel_for_log}): Anwendung für {applet} auswählen"
                );
                return;
            }
            match (hooks.add)(&applet, None) {
                Ok(()) => status.set_visible(false),
                Err(err) => {
                    eprintln!(
                        "querencia-panel: Dialog „{TITLE}“ (Panel {panel_id}): {applet} nicht hinzugefügt: {err}"
                    );
                    status.set_label(&format!(
                        "Das Objekt konnte nicht hinzugefügt werden: {err}"
                    ));
                    status.set_visible(true);
                }
            }
            refresh();
        }
    ));
    add.connect_clicked(glib::clone!(
        #[strong]
        add_selected,
        move |_| add_selected()
    ));
    forward.connect_clicked(glib::clone!(
        #[strong]
        add_selected,
        move |_| add_selected()
    ));
    // Double click or Enter on a row, as in GNOME 2. Rows that cannot be
    // added are not activatable.
    list.connect_row_activated(glib::clone!(
        #[strong]
        add_selected,
        move |list, row| {
            list.select_row(Some(row));
            add_selected();
        }
    ));
    list.connect_row_selected(glib::clone!(
        #[weak]
        add,
        #[weak]
        forward,
        #[weak]
        status,
        #[strong]
        hooks,
        move |list, _| {
            status.set_visible(false);
            show_buttons(&add, &forward, selected_target(list, &hooks).as_deref());
        }
    ));
    search.connect_search_changed(glib::clone!(
        #[weak]
        list,
        move |_| {
            list.invalidate_filter();
            // A selection hidden by the filter must not stay selected.
            if list
                .selected_row()
                .is_some_and(|row| !row.is_child_visible())
            {
                list.unselect_all();
            }
        }
    ));
    // Escape in the search field first clears it, then closes the dialog.
    search.connect_stop_search(glib::clone!(
        #[weak]
        window,
        move |search| {
            if search.text().is_empty() {
                window.close();
            } else {
                search.set_text("");
            }
        }
    ));
    window.connect_is_active_notify(glib::clone!(
        #[strong]
        refresh,
        move |window| {
            if window.is_active() {
                refresh();
                // raised again after a right click elsewhere in the panel
                sync_place();
            }
        }
    ));

    let scrolled = ScrolledWindow::builder()
        .child(&list)
        .has_frame(true)
        .hscrollbar_policy(PolicyType::Never)
        .vexpand(true)
        .build();

    let content = GtkBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(6)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();
    content.append(&prompt);
    content.append(&search);
    content.append(&scrolled);
    content.append(&status);
    content.append(&buttons);
    stack.add_named(&content, Some(OBJECTS_PAGE));
    stack.add_named(&apps, Some(APPS_PAGE));
    stack.set_visible_child_name(OBJECTS_PAGE);
    window.set_child(Some(&stack));

    // Escape anywhere else in the dialog closes it, as in GNOME 2 dialogs.
    let shortcuts = ShortcutController::new();
    shortcuts.add_shortcut(Shortcut::new(
        Some(KeyvalTrigger::new(
            gdk::Key::Escape,
            gdk::ModifierType::empty(),
        )),
        Some(NamedAction::new("window.close")),
    ));
    window.add_controller(shortcuts);
    GtkWindowExt::set_focus(&window, Some(&search));
    window
}

/// Page names of the dialog's stack.
const OBJECTS_PAGE: &str = "objekte";
const APPS_PAGE: &str = "anwendungen";

/// "Vor" instead of "Hinzufügen" for a type that needs an application;
/// either is sensitive only with an addable type selected.
fn show_buttons(add: &Button, forward: &Button, selected: Option<&str>) {
    let choosing = selected
        .and_then(applets::find)
        .is_some_and(|applet| applet.chooses_application);
    add.set_visible(!choosing);
    forward.set_visible(choosing);
    add.set_sensitive(selected.is_some());
    forward.set_sensitive(selected.is_some());
}

/// The type ID of the selected row, if it is visible and may be added.
fn selected_target(list: &ListBox, hooks: &Hooks) -> Option<String> {
    let row = list.selected_row().filter(|row| row.is_child_visible())?;
    let id = row.widget_name();
    add_target(Some(id.as_str()), |id| (hooks.available)(id)).map(str::to_string)
}

/// A button with a small icon before its label, like the GTK 2 stock
/// buttons of GNOME 2 dialogs.
pub(crate) fn stock_button(mnemonic: &str, icon: &str) -> Button {
    let image = crate::symbole::image(
        &gio::ThemedIcon::from_names(&[icon, &format!("{icon}-symbolic")]),
        16,
    );
    let label = Label::with_mnemonic(mnemonic);
    let inner = GtkBox::new(Orientation::Horizontal, 4);
    inner.set_halign(Align::Center);
    inner.append(&image);
    inner.append(&label);
    let button = Button::builder().child(&inner).build();
    label.set_mnemonic_widget(Some(&button));
    button
}

/// One list row: icon, name in bold, description below. "Anwendungen"
/// shows the built-in Querencia logo (QREL-04c) at the same size,
/// "Lautstärke" (G2-J), "Netzwerk" (G2-M) and "Mitteilungen" (G2-AL) their
/// own icons.
fn row(applet: &AppletType) -> ListBoxRow {
    let image = if applet.icon == logo::ICON_NAME {
        logo::image_of_size(ICON_SIZE)
    } else if applet.id == crate::applets::VOLUME {
        // G2-J: the volume applet's own Querencia icon, not a theme icon.
        crate::volume_applet::image_of_size(ICON_SIZE)
    } else if applet.id == crate::applets::NETWORK {
        // G2-M: the network applet's own Querencia icon.
        crate::network_applet::image_of_size(ICON_SIZE)
    } else if applet.id == crate::applets::NOTIFICATIONS {
        // G2-AL: the bell's own Querencia icon.
        crate::glocke::image_of_size(ICON_SIZE)
    } else {
        let names = icon_names(applet);
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        crate::symbole::image(&gio::ThemedIcon::from_names(&names), ICON_SIZE)
    };

    let name = Label::builder()
        .label(format!("<b>{}</b>", glib::markup_escape_text(applet.name)))
        .use_markup(true)
        .xalign(0.0)
        .build();
    let description = Label::builder()
        .label(applet.description)
        .xalign(0.0)
        .wrap(true)
        .css_classes(["dim-label"])
        .build();
    let text = GtkBox::new(Orientation::Vertical, 2);
    text.set_valign(Align::Center);
    text.append(&name);
    text.append(&description);

    let line = GtkBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(12)
        .margin_top(6)
        .margin_bottom(6)
        .margin_start(6)
        .margin_end(6)
        .build();
    line.append(&image);
    line.append(&text);

    let row = ListBoxRow::builder().child(&line).build();
    // The type ID: what "Hinzufügen" adds; also for the GTK inspector.
    row.set_widget_name(applet.id);
    row
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choices_are_all_types_sorted_by_name() {
        let names: Vec<_> = choices().iter().map(|a| a.name).collect();
        assert_eq!(
            names,
            [
                "Anwendungen",
                "Anwendungsstarter …",
                "Arbeitsfläche anzeigen",
                "Arbeitsflächenumschalter",
                "Fensterliste",
                "Lautstärke",
                "Mitteilungen",
                "Netzwerk",
                "Orte",
                "System",
                "Uhr"
            ]
        );
        assert_eq!(choices().len(), applets::APPLETS.len());
    }

    #[test]
    fn empty_search_shows_everything() {
        for query in ["", "   "] {
            assert!(applets::APPLETS.iter().all(|a| matches(a, query)));
        }
    }

    #[test]
    fn search_looks_at_name_and_description_ignoring_case() {
        let found = |query: &str| -> Vec<&str> {
            choices()
                .into_iter()
                .filter(|a| matches(a, query))
                .map(|a| a.id)
                .collect()
        };
        assert_eq!(found("UHR"), ["clock"]);
        assert_eq!(found(" orte "), ["places"]);
        // G2-AL: „Alle Fenster minimieren …“
        assert_eq!(found("fenster"), ["show-desktop", "window-list"]);
        // only in the description
        assert_eq!(found("datum"), ["clock"]);
        assert_eq!(found("zentrale"), ["benachrichtigungen"]);
        // non-ASCII, in name and description of the same type
        assert_eq!(found("FLÄCHE"), ["show-desktop", "workspace-switcher"]);
        assert!(found("wetter").is_empty());
    }

    #[test]
    fn icons_fall_back_to_symbolic_then_generic() {
        let places = applets::find("places").unwrap();
        assert_eq!(
            icon_names(places),
            ["folder", "folder-symbolic", "application-x-addon"]
        );
        for applet in applets::APPLETS {
            assert!(icon_names(applet).contains(&applet.icon.to_string()));
        }
    }

    #[test]
    fn classic_gnome2_icons_come_first() {
        // GNOME Human has the GNOME 2 panel's own icons for these two, but
        // none of the freedesktop names
        assert_eq!(
            icon_names(applets::find("clock").unwrap()),
            [
                "clock",
                "preferences-system-time",
                "preferences-system-time-symbolic",
                "application-x-addon"
            ]
        );
        assert_eq!(
            icon_names(applets::find("workspace-switcher").unwrap())[0],
            "workspace-switcher"
        );
    }

    #[test]
    fn add_target_needs_a_selected_available_type() {
        let available = |id: &str| id != "window-list";
        assert_eq!(add_target(Some("clock"), available), Some("clock"));
        assert_eq!(add_target(Some("window-list"), available), None);
        assert_eq!(add_target(None, available), None);
        assert_eq!(add_target(Some(""), available), None);
    }
}
