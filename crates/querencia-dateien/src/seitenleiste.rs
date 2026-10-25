//! The side pane "Orte" (B §3.5): (1) Persönlicher Ordner, Arbeitsfläche,
//! Dateisystem, Netzwerk, Papierkorb with the icons of the panel's "Orte"
//! menu (the trash full or empty); (2) volumes and connected servers from
//! `querencia_dateiarbeit::laufwerke::alle`, a click mounts, ⏏ unmounts or
//! ejects (with the desktop's German "still in use" question); (3) the
//! bookmarks. Right click: Öffnen, In neuem Fenster öffnen, Lesezeichen
//! entfernen, Einhängen/Aushängen/Auswerfen, Papierkorb leeren …
//! The tree (Baum) is Beta; the drop-down has only "Orte" until then.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};

use gtk4::prelude::*;
use gtk4::{gdk, gio, glib};
use querencia_dateiarbeit::laufwerke::{self, Laufwerk, Quelle, Removal, Rueckfrage};
use querencia_oberflaeche::{classic_menu, symbole};

use crate::fenster::Fenster;
use crate::i18n::gettext;
use crate::menue::eintrag;

/// One row of the pane.
#[derive(Clone)]
enum Platz {
    Ort {
        datei: gio::File,
        name: String,
        icon: gio::Icon,
    },
    Laufwerk(Laufwerk),
    Lesezeichen {
        datei: gio::File,
        name: String,
        uri: String,
    },
}

impl Platz {
    fn gruppe(&self) -> u8 {
        match self {
            Platz::Ort { .. } => 0,
            Platz::Laufwerk(_) => 1,
            Platz::Lesezeichen { .. } => 2,
        }
    }

    /// The folder the row opens (`None`: a volume that is not mounted).
    fn datei(&self) -> Option<gio::File> {
        match self {
            Platz::Ort { datei, .. } | Platz::Lesezeichen { datei, .. } => Some(datei.clone()),
            Platz::Laufwerk(l) => l.uri.as_deref().map(gio::File::for_uri),
        }
    }

    fn name(&self) -> String {
        match self {
            Platz::Ort { name, .. } | Platz::Lesezeichen { name, .. } => name.clone(),
            Platz::Laufwerk(l) => l.name.clone(),
        }
    }
}

/// The fixed places in the panel's order.
fn feste_orte(papierkorb_symbol: gio::Icon) -> Vec<Platz> {
    let home = glib::home_dir();
    let mut orte = vec![Platz::Ort {
        datei: gio::File::for_path(&home),
        name: gettext("Home Folder"),
        icon: gio::ThemedIcon::new("user-home").upcast(),
    }];
    if let Some(desk) =
        glib::user_special_dir(glib::UserDirectory::Desktop).filter(|d| *d != home && d.is_dir())
    {
        orte.push(Platz::Ort {
            datei: gio::File::for_path(desk),
            name: gettext("Desktop"),
            icon: gio::ThemedIcon::new("user-desktop").upcast(),
        });
    }
    orte.push(Platz::Ort {
        datei: gio::File::for_path("/"),
        name: gettext("File System"),
        icon: gio::ThemedIcon::new("drive-harddisk").upcast(),
    });
    if crate::orte::schema_verfuegbar("network") {
        orte.push(Platz::Ort {
            datei: gio::File::for_uri("network:///"),
            name: gettext("Network"),
            icon: gio::ThemedIcon::new("network-workgroup").upcast(),
        });
    }
    orte.push(Platz::Ort {
        datei: gio::File::for_uri("trash:///"),
        name: gettext("Trash"),
        icon: papierkorb_symbol,
    });
    orte
}

/// Mount operation with the desktop's German "still in use" question
/// (QE-03); dialogs belong to `window`.
pub struct Fragen {
    window: gtk4::Window,
    removal: Removal,
    name: String,
    wurzel: Option<PathBuf>,
}

impl Rueckfrage for Fragen {
    fn mount_operation(&self) -> Option<gio::MountOperation> {
        let operation = gtk4::MountOperation::new(Some(&self.window));
        let (removal, name, wurzel) = (self.removal, self.name.clone(), self.wurzel.clone());
        let offen: Rc<Cell<bool>> = Rc::default();
        operation.connect_local("show-processes", false, move |values| {
            let operation = values.first()?.get::<gio::MountOperation>().ok()?;
            operation.stop_signal_emission_by_name("show-processes");
            if offen.get() {
                return None;
            }
            let users = wurzel
                .as_deref()
                .map(|w| laufwerke::users_of(Path::new("/proc"), w))
                .unwrap_or_default();
            let (message, detail, buttons) = laufwerke::busy_question(removal, &name, &users);
            let dialog = gtk4::AlertDialog::builder()
                .message(message)
                .detail(detail)
                .buttons(buttons)
                .cancel_button(0)
                .default_button(0)
                .modal(false)
                .build();
            offen.set(true);
            let offen = offen.clone();
            dialog.choose(
                None::<&gtk4::Window>,
                gio::Cancellable::NONE,
                move |antwort| {
                    offen.set(false);
                    if let Ok(1) = antwort {
                        operation.set_choice(0);
                        operation.reply(gio::MountOperationResult::Handled);
                    } else {
                        operation.reply(gio::MountOperationResult::Aborted);
                    }
                },
            );
            None
        });
        Some(operation.upcast())
    }
}

/// Mounts the desktop rules leave out because they have no GVolume, but
/// that a person mounted below /media, /run/media or /mnt (a stick mounted
/// by hand, the unix volume monitor of the test harness): Nautilus 2.30
/// listed them, so does the side pane.
fn eingehaengte_ohne_volume(monitor: &gio::VolumeMonitor, schon: &[Laufwerk]) -> Vec<Laufwerk> {
    monitor
        .mounts()
        .into_iter()
        .filter(|m| m.volume().is_none() && !m.is_shadowed())
        .filter_map(|mount| {
            let pfad = mount.root().path()?;
            let unter = ["/media/", "/run/media/", "/mnt/"]
                .iter()
                .any(|p| pfad.starts_with(p));
            let uri = mount.root().uri().to_string();
            if !unter || schon.iter().any(|l| l.uri.as_deref() == Some(uri.as_str())) {
                return None;
            }
            let facts = laufwerke::facts(&mount);
            let picture = laufwerke::Picture::Removable;
            Some(Laufwerk {
                name: facts.name.clone(),
                uri: Some(uri),
                typ: laufwerke::VolumeType::Removable,
                picture,
                mounted: true,
                removal: laufwerke::removal(&facts),
                icon: laufwerke::volume_icon(picture),
                quelle: Quelle::Mount(mount),
            })
        })
        .collect()
}

pub fn fehler_zeigen(parent: &impl IsA<gtk4::Window>, titel: &str, text: &str) {
    gtk4::AlertDialog::builder()
        .message(titel)
        .detail(text)
        .modal(true)
        .build()
        .show(Some(parent));
}

pub struct Seitenleiste {
    fenster: Weak<Fenster>,
    leiste: gtk4::Box,
    liste: gtk4::ListBox,
    plaetze: RefCell<Vec<Platz>>,
    anker: gtk4::MenuButton,
    menue: gio::Menu,
    /// Row index of the open context menu.
    kontext: Cell<Option<usize>>,
    ort: RefCell<Option<gio::File>>,
}

impl Seitenleiste {
    pub fn new(fenster: &Rc<Fenster>) -> Rc<Self> {
        let leiste = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        leiste.add_css_class("querencia-seitenleiste");
        let kopf = gtk4::Box::new(gtk4::Orientation::Horizontal, 2);
        let art = gtk4::DropDown::from_strings(&[&gettext("Places")]);
        art.set_hexpand(true);
        art.set_can_focus(false);
        let zu = gtk4::Button::from_icon_name("window-close-symbolic");
        zu.set_has_frame(false);
        zu.set_can_focus(false);
        zu.set_tooltip_text(Some(&gettext("Close the side pane")));
        zu.set_action_name(Some("win.seitenleiste"));
        kopf.append(&art);
        kopf.append(&zu);
        kopf.set_margin_start(3);
        kopf.set_margin_end(3);
        kopf.set_margin_top(3);
        kopf.set_margin_bottom(3);
        let liste = gtk4::ListBox::new();
        liste.set_selection_mode(gtk4::SelectionMode::Single);
        liste.add_css_class("navigation-sidebar");
        let scroller = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .child(&liste)
            .vexpand(true)
            .build();
        let anker = gtk4::MenuButton::new();
        anker.set_can_focus(false);
        anker.set_has_frame(false);
        anker.set_opacity(0.0);
        anker.set_size_request(1, 1);
        anker.set_halign(gtk4::Align::Start);
        anker.set_valign(gtk4::Align::Start);
        let menue = gio::Menu::new();
        classic_menu::attach_popup(&anker, menue.upcast_ref());
        let ueber = gtk4::Overlay::new();
        ueber.set_child(Some(&scroller));
        ueber.add_overlay(&anker);
        leiste.append(&kopf);
        leiste.append(&ueber);
        leiste.set_size_request(120, -1);
        let s = Rc::new(Self {
            fenster: Rc::downgrade(fenster),
            leiste,
            liste,
            plaetze: RefCell::default(),
            anker,
            menue,
            kontext: Cell::new(None),
            ort: RefCell::default(),
        });
        s.signale(fenster);
        s.fuellen();
        s
    }

    pub fn widget(&self) -> &gtk4::Box {
        &self.leiste
    }

    fn signale(self: &Rc<Self>, fenster: &Rc<Fenster>) {
        self.liste.set_header_func(|zeile, vorher| {
            let gruppe = |z: &gtk4::ListBoxRow| z.widget_name().to_string();
            let trennen = vorher.is_some_and(|v| gruppe(v) != gruppe(zeile));
            zeile.set_header(
                trennen
                    .then(|| gtk4::Separator::new(gtk4::Orientation::Horizontal))
                    .as_ref(),
            );
        });
        let weak = Rc::downgrade(self);
        self.liste.connect_row_activated(move |_, zeile| {
            if let Some(s) = weak.upgrade() {
                s.oeffnen(zeile.index() as usize, false);
            }
        });
        let klick = gtk4::GestureClick::new();
        klick.set_button(0);
        let weak = Rc::downgrade(self);
        klick.connect_pressed(move |g, _, x, y| {
            let Some(s) = weak.upgrade() else { return };
            let Some(zeile) = s.liste.row_at_y(y as i32) else {
                return;
            };
            match g.current_button() {
                gdk::BUTTON_SECONDARY => {
                    g.set_state(gtk4::EventSequenceState::Claimed);
                    s.kontextmenue(zeile.index() as usize, x, y);
                }
                gdk::BUTTON_MIDDLE => {
                    // Nautilus 2.30: the middle button opens a new tab
                    g.set_state(gtk4::EventSequenceState::Claimed);
                    s.oeffnen_in_reiter(zeile.index() as usize);
                }
                _ => {}
            }
        });
        self.liste.add_controller(klick);
        // context menu actions on the pane
        let gruppe = gio::SimpleActionGroup::new();
        for (name, f) in [
            ("oeffnen", Self::k_oeffnen as fn(&Rc<Self>)),
            ("neues-fenster", Self::k_neues_fenster),
            ("neuer-reiter", Self::k_neuer_reiter),
            ("entfernen", Self::k_entfernen),
            ("einhaengen", Self::k_einhaengen),
            ("aushaengen", Self::k_aushaengen),
            ("auswerfen", Self::k_auswerfen),
        ] {
            let a = gio::SimpleAction::new(name, None);
            let weak = Rc::downgrade(self);
            a.connect_activate(move |_, _| {
                if let Some(s) = weak.upgrade() {
                    f(&s);
                }
            });
            gruppe.add_action(&a);
        }
        self.leiste.insert_action_group("seite", Some(&gruppe));
        // keep current
        if let Some(app) = fenster.app() {
            let mon = app.laufwerke();
            let weak = Rc::downgrade(self);
            let neu = move || {
                if let Some(s) = weak.upgrade() {
                    s.fuellen();
                }
            };
            let n = neu.clone();
            mon.connect_mount_added(move |_, _| n());
            let n = neu.clone();
            mon.connect_mount_removed(move |_, _| n());
            let n = neu.clone();
            mon.connect_mount_changed(move |_, _| n());
            let n = neu.clone();
            mon.connect_volume_added(move |_, _| n());
            let n = neu.clone();
            mon.connect_volume_removed(move |_, _| n());
            let n = neu.clone();
            app.lesezeichen().bei_aenderung(n);
            let n = neu.clone();
            app.papierkorb().connect_changed(move |_| n());
        }
    }

    /// Builds the rows from the places, volumes and bookmarks.
    pub fn fuellen(self: &Rc<Self>) {
        let Some(f) = self.fenster.upgrade() else {
            return;
        };
        let Some(app) = f.app() else { return };
        let mut plaetze = feste_orte(app.papierkorb().icon());
        let mut alle = laufwerke::alle(app.laufwerke());
        alle.extend(eingehaengte_ohne_volume(app.laufwerke(), &alle));
        plaetze.extend(alle.into_iter().map(Platz::Laufwerk));
        plaetze.extend(
            app.lesezeichen()
                .liste()
                .into_iter()
                .map(|l| Platz::Lesezeichen {
                    datei: l.datei(),
                    name: l.anzeige(),
                    uri: l.uri.clone(),
                }),
        );
        while let Some(z) = self.liste.first_child() {
            self.liste.remove(&z);
        }
        let ablage = app.register().ablage.clone();
        for (i, p) in plaetze.iter().enumerate() {
            let zeile = gtk4::ListBoxRow::new();
            zeile.set_widget_name(&p.gruppe().to_string());
            let inhalt = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
            let icon: gio::Icon = match p {
                Platz::Ort { icon, .. } => icon.clone(),
                Platz::Laufwerk(l) => l.icon.clone(),
                Platz::Lesezeichen { datei, .. } => {
                    gio::ThemedIcon::new(crate::orte::symbol(datei)).upcast()
                }
            };
            inhalt.append(&symbole::image(&icon, 16));
            let text = gtk4::Label::new(Some(&p.name()));
            text.set_xalign(0.0);
            text.set_hexpand(true);
            text.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            inhalt.append(&text);
            zeile.set_tooltip_text(Some(&match p.datei() {
                Some(d) => d.parse_name().to_string(),
                None => p.name(),
            }));
            if let Platz::Laufwerk(l) = p
                && l.mounted
                && let Some(how) = l.removal
            {
                // the sign itself: the Querencia icon theme (gnome-human)
                // has no media-eject
                let aus = gtk4::Button::with_label("⏏");
                aus.update_property(&[gtk4::accessible::Property::Label(&how.menu_label())]);
                aus.set_has_frame(false);
                aus.set_can_focus(false);
                aus.set_tooltip_text(Some(&how.menu_label()));
                let weak = Rc::downgrade(self);
                aus.connect_clicked(move |_| {
                    if let Some(s) = weak.upgrade() {
                        s.entfernen_bei(i);
                    }
                });
                inhalt.append(&aus);
            }
            inhalt.set_margin_top(2);
            inhalt.set_margin_bottom(2);
            zeile.set_child(Some(&inhalt));
            self.liste.append(&zeile);
            if let (Some(h), Some(d)) = (&ablage, p.datei()) {
                h(&f, zeile.upcast_ref(), Rc::new(move |_, _| Some(d.clone())));
            }
        }
        self.plaetze.replace(plaetze);
        if let Some(o) = self.ort.borrow().clone() {
            self.markieren(&o);
        }
    }

    /// Marks the row of `ort` (if any).
    pub fn ort_anzeigen(&self, ort: &gio::File) {
        self.ort.replace(Some(ort.clone()));
        self.markieren(ort);
    }

    fn markieren(&self, ort: &gio::File) {
        let i = self
            .plaetze
            .borrow()
            .iter()
            .position(|p| p.datei().is_some_and(|d| d.equal(ort)));
        match i.and_then(|i| self.liste.row_at_index(i as i32)) {
            Some(z) => self.liste.select_row(Some(&z)),
            None => self.liste.unselect_all(),
        }
    }

    fn platz(&self, i: usize) -> Option<Platz> {
        self.plaetze.borrow().get(i).cloned()
    }

    fn oeffnen(self: &Rc<Self>, i: usize, neues_fenster: bool) {
        let (Some(p), Some(f)) = (self.platz(i), self.fenster.upgrade()) else {
            return;
        };
        if let Some(d) = p.datei() {
            if neues_fenster {
                if let Some(app) = f.app() {
                    app.neues_fenster(Some(&d));
                }
            } else {
                f.gehe_zu(&d);
            }
            return;
        }
        if let Platz::Laufwerk(l) = p
            && let Quelle::Volume(volume) = &l.quelle
        {
            self.einhaengen(volume.clone(), l.name.clone(), neues_fenster);
        }
    }

    fn einhaengen(self: &Rc<Self>, volume: gio::Volume, name: String, neues_fenster: bool) {
        let Some(f) = self.fenster.upgrade() else {
            return;
        };
        let fragen = Fragen {
            window: f.window().clone().upcast(),
            removal: Removal::Unmount,
            name: name.clone(),
            wurzel: None,
        };
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let ergebnis = laufwerke::einhaengen(&volume, &fragen).await;
            let Some(s) = weak.upgrade() else { return };
            let Some(f) = s.fenster.upgrade() else { return };
            match ergebnis {
                Ok(mount) => {
                    let wurzel = mount.root();
                    if neues_fenster {
                        if let Some(app) = f.app() {
                            app.neues_fenster(Some(&wurzel));
                        }
                    } else {
                        f.gehe_zu(&wurzel);
                    }
                }
                Err(e) => {
                    if let Some((titel, text)) =
                        laufwerke::mount_failure(e.kind::<gio::IOErrorEnum>(), &name, e.message())
                    {
                        fehler_zeigen(f.window(), &titel, &text);
                    }
                }
            }
        });
    }

    fn entfernen_bei(self: &Rc<Self>, i: usize) {
        let Some(Platz::Laufwerk(l)) = self.platz(i) else {
            return;
        };
        if let Some(how) = l.removal {
            self.entfernen(&l, how);
        }
    }

    fn entfernen(self: &Rc<Self>, l: &Laufwerk, how: Removal) {
        let (Some(f), Quelle::Mount(mount)) = (self.fenster.upgrade(), &l.quelle) else {
            return;
        };
        let wurzel = mount.root();
        // a window showing the volume goes home first (Nautilus 2.30)
        if f.ort()
            .is_some_and(|o| o.has_prefix(&wurzel) || o.equal(&wurzel))
        {
            f.gehe_zu(&gio::File::for_path(glib::home_dir()));
        }
        let fragen = Fragen {
            window: f.window().clone().upcast(),
            removal: how,
            name: l.name.clone(),
            wurzel: wurzel.path(),
        };
        let (mount, name) = (mount.clone(), l.name.clone());
        let fw = Rc::downgrade(&f);
        glib::spawn_future_local(async move {
            if let Err(e) = laufwerke::entfernen(&mount, how, &fragen).await
                && let Some((titel, text)) =
                    laufwerke::failure(e.kind::<gio::IOErrorEnum>(), how, &name, e.message())
                && let Some(f) = fw.upgrade()
            {
                fehler_zeigen(f.window(), &titel, &text);
            }
        });
    }

    fn kontextmenue(self: &Rc<Self>, i: usize, x: f64, y: f64) {
        let Some(p) = self.platz(i) else { return };
        self.kontext.set(Some(i));
        self.menue.remove_all();
        let oben = gio::Menu::new();
        oben.append_item(&eintrag(
            &gettext("_Open"),
            Some("seite.oeffnen"),
            None,
            None,
            None,
        ));
        oben.append_item(&eintrag(
            &gettext("Open in New _Tab"),
            Some("seite.neuer-reiter"),
            None,
            None,
            None,
        ));
        oben.append_item(&eintrag(
            &gettext("Open in New _Window"),
            Some("seite.neues-fenster"),
            None,
            None,
            None,
        ));
        self.menue.append_section(None, &oben);
        let mitte = gio::Menu::new();
        match &p {
            Platz::Lesezeichen { .. } => {
                mitte.append_item(&eintrag(
                    &gettext("_Remove Bookmark"),
                    Some("seite.entfernen"),
                    None,
                    None,
                    Some(&gio::ThemedIcon::new("list-remove").upcast()),
                ));
            }
            Platz::Laufwerk(l) => {
                if !l.mounted {
                    mitte.append_item(&eintrag(
                        &gettext("_Mount"),
                        Some("seite.einhaengen"),
                        None,
                        None,
                        None,
                    ));
                } else {
                    match l.removal {
                        Some(Removal::Eject) => mitte.append_item(&eintrag(
                            &gettext("_Eject"),
                            Some("seite.auswerfen"),
                            None,
                            None,
                            None,
                        )),
                        Some(Removal::Unmount) => mitte.append_item(&eintrag(
                            &gettext("_Unmount"),
                            Some("seite.aushaengen"),
                            None,
                            None,
                            None,
                        )),
                        None => {}
                    }
                }
            }
            Platz::Ort { datei, .. } if datei.uri_scheme().as_deref() == Some("trash") => {
                mitte.append_item(&eintrag(
                    &gettext("_Empty Trash…"),
                    Some("win.papierkorb-leeren"),
                    None,
                    None,
                    None,
                ));
            }
            // Welle C1 (dateien-netzwerk): "Mit Server verbinden …" at
            // "Netzwerk"
            Platz::Ort { datei, .. } if datei.uri_scheme().as_deref() == Some("network") => {
                mitte.append_item(&eintrag(
                    &gettext("Connect to _Server…"),
                    Some("win.mit-server-verbinden"),
                    None,
                    None,
                    Some(&gio::ThemedIcon::new("network-server").upcast()),
                ));
            }
            Platz::Ort { .. } => {}
        }
        if mitte.n_items() > 0 {
            self.menue.append_section(None, &mitte);
        }
        let Some(zeile) = self.liste.row_at_index(i as i32) else {
            return;
        };
        let _ = zeile;
        let rect = gdk::Rectangle::new(x as i32, y as i32, 1, 1);
        if let Some(pop) = self.anker.popover() {
            pop.set_pointing_to(Some(&rect));
        }
        self.anker.popup();
    }

    fn k_platz(&self) -> Option<Platz> {
        self.kontext.get().and_then(|i| self.platz(i))
    }

    fn k_oeffnen(self: &Rc<Self>) {
        if let Some(i) = self.kontext.get() {
            self.oeffnen(i, false);
        }
    }

    fn k_neues_fenster(self: &Rc<Self>) {
        if let Some(i) = self.kontext.get() {
            self.oeffnen(i, true);
        }
    }

    fn k_neuer_reiter(self: &Rc<Self>) {
        if let Some(i) = self.kontext.get() {
            self.oeffnen_in_reiter(i);
        }
    }

    /// A place in a new tab behind the current one; a drive that is not
    /// mounted yet opens in a new window after mounting (as before).
    fn oeffnen_in_reiter(self: &Rc<Self>, i: usize) {
        let (Some(p), Some(f)) = (self.platz(i), self.fenster.upgrade()) else {
            return;
        };
        match p.datei() {
            Some(d) => {
                f.reiter_oeffnen(Some(&d), false, None);
            }
            None => self.oeffnen(i, true),
        }
    }

    fn k_entfernen(self: &Rc<Self>) {
        if let (Some(Platz::Lesezeichen { uri, .. }), Some(f)) =
            (self.k_platz(), self.fenster.upgrade())
            && let Some(app) = f.app()
            && let Err(e) = app.lesezeichen().entfernen(&uri)
        {
            f.melden(&e.to_string());
        }
    }

    fn k_einhaengen(self: &Rc<Self>) {
        if let Some(Platz::Laufwerk(l)) = self.k_platz()
            && let Quelle::Volume(v) = &l.quelle
        {
            self.einhaengen(v.clone(), l.name.clone(), false);
        }
    }

    fn k_aushaengen(self: &Rc<Self>) {
        if let Some(Platz::Laufwerk(l)) = self.k_platz() {
            self.entfernen(&l, Removal::Unmount);
        }
    }

    fn k_auswerfen(self: &Rc<Self>) {
        if let Some(Platz::Laufwerk(l)) = self.k_platz() {
            self.entfernen(&l, Removal::Eject);
        }
    }
}
