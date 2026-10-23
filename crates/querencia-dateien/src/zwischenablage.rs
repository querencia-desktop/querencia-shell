//! Ausschneiden, Kopieren, Einfügen with the clipboard of every file manager.
//!
//! - Writing: a `GdkFileList`, `text/uri-list` and GNOME's
//!   `x-special/gnome-copied-files` (`copy`/`cut`), with KDE's cut marker
//!   `application/x-kde-cutselection` when cut – the formats Querencia
//!   Files, Nautilus, Caja, Nemo, Thunar, Dolphin and the Querencia desktop
//!   read and write (`querencia_dateiarbeit::uri`, Desktop 0.2.3 QD-108).
//! - Reading: GNOME's text first (it says copy or cut), then a
//!   `GdkFileList`, then a plain URI list; cut is known from GNOME's text or
//!   KDE's marker. A plain URI list without marker is a copy.
//! - Cut files are moved when pasted; afterwards the clipboard is emptied
//!   (there is nothing left to paste), as Nautilus 2.30 did.
//!
//! One clipboard per display: what this process put there is kept in
//! [`Zwischenablage`], so that pasting its own files needs no reading and
//! views can show cut files faded.

use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{gdk, gio, glib};
use querencia_dateiarbeit::uri;

/// What a paste brings: whether the files were cut, and their URIs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inhalt {
    pub ausgeschnitten: bool,
    pub uris: Vec<String>,
}

/// The formats found on a clipboard, already read (for [`auswerten`]).
#[derive(Clone, Debug, Default)]
pub struct Gelesen {
    pub gnome: Option<Vec<u8>>,
    pub kde_cut: Option<Vec<u8>>,
    pub file_list: Option<Vec<String>>,
    pub uri_list: Option<Vec<u8>>,
}

/// Decides what a clipboard holds, in the order of the module text; without
/// GTK, so that it is tested on its own.
pub fn auswerten(gelesen: &Gelesen) -> Option<Inhalt> {
    if let Some((ausgeschnitten, uris)) = gelesen
        .gnome
        .as_deref()
        .and_then(uri::parse_gnome_copied_files)
    {
        return Some(Inhalt {
            ausgeschnitten,
            uris,
        });
    }
    let kde_cut = gelesen
        .kde_cut
        .as_deref()
        .is_some_and(uri::parse_kde_cut_selection);
    if let Some(uris) = gelesen.file_list.as_ref().filter(|u| !u.is_empty()) {
        return Some(Inhalt {
            ausgeschnitten: kde_cut,
            uris: uris.clone(),
        });
    }
    let uris = gelesen
        .uri_list
        .as_deref()
        .map(|bytes| uri::parse_uri_list(&String::from_utf8_lossy(bytes)))
        .unwrap_or_default();
    (!uris.is_empty()).then_some(Inhalt {
        ausgeschnitten: kde_cut,
        uris,
    })
}

/// The texts this process offers for `uris` (`cut`: Ausschneiden), each with
/// its MIME type; the `GdkFileList` comes on top in [`inhalt`].
pub fn angebote(cut: bool, uris: &[String]) -> Vec<(&'static str, Vec<u8>)> {
    let mut list = vec![
        (uri::URI_LIST, uri::uri_list(uris).into_bytes()),
        (
            uri::GNOME_COPIED_FILES,
            uri::gnome_copied_files(cut, uris).into_bytes(),
        ),
    ];
    if cut {
        list.push((uri::KDE_CUT_SELECTION, b"1".to_vec()));
    }
    list
}

/// The content for the clipboard (`cut` set) or a drag (`None`: only the
/// `GdkFileList` and `text/uri-list`).
pub fn inhalt(dateien: &[gio::File], cut: Option<bool>) -> gdk::ContentProvider {
    let uris: Vec<String> = dateien.iter().map(|f| f.uri().to_string()).collect();
    let mut providers = vec![gdk::ContentProvider::for_value(
        &gdk::FileList::from_array(dateien).to_value(),
    )];
    match cut {
        Some(cut) => {
            for (mime, bytes) in angebote(cut, &uris) {
                providers.push(gdk::ContentProvider::for_bytes(
                    mime,
                    &glib::Bytes::from_owned(bytes),
                ));
            }
        }
        None => providers.push(gdk::ContentProvider::for_bytes(
            uri::URI_LIST,
            &glib::Bytes::from_owned(uri::uri_list(&uris).into_bytes()),
        )),
    }
    gdk::ContentProvider::new_union(&providers)
}

async fn alles_lesen(stream: gio::InputStream) -> Option<Vec<u8>> {
    let output = gio::MemoryOutputStream::new_resizable();
    output
        .splice_future(
            &stream,
            gio::OutputStreamSpliceFlags::CLOSE_SOURCE | gio::OutputStreamSpliceFlags::CLOSE_TARGET,
            glib::Priority::DEFAULT,
        )
        .await
        .ok()?;
    Some(output.steal_as_bytes().to_vec())
}

async fn bytes(clipboard: &gdk::Clipboard, mime: &str) -> Option<Vec<u8>> {
    let (stream, _) = clipboard
        .read_future(&[mime], glib::Priority::DEFAULT)
        .await
        .ok()?;
    alles_lesen(stream).await
}

/// Reads the files on the clipboard of any program.
pub async fn lesen(clipboard: &gdk::Clipboard) -> Option<Inhalt> {
    let formats = clipboard.formats();
    let mut gelesen = Gelesen::default();
    if formats.contain_mime_type(uri::GNOME_COPIED_FILES) {
        gelesen.gnome = bytes(clipboard, uri::GNOME_COPIED_FILES).await;
        if let Some(found) = auswerten(&gelesen) {
            return Some(found);
        }
    }
    if formats.contain_mime_type(uri::KDE_CUT_SELECTION) {
        gelesen.kde_cut = bytes(clipboard, uri::KDE_CUT_SELECTION).await;
    }
    if formats.contains_type(gdk::FileList::static_type())
        && let Ok(value) = clipboard
            .read_value_future(gdk::FileList::static_type(), glib::Priority::DEFAULT)
            .await
        && let Ok(list) = value.get::<gdk::FileList>()
    {
        gelesen.file_list = Some(list.files().iter().map(|f| f.uri().to_string()).collect());
    }
    if gelesen.file_list.as_ref().is_none_or(Vec::is_empty)
        && formats.contain_mime_type(uri::URI_LIST)
    {
        gelesen.uri_list = bytes(clipboard, uri::URI_LIST).await;
    }
    auswerten(&gelesen)
}

/// Whether `formats` may hold files (for "Einfügen" in menus; the content
/// itself is read only when pasting).
pub fn hat_dateien(formats: &gdk::ContentFormats) -> bool {
    formats.contains_type(gdk::FileList::static_type())
        || formats.contain_mime_type(uri::URI_LIST)
        || formats.contain_mime_type(uri::GNOME_COPIED_FILES)
}

struct Eigenes {
    provider: gdk::ContentProvider,
    inhalt: Inhalt,
}

/// What this process put on the clipboard, while it is still there.
#[derive(Default)]
pub struct Zwischenablage {
    eigenes: RefCell<Option<Eigenes>>,
    beobachter: RefCell<Vec<Rc<dyn Fn()>>>,
    verbunden: std::cell::Cell<bool>,
}

impl Zwischenablage {
    /// The one of this process (the clipboard is the display's).
    pub fn get() -> Rc<Self> {
        thread_local! {
            static EINE: Rc<Zwischenablage> = Rc::default();
        }
        EINE.with(Rc::clone)
    }

    fn clipboard() -> Option<gdk::Clipboard> {
        gdk::Display::default().map(|d| d.clipboard())
    }

    /// `f` runs whenever the clipboard changes (cut files no longer faded,
    /// "Einfügen" en- or disabled).
    pub fn beobachten(self: &Rc<Self>, f: impl Fn() + 'static) {
        self.beobachter.borrow_mut().push(Rc::new(f));
        if self.verbunden.replace(true) {
            return;
        }
        let Some(clipboard) = Self::clipboard() else {
            return;
        };
        let weak = Rc::downgrade(self);
        clipboard.connect_changed(move |clipboard| {
            let Some(this) = weak.upgrade() else { return };
            let gone = this
                .eigenes
                .borrow()
                .as_ref()
                .is_some_and(|e| clipboard.content().as_ref() != Some(&e.provider));
            if gone {
                this.eigenes.borrow_mut().take();
            }
            this.melden();
        });
    }

    fn melden(&self) {
        let beobachter: Vec<Rc<dyn Fn()>> = self.beobachter.borrow().clone();
        for f in beobachter {
            f();
        }
    }

    /// Ausschneiden (`cut`) or Kopieren of `dateien`.
    pub fn setzen(&self, dateien: &[gio::File], cut: bool) -> Result<(), glib::BoolError> {
        let clipboard = Self::clipboard().ok_or_else(|| glib::bool_error!("no display"))?;
        let provider = inhalt(dateien, Some(cut));
        clipboard.set_content(Some(&provider))?;
        *self.eigenes.borrow_mut() = Some(Eigenes {
            provider,
            inhalt: Inhalt {
                ausgeschnitten: cut,
                uris: dateien.iter().map(|f| f.uri().to_string()).collect(),
            },
        });
        self.melden();
        Ok(())
    }

    /// After cut files were pasted: nothing is left to paste.
    pub fn leeren(&self) {
        if let Some(clipboard) = Self::clipboard()
            && let Err(err) = clipboard.set_content(None::<&gdk::ContentProvider>)
        {
            crate::operationen::protokoll(&format!("Zwischenablage nicht geleert: {err}"));
        }
        self.eigenes.borrow_mut().take();
        self.melden();
    }

    /// Whether `uri` was cut by this process and waits to be pasted (views
    /// show it faded).
    #[allow(dead_code)] // through operationen::ist_ausgeschnitten for the views
    pub fn ist_ausgeschnitten(&self, uri: &str) -> bool {
        self.eigenes
            .borrow()
            .as_ref()
            .is_some_and(|e| e.inhalt.ausgeschnitten && e.inhalt.uris.iter().any(|u| u == uri))
    }

    /// Whether "Einfügen" can have something to paste.
    pub fn kann_einfuegen(&self) -> bool {
        self.eigenes.borrow().is_some()
            || Self::clipboard().is_some_and(|c| hat_dateien(&c.formats()))
    }

    /// The files to paste: this process's own without reading, else read.
    pub async fn holen(&self) -> Option<Inhalt> {
        let clipboard = Self::clipboard()?;
        let own = self
            .eigenes
            .borrow()
            .as_ref()
            .filter(|e| clipboard.content().as_ref() == Some(&e.provider))
            .map(|e| e.inhalt.clone());
        match own {
            Some(own) => Some(own),
            None => lesen(&clipboard).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uris() -> Vec<String> {
        vec![
            "file:///tmp/a%20b.txt".to_owned(),
            "file:///tmp/%C3%A4rger".to_owned(),
        ]
    }

    #[test]
    fn what_we_write_says_copy_or_cut() {
        let copy = angebote(false, &uris());
        let mimes: Vec<&str> = copy.iter().map(|(m, _)| *m).collect();
        assert_eq!(mimes, vec![uri::URI_LIST, uri::GNOME_COPIED_FILES]);
        assert_eq!(
            copy[0].1,
            b"file:///tmp/a%20b.txt\r\nfile:///tmp/%C3%A4rger\r\n".to_vec()
        );
        assert_eq!(
            copy[1].1,
            b"copy\nfile:///tmp/a%20b.txt\nfile:///tmp/%C3%A4rger".to_vec()
        );
        let cut = angebote(true, &uris());
        assert_eq!(cut.len(), 3);
        assert!(cut[1].1.starts_with(b"cut\n"));
        assert_eq!(cut[2], (uri::KDE_CUT_SELECTION, b"1".to_vec()));
    }

    #[test]
    fn what_we_write_reads_back() {
        for cut in [false, true] {
            let mut gelesen = Gelesen::default();
            for (mime, bytes) in angebote(cut, &uris()) {
                match mime {
                    uri::GNOME_COPIED_FILES => gelesen.gnome = Some(bytes),
                    uri::KDE_CUT_SELECTION => gelesen.kde_cut = Some(bytes),
                    _ => gelesen.uri_list = Some(bytes),
                }
            }
            assert_eq!(
                auswerten(&gelesen),
                Some(Inhalt {
                    ausgeschnitten: cut,
                    uris: uris()
                })
            );
        }
    }

    #[test]
    fn gnome_text_wins_over_the_kde_marker() {
        // Nautilus/Files: GNOME's text decides, a stray KDE marker does not
        let gelesen = Gelesen {
            gnome: Some(b"copy\nfile:///a".to_vec()),
            kde_cut: Some(b"1".to_vec()),
            uri_list: Some(b"file:///a\r\n".to_vec()),
            ..Gelesen::default()
        };
        assert_eq!(
            auswerten(&gelesen),
            Some(Inhalt {
                ausgeschnitten: false,
                uris: vec!["file:///a".to_owned()]
            })
        );
    }

    #[test]
    fn kde_marks_cut_files_beside_the_uri_list() {
        // Dolphin writes text/uri-list and application/x-kde-cutselection
        let dolphin = Gelesen {
            kde_cut: Some(b"1".to_vec()),
            uri_list: Some(b"file:///home/x/Bild.png\r\n".to_vec()),
            ..Gelesen::default()
        };
        let found = auswerten(&dolphin).unwrap();
        assert!(found.ausgeschnitten);
        assert_eq!(found.uris, vec!["file:///home/x/Bild.png".to_owned()]);
        let copied = Gelesen {
            kde_cut: Some(b"0".to_vec()),
            file_list: Some(vec!["file:///b".to_owned()]),
            ..Gelesen::default()
        };
        assert_eq!(auswerten(&copied).map(|i| i.ausgeschnitten), Some(false));
    }

    #[test]
    fn a_file_list_comes_before_the_uri_list() {
        let gelesen = Gelesen {
            file_list: Some(vec!["file:///liste".to_owned()]),
            uri_list: Some(b"file:///text\r\n".to_vec()),
            ..Gelesen::default()
        };
        assert_eq!(
            auswerten(&gelesen).map(|i| i.uris),
            Some(vec!["file:///liste".to_owned()])
        );
        // an empty file list falls back to the text
        let leer = Gelesen {
            file_list: Some(Vec::new()),
            uri_list: Some(b"# Kommentar\r\nfile:///text\r\n".to_vec()),
            ..Gelesen::default()
        };
        assert_eq!(
            auswerten(&leer).map(|i| i.uris),
            Some(vec!["file:///text".to_owned()])
        );
    }

    #[test]
    fn no_files_no_paste() {
        assert_eq!(auswerten(&Gelesen::default()), None);
        let kaputt = Gelesen {
            gnome: Some(b"irgendwas\nfile:///a".to_vec()),
            uri_list: Some(b"\r\n".to_vec()),
            ..Gelesen::default()
        };
        assert_eq!(auswerten(&kaputt), None);
        // an invalid GNOME text does not hide a valid URI list
        let gemischt = Gelesen {
            gnome: Some(vec![0xff, 0xfe]),
            uri_list: Some(b"file:///c\r\n".to_vec()),
            ..Gelesen::default()
        };
        assert_eq!(
            auswerten(&gemischt),
            Some(Inhalt {
                ausgeschnitten: false,
                uris: vec!["file:///c".to_owned()]
            })
        );
    }
}
