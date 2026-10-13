# M1.4b – Arbeitsflächenumschalter im unteren Panel

**Datum:** 2026-10-14
**Status:** abgeschlossen. Der Platzhalter „Arbeitsflächen“ ist ein
Umschalter mit einem Button pro sichtbarer Arbeitsfläche; ein Klick bittet
labwc, die Arbeitsfläche zu aktivieren.
**Code:** `crates/classic2-panel/src/workspace_switcher.rs` (neu), `main.rs`
(Einbindung, Panel-CSS), `Cargo.toml`/`Cargo.lock`.
**Grundlage:** Machbarkeitsprüfung M1.4a (Bericht außerhalb des Repos,
Prototyp hier unter `assets/m1-4b/werkzeug/wsprobe/`), Code-Review mit zwei
Korrekturen (siehe unten).
**Belege:** `assets/m1-4b/`, Werkzeug in `assets/m1-4b/werkzeug/`.

## Umsetzung

- **Protokoll und Verbindung:** `ext_workspace_manager_v1` v1 über eine
  eigene Wayland-Verbindung, unabhängig von der Fensterliste (Entscheidung
  aus M1.4a: getrennte Module, getrennte Fehler; die etwa 60 Zeilen
  gleicher Schleifenkopplung sind bewusst dupliziert). Der Socket ist wie
  in M1.3b eine GLib-Quelle im GTK-Hauptkontext; kein Thread, kein Polling.
  `window_list.rs` ist unverändert.
- **Zustand:** Ereignisse ändern das Modell; erst `manager.done` erzeugt die
  angezeigte Liste. Das Protokoll macht alle Änderungen bis zu diesem `done`
  zu einer atomaren Aktualisierung; labwc schickt `workspace_enter` vor
  `name` und `state` einer neuen Arbeitsfläche.
- **Anzeige:** Arbeitsflächen der ersten Gruppe, ohne `hidden`. Reihenfolge
  nach `coordinates`, wenn alle sichtbaren welche haben, sonst nach
  Erzeugung (labwc 0.9.8 liefert keine Koordinaten). Beschriftung `1`, `2`,
  …, Name als Tooltip; aktive Arbeitsfläche fett und hinterlegt (dieselbe
  CSS-Regel wie die Fensterbuttons). Bei einer Arbeitsfläche steht ein
  hervorgehobener Button `1`.
- **Identität:** Buttons kennen nur einen eigenen fortlaufenden Schlüssel,
  nie das Wayland-Handle; Protokoll-IDs werden nach `removed`
  wiederverwendet.
- **Aktivieren:** Button nur sensitiv, wenn labwc `activate` meldet. Ein
  Klick prüft zusätzlich den aktuellen Modellstand (siehe Korrekturen) und
  sendet `activate`, `commit`, `flush`. Hervorgehoben wird erst nach
  gemeldetem `active`. Ein Klick auf die aktive Arbeitsfläche bleibt
  ereignislos.
- **Lebensdauer wie in M1.3b:** Quelle an das Widget gebunden
  (`destroy`-Handler), Zustand hält das Widget schwach, `SourceId` wird vor
  `Break` abgegeben. `removed` von Arbeitsflächen und Gruppen →
  `destroy()`. `finished` oder Verbindungsfehler → Zustand verworfen,
  Buttons leer. Keine Wiederverbindung.
- Neue direkte Abhängigkeit (freigegeben): `wayland-protocols` 0.32 mit
  `client` und `staging`. Kein neues Paket; es steckte schon über
  `wayland-protocols-wlr` im Lockfile. `KeyboardMode::None` und die
  Fokusstrategie sind unverändert.

## Review-Korrekturen

1. **Sensitivität:** `update_button()` setzt `set_sensitive(activatable)`.
2. **Aktivierung am aktuellen Modell:** `activation()` prüft außer der
   angezeigten Liste, dass der Eintrag noch `activate` kann, nicht `hidden`
   ist und zur ersten Gruppe gehört. Änderungen vor dem nächsten
   `manager.done` verhindern damit schon eine Anfrage.

## Automatische Tests

33 Tests (29 bisherige, 4 neue in `workspace_switcher.rs`, ohne Compositor):

| Test | prüft |
|---|---|
| `events_count_only_with_manager_done` | Übernahme erst mit `done`, `workspace_enter` vor `name`, `done` ohne Unterschied |
| `switching_between_two_workspaces_shows_one_active` | Wechsel in beide Richtungen ergibt genau eine aktive Fläche |
| `hidden_group_membership_and_order` | `hidden`, ohne Gruppe, zweite Gruppe, Erzeugungsreihenfolge, `coordinates`, Gruppe entfernt |
| `removed_workspace_and_stale_key_activate_nothing` | entfernte Fläche, veralteter Schlüssel, wiederverwendete ID; Fähigkeit, `hidden` und Gruppe vor dem nächsten `done` (Korrektur 2) |

Prüfkette nach den Korrekturen auf der VM: `cargo fmt --all --check`,
`cargo build --locked`, `cargo test --locked` (33 bestanden), `cargo clippy
--all-targets -- -D warnings` ohne Befund.

## GUI-Abnahme

labwc 0.9.8 auf tty2, Teststand im Klon, danach das reguläre Panel wieder
gestartet. Zwei Arbeitsflächen nur vorübergehend mit
`werkzeug/zweite-arbeitsflaeche.sh an|aus` (prüft den Konfigurationspfad,
überschreibt nichts, entfernt nur die eigene markierte Datei, lädt per
`labwc --reconfigure` neu, kein Neustart). Klicks über das gehärtete
`qmp-input.py`; Zustand beim Compositor mit `werkzeug/wsprobe`
gegengeprüft, Tastatur mit Markerbefehlen im Fenster `c2-fokus`.

| Fall | Ergebnis | Beleg |
|---|---|---|
| 1. eine Arbeitsfläche | Button `1`, hervorgehoben | `eine-arbeitsflaeche.png` |
| 2. Neuladen mit zwei | Button `2` erscheint ohne Panel-Neustart | `zwei-arbeitsflaechen.png` |
| 3. Klick `2` | labwc meldet Workspace 2 aktiv, Button folgt | `klick-2-bestaetigt.png` |
| 4. Klick `1` | Rückwechsel bestätigt, Marker kommt in `c2-fokus` an | `klick-1-fokus-zurueck.png` |
| Klick auf aktive `1` | ereignislos beim Beobachter | – |
| 5. Konfiguration entfernt | `workspace_leave`, `removed`, `done`; Button `2` verschwindet | `zweite-entfernt.png` |
| 6. Fensterliste, Orte-Menü | Fensterklick aktiviert, Menü öffnet, Escape, Marker kommt an | `panel-log.txt` |

6 von 6 Fällen bestanden. Die Abnahme lief vor den Review-Korrekturen; diese
sind mit Unit-Tests geprüft. Ein ausgegrauter Button ließ sich nicht zeigen,
weil labwc für jede Arbeitsfläche `activate` meldet.
`ereignisse-wechsel-entfernen.txt` zeigt die Ereignisfolge eines zweiten
Clients für denselben Ablauf (aus der Probe M1.4a).

## Bekannte Grenzen

- **Nur die erste Arbeitsflächengruppe:** Weitere Gruppen werden geloggt,
  nicht angezeigt; keine Mehrmonitorlogik (labwc legt eine globale Gruppe
  an).
- **Fensterliste ohne Arbeitsflächenzuordnung:** Sie zeigt weiter die
  Fenster aller Arbeitsflächen. Weder `zwlr_foreign_toplevel` v3 noch
  `ext-foreign-toplevel-list` ordnen Fenster einer Arbeitsfläche zu; unter
  labwc 0.9.8 ist die Filterung nicht möglich.
- **`WouldBlock` beim Senden:** wie in M1.3b keine Überwachung auf
  Schreibbereitschaft; gesendet wird beim nächsten Ereignis oder Klick.
- **Staging-Protokoll:** `ext-workspace-v1` ist „staging“; Änderungen kämen
  als neue Version, gebunden wird ausdrücklich v1.
- Keine stabile Reihenfolge oder Identität über Compositor-Neustarts
  (labwc liefert keine `id`). `finished` und Verbindungsabbruch nur im Code
  behandelt, nicht ausgelöst.
- Mehr als eine Arbeitsfläche braucht eine `rc.xml` der Classic2-Sitzung
  (Sitzungsintegration). Das Neuladen der labwc-Konfiguration löste im Test
  jeweils einen Neuaufbau des Anwendungsmenüs aus (vermutlich GIO-Meldung
  wegen `~/.config`), harmlos.
- Die Logzeile beim Klick nennt den internen Schlüssel, nicht die
  Button-Nummer.
