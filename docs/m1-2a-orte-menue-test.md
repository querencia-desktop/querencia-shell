# M1.2a – Orte-Menü

**Datum:** 2026-10-13
**Status:** abgeschlossen. Der Platzhalter „Orte“ im oberen Panel ist
ein Menü mit dem persönlichen Ordner und den XDG-Benutzerordnern; ein
Eintrag öffnet den Ordner mit der Standardanwendung für Verzeichnisse.
**Code:** `crates/classic2-panel/src/places_menu.rs` (neu), `main.rs`.
**Grundlage:** Architektur-Review des Diffs (bestanden, eine Korrektur
am Logtext, siehe unten).
**Belege:** `assets/m1-2a/`, Testwerkzeug in `assets/m1-2a/werkzeug/`.

## Umsetzung

- Einträge: zuerst „Persönlicher Ordner“ (`glib::home_dir()`), dann in
  fester Reihenfolge Schreibtisch, Dokumente, Downloads, Bilder, Musik,
  Videos über `glib::user_special_dir()`. Ein Ordner erscheint nur, wenn
  GLib einen Pfad liefert, der Pfad existiert und nicht `$HOME` selbst ist
  (so markiert xdg-user-dirs einen abgeschalteten Ordner). Ein fehlender
  Ordner wird ausgelassen und einmal geloggt.
- Das Menü ist ein `gio::Menu` am `MenuButton` „Orte“, wie das
  Anwendungsmenü. Jeder Eintrag trägt die Aktion `places.open` mit der
  `file://`-URI als Ziel.
- Öffnen über `gio::AppInfo::launch_default_for_uri` mit dem
  `AppLaunchContext` des GDK-Displays (reicht das Wayland-Aktivierungstoken
  weiter). Erfolg wie Fehler werden geloggt; ein Fehler beendet das Panel
  nie. Wie beim Anwendungsmenü belegt `Ok` nur die Übergabe an GIO, nicht
  den Programmstart.
- Keine neuen Abhängigkeiten, keine eigene Dateiüberwachung.
  `KeyboardMode::None` und die Fokusstrategie sind unverändert.

## Automatische Tests

23 Tests (21 bisherige und 2 neue in `places_menu.rs`):

- `home_first_then_configured_existing_directories`: Reihenfolge,
  Auslassen eines auf `$HOME` gesetzten, eines fehlenden und nicht
  gelieferter Ordner (eigene Lookup-Funktion statt GLib).
- `menu_items_open_the_directory_uri`: Beschriftung, Aktion
  `places.open` und `file:///`-Ziel des Menüeintrags.

Prüfkette nach der Review-Korrektur auf der VM: `cargo fmt --all --check`,
`cargo build`, `cargo test` (23 bestanden), `cargo clippy --all-targets
-- -D warnings` ohne Befund.

## GUI-Abnahme

labwc 0.9.8 auf tty2, Bedienung über das gehärtete QMP-Werkzeug,
Tastaturfokus über Markerbefehle im foot-Fenster `c2-fokus` geprüft.

**Testbaum** (`werkzeug/orte-setup.sh`, alles unter `/tmp/c2orte`):
eigenes `HOME` mit `Dokumente` und `Downloads`, `user-dirs.dirs` mit
Schreibtisch auf `$HOME` (abgeschaltet), Bilder konfiguriert, aber nicht
vorhanden, Musik und Videos nicht gesetzt. Ein Testhandler für
`inode/directory` (`mimeapps.list`) schreibt jede übergebene URI in eine
Markerdatei. Start des Panels mit diesem Baum: `werkzeug/orte-panel.sh`.

| Fall | Ergebnis |
|---|---|
| Menü zeigt genau Persönlicher Ordner, Dokumente, Downloads | ja, `orte-menue-testbaum.png` |
| Bilder wird ausgelassen und geloggt | ja |
| Öffnen von Dokumente, Downloads und Persönlicher Ordner | je eine URI an den Testhandler, zwei Läufe |
| Tastatur nach Schließen des Menüs zurück bei foot | Marker nach jedem Schritt |
| Anwendungsmenü unverändert bedienbar | ja |
| Echtes `HOME` der VM ohne `user-dirs.dirs` | nur Persönlicher Ordner, `orte-menue-vm-home.png` |
| Öffnen ohne Handler für `inode/directory` | Fehlerzeile im Log, Panel läuft weiter |

Ergebnis: 8 von 8 Prüfpunkten bestanden. Die Einzelzuordnung der acht
Punkte ist nicht als Tabelle erhalten; die Tabelle oben gibt die belegten
Fälle wieder (Logs in `panel-logs.txt`, Marker in den Screenshots).

Die GUI-Läufe liefen auf einem Zwischenstand mit älterem Logtext
(„Ortemenü mit N Einträgen“, „… ist konfiguriert, aber … fehlt“). Die
Review-Korrektur betrifft nur Logtext und Doc-Kommentar; danach wurde
die Prüfkette, nicht aber die GUI-Abnahme wiederholt.

## Bekannte Grenzen

- **GLib-Rückfall für den Schreibtisch:** Ohne `user-dirs.dirs` liefert
  `g_get_user_special_dir()` für den Schreibtisch trotzdem
  `$HOME/Desktop`. Fehlt der Ordner, wird er ausgelassen und geloggt; das
  ist auf der VM der Normalfall und kein Konfigurationsfehler.
- **Keine Aktualisierung zur Laufzeit:** Das Menü wird einmal beim Start
  gebaut. Änderungen an `user-dirs.dirs` sowie angelegte oder gelöschte
  Ordner erscheinen erst nach einem Neustart des Panels.
- Auf der VM ist kein Dateimanager und kein Handler für `inode/directory`
  installiert; Öffnen scheitert dort mit Logzeile (Paketfrage, offen).
- Kein Papierkorb (`trash:///`), keine Datenträger, Lesezeichen oder
  Netzwerkorte; das folgt in eigenen Schritten.
