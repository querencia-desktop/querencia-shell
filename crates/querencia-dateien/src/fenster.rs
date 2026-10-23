//! One browser window (B §3.2): menu bar, toolbar, location bar (path
//! buttons or entry), side pane with places, the view switch, the trash
//! bar and the status bar. The window owns its folder model and its
//! history; views report through [`Rueckruf`].
//!
//! Public for the modules of Welle B2/B3 (`docs/dateien.md` §3.4):
//! [`Fenster::window`], [`Fenster::ort`], [`Fenster::modell`],
//! [`Fenster::ansicht`], [`Fenster::gehe_zu`], [`Fenster::bei_ortswechsel`],
//! [`Fenster::bei_auswahl`], [`Fenster::nach_erscheinen_umbenennen`],
//! [`Fenster::melden`], [`Fenster::ansichtsbereich`].

use std::cell::{OnceCell, RefCell};
use std::rc::{Rc, Weak};

use gtk4::prelude::*;
use gtk4::{gdk, gio, glib};
use querencia_dateiarbeit::auftraege::{Job, Op};

use crate::adressleiste::{Adressleiste, Verlauf};
use crate::ansicht::{Ansicht, Ansichten, Kontextziel, Neu, Rueckruf};
use crate::app::App;
use crate::eintrag::Eintrag;
use crate::i18n::gettext;
use crate::konfig::Werte;
use crate::modell::{Ordnermodell, Zustand};
use crate::regeln::{Ansichtsart, Kriterium, Sortierung, Zoom};
use crate::seitenleiste::Seitenleiste;
use crate::statusleiste::Statusleiste;
use crate::werkzeugleiste::Werkzeugleiste;
use crate::{menue, oeffnen};

type OrtHandler = Rc<dyn Fn(&gio::File)>;
type AuswahlHandler = Rc<dyn Fn()>;

/// A browser window.
pub struct Fenster {
    app: Weak<App>,
    window: gtk4::ApplicationWindow,
    modell: Ordnermodell,
    ansichten: OnceCell<Ansichten>,
    verlauf: RefCell<Verlauf>,
    menue: OnceCell<menue::Menueleiste>,
    werkzeugleiste: OnceCell<Werkzeugleiste>,
    adressleiste: OnceCell<Rc<Adressleiste>>,
    seitenleiste: OnceCell<Rc<Seitenleiste>>,
    statusleiste: OnceCell<Rc<Statusleiste>>,
    /// Holds the views; keys that act on files are bound here.
    bereich: gtk4::Box,
    papierkorb_leiste: gtk4::Box,
    paned: gtk4::Paned,
    ort_handler: RefCell<Vec<OrtHandler>>,
    auswahl_handler: RefCell<Vec<AuswahlHandler>>,
}

/// The window's side of the view callbacks.
struct FensterRueckruf(Weak<Fenster>);

impl Rueckruf for FensterRueckruf {
    fn aktivieren(&self, eintraege: &[Eintrag], neues_fenster: bool) {
        if let Some(f) = self.0.upgrade() {
            oeffnen::oeffnen(&f, eintraege, neues_fenster);
        }
    }

    fn kontextmenue(&self, ziel: Kontextziel, widget: &gtk4::Widget, x: f64, y: f64) {
        if let Some(f) = self.0.upgrade()
            && let Some(app) = f.app.upgrade()
            && let Some(haken) = app.register().kontextmenue.clone()
        {
            haken(&f, ziel, widget, x, y);
        }
    }

    fn adresse_tippen(&self, anfang: &str) {
        if let Some(f) = self.0.upgrade()
            && let Some(a) = f.adressleiste.get()
        {
            a.textfeld_oeffnen(Some(anfang));
        }
    }

    fn zoom_geaendert(&self, zoom: Zoom) {
        if let Some(f) = self.0.upgrade() {
            f.zoom_anzeigen(zoom);
        }
    }

    fn nach_oben(&self) {
        if let Some(f) = self.0.upgrade() {
            f.nach_oben();
        }
    }

    fn umbenennen(&self, datei: &gio::File, neuer_name: &str) {
        if let Some(f) = self.0.upgrade() {
            f.umbenennen(datei, neuer_name);
        }
    }

    fn melden(&self, text: &str) {
        if let Some(f) = self.0.upgrade() {
            f.melden(text);
        }
    }
}

impl Fenster {
    pub(crate) fn new(app: &Rc<App>, ort: &gio::File) -> Rc<Self> {
        let werte = app.konfig().werte();
        let window = gtk4::ApplicationWindow::builder()
            .application(app.gtk())
            .default_width(werte.breite)
            .default_height(werte.hoehe)
            .icon_name(crate::APP_ID)
            .build();
        window.add_css_class("querencia-dateien");
        let modell = Ordnermodell::new(
            Sortierung {
                kriterium: werte.anordnen,
                ordner_zuerst: werte.ordner_zuerst,
                umgekehrt: false,
            },
            werte.verborgene,
        );
        let bereich = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        bereich.set_hexpand(true);
        bereich.set_vexpand(true);
        let fenster = Rc::new(Self {
            app: Rc::downgrade(app),
            window,
            modell,
            ansichten: OnceCell::new(),
            verlauf: RefCell::default(),
            menue: OnceCell::new(),
            werkzeugleiste: OnceCell::new(),
            adressleiste: OnceCell::new(),
            seitenleiste: OnceCell::new(),
            statusleiste: OnceCell::new(),
            bereich,
            papierkorb_leiste: gtk4::Box::new(gtk4::Orientation::Horizontal, 6),
            paned: gtk4::Paned::new(gtk4::Orientation::Horizontal),
            ort_handler: RefCell::default(),
            auswahl_handler: RefCell::default(),
        });
        fenster.aufbauen(app, &werte);
        fenster.gehe_zu(ort);
        fenster
    }

    fn aufbauen(self: &Rc<Self>, app: &Rc<App>, werte: &Werte) {
        let weak = Rc::downgrade(self);
        let rueckruf: Rc<dyn Rueckruf> = Rc::new(FensterRueckruf(weak.clone()));
        let bei_neu: Vec<Neu> = app
            .register()
            .neue_ansicht
            .iter()
            .map(|h| {
                let h = h.clone();
                let weak = weak.clone();
                Rc::new(move |a: &Rc<dyn Ansicht>| {
                    if let Some(f) = weak.upgrade() {
                        h(&f, a);
                    }
                }) as Neu
            })
            .collect();
        let ansichten = Ansichten::new(
            &self.modell,
            rueckruf,
            app.register().ansichten.clone(),
            bei_neu,
            werte,
        );

        self.aktionen_einrichten(app);
        let menue = menue::Menueleiste::new(self);
        let werkzeugleiste = Werkzeugleiste::new(self);
        let adressleiste = Adressleiste::new(self);
        let seitenleiste = Seitenleiste::new(self);
        let statusleiste = Statusleiste::new(self);

        // the trash bar (B §1.10): above the view while the trash is shown
        let titel = gtk4::Label::new(Some(&gettext("Trash")));
        titel.add_css_class("heading");
        titel.set_hexpand(true);
        titel.set_xalign(0.0);
        let leeren = gtk4::Button::with_label(&gettext("Empty Trash…"));
        leeren.set_action_name(Some("win.papierkorb-leeren"));
        self.papierkorb_leiste.append(&titel);
        self.papierkorb_leiste.append(&leeren);
        self.papierkorb_leiste
            .add_css_class("querencia-papierkorb-leiste");
        self.papierkorb_leiste.set_visible(false);

        self.bereich.append(&self.papierkorb_leiste);
        self.bereich.append(ansichten.widget());
        self.tasten_im_bereich();

        self.paned.set_start_child(Some(seitenleiste.widget()));
        self.paned.set_end_child(Some(&self.bereich));
        self.paned.set_resize_start_child(false);
        self.paned.set_shrink_start_child(false);
        self.paned.set_position(werte.seitenleiste_breite);
        seitenleiste.widget().set_visible(werte.seitenleiste);

        let inhalt = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        inhalt.append(menue.widget());
        inhalt.append(werkzeugleiste.widget());
        inhalt.append(adressleiste.widget());
        inhalt.append(&self.paned);
        inhalt.append(statusleiste.widget());
        self.paned.set_vexpand(true);
        werkzeugleiste.widget().set_visible(werte.werkzeugleiste);
        adressleiste.widget().set_visible(werte.adressleiste);
        statusleiste.widget().set_visible(werte.statusleiste);
        self.window.set_child(Some(&inhalt));

        let _ = self.menue.set(menue);
        let _ = self.werkzeugleiste.set(werkzeugleiste);
        let _ = self.adressleiste.set(adressleiste);
        let _ = self.seitenleiste.set(seitenleiste);
        let _ = self.statusleiste.set(statusleiste);
        let _ = self.ansichten.set(ansichten);
        self.ansicht_umschalten(werte.ansicht);

        self.signale(app);
    }

    fn signale(self: &Rc<Self>, app: &Rc<App>) {
        let weak = Rc::downgrade(self);
        self.modell.bei_zustand(move |z| {
            if let Some(f) = weak.upgrade() {
                f.zustand(z);
            }
        });
        let weak = Rc::downgrade(self);
        self.modell
            .auswahl()
            .connect_selection_changed(move |_, _, _| {
                if let Some(f) = weak.upgrade() {
                    f.auswahl_geaendert();
                }
            });
        let weak = Rc::downgrade(self);
        self.modell
            .auswahl()
            .connect_items_changed(move |_, _, _, _| {
                if let Some(f) = weak.upgrade() {
                    f.auswahl_geaendert();
                }
            });
        // settings from the dialog or another window
        let weak = Rc::downgrade(self);
        app.konfig().bei_aenderung(move |w| {
            if let Some(f) = weak.upgrade() {
                f.einstellungen_anwenden(w);
            }
        });
        // remember size and pane width when the window closes
        let weak = Rc::downgrade(self);
        self.window.connect_close_request(move |w| {
            if let (Some(f), Some(app)) = (weak.upgrade(), crate::app::app_wenn()) {
                let (breite, hoehe) = (w.default_width(), w.default_height());
                let seite = f.paned.position();
                app.konfig().aendern(|k| {
                    k.breite = breite;
                    k.hoehe = hoehe;
                    if f.seitenleiste().widget().is_visible() {
                        k.seitenleiste_breite = seite;
                    }
                });
            }
            glib::Propagation::Proceed
        });
        // mouse buttons 8/9: back and forward (B §1.1)
        let tasten = gtk4::GestureClick::new();
        tasten.set_button(0);
        tasten.set_propagation_phase(gtk4::PropagationPhase::Capture);
        let weak = Rc::downgrade(self);
        tasten.connect_pressed(move |g, _, _, _| {
            let Some(f) = weak.upgrade() else { return };
            match g.current_button() {
                8 => {
                    f.zurueck();
                    g.set_state(gtk4::EventSequenceState::Claimed);
                }
                9 => {
                    f.vor();
                    g.set_state(gtk4::EventSequenceState::Claimed);
                }
                _ => {}
            }
        });
        self.window.add_controller(tasten);
        let weak = Rc::downgrade(self);
        app.papierkorb().connect_changed(move |_| {
            if let Some(f) = weak.upgrade() {
                f.aktionen_pruefen();
            }
        });
    }

    /// Keys that act on files, only while the focus is in the view area:
    /// as application accels they would also fire in the location entry
    /// and in rename fields (GTK runs those in the capture phase).
    fn tasten_im_bereich(&self) {
        let sc = gtk4::ShortcutController::new();
        sc.set_scope(gtk4::ShortcutScope::Local);
        sc.set_propagation_phase(gtk4::PropagationPhase::Bubble);
        for (aktion, kuerzel) in menue::KUERZEL_IM_BEREICH {
            for k in *kuerzel {
                if let Some(trigger) = gtk4::ShortcutTrigger::parse_string(k) {
                    sc.add_shortcut(gtk4::Shortcut::new(
                        Some(trigger),
                        Some(gtk4::NamedAction::new(&format!("win.{aktion}"))),
                    ));
                }
            }
        }
        self.bereich.add_controller(sc);
    }

    // ------------------------------------------------------------ accessors

    pub fn window(&self) -> &gtk4::ApplicationWindow {
        &self.window
    }

    pub fn app(&self) -> Option<Rc<App>> {
        self.app.upgrade()
    }

    /// The folder shown.
    pub fn ort(&self) -> Option<gio::File> {
        self.modell.ordner()
    }

    pub fn modell(&self) -> &Ordnermodell {
        &self.modell
    }

    /// The view shown.
    pub fn ansicht(&self) -> Rc<dyn Ansicht> {
        self.ansichten().aktuell()
    }

    pub fn ansichten(&self) -> &Ansichten {
        self.ansichten.get().expect("Fenster ohne Ansichten")
    }

    /// The box around the views (drop targets on free space, keys).
    pub fn ansichtsbereich(&self) -> &gtk4::Box {
        &self.bereich
    }

    pub(crate) fn seitenleiste(&self) -> &Rc<Seitenleiste> {
        self.seitenleiste.get().expect("Fenster ohne Seitenleiste")
    }

    pub(crate) fn adressleiste(&self) -> &Rc<Adressleiste> {
        self.adressleiste.get().expect("Fenster ohne Adressleiste")
    }

    pub(crate) fn werkzeugleiste(&self) -> &Werkzeugleiste {
        self.werkzeugleiste
            .get()
            .expect("Fenster ohne Werkzeugleiste")
    }

    pub(crate) fn statusleiste(&self) -> &Rc<Statusleiste> {
        self.statusleiste.get().expect("Fenster ohne Statusleiste")
    }

    pub(crate) fn menueleiste(&self) -> &menue::Menueleiste {
        self.menue.get().expect("Fenster ohne Menüleiste")
    }

    pub(crate) fn verlauf(&self) -> std::cell::Ref<'_, Verlauf> {
        self.verlauf.borrow()
    }

    /// `f` runs after every change of folder (also back/forward/reload).
    pub fn bei_ortswechsel(&self, f: impl Fn(&gio::File) + 'static) {
        self.ort_handler.borrow_mut().push(Rc::new(f));
    }

    /// `f` runs after every change of selection or of the shown entries.
    pub fn bei_auswahl(&self, f: impl Fn() + 'static) {
        self.auswahl_handler.borrow_mut().push(Rc::new(f));
    }

    /// A short message in the status bar (until the next change).
    pub fn melden(&self, text: &str) {
        if let Some(s) = self.statusleiste.get() {
            s.melden(text);
        }
    }

    /// When `datei` appears in the shown folder: select it, show it and
    /// start renaming it (new folder, new document; B §5.1 D18/D19).
    pub fn nach_erscheinen_umbenennen(self: &Rc<Self>, datei: &gio::File) {
        let weak = Rc::downgrade(self);
        let datei2 = datei.clone();
        self.modell.bei_erscheinen(datei, move |_| {
            let Some(f) = weak.upgrade() else { return };
            let datei = datei2.clone();
            // after the view has its cell for the new entry
            glib::idle_add_local_once(move || {
                let a = f.ansicht();
                a.fokus_auf(&datei, true);
                a.umbenennen_anstossen(&datei, false);
            });
        });
    }

    /// When `dateien` appear in the shown folder: select them (results of
    /// copy, move, paste).
    pub fn nach_erscheinen_waehlen(self: &Rc<Self>, dateien: &[gio::File]) {
        for d in dateien {
            let weak = Rc::downgrade(self);
            let alle = dateien.to_vec();
            self.modell.bei_erscheinen(d, move |_| {
                if let Some(f) = weak.upgrade() {
                    let da: Vec<gio::File> = alle
                        .iter()
                        .filter(|x| f.modell.eintrag_von(x).is_some())
                        .cloned()
                        .collect();
                    f.modell.waehlen(&da);
                }
            });
        }
    }

    // ------------------------------------------------------------ navigation

    /// Shows `ort` in this window (history: a new step).
    pub fn gehe_zu(self: &Rc<Self>, ort: &gio::File) {
        if self.ort().is_some_and(|o| o.equal(ort)) {
            return;
        }
        self.verlauf.borrow_mut().besuchen(ort);
        self.laden(ort, None);
    }

    fn laden(self: &Rc<Self>, ort: &gio::File, markieren: Option<gio::File>) {
        self.modell.laden(ort);
        if let Some(kind) = markieren {
            let weak = Rc::downgrade(self);
            let k = kind.clone();
            self.modell.bei_erscheinen(&kind, move |_| {
                if let Some(f) = weak.upgrade() {
                    let k = k.clone();
                    glib::idle_add_local_once(move || f.ansicht().fokus_auf(&k, true));
                }
            });
        }
        self.titel_setzen(ort);
        if let Some(a) = self.adressleiste.get() {
            a.ort_anzeigen(ort);
        }
        if let Some(s) = self.seitenleiste.get() {
            s.ort_anzeigen(ort);
        }
        self.papierkorb_leiste
            .set_visible(ort.uri_scheme().as_deref() == Some("trash"));
        self.aktionen_pruefen();
        let handler: Vec<OrtHandler> = self.ort_handler.borrow().clone();
        for h in handler {
            h(ort);
        }
    }

    pub fn zurueck(self: &Rc<Self>) {
        let ziel = self.verlauf.borrow_mut().zurueck();
        if let Some((ort, kind)) = ziel {
            self.laden(&ort, kind);
        }
    }

    pub fn vor(self: &Rc<Self>) {
        let ziel = self.verlauf.borrow_mut().vor();
        if let Some((ort, _)) = ziel {
            self.laden(&ort, None);
        }
    }

    /// Gehe zu → Nach oben: the parent folder, with the folder we came from
    /// selected (Nautilus 2.30).
    pub fn nach_oben(self: &Rc<Self>) {
        let Some(ort) = self.ort() else { return };
        if let Some(eltern) = ort.parent() {
            self.verlauf.borrow_mut().besuchen(&eltern);
            self.laden(&eltern, Some(ort));
        }
    }

    fn titel_setzen(&self, ort: &gio::File) {
        let titel = crate::orte::anzeigename(ort);
        self.window.set_title(Some(&titel));
        if ort.is_native() {
            return;
        }
        // GVfs places name themselves ("Papierkorb", the share's name)
        let w = self.window.downgrade();
        let ort = ort.clone();
        glib::spawn_future_local(async move {
            if let Ok(info) = ort
                .query_info_future(
                    "standard::display-name",
                    gio::FileQueryInfoFlags::NONE,
                    glib::Priority::DEFAULT,
                )
                .await
                && let Some(w) = w.upgrade()
            {
                let name = info.display_name();
                if !name.is_empty() && ort.uri_scheme().as_deref() != Some("trash") {
                    w.set_title(Some(&name));
                }
            }
        });
    }

    fn zustand(self: &Rc<Self>, z: &Zustand) {
        match z {
            Zustand::OrdnerWeg => {
                // QD-112 pattern: go to the nearest existing parent
                let Some(ort) = self.ort() else { return };
                let mut ziel = ort.parent();
                while let Some(z) = ziel.clone() {
                    if z.query_exists(gio::Cancellable::NONE) {
                        break;
                    }
                    ziel = z.parent();
                }
                let ziel = ziel.unwrap_or_else(|| gio::File::for_path(glib::home_dir()));
                self.gehe_zu(&ziel);
            }
            Zustand::ErsterBlock => {
                if self.ansicht_hat_fokus() || GtkWindowExt::focus(&self.window).is_none() {
                    self.ansicht().fokus();
                }
            }
            _ => {}
        }
        if let Some(w) = self.werkzeugleiste.get() {
            w.laedt(self.modell.laedt());
        }
        self.aktion_aktiv("stopp", self.modell.laedt());
        if let Some(s) = self.statusleiste.get() {
            s.zustand(z);
        }
    }

    fn ansicht_hat_fokus(&self) -> bool {
        GtkWindowExt::focus(&self.window).is_some_and(|f| f.is_ancestor(&self.bereich))
    }

    fn auswahl_geaendert(&self) {
        if let Some(s) = self.statusleiste.get() {
            s.aktualisieren();
        }
        self.aktionen_pruefen();
        let handler: Vec<AuswahlHandler> = self.auswahl_handler.borrow().clone();
        for h in handler {
            h();
        }
    }

    // ------------------------------------------------------------ views

    pub fn ansicht_umschalten(self: &Rc<Self>, art: Ansichtsart) {
        let a = self.ansichten().umschalten(art);
        self.zoom_anzeigen(a.zoom());
        if let Some(w) = self.werkzeugleiste.get() {
            w.ansicht_anzeigen(art);
        }
        if let Some(aktion) = self.window.lookup_action("ansicht") {
            aktion.change_state(&art.kennung().to_variant());
        }
    }

    fn zoom_anzeigen(&self, zoom: Zoom) {
        if let Some(w) = self.werkzeugleiste.get() {
            let a = self.ansicht();
            let stufen = a.zoomstufen();
            w.zoom_anzeigen(
                zoom,
                stufen.first().is_some_and(|s| *s < zoom.0),
                stufen.last().is_some_and(|s| *s > zoom.0),
            );
        }
    }

    pub fn zoom_schritt(self: &Rc<Self>, schritt: i32) {
        let a = self.ansicht();
        let neu = if schritt == 0 {
            let w = self.app().map(|a| a.konfig().werte()).unwrap_or_default();
            w.zoom_fuer(a.art())
        } else {
            a.zoom().schritt(schritt, a.zoomstufen())
        };
        a.zoom_setzen(neu);
        self.zoom_anzeigen(neu);
    }

    fn einstellungen_anwenden(self: &Rc<Self>, w: &Werte) {
        let mut s = self.modell.sortierung();
        s.kriterium = w.anordnen;
        s.ordner_zuerst = w.ordner_zuerst;
        self.modell.sortierung_setzen(s);
        self.modell.verborgene_setzen(w.verborgene);
        self.ansichten().einstellungen_anwenden(w);
        if let Some(a) = self.adressleiste.get() {
            a.einstellungen_anwenden(w);
        }
        for (name, wert) in [
            ("verborgene", w.verborgene),
            ("werkzeugleiste", w.werkzeugleiste),
            ("seitenleiste", w.seitenleiste),
            ("adressleiste", w.adressleiste),
            ("statusleiste", w.statusleiste),
        ] {
            if let Some(a) = self.window.lookup_action(name) {
                a.change_state(&wert.to_variant());
            }
        }
        if let Some(a) = self.window.lookup_action("anordnen") {
            a.change_state(&w.anordnen.kennung().to_variant());
        }
        self.aktionen_pruefen();
    }

    fn umbenennen(self: &Rc<Self>, datei: &gio::File, neuer_name: &str) {
        let Some(app) = self.app() else { return };
        let Some(alt) = datei.basename() else { return };
        if alt.to_string_lossy() == neuer_name {
            return;
        }
        let job = Job::new(
            Op::Rename {
                new: neuer_name.to_owned(),
            },
            vec![datei.uri().to_string()],
            None,
        );
        if let Some(eltern) = datei.parent() {
            let neu = eltern.child(neuer_name);
            let weak = Rc::downgrade(self);
            let ziel = neu.clone();
            self.modell.bei_erscheinen(&ziel, move |_| {
                if let Some(f) = weak.upgrade() {
                    let neu = neu.clone();
                    glib::idle_add_local_once(move || f.ansicht().fokus_auf(&neu, true));
                }
            });
        }
        app.auftrag_starten(job, Some(self.window.upcast_ref()));
    }

    // ------------------------------------------------------------ actions

    fn aktion(self: &Rc<Self>, name: &str, f: impl Fn(&Rc<Self>) + 'static) {
        let a = gio::SimpleAction::new(name, None);
        let weak = Rc::downgrade(self);
        a.connect_activate(move |_, _| {
            if let Some(fenster) = weak.upgrade() {
                f(&fenster);
            }
        });
        self.window.add_action(&a);
    }

    fn schalter(self: &Rc<Self>, name: &str, an: bool, f: impl Fn(&Rc<Self>, bool) + 'static) {
        let a = gio::SimpleAction::new_stateful(name, None, &an.to_variant());
        let weak = Rc::downgrade(self);
        a.connect_activate(|a, _| {
            let jetzt = a.state().and_then(|s| s.get::<bool>()).unwrap_or(false);
            a.change_state(&(!jetzt).to_variant());
        });
        a.connect_change_state(move |a, wert| {
            let Some(wert) = wert.and_then(|w| w.get::<bool>()) else {
                return;
            };
            if a.state().and_then(|s| s.get::<bool>()) == Some(wert) {
                return;
            }
            a.set_state(&wert.to_variant());
            if let Some(fenster) = weak.upgrade() {
                f(&fenster, wert);
            }
        });
        self.window.add_action(&a);
    }

    fn auswahl_aktion(
        self: &Rc<Self>,
        name: &str,
        anfang: &str,
        f: impl Fn(&Rc<Self>, &str) + 'static,
    ) {
        let a = gio::SimpleAction::new_stateful(
            name,
            Some(glib::VariantTy::STRING),
            &anfang.to_variant(),
        );
        let weak = Rc::downgrade(self);
        a.connect_activate(|a, wert| {
            if let Some(w) = wert {
                a.change_state(w);
            }
        });
        a.connect_change_state(move |a, wert| {
            let Some(wert) = wert.and_then(|w| w.get::<String>()) else {
                return;
            };
            let alt = a.state().and_then(|s| s.get::<String>());
            a.set_state(&wert.to_variant());
            if alt.as_deref() != Some(wert.as_str())
                && let Some(fenster) = weak.upgrade()
            {
                f(&fenster, &wert);
            }
        });
        self.window.add_action(&a);
    }

    fn aktionen_einrichten(self: &Rc<Self>, app: &Rc<App>) {
        let w = app.konfig().werte();
        // Datei
        self.aktion("schliessen", |f| f.window.close());
        self.aktion("oeffnen", |f| {
            let auswahl = f.modell.gewaehlte();
            oeffnen::oeffnen(f, &auswahl, false);
        });
        self.aktion("oeffnen-neues-fenster", |f| {
            let auswahl = f.modell.gewaehlte();
            oeffnen::oeffnen(f, &auswahl, true);
        });
        let mit = gio::SimpleAction::new("oeffnen-mit", Some(glib::VariantTy::STRING));
        let weak = Rc::downgrade(self);
        mit.connect_activate(move |_, wert| {
            if let (Some(f), Some(id)) = (weak.upgrade(), wert.and_then(|w| w.get::<String>())) {
                let auswahl = f.modell.gewaehlte();
                oeffnen::oeffnen_mit(&f, &auswahl, &id);
            }
        });
        self.window.add_action(&mit);
        self.aktion("eigenschaften", crate::eigenschaften::zeigen);
        // Bearbeiten
        self.aktion("alles-auswaehlen", |f| f.modell.alle_waehlen());
        self.aktion("auswahl-umkehren", |f| f.modell.auswahl_umkehren());
        self.aktion("umbenennen", |f| f.umbenennen_anstossen(false));
        self.aktion("umbenennen-ganz", |f| f.umbenennen_anstossen(true));
        self.aktion("einstellungen", crate::einstellungen::zeigen);
        // Ansicht
        self.aktion("stopp", |f| f.modell.stoppen());
        self.aktion("aktualisieren", |f| f.modell.neu_laden());
        self.schalter("werkzeugleiste", w.werkzeugleiste, |f, an| {
            f.werkzeugleiste().widget().set_visible(an);
            f.konfig_aendern(|k| k.werkzeugleiste = an);
        });
        self.schalter("seitenleiste", w.seitenleiste, |f, an| {
            f.seitenleiste().widget().set_visible(an);
            f.konfig_aendern(|k| k.seitenleiste = an);
        });
        self.schalter("adressleiste", w.adressleiste, |f, an| {
            f.adressleiste().widget().set_visible(an);
            f.konfig_aendern(|k| k.adressleiste = an);
        });
        self.schalter("statusleiste", w.statusleiste, |f, an| {
            f.statusleiste().widget().set_visible(an);
            f.konfig_aendern(|k| k.statusleiste = an);
        });
        self.schalter("verborgene", w.verborgene, |f, an| {
            f.modell.verborgene_setzen(an);
            f.konfig_aendern(|k| k.verborgene = an);
        });
        self.auswahl_aktion("anordnen", w.anordnen.kennung(), |f, k| {
            if let Some(k) = Kriterium::aus_kennung(k) {
                let mut s = f.modell.sortierung();
                s.kriterium = k;
                f.modell.sortierung_setzen(s);
                f.konfig_aendern(|w| w.anordnen = k);
            }
        });
        self.schalter("umgekehrt", false, |f, an| {
            let mut s = f.modell.sortierung();
            s.umgekehrt = an;
            f.modell.sortierung_setzen(s);
        });
        self.aktion("vergroessern", |f| f.zoom_schritt(1));
        self.aktion("verkleinern", |f| f.zoom_schritt(-1));
        self.aktion("normale-groesse", |f| f.zoom_schritt(0));
        self.auswahl_aktion("ansicht", w.ansicht.kennung(), |f, k| {
            if let Some(art) = Ansichtsart::aus_kennung(k) {
                f.ansicht_umschalten(art);
            }
        });
        // Gehe zu
        self.aktion("nach-oben", |f| f.nach_oben());
        self.aktion("zurueck", |f| f.zurueck());
        self.aktion("vor", |f| f.vor());
        self.aktion("persoenlich", |f| {
            f.gehe_zu(&gio::File::for_path(glib::home_dir()));
        });
        self.aktion("rechner", |f| {
            f.gehe_zu(&gio::File::for_uri("computer:///"))
        });
        self.aktion("papierkorb-zeigen", |f| {
            f.gehe_zu(&gio::File::for_uri("trash:///"));
        });
        self.aktion("ort", |f| f.adressleiste().textfeld_oeffnen(None));
        let gehe = gio::SimpleAction::new("gehe-zu", Some(glib::VariantTy::STRING));
        let weak = Rc::downgrade(self);
        gehe.connect_activate(move |_, wert| {
            if let (Some(f), Some(uri)) = (weak.upgrade(), wert.and_then(|w| w.get::<String>())) {
                f.gehe_zu(&gio::File::for_uri(&uri));
            }
        });
        self.window.add_action(&gehe);
        // Lesezeichen
        self.aktion("lesezeichen-hinzufuegen", |f| {
            let (Some(app), Some(ort)) = (f.app(), f.ort()) else {
                return;
            };
            if let Err(err) = app.lesezeichen().hinzufuegen(&ort) {
                f.melden(&crate::i18n::gettext_f(
                    "The bookmark could not be saved: {error}",
                    &[("error", &err.to_string())],
                ));
            }
        });
        // Hilfe
        self.aktion("hilfe", |f| {
            crate::info::online_hilfe(f.window.upcast_ref())
        });
        self.aktion("fehler-melden", |f| {
            crate::info::fehler_melden(f.window.upcast_ref())
        });
        self.aktion("info", |f| crate::info::zeigen(f.window.upcast_ref()));
    }

    fn konfig_aendern(&self, f: impl FnOnce(&mut Werte)) {
        if let Some(app) = self.app() {
            app.konfig().aendern(f);
        }
    }

    fn umbenennen_anstossen(self: &Rc<Self>, ganz: bool) {
        let auswahl = self.modell.gewaehlte();
        if let [eins] = auswahl.as_slice() {
            let a = self.ansicht();
            a.fokus_auf(eins.datei(), true);
            a.umbenennen_anstossen(eins.datei(), ganz);
        }
    }

    fn aktion_aktiv(&self, name: &str, an: bool) {
        if let Some(a) = self
            .window
            .lookup_action(name)
            .and_downcast::<gio::SimpleAction>()
        {
            a.set_enabled(an);
        }
    }

    /// Enables the actions of this module after a change of place,
    /// selection or history (the menus are rebuilt from the enabled state
    /// whenever one opens, `menue.rs`).
    pub(crate) fn aktionen_pruefen(&self) {
        let auswahl = self.modell.gewaehlte();
        let eine = auswahl.len() == 1;
        let ort = self.ort();
        let verlauf = self.verlauf.borrow();
        self.aktion_aktiv("zurueck", verlauf.kann_zurueck());
        self.aktion_aktiv("vor", verlauf.kann_vor());
        drop(verlauf);
        self.aktion_aktiv("nach-oben", ort.as_ref().and_then(|o| o.parent()).is_some());
        self.aktion_aktiv("oeffnen", !auswahl.is_empty());
        self.aktion_aktiv(
            "oeffnen-neues-fenster",
            auswahl.iter().any(Eintrag::ist_ordner),
        );
        self.aktion_aktiv(
            "umbenennen",
            eine && {
                let info = &auswahl[0].daten().info;
                !info.has_attribute("access::can-rename") || info.boolean("access::can-rename")
            },
        );
        self.aktion_aktiv("umbenennen-ganz", eine);
        self.aktion_aktiv("lesezeichen-hinzufuegen", ort.is_some());
        if let Some(m) = self.menue.get() {
            m.zustand_geaendert();
        }
        if let Some(w) = self.werkzeugleiste.get() {
            w.aktualisieren(self);
        }
    }
}

/// The key of a press without Strg/Alt (helper for views and bars).
pub fn ohne_strg_alt(zustand: gdk::ModifierType) -> bool {
    zustand & (gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::ALT_MASK)
        == gdk::ModifierType::empty()
}
