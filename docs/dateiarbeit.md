# Querencia Dateiarbeit (`crates/querencia-dateiarbeit`)

**Stand:** 0.1.1, 23.10.2026 (Welle A von „Querencia Dateien“, QD-140, Schritt 2).
Reines Bibliotheks-Crate ohne GTK, gemeinsam für Desktop 0.3.0 und den
Dateimanager „Querencia Dateien“. Nichts ist aktiviert; der Desktop nutzt
das Crate noch nicht (das macht Desktop 0.3.0, siehe
 §6). Grundlage: Machbarkeitsvorlage
, Recherchen A (§2.9 API-Skizze) und C
(§3 Dateioperationen, §4 Laufwerke, §10.2 Prüfrahmen).

## 1. Abhängigkeiten und Übersetzung

- `gio`/`glib` 0.22 mit `v2_66` (dieselben Versionen und Merkmale, die gtk4
  0.11 mitbringt; nichts Neues im `Cargo.lock`), `gio-unix` 0.22
  (`DesktopAppInfo`), `gettext-rs` 0.8 (`gettext-system`), `libc` 0.2 (nur
  `renameat2`).
- Textdomäne `querencia-dateiarbeit`, msgid englisch, Deutsch in
  `po/de.po`. Programme binden die Domäne mit
  `querencia_dateiarbeit::i18n::bind(<ihr locale-Verzeichnis>)`.
  Pflege wie bei den Panel-Bibliotheken: im Crate-Ordner
  `python3 -I werkzeug/i18n.py pot | update | pruefen | mo <ziel>`.
- Log: `querencia_dateiarbeit::log_line` / Makro `say!` (eine Zeile, ein
  `write`, wie `querencia_netzwerk`).

## 2. Module und Schnittstelle

| Modul | Inhalt | Herkunft |
|---|---|---|
| `uri` | `uri_list`, `parse_uri_list`, `gnome_copied_files`, `parse_gnome_copied_files`, `parse_kde_cut_selection`, `URI_LIST`, `GNOME_COPIED_FILES`, `KDE_CUT_SELECTION`, `of_path`, `name_of` | Desktop `fileops.rs` |
| `namen` | `split_extension`, `unique_name`, `copy_name`, `link_name`, `build_name`/`is_build_name`, `check_file_name`, `check_label`, `Problem`, `selected_chars`, `escape_value`, `name_key`, `with_name` | Desktop `icons.rs`, `fileops.rs`, `rename.rs` |
| `typ` | `type_from_name`, `is_empty_document`, `icon_for_info`, `icon_for` (Starter mit eigenem Symbol), `trash_icon`, `tally`, `size_text` | Desktop `icons.rs`, `desktop_view.rs` `icon_for`, `fileops.rs` |
| `laufwerke` | `Facts`, `classify`, `classify_unmounted`, `picture`, `removal`, `failure`, `mount_failure`, `users_of`, `busy_question`, `facts`, `volume_facts`, `find_mount`, `volume_icon` (vier eigene SVGs), **neu** `alle`, `Laufwerk`, `Quelle`, `Rueckfrage`, `OhneRueckfrage`, `einhaengen`, `entfernen` | Desktop `volumes.rs` |
| `oeffnen` | `FILE_MANAGERS` (`org.querencia.Dateien`, `org.querencia.Files`, `io.github.lgse.Strata`), `file_manager`, `first_found`, `OpenPlan`, `plan_open`, `OPEN_ATTRIBUTES`, `apps_for`, `is_trusted`, `mark_trusted` | Desktop `desktop_view.rs` `open*`, `is_trusted` |
| `papierkorb` | `Papierkorb` (Zähler mit Signal, Symbol), `count_now`, `contents`, `orig_path`, `home_trash`, `gvfs_trash`, `forget_info`; `wiederherstellen` (MIT), `mounts` (MIT) | Desktop `desktop_view.rs` Papierkorb; Strata |
| `ueberwachung` | `FolderWatch`, `Aenderung`, `waiting_point`, `concerns`, `BUNDLE`, `MAX_CHANGES` | Desktop `desktop_view.rs` `watch`, `icons.rs` |
| `auftraege` | `Queue`, `Job`, `JobId`, `Op`, `Event`, `Conflict`, `Answer`, `Answered`, `Reply`, `FileFacts`, `Outcome`, `failure_text`, `Steuerung`, `ausfuehren`, `PROGRESS_EVERY` | neu (Desktop `fileops.rs` `run` als Ausgang) |
| `netz` (MIT) | `merge_network_entries`, `device_key`, `NetzEintrag`, Host-Key: `host_key_probe_argv`, `host_key_from_ssh_log`, `probe_host_key`, `command_output`, `gvfs_question_parts`, `shown_fingerprint`, `known_hosts_target`, `known_hosts_fingerprints`, `compare_known_hosts` | Querencia Files QF-0o, QF-0o-2 |

Abweichungen von der Skizze A §2.9: `Answer::Rename(String)` statt eines
Copy-Typs (leer = freier Name, `Conflict` bringt dafür `vorschlag` mit);
`Answer::Delete` und `Answer::Retry` für „kein Papierkorb“ und
Fehlerfragen; `Op::NewFile { name, template }` mit Namen; `Job.sources` und
`Job.target` als URI-Text (die Ereignisse enthalten keine GIO-Objekte);
`Queue::subscribe` liefert ein eigenes `Abo` statt einer
`SignalHandlerId` (kein GObject nötig). `laufwerke::Laufwerk` hat zusätzlich
`picture` und `quelle`. `oeffnen::OpenPlan` hat zusätzlich
`Executable { text }` (ausführbare Programme und Skripte, Rückfrage wie
Nautilus 2.30) und `Folder`/`Location` mit `Option<AppInfo>`.

## 3. Regeln der Dateioperationen

1. **Nie überschreiben ohne Antwort.** Ein belegter Name ergibt eine Frage
   (`Event::Ask`): Datei → `Conflict::Exists` (Ersetzen, Überspringen,
   Umbenennen, Abbrechen), Ordner auf Ordner → `Conflict::FolderMerge`
   (zusätzlich Zusammenführen). „Für alle“ gilt je Art der Frage. Der
   Vorgabeknopf im Programm ist „Überspringen“ (E2).
2. **Erst fertig, dann an den Platz.** Jedes neue Objekt entsteht unter
   einem versteckten Baunamen im Zielordner
   (`.<name>.querencia-teil-<pid>-<auftrag>-<nr>`) und bekommt seinen Namen
   in einem Schritt, der nie ersetzt: lokal `renameat2(RENAME_NOREPLACE)`,
   auf GVfs GIOs `move` ohne `OVERWRITE` und ohne Kopier-Rückfall. Ein
   Abbruch oder Fehler entfernt das Teilobjekt; scheitert auch das, sagt der
   Bauname, was es ist (`namen::is_build_name`). Ein halbes Objekt trägt nie
   den echten Namen.
3. **Ersetzen atomar.** Das neue Objekt wird ganz gebaut, dann mit
   `RENAME_EXCHANGE` gegen das alte getauscht; erst danach wird das alte
   gelöscht. Ohne `RENAME_EXCHANGE` (manche FUSE/NFS) und auf GVfs: altes
   Objekt auf einen Ersatznamen (`NOREPLACE`), neues an den Platz, bei
   Fehler das alte zurück. Ein Ordner wird nie durch etwas ersetzt, das er
   enthält.
4. **Verschieben über Dateisystemgrenzen** = vollständig kopieren (Regel 2),
   dann das Original löschen. Ein Abbruch während der Kopie lässt das
   Original unberührt; das Löschen des Originals nach fertiger Kopie läuft
   ohne Abbruch zu Ende. Innerhalb eines Dateisystems ist Verschieben ein
   `RENAME_NOREPLACE`; „Ersetzen“ tauscht dann das Objekt selbst ein.
5. **Verweise werden nie verfolgt** (`NOFOLLOW_SYMLINKS` beim Zählen,
   Kopieren, Löschen; ein Verweis wird als Verweis kopiert). Ein Ordner geht
   nie in sich selbst; Kopieren in den eigenen Ordner ergibt eine Kopie mit
   freiem Namen („notiz (Kopie).txt“), Verschieben in den eigenen Ordner
   ändert nichts.
6. **Fehler einzelner Dateien** in einem Ordner werden gemeldet
   (`Event::Failed`, Name relativ zum Objekt) und die übrigen kopiert; mit
   `Job::fehler_fragen` wird gefragt (`Conflict::Fehler`: Wiederholen,
   Überspringen, Abbrechen). **Ziel voll, schreibgeschützt oder
   ausgehängt** lässt das ganze Objekt scheitern; sein Teilobjekt wird
   entfernt.
7. **Papierkorb:** fehlt er am Ort (`NOT_SUPPORTED`), fragt der Auftrag
   (`Conflict::NoTrash`: Löschen, Überspringen, Abbrechen; Vorgabe im
   Programm Abbrechen). **Wiederherstellen ersetzt nie**: ein belegter Ort
   fragt `Conflict::Exists`, sinnvoll sind nur Umbenennen, Überspringen,
   Abbrechen; „Ersetzen“ wird als Überspringen behandelt.
8. **Löschen** und **Papierkorb leeren** brechen zwischen zwei Objekten ab;
   was schon gelöscht ist, bleibt gelöscht (wie Nautilus). Einträge aus
   einem Papierkorb-`files`-Ordner nehmen ihre `.trashinfo` mit; in
   `trash:///` löscht GVfs nur oberste Einträge (mit Inhalt).
9. **Rückgängig** gibt es nicht (E8: erst nach der Beta).

## 4. Warteschlange und Ereignisse

- `Queue::new()` auf dem Hauptkontext des Programms (thread-default); je
  Auftrag ein eigener Faden `dateiarbeit-<id>` (nie GLibs Thread-Pool, den
  GTK für Symbole nutzt, QD-106). Ereignisse kommen über einen eigenen
  Kanal mit Waker im Hauptkontext an (`glib::MainContext::channel` gibt es
  in glib 0.22 nicht mehr).
- Reihenfolge je Auftrag: `Started` (nach dem Vorlauf: Objekte und Bytes;
  beim Verschieben innerhalb eines Dateisystems nur die Zahl der Quellen,
  Bytes `None`), `Progress` (höchstens alle `Job::fortschritt`, Vorgabe
  100 ms, und nach jeder gewählten Quelle), `Ask`, `Failed`, `Finished`.
- `Event::Ask` trägt einen `Reply`; `reply.send(Answered)` aus jedem Faden.
  Der Auftrag wartet; `Queue::cancel(id)` beendet auch eine offene Frage
  (gilt als „Abbrechen“). Werden alle `Reply` ohne Antwort verworfen, gilt
  ebenfalls „Abbrechen“. Das Fortschrittsfenster soll laut C §3 erst nach
  etwa 1 s erscheinen; das entscheidet das Programm.
- `ausfuehren(id, &job, &cancellable, &mut dyn Steuerung)` ist der Kern
  ohne Warteschlange (Tests, Kommandozeile, später ein D-Bus-Dienst nach
  Variante B).

## 5. Papierkorb, Laufwerke, Überwachung, Öffnen

- `Papierkorb::new()` überwacht `trash:///` (GVfs) bzw. ohne GVfs
  `$XDG_DATA_HOME/Trash/files` und zählt 120 ms nach der letzten Änderung
  asynchron; `connect_changed` meldet nur echte Änderungen. Keine Abfrage
  im Hauptfaden.
- `laufwerke::alle(&monitor)`: eingehängte Laufwerke mit Typ und
  nicht eingehängte Volumes, die GIO einhängen kann.
  `einhaengen`/`entfernen` sind `async`; die Fragen (Passwort, „wird noch
  benutzt“) kommen aus der `MountOperation` des Programms
  (`Rueckfrage`, im GUI-Crate `gtk::MountOperation` mit der deutschen
  Belegt-Frage aus `busy_question`). **Nur aus dem GTK-Hauptkontext
  aufrufen** (Falle 2).
- `FolderWatch::new(&dir, warten)`: Änderungen gebündelt (120 ms,
  Umbenennen als ein Ereignis, über 4096 auf einmal `NeuLaden`), mit
  `warten` wird ein fehlender Ordner im nächsten vorhandenen Elternordner
  erwartet (`OrdnerDa`), ein verschwundener gemeldet (`OrdnerWeg`) und
  wieder erwartet (QD-112).
- `oeffnen::plan_open(&file, &info)` entscheidet ohne Dialog; Dialoge und
  Start (mit dem `AppLaunchContext` des Displays) bleiben beim Programm.

## 6. Fallen (gemessen am 23.10.2026)

1. `glib::timeout_add_local*` hängt an den **globalen** Standardkontext,
   nicht an den thread-default-Kontext. Entprellung deshalb über
   `glib::timeout_future` in `spawn_local` auf dem thread-default-Kontext;
   so läuft sie im GTK-Hauptkontext und in Tests mit eigenem Kontext.
2. `GUnixVolume::mount` (unix-Volume-Monitor) liest die Ausgabe von
   `mount(8)` über den **globalen** Standardkontext: auf einem eigenen
   Kontext wird das Einhängen nie fertig (Test: 20 s Zeitüberschreitung,
   mit dem Standardkontext 0,6 s). Programme mit GTK haben ihn ohnehin.
3. GIO übergeht `bind`-Zeilen einer fstab (`LIBMOUNT_FSTAB`): sie ergeben
   kein Volume. Geräte- und tmpfs-Zeilen mit `x-gvfs-show` schon (Hülle
   `werkzeug/namensraum.sh`).
4. `g_file_copy` meldet auf **demselben** lokalen Dateisystem den
   Fortschritt einer Datei nur einmal am Ende (copy_file_range); ein
   Abbruch wirkt dort erst nach der Datei (48 MiB: 2 Ereignisse). Über die
   Dateisystemgrenze kommen Zwischenstände (32 MiB: 35 Ereignisse).
5. GLib liest `HOME` und `XDG_DATA_HOME` einmal je Prozess. Papierkorb-Tests
   laufen deshalb als eigener Kindprozess mit umgelenkten Ordnern und
   `GIO_USE_VFS=local`; der Test prüft vor dem ersten Wegwerfen, dass
   `user_data_dir` im Testordner liegt.
6. Im Benutzer-Namensraum ist die eigene UID 0, in der Prüfrahmen-Hülle
   1001: der Papierkorb eines Laufwerks heißt `.Trash-<uid>`. Eine Datei
   dieses Namens verhindert ihn (Prüffall „kein Papierkorb“).
7. Ein voller Zielordner mitten in einem Ordnerbaum war zuerst ein
   Einzelfehler und hinterließ einen halben Ordner unter echtem Namen; jetzt
   lässt `NoSpace`/`ReadOnly`/`NotMounted` das ganze Objekt scheitern
   (Regel 6, Test `namensraum` „voll“).

## 7. Prüfung

- `cargo test -p querencia-dateiarbeit`: Unit-Tests je Modul und
  `tests/auftraege.rs` (Tabellentests auf Wegwerfbäumen unter
  `target/tmp`), `tests/papierkorb.rs` (Kindprozess mit eigenem HOME),
  `tests/ueberwachung.rs`, `tests/namensraum.rs` (Kindprozess in
  `werkzeug/namensraum.sh`: `unshare -Urm`, tmpfs als Dateisystemgrenze,
  volles Ziel, Ort ohne Papierkorb, Laufwerks-Attrappen über
  `LIBMOUNT_FSTAB` und `GIO_USE_VOLUME_MONITOR=unix`). Ohne
  Benutzer-Namensräume schlägt der Test fehl, außer
  `QUERENCIA_DATEIARBEIT_OHNE_NAMENSRAUM=1` (dann „NICHT GEMESSEN“).
- Prüfrahmen (main, `werkzeug/dateien-pruefrahmen/`):
  `bash werkzeug/dateien-pruefrahmen/beispiel-cargo-test.sh <worktree> L|G`
  baut außen und startet jedes Testprogramm in der Hülle. Dort gibt es kein
  eigenes `unshare`; der Namensraum-Test nimmt dann die `--tmpfs`-Ordner der
  Hülle (`huelle.sh --tmpfs grenze:64M --tmpfs voll:1M --tmpfs
  ohne-papierkorb:4M`), ohne sie meldet er „NICHT GEMESSEN“. Die
  Laufwerks-Attrappen prüft nur `werkzeug/namensraum.sh`.
- Die Kindprozesse laufen mit leerer Umgebung (nur HOME, XDG-Ordner, PATH,
  `LANG=C.UTF-8`, `GIO_USE_VFS=local`, `GIO_USE_VOLUME_MONITOR=unix`,
  `GSETTINGS_BACKEND=memory`, `DBUS_SESSION_BUS_ADDRESS=disabled:`): kein
  gvfsd, kein wsdd, kein Sitzungsbus, kein echter Papierkorb.
- Zahlen und Messwerte: .

## 8. Lizenz der übernommenen Module (E1, E9)

Das Crate steht unter GPL-3.0-or-later. Drei Dateien enthalten Code aus dem
Strata-Fork „Querencia Files“ (MIT), jede mit SPDX-Kopf und Herkunft:

| Datei | Quelle (Zweig `querencia`, 3dfdb25f) | Urheber |
|---|---|---|
| `src/papierkorb/wiederherstellen.rs` | `src/adapters/trash_restore.rs`, `docs/trash-restore-testing.md` | LGSE Ltd. (Strata), Querencia Desktop contributors |
| `src/papierkorb/mounts.rs` | `src/adapters/volume/mounts.rs` | LGSE Ltd. (Strata), Querencia Desktop contributors |
| `src/netz.rs` | `src/adapters/local_files/network_devices.rs`, Host-Key-Teil von `src/ui/browser/network.rs` | Querencia Desktop contributors |

Der MIT-Text steht in `crates/querencia-dateiarbeit/LICENSES/MIT-Strata.txt`.
Offen für das Packaging (E9): Eintrag in `debian/copyright` und der Satz
„Enthält Code aus Strata, MIT“ im Info-Dialog der Programme, die das Crate
nutzen.

## 9. Grenzen (nicht gemessen)

- GVfs-Orte (Stufe G): Kopieren/Verschieben/Ersetzen auf SMB/SFTP,
  `trash:///` mit gvfsd (Zählen, Leeren, Wiederherstellen über
  `target-uri`), Passwort- und Belegt-Fragen einer echten `MountOperation`.
  Braucht den Prüfrahmen mit privatem System-Bus (C §10.2).
- Echte Wechseldatenträger und udisks (Stufe H).
- FAT/exFAT-Ziele (Rückfall ohne `ALL_METADATA` ist eingebaut, aber ohne
  fuse2fs nicht geprüft).
- Nicht Teil von 0.1.0: Fortschrittsdienst per D-Bus (Variante B),
  Rückgängig (E8), Miniaturen, Lesezeichen und Orte-Modell (A §4),
  Zwischenablage-Inhalte als GTK-Objekte (gehören ins GUI-Crate).
