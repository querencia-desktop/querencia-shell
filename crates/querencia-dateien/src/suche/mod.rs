//! Search for file names (Welle C2, B §1 „Suche“: from the shown folder
//! down, no criteria, no index; 0.3.0: the switch „Auch Dateiinhalt“, off
//! for every new search bar, reads text files too): the button „Suchen“ in the
//! toolbar, Gehe zu → „Suchen …“ and Strg+F open a search bar above the
//! view, as Nautilus 2.30; Enter searches in a background thread
//! (`sucher.rs`), the hits replace the view as „Suchergebnisse“ with a
//! location column (`ergebnisse.rs`). Any change of the text stops a
//! running search; Esc stops it too, a second
//! Esc (or the button, Strg+F, any change of folder) ends it and shows the
//! folder again. Hidden files are searched when the window shows them.
//! „Erweiterte Suche …“ starts Querencia Dateisuche with folder and name.

pub mod ergebnisse;
pub mod sucher;

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::time::Instant;

use gtk4::prelude::*;
use gtk4::{gdk, gio, glib};
use querencia_oberflaeche::symbole;

use crate::ansichtskern::protokoll;
use crate::app::Register;
use crate::eintrag::Eintrag;
use crate::fenster::Fenster;
use crate::i18n::{gettext, gettext_f, ngettext_f};
use ergebnisse::{Ergebnisse, Handlung};
use sucher::Meldung;

const STIL: &str = "window.querencia-dateien box.querencia-suchleiste { padding: 4px 8px; \
background-color: @theme_bg_color; border-bottom: 1px solid @borders; }";

/// The search of one window.
pub struct Suche {
    fenster: Weak<Fenster>,
    leiste: gtk4::Box,
    feld: gtk4::Entry,
    /// „Auch Dateiinhalt“ (0.3.0).
    inhalt: gtk4::CheckButton,
    drehen: gtk4::Spinner,
    ergebnisse: Rc<Ergebnisse>,
    /// Number of the running (or last) search.
    nummer: Cell<u64>,
    abbruch: RefCell<Option<gio::Cancellable>>,
    laeuft: Cell<bool>,
    /// The results are shown instead of the views.
    zeigt: Cell<bool>,
    /// The views' stack the results stand in for (the tab's, Welle C3).
    verdeckt: RefCell<Option<glib::WeakRef<gtk4::Stack>>>,
    beginn: Cell<Option<Instant>>,
    /// The status bar's text of the search without a selection of hits
    /// („Suche …“, „3 Treffer“).
    stand: RefCell<String>,
    selbst: Weak<Suche>,
}

thread_local! {
    static SUCHEN: RefCell<Vec<Weak<Suche>>> = const { RefCell::new(Vec::new()) };
}

/// The program of „Erweiterte Suche …“ (PATH, then `~/opt/querencia/bin`).
fn dateisuche() -> Option<std::path::PathBuf> {
    glib::find_program_in_path("querencia-dateisuche").or_else(|| {
        let p = glib::home_dir().join("opt/querencia/bin/querencia-dateisuche");
        p.is_file().then_some(p)
    })
}

/// The toolbar's „Suchen“ button (a toggle of `win.suchen`), placed after
/// „Rechner“ (B §3.4).
pub fn werkzeugknopf() -> gtk4::ToggleButton {
    let k = gtk4::ToggleButton::new();
    k.set_child(Some(&symbole::image_named("system-search", 16)));
    let tipp = gettext("Search documents and folders by name");
    k.set_tooltip_text(Some(&tipp));
    k.set_action_name(Some("win.suchen"));
    k.set_has_frame(false);
    k.set_can_focus(false);
    k.update_property(&[gtk4::accessible::Property::Label(&tipp)]);
    k
}

struct FensterHandlung(Weak<Suche>);

impl Handlung for FensterHandlung {
    fn oeffnen(&self, eintraege: &[Eintrag]) {
        if let Some(s) = self.0.upgrade()
            && let Some(f) = s.fenster.upgrade()
            && !eintraege.is_empty()
        {
            crate::oeffnen::oeffnen(&f, eintraege, false);
        }
    }

    fn im_ordner_zeigen(&self, eintrag: &Eintrag) {
        let Some(s) = self.0.upgrade() else { return };
        let (Some(f), Some(ordner)) = (s.fenster.upgrade(), eintrag.datei().parent()) else {
            return;
        };
        s.beenden();
        f.gehe_zu(&ordner);
        let datei = eintrag.datei().clone();
        let weak = Rc::downgrade(&f);
        f.modell()
            .bei_erscheinen(&eintrag.datei().clone(), move |_| {
                if let Some(f) = weak.upgrade() {
                    glib::idle_add_local_once(move || f.ansicht().fokus_auf(&datei, true));
                }
            });
    }

    fn escape(&self) {
        if let Some(s) = self.0.upgrade() {
            if s.laeuft.get() {
                s.stoppen();
            } else {
                s.beenden();
            }
        }
    }
}

impl Suche {
    fn new(fenster: &Rc<Fenster>) -> Rc<Self> {
        let leiste = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        leiste.add_css_class("querencia-suchleiste");
        leiste.append(&symbole::image_named("system-search", 16));
        let titel = gtk4::Label::with_mnemonic(&gettext("_Search for:"));
        leiste.append(&titel);
        let feld = gtk4::Entry::new();
        feld.set_hexpand(true);
        titel.set_mnemonic_widget(Some(&feld));
        leiste.append(&feld);
        let inhalt = gtk4::CheckButton::with_mnemonic(&gettext("Also file _contents"));
        inhalt.set_tooltip_text(Some(&gettext(
            "Also search the text of text files (up to 10 MB each)",
        )));
        leiste.append(&inhalt);
        let drehen = gtk4::Spinner::new();
        drehen.set_visible(false);
        leiste.append(&drehen);
        let suche = Rc::new_cyclic(|weak: &Weak<Suche>| Self {
            fenster: Rc::downgrade(fenster),
            leiste,
            feld,
            inhalt,
            drehen,
            ergebnisse: Ergebnisse::new(Rc::new(FensterHandlung(weak.clone()))),
            nummer: Cell::new(0),
            abbruch: RefCell::default(),
            laeuft: Cell::new(false),
            zeigt: Cell::new(false),
            verdeckt: RefCell::default(),
            beginn: Cell::new(None),
            stand: RefCell::default(),
            selbst: weak.clone(),
        });
        if dateisuche().is_some() {
            let erweitert = gtk4::Button::with_mnemonic(&gettext("_Advanced Search…"));
            erweitert.set_tooltip_text(Some(&gettext(
                "Search with more criteria in Querencia File Search",
            )));
            let weak = Rc::downgrade(&suche);
            erweitert.connect_clicked(move |_| {
                if let Some(s) = weak.upgrade() {
                    s.erweitert();
                }
            });
            suche.leiste.append(&erweitert);
        }
        let zu = gtk4::Button::new();
        zu.set_child(Some(&symbole::image_named("window-close", 16)));
        zu.set_has_frame(false);
        zu.set_tooltip_text(Some(&gettext("Close the search")));
        zu.set_action_name(Some("win.suchen"));
        suche.leiste.append(&zu);
        suche.leiste.set_visible(false);
        suche.ergebnisse.widget().set_visible(false);
        // a selection of hits shows in the status bar as in a folder
        let weak = Rc::downgrade(&suche);
        suche.ergebnisse.bei_auswahl(move || {
            if let Some(s) = weak.upgrade()
                && s.zeigt.get()
                && let Some(f) = s.fenster()
            {
                f.statusleiste().aktualisieren();
            }
        });

        // the switch searches again when hits are shown
        let weak = Rc::downgrade(&suche);
        suche.inhalt.connect_toggled(move |k| {
            let Some(s) = weak.upgrade() else { return };
            protokoll(&format!(
                "Suche: Dateiinhalt {}",
                if k.is_active() { "an" } else { "aus" }
            ));
            let text = s.feld.text();
            if s.zeigt.get() && !text.trim().is_empty() {
                s.starten(&text);
            }
        });
        // every change of the text stops a running search (the hits of the
        // old text would not match the field any more)
        let weak = Rc::downgrade(&suche);
        suche.feld.connect_changed(move |_| {
            if let Some(s) = weak.upgrade()
                && s.laeuft.get()
            {
                protokoll("Suche bei Eingabe angehalten");
                s.stoppen();
            }
        });
        let weak = Rc::downgrade(&suche);
        suche.feld.connect_activate(move |feld| {
            if let Some(s) = weak.upgrade() {
                s.starten(&feld.text());
            }
        });
        let tasten = gtk4::EventControllerKey::new();
        let weak = Rc::downgrade(&suche);
        tasten.connect_key_pressed(move |_, taste, _, _| {
            let Some(s) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            // ↓ in the field goes to the hits
            if matches!(taste, gdk::Key::Down | gdk::Key::KP_Down) && s.zeigt.get() {
                s.ergebnisse.fokus();
                return glib::Propagation::Stop;
            }
            if taste == gdk::Key::Escape {
                if s.laeuft.get() {
                    s.stoppen();
                } else {
                    s.beenden();
                }
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        suche.feld.add_controller(tasten);
        suche
    }

    /// Places bar and results into the view area of the tab shown: trash
    /// bar, (search bar), views, (results). Each tab has its own area
    /// (Welle C3); the search moves along when it opens in another tab.
    fn einbauen(&self, f: &Fenster) {
        let stapel = f.ansichten().widget().clone();
        let Some(bereich) = stapel.parent().and_downcast::<gtk4::Box>() else {
            crate::log("Suche: Ansichtsbereich nicht gefunden");
            return;
        };
        if self.leiste.parent().as_ref() == Some(bereich.upcast_ref()) {
            return;
        }
        for w in [
            self.leiste.upcast_ref::<gtk4::Widget>(),
            self.ergebnisse.widget().upcast_ref(),
        ] {
            if let Some(alt) = w.parent().and_downcast::<gtk4::Box>() {
                alt.remove(w);
            }
        }
        bereich.insert_child_after(&self.leiste, bereich.first_child().as_ref());
        bereich.insert_child_after(self.ergebnisse.widget(), Some(&stapel));
    }

    fn fenster(&self) -> Option<Rc<Fenster>> {
        self.fenster.upgrade()
    }

    /// Shows the search bar (Strg+F, the button).
    pub fn oeffnen(&self) {
        if let Some(f) = self.fenster() {
            self.einbauen(&f);
        }
        protokoll("Suchleiste geöffnet");
        self.leiste.set_visible(true);
        self.feld.grab_focus();
        self.feld.select_region(0, -1);
    }

    /// Ends the search: bar and results go, the folder is shown again.
    pub fn beenden(&self) {
        self.stoppen();
        // later reports of the stopped search are dropped
        self.nummer.set(0);
        self.laeuft.set(false);
        self.drehen.set_spinning(false);
        self.drehen.set_visible(false);
        let war_offen = self.leiste.is_visible();
        protokoll("Suche beendet");
        self.leiste.set_visible(false);
        self.ergebnisse_zeigen(false);
        self.ergebnisse.leeren();
        if let Some(f) = self.fenster() {
            if let Some(a) = f.window().lookup_action("suchen")
                && a.state().and_then(|s| s.get::<bool>()) == Some(true)
            {
                a.change_state(&false.to_variant());
            }
            if war_offen {
                f.ansicht().fokus();
            }
        }
    }

    fn ergebnisse_zeigen(&self, an: bool) {
        let Some(f) = self.fenster() else { return };
        if self.zeigt.replace(an) == an {
            return;
        }
        // the stack hidden is the one shown again, also after a change of
        // tab ended the search (Welle C3)
        if an {
            let stapel = f.ansichten().widget().clone();
            stapel.set_visible(false);
            self.verdeckt.replace(Some(stapel.downgrade()));
        } else if let Some(stapel) = self.verdeckt.take().and_then(|w| w.upgrade()) {
            stapel.set_visible(true);
        }
        self.ergebnisse.widget().set_visible(an);
        if an {
            f.window().set_title(Some(&gettext("Search results")));
            let weak = self.selbst.clone();
            f.statusleiste()
                .ersatz_setzen(Some(Box::new(move || status(&weak))));
            // the window's file actions act on the folder's selection
            f.modell().auswahl().unselect_all();
        } else {
            if let Some(ort) = f.ort() {
                f.window().set_title(Some(&crate::orte::anzeigename(&ort)));
            }
            // the folder's line again (QD-173 P-3: it stayed empty)
            f.statusleiste().ersatz_setzen(None);
        }
    }

    /// The status bar's text of the search (no selection of hits).
    fn stand_setzen(&self, f: &Fenster, text: String) {
        self.stand.replace(text);
        f.statusleiste().aktualisieren();
    }

    /// Searches the shown folder for `text`.
    pub fn starten(&self, text: &str) {
        let Some(f) = self.fenster() else { return };
        self.einbauen(&f);
        let worte = sucher::worte(text);
        self.stoppen();
        if worte.is_empty() {
            return;
        }
        let Some(ort) = f.ort() else { return };
        let verborgene = f
            .app()
            .map(|a| a.konfig().werte().verborgene)
            .unwrap_or(false);
        let inhalt = self.inhalt.is_active();
        self.ergebnisse.fundstellen_zeigen(inhalt);
        let nummer = sucher::neue_nummer();
        self.nummer.set(nummer);
        let abbruch = gio::Cancellable::new();
        self.abbruch.replace(Some(abbruch.clone()));
        self.ergebnisse.leeren();
        self.ergebnisse_zeigen(true);
        self.laeuft.set(true);
        self.drehen.set_visible(true);
        self.drehen.set_spinning(true);
        self.beginn.set(Some(Instant::now()));
        self.stand_setzen(&f, gettext("Searching…"));
        protokoll(&format!(
            "Suche {nummer}: „{}“ ab {} (verborgene {verborgene}, Inhalt {inhalt})",
            text,
            ort.parse_name()
        ));
        if let Err(e) = sucher::starten(nummer, ort, worte, verborgene, inhalt, abbruch, empfangen)
        {
            crate::log(&format!("Suche: Faden nicht gestartet: {e}"));
            self.laeuft.set(false);
        }
    }

    /// Stops a running search; the hits so far stay.
    pub fn stoppen(&self) {
        if let Some(c) = self.abbruch.take() {
            c.cancel();
        }
    }

    fn melden(&self, m: Meldung) {
        let Some(f) = self.fenster() else { return };
        match m {
            Meldung::Funde(funde) => {
                let neu: Vec<Eintrag> = funde.iter().map(sucher::Fund::eintrag).collect();
                let stellen: Vec<(String, sucher::Fundstelle)> = funde
                    .iter()
                    .zip(&neu)
                    .filter_map(|(f, e)| {
                        f.fundstelle
                            .clone()
                            .map(|s| (e.datei().uri().to_string(), s))
                    })
                    .collect();
                if crate::operationen::debugging() {
                    for (uri, s) in &stellen {
                        protokoll(&format!("Fundstelle: {uri} Zeile {}: {}", s.zeile, s.text));
                    }
                }
                self.ergebnisse.anhaengen(&neu, stellen);
                let n = self.ergebnisse.anzahl();
                self.stand_setzen(
                    &f,
                    ngettext_f(
                        "Searching… {n} match so far",
                        "Searching… {n} matches so far",
                        n,
                        &[],
                    ),
                );
            }
            Meldung::Ende {
                eintraege,
                ordner,
                fehler,
                ms,
                abgebrochen,
                begrenzt,
                inhalte,
            } => {
                self.laeuft.set(false);
                self.abbruch.replace(None);
                self.drehen.set_spinning(false);
                self.drehen.set_visible(false);
                let n = self.ergebnisse.anzahl();
                let treffer = ngettext_f("{n} match", "{n} matches", n, &[]);
                let text = if abgebrochen {
                    gettext_f("{matches} (search stopped)", &[("matches", &treffer)])
                } else if begrenzt {
                    gettext_f("{matches} (limit reached)", &[("matches", &treffer)])
                } else {
                    treffer
                };
                self.stand_setzen(&f, text);
                let gesamt = self.beginn.take().map_or(0, |b| b.elapsed().as_millis());
                protokoll(&format!(
                    "Suche {} fertig: {n} Treffer, {eintraege} Einträge in {ordner} Ordnern \
                     ({fehler} nicht lesbar), {inhalte} Inhalte gelesen, Faden {ms} ms, \
                     bis zur Anzeige {gesamt} ms{}{}",
                    self.nummer.get(),
                    if abgebrochen { ", angehalten" } else { "" },
                    if begrenzt { ", Grenze erreicht" } else { "" }
                ));
            }
        }
    }

    /// „Erweiterte Suche …“: Querencia Dateisuche with folder and name.
    fn erweitert(&self) {
        let (Some(f), Some(programm)) = (self.fenster(), dateisuche()) else {
            return;
        };
        let mut args: Vec<std::ffi::OsString> = vec![programm.into()];
        if let Some(p) = f.ort().and_then(|o| o.path()) {
            args.push("--ordner".into());
            args.push(p.into());
        }
        let text = self.feld.text();
        if !text.trim().is_empty() {
            args.push("--name".into());
            args.push(format!("*{}*", text.trim()).into());
        }
        let args: Vec<&std::ffi::OsStr> = args.iter().map(|a| a.as_os_str()).collect();
        if let Err(e) = gio::Subprocess::newv(&args, gio::SubprocessFlags::NONE) {
            f.melden(&gettext_f(
                "Querencia File Search could not be started: {error}",
                &[("error", e.message())],
            ));
        }
    }
}

/// The status bar while hits are shown: the selection of hits as in a
/// folder („„a.txt“ ausgewählt (12,3 kB)“, a hit in the text with its
/// line), else the search's own text.
fn status(suche: &Weak<Suche>) -> String {
    let Some(s) = suche.upgrade() else {
        return String::new();
    };
    let gewaehlt = s.ergebnisse.gewaehlte();
    if gewaehlt.is_empty() {
        return s.stand.borrow().clone();
    }
    let g: Vec<crate::regeln::Gewaehlt> = gewaehlt.iter().map(|e| e.gewaehlt()).collect();
    let text = crate::regeln::status_mit_auswahl(&g);
    // one hit in the text: where (0.3.0)
    match gewaehlt.as_slice() {
        [e] => match s.ergebnisse.fundstelle(e.datei().uri().as_str()) {
            Some(f) => format!("{text} – {}", ergebnisse::fundstelle_tipp(&f)),
            None => text,
        },
        _ => text,
    }
}

/// A report of a search thread (main thread).
fn empfangen(nummer: u64, m: Meldung) {
    let suche = SUCHEN.with(|s| {
        let mut s = s.borrow_mut();
        s.retain(|w| w.upgrade().is_some());
        s.iter()
            .filter_map(Weak::upgrade)
            .find(|x| x.nummer.get() == nummer)
    });
    // reports of a replaced search are dropped
    if let Some(s) = suche {
        s.melden(m);
    }
}

/// Builds the search of a new window: bar and results in the view area,
/// the action `win.suchen` (state: bar shown).
fn einrichten(f: &Rc<Fenster>) {
    let suche = Suche::new(f);
    suche.einbauen(f);
    SUCHEN.with(|s| s.borrow_mut().push(Rc::downgrade(&suche)));
    let aktion = gio::SimpleAction::new_stateful("suchen", None, &false.to_variant());
    aktion.connect_activate(|a, _| {
        let an = a.state().and_then(|s| s.get::<bool>()).unwrap_or(false);
        a.change_state(&(!an).to_variant());
    });
    let weak = Rc::downgrade(&suche);
    aktion.connect_change_state(move |a, wert| {
        let Some(an) = wert.and_then(|w| w.get::<bool>()) else {
            return;
        };
        a.set_state(&an.to_variant());
        if let Some(s) = weak.upgrade() {
            if an {
                s.oeffnen();
            } else if s.leiste.is_visible() || s.zeigt.get() {
                s.beenden();
            }
        }
    });
    f.window().add_action(&aktion);
    // any change of folder ends the search (the window shows the folder)
    let weak = Rc::downgrade(&suche);
    f.bei_ortswechsel(move |_| {
        if let Some(s) = weak.upgrade()
            && (s.leiste.is_visible() || s.zeigt.get())
        {
            s.beenden();
        }
    });
    // the window holds the search as long as it lives
    let halten = RefCell::new(Some(suche));
    f.window().connect_destroy(move |_| {
        if let Some(s) = halten.take() {
            s.stoppen();
        }
    });
}

// Welle C2 (dateien-miniaturen-suche): registered in main.rs.
/// Search bar per window, Strg+F, the bar's style.
pub fn registrieren(register: &mut Register) {
    register.fenster(einrichten);
    register.anwendung(|app| {
        app.gtk()
            .set_accels_for_action("win.suchen", &["<Control>f"]);
        let css = gtk4::CssProvider::new();
        css.load_from_data(STIL);
        if let Some(d) = gdk::Display::default() {
            gtk4::style_context_add_provider_for_display(
                &d,
                &css,
                gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    });
}
