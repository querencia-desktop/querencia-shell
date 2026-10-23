//! Tabs (Reiter) of a window, slim as in Nautilus 2.30 (B §3.8): every
//! tab has its own folder model, views and history ([`Reiter`]); menu
//! bar, toolbar, location bar, side pane and status bar belong to the
//! window and follow the tab shown. The tabs live in a `gtk::Notebook`
//! whose tab bar shows from the second tab on ([`Reiterliste`]); the menu
//! „Reiter“ shows then, too (`menue.rs`).
//!
//! Keys (Nautilus 2.30, `nautilus-navigation-window-menus.c`): Strg+T new
//! tab for the shown folder (it becomes the current one), Strg+W closes the
//! current tab (the last one closes the window), Strg+Bild↑/↓ previous
//! and next tab, Umschalt+Strg+Bild↑/↓ moves the tab, Alt+1 … Alt+0 the
//! first ten tabs. The middle button on a folder, a path button or a place
//! and „In neuem Reiter öffnen“ open folders in new tabs behind the
//! current one; files dropped on a tab's head go into its folder. No
//! restore at start, no dragging of tabs, no detaching.

pub mod regeln;

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use gtk4::prelude::*;
use gtk4::{gdk, gio, glib};
use querencia_oberflaeche::symbole;

use crate::adressleiste::Verlauf;
use crate::ansicht::{Ansicht, Ansichten, Kontextziel, Neu, Rueckruf};
use crate::app::App;
use crate::eintrag::Eintrag;
use crate::fenster::Fenster;
use crate::i18n::{gettext, pgettext};
use crate::konfig::Werte;
use crate::menue;
use crate::modell::{Ordnermodell, Zustand};
use crate::regeln::{Ansichtsart, Sortierung, Zoom};

thread_local! {
    static NUMMER: Cell<u64> = const { Cell::new(0) };
}

/// One tab: folder model, views, history, the page in the notebook (the
/// trash bar above the views) and the head (busy indicator, name, close
/// button).
pub struct Reiter {
    pub(crate) modell: Ordnermodell,
    pub(crate) ansichten: Rc<Ansichten>,
    pub(crate) verlauf: RefCell<Verlauf>,
    seite: gtk4::Box,
    papierkorb_leiste: gtk4::Box,
    kopf: gtk4::Box,
    titel: gtk4::Label,
    drehen: gtk4::Spinner,
    /// The read error of the folder shown (status bar after a switch).
    fehler: RefCell<Option<String>>,
    /// Number in the log lines („Reiter 3“), counted per process.
    nummer: u64,
}

impl Drop for Reiter {
    fn drop(&mut self) {
        // the isolated checks read this line: a closed tab leaves nothing
        crate::log(&format!("Reiter {} freigegeben", self.nummer));
    }
}

/// The tab's side of the view callbacks: everything goes to the window,
/// renames to this tab's model.
struct ReiterRueckruf {
    fenster: Weak<Fenster>,
    reiter: Weak<Reiter>,
}

impl Rueckruf for ReiterRueckruf {
    fn aktivieren(&self, eintraege: &[Eintrag], neues_fenster: bool) {
        let Some(f) = self.fenster.upgrade() else {
            return;
        };
        // Nautilus 2.30 (fm-icon-view.c, activate_alternate): the middle
        // button opens folders in new tabs, with Umschalt in new windows;
        // Umschalt+Doppelklick is a new window as before.
        if neues_fenster && !umschalt_gedrueckt(f.window().upcast_ref()) {
            crate::oeffnen::oeffnen_in_reitern(&f, eintraege);
        } else {
            crate::oeffnen::oeffnen(&f, eintraege, neues_fenster);
        }
    }

    fn kontextmenue(&self, ziel: Kontextziel, widget: &gtk4::Widget, x: f64, y: f64) {
        if let Some(f) = self.fenster.upgrade()
            && let Some(app) = f.app()
            && let Some(haken) = app.register().kontextmenue.clone()
        {
            haken(&f, ziel, widget, x, y);
        }
    }

    fn adresse_tippen(&self, anfang: &str) {
        if let Some(f) = self.fenster.upgrade() {
            f.adressleiste().textfeld_oeffnen(Some(anfang));
        }
    }

    fn zoom_geaendert(&self, zoom: Zoom) {
        if let (Some(f), Some(r)) = (self.fenster.upgrade(), self.reiter.upgrade())
            && f.reiter_ist_aktiv(&r)
        {
            f.zoom_anzeigen(zoom);
        }
    }

    fn nach_oben(&self) {
        if let Some(f) = self.fenster.upgrade() {
            f.nach_oben();
        }
    }

    fn umbenennen(&self, datei: &gio::File, neuer_name: &str) {
        if let (Some(f), Some(r)) = (self.fenster.upgrade(), self.reiter.upgrade()) {
            f.umbenennen_in(&r, datei, neuer_name);
        }
    }

    fn melden(&self, text: &str) {
        if let Some(f) = self.fenster.upgrade() {
            f.melden(text);
        }
    }
}

/// Whether Umschalt is held now (the views report the middle button and
/// Umschalt+Doppelklick alike).
pub fn umschalt_gedrueckt(widget: &gtk4::Widget) -> bool {
    WidgetExt::display(widget)
        .default_seat()
        .and_then(|s| s.keyboard())
        .is_some_and(|k| k.modifier_state().contains(gdk::ModifierType::SHIFT_MASK))
}

impl Reiter {
    /// A tab for `fenster` with the view `art` (zoom: the view's default
    /// from `dateien.ini` unless given). Shows nothing yet; the window
    /// places it ([`Reiterliste::einfuegen`]) and loads a folder.
    pub(crate) fn neu(
        fenster: &Rc<Fenster>,
        app: &Rc<App>,
        werte: &Werte,
        art: Ansichtsart,
        zoom: Option<Zoom>,
    ) -> Rc<Self> {
        let nummer = NUMMER.with(|n| {
            n.set(n.get() + 1);
            n.get()
        });
        let modell = Ordnermodell::new(
            Sortierung {
                kriterium: werte.anordnen,
                ordner_zuerst: werte.ordner_zuerst,
                umgekehrt: false,
            },
            werte.verborgene,
        );
        let fenster_weak = Rc::downgrade(fenster);
        let reiter = Rc::new_cyclic(|weak: &Weak<Reiter>| {
            let rueckruf: Rc<dyn Rueckruf> = Rc::new(ReiterRueckruf {
                fenster: fenster_weak.clone(),
                reiter: weak.clone(),
            });
            let bei_neu: Vec<Neu> = app
                .register()
                .neue_ansicht
                .iter()
                .map(|h| {
                    let h = h.clone();
                    let f = fenster_weak.clone();
                    Rc::new(move |a: &Rc<dyn Ansicht>| {
                        if let Some(f) = f.upgrade() {
                            h(&f, a);
                        }
                    }) as Neu
                })
                .collect();
            let ansichten = Rc::new(Ansichten::new(
                &modell,
                rueckruf,
                app.register().ansichten.clone(),
                bei_neu,
                werte,
            ));
            // the trash bar (B §1.10): above the view while the trash is shown
            let papierkorb_leiste = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
            let titel = gtk4::Label::new(Some(&gettext("Trash")));
            titel.add_css_class("heading");
            titel.set_hexpand(true);
            titel.set_xalign(0.0);
            let leeren = gtk4::Button::with_label(&gettext("Empty Trash…"));
            leeren.set_action_name(Some("win.papierkorb-leeren"));
            papierkorb_leiste.append(&titel);
            papierkorb_leiste.append(&leeren);
            papierkorb_leiste.add_css_class("querencia-papierkorb-leiste");
            papierkorb_leiste.set_visible(false);
            let seite = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
            seite.set_hexpand(true);
            seite.set_vexpand(true);
            seite.append(&papierkorb_leiste);
            seite.append(ansichten.widget());
            let (kopf, titel, drehen) = kopf_bauen(weak, &fenster_weak);
            Reiter {
                modell: modell.clone(),
                ansichten,
                verlauf: RefCell::default(),
                seite,
                papierkorb_leiste,
                kopf,
                titel,
                drehen,
                fehler: RefCell::default(),
                nummer,
            }
        });
        let a = reiter.ansichten.umschalten(art);
        if let Some(z) = zoom {
            a.zoom_setzen(z);
        }
        // B03: files dropped on the head go into the tab's folder (the drop
        // rules of the path buttons, `register.ablage`)
        if let Some(ablage) = app.register().ablage.clone() {
            let weak = Rc::downgrade(&reiter);
            ablage(
                fenster,
                reiter.kopf.upcast_ref(),
                Rc::new(move |_, _| weak.upgrade().and_then(|r| r.ort())),
            );
        }
        reiter
    }

    pub fn nummer(&self) -> u64 {
        self.nummer
    }

    /// The folder shown (also while it is read).
    pub fn ort(&self) -> Option<gio::File> {
        self.modell.ordner()
    }

    /// The view shown in this tab.
    pub fn ansicht(&self) -> Rc<dyn Ansicht> {
        self.ansichten.aktuell()
    }

    /// The folder model of this tab (for `register.reiter`).
    pub fn modell(&self) -> &Ordnermodell {
        &self.modell
    }

    /// The name in the tab head (the folder's name, as the window title).
    pub fn titel(&self) -> String {
        self.titel.text().to_string()
    }

    pub(crate) fn titel_setzen(&self, name: &str) {
        self.titel.set_text(name);
        self.kopf.set_tooltip_text(Some(name));
    }

    pub(crate) fn papierkorb_zeigen(&self, an: bool) {
        self.papierkorb_leiste.set_visible(an);
    }

    /// Follows the model's state in the tab head (busy indicator) and keeps
    /// a read error for the status bar.
    pub(crate) fn zustand_merken(&self, z: &Zustand) {
        match z {
            Zustand::Laedt(_) => {
                self.fehler.replace(None);
            }
            Zustand::Fehler(text) => {
                self.fehler.replace(Some(text.clone()));
            }
            _ => {}
        }
        let laedt = self.modell.laedt();
        self.drehen.set_spinning(laedt);
        self.drehen.set_visible(laedt);
    }

    pub(crate) fn fehler(&self) -> Option<String> {
        self.fehler.borrow().clone()
    }
}

/// The tab head as Nautilus 2.30 (`nautilus-notebook.c`, build_tab_label):
/// busy indicator, the name (ellipsized), a flat close button „Schließen“.
fn kopf_bauen(
    reiter: &Weak<Reiter>,
    fenster: &Weak<Fenster>,
) -> (gtk4::Box, gtk4::Label, gtk4::Spinner) {
    let kopf = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
    kopf.add_css_class("querencia-reiterkopf");
    let drehen = gtk4::Spinner::new();
    drehen.set_visible(false);
    let titel = gtk4::Label::new(None);
    titel.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    titel.set_single_line_mode(true);
    titel.set_xalign(0.0);
    // the name fills the head, the close button sits on the right; with
    // many tabs the names shrink to „…“ (nautilus-notebook.c packs the
    // label with expand and fill)
    titel.set_hexpand(true);
    let zu = gtk4::Button::new();
    zu.set_child(Some(&symbole::image_named("window-close", 16)));
    zu.set_has_frame(false);
    zu.set_can_focus(false);
    zu.set_focus_on_click(false);
    zu.add_css_class("querencia-reiter-schliessen");
    let tipp = gettext("Close tab");
    zu.set_tooltip_text(Some(&tipp));
    zu.update_property(&[gtk4::accessible::Property::Label(&tipp)]);
    let (r, f) = (reiter.clone(), fenster.clone());
    zu.connect_clicked(move |_| {
        // after the click: the button goes with its tab
        let (r, f) = (r.clone(), f.clone());
        glib::idle_add_local_once(move || {
            if let (Some(r), Some(f)) = (r.upgrade(), f.upgrade()) {
                f.reiter_schliessen(&r);
            }
        });
    });
    kopf.append(&drehen);
    kopf.append(&titel);
    kopf.append(&zu);
    (kopf, titel, drehen)
}

/// The tabs of a window in a `gtk::Notebook` (order of the list = order of
/// the pages).
pub struct Reiterliste {
    notebook: gtk4::Notebook,
    liste: RefCell<Vec<Rc<Reiter>>>,
    aktiv: RefCell<Option<Rc<Reiter>>>,
}

impl Default for Reiterliste {
    fn default() -> Self {
        Self::new()
    }
}

impl Reiterliste {
    pub fn new() -> Self {
        let notebook = gtk4::Notebook::new();
        notebook.set_show_tabs(false);
        notebook.set_show_border(false);
        notebook.set_scrollable(true);
        notebook.set_hexpand(true);
        notebook.set_vexpand(true);
        // the views take the keyboard; tabs are reached with Strg+Bild↑/↓
        notebook.set_focusable(false);
        notebook.add_css_class("querencia-reiter");
        Self {
            notebook,
            liste: RefCell::default(),
            aktiv: RefCell::default(),
        }
    }

    pub fn widget(&self) -> &gtk4::Notebook {
        &self.notebook
    }

    /// The tab shown.
    pub fn aktiv(&self) -> Rc<Reiter> {
        self.aktiv_wenn().expect("Fenster ohne Reiter")
    }

    pub fn aktiv_wenn(&self) -> Option<Rc<Reiter>> {
        self.aktiv
            .borrow()
            .clone()
            .or_else(|| self.liste.borrow().first().cloned())
    }

    pub fn alle(&self) -> Vec<Rc<Reiter>> {
        self.liste.borrow().clone()
    }

    pub fn anzahl(&self) -> usize {
        self.liste.borrow().len()
    }

    pub fn index(&self, r: &Rc<Reiter>) -> Option<usize> {
        self.liste.borrow().iter().position(|x| Rc::ptr_eq(x, r))
    }

    pub fn aktiv_index(&self) -> Option<usize> {
        self.aktiv_wenn().and_then(|r| self.index(&r))
    }

    pub fn reiter(&self, i: usize) -> Option<Rc<Reiter>> {
        self.liste.borrow().get(i).cloned()
    }

    /// Places `r` at `pos` (the first tab becomes the current one).
    pub fn einfuegen(&self, r: &Rc<Reiter>, pos: usize) {
        let pos = pos.min(self.anzahl());
        self.liste.borrow_mut().insert(pos, r.clone());
        self.notebook
            .set_show_tabs(regeln::leiste_sichtbar(self.anzahl()));
        self.notebook
            .insert_page(&r.seite, Some(&r.kopf), Some(pos as u32));
        // the heads share the bar (nautilus-notebook.c: tab-expand)
        self.notebook.page(&r.seite).set_tab_expand(true);
    }

    /// Shows the tab at `i` (the notebook's `switch-page` tells the window).
    pub fn zeigen(&self, i: usize) {
        if i < self.anzahl() {
            self.notebook.set_current_page(Some(i as u32));
        }
    }

    /// Takes `r` out; the notebook shows the next tab if it was the current
    /// one. False when `r` is not in the list.
    pub fn entfernen(&self, r: &Rc<Reiter>) -> bool {
        let Some(i) = self.index(r) else {
            return false;
        };
        self.liste.borrow_mut().remove(i);
        if self
            .aktiv
            .borrow()
            .as_ref()
            .is_some_and(|a| Rc::ptr_eq(a, r))
        {
            self.aktiv.replace(None);
        }
        self.notebook
            .set_show_tabs(regeln::leiste_sichtbar(self.anzahl()));
        self.notebook.remove_page(Some(i as u32));
        true
    }

    /// Moves `r` to `ziel` (Umschalt+Strg+Bild↑/↓).
    pub fn verschieben(&self, r: &Rc<Reiter>, ziel: usize) {
        let Some(i) = self.index(r) else { return };
        if ziel >= self.anzahl() || ziel == i {
            return;
        }
        {
            let mut l = self.liste.borrow_mut();
            let x = l.remove(i);
            l.insert(ziel, x);
        }
        self.notebook.reorder_child(&r.seite, Some(ziel as u32));
    }

    /// The notebook shows `seite` now: the tab becomes the current one.
    pub(crate) fn aktiv_nach_seite(&self, seite: &gtk4::Widget) -> Option<Rc<Reiter>> {
        let r = self
            .liste
            .borrow()
            .iter()
            .find(|r| r.seite.upcast_ref::<gtk4::Widget>() == seite)
            .cloned()?;
        self.aktiv.replace(Some(r.clone()));
        Some(r)
    }
}

// ----------------------------------------------------------- the actions

/// The `win.` actions of the tabs (called once per window from
/// `Fenster::aktionen_einrichten`).
pub(crate) fn aktionen_einrichten(fenster: &Rc<Fenster>) {
    let aktion = |name: &str, f: fn(&Rc<Fenster>)| {
        let a = gio::SimpleAction::new(name, None);
        let weak = Rc::downgrade(fenster);
        a.connect_activate(move |_, _| {
            if let Some(fenster) = weak.upgrade() {
                f(&fenster);
            }
        });
        fenster.window().add_action(&a);
    };
    aktion("neuer-reiter", |f| {
        let ort = f.ort();
        let a = f.ansicht();
        f.reiter_oeffnen(ort.as_ref(), true, Some((a.art(), a.zoom())));
    });
    aktion("oeffnen-neuer-reiter", |f| {
        let auswahl = f.modell().gewaehlte();
        crate::oeffnen::oeffnen_in_reitern(f, &auswahl);
    });
    aktion("reiter-vorheriger", |f| schritt(f, -1));
    aktion("reiter-naechster", |f| schritt(f, 1));
    aktion("reiter-links", |f| verschieben(f, -1));
    aktion("reiter-rechts", |f| verschieben(f, 1));
    // Alt+1 … Alt+0 and the list in the menu „Reiter“: target = index
    let zeigen = gio::SimpleAction::new("reiter", Some(glib::VariantTy::STRING));
    let weak = Rc::downgrade(fenster);
    zeigen.connect_activate(move |_, wert| {
        let (Some(f), Some(w)) = (weak.upgrade(), wert.and_then(|w| w.get::<String>())) else {
            return;
        };
        let liste = f.reiterliste();
        let i = w
            .chars()
            .next()
            .filter(|_| w.chars().count() == 1)
            .and_then(|z| regeln::index_fuer_ziffer(z, liste.anzahl()));
        if let Some(i) = i {
            liste.zeigen(i);
        }
    });
    fenster.window().add_action(&zeigen);
}

fn schritt(f: &Rc<Fenster>, richtung: isize) {
    let liste = f.reiterliste();
    if let Some(i) = liste
        .aktiv_index()
        .and_then(|a| regeln::nachbar(a, liste.anzahl(), richtung))
    {
        liste.zeigen(i);
    }
}

fn verschieben(f: &Rc<Fenster>, richtung: isize) {
    let liste = f.reiterliste();
    let Some(a) = liste.aktiv_index() else { return };
    if let (Some(ziel), Some(r)) = (
        regeln::nachbar(a, liste.anzahl(), richtung),
        liste.reiter(a),
    ) {
        liste.verschieben(&r, ziel);
        crate::log(&format!(
            "Reiter {} verschoben: {}/{}",
            r.nummer(),
            ziel + 1,
            liste.anzahl()
        ));
        f.reiter_aktionen_pruefen();
    }
}

/// Enables the tab actions after a change of tabs (no wrap-around).
pub(crate) fn aktionen_pruefen(fenster: &Fenster) {
    let liste = fenster.reiterliste();
    let n = liste.anzahl();
    let a = liste.aktiv_index().unwrap_or(0);
    for (name, an) in [
        ("reiter-vorheriger", regeln::nachbar(a, n, -1).is_some()),
        ("reiter-naechster", regeln::nachbar(a, n, 1).is_some()),
        ("reiter-links", regeln::nachbar(a, n, -1).is_some()),
        ("reiter-rechts", regeln::nachbar(a, n, 1).is_some()),
    ] {
        if let Some(x) = fenster
            .window()
            .lookup_action(name)
            .and_downcast::<gio::SimpleAction>()
        {
            x.set_enabled(an);
        }
    }
    let ordner = fenster.modell().gewaehlte().iter().any(Eintrag::ist_ordner);
    if let Some(x) = fenster
        .window()
        .lookup_action("oeffnen-neuer-reiter")
        .and_downcast::<gio::SimpleAction>()
    {
        x.set_enabled(ordner);
    }
}

// --------------------------------------------------------------- the menu

fn aktiv(fenster: &Fenster, name: &str) -> bool {
    fenster
        .window()
        .lookup_action(name)
        .is_some_and(|a| a.is_enabled())
}

/// The menu „Reiter“ (Nautilus 2.30): Vorheriger/Nächster Reiter, Reiter
/// nach links/rechts verschieben, then one entry per tab (radio mark on the
/// current one, Alt+1 … Alt+0).
pub fn menue_fuellen(menue: &gio::Menu, fenster: &Rc<Fenster>) {
    menue.remove_all();
    let punkt = |text: String, name: &str, kuerzel: String| {
        menue::eintrag(
            &text,
            aktiv(fenster, name)
                .then(|| format!("win.{name}"))
                .as_deref(),
            None,
            Some(&kuerzel),
            None,
        )
    };
    // the classic menus show key names as they are: the German ones
    // („Bild↑“) come from the catalog, as „Pos1“ in `menue.rs`
    let wechseln = gio::Menu::new();
    wechseln.append_item(&punkt(
        gettext("_Previous Tab"),
        "reiter-vorheriger",
        // TRANSLATORS: shortcut shown in the Tabs menu, deutsch „<Control>Bild↑“
        pgettext("accel", "<Control>PageUp"),
    ));
    wechseln.append_item(&punkt(
        gettext("_Next Tab"),
        "reiter-naechster",
        // TRANSLATORS: shortcut shown in the Tabs menu, deutsch „<Control>Bild↓“
        pgettext("accel", "<Control>PageDown"),
    ));
    menue.append_section(None, &wechseln);
    let schieben = gio::Menu::new();
    schieben.append_item(&punkt(
        gettext("Move Tab _Left"),
        "reiter-links",
        // TRANSLATORS: shortcut shown in the Tabs menu
        pgettext("accel", "<Shift><Control>PageUp"),
    ));
    schieben.append_item(&punkt(
        gettext("Move Tab _Right"),
        "reiter-rechts",
        // TRANSLATORS: shortcut shown in the Tabs menu
        pgettext("accel", "<Shift><Control>PageDown"),
    ));
    menue.append_section(None, &schieben);
    let liste = gio::Menu::new();
    let reiter = fenster.reiterliste();
    let aktiv_i = reiter.aktiv_index();
    for (i, r) in reiter.alle().iter().enumerate() {
        let Some(ziffer) = regeln::alt_ziffer(i) else {
            break;
        };
        let icon: Option<gio::Icon> =
            (Some(i) == aktiv_i).then(|| gio::ThemedIcon::new("media-record-symbolic").upcast());
        let kuerzel = format!("<Alt>{ziffer}");
        liste.append_item(&menue::eintrag(
            &r.titel().replace('_', "__"),
            Some("win.reiter"),
            Some(&ziffer.to_string()),
            Some(&kuerzel),
            icon.as_ref(),
        ));
    }
    if liste.n_items() > 0 {
        menue.append_section(None, &liste);
    }
}
