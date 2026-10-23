//! Desktop 0.3.0 (E2): the questions of a running job, in the words of
//! Nautilus 2.30.
//!
//! - A name that is taken: "Datei „x“ ersetzen?" (or "Ordner „x“
//!   zusammenführen?") with size and date of both, a new name to choose,
//!   "Diese Aktion auf alle Dateien anwenden" and Abbrechen, Überspringen,
//!   Umbenennen, Ersetzen (Zusammenführen). **Überspringen is the default**
//!   (Enter); Escape cancels the whole job. Nothing is replaced without a
//!   click on Ersetzen.
//! - A place without a trash: "endgültig löschen?" with Abbrechen as the
//!   default.
//!
//! The answer goes back to the job (`Reply`); a job cancelled meanwhile
//! closes its question (`schliessen`).

use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Align, EventControllerKey, Orientation, gdk, glib};
use querencia_dateiarbeit::auftraege::{Answer, Answered, Conflict, FileFacts, JobId, Reply};
use querencia_dateiarbeit::namen;
use querencia_oberflaeche::symbole;

use crate::i18n::{gettext, gettext_f};

/// The texts of a question: (title of the window, first line, explanation).
pub fn texts(conflict: &Conflict, folder: &str) -> (String, String, String) {
    match conflict {
        Conflict::Exists { target, .. } if target.is_dir => (
            gettext("File conflict"),
            gettext_f("Replace folder “{name}”?", &[("name", &target.name)]),
            format!(
                "{}\n{}",
                gettext_f(
                    "Another folder with the same name already exists in “{folder}”.",
                    &[("folder", folder)]
                ),
                gettext("Replacing it will remove all files in the folder.")
            ),
        ),
        Conflict::Exists { target, .. } => (
            gettext("File conflict"),
            gettext_f("Replace file “{name}”?", &[("name", &target.name)]),
            format!(
                "{}\n{}",
                gettext_f(
                    "Another file with the same name already exists in “{folder}”.",
                    &[("folder", folder)]
                ),
                gettext("Replacing it will overwrite its content.")
            ),
        ),
        Conflict::FolderMerge { target, .. } => (
            gettext("File conflict"),
            gettext_f("Merge folder “{name}”?", &[("name", &target.name)]),
            format!(
                "{}\n{}",
                gettext_f(
                    "Another folder with the same name already exists in “{folder}”.",
                    &[("folder", folder)]
                ),
                gettext(
                    "Merging will ask for confirmation before replacing any files in the \
                     folder that conflict with the files being copied."
                )
            ),
        ),
        Conflict::NoTrash { file } => (
            gettext("No trash"),
            gettext_f(
                "Cannot move “{name}” to the trash. Do you want to delete it immediately?",
                &[("name", &file.name)],
            ),
            gettext("This place has no trash. Deleted items cannot be restored."),
        ),
        Conflict::Fehler { file, why } => (
            gettext("Error"),
            gettext_f("“{name}” could not be handled.", &[("name", &file.name)]),
            why.clone(),
        ),
    }
}

/// "Größe: … · Zuletzt geändert: …" of one side of a conflict.
pub fn facts_line(facts: &FileFacts) -> String {
    let mut parts = Vec::new();
    if !facts.is_dir {
        parts.push(gettext_f(
            "Size: {size}",
            &[("size", &glib::format_size(facts.size))],
        ));
    }
    if let Some(when) = facts
        .modified
        .and_then(|t| glib::DateTime::from_unix_local(t).ok())
        // TRANSLATORS: date and time of the last change (strftime
        // format of GLib's GDateTime)
        .and_then(|t| t.format(&gettext("%Y-%m-%d %H:%M")).ok())
    {
        parts.push(gettext_f("Last modified: {when}", &[("when", &when)]));
    }
    parts.join(" · ")
}

/// For the log of the isolated test.
pub fn answer_name(answer: &Answer) -> String {
    match answer {
        Answer::Replace => "Ersetzen".to_owned(),
        Answer::Skip => "Überspringen".to_owned(),
        Answer::Rename(name) => format!("Umbenennen in {name}"),
        Answer::Merge => "Zusammenführen".to_owned(),
        Answer::Delete => "Löschen".to_owned(),
        Answer::Retry => "Wiederholen".to_owned(),
        Answer::Cancel => "Abbrechen".to_owned(),
    }
}

/// Open questions, by job.
#[derive(Default)]
pub struct Fragen(RefCell<Vec<(JobId, gtk4::Window)>>);

impl Fragen {
    /// The job ended (or was cancelled): its question goes.
    pub fn schliessen(&self, id: JobId) {
        let windows: Vec<gtk4::Window> = {
            let mut open = self.0.borrow_mut();
            let (gone, keep): (Vec<_>, Vec<_>) = open.drain(..).partition(|(j, _)| *j == id);
            *open = keep;
            gone.into_iter().map(|(_, w)| w).collect()
        };
        for window in windows {
            super::log(&format!("Frage zu Auftrag {id} geschlossen"));
            window.destroy();
        }
    }

    /// Asks `conflict` for job `id`; the answer goes to `reply`, then
    /// `beantwortet` runs. `folder`: the name of the target folder.
    pub fn fragen(
        self: &Rc<Self>,
        id: JobId,
        conflict: &Conflict,
        reply: Reply,
        folder: &str,
        beantwortet: impl Fn() + 'static,
    ) {
        let (title, primary, secondary) = texts(conflict, folder);
        super::log(&format!("Frage zu Auftrag {id}: {primary}"));
        let window = gtk4::Window::builder()
            .title(title)
            .resizable(false)
            .build();
        window.add_css_class("querencia-desktop-question");
        let outer = gtk4::Box::new(Orientation::Vertical, 12);
        outer.set_margin_top(12);
        outer.set_margin_bottom(12);
        outer.set_margin_start(12);
        outer.set_margin_end(12);
        let head = gtk4::Box::new(Orientation::Horizontal, 12);
        let icon = symbole::image_named("dialog-warning", 48);
        icon.set_valign(Align::Start);
        let texts_box = gtk4::Box::new(Orientation::Vertical, 6);
        let first = gtk4::Label::new(None);
        first.set_markup(&format!(
            "<b><big>{}</big></b>",
            glib::markup_escape_text(&primary)
        ));
        first.set_xalign(0.0);
        first.set_wrap(true);
        first.set_max_width_chars(50);
        let second = gtk4::Label::new(Some(&secondary));
        second.set_xalign(0.0);
        second.set_wrap(true);
        second.set_max_width_chars(60);
        texts_box.append(&first);
        texts_box.append(&second);
        head.append(&icon);
        head.append(&texts_box);
        outer.append(&head);

        // both sides of a taken name, and a new name of one's own choice
        let (sides, vorschlag, target_name) = match conflict {
            Conflict::Exists {
                source,
                target,
                vorschlag,
            }
            | Conflict::FolderMerge {
                source,
                target,
                vorschlag,
            } => (
                Some((source.clone(), target.clone())),
                Some(vorschlag.clone()),
                target.name.clone(),
            ),
            Conflict::NoTrash { file } | Conflict::Fehler { file, .. } => {
                (None, None, file.name.clone())
            }
        };
        if let Some((source, target)) = &sides {
            let grid = gtk4::Grid::builder()
                .row_spacing(4)
                .column_spacing(12)
                .build();
            for (row, (label, facts)) in [
                (gettext("Original file"), target),
                (gettext("Replace with"), source),
            ]
            .iter()
            .enumerate()
            {
                let row = i32::try_from(row).unwrap_or(0);
                let key = gtk4::Label::new(None);
                key.set_markup(&format!("<b>{}</b>", glib::markup_escape_text(label)));
                key.set_xalign(0.0);
                let value = gtk4::Label::new(Some(&facts_line(facts)));
                value.set_xalign(0.0);
                grid.attach(&key, 0, row, 1, 1);
                grid.attach(&value, 1, row, 1, 1);
            }
            outer.append(&grid);
        }
        let entry = gtk4::Entry::new();
        if let Some(vorschlag) = &vorschlag {
            let expander =
                gtk4::Expander::with_mnemonic(&gettext("Select a new _name for the destination"));
            entry.set_text(vorschlag);
            entry.set_width_chars(40);
            expander.set_child(Some(&entry));
            outer.append(&expander);
        }
        let all = gtk4::CheckButton::with_mnemonic(&gettext("Apply this action to all _files"));
        // a single object has nothing else to apply it to: still offered, as
        // the job may hold more conflicts in a folder
        outer.append(&all);

        let buttons = gtk4::Box::new(Orientation::Horizontal, 6);
        buttons.set_halign(Align::End);
        let mut choices: Vec<(gtk4::Button, Answer)> = vec![(
            gtk4::Button::with_mnemonic(&gettext("_Cancel")),
            Answer::Cancel,
        )];
        match conflict {
            Conflict::Exists { .. } => {
                choices.push((gtk4::Button::with_mnemonic(&gettext("_Skip")), Answer::Skip));
                choices.push((
                    gtk4::Button::with_mnemonic(&gettext("Re_name")),
                    Answer::Rename(String::new()),
                ));
                choices.push((
                    gtk4::Button::with_mnemonic(&gettext("_Replace")),
                    Answer::Replace,
                ));
            }
            Conflict::FolderMerge { .. } => {
                choices.push((gtk4::Button::with_mnemonic(&gettext("_Skip")), Answer::Skip));
                choices.push((
                    gtk4::Button::with_mnemonic(&gettext("Re_name")),
                    Answer::Rename(String::new()),
                ));
                choices.push((
                    gtk4::Button::with_mnemonic(&gettext("_Replace")),
                    Answer::Replace,
                ));
                choices.push((
                    gtk4::Button::with_mnemonic(&gettext("_Merge")),
                    Answer::Merge,
                ));
            }
            Conflict::NoTrash { .. } => {
                choices.push((gtk4::Button::with_mnemonic(&gettext("_Skip")), Answer::Skip));
                choices.push((
                    gtk4::Button::with_mnemonic(&gettext("_Delete")),
                    Answer::Delete,
                ));
            }
            Conflict::Fehler { .. } => {
                choices.push((gtk4::Button::with_mnemonic(&gettext("_Skip")), Answer::Skip));
                choices.push((
                    gtk4::Button::with_mnemonic(&gettext("_Retry")),
                    Answer::Retry,
                ));
            }
        }
        // E2: Überspringen is the default; without a trash, Abbrechen
        let default = match conflict {
            Conflict::NoTrash { .. } => Answer::Cancel,
            _ => Answer::Skip,
        };
        let answered: Rc<RefCell<Option<Reply>>> = Rc::new(RefCell::new(Some(reply)));
        let send = {
            let answered = answered.clone();
            let window = window.downgrade();
            let entry = entry.clone();
            let all = all.clone();
            let this = Rc::downgrade(self);
            Rc::new(move |answer: Answer| {
                let Some(reply) = answered.borrow_mut().take() else {
                    return;
                };
                let answer = match answer {
                    Answer::Rename(_) => Answer::Rename(entry.text().to_string()),
                    other => other,
                };
                let for_all = all.is_active();
                super::log(&format!(
                    "Antwort zu Auftrag {id}: {}{}",
                    answer_name(&answer),
                    if for_all { " (für alle)" } else { "" }
                ));
                reply.send(Answered { answer, for_all });
                beantwortet();
                if let Some(this) = this.upgrade() {
                    this.0.borrow_mut().retain(|(j, _)| *j != id);
                }
                if let Some(window) = window.upgrade() {
                    window.destroy();
                }
            })
        };
        // "Umbenennen" only with a usable name that differs from the taken one
        if let Some((button, _)) = choices.iter().find(|(_, a)| matches!(a, Answer::Rename(_))) {
            let button = button.clone();
            let check = move |entry: &gtk4::Entry| {
                let name = entry.text();
                button.set_sensitive(
                    name.as_str() != target_name && namen::check_file_name(&name).is_ok(),
                );
            };
            check(&entry);
            entry.connect_changed(check);
        }
        let mut default_button = None;
        for (button, answer) in choices {
            let send = send.clone();
            let chosen = answer.clone();
            button.connect_clicked(move |_| send(chosen.clone()));
            if answer == default {
                default_button = Some(button.clone());
            }
            buttons.append(&button);
        }
        outer.append(&buttons);
        window.set_child(Some(&outer));
        if let Some(button) = &default_button {
            window.set_default_widget(Some(button));
        }
        // Escape and closing the window cancel the job, as on GNOME 2
        let keys = EventControllerKey::new();
        let cancel = send.clone();
        keys.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Escape {
                cancel(Answer::Cancel);
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        window.add_controller(keys);
        let close = send.clone();
        window.connect_close_request(move |_| {
            close(Answer::Cancel);
            glib::Propagation::Proceed
        });
        window.present();
        if let Some(button) = default_button {
            button.grab_focus();
        }
        self.0.borrow_mut().push((id, window));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(name: &str, is_dir: bool) -> FileFacts {
        FileFacts {
            uri: format!("file:///d/{name}"),
            name: name.to_owned(),
            is_dir,
            size: 1234,
            modified: None,
        }
    }

    #[test]
    fn the_question_names_file_or_folder() {
        let exists = Conflict::Exists {
            source: file("notiz.txt", false),
            target: file("notiz.txt", false),
            vorschlag: "notiz (copy).txt".to_owned(),
        };
        let (title, first, second) = texts(&exists, "Schreibtisch");
        assert_eq!(title, "File conflict");
        assert_eq!(first, "Replace file “notiz.txt”?");
        assert!(
            second.starts_with("Another file with the same name already exists in “Schreibtisch”.")
        );
        let merge = Conflict::FolderMerge {
            source: file("Ablage", true),
            target: file("Ablage", true),
            vorschlag: "Ablage (copy)".to_owned(),
        };
        assert_eq!(texts(&merge, "x").1, "Merge folder “Ablage”?");
        let folder = Conflict::Exists {
            source: file("a", false),
            target: file("a", true),
            vorschlag: String::new(),
        };
        assert_eq!(texts(&folder, "x").1, "Replace folder “a”?");
        let no_trash = Conflict::NoTrash {
            file: file("b", false),
        };
        assert!(texts(&no_trash, "x").1.contains("delete it immediately"));
    }

    #[test]
    fn sizes_only_for_files() {
        assert_eq!(facts_line(&file("a", false)), "Size: 1.2\u{a0}kB");
        assert_eq!(facts_line(&file("d", true)), "");
        assert_eq!(
            answer_name(&Answer::Rename("n.txt".into())),
            "Umbenennen in n.txt"
        );
    }
}
