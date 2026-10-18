# HMP-09: Desktop-Symbole und Desktop-Kontextmenü

`querencia-desktop` (HMP-08) bekommt eine zweite Layer-Shell-Fläche, die Symbolebene:

| Ebene | Fläche | Eingaben |
|---|---|---|
| background | Hintergrundbild (HMP-08), ganzer Bildschirm | keine |
| bottom | Symbolebene (HMP-09), transparent, zwischen den Panels (exclusive zone 0) | Zeiger; Tastatur nur bei Bedarf |
| normal | Fenster | |
| top | Panels | |

Die Ebene liegt nur auf dem ersten Bildschirm, wie bei GNOME 2. Mit `QUERENCIA_DESKTOP_ICONS=0` gibt es nur den Hintergrund. Dann erreicht ein Rechtsklick wieder labwcs Wurzelmenü.

## Symbole

- Rechner (`computer:///`) und Papierkorb (`trash:///`, leer/voll) erscheinen nur, wenn Querencia Files (`io.github.lgse.Strata.desktop`) installiert ist.
- Persönlicher Ordner erscheint nur, wenn ein Programm für `inode/directory` da ist.
- Querencia Terminal erscheint nur mit installierter Desktop-Datei.
- Dazu jede Datei und jeder Ordner im XDG-Arbeitsflächenordner (`XDG_DESKTOP_DIR`, ohne `user-dirs.dirs`: `~/Desktop`). Der Persönliche Ordner selbst zählt nie als Arbeitsflächenordner.
- Raster: 100×88 px, Spalten von links, jede von oben gefüllt. Die Beschriftung ist weiß mit Schatten, höchstens zwei Zeilen. Umbrochen wird zwischen Wörtern, ohne Trennstrich.
- Eine leere Datei zeigt eine weiße Seite (`text-x-generic`).
- Reihenfolge:
  - wie die Symbole erschienen sind (`$XDG_STATE_HOME/querencia/desktop-order`), neue hinten;
  - „Nach Namen aufräumen“: Rechner, Persönlicher Ordner, Papierkorb, Programme, dann der Rest nach Namen. Das ändert nur die Anordnung, nie eine Datei.

## Bedienung

- Einzelklick wählt aus, ein Klick ins Leere hebt die Auswahl auf.
- Doppelklick öffnet:
  - Rechner und Papierkorb in Querencia Files;
  - den Persönlichen Ordner und Dateien mit dem Standardprogramm;
  - das Terminal über seine Desktop-Datei.
- Rechtsklick auf ein Symbol zeigt „Öffnen“.
- Rechtsklick ins Leere zeigt das Arbeitsflächenmenü im Stil der Panelmenüs (geteilte Quelle `classic_menu.rs`). Die linke obere Ecke liegt am Zeiger.
  - **Ordner anlegen**: „Neuer Ordner“, „Neuer Ordner 2“ …;
  - **Dokument anlegen ▸** mit „Leere Datei“ und den echten Dateien des XDG-Vorlagenordners;
  - **Nach Namen aufräumen**;
  - **Hintergrund der Arbeitsfläche ändern …**: Dateiauswahl für ein Bild. Es wird in `desktop.ini` geschrieben und sofort angewendet; Modus und Farbe bleiben.
- Nichts wird überschrieben:
  - Der freie Name kommt aus `icons::unique_name`.
  - GIO `make_directory`, `create` und `copy` ohne Überschreib-Flag schlagen bei vorhandenem Namen fehl.
  - Den Arbeitsflächenordner legt erst „Ordner anlegen“ bzw. „Dokument anlegen“ an.
- Bewusst nicht angeboten, weil noch nicht umgesetzt: „Starter anlegen …“, „Anordnung fixieren“ und Ziehen und Ablegen, „Einfügen“. Ebenso Umbenennen, Ausschneiden, Kopieren, „In den Müll verschieben“ und „Eigenschaften“ im Symbolmenü.
- HMP-10 ergänzt Ziehen und Ablegen, gespeicherte Plätze statt der Reihenfolge und Umbenennen: `hmp-10-symbole-verschieben-umbenennen.md`.

## Prüfung

- `tools/isolierte-gui-pruefung/pruefen-desktop.sh <desktop> <panel> <profil>` und `auswerten-desktop.py`: 36 Prüfungen.
- `pruefen-hintergrund.sh` (HMP-08) läuft mit Symbolebene und mit `HMP_DESKTOP_ENV=QUERENCIA_DESKTOP_ICONS=0`.
