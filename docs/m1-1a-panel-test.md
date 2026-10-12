# M1.1a – Testprotokoll klassische Panelstruktur

**Datum:** 2026-10-13
**Umgebung:** Proxmox-Entwicklungs-VM `entwicklungs-vm`, Ubuntu 26.04.1 LTS,
labwc 0.9.3 auf tty2 als Benutzer `user`, Ausgabe `Virtual-1`
1280×800, Software-Rendering, Zeitzone Europe/Berlin, Locale `C.UTF-8`.
**Ergebnis:** Alle acht Abnahmepunkte bestanden.

## Umfang

- **Oberes Panel**, 32 px: links `Anwendungen`, `Orte`, `System` als
  Labels, rechts Uhr mit Datum im Format `TT.MM.JJJJ HH:MM`.
- **Unteres Panel**, 32 px: links Platzhalter `Fensterliste`, rechts
  Platzhalter `Arbeitsflächen`, beide abgeblendet (`dim-label`).
- Ein Prozess, zwei Layer-Shell-Surfaces, gleiche Konfiguration bis auf
  die Kante: Layer `Top`, automatische Exclusive Zone, Namespace
  `classic2-panel`, Tastaturmodus `None`.

## Aufbau

| Datei | Inhalt |
|---|---|
| `src/main.rs` | Start, Layer-Shell-Prüfung, `quit`-Aktion, beide Panels über eine gemeinsame `panel_window`-Funktion |
| `src/clock.rs` | Uhr: Formatierung, Wartezeit bis zur nächsten Minute, Neuplanung |

Die Uhr zeichnet nur einmal pro Minute neu. Nach jeder Aktualisierung
wird ein einmaliger GLib-Timer bis zur nächsten vollen Minute gestellt.
Dadurch gibt es keinen Drift und keinen Sekundentakt. Der Timer hält nur
eine schwache Referenz auf das Label.

## Build-Prüfungen

| Befehl | Ergebnis |
|---|---|
| `cargo fmt --all --check` | ok |
| `cargo check` | ok, keine Warnungen |
| `cargo build` | ok, keine Warnungen |
| `cargo test` | ok, 4 Tests bestanden |
| `cargo clippy --all-targets -- -D warnings` | ok |

Tests in `clock.rs`:

- `full_minute_waits_sixty_seconds`
- `mid_minute_waits_the_rest`
- `end_of_minute_and_leap_second_never_return_zero`
- `formats_day_month_year_and_minutes`

Log: `assets/m1-1a/m1-1a-build.log`.

## Grafische Abnahme

Gemessen in den QEMU-Screendumps an Pixelzeilen bei x=0, x=640 und x=1279.

| # | Prüfung | Ergebnis | Beleg |
|---|---|---|---|
| 1 | Oberes Panel sichtbar | bestanden, Zeilen 0–31, volle Breite | `m1-1a-01-panels.png` |
| 2 | Unteres Panel sichtbar | bestanden, Zeilen 768–799, volle Breite | `m1-1a-01-panels.png` |
| 3 | Uhr lokal korrekt und aktualisiert | bestanden, 01:36 bei VM-Zeit 01:36:43, später 01:38 bei 01:38:03 | `m1-1a-01-panels.png`, `m1-1a-02-uhr-aktualisiert.png` |
| 4 | beide Panels reservieren Platz | bestanden | `m1-1a-03-maximiert.png` |
| 5 | maximiertes Fenster überdeckt kein Panel | bestanden, Fenster exakt in Zeilen 32–767 | `m1-1a-03-maximiert.png` |
| 6 | Beenden gibt beide Bereiche frei | bestanden, Fenster danach Zeilen 0–799 | `m1-1a-04-freigegeben.png` |
| 7 | kein Duplikat bei erneutem Start | bestanden, Zweitstart Exit 0, weiter ein Prozess | `m1-1a-second-launch.log` |
| 8 | labwc läuft nach dem Beenden weiter | bestanden, PID 18722 unverändert | – |

Beenden über `gapplication action org.example.Classic2.Panel quit`, Exit 0
(`m1-1a-panel-run.log`). Beide Panels verschwinden gemeinsam.

## Ressourcen im Leerlauf

| Messung | Wert |
|---|---|
| CPU-Zeit über 59 s inkl. einer Uhraktualisierung | 0 Ticks bei 100 Hz |
| Resident Set Size | ca. 143 MiB |
| Threads | 24 |

Debug-Build, Software-Rendering. Die Speicherwerte sind ein erster
Anhaltspunkt und liegen unter dem Zielbudget von 250 MiB aus dem Plan.

## Bekannte Probleme und Workarounds

- **Datumsformat numerisch.** Auf der VM sind nur die Locales C und
  en_US installiert. Ein lokalisierter Wochentag wie `Di` würde auf
  Englisch erscheinen. Deshalb vorerst das feste Format
  `TT.MM.JJJJ HH:MM`. Lokalisierung kommt mit der Internationalisierung.
- **Zeitzonen- und Suspend-Wechsel.** Die Uhr liest bei jeder
  Aktualisierung die lokale Zeit neu. Nach einem Zeitzonenwechsel oder
  Resume kann die Anzeige bis zu einer Minute veraltet sein. Für M1.1a
  akzeptiert.
- **Gleicher Namespace für beide Panels.** Reicht für labwc. Falls später
  Fensterregeln je Panel nötig sind, Namespaces trennen.
- Weiterhin gültig aus M1.0: EGL-Warnungen durch Software-Rendering,
  kein Unix-Signal-Handler, `gapplication list-actions` braucht eine
  `.desktop`-Datei.

## Dateien

`docs/assets/m1-1a/`:

| Datei | Inhalt |
|---|---|
| `m1-1a-01-panels.png` | beide Panels, Uhr 01:36 |
| `m1-1a-02-uhr-aktualisiert.png` | Uhr nach Minutenwechsel, 01:38 |
| `m1-1a-03-maximiert.png` | maximiertes foot zwischen den Panels |
| `m1-1a-04-freigegeben.png` | maximiertes foot nach dem Beenden |
| `m1-1a-build.log` | fmt, check, build, test, clippy |
| `m1-1a-panel-run.log` | Hauptlauf inkl. Exit-Code |
| `m1-1a-second-launch.log` | Zweitstart, leer = keine Ausgabe |
