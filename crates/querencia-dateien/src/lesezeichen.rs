//! Bookmarks: the file the GTK file chooser and Querencia Files use,
//! `~/.config/gtk-3.0/bookmarks` (one `URI [Name]` per line). Shared, so
//! no migration is needed (B §4.2 M1); changes by other programs arrive
//! through a file monitor.
//!
//! Without the file the sidebar and the menu show the XDG folders
//! Dokumente, Musik, Bilder, Videos and Downloads as Ubuntu 10.04 did
//! (B §3.5); the file is only written when the user adds a bookmark, and
//! then with these defaults first.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{gio, glib};

/// One bookmark.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Marke {
    pub uri: String,
    /// The name given in the file, else `None` (the folder's name is shown).
    pub name: Option<String>,
}

impl Marke {
    /// The name shown: the own name, else the folder's display name.
    pub fn anzeige(&self) -> String {
        if let Some(n) = &self.name {
            return n.clone();
        }
        let datei = gio::File::for_uri(&self.uri);
        match datei.path() {
            Some(p) if p == glib::home_dir() => p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            _ => datei
                .basename()
                .map(|n| n.to_string_lossy().into_owned())
                .filter(|n| n != "/")
                .unwrap_or_else(|| datei.parse_name().to_string()),
        }
    }

    pub fn datei(&self) -> gio::File {
        gio::File::for_uri(&self.uri)
    }
}

/// Parses the bookmarks file: `URI` or `URI Name` per line.
pub fn parsen(text: &str) -> Vec<Marke> {
    text.lines()
        .map(str::trim)
        .filter(|z| !z.is_empty() && !z.starts_with('#'))
        .map(|z| match z.split_once(' ') {
            Some((uri, name)) if !name.trim().is_empty() => Marke {
                uri: uri.to_owned(),
                name: Some(name.trim().to_owned()),
            },
            _ => Marke {
                uri: z.split(' ').next().unwrap_or(z).to_owned(),
                name: None,
            },
        })
        .collect()
}

/// One line per bookmark, the name after a space (the format GTK reads).
pub fn als_text(liste: &[Marke]) -> String {
    let mut s = String::new();
    for l in liste {
        s.push_str(&l.uri);
        if let Some(n) = &l.name {
            s.push(' ');
            s.push_str(n);
        }
        s.push('\n');
    }
    s
}

/// Ubuntu 10.04's first bookmarks: the XDG folders that exist.
pub fn vorgaben() -> Vec<Marke> {
    use glib::UserDirectory as U;
    [U::Documents, U::Music, U::Pictures, U::Videos, U::Downloads]
        .into_iter()
        .filter_map(glib::user_special_dir)
        .filter(|p| p.is_dir() && *p != glib::home_dir())
        .map(|p| Marke {
            uri: gio::File::for_path(p).uri().to_string(),
            name: None,
        })
        .collect()
}

pub fn pfad() -> PathBuf {
    glib::user_config_dir().join("gtk-3.0").join("bookmarks")
}

type Handler = Rc<dyn Fn()>;

/// The bookmarks of the process, kept current.
pub struct Lesezeichen {
    pfad: PathBuf,
    liste: RefCell<Vec<Marke>>,
    /// Whether the file exists (else the list is [`vorgaben`]).
    aus_datei: RefCell<bool>,
    handler: RefCell<Vec<Handler>>,
    _monitor: RefCell<Option<gio::FileMonitor>>,
}

impl Lesezeichen {
    pub fn laden() -> Rc<Self> {
        let l = Rc::new(Self {
            pfad: pfad(),
            liste: RefCell::default(),
            aus_datei: RefCell::new(false),
            handler: RefCell::default(),
            _monitor: RefCell::default(),
        });
        l.lesen();
        let monitor = gio::File::for_path(&l.pfad)
            .monitor_file(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE)
            .ok();
        if let Some(m) = &monitor {
            let weak = Rc::downgrade(&l);
            m.connect_changed(move |_, _, _, ereignis| {
                if matches!(
                    ereignis,
                    gio::FileMonitorEvent::ChangesDoneHint
                        | gio::FileMonitorEvent::Created
                        | gio::FileMonitorEvent::Deleted
                        | gio::FileMonitorEvent::Renamed
                        | gio::FileMonitorEvent::MovedIn
                ) && let Some(l) = weak.upgrade()
                {
                    l.lesen();
                    l.melden();
                }
            });
        }
        l._monitor.replace(monitor);
        l
    }

    fn lesen(&self) {
        match std::fs::read_to_string(&self.pfad) {
            Ok(text) => {
                self.liste.replace(parsen(&text));
                self.aus_datei.replace(true);
            }
            Err(_) => {
                self.liste.replace(vorgaben());
                self.aus_datei.replace(false);
            }
        }
    }

    fn melden(&self) {
        let handler: Vec<Handler> = self.handler.borrow().clone();
        for h in handler {
            h();
        }
    }

    pub fn liste(&self) -> Vec<Marke> {
        self.liste.borrow().clone()
    }

    /// `f` runs after every change (also by other programs).
    pub fn bei_aenderung(&self, f: impl Fn() + 'static) {
        self.handler.borrow_mut().push(Rc::new(f));
    }

    /// Lesezeichen → Lesezeichen hinzufügen (Strg+D): appends `ort` unless
    /// it is there already; writes the file (atomically).
    pub fn hinzufuegen(&self, ort: &gio::File) -> std::io::Result<bool> {
        let uri = ort.uri().to_string();
        if self.liste.borrow().iter().any(|l| l.uri == uri) && *self.aus_datei.borrow() {
            return Ok(false);
        }
        let mut liste = self.liste.borrow().clone();
        if !liste.iter().any(|l| l.uri == uri) {
            liste.push(Marke { uri, name: None });
        }
        self.schreiben(&liste)?;
        self.liste.replace(liste);
        self.aus_datei.replace(true);
        self.melden();
        Ok(true)
    }

    /// Removes the bookmark with `uri` (sidebar: "Lesezeichen entfernen").
    pub fn entfernen(&self, uri: &str) -> std::io::Result<()> {
        let mut liste = self.liste.borrow().clone();
        liste.retain(|l| l.uri != uri);
        self.schreiben(&liste)?;
        self.liste.replace(liste);
        self.aus_datei.replace(true);
        self.melden();
        Ok(())
    }

    fn schreiben(&self, liste: &[Marke]) -> std::io::Result<()> {
        if let Some(dir) = self.pfad.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = self.pfad.with_extension("querencia-neu");
        std::fs::write(&tmp, als_text(liste))?;
        std::fs::rename(&tmp, &self.pfad)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_uri_and_optional_name() {
        let l = parsen(
            "file:///home/u/Dokumente\nfile:///home/u/Projekte%20alt Projekte\n\nsftp://h/ Server Eins\n",
        );
        assert_eq!(l.len(), 3);
        assert_eq!(l[0].name, None);
        assert_eq!(l[0].anzeige(), "Dokumente");
        assert_eq!(l[1].name.as_deref(), Some("Projekte"));
        assert_eq!(l[2].name.as_deref(), Some("Server Eins"));
        assert_eq!(parsen(&als_text(&l)), l);
        let ohne = Marke {
            uri: "file:///home/u/Projekte%20alt".into(),
            name: None,
        };
        assert_eq!(ohne.anzeige(), "Projekte alt");
    }
}
