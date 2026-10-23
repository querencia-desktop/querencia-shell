//! Opening entries (double click, Enter, Datei → Öffnen, Öffnen mit):
//! folders in the window (or new windows), files through
//! `querencia_dateiarbeit::oeffnen::plan_open` with the default program,
//! started through GIO with the display's launch context (xdg-activation
//! token, no portal: C §8). Executable text files ask "Ausführen /
//! Anzeigen / Abbrechen" as Nautilus 2.30 (setting "Ausführbare
//! Textdateien"), untrusted starters ask before they run.

use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{gio, glib};
use querencia_dateiarbeit::oeffnen::{self as plan, OpenPlan};

use crate::eintrag::Eintrag;
use crate::fenster::Fenster;
use crate::i18n::{gettext, gettext_f};
use crate::konfig::AusfuehrbareTexte;
use crate::seitenleiste::fehler_zeigen;

fn kontext(fenster: &Fenster) -> gtk4::gdk::AppLaunchContext {
    let k = WidgetExt::display(fenster.window()).app_launch_context();
    querencia_oberflaeche::portal::for_children(&k);
    k
}

/// Opens `eintraege` (the selection): one folder in this window (several,
/// or with `neues_fenster`, each in a new window), files with their
/// programs.
pub fn oeffnen(fenster: &Rc<Fenster>, eintraege: &[Eintrag], neues_fenster: bool) {
    let ordner: Vec<&Eintrag> = eintraege.iter().filter(|e| e.ist_ordner()).collect();
    let dateien: Vec<&Eintrag> = eintraege.iter().filter(|e| !e.ist_ordner()).collect();
    if ordner.len() == 1 && !neues_fenster {
        ordner_oeffnen(fenster, ordner[0], false);
    } else {
        for o in &ordner {
            ordner_oeffnen(fenster, o, true);
        }
    }
    for d in dateien {
        datei_oeffnen(fenster, d);
    }
}

fn ordner_oeffnen(fenster: &Rc<Fenster>, e: &Eintrag, neues_fenster: bool) {
    let ziel = e.ziel();
    let mountable = e.daten().info.file_type() == gio::FileType::Mountable
        && !e.daten().info.has_attribute("standard::target-uri");
    if mountable {
        // an entry of "Rechner" that is not mounted yet
        let datei = e.datei().clone();
        let quelle = datei.clone();
        let f = Rc::downgrade(fenster);
        let op = gtk4::MountOperation::new(Some(fenster.window()));
        quelle.mount_mountable(
            gio::MountMountFlags::NONE,
            Some(&op),
            gio::Cancellable::NONE,
            move |ergebnis| {
                let Some(f) = f.upgrade() else { return };
                match ergebnis {
                    Ok(wurzel) => {
                        let wurzel = wurzel
                            .query_info(
                                "standard::target-uri",
                                gio::FileQueryInfoFlags::NONE,
                                gio::Cancellable::NONE,
                            )
                            .ok()
                            .and_then(|i| i.attribute_string("standard::target-uri"))
                            .map(|u| gio::File::for_uri(&u))
                            .unwrap_or(wurzel);
                        hin(&f, &wurzel, neues_fenster);
                    }
                    Err(err) if err.matches(gio::IOErrorEnum::FailedHandled) => {}
                    Err(err) => fehler_zeigen(
                        f.window(),
                        &gettext_f(
                            "“{name}” could not be opened.",
                            &[("name", &datei.parse_name())],
                        ),
                        err.message(),
                    ),
                }
            },
        );
        return;
    }
    hin(fenster, &ziel, neues_fenster);
}

fn hin(fenster: &Rc<Fenster>, ort: &gio::File, neues_fenster: bool) {
    if neues_fenster {
        if let Some(app) = fenster.app() {
            app.neues_fenster(Some(ort));
        }
    } else {
        fenster.gehe_zu(ort);
    }
}

fn datei_oeffnen(fenster: &Rc<Fenster>, e: &Eintrag) {
    let datei = e.ziel();
    let name = e.daten().anzeige.clone();
    let f = Rc::downgrade(fenster);
    glib::spawn_future_local(async move {
        let info = datei
            .query_info_future(
                plan::OPEN_ATTRIBUTES,
                gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
                glib::Priority::DEFAULT,
            )
            .await;
        let Some(f) = f.upgrade() else { return };
        let info = match info {
            Ok(i) => i,
            Err(err) => {
                fehler_zeigen(
                    f.window(),
                    &gettext_f("“{name}” could not be opened.", &[("name", &name)]),
                    err.message(),
                );
                return;
            }
        };
        match plan::plan_open(&datei, &info) {
            OpenPlan::Default(app) | OpenPlan::ByName(_, app) => starten(&f, &app, &datei),
            OpenPlan::NoProgram(typ) => fehler_zeigen(
                f.window(),
                &gettext_f("“{name}” cannot be opened.", &[("name", &name)]),
                &gettext_f(
                    "There is no program installed for “{type}” files.",
                    &[("type", &gio::content_type_get_description(&typ))],
                ),
            ),
            OpenPlan::Folder(_) | OpenPlan::Location(_, _) => f.gehe_zu(&datei),
            OpenPlan::Starter { trusted } => starter(&f, &datei, &name, trusted),
            OpenPlan::Executable { text } => ausfuehrbar(&f, &datei, &name, text),
        }
    });
}

/// Starts `app` with `datei` (URI; GIO gives a FUSE path to programs
/// without URI support).
pub fn starten(fenster: &Fenster, app: &gio::AppInfo, datei: &gio::File) {
    let k = kontext(fenster);
    if let Err(err) = app.launch(std::slice::from_ref(datei), Some(&k)) {
        fehler_zeigen(
            fenster.window(),
            &gettext_f(
                "“{program}” could not be started.",
                &[("program", &app.display_name())],
            ),
            err.message(),
        );
    }
}

/// Datei → Öffnen mit ▸ program `id`: the selected files with it (the
/// default program stays).
pub fn oeffnen_mit(fenster: &Rc<Fenster>, eintraege: &[Eintrag], id: &str) {
    let Some(app) = gio_unix::DesktopAppInfo::new(id) else {
        return;
    };
    let dateien: Vec<gio::File> = eintraege.iter().map(Eintrag::ziel).collect();
    let k = kontext(fenster);
    if let Err(err) = app.launch(&dateien, Some(&k)) {
        fehler_zeigen(
            fenster.window(),
            &gettext_f(
                "“{program}” could not be started.",
                &[("program", &app.display_name())],
            ),
            err.message(),
        );
    }
}

fn starter(fenster: &Rc<Fenster>, datei: &gio::File, name: &str, trusted: bool) {
    let Some(pfad) = datei.path() else { return };
    let los = {
        let f = Rc::downgrade(fenster);
        let pfad = pfad.clone();
        move || {
            let Some(f) = f.upgrade() else { return };
            if let Some(app) = gio_unix::DesktopAppInfo::from_filename(&pfad) {
                let k = kontext(&f);
                if let Err(err) = app.launch(&[], Some(&k)) {
                    fehler_zeigen(
                        f.window(),
                        &gettext_f(
                            "“{program}” could not be started.",
                            &[("program", &app.display_name())],
                        ),
                        err.message(),
                    );
                }
            }
        }
    };
    if trusted {
        los();
        return;
    }
    let dialog = gtk4::AlertDialog::builder()
        .message(gettext("Untrusted application launcher"))
        .detail(gettext_f(
            "The application launcher “{name}” has not been marked as trusted. If you do not know \
             the source of this file, launching it may be unsafe.",
            &[("name", name)],
        ))
        .buttons([gettext("_Cancel"), gettext("_Trust and Launch")])
        .cancel_button(0)
        .default_button(0)
        .modal(true)
        .build();
    let f = Rc::downgrade(fenster);
    dialog.choose(
        Some(fenster.window()),
        gio::Cancellable::NONE,
        move |antwort| {
            if antwort != Ok(1) {
                return;
            }
            if let Err(err) = plan::mark_trusted(&pfad) {
                if let Some(f) = f.upgrade() {
                    fehler_zeigen(
                        f.window(),
                        &gettext("The launcher could not be marked as trusted."),
                        err.message(),
                    );
                }
                return;
            }
            los();
        },
    );
}

/// Runs an executable file in its folder (no terminal).
fn ausfuehren(fenster: &Fenster, datei: &gio::File) {
    let Some(pfad) = datei.path() else { return };
    let launcher = gio::SubprocessLauncher::new(gio::SubprocessFlags::NONE);
    if let Some(dir) = pfad.parent() {
        launcher.set_cwd(dir);
    }
    querencia_oberflaeche::portal::launcher_for_children(&launcher);
    if let Err(err) = launcher.spawn(&[pfad.as_os_str()]) {
        fehler_zeigen(
            fenster.window(),
            &gettext_f(
                "“{name}” could not be run.",
                &[("name", &pfad.display().to_string())],
            ),
            err.message(),
        );
    }
}

/// Shows a text file with the default program for plain text.
fn anzeigen(fenster: &Fenster, datei: &gio::File) {
    match gio::AppInfo::default_for_type("text/plain", false) {
        Some(app) => starten(fenster, &app, datei),
        None => fehler_zeigen(
            fenster.window(),
            &gettext_f(
                "“{name}” cannot be opened.",
                &[("name", &datei.parse_name())],
            ),
            &gettext("There is no program installed for text files."),
        ),
    }
}

fn ausfuehrbar(fenster: &Rc<Fenster>, datei: &gio::File, name: &str, text: bool) {
    let werte = fenster
        .app()
        .map(|a| a.konfig().werte())
        .unwrap_or_default();
    if text {
        match werte.ausfuehrbare_texte {
            AusfuehrbareTexte::Anzeigen => return anzeigen(fenster, datei),
            AusfuehrbareTexte::Ausfuehren => return ausfuehren(fenster, datei),
            AusfuehrbareTexte::Nachfragen => {}
        }
    }
    let (frage, knoepfe): (String, Vec<String>) = if text {
        (
            gettext_f(
                "Do you want to run “{name}”, or display its contents?",
                &[("name", name)],
            ),
            vec![gettext("_Display"), gettext("_Cancel"), gettext("_Run")],
        )
    } else {
        (
            gettext_f("Do you want to run “{name}”?", &[("name", name)]),
            vec![gettext("_Cancel"), gettext("_Run")],
        )
    };
    let dialog = gtk4::AlertDialog::builder()
        .message(frage)
        .detail(gettext_f(
            "“{name}” is an executable file.",
            &[("name", name)],
        ))
        .buttons(knoepfe.iter().map(String::as_str).collect::<Vec<_>>())
        .cancel_button(if text { 1 } else { 0 })
        .default_button(if text { 1 } else { 0 })
        .modal(true)
        .build();
    let f = Rc::downgrade(fenster);
    let datei = datei.clone();
    dialog.choose(
        Some(fenster.window()),
        gio::Cancellable::NONE,
        move |antwort| {
            let Some(f) = f.upgrade() else { return };
            match (text, antwort) {
                (true, Ok(0)) => anzeigen(&f, &datei),
                (true, Ok(2)) | (false, Ok(1)) => ausfuehren(&f, &datei),
                _ => {}
            }
        },
    );
}
