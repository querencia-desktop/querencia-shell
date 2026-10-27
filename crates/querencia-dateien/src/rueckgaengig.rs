//! Undo and redo of file operations (0.3.0). Nautilus 2.30 had none; the
//! model is Nautilus 3.4's undo manager, slimmed down: one stack per
//! window (for all its tabs), filled from the jobs the window started
//! (`operationen.rs`), labels "Rückgängig: Verschieben von „a.txt“".
//!
//! | Done | Undo | Redo |
//! |---|---|---|
//! | Verschieben | moves back into the old folders | the job again |
//! | Kopieren | the copies into the trash, without a question (where a place has
//! no trash, the job asks before deleting for good) | the job again |
//! | Verknüpfen | deletes the links | the job again |
//! | Umbenennen | the old name | the job again |
//! | In den Papierkorb | restores the entries deleted from then on | the job again |
//! | Neuer Ordner, neue Datei | deletes it when empty, else into the trash | the job again |
//!
//! Only objects `querencia_dateiarbeit` lists in `Outcome::undo` take part:
//! nothing replaced or merged, a move with its own name; a move out of the
//! trash is not remembered (`operationen.rs`). Deleting for good
//! and emptying the trash cannot be undone: they clear the stack, so that
//! Strg+Z never undoes something older instead (the entry is inactive).
//! The stack lives as long as the window; it is not saved.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use gtk4::gio;
use gtk4::prelude::*;
use querencia_dateiarbeit::auftraege::{Op, Outcome};
use querencia_dateiarbeit::uri;

use crate::i18n::{gettext_f, ngettext_f};

/// How many operations a window remembers.
pub const TIEFE: usize = 50;

/// What was done.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Art {
    Kopieren,
    Verschieben,
    Verknuepfen,
    Umbenennen,
    Papierkorb,
    Ordner,
    Datei,
}

/// One operation that can go back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Vorgang {
    pub art: Art,
    /// The job as it was started (for "Wiederholen").
    pub op: Op,
    pub quellen: Vec<String>,
    pub ziel: Option<String>,
    /// (where the object was, where it is now) as URIs; "now" is empty for
    /// the trash, "was" for a new object.
    pub paare: Vec<(String, String)>,
    /// Local time at the start, `YYYY-MM-DDThh:mm:ss` as in `.trashinfo`
    /// (the trash entries of this operation are not older).
    pub beginn: String,
}

/// A job that undoes (part of) a [`Vorgang`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Schritt {
    pub op: Op,
    pub quellen: Vec<String>,
    pub ziel: Option<String>,
}

fn kind(ordner: &str, name: &str) -> String {
    gio::File::for_uri(ordner).child(name).uri().to_string()
}

fn eltern(uri: &str) -> Option<String> {
    gio::File::for_uri(uri)
        .parent()
        .map(|p| p.uri().to_string())
}

impl Vorgang {
    /// The operation a finished job did, if any of it can go back.
    pub fn aus(
        op: &Op,
        quellen: &[String],
        ziel: Option<&str>,
        outcome: &Outcome,
        beginn: &str,
    ) -> Option<Self> {
        let (art, paare): (Art, Vec<(String, String)>) = match op {
            Op::Copy | Op::Move | Op::Link => {
                let ziel = ziel?;
                let art = match op {
                    Op::Copy => Art::Kopieren,
                    Op::Move => Art::Verschieben,
                    _ => Art::Verknuepfen,
                };
                (
                    art,
                    outcome
                        .undo
                        .iter()
                        .filter(|(_, name)| !name.is_empty())
                        .map(|(q, name)| (q.clone(), kind(ziel, name)))
                        .collect(),
                )
            }
            Op::Rename { .. } => (
                Art::Umbenennen,
                outcome
                    .undo
                    .iter()
                    .filter_map(|(q, neu)| Some((q.clone(), kind(&eltern(q)?, neu))))
                    .collect(),
            ),
            Op::Trash => (
                Art::Papierkorb,
                outcome
                    .undo
                    .iter()
                    .map(|(q, _)| (q.clone(), String::new()))
                    .collect(),
            ),
            Op::NewFolder { .. } | Op::NewFile { .. } => {
                let ziel = ziel?;
                let art = if matches!(op, Op::NewFolder { .. }) {
                    Art::Ordner
                } else {
                    Art::Datei
                };
                (
                    art,
                    outcome
                        .arrived
                        .first()
                        .map(|name| (String::new(), kind(ziel, name)))
                        .into_iter()
                        .collect(),
                )
            }
            Op::Delete | Op::Restore | Op::EmptyTrash => return None,
        };
        if paare.is_empty() {
            return None;
        }
        Some(Self {
            art,
            op: op.clone(),
            quellen: quellen.to_vec(),
            ziel: ziel.map(str::to_owned),
            paare,
            beginn: beginn.to_owned(),
        })
    }

    /// The URIs where the objects are now (not for the trash).
    pub fn jetzt(&self) -> Vec<String> {
        self.paare
            .iter()
            .map(|(_, jetzt)| jetzt.clone())
            .filter(|j| !j.is_empty())
            .collect()
    }

    /// The paths the trashed objects had.
    pub fn originale(&self) -> Vec<PathBuf> {
        self.paare
            .iter()
            .filter_map(|(war, _)| gio::File::for_uri(war).path())
            .collect()
    }

    /// The jobs that undo it, for the objects of `da` (URIs that still
    /// exist where [`Self::jetzt`] says); `leer` tells whether a new object
    /// is still empty. Not for the trash (that needs its entries, see
    /// [`passende_eintraege`]).
    pub fn umkehren(&self, da: &[String], leer: impl Fn(&str) -> bool) -> Vec<Schritt> {
        let paare: Vec<&(String, String)> =
            self.paare.iter().filter(|(_, j)| da.contains(j)).collect();
        if paare.is_empty() {
            return Vec::new();
        }
        let alle = |op: Op| {
            vec![Schritt {
                op,
                quellen: paare.iter().map(|(_, j)| j.clone()).collect(),
                ziel: None,
            }]
        };
        match self.art {
            Art::Verschieben => {
                // one move per old folder, in the order of the first object
                let mut ordner: BTreeMap<usize, (String, Vec<String>)> = BTreeMap::new();
                let mut index: Vec<String> = Vec::new();
                for (war, jetzt) in &paare {
                    let Some(e) = eltern(war) else { continue };
                    let i = index.iter().position(|x| *x == e).unwrap_or_else(|| {
                        index.push(e.clone());
                        index.len() - 1
                    });
                    ordner
                        .entry(i)
                        .or_insert_with(|| (e, Vec::new()))
                        .1
                        .push(jetzt.clone());
                }
                ordner
                    .into_values()
                    .map(|(e, quellen)| Schritt {
                        op: Op::Move,
                        quellen,
                        ziel: Some(e),
                    })
                    .collect()
            }
            // reversible beats a question (decision of the start session)
            Art::Kopieren => alle(Op::Trash),
            Art::Verknuepfen => alle(Op::Delete),
            Art::Umbenennen => paare
                .iter()
                .map(|(war, jetzt)| Schritt {
                    op: Op::Rename {
                        new: uri::name_of(war),
                    },
                    quellen: vec![jetzt.clone()],
                    ziel: None,
                })
                .collect(),
            Art::Ordner | Art::Datei => {
                let (leere, volle): (Vec<String>, Vec<String>) =
                    paare.iter().map(|(_, j)| j.clone()).partition(|j| leer(j));
                let mut schritte = Vec::new();
                if !leere.is_empty() {
                    schritte.push(Schritt {
                        op: Op::Delete,
                        quellen: leere,
                        ziel: None,
                    });
                }
                if !volle.is_empty() {
                    schritte.push(Schritt {
                        op: Op::Trash,
                        quellen: volle,
                        ziel: None,
                    });
                }
                schritte
            }
            Art::Papierkorb => Vec::new(),
        }
    }

    /// The job "Wiederholen" starts.
    pub fn wiederholen(&self) -> Schritt {
        Schritt {
            op: self.op.clone(),
            quellen: self.quellen.clone(),
            ziel: self.ziel.clone(),
        }
    }

    /// What the menu says after "Rückgängig:": the description, for a copy
    /// with where the copies go („… (in den Papierkorb)“).
    pub fn beschreibung_rueckgaengig(&self) -> String {
        let b = self.beschreibung();
        if self.art == Art::Kopieren {
            gettext_f("{what} (to the trash)", &[("what", &b)])
        } else {
            b
        }
    }

    /// What the menu says after "Rückgängig:" / "Wiederholen:"; underscores
    /// of names are doubled (the menus take access keys out).
    pub fn beschreibung(&self) -> String {
        let name = |u: &str| uri::name_of(u).replace('_', "__");
        let n = u32::try_from(self.paare.len()).unwrap_or(u32::MAX);
        let erster = self.paare.first().map(|(war, jetzt)| match self.art {
            Art::Ordner | Art::Datei => name(jetzt),
            _ => name(war),
        });
        let erster = erster.unwrap_or_default();
        let a = [("name", erster.as_str())];
        match (self.art, n) {
            (Art::Kopieren, 1) => gettext_f("Copy of “{name}”", &a),
            (Art::Kopieren, _) => ngettext_f("Copy of {n} item", "Copy of {n} items", n, &[]),
            (Art::Verschieben, 1) => gettext_f("Move of “{name}”", &a),
            (Art::Verschieben, _) => ngettext_f("Move of {n} item", "Move of {n} items", n, &[]),
            (Art::Verknuepfen, 1) => gettext_f("Link to “{name}”", &a),
            (Art::Verknuepfen, _) => ngettext_f("Links to {n} item", "Links to {n} items", n, &[]),
            (Art::Umbenennen, _) => gettext_f("Rename of “{name}”", &a),
            (Art::Papierkorb, 1) => gettext_f("Move of “{name}” to the trash", &a),
            (Art::Papierkorb, _) => ngettext_f(
                "Move of {n} item to the trash",
                "Move of {n} items to the trash",
                n,
                &[],
            ),
            (Art::Ordner | Art::Datei, _) => gettext_f("Creation of “{name}”", &a),
        }
    }
}

/// A trash entry: its URI, where it was deleted from, when (as in
/// `.trashinfo`, local time).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Eintrag {
    pub uri: String,
    pub herkunft: PathBuf,
    pub datum: String,
}

/// The trash entries that undo "In den Papierkorb" of `originale` started
/// at `ab`: for each original the newest entry from it not older than
/// `ab` (a second earlier counts: `.trashinfo` has whole seconds).
pub fn passende_eintraege(eintraege: &[Eintrag], originale: &[PathBuf], ab: &str) -> Vec<String> {
    let frueher = eine_sekunde_frueher(ab);
    originale
        .iter()
        .filter_map(|o| {
            eintraege
                .iter()
                .filter(|e| e.herkunft == *o && e.datum.as_str() >= frueher.as_str())
                .max_by(|a, b| a.datum.cmp(&b.datum))
                .map(|e| e.uri.clone())
        })
        .collect()
}

fn eine_sekunde_frueher(zeit: &str) -> String {
    let format = "%Y-%m-%dT%H:%M:%S";
    gtk4::glib::DateTime::from_iso8601(&format!("{zeit}Z"), None)
        .and_then(|d| d.add_seconds(-1.0))
        .and_then(|d| d.format(format))
        .map(|s| s.to_string())
        .unwrap_or_else(|_| zeit.to_owned())
}

/// The local time now as `.trashinfo` writes it.
pub fn jetzt_zeit() -> String {
    gtk4::glib::DateTime::now_local()
        .and_then(|d| d.format("%Y-%m-%dT%H:%M:%S"))
        .map(|s| s.to_string())
        .unwrap_or_default()
}

/// `Path=` and `DeletionDate=` of a `.trashinfo` file.
pub fn trashinfo_lesen(text: &str) -> Option<(PathBuf, String)> {
    let mut pfad = None;
    let mut datum = None;
    for zeile in text.lines() {
        if let Some(p) = zeile.strip_prefix("Path=") {
            pfad = querencia_dateiarbeit::papierkorb::wiederherstellen::decode_trashinfo_path(p);
        } else if let Some(d) = zeile.strip_prefix("DeletionDate=") {
            datum = Some(d.trim().to_owned());
        }
    }
    Some((pfad?, datum?))
}

/// A path relative to a trash's top folder made absolute (`.trashinfo` of
/// other volumes).
pub fn herkunft_absolut(herkunft: PathBuf, oben: &Path) -> PathBuf {
    if herkunft.is_absolute() {
        herkunft
    } else {
        oben.join(herkunft)
    }
}

/// The undo and redo stacks of a window.
#[derive(Default, Debug)]
pub struct Stapel {
    zurueck: Vec<Vorgang>,
    vor: Vec<Vorgang>,
}

impl Stapel {
    /// A new operation: on top, the redo stack goes.
    pub fn neu(&mut self, v: Vorgang) {
        self.vor.clear();
        self.oben_auf(v);
    }

    fn oben_auf(&mut self, v: Vorgang) {
        self.zurueck.push(v);
        if self.zurueck.len() > TIEFE {
            self.zurueck.remove(0);
        }
    }

    /// Deleting for good: nothing older is undone instead.
    pub fn leeren(&mut self) {
        self.zurueck.clear();
        self.vor.clear();
    }

    /// The operation to undo now (taken off).
    pub fn rueckgaengig(&mut self) -> Option<Vorgang> {
        self.zurueck.pop()
    }

    /// After an undo: `v` can be redone.
    pub fn nach_rueckgaengig(&mut self, v: Vorgang) {
        self.vor.push(v);
    }

    /// The operation to redo now (taken off).
    pub fn wiederholen(&mut self) -> Option<Vorgang> {
        self.vor.pop()
    }

    /// After a redo: what it did can be undone again (the rest of the redo
    /// stack stays).
    pub fn nach_wiederholen(&mut self, v: Vorgang) {
        self.oben_auf(v);
    }

    pub fn naechstes_zurueck(&self) -> Option<&Vorgang> {
        self.zurueck.last()
    }

    pub fn naechstes_vor(&self) -> Option<&Vorgang> {
        self.vor.last()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(undo: &[(&str, &str)], arrived: &[&str]) -> Outcome {
        Outcome {
            arrived: arrived.iter().map(|s| (*s).to_owned()).collect(),
            done: undo.len().max(arrived.len()),
            undo: undo
                .iter()
                .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
                .collect(),
            ..Outcome::default()
        }
    }

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| (*x).to_owned()).collect()
    }

    #[test]
    fn a_move_goes_back_folder_by_folder() {
        let q = s(&["file:///h/a/x.txt", "file:///h/b/y.txt", "file:///h/a/z"]);
        let o = outcome(
            &[
                ("file:///h/a/x.txt", "x.txt"),
                ("file:///h/b/y.txt", "y.txt"),
                ("file:///h/a/z", "z"),
            ],
            &["x.txt", "y.txt", "z"],
        );
        let v = Vorgang::aus(
            &Op::Move,
            &q,
            Some("file:///h/ziel"),
            &o,
            "2026-10-27T12:00:00",
        )
        .expect("Vorgang");
        assert_eq!(v.art, Art::Verschieben);
        assert_eq!(
            v.jetzt(),
            s(&[
                "file:///h/ziel/x.txt",
                "file:///h/ziel/y.txt",
                "file:///h/ziel/z"
            ])
        );
        assert_eq!(v.beschreibung(), "Move of 3 items");
        // y.txt is gone meanwhile: only the others go back
        let da = s(&["file:///h/ziel/x.txt", "file:///h/ziel/z"]);
        assert_eq!(
            v.umkehren(&da, |_| true),
            vec![Schritt {
                op: Op::Move,
                quellen: da.clone(),
                ziel: Some("file:///h/a".to_owned()),
            }]
        );
        let alle = v.jetzt();
        let schritte = v.umkehren(&alle, |_| true);
        assert_eq!(schritte.len(), 2);
        assert_eq!(schritte[1].ziel.as_deref(), Some("file:///h/b"));
        assert!(v.umkehren(&[], |_| true).is_empty());
        assert_eq!(v.wiederholen().quellen, q);
    }

    #[test]
    fn copies_go_to_the_trash_links_are_deleted_renames_get_the_old_name() {
        let o = outcome(&[("file:///h/a/x.txt", "x (copy).txt")], &["x (copy).txt"]);
        let q = s(&["file:///h/a/x.txt"]);
        let v = Vorgang::aus(&Op::Copy, &q, Some("file:///h/a"), &o, "t").expect("Kopie");
        assert_eq!(v.beschreibung(), "Copy of “x.txt”");
        assert_eq!(
            v.beschreibung_rueckgaengig(),
            "Copy of “x.txt” (to the trash)"
        );
        assert_eq!(
            v.umkehren(&v.jetzt(), |_| true)[0],
            Schritt {
                op: Op::Trash,
                quellen: s(&["file:///h/a/x%20(copy).txt"]),
                ziel: None
            }
        );
        let v = Vorgang::aus(&Op::Link, &q, Some("file:///h/b"), &o, "t").expect("Link");
        assert_eq!(v.umkehren(&v.jetzt(), |_| true)[0].op, Op::Delete);
        let o = outcome(&[("file:///h/a/x_1.txt", "neu.txt")], &["neu.txt"]);
        let rename = Op::Rename {
            new: "neu.txt".to_owned(),
        };
        let v =
            Vorgang::aus(&rename, &s(&["file:///h/a/x_1.txt"]), None, &o, "t").expect("Umbenennen");
        assert_eq!(v.beschreibung(), "Rename of “x__1.txt”");
        assert_eq!(
            v.umkehren(&v.jetzt(), |_| true),
            vec![Schritt {
                op: Op::Rename {
                    new: "x_1.txt".to_owned()
                },
                quellen: s(&["file:///h/a/neu.txt"]),
                ziel: None,
            }]
        );
    }

    #[test]
    fn new_objects_go_when_empty_else_into_the_trash() {
        let o = outcome(&[], &["New Folder"]);
        let op = Op::NewFolder {
            name: "New Folder".to_owned(),
        };
        let v = Vorgang::aus(&op, &[], Some("file:///h"), &o, "t").expect("Ordner");
        assert_eq!(v.beschreibung(), "Creation of “New Folder”");
        assert_eq!(v.umkehren(&v.jetzt(), |_| true)[0].op, Op::Delete);
        assert_eq!(v.umkehren(&v.jetzt(), |_| false)[0].op, Op::Trash);
        assert_eq!(v.wiederholen().op, op);
    }

    #[test]
    fn nothing_to_undo_for_delete_restore_or_nothing_done() {
        let o = outcome(&[("file:///h/a", "")], &[]);
        for op in [Op::Delete, Op::Restore, Op::EmptyTrash] {
            assert!(Vorgang::aus(&op, &[], None, &o, "t").is_none());
        }
        let leer = Outcome::default();
        assert!(Vorgang::aus(&Op::Move, &[], Some("file:///z"), &leer, "t").is_none());
        assert!(Vorgang::aus(&Op::Move, &[], None, &o, "t").is_none());
    }

    #[test]
    fn trash_entries_are_found_by_origin_and_time() {
        let o = outcome(&[("file:///h/a.txt", ""), ("file:///h/b.txt", "")], &[]);
        let v = Vorgang::aus(
            &Op::Trash,
            &s(&["file:///h/a.txt", "file:///h/b.txt"]),
            None,
            &o,
            "2026-10-27T12:00:00",
        )
        .expect("Papierkorb");
        assert_eq!(v.beschreibung(), "Move of 2 items to the trash");
        assert!(v.jetzt().is_empty());
        let e = |uri: &str, herkunft: &str, datum: &str| Eintrag {
            uri: uri.to_owned(),
            herkunft: PathBuf::from(herkunft),
            datum: datum.to_owned(),
        };
        let eintraege = [
            e("trash:///a.txt", "/h/a.txt", "2026-10-26T09:00:00"),
            e("trash:///a.2.txt", "/h/a.txt", "2026-10-27T11:59:59"),
            e("trash:///a.3.txt", "/h/a.txt", "2026-10-27T12:00:01"),
            e("trash:///b.txt", "/h/b.txt", "2026-10-27T11:00:00"),
            e("trash:///c.txt", "/h/c.txt", "2026-10-27T12:00:00"),
        ];
        assert_eq!(
            passende_eintraege(&eintraege, &v.originale(), &v.beginn),
            s(&["trash:///a.3.txt"])
        );
        assert_eq!(
            eine_sekunde_frueher("2026-10-27T00:00:00"),
            "2026-10-26T23:59:59"
        );
        assert_eq!(
            trashinfo_lesen("[Trash Info]\nPath=/h/a%20b.txt\nDeletionDate=2026-10-27T12:00:01\n"),
            Some((
                PathBuf::from("/h/a b.txt"),
                "2026-10-27T12:00:01".to_owned()
            ))
        );
        assert_eq!(
            herkunft_absolut(PathBuf::from("x/y"), Path::new("/media/s")),
            PathBuf::from("/media/s/x/y")
        );
    }

    #[test]
    fn the_stacks_undo_redo_and_forget() {
        let v = |n: &str| Vorgang {
            art: Art::Umbenennen,
            op: Op::Rename { new: n.to_owned() },
            quellen: Vec::new(),
            ziel: None,
            paare: vec![(format!("file:///{n}"), String::new())],
            beginn: String::new(),
        };
        let mut st = Stapel::default();
        st.neu(v("a"));
        st.neu(v("b"));
        let b = st.rueckgaengig().expect("b");
        assert_eq!(b, v("b"));
        st.nach_rueckgaengig(b);
        assert_eq!(st.naechstes_vor(), Some(&v("b")));
        assert_eq!(st.naechstes_zurueck(), Some(&v("a")));
        let b = st.wiederholen().expect("b vor");
        st.nach_wiederholen(b);
        assert_eq!(st.naechstes_zurueck(), Some(&v("b")));
        // a new operation drops what could be redone
        st.rueckgaengig();
        st.nach_rueckgaengig(v("b"));
        st.neu(v("c"));
        assert!(st.naechstes_vor().is_none());
        for i in 0..TIEFE + 5 {
            st.neu(v(&i.to_string()));
        }
        let mut n = 0;
        while st.rueckgaengig().is_some() {
            n += 1;
        }
        assert_eq!(n, TIEFE);
        st.neu(v("x"));
        st.nach_rueckgaengig(v("y"));
        st.leeren();
        assert!(st.naechstes_zurueck().is_none() && st.naechstes_vor().is_none());
    }
}
