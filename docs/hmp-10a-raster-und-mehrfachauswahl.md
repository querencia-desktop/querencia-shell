# HMP-10a: Raster (Snap-to-Grid) und Mehrfachauswahl

Baut auf HMP-10 (`hmp-10-symbole-verschieben-umbenennen.md`) auf. Alles aus HMP-10 bleibt: Ziehen, Kante-und-Abstand-Speicherung, Reflow, Umbenennen, Starter, Dateimonitor. Mit ausgeschaltetem Raster verhält sich der Desktop wie HMP-10 (HMP-10-Prüfung mit `HMP10_RASTER=aus`).

## Raster

- **Unsichtbares Raster der Symbolfläche** (der Fläche zwischen den Panels), nicht des Bildschirms.
  - Spalten = so viele ganze Kacheln (100 px) wie in die Breite minus 2 × 8 px Rand passen; Zeilen ebenso mit 88 px.
  - Die Zellen werden **gleichmäßig verteilt**: Die erste Spalte hat 8 px Abstand zum linken Rand, die letzte 8 px zum rechten. Der Abstand ist nie kleiner als eine Kachel.
  - Beispiele:

    | Fläche | Raster | Abstand |
    |---|---|---|
    | 1280×656 | 12×7 | 105,8 × 92 px |
    | 1920×1016 | 19×11 | 100,2 × 91,2 px |
    | 1024×704 | 10×7 | 100,9 × 100 px |
    | 640×296 (Skalierung 2) | 6×3 | 104,8 × 96 px |

  - Pixel werden je Zelle gerundet: benachbarte Abstände unterscheiden sich höchstens um 1 px.
- **Einrasten:** Beim Ziehen folgt das durchscheinende Abbild frei dem Zeiger.
  - Beim Loslassen kommt das Symbol in die Zelle, die seinem Ablagepunkt am nächsten liegt.
  - Ist sie belegt, kommt es in die nächste freie Zelle (kürzeste Entfernung, bei Gleichstand die obere, dann die linke).
  - Andere Symbole bewegen sich nie.
- **Beschriftungen:** Eine Kachel ist nie größer als ihre Zelle.
  - GtkFixed teilt die Mindestgröße zu; lange Namen brechen innerhalb der Kachel um und enden nach zwei Zeilen mit „…“.
  - Der Desktop meldet jede zu große Kachel („Kachel größer als die Zelle“); die Prüfung verlangt null solche Meldungen.
- **Gespeichert wird wie in HMP-10** (Kante und Abstand in Pixeln). Angezeigt wird der gespeicherte Platz auf der nächsten Zelle des aktuellen Rasters. Deshalb:
  - geht bei einer anderen Auflösung oder Skalierung nichts verloren, und eine Ecke bleibt eine Ecke (rechts/unten verankerte Symbole liegen in der letzten Spalte/Zeile);
  - ist für alte Plätze (HMP-10, pixelgenau) **keine Datenmigration nötig**. Sie werden auf dem Raster *angezeigt*; die Datei bleibt, bis der Benutzer etwas verschiebt oder ausrichtet.
- **Neue Symbole:** erste freie Zelle, Spalten von links, jede von oben (wie GNOME 2).
- **„Nach Namen aufräumen“:** füllt dieselben Zellen in GNOME-2-Reihenfolge.

## Arbeitsflächenmenü

| Eintrag | Wirkung |
|---|---|
| Nach Namen aufräumen | wie bisher, jetzt auf dem Raster |
| Symbole am Raster ausrichten | setzt jedes Symbol einmal auf die nächste freie Zelle und speichert das; jedes behält die Kante, von der es gemessen wird; keine Datei wird berührt |
| Am Raster ausrichten (✓) | Einrasten an/aus, Standard an; gespeichert. Aus: freie Pixelplätze wie HMP-10, die gespeicherten Plätze erscheinen wieder pixelgenau |

Das Häkchen steht in der Symbolspalte des klassischen Menüs, wie GNOME 2 Häkchen zeichnete. Das geteilte Menü-Widget des Panels (`classic_menu.rs`) ist dafür **nicht** geändert worden.

## Mehrfachauswahl

| Bedienung | Wirkung |
|---|---|
| Klick | wählt genau dieses Symbol |
| Strg+Klick | nimmt es in die Auswahl auf oder heraus |
| Umschalt+Klick | Bereich vom zuletzt angeklickten Symbol bis zu diesem, in Leserichtung des Desktops (Spalten von links, jede von oben) |
| Strg+A | alle Symbole |
| Esc, Klick ins Leere | Auswahl aufheben |
| Ziehen auf freier Fläche | Auswahlrahmen (dünne orange Kante, leicht orange gefüllt, Ambiance-Farbe), in jede Richtung; ausgewählt ist, was der Rahmen berührt. Mit Strg/Umschalt kommt es zur bisherigen Auswahl dazu |

- Hervorhebung wie bisher: orange hinterlegte Beschriftung und abgedunkeltes Symbol.
- Ein Klick auf ein Symbol einer größeren Auswahl lässt die Auswahl beim Drücken stehen (damit sie gezogen werden kann). Wird nicht gezogen, bleibt beim Loslassen nur dieses Symbol ausgewählt (GNOME-2-Verhalten).
- Rechtsklick auf ein ausgewähltes Symbol behält die Auswahl.
- **Keine Mehrfachoperationen:**
  - „Öffnen“, Doppelklick und F2 wirken nur auf das angeklickte Symbol;
  - „Umbenennen …“ ist bei mehreren ausgegraut;
  - es gibt kein Löschen.

## Gemeinsames Verschieben

Gezogen wird das angeklickte Symbol; die übrige Auswahl folgt als durchscheinende Abbilder.

**Beim Loslassen** (`Geometry::place_group`):

- Mit Raster bewegt sich die Gruppe um **ganze Zellen**: Die Form bleibt exakt, die Pixelabstände bis auf ±1 px Rundung.
- Ohne Raster bewegt sie sich um Pixel.
- Am Bildschirmrand hält die ganze Gruppe an (kein Symbol verlässt die Fläche, keins rutscht unter ein Panel).
- Läge ein Mitglied auf einem nicht ausgewählten Symbol, wählt der Desktop die **nächste Verschiebung, bei der alle frei sind**. Die Form bleibt, andere Symbole bewegen sich nie.
- Passt die Anordnung nirgends hin, kommen die Symbole einzeln auf die nächsten freien Plätze, aber nur wenn **jedes** einen findet (`Geometry::move_group`).
- Sonst wird die Verschiebung **abgelehnt** („Verschieben abgelehnt (kein Platz für N Symbole)“): Alle bleiben, wo sie sind, nichts wird gespeichert. Ein einzelnes Symbol ohne freien Platz ebenso.
- Landet ein Symbol wieder auf seinem bisherigen Platz (z. B. weil sonst nichts frei ist), ist das keine Verschiebung: Sein gespeicherter Platz bleibt unverändert. So überschreibt eine Ablage auf einer kleinen oder vollen Fläche nicht den Platz, der für einen größeren Bildschirm gewählt wurde.

**Gespeichert** werden alle neuen Plätze, vorn in der Zustandsdatei (Vorrang beim Reflow).

## Speicherung

- **Format bleibt Version 1.** Neu ist nur die Zeile `einstellung → raster → ja|nein`. HMP-10 überspringt sie als unbekannt und liest und schreibt alles andere. Ein Rückfall auf HMP-10 verliert keinen Platz.
- **Vor dem ersten Schreiben** legt HMP-10a einmal eine Sicherung an (`store::back_up`), absturzsicher und ohne je etwas zu überschreiben:
  1. Kopie (`desktop-symbole.vor-hmp10a`) und Abschlussmarke (`….sha256`: SHA-256 und Länge) werden in neue temporäre Dateien geschrieben (`create_new`), mit `sync_all` auf die Platte gebracht und zurückgelesen.
  2. Erst dann werden sie mit `hard_link` veröffentlicht, das bei vorhandenem Namen scheitert: die Kopie zuerst, die Marke zuletzt. Die Marke schließt die Sicherung ab.
  3. **Gültig** ist eine Sicherung nur mit passender Marke. Eine Kopie ohne Marke (Absturz, ältere Version), eine falsche Marke oder eine Marke allein gilt nie als Sicherung. Solche Reste werden weder überschrieben noch gelöscht, sondern als `….unvollstaendig-<pid>-<zeit>` beiseitegelegt und protokolliert; danach wird neu gesichert.
  4. Ein Ordner oder Link an diesen Stellen ist ein Fehler; durch einen Link wird nie geschrieben.
  5. Gibt es noch keine Positionsdatei, ist nichts zu sichern; Reste bleiben dann unberührt.
- **Ohne nachgewiesene Sicherung wird nichts geschrieben** („Sicherung fehlgeschlagen …: Anordnung nicht gespeichert“), weder `desktop-symbole` noch `desktop-order`. Die Änderungen bleiben im Speicher; beim nächsten Schreibversuch wird die Sicherung erneut versucht und erst danach gespeichert.

## Prüfung

- `pruefen-hmp10.sh` mit `HMP10_TREIBER=…/hmp10a-treiber.py`;
- `HMP10_ANMELDUNG2=1`: zweite Anmeldung mit neuem labwc und gleichem Profil;
- `HMP10_NUR=migration` mit `HMP10_ZUSTAND=<Datei>`: vorhandene HMP-10-Datei;
- `HMP10_RASTER=aus`: HMP-10-Regression;
- `HMP10_NUR=sicherung` (mit `HMP10_ZUSTAND`): Sicherung scheitert absichtlich (Ordner im Weg);
- `HMP10_NUR=platzmangel`: 400×300, 6 Rasterplätze für 14 Symbole;
- `HMP10_NUR=sicherung-unvollstaendig` (mit `HMP10_ZUSTAND`): halbe Sicherung ohne Marke liegt schon da.
