//! The location bar (B §1.3): the path buttons (one button per folder,
//! arrows when they do not fit, a click changes to the folder, drop on a
//! button) and, temporarily, the entry "Ort:" (Strg+L, "/" or "~" typed in
//! the view; Tab completes folder names, Enter goes there, Esc returns to
//! the buttons). With the setting "Adressleiste immer als Textfeld" the
//! entry stays. Also the history of the window ([`Verlauf`]).

use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::{Rc, Weak};

use gtk4::prelude::*;
use gtk4::{gdk, gio, glib};
use querencia_oberflaeche::symbole;

use crate::fenster::Fenster;
use crate::i18n::{gettext, gettext_f};
use crate::konfig::Werte;
use crate::regeln::{self, Knopfart};

// --------------------------------------------------------------- history

/// Back and forward of a window (Gehe zu → Zurück/Vor, toolbar).
#[derive(Default)]
pub struct Verlauf {
    zurueck: Vec<gio::File>,
    jetzt: Option<gio::File>,
    vor: Vec<gio::File>,
}

impl Verlauf {
    /// A new place: the current one goes back, forward is cleared.
    pub fn besuchen(&mut self, ort: &gio::File) {
        if self.jetzt.as_ref().is_some_and(|j| j.equal(ort)) {
            return;
        }
        if let Some(j) = self.jetzt.take() {
            self.zurueck.push(j);
        }
        self.vor.clear();
        self.jetzt = Some(ort.clone());
    }

    /// One step back: (place, the place we leave if it lies inside it – to
    /// select it there).
    pub fn zurueck(&mut self) -> Option<(gio::File, Option<gio::File>)> {
        let ziel = self.zurueck.pop()?;
        let alt = self.jetzt.replace(ziel.clone());
        if let Some(a) = &alt {
            self.vor.push(a.clone());
        }
        let kind = alt.filter(|a| a.parent().is_some_and(|p| p.equal(&ziel)));
        Some((ziel, kind))
    }

    pub fn vor(&mut self) -> Option<(gio::File, Option<gio::File>)> {
        let ziel = self.vor.pop()?;
        if let Some(a) = self.jetzt.replace(ziel.clone()) {
            self.zurueck.push(a);
        }
        Some((ziel, None))
    }

    pub fn kann_zurueck(&self) -> bool {
        !self.zurueck.is_empty()
    }

    pub fn kann_vor(&self) -> bool {
        !self.vor.is_empty()
    }
}

// ------------------------------------------------------------ pure parts

/// The place an entered text stands for: "~" is the home folder, URIs and
/// absolute paths as they are, anything else relative to `hier`.
pub fn aufloesen(text: &str, hier: Option<&gio::File>, home: &Path) -> gio::File {
    let text = text.trim();
    let text = if text == "~" {
        home.display().to_string()
    } else if let Some(rest) = text.strip_prefix("~/") {
        home.join(rest).display().to_string()
    } else {
        text.to_owned()
    };
    let hat_schema = text.split_once(':').is_some_and(|(s, _)| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
    });
    if hat_schema || text.starts_with('/') {
        return gio::File::for_commandline_arg(&text);
    }
    match hier {
        Some(h) => h.resolve_relative_path(&text),
        None => gio::File::for_path(home.join(&text)),
    }
}

/// Tab completion of a typed local path: the longest common start of the
/// folder names in the typed folder that begin with the typed rest; a
/// single match gets a "/". `None` when nothing matches or nothing is
/// added. `ordner(dir)` lists the folder names in `dir`.
pub fn vervollstaendigen(text: &str, ordner: impl Fn(&Path) -> Vec<String>) -> Option<String> {
    let (dir, anfang) = match text.rfind('/') {
        Some(i) => (&text[..=i], &text[i + 1..]),
        None => return None,
    };
    let mut treffer: Vec<String> = ordner(Path::new(dir))
        .into_iter()
        .filter(|n| n.starts_with(anfang) && (!n.starts_with('.') || anfang.starts_with('.')))
        .collect();
    treffer.sort();
    let erster = treffer.first()?;
    let mut gemeinsam = erster.clone();
    for t in &treffer[1..] {
        let n = gemeinsam
            .chars()
            .zip(t.chars())
            .take_while(|(a, b)| a == b)
            .map(|(a, _)| a.len_utf8())
            .sum();
        gemeinsam.truncate(n);
    }
    let mut neu = format!("{dir}{gemeinsam}");
    if treffer.len() == 1 {
        neu.push('/');
    }
    (neu != text).then_some(neu)
}

/// Tab in the entry: completes `text` as typed – "~/…", absolute, or
/// relative to the shown folder `hier` – and gives it back in the same form.
pub fn tab_ergaenzen(text: &str, hier: Option<&Path>, home: &Path) -> Option<String> {
    let (vorne, absolut) = if let Some(rest) = text.strip_prefix("~/") {
        ("~/".to_owned(), format!("{}/{rest}", home.display()))
    } else if text.starts_with('/') {
        (String::new(), text.to_owned())
    } else {
        let hier = hier?;
        (String::new(), format!("{}/{text}", hier.display()))
    };
    let neu = vervollstaendigen(&absolut, ordner_in)?;
    let basis = &absolut[..absolut.len() - text.len() + vorne.len()];
    let rest = neu.strip_prefix(basis)?;
    Some(format!("{vorne}{rest}"))
}

fn ordner_in(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .map(|r| {
            r.filter_map(Result::ok)
                .filter(|e| e.path().is_dir())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default()
}

// ------------------------------------------------------------ the bar

pub struct Adressleiste {
    fenster: Weak<Fenster>,
    leiste: gtk4::Stack,
    knoepfe: gtk4::Box,
    scroller: gtk4::ScrolledWindow,
    links: gtk4::Button,
    rechts: gtk4::Button,
    feld: gtk4::Entry,
    /// The deepest path shown as buttons (kept when going up, GtkPathBar).
    gezeigt: RefCell<Vec<(gtk4::ToggleButton, gio::File)>>,
    immer_text: Cell<bool>,
    setzt: Cell<bool>,
    /// The button to scroll into view once the bar has its new size.
    zeigen_nach: RefCell<Option<gtk4::ToggleButton>>,
}

impl Adressleiste {
    pub fn new(fenster: &Rc<Fenster>) -> Rc<Self> {
        let leiste = gtk4::Stack::new();
        leiste.add_css_class("querencia-adressleiste");
        // path buttons with arrows
        let knoepfe = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        knoepfe.add_css_class("linked");
        let scroller = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::External)
            .vscrollbar_policy(gtk4::PolicyType::Never)
            .child(&knoepfe)
            .hexpand(true)
            .build();
        let links = gtk4::Button::from_icon_name("pan-start-symbolic");
        let rechts = gtk4::Button::from_icon_name("pan-end-symbolic");
        for k in [&links, &rechts] {
            k.set_has_frame(false);
            k.set_can_focus(false);
            k.set_visible(false);
        }
        links.set_tooltip_text(Some(&gettext("Show the folders before")));
        rechts.set_tooltip_text(Some(&gettext("Show the folders after")));
        let pfad = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        pfad.append(&links);
        pfad.append(&scroller);
        pfad.append(&rechts);
        leiste.add_named(&pfad, Some("knoepfe"));
        // location entry
        let ort = gtk4::Label::new(Some(&gettext("Location:")));
        ort.set_margin_start(6);
        let feld = gtk4::Entry::new();
        feld.set_hexpand(true);
        ort.set_mnemonic_widget(Some(&feld));
        let zeile = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        zeile.append(&ort);
        zeile.append(&feld);
        leiste.add_named(&zeile, Some("feld"));
        leiste.set_visible_child_name("knoepfe");
        let a = Rc::new(Self {
            fenster: Rc::downgrade(fenster),
            leiste,
            knoepfe,
            scroller,
            links,
            rechts,
            feld,
            gezeigt: RefCell::default(),
            immer_text: Cell::new(false),
            setzt: Cell::new(false),
            zeigen_nach: RefCell::default(),
        });
        a.signale();
        if let Some(app) = fenster.app() {
            a.einstellungen_anwenden(&app.konfig().werte());
        }
        a
    }

    fn signale(self: &Rc<Self>) {
        let adj = self.scroller.hadjustment();
        let weak = Rc::downgrade(self);
        adj.connect_changed(move |_| {
            if let Some(a) = weak.upgrade() {
                a.pfeile();
                let k = a.zeigen_nach.borrow().clone();
                if let Some(k) = k {
                    a.zeigen(&k);
                }
            }
        });
        let weak = Rc::downgrade(self);
        adj.connect_value_changed(move |_| {
            if let Some(a) = weak.upgrade() {
                a.pfeile();
            }
        });
        for (k, richtung) in [(&self.links, -1.0), (&self.rechts, 1.0)] {
            let weak = Rc::downgrade(self);
            k.connect_clicked(move |_| {
                if let Some(a) = weak.upgrade() {
                    let adj = a.scroller.hadjustment();
                    adj.set_value(adj.value() + richtung * 120.0);
                }
            });
        }
        let weak = Rc::downgrade(self);
        self.feld.connect_activate(move |e| {
            if let Some(a) = weak.upgrade() {
                a.eingabe(&e.text());
            }
        });
        let tasten = gtk4::EventControllerKey::new();
        tasten.set_propagation_phase(gtk4::PropagationPhase::Capture);
        let weak = Rc::downgrade(self);
        tasten.connect_key_pressed(move |_, taste, _, zustand| {
            let Some(a) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            match taste {
                gdk::Key::Escape => {
                    a.textfeld_schliessen(true);
                    glib::Propagation::Stop
                }
                gdk::Key::Tab if crate::fenster::ohne_strg_alt(zustand) => {
                    let text = a.feld.text().to_string();
                    let hier = a
                        .fenster
                        .upgrade()
                        .and_then(|f| f.ort())
                        .and_then(|o| o.path());
                    if let Some(neu) = tab_ergaenzen(&text, hier.as_deref(), &glib::home_dir()) {
                        a.feld.set_text(&neu);
                        a.feld.set_position(-1);
                    }
                    // Tab stays in the entry (Nautilus 2.30)
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        });
        self.feld.add_controller(tasten);
    }

    pub fn widget(&self) -> &gtk4::Stack {
        &self.leiste
    }

    fn pfeile(&self) {
        let adj = self.scroller.hadjustment();
        let ueber = adj.upper() > adj.page_size() + 0.5;
        self.links.set_visible(ueber);
        self.rechts.set_visible(ueber);
        self.links.set_sensitive(adj.value() > 0.5);
        self.rechts
            .set_sensitive(adj.value() + adj.page_size() < adj.upper() - 0.5);
    }

    pub fn einstellungen_anwenden(&self, w: &Werte) {
        self.immer_text.set(w.adresse_als_text);
        if w.adresse_als_text {
            self.leiste.set_visible_child_name("feld");
        } else if !self.feld.has_focus() && !hat_fokus(&self.feld) {
            self.leiste.set_visible_child_name("knoepfe");
        }
    }

    /// Shows `ort` (buttons and entry text).
    pub fn ort_anzeigen(self: &Rc<Self>, ort: &gio::File) {
        self.feld.set_text(&ort.parse_name());
        if !self.immer_text.get() {
            self.leiste.set_visible_child_name("knoepfe");
        }
        // going up inside the shown path keeps the deeper buttons
        let bleibt = self.gezeigt.borrow().iter().any(|(_, d)| d.equal(ort));
        if !bleibt {
            self.knoepfe_bauen(ort);
        }
        self.setzt.set(true);
        let mut aktiv = None;
        for (k, d) in self.gezeigt.borrow().iter() {
            let an = d.equal(ort);
            k.set_active(an);
            if an {
                aktiv = Some(k.clone());
            }
        }
        self.setzt.set(false);
        if let Some(k) = aktiv {
            // now if the size is known, else when the adjustment changes
            self.zeigen(&k);
            self.zeigen_nach.replace(Some(k));
        }
    }

    /// Scrolls so `knopf` is visible.
    fn zeigen(&self, knopf: &gtk4::ToggleButton) {
        let Some(b) = knopf.compute_bounds(&self.knoepfe) else {
            return;
        };
        let adj = self.scroller.hadjustment();
        let (x, breite) = (f64::from(b.x()), f64::from(b.width()));
        if x < adj.value() {
            adj.set_value(x);
        } else if x + breite > adj.value() + adj.page_size() {
            adj.set_value(x + breite - adj.page_size());
        }
    }

    fn knoepfe_bauen(self: &Rc<Self>, ort: &gio::File) {
        while let Some(k) = self.knoepfe.first_child() {
            self.knoepfe.remove(&k);
        }
        let mut neu = Vec::new();
        match ort.path() {
            Some(pfad) => {
                for (name, p, art) in regeln::knopfpfad(&pfad, &glib::home_dir()) {
                    let icon = match art {
                        Knopfart::Persoenlich => Some("user-home"),
                        Knopfart::Dateisystem => Some("drive-harddisk"),
                        Knopfart::Ordner => None,
                    };
                    let name = if art == Knopfart::Dateisystem {
                        None
                    } else {
                        Some(name)
                    };
                    neu.push((self.knopf(icon, name.as_deref()), gio::File::for_path(p)));
                }
            }
            None => {
                let name = crate::orte::anzeigename(ort);
                neu.push((
                    self.knopf(Some(crate::orte::symbol(ort)), Some(&name)),
                    ort.clone(),
                ));
            }
        }
        let fenster = self.fenster.upgrade();
        let ablage = fenster
            .as_ref()
            .and_then(|f| f.app())
            .and_then(|a| a.register().ablage.clone());
        for (k, d) in &neu {
            self.knoepfe.append(k);
            let weak = Rc::downgrade(self);
            let d2 = d.clone();
            k.connect_clicked(move |k| {
                let Some(a) = weak.upgrade() else { return };
                if a.setzt.get() {
                    return;
                }
                if let Some(f) = a.fenster.upgrade() {
                    if f.ort().is_some_and(|o| o.equal(&d2)) {
                        a.setzt.set(true);
                        k.set_active(true);
                        a.setzt.set(false);
                    } else {
                        f.gehe_zu(&d2);
                    }
                }
            });
            // the middle button opens the folder in a new tab (B §1.3)
            let mitte = gtk4::GestureClick::new();
            mitte.set_button(gdk::BUTTON_MIDDLE);
            let weak = Rc::downgrade(self);
            let d4 = d.clone();
            mitte.connect_released(move |g, _, _, _| {
                g.set_state(gtk4::EventSequenceState::Claimed);
                if let Some(f) = weak.upgrade().and_then(|a| a.fenster.upgrade()) {
                    f.reiter_oeffnen(Some(&d4), false, None);
                }
            });
            k.add_controller(mitte);
            if let (Some(f), Some(h)) = (&fenster, &ablage) {
                let d3 = d.clone();
                h(f, k.upcast_ref(), Rc::new(move |_, _| Some(d3.clone())));
            }
        }
        self.gezeigt.replace(neu);
    }

    fn knopf(&self, icon: Option<&str>, name: Option<&str>) -> gtk4::ToggleButton {
        let k = gtk4::ToggleButton::new();
        let inhalt = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
        if let Some(i) = icon {
            inhalt.append(&symbole::image_named(i, 16));
        }
        if let Some(n) = name {
            let l = gtk4::Label::new(Some(n));
            // the minimum width is the text (up to 24 characters), so a long
            // path overflows into the arrows instead of squeezing every button
            let zeichen = i32::try_from(n.chars().count().min(24) + 1).unwrap_or(24);
            l.set_width_chars(zeichen);
            l.set_max_width_chars(24);
            l.set_ellipsize(gtk4::pango::EllipsizeMode::Middle);
            inhalt.append(&l);
            k.set_tooltip_text(Some(n));
        } else {
            k.set_tooltip_text(Some(&gettext("File System")));
        }
        k.set_child(Some(&inhalt));
        k.set_can_focus(false);
        k
    }

    /// Strg+L (`anfang` `None`: the whole location selected) or a typed
    /// "/" / "~" (the entry starts with it).
    pub fn textfeld_oeffnen(&self, anfang: Option<&str>) {
        if let Some(f) = self.fenster.upgrade()
            && !self.leiste.is_visible()
        {
            // the bar is hidden (Ansicht → Adressleiste): show it for now
            let _ = f;
            self.leiste.set_visible(true);
        }
        self.leiste.set_visible_child_name("feld");
        self.feld.grab_focus();
        match anfang {
            Some(t) => {
                self.feld.set_text(t);
                self.feld.set_position(-1);
            }
            None => self.feld.select_region(0, -1),
        }
    }

    fn textfeld_schliessen(&self, fokus_zurueck: bool) {
        if let Some(f) = self.fenster.upgrade() {
            if let Some(o) = f.ort() {
                self.feld.set_text(&o.parse_name());
            }
            if !self.immer_text.get() {
                self.leiste.set_visible_child_name("knoepfe");
            }
            let werte = f.app().map(|a| a.konfig().werte()).unwrap_or_default();
            if !werte.adressleiste {
                self.leiste.set_visible(false);
            }
            if fokus_zurueck {
                f.ansicht().fokus();
            }
        }
    }

    fn eingabe(&self, text: &str) {
        let Some(f) = self.fenster.upgrade() else {
            return;
        };
        if text.trim().is_empty() {
            return;
        }
        let ziel = aufloesen(text, f.ort().as_ref(), &glib::home_dir());
        let info = ziel.query_info(
            "standard::type",
            gio::FileQueryInfoFlags::NONE,
            gio::Cancellable::NONE,
        );
        match info.map(|i| i.file_type()) {
            Ok(gio::FileType::Directory | gio::FileType::Mountable) => {
                self.textfeld_schliessen(false);
                f.gehe_zu(&ziel);
                f.ansicht().fokus();
            }
            Ok(_) => {
                // a file: its folder, with the file selected
                if let Some(eltern) = ziel.parent() {
                    self.textfeld_schliessen(false);
                    f.gehe_zu(&eltern);
                    f.nach_erscheinen_waehlen(std::slice::from_ref(&ziel));
                    f.ansicht().fokus();
                }
            }
            Err(e) if !ziel.is_native() && e.matches(gio::IOErrorEnum::NotMounted) => {
                self.textfeld_schliessen(false);
                f.gehe_zu(&ziel);
            }
            Err(_) => f.melden(&gettext_f(
                "The location “{location}” could not be found.",
                &[("location", text.trim())],
            )),
        }
    }
}

fn hat_fokus(w: &impl IsA<gtk4::Widget>) -> bool {
    w.root()
        .and_then(|r| r.focus())
        .is_some_and(|f| f.is_ancestor(w))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_goes_back_and_forward_and_marks_the_child() {
        let f = |p: &str| gio::File::for_path(p);
        let mut v = Verlauf::default();
        assert!(!v.kann_zurueck() && !v.kann_vor());
        v.besuchen(&f("/a"));
        v.besuchen(&f("/a/b"));
        v.besuchen(&f("/a/b"));
        v.besuchen(&f("/c"));
        assert!(v.kann_zurueck());
        let (ort, kind) = v.zurueck().unwrap();
        assert_eq!(ort.path().unwrap(), Path::new("/a/b"));
        assert!(kind.is_none());
        let (ort, kind) = v.zurueck().unwrap();
        assert_eq!(ort.path().unwrap(), Path::new("/a"));
        assert_eq!(kind.unwrap().path().unwrap(), Path::new("/a/b"));
        assert!(!v.kann_zurueck() && v.kann_vor());
        let (ort, _) = v.vor().unwrap();
        assert_eq!(ort.path().unwrap(), Path::new("/a/b"));
        v.besuchen(&f("/d"));
        assert!(!v.kann_vor());
    }

    #[test]
    fn typed_locations_resolve() {
        let home = Path::new("/home/u");
        let hier = gio::File::for_path("/home/u/Dokumente");
        assert_eq!(aufloesen("~", None, home).path().unwrap(), home);
        assert_eq!(
            aufloesen("~/Bilder", None, home).path().unwrap(),
            Path::new("/home/u/Bilder")
        );
        assert_eq!(
            aufloesen("/usr/share", None, home).path().unwrap(),
            Path::new("/usr/share")
        );
        assert_eq!(
            aufloesen("Rechnungen", Some(&hier), home).path().unwrap(),
            Path::new("/home/u/Dokumente/Rechnungen")
        );
        assert_eq!(
            aufloesen("../Musik", Some(&hier), home).path().unwrap(),
            Path::new("/home/u/Musik")
        );
        assert_eq!(
            aufloesen(" file:///tmp ", None, home).path().unwrap(),
            Path::new("/tmp")
        );
    }

    #[test]
    fn tab_keeps_the_typed_form() {
        let dir = std::env::temp_dir().join(format!("qdat-tab-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("Dokumente")).unwrap();
        std::fs::create_dir_all(dir.join("Downloads")).unwrap();
        assert_eq!(
            tab_ergaenzen("Dok", Some(&dir), Path::new("/x")).as_deref(),
            Some("Dokumente/")
        );
        assert_eq!(
            tab_ergaenzen("~/Dok", None, &dir).as_deref(),
            Some("~/Dokumente/")
        );
        let abs = format!("{}/Dow", dir.display());
        assert_eq!(
            tab_ergaenzen(&abs, None, Path::new("/x")),
            Some(format!("{}/Downloads/", dir.display()))
        );
        assert_eq!(tab_ergaenzen("Do", Some(&dir), Path::new("/x")), None);
        assert_eq!(tab_ergaenzen("Dok", None, Path::new("/x")), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn tab_completes_folder_names() {
        let liste = |_: &Path| {
            vec![
                "Dokumente".to_string(),
                "Downloads".into(),
                "Bilder".into(),
                ".cache".into(),
            ]
        };
        assert_eq!(
            vervollstaendigen("/home/u/Bi", liste).as_deref(),
            Some("/home/u/Bilder/")
        );
        assert_eq!(vervollstaendigen("/home/u/Do", liste), None);
        assert_eq!(
            vervollstaendigen("/home/u/Dok", liste).as_deref(),
            Some("/home/u/Dokumente/")
        );
        assert_eq!(vervollstaendigen("/home/u/x", liste), None);
        assert_eq!(
            vervollstaendigen("/home/u/.c", liste).as_deref(),
            Some("/home/u/.cache/")
        );
        let liste = |_: &Path| vec!["Projekt-A".to_string(), "Projekt-B".into()];
        assert_eq!(
            vervollstaendigen("/p/Pro", liste).as_deref(),
            Some("/p/Projekt-")
        );
        assert_eq!(vervollstaendigen("ohne-schraegstrich", liste), None);
    }
}
