//! The application: one process for all windows (GApplication
//! `org.querencia.Dateien`), shared state ([`App`]) and the hooks through
//! which the modules of Welle B2 (views, context menus, rename) and B3
//! (operations, clipboard, drag and drop, progress, conflicts) plug in
//! ([`Register`]). See `docs/dateien.md` §3.
//!
//! The modules register themselves in `main.rs` (`erweiterungen`), so a
//! merge of their branches only adds `mod` lines and one call each.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use gtk4::prelude::*;
use gtk4::{gio, glib};
use querencia_dateiarbeit::auftraege::{Job, JobId, Queue};
use querencia_dateiarbeit::papierkorb::Papierkorb;

use crate::ansicht::{Ansicht, Kontextziel};
use crate::fenster::Fenster;
use crate::konfig::Konfig;
use crate::lesezeichen::Lesezeichen;
use crate::regeln::Ansichtsart;

/// Makes a view of one kind (a new instance per window).
pub type AnsichtFabrik = Rc<dyn Fn() -> Rc<dyn Ansicht>>;
/// Called once for every new window, after it is built (actions, …).
pub type FensterHaken = Rc<dyn Fn(&Rc<Fenster>)>;
/// Called for every view instance a window makes (drag and drop, …).
pub type AnsichtHaken = Rc<dyn Fn(&Rc<Fenster>, &Rc<dyn Ansicht>)>;
/// Shows the context menu of a view at `x`, `y` in `widget`.
pub type KontextHaken = Rc<dyn Fn(&Rc<Fenster>, Kontextziel, &gtk4::Widget, f64, f64)>;
/// Makes `widget` a drop target; the function gives the folder under
/// `x`, `y` (`None`: no target there). Used for path buttons and places.
pub type AblageHaken = Rc<dyn Fn(&Rc<Fenster>, &gtk4::Widget, Zielfinder)>;
/// The folder a drop at `x`, `y` goes to.
pub type Zielfinder = Rc<dyn Fn(f64, f64) -> Option<gio::File>>;
/// Called once at startup (subscribe to the queue, …).
pub type AnwendungsHaken = Rc<dyn Fn(&Rc<App>)>;

/// The hooks of the modules (filled in `main.rs`, `erweiterungen`).
#[derive(Default, Clone)]
pub struct Register {
    pub(crate) ansichten: HashMap<Ansichtsart, AnsichtFabrik>,
    pub(crate) fenster: Vec<FensterHaken>,
    pub(crate) neue_ansicht: Vec<AnsichtHaken>,
    pub(crate) kontextmenue: Option<KontextHaken>,
    pub(crate) ablage: Option<AblageHaken>,
    pub(crate) anwendung: Vec<AnwendungsHaken>,
}

impl Register {
    /// The view for `art` (replaces the fallback list; the symbol view is
    /// exchangeable this way).
    pub fn ansicht(&mut self, art: Ansichtsart, fabrik: impl Fn() -> Rc<dyn Ansicht> + 'static) {
        self.ansichten.insert(art, Rc::new(fabrik));
    }

    /// Called for every new window (add `win.` actions here).
    pub fn fenster(&mut self, haken: impl Fn(&Rc<Fenster>) + 'static) {
        self.fenster.push(Rc::new(haken));
    }

    /// Called for every view instance of every window.
    pub fn neue_ansicht(&mut self, haken: impl Fn(&Rc<Fenster>, &Rc<dyn Ansicht>) + 'static) {
        self.neue_ansicht.push(Rc::new(haken));
    }

    /// Shows the context menus of the views (one module: `kontextmenue.rs`).
    pub fn kontextmenue(
        &mut self,
        haken: impl Fn(&Rc<Fenster>, Kontextziel, &gtk4::Widget, f64, f64) + 'static,
    ) {
        self.kontextmenue = Some(Rc::new(haken));
    }

    /// Makes path buttons and places drop targets (one module: `ziehen.rs`).
    pub fn ablage(&mut self, haken: impl Fn(&Rc<Fenster>, &gtk4::Widget, Zielfinder) + 'static) {
        self.ablage = Some(Rc::new(haken));
    }

    /// Called once at startup.
    pub fn anwendung(&mut self, haken: impl Fn(&Rc<App>) + 'static) {
        self.anwendung.push(Rc::new(haken));
    }
}

/// Shared state of the process.
pub struct App {
    gtk: gtk4::Application,
    register: Register,
    konfig: Rc<Konfig>,
    queue: Queue,
    papierkorb: Papierkorb,
    lesezeichen: Rc<Lesezeichen>,
    laufwerke: gio::VolumeMonitor,
    fenster: RefCell<Vec<Weak<Fenster>>>,
    auftraege: RefCell<HashMap<JobId, glib::WeakRef<gtk4::Window>>>,
}

thread_local! {
    static APP: RefCell<Option<Rc<App>>> = const { RefCell::new(None) };
}

/// The application of this process (after `startup`).
pub fn app() -> Rc<App> {
    APP.with(|a| a.borrow().clone())
        .expect("App vor dem Start benutzt")
}

/// The application, if started (for code that may run in tests).
pub fn app_wenn() -> Option<Rc<App>> {
    APP.with(|a| a.borrow().clone())
}

impl App {
    /// Builds the shared state in `startup` (on the GTK main context).
    pub(crate) fn starten(gtk: &gtk4::Application, register: Register) -> Rc<Self> {
        let app = Rc::new(Self {
            gtk: gtk.clone(),
            register,
            konfig: Konfig::laden(),
            queue: Queue::new(),
            papierkorb: Papierkorb::new(),
            lesezeichen: Lesezeichen::laden(),
            laufwerke: gio::VolumeMonitor::get(),
            fenster: RefCell::default(),
            auftraege: RefCell::default(),
        });
        APP.with(|a| a.replace(Some(app.clone())));
        for h in app.register.anwendung.clone() {
            h(&app);
        }
        app
    }

    pub fn gtk(&self) -> &gtk4::Application {
        &self.gtk
    }

    pub fn register(&self) -> &Register {
        &self.register
    }

    /// The settings (`dateien.ini`).
    pub fn konfig(&self) -> &Rc<Konfig> {
        &self.konfig
    }

    /// The job queue of the process (file operations, `querencia-dateiarbeit`).
    pub fn queue(&self) -> &Queue {
        &self.queue
    }

    /// The trash counter (full/empty icon, "Papierkorb leeren …" enabled).
    pub fn papierkorb(&self) -> &Papierkorb {
        &self.papierkorb
    }

    /// The bookmarks (`~/.config/gtk-3.0/bookmarks`).
    pub fn lesezeichen(&self) -> &Rc<Lesezeichen> {
        &self.lesezeichen
    }

    pub fn laufwerke(&self) -> &gio::VolumeMonitor {
        &self.laufwerke
    }

    /// Starts `job` in the queue; dialogs of the job (progress, conflicts,
    /// errors) belong to `fenster` (see [`Self::fenster_fuer_auftrag`]).
    pub fn auftrag_starten(&self, job: Job, fenster: Option<&gtk4::Window>) -> JobId {
        let id = self.queue.start(job);
        if let Some(f) = fenster {
            self.auftraege.borrow_mut().insert(id, f.downgrade());
        }
        id
    }

    /// The window a job was started from, if it is still open; else any
    /// open window.
    pub fn fenster_fuer_auftrag(&self, id: JobId) -> Option<gtk4::Window> {
        self.auftraege
            .borrow()
            .get(&id)
            .and_then(|w| w.upgrade())
            .or_else(|| self.gtk.active_window())
    }

    /// Forgets the window of a finished job.
    pub fn auftrag_vergessen(&self, id: JobId) {
        self.auftraege.borrow_mut().remove(&id);
    }

    pub(crate) fn fenster_merken(&self, fenster: &Rc<Fenster>) {
        let mut alle = self.fenster.borrow_mut();
        alle.retain(|f| f.strong_count() > 0);
        alle.push(Rc::downgrade(fenster));
    }

    /// All open windows.
    pub fn alle_fenster(&self) -> Vec<Rc<Fenster>> {
        self.fenster
            .borrow()
            .iter()
            .filter_map(Weak::upgrade)
            .collect()
    }

    /// Finds the `Fenster` behind a GTK window (for dialogs that only know
    /// their parent window).
    pub fn fenster_von(&self, window: &gtk4::Window) -> Option<Rc<Fenster>> {
        self.alle_fenster()
            .into_iter()
            .find(|f| f.window().upcast_ref::<gtk4::Window>() == window)
    }

    /// Opens a new window showing `ort` (home folder if `None`).
    pub fn neues_fenster(self: &Rc<Self>, ort: Option<&gio::File>) -> Rc<Fenster> {
        let ort = ort
            .cloned()
            .unwrap_or_else(|| gio::File::for_path(glib::home_dir()));
        let fenster = Fenster::new(self, &ort);
        self.fenster_merken(&fenster);
        for h in self.register.fenster.clone() {
            h(&fenster);
        }
        fenster.window().present();
        fenster
    }
}
