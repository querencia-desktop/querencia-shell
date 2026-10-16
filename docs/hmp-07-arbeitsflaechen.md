# HMP-07: GNOME-2-Arbeitsflächenumschalter

Stand: 2026-10-16. Baut auf HMP-06 auf. Weiter `ext_workspace_manager_v1`
(labwc 0.9.8: v1) auf der vorhandenen Verbindung von
`workspace_switcher.rs`; Anzahl, Namen und aktive Fläche kommen allein vom
Compositor, nichts ist fest verdrahtet.

## Darstellung

Zellen nebeneinander, ohne Ziffern, als Miniatur des Bildschirms: 22 px hoch,
Breite nach dem Seitenverhältnis des Monitors (16:9 → 39 px; begrenzt auf
quadratisch bis dreifach breit). Farben aus der GNOME-2-Referenz gemessen:

| Teil | Farbe |
|---|---|
| Rahmen | 1 px `#cdc6be`, Ecken 2 px |
| Trennlinie | 1 px `#faf1e8` |
| inaktive Zelle | `#b7ab9f`, beim Überfahren `#c5bbb1` |
| aktive Zelle | 3 px dunkelorange `#a5522d`, 1 px creme `#fff2da`, innen lachs `#feb396` |
| nicht aktivierbar | 50 % Deckkraft |

GTK zeichnet von zwei Inset-Schatten den später genannten oben; der
3-px-Rand steht deshalb hinter der 4-px-Cremelinie (die GUI-Probe fand die
umgekehrte Reihenfolge als Fehler).

## Bedienung

Klick auf eine Zelle: `activate` + `commit`. Mausrad (wie GNOME 2): nach
unten/rechts nächste, nach oben/links vorige Fläche, ohne Umlauf, nicht
aktivierbare werden übersprungen. Tooltip „Arbeitsfläche N“ oder der eigene
Name.

## Grenzen von labwc 0.9.8

- **Keine Fenster in den Zellen:** Weder `zwlr_foreign_toplevel_manager_v1`
  noch `ext_workspace_v1` sagen, auf welcher Arbeitsfläche ein Fenster liegt.
  Aus demselben Grund zeigt die Fensterliste die Fenster aller Flächen
  (GNOME 2 zeigte standardmäßig nur die aktuelle).
- **Anzahl:** kommt aus labwc (`<desktops number="…"/>` in `rc.xml`). Die
  laufende Querencia-Sitzung hat kein `rc.xml` und damit eine Fläche.
  Mehrere Flächen brauchen ein `rc.xml` im Runtime-Konfigurationsordner und
  wirken erst nach `labwc --reconfigure` oder dem nächsten Login; das muss
  mit der vorbereiteten Druck-Taste (QS-0c, eigenes `rc.xml`) zusammen
  eingeführt werden.

## Prüfung

Unit-Tests: Zellgröße, Tooltip, Mausrad (kein Umlauf, Überspringen), CSS.
Isoliert mit 1/2/4/6 Flächen (nur im `rc.xml` des Testprofils) und echtem
Zeiger: `tools/isolierte-gui-pruefung/pruefen-arbeitsflaechen.sh`, Pixel-
auswertung `auswerten-arbeitsflaechen.py`.
