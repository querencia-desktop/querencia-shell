//! The "Zum Panel hinzufügen" dialog of the GNOME 2 panel: a search field
//! and the applet types from the registry with icon, name and description,
//! sorted by name. A type can be selected; adding it to the panel follows
//! with M2.2c, until then the "Hinzufügen" button stays insensitive and
//! nothing is written.
//!
//! The dialog is an ordinary toplevel window, not a layer surface, so labwc
//! gives it the keyboard like any other window. It has no transient parent:
//! the panels are layer surfaces, not xdg toplevels.

use gtk4::prelude::*;
use gtk4::{
    Align, Application, Box as GtkBox, Button, Image, KeyvalTrigger, Label, ListBox, ListBoxRow,
    NamedAction, Orientation, PolicyType, ScrolledWindow, SearchEntry, SelectionMode, Shortcut,
    ShortcutController, Window, gdk, gio, glib,
};

use crate::applets::{self, AppletType};

const TITLE: &str = "Zum Panel hinzufügen";
/// Shown when the theme has neither the applet's icon nor its symbolic
/// variant.
const FALLBACK_ICON: &str = "application-x-addon";
const ICON_SIZE: i32 = 32;

/// Raises the dialog in `slot` or, if there is none, opens a new one there.
pub fn present(app: &Application, slot: &glib::WeakRef<Window>, panel_id: &str) {
    if let Some(window) = slot.upgrade() {
        window.present();
        return;
    }
    let window = build(app, panel_id);
    slot.set(Some(&window));
    window.present();
    eprintln!("querencia-panel: Dialog „{TITLE}“ für Panel {panel_id} geöffnet");
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

/// Icon names tried in order. GTK 4 does not fall back to the symbolic
/// variant by itself, and Adwaita ships most of the registry's names only
/// as symbolic icons.
fn icon_names(icon: &str) -> [String; 3] {
    [
        icon.to_string(),
        format!("{icon}-symbolic"),
        FALLBACK_ICON.to_string(),
    ]
}

fn build(app: &Application, panel_id: &str) -> Window {
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

    let list = ListBox::builder()
        .selection_mode(SelectionMode::Single)
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
    let panel_id = panel_id.to_string();
    list.connect_row_selected(move |_, row| {
        if let Some(row) = row {
            eprintln!(
                "querencia-panel: Dialog „{TITLE}“ (Panel {panel_id}): {} ausgewählt",
                row.widget_name()
            );
        }
    });
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

    let scrolled = ScrolledWindow::builder()
        .child(&list)
        .has_frame(true)
        .hscrollbar_policy(PolicyType::Never)
        .vexpand(true)
        .build();

    let close = Button::with_mnemonic("_Schließen");
    close.connect_clicked(glib::clone!(
        #[weak]
        window,
        move |_| window.close()
    ));
    let add = Button::builder()
        .label("_Hinzufügen")
        .use_underline(true)
        .sensitive(false)
        .tooltip_text("Hinzufügen ist noch nicht verfügbar")
        .build();
    let buttons = GtkBox::new(Orientation::Horizontal, 6);
    buttons.set_halign(Align::End);
    buttons.append(&close);
    buttons.append(&add);

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
    content.append(&buttons);
    window.set_child(Some(&content));

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

/// One list row: icon, name in bold, description below.
fn row(applet: &AppletType) -> ListBoxRow {
    let names = icon_names(applet.icon);
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let image = Image::from_gicon(&gio::ThemedIcon::from_names(&names));
    image.set_pixel_size(ICON_SIZE);

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
    // The type ID, e.g. for M2.2c and the GTK inspector.
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
                "Arbeitsflächenumschalter",
                "Fensterliste",
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
        assert_eq!(found("fenster"), ["window-list"]);
        // only in the description
        assert_eq!(found("datum"), ["clock"]);
        // non-ASCII, in name and description of the same type
        assert_eq!(found("FLÄCHE"), ["workspace-switcher"]);
        assert!(found("wetter").is_empty());
    }

    #[test]
    fn icons_fall_back_to_symbolic_then_generic() {
        assert_eq!(
            icon_names("start-here"),
            ["start-here", "start-here-symbolic", "application-x-addon"]
        );
        for applet in applets::APPLETS {
            assert_eq!(icon_names(applet.icon)[0], applet.icon);
        }
    }
}
