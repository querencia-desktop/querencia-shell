//! Second page of the "Zum Panel hinzufügen" dialog (HMP-11): after
//! "Anwendungsstarter …" and "Vor", the GNOME 2 panel showed the
//! applications of its menus to pick the one the new launcher starts. The
//! list comes from the same catalog as the "Anwendungen" and "System" menus
//! (`apps.rs`, GIO), in the same groups and order: the application groups
//! first, then the "Einstellungen" and "Systemverwaltung" sections of the
//! System menu. It follows installed, removed and changed applications
//! while the dialog is open. No second application menu: nothing here is
//! shown in the panel.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Label, ListBox, ListBoxRow, Orientation, PolicyType, ScrolledWindow,
    SearchEntry, SelectionMode, gio, glib,
};

use crate::add_dialog::stock_button;
use crate::apps::{AppEntry, Catalog, SystemSection};

/// Icon of an application without a usable `Icon=` (as in the menus).
const GENERIC_ICON: &str = "application-x-executable";
const APP_ICON_SIZE: i32 = 24;
const GROUP_ICON_SIZE: i32 = 16;

/// One heading of the list with its applications.
#[derive(Debug)]
pub struct Section {
    pub label: &'static str,
    pub icon: &'static str,
    pub entries: Vec<AppEntry>,
}

/// The applications a launcher can be made for, as the menus show them:
/// every group of "Anwendungen" (settings left out there), then the two
/// sections of "System" with the settings. Empty groups are left out.
pub fn sections(catalog: &Catalog) -> Vec<Section> {
    let mut sections: Vec<Section> = catalog
        .groups()
        .map(|(group, entries)| Section {
            label: group.label(),
            icon: group.icon(),
            entries: entries
                .iter()
                .filter(|e| !e.is_settings())
                .cloned()
                .collect(),
        })
        .filter(|s| !s.entries.is_empty())
        .collect();
    for (section, entries) in catalog.system_sections() {
        sections.push(Section {
            label: section.label(),
            icon: match section {
                SystemSection::Preferences => "preferences-desktop",
                SystemSection::Administration => "preferences-system",
            },
            entries: entries.into_iter().cloned().collect(),
        });
    }
    sections
}

/// Whether `entry` fits the search text: its name or desktop ID contains
/// it, ignoring case and surrounding space. An empty search fits all.
pub fn matches(entry: &AppEntry, query: &str) -> bool {
    let query = query.trim().to_lowercase();
    query.is_empty()
        || entry.name.to_lowercase().contains(&query)
        || entry.id.to_lowercase().contains(&query)
}

/// What a row of the list stands for.
enum Row {
    Heading(usize),
    App(usize, AppEntry),
}

/// Adds a launcher for a desktop ID; `Err` carries the reason shown.
pub type AddFn = Box<dyn Fn(&str) -> Result<(), String>>;

/// Callbacks of the page.
pub struct Hooks {
    /// Adds a launcher for the desktop ID; the error text is shown.
    pub add: AddFn,
    /// "Zurück": back to the first page.
    pub back: Box<dyn Fn()>,
    /// "Schließen".
    pub close: Box<dyn Fn()>,
}

/// The page and its search field (to focus when the page is shown).
pub fn page(hooks: Hooks) -> (GtkBox, SearchEntry) {
    let hooks = Rc::new(hooks);
    let search = SearchEntry::new();
    let prompt = Label::builder()
        .label("_Anwendung auswählen, deren Starter dem Panel hinzugefügt werden soll:")
        .use_underline(true)
        .mnemonic_widget(&search)
        .xalign(0.0)
        .wrap(true)
        .build();
    let list = ListBox::builder()
        .selection_mode(SelectionMode::Single)
        .activate_on_single_click(false)
        .build();
    let rows: Rc<RefCell<Vec<Row>>> = Rc::new(RefCell::new(Vec::new()));
    list.set_filter_func(glib::clone!(
        #[weak]
        search,
        #[strong]
        rows,
        #[upgrade_or]
        true,
        move |row| visible(&rows.borrow(), row.index(), &search.text())
    ));

    let status = Label::builder()
        .xalign(0.0)
        .wrap(true)
        .visible(false)
        .css_classes(["error"])
        .build();
    let back = stock_button("_Zurück", "go-previous");
    let close = stock_button("_Schließen", "window-close");
    let add = stock_button("_Hinzufügen", "list-add");
    add.set_sensitive(false);
    add.set_tooltip_text(Some(
        "Einen Starter für die ausgewählte Anwendung hinzufügen",
    ));
    let buttons = GtkBox::new(Orientation::Horizontal, 6);
    let spacer = GtkBox::new(Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    buttons.append(&back);
    buttons.append(&spacer);
    buttons.append(&close);
    buttons.append(&add);

    fill(&list, &rows);

    back.connect_clicked(glib::clone!(
        #[strong]
        hooks,
        move |_| (hooks.back)()
    ));
    close.connect_clicked(glib::clone!(
        #[strong]
        hooks,
        move |_| (hooks.close)()
    ));
    let add_selected = Rc::new(glib::clone!(
        #[weak]
        list,
        #[weak]
        status,
        #[strong]
        hooks,
        move || {
            let Some(app) = selected_app(&list) else {
                return;
            };
            match (hooks.add)(&app) {
                Ok(()) => status.set_visible(false),
                Err(err) => {
                    eprintln!("querencia-panel: Starter für {app} nicht hinzugefügt: {err}");
                    status.set_label(&format!(
                        "Der Starter konnte nicht hinzugefügt werden: {err}"
                    ));
                    status.set_visible(true);
                }
            }
        }
    ));
    add.connect_clicked(glib::clone!(
        #[strong]
        add_selected,
        move |_| add_selected()
    ));
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
        status,
        move |list, _| {
            status.set_visible(false);
            add.set_sensitive(selected_app(list).is_some());
        }
    ));
    search.connect_search_changed(glib::clone!(
        #[weak]
        list,
        move |_| {
            list.invalidate_filter();
            if list
                .selected_row()
                .is_some_and(|row| !row.is_child_visible())
            {
                list.unselect_all();
            }
        }
    ));
    search.connect_stop_search(glib::clone!(
        #[strong]
        hooks,
        move |search| {
            if search.text().is_empty() {
                (hooks.close)();
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

    // Follows the installed applications while the dialog exists, keeping
    // the selection if the application is still there.
    let monitor = gio::AppInfoMonitor::get();
    let changed = monitor.connect_changed(glib::clone!(
        #[weak]
        list,
        #[weak]
        add,
        #[strong]
        rows,
        move |_| {
            let selected = selected_app(&list);
            fill(&list, &rows);
            if let Some(app) = selected {
                select(&list, &app);
            }
            add.set_sensitive(selected_app(&list).is_some());
            eprintln!("querencia-panel: Anwendungen geändert, Starterauswahl neu aufgebaut");
        }
    ));
    let changed = Cell::new(Some(changed));
    content.connect_destroy(move |_| {
        if let Some(id) = changed.take() {
            monitor.disconnect(id);
        }
    });
    (content, search)
}

/// Whether row `index` is shown for the search text `query`: an
/// application if it matches, a heading if one of its applications does.
fn visible(rows: &[Row], index: i32, query: &str) -> bool {
    let Some(row) = usize::try_from(index).ok().and_then(|i| rows.get(i)) else {
        return true;
    };
    match row {
        Row::App(_, entry) => matches(entry, query),
        Row::Heading(section) => rows
            .iter()
            .any(|r| matches!(r, Row::App(s, e) if s == section && matches(e, query))),
    }
}

/// Reads the catalog and refills `list` and `rows` from it.
fn fill(list: &ListBox, rows: &RefCell<Vec<Row>>) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
    let mut new_rows = Vec::new();
    let sections = sections(&Catalog::load());
    for (index, section) in sections.into_iter().enumerate() {
        list.append(&heading_row(&section));
        new_rows.push(Row::Heading(index));
        for entry in section.entries {
            list.append(&app_row(&entry));
            new_rows.push(Row::App(index, entry));
        }
    }
    *rows.borrow_mut() = new_rows;
    list.invalidate_filter();
}

/// The desktop ID of the selected, visible application row.
fn selected_app(list: &ListBox) -> Option<String> {
    let row = list
        .selected_row()
        .filter(|row| row.is_child_visible() && row.is_selectable())?;
    let id = row.widget_name();
    (!id.is_empty()).then(|| id.to_string())
}

fn select(list: &ListBox, app: &str) {
    let mut index = 0;
    while let Some(row) = list.row_at_index(index) {
        if row.widget_name() == app {
            list.select_row(Some(&row));
            return;
        }
        index += 1;
    }
}

/// A group heading: small icon and bold label; cannot be chosen.
fn heading_row(section: &Section) -> ListBoxRow {
    let image = crate::symbole::image_named(section.icon, GROUP_ICON_SIZE);
    let label = Label::builder()
        .label(format!(
            "<b>{}</b>",
            glib::markup_escape_text(section.label)
        ))
        .use_markup(true)
        .xalign(0.0)
        .build();
    let line = GtkBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(6)
        .margin_top(6)
        .margin_bottom(2)
        .margin_start(4)
        .build();
    line.append(&image);
    line.append(&label);
    ListBoxRow::builder()
        .child(&line)
        .selectable(false)
        .activatable(false)
        .build()
}

/// An application: icon and name, indented below its heading. The row's
/// widget name is the desktop ID.
fn app_row(entry: &AppEntry) -> ListBoxRow {
    let icon = entry
        .icon
        .as_deref()
        .and_then(|icon| gio::Icon::for_string(icon).ok())
        .unwrap_or_else(|| gio::ThemedIcon::new(GENERIC_ICON).upcast());
    let image = crate::symbole::image(&icon, APP_ICON_SIZE);
    let label = Label::builder().label(&entry.name).xalign(0.0).build();
    let line = GtkBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(8)
        .margin_top(2)
        .margin_bottom(2)
        .margin_start(24)
        .valign(Align::Center)
        .build();
    line.append(&image);
    line.append(&label);
    let row = ListBoxRow::builder().child(&line).build();
    row.set_widget_name(&entry.id);
    row.set_tooltip_text(Some(&entry.id));
    row
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_menu::tests::entry;
    use crate::apps::MenuGroup;

    fn catalog(entries: Vec<AppEntry>) -> Catalog {
        let mut entries = entries;
        entries.sort_by(|a, b| (a.group, &a.name).cmp(&(b.group, &b.name)));
        Catalog {
            entries,
            excluded: Vec::new(),
        }
    }

    fn layout(sections: &[Section]) -> Vec<(&str, Vec<&str>)> {
        sections
            .iter()
            .map(|s| (s.label, s.entries.iter().map(|e| e.id.as_str()).collect()))
            .collect()
    }

    #[test]
    fn sections_follow_the_menus_settings_last() {
        let sections = sections(&catalog(vec![
            entry("firefox.desktop", "Firefox", &["Network", "WebBrowser"]),
            entry("beispiel.desktop", "Beispiel", &["Office"]),
            entry("foot.desktop", "Terminal", &["System", "TerminalEmulator"]),
            entry("files.desktop", "Dateien", &["Utility", "FileManager"]),
            entry("wlan.desktop", "Netzwerk", &["Settings", "Network"]),
            entry("drucker.desktop", "Drucker", &["Settings", "System"]),
        ]));
        assert_eq!(
            layout(&sections),
            [
                ("Zubehör", vec!["files.desktop"]),
                ("Internet", vec!["firefox.desktop"]),
                ("Büro", vec!["beispiel.desktop"]),
                ("Systemwerkzeuge", vec!["foot.desktop"]),
                ("Einstellungen", vec!["wlan.desktop"]),
                ("Systemverwaltung", vec!["drucker.desktop"]),
            ]
        );
        assert_eq!(sections[1].icon, MenuGroup::Internet.icon());
        assert_eq!(sections[4].icon, "preferences-desktop");
        assert_eq!(sections[5].icon, "preferences-system");
    }

    #[test]
    fn a_group_of_settings_only_is_no_application_heading() {
        let sections = sections(&catalog(vec![entry(
            "wlan.desktop",
            "Netzwerk",
            &["Settings", "Network"],
        )]));
        assert_eq!(layout(&sections), [("Einstellungen", vec!["wlan.desktop"])]);
    }

    #[test]
    fn search_finds_name_or_id_and_headings_follow_their_entries() {
        let firefox = entry("firefox_firefox.desktop", "Firefox", &["Network"]);
        let beispiel = entry("org.example.beispiel.desktop", "Beispiel", &["Office"]);
        assert!(matches(&firefox, ""));
        assert!(matches(&firefox, " FIRE "));
        assert!(matches(&beispiel, "example"));
        assert!(!matches(&beispiel, "fire"));
        let rows = vec![
            Row::Heading(0),
            Row::App(0, firefox),
            Row::Heading(1),
            Row::App(1, beispiel),
        ];
        let shown = |query| (0..4).map(|i| visible(&rows, i, query)).collect::<Vec<_>>();
        assert_eq!(shown("beispiel"), [false, false, true, true]);
        assert_eq!(shown(""), [true, true, true, true]);
        assert_eq!(shown("nichts"), [false, false, false, false]);
    }
}
