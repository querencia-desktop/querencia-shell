# M1.3b – Fensterliste im unteren Panel

**Datum:** 2026-10-13
**Status:** abgeschlossen. Der Platzhalter „Fensterliste“ ist eine
Fensterliste mit einem Button pro Fenster; ein Klick bittet labwc, das
Fenster zu aktivieren.
**Code:** `crates/classic2-panel/src/window_list.rs` (neu), `main.rs`
(Einbindung, Panel-CSS), `Cargo.toml`/`Cargo.lock`.
**Grundlage:** Machbarkeitsprüfung M1.3a (Bericht außerhalb des Repos,
Prototyp hier unter `assets/m1-3b/werkzeug/probe/`), Architektur-Review
des Patches mit drei Korrekturen (siehe unten).
**Belege:** `assets/m1-3b/`, Werkzeug in `assets/m1-3b/werkzeug/`.

## Umsetzung

- **Protokoll und Verbindung:** eigene Wayland-Verbindung (`wayland-client`),
  darauf `zwlr_foreign_toplevel_manager_v1` (v1–3, labwc 0.9.8 bietet v3)
  und `wl_seat`. Fehlt das Protokoll, bleibt der Bereich leer und eine
  Logzeile erklärt warum.
- **Ereignisse im GTK-Hauptkontext:** Der Socket ist eine GLib-Quelle
  (`glib_unix::unix_fd_add_local`). Kein Thread, kein Polling, kein Tokio.
  Pro Weckruf: `prepare_read`, `read`, `dispatch_pending`, `flush`.
- **Zustand:** `title`, `app_id` und `state` schreiben einen vorläufigen
  Stand; erst `done` übernimmt ihn. Die Buttons werden einmal pro
  Ereignisschub abgeglichen.
- **Identität:** Buttons kennen nur einen eigenen fortlaufenden Schlüssel,
  nie das Wayland-Handle. Nach `closed` wird das Handle freigegeben und der
  Eintrag entfernt; ein veralteter Button findet nichts und aktiviert nichts.
  Das gilt auch, wenn labwc die Protokoll-ID wiederverwendet (in M1.3a
  beobachtet).
- **Aktivieren:** Klick → `activate(seat)` → `flush`. Die Hervorhebung folgt
  allein dem gemeldeten Zustand.
- **Darstellung:** Titel bis etwa 24 Zeichen, dann „…“, voller Titel als
  Tooltip; aktives Fenster fett und leicht hinterlegt (eine CSS-Regel),
  minimiertes Fenster als `[Titel]`. Ohne Fenster bleibt der Bereich leer.
  Panelhöhe weiter 32 px.
- **Lebensdauer:** Die Closure der fd-Quelle hält die einzige starke
  Referenz auf den Zustand; der Zustand hält das Container-Widget nur
  schwach. Wird das Widget zerstört, entfernt der `destroy`-Handler die
  Quelle; damit fallen Zustand, Handles und Verbindung weg. Bricht die
  Verbindung ab oder meldet labwc `finished`, endet die Quelle mit `Break`,
  gibt vorher ihre `SourceId` ab (kein zweites `remove()`) und leert die
  Buttons.
- Neue direkte Abhängigkeiten (freigegeben): `wayland-client` 0.31,
  `wayland-protocols-wlr` 0.3 (Feature `client`), `glib-unix` 0.22 (in
  glib 0.22 ausgelagertes `unix_fd_add_local`). `Cargo.lock`: 16 neue Pakete.
  `KeyboardMode::None` und die Fokusstrategie sind unverändert.

## Review-Korrekturen

1. **GLib-Quelle:** Die `SourceId` wurde verworfen, die Closure hielt den
   Zustand und darüber das Widget stark. Jetzt wie oben unter Lebensdauer
   beschrieben.
2. **Kommentar zu `done`:** unterscheidet jetzt die Protokollgarantie (nur
   pro Fenster bis zu dessen `done`) vom beobachteten labwc-Verhalten
   (Fokuswechsel in einem Schub). Endet ein Schub dazwischen, ergänzt der
   nächste den Stand; keine zusätzliche Synchronisation.
3. **`WouldBlock` beim Senden:** als bekannte Grenze im Code dokumentiert
   (siehe unten).

## Automatische Tests

29 Tests (23 bisherige, 6 neue in `window_list.rs`, alle ohne Compositor):

| Test | prüft |
|---|---|
| `windows_appear_after_done_and_disappear_when_closed` | Hinzufügen, erst mit `done` sichtbar, Entfernen |
| `title_and_state_change_only_with_done` | Titel und Zustand erst mit `done`; `done` ohne Unterschied ist keine Änderung |
| `focus_change_across_two_windows_is_one_batch` | zwei `done` verschiedener Fenster ergeben einen konsistenten Stand |
| `two_windows_of_the_same_application_stay_separate` | getrennte Schlüssel und Handles bei gleicher App-ID |
| `stale_key_never_reaches_a_window_with_a_reused_protocol_id` | veralteter Schlüssel, späte Ereignisse, wiederverwendete Protokoll-ID |
| `state_array_and_button_text` | Zustandsfeld, `[Titel]`, Ersatztext ohne Titel |

Prüfkette nach den Review-Korrekturen auf der VM: `cargo fmt --all --check`,
`cargo build --locked`, `cargo test --locked` (29 bestanden), `cargo clippy
--all-targets -- -D warnings` ohne Befund.

## Lebenszyklus der fd-Quelle

`werkzeug/lebenszyklus/` bindet `window_list.rs` unverändert ein, baut die
Liste in einem nie angezeigten Fenster auf und prüft zwei Abläufe in der
laufenden labwc-Sitzung (das reguläre Panel blieb unberührt):

| Ablauf | korrigierter Stand | Review-Stand (Gegenprobe) |
|---|---|---|
| 1. Widget zerstört, Quelle aktiv | Widget freigegeben, Verbindung geschlossen | Widget **nicht** freigegeben, Verbindung **offen** |
| 2. Verbindung bricht ab (`Break`), danach Widget zerstört | Buttons leer, Verbindung zu, Widget freigegeben, keine GLib-Warnung | wie links |

Logs: `lebenszyklus-lauf.txt`, `lebenszyklus-gegenprobe-review-stand.txt`.
Die Zeile `Io error: Broken pipe` stammt vom absichtlich getrennten Socket.

## GUI-Abnahme

labwc 0.9.8 auf tty2. Das reguläre Panel wurde kurz beendet, der Teststand
mit derselben Umgebung gestartet und danach das reguläre Panel wieder
gestartet. Testfenster: foot mit App-ID `c2probe` (`werkzeug/fenster.sh`);
Klicks über das gehärtete `qmp-input.py`. Den Fensterzustand hat jeweils der
Prototyp (`werkzeug/probe`, zweiter Wayland-Client) beim Compositor
gegengeprüft; den Tastaturfokus Markerbefehle, die per QMP in das aktive
Fenster getippt wurden.

| Fall | Ergebnis | Beleg |
|---|---|---|
| 1. Fenster öffnen | Button erscheint, neues Fenster aktiv | `zwei-fenster-gleiche-app.png` |
| 2. zweites Fenster derselben App | eigener Button | `zwei-fenster-gleiche-app.png` |
| 3. Titel ändern | Button zeigt `c2-probe-a2` | `titelwechsel.png` |
| 4. Klick aktiviert | Compositor meldet `c2-probe-a2` aktiv; Marker `ka` im Fenster | `klick-aktiviert.png` |
| 5. minimieren (über den Prototyp), Klick stellt wieder her | `[c2-probe-b]`, danach aktiv und ohne Klammern, bestätigt; Marker `kb` | `minimiert.png`, `wiederhergestellt.png` |
| 6. schließen, neues Fenster | Button weg; `c2-probe-c` mit eigenem Button, per Klick aktiviert; Marker `kc` | `geschlossen-neues-fenster.png` (mit Tooltip) |
| Fokus | Orte-Menü öffnen, Escape, Marker `km` im aktiven Fenster | `fokus-nach-escape.png` |

6 von 6 Fällen bestanden; das Panel hielt nach keinem Klick die Tastatur.
Die GUI-Abnahme lief vor den Review-Korrekturen. Diese ändern nur Lebensdauer
und Kommentare; geprüft wurden sie mit der Prüfkette und dem
Lebenszyklus-Werkzeug, nicht mit einer zweiten GUI-Abnahme.

## Bekannte Grenzen

- **`WouldBlock` beim Senden:** Ist der Socket voll, behält wayland-client
  die Anfragen. Es gibt keine Überwachung auf Schreibbereitschaft; gesendet
  wird erst beim nächsten Ereignis des Compositors oder Klick.
- **Reihenfolge von `done` zwischen Fenstern** ist vom Protokoll nicht
  garantiert; bei labwc 0.9.8 kam ein Fokuswechsel stets in einem Schub.
- Verbindungsabbruch nur mit dem Werkzeug geprüft (getrennter Socket),
  nicht durch einen Compositor-Neustart; `finished` nicht ausgelöst.
- Nicht geprüft: viele Fenster (Platz neben „Arbeitsflächen“),
  XWayland-Fenster, Dialoge (erscheinen als eigene Buttons, `parent` wird
  nicht ausgewertet), labwc 0.9.3.
- Klick auf das aktive Fenster minimiert nicht; keine Icons, Gruppierung,
  Kontextmenüs, Arbeitsflächen- oder Mehrmonitorlogik.
- Jeder Klick schreibt eine Logzeile („Aktivierung von Fenster N angefragt“).
