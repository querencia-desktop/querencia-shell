//! Desktop 0.2.3 (QD-108): files on the desktop work with other programs,
//! as on a GNOME 2 desktop.
//!
//! - Ausschneiden, Kopieren, Einfügen (menu and Strg+X/C/V): the clipboard
//!   holds a `GdkFileList`, `text/uri-list` and GNOME's
//!   `x-special/gnome-copied-files` (with KDE's cut marker), as Querencia
//!   Files writes it; pasting reads the same from any program. Cut files
//!   are moved when pasted and look faded until then.
//! - In den Papierkorb verschieben (menu, Entf, a drop on the trash icon);
//!   Umschalt+Entf deletes for good after a question.
//! - Dragging: desktop files are a `GdkFileList` (and `text/uri-list`) for
//!   every program (Files, file dialogs, editors). Files dropped on the
//!   desktop are copied into the desktop folder (moved when the source
//!   offers only moving, Umschalt in Files); a drop on a folder icon puts
//!   them into it, on the trash icon into the trash. Inside the desktop a
//!   drop on free space only moves the icons, as before (HMP-10).
//! - Every operation runs through GIO off the main thread (`fileops`); what
//!   fails is shown in a dialog, not only logged. The desktop folder's
//!   monitor shows the result.
//! - Öffnen mit (the programs for the type, "Andere Anwendung …" with GTK's
//!   app chooser) and Eigenschaften (name, type, size, place, date).

use super::*;
use crate::fileops::{self, Op};

/// The desktop's own drag of icons (HMP-10 moves, now a real drag so that
/// files can leave the desktop).
pub(super) struct OwnDrag {
    /// The icon under the pointer first, then the rest of the selection.
    members: Vec<usize>,
    /// Pointer position inside the first icon.
    grab: (f64, f64),
    /// Translucent copies on the desktop while the pointer is over it.
    ghosts: Vec<(usize, gtk4::Box)>,
    drag: Option<gdk::Drag>,
    /// The first icon's picture, following the pointer outside the desktop
    /// and over a folder or the trash; hidden over free desktop, where the
    /// copies show the place.
    picture: Option<gdk::Paintable>,
    hotspot: (i32, i32),
    picture_shown: Cell<Option<bool>>,
}

impl OwnDrag {
    fn show_picture(&self, show: bool) {
        if self.picture_shown.get() == Some(show) {
            return;
        }
        let Some(drag) = &self.drag else { return };
        match (&self.picture, show) {
            (Some(picture), true) => {
                gtk4::DragIcon::set_from_paintable(drag, picture, self.hotspot.0, self.hotspot.1);
            }
            _ => gtk4::DragIcon::set_from_paintable(drag, &gdk::Paintable::new_empty(1, 1), 0, 0),
        }
        self.picture_shown.set(Some(show));
    }

    fn show_ghosts(&self, show: bool) {
        for (_, ghost) in &self.ghosts {
            ghost.set_visible(show);
        }
    }
}

/// What the clipboard holds while the desktop owns it.
pub(super) struct Clip {
    provider: gdk::ContentProvider,
    cut: bool,
    paths: Vec<PathBuf>,
}

/// Where a drop at a point goes.
#[derive(Clone, Debug, PartialEq)]
enum DropOn {
    /// Free space or an icon that takes nothing: the desktop folder.
    Desktop,
    /// A folder icon (or Persönlicher Ordner): into that folder.
    Folder(usize, PathBuf),
    Trash(usize),
}

impl DropOn {
    fn index(&self) -> Option<usize> {
        match self {
            DropOn::Desktop => None,
            DropOn::Folder(i, _) | DropOn::Trash(i) => Some(*i),
        }
    }
}

/// The file behind an icon, for icons that are files.
pub(super) fn item_path(item: &Item) -> Option<&Path> {
    match &item.kind {
        Kind::File(path) | Kind::Folder(path) | Kind::Launcher(path) => Some(path),
        _ => None,
    }
}

fn uri_of(path: &Path) -> String {
    gio::File::for_path(path).uri().to_string()
}

/// `GdkFileList`, `text/uri-list` and, for the clipboard (`cut` set),
/// GNOME's copied-files text and KDE's cut marker.
fn file_content(paths: &[PathBuf], cut: Option<bool>) -> gdk::ContentProvider {
    let files: Vec<gio::File> = paths.iter().map(gio::File::for_path).collect();
    let uris: Vec<String> = files.iter().map(|f| f.uri().to_string()).collect();
    let mut providers = vec![
        gdk::ContentProvider::for_value(&gdk::FileList::from_array(&files).to_value()),
        gdk::ContentProvider::for_bytes(
            fileops::URI_LIST,
            &glib::Bytes::from_owned(fileops::uri_list(&uris).into_bytes()),
        ),
    ];
    if let Some(cut) = cut {
        providers.push(gdk::ContentProvider::for_bytes(
            fileops::GNOME_COPIED_FILES,
            &glib::Bytes::from_owned(fileops::gnome_copied_files(cut, &uris).into_bytes()),
        ));
        if cut {
            providers.push(gdk::ContentProvider::for_bytes(
                fileops::KDE_CUT_SELECTION,
                &glib::Bytes::from_static(b"1"),
            ));
        }
    }
    gdk::ContentProvider::new_union(&providers)
}

/// The content type a program is chosen for: an empty file is the type its
/// name suggests (Desktop 0.2.2), a folder `inode/directory`.
fn content_type_of(path: &Path) -> Option<String> {
    let found = gio::File::for_path(path)
        .query_info(
            "standard::content-type",
            gio::FileQueryInfoFlags::NONE,
            gio::Cancellable::NONE,
        )
        .ok()
        .and_then(|info| info.content_type())
        .map(|t| t.to_string());
    if icons::is_empty_document(found.as_deref()) {
        return path
            .file_name()
            .and_then(|n| icons::type_from_name(&n.to_string_lossy()))
            .or(found);
    }
    found
}

async fn read_all(stream: gio::InputStream) -> Option<glib::Bytes> {
    let output = gio::MemoryOutputStream::new_resizable();
    output
        .splice_future(
            &stream,
            gio::OutputStreamSpliceFlags::CLOSE_SOURCE | gio::OutputStreamSpliceFlags::CLOSE_TARGET,
            glib::Priority::DEFAULT,
        )
        .await
        .ok()?;
    Some(output.steal_as_bytes())
}

async fn clipboard_bytes(clipboard: &gdk::Clipboard, mime: &str) -> Option<glib::Bytes> {
    let (stream, _) = clipboard
        .read_future(&[mime], glib::Priority::DEFAULT)
        .await
        .ok()?;
    read_all(stream).await
}

/// Files on the clipboard of any program: (cut, URIs). GNOME's text first
/// (it says whether the files were cut), then a `GdkFileList`, then a plain
/// URI list (with KDE's cut marker).
async fn clipboard_files(clipboard: &gdk::Clipboard) -> Option<(bool, Vec<String>)> {
    let formats = clipboard.formats();
    if formats.contain_mime_type(fileops::GNOME_COPIED_FILES)
        && let Some(found) = clipboard_bytes(clipboard, fileops::GNOME_COPIED_FILES)
            .await
            .and_then(|bytes| fileops::parse_gnome_copied_files(&bytes))
    {
        return Some(found);
    }
    let kde_cut = async {
        formats.contain_mime_type(fileops::KDE_CUT_SELECTION)
            && clipboard_bytes(clipboard, fileops::KDE_CUT_SELECTION)
                .await
                .is_some_and(|b| b.as_ref() == b"1")
    };
    if formats.contains_type(gdk::FileList::static_type())
        && let Ok(value) = clipboard
            .read_value_future(gdk::FileList::static_type(), glib::Priority::DEFAULT)
            .await
        && let Ok(list) = value.get::<gdk::FileList>()
    {
        let uris: Vec<String> = list.files().iter().map(|f| f.uri().to_string()).collect();
        if !uris.is_empty() {
            return Some((kde_cut.await, uris));
        }
    }
    if formats.contain_mime_type(fileops::URI_LIST)
        && let Some(bytes) = clipboard_bytes(clipboard, fileops::URI_LIST).await
    {
        let uris = fileops::parse_uri_list(&String::from_utf8_lossy(&bytes));
        if !uris.is_empty() {
            return Some((kde_cut.await, uris));
        }
    }
    None
}

/// Files of a drop from another program: a `GdkFileList`, else a URI list.
async fn dropped_files(drop: &gdk::Drop) -> Vec<String> {
    if drop.formats().contains_type(gdk::FileList::static_type())
        && let Ok(value) = drop
            .read_value_future(gdk::FileList::static_type(), glib::Priority::DEFAULT)
            .await
        && let Ok(list) = value.get::<gdk::FileList>()
    {
        return list.files().iter().map(|f| f.uri().to_string()).collect();
    }
    if drop.formats().contain_mime_type(fileops::URI_LIST)
        && let Ok((stream, _)) = drop
            .read_future(&[fileops::URI_LIST], glib::Priority::DEFAULT)
            .await
        && let Some(bytes) = read_all(stream).await
    {
        return fileops::parse_uri_list(&String::from_utf8_lossy(&bytes));
    }
    Vec::new()
}

/// One copy, move, trash or delete.
struct Job {
    op: Op,
    sources: Vec<String>,
    /// The folder for copy and move.
    target: Option<PathBuf>,
    /// Desktop: where the first arrived icon goes (a drop, a paste from
    /// the desktop menu).
    at: Option<Point>,
    /// Cut files were pasted: the clipboard is emptied afterwards.
    clear_clipboard: bool,
}

impl Icons {
    /// The selected icons that are files, folders or starters.
    pub(super) fn file_items(&self) -> Vec<Item> {
        let items = self.items.borrow();
        self.selection_list()
            .into_iter()
            .filter_map(|i| items.get(i))
            .filter(|item| item_path(item).is_some())
            .cloned()
            .collect()
    }

    /// Whether the clipboard holds files (for "Einfügen").
    pub(super) fn clipboard_has_files(&self) -> bool {
        let formats = self.window.clipboard().formats();
        formats.contains_type(gdk::FileList::static_type())
            || formats.contain_mime_type(fileops::URI_LIST)
            || formats.contain_mime_type(fileops::GNOME_COPIED_FILES)
    }

    /// Ausschneiden (`cut`) or Kopieren: the selected files go onto the
    /// clipboard.
    pub(super) fn put_on_clipboard(self: &Rc<Self>, cut: bool) {
        let paths: Vec<PathBuf> = self
            .file_items()
            .iter()
            .filter_map(|item| item_path(item).map(Path::to_path_buf))
            .collect();
        if paths.is_empty() {
            log("Zwischenablage: keine Datei ausgewählt");
            return;
        }
        let provider = file_content(&paths, Some(cut));
        match self.window.clipboard().set_content(Some(&provider)) {
            Ok(()) => {
                log(&format!(
                    "{}: {}",
                    if cut { "ausgeschnitten" } else { "kopiert" },
                    names_of(&paths)
                ));
                *self.clip.borrow_mut() = Some(Clip {
                    provider,
                    cut,
                    paths,
                });
                self.show_cut();
            }
            Err(err) => log(&format!("Zwischenablage nicht gesetzt: {err}")),
        }
    }

    /// Follows the clipboard: when another program takes it, the faded cut
    /// icons look normal again.
    pub(super) fn watch_clipboard(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.window.clipboard().connect_changed(move |clipboard| {
            let Some(icons) = weak.upgrade() else { return };
            let gone = icons
                .clip
                .borrow()
                .as_ref()
                .is_some_and(|clip| clipboard.content().as_ref() != Some(&clip.provider));
            if gone {
                icons.clip.borrow_mut().take();
                icons.show_cut();
            }
        });
    }

    /// Fades the icons of cut files (GNOME 2 did the same).
    pub(super) fn show_cut(&self) {
        let cut: Vec<PathBuf> = self
            .clip
            .borrow()
            .as_ref()
            .filter(|clip| clip.cut)
            .map(|clip| clip.paths.clone())
            .unwrap_or_default();
        for (item, widget) in self.items.borrow().iter().zip(self.widgets.borrow().iter()) {
            if item_path(item).is_some_and(|p| cut.iter().any(|c| c == p)) {
                widget.add_css_class("cut");
            } else {
                widget.remove_css_class("cut");
            }
        }
    }

    /// Einfügen: the clipboard's files into `into` (the desktop folder when
    /// `None`); `at`: where the first new icon goes.
    pub(super) fn paste(self: &Rc<Self>, into: Option<PathBuf>, at: Option<Point>) {
        let target = match into {
            Some(dir) => dir,
            None => match self.ensure_desktop_dir() {
                Some(dir) => dir,
                None => return,
            },
        };
        let clipboard = self.window.clipboard();
        let own = self
            .clip
            .borrow()
            .as_ref()
            .filter(|clip| clipboard.content().as_ref() == Some(&clip.provider))
            .map(|clip| (clip.cut, clip.paths.iter().map(|p| uri_of(p)).collect()));
        if let Some((cut, sources)) = own {
            self.start_job(Job {
                op: if cut { Op::Move } else { Op::Copy },
                sources,
                target: Some(target),
                at,
                clear_clipboard: cut,
            });
            return;
        }
        let weak = Rc::downgrade(self);
        glib::MainContext::default().spawn_local(async move {
            let found = clipboard_files(&clipboard).await;
            let Some(icons) = weak.upgrade() else { return };
            match found {
                Some((cut, sources)) => icons.start_job(Job {
                    op: if cut { Op::Move } else { Op::Copy },
                    sources,
                    target: Some(target),
                    at,
                    clear_clipboard: cut,
                }),
                None => log("Einfügen: keine Dateien in der Zwischenablage"),
            }
        });
    }

    /// In den Papierkorb verschieben (menu, Entf).
    pub(super) fn trash_selection(self: &Rc<Self>) {
        let sources: Vec<String> = self
            .file_items()
            .iter()
            .filter_map(|item| item_path(item).map(uri_of))
            .collect();
        if sources.is_empty() {
            log("Papierkorb: keine Datei ausgewählt");
            return;
        }
        self.start_job(Job {
            op: Op::Trash,
            sources,
            target: None,
            at: None,
            clear_clipboard: false,
        });
    }

    /// Umschalt+Entf: asks first ("Abbrechen" is the default, Escape
    /// cancels); only "Löschen" deletes.
    pub(super) fn confirm_delete(self: &Rc<Self>) {
        let items = self.file_items();
        let sources: Vec<String> = items
            .iter()
            .filter_map(|item| item_path(item).map(uri_of))
            .collect();
        if sources.is_empty() {
            return;
        }
        let message = match items.as_slice() {
            [one] => format!("Möchten Sie „{}“ wirklich endgültig löschen?", one.name),
            more => format!(
                "Möchten Sie die {} ausgewählten Objekte wirklich endgültig löschen?",
                more.len()
            ),
        };
        let dialog = gtk4::AlertDialog::builder()
            .message(message)
            .detail("Wenn Sie ein Objekt löschen, ist es unwiderruflich verloren.")
            .buttons(["Abbrechen", "Löschen"])
            .cancel_button(0)
            .default_button(0)
            .modal(false)
            .build();
        let weak = Rc::downgrade(self);
        dialog.choose(
            None::<&gtk4::Window>,
            gio::Cancellable::NONE,
            move |answer| match answer {
                Ok(1) => {
                    if let Some(icons) = weak.upgrade() {
                        icons.start_job(Job {
                            op: Op::Delete,
                            sources,
                            target: None,
                            at: None,
                            clear_clipboard: false,
                        });
                    }
                }
                _ => log("endgültig löschen abgebrochen"),
            },
        );
    }

    /// Runs `job` off the main thread; afterwards shows what failed, places
    /// dropped or pasted icons and refreshes.
    fn start_job(self: &Rc<Self>, job: Job) {
        let Job {
            op,
            sources,
            target,
            at,
            clear_clipboard,
        } = job;
        log(&format!(
            "{}: {}{}",
            op.verb(),
            sources
                .iter()
                .map(|uri| gio::File::for_uri(uri)
                    .basename()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| uri.clone()))
                .collect::<Vec<_>>()
                .join(", "),
            target
                .as_ref()
                .map_or(String::new(), |t| format!(" → {}", t.display()))
        ));
        let target_uri = target.as_deref().map(uri_of);
        let handle = gio::spawn_blocking(move || fileops::run(op, &sources, target_uri.as_deref()));
        let weak = Rc::downgrade(self);
        glib::MainContext::default().spawn_local(async move {
            let Ok(outcome) = handle.await else {
                log("Dateioperation abgebrochen");
                return;
            };
            if let Some(icons) = weak.upgrade() {
                icons.job_done(op, &outcome, target.as_deref(), at, clear_clipboard);
            }
        });
    }

    fn job_done(
        self: &Rc<Self>,
        op: Op,
        outcome: &fileops::Outcome,
        target: Option<&Path>,
        at: Option<Point>,
        clear_clipboard: bool,
    ) {
        log(&format!(
            "{} fertig: {} erledigt, {} nicht{}",
            op.verb(),
            outcome.done,
            outcome.failed.len(),
            if outcome.arrived.is_empty() {
                String::new()
            } else {
                format!(" (angekommen: {})", outcome.arrived.join(", "))
            }
        ));
        for (name, why) in &outcome.failed {
            log(&format!("nicht {}: {name}: {why}", op.verb()));
        }
        if clear_clipboard && outcome.done > 0 {
            // cut files are where they belong now: nothing left to paste
            if let Err(err) = self
                .window
                .clipboard()
                .set_content(None::<&gdk::ContentProvider>)
            {
                log(&format!("Zwischenablage nicht geleert: {err}"));
            }
            self.clip.borrow_mut().take();
        }
        let on_desktop = target.is_some_and(|t| desktop_dir().is_some_and(|d| d == t));
        match at {
            Some(at) if on_desktop && !outcome.arrived.is_empty() => {
                self.place_arrived(&outcome.arrived, at);
            }
            _ => self.reload(),
        }
        if op == Op::Trash {
            self.update_trash_soon();
        }
        if !outcome.failed.is_empty() {
            show_error(op.failure_title(), &fileops::failure_text(op, outcome));
        }
    }

    /// Icons that just arrived in the desktop folder go to `at` (the first)
    /// and the nearest free spots after it, as GNOME 2 put dropped files.
    fn place_arrived(self: &Rc<Self>, names: &[String], at: Point) {
        self.reload();
        if !self.geometry.get().known() || self.reload_waiting.get() {
            return;
        }
        let geometry = self.geometry.get();
        let mut positions = self.positions.borrow().clone();
        let mut moved = Vec::new();
        let mut want = at;
        for name in names {
            let index = self
                .items
                .borrow()
                .iter()
                .position(|item| item_path(item).is_some() && item.ident() == *name);
            let Some(index) = index else { continue };
            let others: Vec<Point> = positions
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != index)
                .map(|(_, p)| *p)
                .collect();
            if let Some(spot) = geometry.nearest_free(want, &others) {
                positions[index] = spot;
                moved.push((index, spot));
                want = spot;
            }
        }
        if self.remember_moved(&moved) {
            log(&format!(
                "abgelegt bei {},{}: {}",
                at.0,
                at.1,
                names.join(", ")
            ));
            self.layout();
            self.save();
        }
    }

    /// Where a drop at `(x, y)` goes; the icons of the own drag (`exclude`)
    /// take nothing.
    fn drop_on(&self, x: f64, y: f64, exclude: &[usize]) -> DropOn {
        let Some(index) = self.hit(x, y).filter(|i| !exclude.contains(i)) else {
            return DropOn::Desktop;
        };
        match self.items.borrow().get(index).map(|item| &item.kind) {
            Some(Kind::Folder(path)) => DropOn::Folder(index, path.clone()),
            Some(Kind::Home) => DropOn::Folder(index, glib::home_dir()),
            Some(Kind::Trash) => DropOn::Trash(index),
            _ => DropOn::Desktop,
        }
    }

    /// Marks the icon a drop would go into (as selected), or none.
    fn highlight(&self, index: Option<usize>) {
        if self.drop_over.get() == index {
            return;
        }
        let widgets = self.widgets.borrow();
        if let Some(old) = self.drop_over.get().and_then(|i| widgets.get(i)) {
            old.remove_css_class("drop-target");
        }
        if let Some(new) = index.and_then(|i| widgets.get(i)) {
            new.add_css_class("drop-target");
        }
        self.drop_over.set(index);
    }

    /// Dragging icons (HMP-10) as a real drag: desktop files leave the
    /// desktop as a file list, other programs' files come in.
    pub(super) fn install_dnd(self: &Rc<Self>) {
        let source = gtk4::DragSource::new();
        source.set_button(gdk::BUTTON_PRIMARY);
        let weak = Rc::downgrade(self);
        source.connect_prepare(move |source, x, y| {
            let icons = weak.upgrade()?;
            icons.prepare_drag(source, x, y)
        });
        let weak = Rc::downgrade(self);
        source.connect_drag_begin(move |_, drag| {
            if let Some(icons) = weak.upgrade() {
                icons.begin_drag(drag);
            }
        });
        let weak = Rc::downgrade(self);
        source.connect_drag_end(move |_, drag, _| {
            if let Some(icons) = weak.upgrade() {
                icons.end_drag(drag);
            }
        });
        self.fixed.add_controller(source);

        let formats = gdk::ContentFormatsBuilder::new()
            .add_type(gdk::FileList::static_type())
            .add_mime_type(fileops::URI_LIST)
            .add_mime_type(fileops::OWN_ICONS)
            .build();
        let target = gtk4::DropTargetAsync::new(
            Some(formats),
            gdk::DragAction::COPY | gdk::DragAction::MOVE,
        );
        let weak = Rc::downgrade(self);
        target.connect_drag_enter(move |_, drop, x, y| {
            weak.upgrade().map_or(gdk::DragAction::empty(), |icons| {
                icons.drop_motion(drop, x, y)
            })
        });
        let weak = Rc::downgrade(self);
        target.connect_drag_motion(move |_, drop, x, y| {
            weak.upgrade().map_or(gdk::DragAction::empty(), |icons| {
                icons.drop_motion(drop, x, y)
            })
        });
        let weak = Rc::downgrade(self);
        target.connect_drag_leave(move |_, _| {
            if let Some(icons) = weak.upgrade() {
                icons.drop_leave();
            }
        });
        let weak = Rc::downgrade(self);
        target.connect_drop(move |_, drop, x, y| {
            weak.upgrade()
                .is_some_and(|icons| icons.drop_files(drop, x, y))
        });
        self.fixed.add_controller(target);
    }

    fn prepare_drag(
        self: &Rc<Self>,
        source: &gtk4::DragSource,
        x: f64,
        y: f64,
    ) -> Option<gdk::ContentProvider> {
        if self.editing.borrow().is_some() || self.press_swallowed.get() {
            return None;
        }
        let index = self.hit(x, y)?;
        // the whole selection goes if the icon is part of it, else this icon
        if !self.selection.borrow().contains(&index) {
            self.select_only(Some(index));
        }
        let mut members = vec![index];
        members.extend(self.selection_list().into_iter().filter(|&m| m != index));
        let (px, py) = self.positions.borrow()[index];
        let paths: Vec<PathBuf> = {
            let items = self.items.borrow();
            members
                .iter()
                .filter_map(|&m| items.get(m))
                .filter_map(|item| item_path(item).map(Path::to_path_buf))
                .collect()
        };
        // Strg: copy only, Umschalt: move only, as in Querencia Files
        let modifiers = source.current_event_state();
        source.set_actions(if modifiers.contains(gdk::ModifierType::CONTROL_MASK) {
            gdk::DragAction::COPY
        } else if modifiers.contains(gdk::ModifierType::SHIFT_MASK) {
            gdk::DragAction::MOVE
        } else {
            gdk::DragAction::COPY | gdk::DragAction::MOVE
        });
        // no picture of GTK's own; `OwnDrag::show_picture` sets ours
        source.set_icon(Some(&gdk::Paintable::new_empty(1, 1)), 0, 0);
        let own =
            gdk::ContentProvider::for_bytes(fileops::OWN_ICONS, &glib::Bytes::from_static(b""));
        let content = if paths.is_empty() {
            own
        } else {
            gdk::ContentProvider::new_union(&[file_content(&paths, None), own])
        };
        *self.own_drag.borrow_mut() = Some(OwnDrag {
            members,
            grab: (x - f64::from(px), y - f64::from(py)),
            ghosts: Vec::new(),
            drag: None,
            picture: None,
            hotspot: (0, 0),
            picture_shown: Cell::new(None),
        });
        Some(content)
    }

    fn begin_drag(self: &Rc<Self>, drag: &gdk::Drag) {
        // the press became this drag: no rubber band, no double click
        self.drag.borrow_mut().take();
        self.drags.set(self.drags.get() + 1);
        self.collapse_on_release.set(None);
        let mut state = self.own_drag.borrow_mut();
        let Some(own) = state.as_mut() else { return };
        let items = self.items.borrow().clone();
        let size = self.settings.get().size;
        let tile = self.geometry.get().tile;
        // translucent copies on the desktop (HMP-10), shown while the
        // pointer is over it
        for &m in &own.members {
            let Some(item) = items.get(m) else { continue };
            let ghost = icon_widget(item, self.trash_full.get().unwrap_or(false), size);
            ghost.add_css_class("dragging");
            ghost.add_css_class("selected");
            ghost.set_can_target(false);
            ghost.set_visible(false);
            self.fixed.put(&ghost, 0.0, 0.0);
            own.ghosts.push((m, ghost));
        }
        // outside the desktop and over a target: the first icon's picture
        let lead = own.members[0];
        if let Some(item) = items.get(lead) {
            let icon = icon_for(item, self.trash_full.get().unwrap_or(false));
            let display = WidgetExt::display(&self.window);
            let paintable = gtk4::IconTheme::for_display(&display).lookup_by_gicon(
                &icon,
                size,
                self.window.scale_factor(),
                gtk4::TextDirection::None,
                gtk4::IconLookupFlags::empty(),
            );
            own.picture = Some(paintable.upcast());
            own.hotspot = (
                (own.grab.0 - f64::from((tile.0 - size) / 2))
                    .round()
                    .clamp(0.0, f64::from(size)) as i32,
                (own.grab.1 - 1.0).round().clamp(0.0, f64::from(size)) as i32,
            );
        }
        own.drag = Some(drag.clone());
        own.show_picture(false);
        let names: Vec<String> = own
            .members
            .iter()
            .filter_map(|&m| items.get(m).map(|item| item.name.clone()))
            .collect();
        log(&format!("Ziehen beginnt: {}", names.join(", ")));
    }

    fn end_drag(self: &Rc<Self>, drag: &gdk::Drag) {
        self.highlight(None);
        if let Some(own) = self.own_drag.borrow_mut().take() {
            for (_, ghost) in own.ghosts {
                self.fixed.remove(&ghost);
            }
        }
        let action = drag.selected_action();
        log(&format!(
            "Ziehen beendet: {}",
            if action.contains(gdk::DragAction::MOVE) {
                "verschieben"
            } else if action.contains(gdk::DragAction::COPY) {
                "kopieren"
            } else {
                "nichts"
            }
        ));
        if self.reload_waiting.get() {
            self.reload();
        }
    }

    /// A drag over the desktop: what a drop here would do.
    fn drop_motion(self: &Rc<Self>, drop: &gdk::Drop, x: f64, y: f64) -> gdk::DragAction {
        let own = drop.drag().is_some();
        let members = self
            .own_drag
            .borrow()
            .as_ref()
            .map(|d| d.members.clone())
            .unwrap_or_default();
        let on = self.drop_on(x, y, &members);
        if own && let Some(state) = self.own_drag.borrow().as_ref() {
            let geometry = self.geometry.get();
            let positions = self.positions.borrow();
            let want = (
                (x - state.grab.0).round() as i32,
                (y - state.grab.1).round() as i32,
            );
            let lead = positions[state.members[0]];
            for (m, ghost) in &state.ghosts {
                let at = geometry.clamp((
                    positions[*m].0 + want.0 - lead.0,
                    positions[*m].1 + want.1 - lead.1,
                ));
                self.fixed.move_(ghost, f64::from(at.0), f64::from(at.1));
            }
            let has_files = state.members.iter().any(|&m| {
                self.items
                    .borrow()
                    .get(m)
                    .is_some_and(|item| item_path(item).is_some())
            });
            let target = on.index().filter(|_| has_files);
            self.highlight(target);
            // over a folder or the trash the files go in: no copies on top
            state.show_ghosts(target.is_none());
            state.show_picture(target.is_some());
            return gdk::DragAction::MOVE;
        }
        self.highlight(on.index());
        match on {
            DropOn::Trash(_) => gdk::DragAction::MOVE & drop.actions(),
            _ => foreign_action(drop),
        }
    }

    fn drop_leave(&self) {
        self.highlight(None);
        if let Some(state) = self.own_drag.borrow().as_ref() {
            state.show_ghosts(false);
            state.show_picture(true);
        }
    }

    fn drop_files(self: &Rc<Self>, drop: &gdk::Drop, x: f64, y: f64) -> bool {
        self.highlight(None);
        let own = drop.drag().is_some()
            && self.own_drag.borrow().is_some()
            && drop.formats().contain_mime_type(fileops::OWN_ICONS);
        if own {
            self.drop_own(drop, x, y);
            return true;
        }
        let on = self.drop_on(x, y, &[]);
        let action = match on {
            DropOn::Trash(_) => gdk::DragAction::MOVE & drop.actions(),
            _ => foreign_action(drop),
        };
        if action.is_empty() {
            return false;
        }
        let at = ((x - 50.0).round() as i32, (y - 25.0).round() as i32);
        let weak = Rc::downgrade(self);
        let drop = drop.clone();
        glib::MainContext::default().spawn_local(async move {
            let sources = dropped_files(&drop).await;
            let Some(icons) = weak.upgrade() else {
                drop.finish(gdk::DragAction::empty());
                return;
            };
            if sources.is_empty() {
                log("Ablage ohne Dateien");
                drop.finish(gdk::DragAction::empty());
                return;
            }
            let op = match (&on, action) {
                (DropOn::Trash(_), _) => Op::Trash,
                (_, a) if a == gdk::DragAction::MOVE => Op::Move,
                _ => Op::Copy,
            };
            let target = match on {
                DropOn::Folder(_, path) => Some(path),
                DropOn::Trash(_) => None,
                DropOn::Desktop => icons.ensure_desktop_dir(),
            };
            if op != Op::Trash && target.is_none() {
                drop.finish(gdk::DragAction::empty());
                return;
            }
            let at = (op != Op::Trash).then_some(at);
            icons.start_job(Job {
                op,
                sources,
                target,
                at,
                clear_clipboard: false,
            });
            drop.finish(action);
        });
        true
    }

    /// The desktop's own icons dropped on the desktop: into a folder, into
    /// the trash, else only the icons move (HMP-10).
    fn drop_own(self: &Rc<Self>, drop: &gdk::Drop, x: f64, y: f64) {
        let Some((members, grab)) = self
            .own_drag
            .borrow()
            .as_ref()
            .map(|d| (d.members.clone(), d.grab))
        else {
            drop.finish(gdk::DragAction::empty());
            return;
        };
        let sources: Vec<String> = {
            let items = self.items.borrow();
            members
                .iter()
                .filter_map(|&m| items.get(m))
                .filter_map(|item| item_path(item).map(uri_of))
                .collect()
        };
        let on = self.drop_on(x, y, &members);
        match on {
            DropOn::Folder(_, path) if !sources.is_empty() => self.start_job(Job {
                op: Op::Move,
                sources,
                target: Some(path),
                at: None,
                clear_clipboard: false,
            }),
            DropOn::Trash(_) if !sources.is_empty() => self.start_job(Job {
                op: Op::Trash,
                sources,
                target: None,
                at: None,
                clear_clipboard: false,
            }),
            _ => {
                let want = ((x - grab.0).round() as i32, (y - grab.1).round() as i32);
                if members.len() == 1 {
                    self.drop_icon(members[0], want);
                } else {
                    self.drop_group(&members, want);
                }
            }
        }
        drop.finish(gdk::DragAction::MOVE);
    }

    /// Öffnen mit: the programs for the file's type, then "Andere
    /// Anwendung …".
    pub(super) fn open_with_menu(&self, item: &Item) -> Option<gio::Menu> {
        let path = item_path(item)?;
        let content_type = content_type_of(path)?;
        let menu = gio::Menu::new();
        let mut seen: Vec<String> = Vec::new();
        let apps = gio::AppInfo::default_for_type(&content_type, false)
            .into_iter()
            .chain(gio::AppInfo::recommended_for_type(&content_type));
        for app in apps {
            let Some(id) = app.id().map(|id| id.to_string()) else {
                continue;
            };
            if seen.contains(&id) || !app.should_show() {
                continue;
            }
            let entry = gio::MenuItem::new(Some(&app.display_name()), None);
            entry.set_action_and_target_value(Some("desktop.open-with"), Some(&id.to_variant()));
            if let Some(icon) = app.icon() {
                entry.set_icon(&icon);
            }
            menu.append_item(&entry);
            seen.push(id);
        }
        let other = gio::Menu::new();
        let entry = gio::MenuItem::new(Some("Andere Anwendung …"), None);
        entry.set_action_and_target_value(Some("desktop.open-with"), Some(&"".to_variant()));
        other.append_item(&entry);
        menu.append_section(None, &other);
        if debugging() {
            log(&format!(
                "Menü Öffnen mit ({content_type}): {}",
                seen.join(", ")
            ));
        }
        Some(menu)
    }

    /// Opens `item` with the program `app_id`; empty: GTK's app chooser.
    pub(super) fn open_with(&self, item: &Item, app_id: &str) {
        let Some(path) = item_path(item) else { return };
        let context = WidgetExt::display(&self.window).app_launch_context();
        crate::portal::for_children(&context);
        let uri = uri_of(path);
        if app_id.is_empty() {
            choose_app(item, path, uri, context);
            return;
        }
        match DesktopAppInfo::new(app_id) {
            Some(app) => match app.launch_uris(&[uri.as_str()], Some(&context)) {
                Ok(()) => log(&format!("geöffnet mit {app_id}: {}", item.name)),
                Err(err) => {
                    log(&format!("{} nicht geöffnet mit {app_id}: {err}", item.name));
                    show_error(
                        &format!("„{}“ konnte nicht geöffnet werden.", item.name),
                        err.message(),
                    );
                }
            },
            None => log(&format!("Programm {app_id} fehlt")),
        }
    }

    /// Eigenschaften: a small window with name, type, size, place and date.
    pub(super) fn show_properties(&self, item: &Item) {
        let Some(path) = item_path(item) else { return };
        let file = gio::File::for_path(path);
        let info = file.query_info(
            "standard::*,time::modified",
            gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
            gio::Cancellable::NONE,
        );
        let info = match info {
            Ok(info) => info,
            Err(err) => {
                show_error(
                    &format!("Die Eigenschaften von „{}“ sind nicht lesbar.", item.name),
                    err.message(),
                );
                return;
            }
        };
        let folder = info.file_type() == gio::FileType::Directory;
        let content_type = content_type_of(path).unwrap_or_default();
        let kind = if info.file_type() == gio::FileType::SymbolicLink {
            format!(
                "Verknüpfung auf {}",
                info.symlink_target()
                    .map(|t| t.display().to_string())
                    .unwrap_or_default()
            )
        } else {
            format!(
                "{} ({content_type})",
                gio::content_type_get_description(&content_type)
            )
        };
        let size = gtk4::Label::new(Some(&if folder {
            "wird gezählt …".to_owned()
        } else {
            fileops::size_text(u64::try_from(info.size()).unwrap_or(0))
        }));
        let modified = info
            .modification_date_time()
            .and_then(|t| t.to_local().ok())
            .and_then(|t| t.format("%d.%m.%Y %H:%M").ok())
            .map(|t| t.to_string())
            .unwrap_or_default();
        let place = path
            .parent()
            .map(|p| p.display().to_string())
            .unwrap_or_default();

        let window = gtk4::Window::builder()
            .title(format!("Eigenschaften von „{}“", item.name))
            .resizable(false)
            .build();
        window.add_css_class("querencia-desktop-properties");
        let grid = gtk4::Grid::builder()
            .row_spacing(6)
            .column_spacing(12)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(12)
            .margin_end(12)
            .build();
        let image = Image::from_gicon(&icon_for(item, false));
        image.set_pixel_size(48);
        image.set_valign(Align::Start);
        grid.attach(&image, 0, 0, 1, 5);
        let rows: [(&str, gtk4::Label); 5] = [
            ("Name:", gtk4::Label::new(Some(&item.name))),
            ("Typ:", gtk4::Label::new(Some(&kind))),
            ("Größe:", size.clone()),
            ("Ort:", gtk4::Label::new(Some(&place))),
            ("Geändert:", gtk4::Label::new(Some(&modified))),
        ];
        for (row, (text, value)) in rows.iter().enumerate() {
            let row = i32::try_from(row).unwrap_or(0);
            let key = gtk4::Label::new(None);
            key.set_markup(&format!("<b>{}</b>", glib::markup_escape_text(text)));
            key.set_halign(Align::End);
            key.set_valign(Align::Start);
            value.set_halign(Align::Start);
            value.set_xalign(0.0);
            value.set_wrap(true);
            value.set_max_width_chars(48);
            value.set_selectable(true);
            // selectable with the mouse, but no pre-selected text on opening
            value.set_focusable(false);
            grid.attach(&key, 1, row, 1, 1);
            grid.attach(value, 2, row, 1, 1);
        }
        let close = gtk4::Button::with_label("Schließen");
        close.set_halign(Align::End);
        let window_weak = window.downgrade();
        close.connect_clicked(move |_| {
            if let Some(window) = window_weak.upgrade() {
                window.close();
            }
        });
        grid.attach(&close, 0, 5, 3, 1);
        window.set_child(Some(&grid));
        let escape = EventControllerKey::new();
        let window_weak = window.downgrade();
        escape.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Escape
                && let Some(window) = window_weak.upgrade()
            {
                window.close();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        window.add_controller(escape);
        window.set_default_widget(Some(&close));
        window.present();
        close.grab_focus();
        log(&format!("Eigenschaften: {} ({kind})", item.name));
        if folder {
            let dir = path.to_path_buf();
            let handle = gio::spawn_blocking(move || fileops::tally(&dir));
            glib::MainContext::default().spawn_local(async move {
                if let Ok((count, bytes)) = handle.await {
                    size.set_text(&format!(
                        "{count} {}, insgesamt {}",
                        if count == 1 { "Objekt" } else { "Objekte" },
                        fileops::size_text(bytes)
                    ));
                }
            });
        }
    }
}

/// Copy or move for files of another program: move when the source offers
/// only moving (Umschalt there), else copy.
fn foreign_action(drop: &gdk::Drop) -> gdk::DragAction {
    let offered = drop.actions();
    if offered.contains(gdk::DragAction::COPY) {
        gdk::DragAction::COPY
    } else {
        offered & gdk::DragAction::MOVE
    }
}

fn names_of(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// "Andere Anwendung …": GTK's app chooser for the file's type.
#[allow(deprecated)] // GtkAppChooserDialog: GTK 4 has no replacement yet
fn choose_app(item: &Item, path: &Path, uri: String, context: gdk::AppLaunchContext) {
    let content_type = content_type_of(path).unwrap_or_else(|| "application/octet-stream".into());
    let dialog = gtk4::AppChooserDialog::for_content_type(
        None::<&gtk4::Window>,
        gtk4::DialogFlags::empty(),
        &content_type,
    );
    dialog.set_title(Some("Öffnen mit"));
    dialog.set_heading(&format!(
        "Wählen Sie ein Programm, um „{}“ zu öffnen",
        item.name
    ));
    let name = item.name.clone();
    dialog.connect_response(move |dialog, response| {
        if response == gtk4::ResponseType::Ok
            && let Some(app) = dialog.app_info()
        {
            match app.launch_uris(&[uri.as_str()], Some(&context)) {
                Ok(()) => log(&format!(
                    "geöffnet mit {}: {name}",
                    app.id().unwrap_or_default()
                )),
                Err(err) => {
                    log(&format!("{name} nicht geöffnet: {err}"));
                    show_error(
                        &format!("„{name}“ konnte nicht geöffnet werden."),
                        err.message(),
                    );
                }
            }
        } else {
            log(&format!("Öffnen mit abgebrochen: {name}"));
        }
        dialog.destroy();
    });
    dialog.present();
    log(&format!("Öffnen mit: Auswahl für {content_type}"));
}
