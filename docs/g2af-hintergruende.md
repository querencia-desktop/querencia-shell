# G2-AF: eingebaute Hintergründe (QD-058)

Stand 20.10.2026. Vier neue Motive kommen zu „Querencia Blau“ hinzu,
jeweils ohne und mit Q-Logo, jeweils als 16:9 und 4:3: 16 Dateien plus
Blau. Herkunft und Lizenzstand je Bild: `data/hintergruende/HERKUNFT.md`.

## Dateien

`data/hintergruende/`:

| Datei | Inhalt |
|---|---|
| `querencia-blau.png` | Standard seit HMP-08, unverändert |
| `querencia-{abend,blau2,erde,nacht}.png` | 2560×1440 ohne Logo |
| `querencia-{…}-4x3.png` | 1600×1200 ohne Logo, eigener Bildausschnitt |
| `querencia-{…}-logo.png`, `-logo-4x3.png` | dieselben mit aufgeprägtem Q-Logo |
| `erzeugen.py` | erzeugt die 16 Dateien aus pottrauschens Rohordner (Prüfsummen der Quellen im Skript) |
| `logo/querencia-logo.svg` | Logo-Vorlage (Kopie aus querencia-systemmonitor) |
| `SHA256SUMS` | alle 17 Bilder |
| `HERKUNFT.md` | Erzeuger, Datum, Prompts, Bearbeitung, Lizenzstand |

Erzeugen und prüfen (braucht nur python3-gi mit GdkPixbuf und dem
SVG-Lader):

```bash
python3 -I data/hintergruende/erzeugen.py --pruefen
```

Die Ausgabe ist bit-gleich reproduzierbar (zwei Läufe verglichen).

## Logo

Die Form stammt aus der SVG-Vorlage, nicht aus den KI-Bildern. Breite 1/8
der Bildhöhe (180 px bzw. 150 px), Abstand rechts 5,2 % und unten 6,9 % der
Bildhöhe, gemessen am Bereich, der bei „Bildschirm ausfüllen“ auf jedem
vorgesehenen Seitenverhältnis sichtbar bleibt:

- 16:9-Datei: 16:10 schneidet links und rechts je 128 px ab; das Logo liegt
  deshalb 203 px vom rechten Rand.
- 4:3-Datei: 5:4 schneidet je 50 px seitlich, 3:2 je 67 px oben und unten ab.

Prägung in drei Schichten: Schatten (schwarz, 20 %) 2 px nach unten rechts,
Licht (weiß, 7 %) 2 px nach oben links, Fläche in `#F07746` mit 10 %.

## Größe

16 neue PNG zusammen 20,5 MB; das gepackte Repo wächst von 15 MB auf rund
36 MB und bleibt unter der Grenze von 40 MB. Die Fassungen ohne Logo sind
die gelieferten Dateien (byte-gleich, außer Abend 16:9), die Logo-Fassungen
mit GdkPixbuf (Kompression 9) neu geschrieben. JPEG war nicht nötig.

## Wahl der Fassung

Der Desktop ist unverändert: er zeigt die Datei, die `desktop.ini` in
`File=` nennt. Querencia Einstellungen zeigt die Bilder als Galerie und
schreibt beim Übernehmen die Fassung, die zum Bildschirm passt
(Seitenverhältnis unter 1,555: 4:3, sonst 16:9). „Querencia Blau“ bleibt der
Standard (`File=` leer). Ein späterer Wechsel des Bildschirms ändert die
gespeicherte Fassung nicht; dafür das Bild in den Einstellungen neu wählen.

## Installation

```bash
bash integration/hintergruende/hintergruende-integrieren.sh pruefen
```

`aktivieren`/`zurueck` nur mit `G2AF_FREIGABE=ja` nach Freigabe der
Status-Session. Ziel `~/.local/share/querencia/hintergruende/`; fremde
Dateien werden nie ersetzt, `zurueck` entfernt nur die selbst angelegten
Dateien (Liste `~/.local/state/querencia/hintergruende-g2af.liste`) und
verweigert, solange `desktop.ini` eines davon zeigt. Probe mit allen
Umlenkungen: `integration/hintergruende/probe-hintergruende.sh`.

Pakete (G2-F) legen dieselben 17 Dateien nach
`/usr/share/querencia/hintergruende/`; Desktop und Einstellungen suchen dort
nach dem Benutzerordner.

## Grenzen

- Ultrabreite Bildschirme (21:9) schneiden die 16:9-Datei oben und unten
  stark ab; das Logo kann dort angeschnitten sein.
- Mehrere Bildschirme mit verschiedenen Seitenverhältnissen bekommen eine
  Datei; gewählt wird nach dem ersten Bildschirm.
- Die Lizenz der Bilder ist offen (ENTSCHEIDUNGEN Nr. 15).
