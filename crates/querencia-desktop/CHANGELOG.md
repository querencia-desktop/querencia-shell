# Änderungen – querencia-desktop

## 0.3.0 – 2026-10-23

Welle A von „Querencia Dateien“ (QD-140): Der Desktop nutzt die Bibliothek
`querencia-dateiarbeit` für alles, was er mit Dateien und Laufwerken tut.

**Verhaltensänderung – Konfliktregel „fragen“ statt „nie überschreiben“
(E2).** Bis 0.2.7 bekam eine Kopie auf einen belegten Namen still einen
freien Namen („notiz (Kopie).txt“), ein Verschieben auf einen belegten Namen
wurde abgelehnt. Jetzt fragt der Desktop wie Nautilus 2.30: „Datei „x“
ersetzen?“ bzw. „Ordner „x“ zusammenführen?“ mit Größe und Datum beider
Dateien und den Antworten Abbrechen, **Überspringen** (Vorgabe, Eingabetaste),
Umbenennen (freier Name vorgeschlagen), Ersetzen bzw. Zusammenführen, dazu
„Diese Aktion auf alle Dateien anwenden“. Ohne Klick auf Ersetzen wird nichts
überschrieben; ersetzt wird erst, wenn das neue Objekt vollständig da ist.
Einfügen in denselben Ordner legt wie bisher eine Kopie mit freiem Namen an.

- **Fortschritt und Abbruch:** Kopieren, Verschieben, Papierkorb, endgültig
  Löschen und Papierkorb leeren laufen als Aufträge in eigenen Fäden. Dauert
  einer länger als eine Sekunde, zeigt das Fenster „Dateioperationen“ was,
  wohin, wie viel und wie lange noch, mit „Abbrechen“. Ein abgebrochener
  Auftrag hinterlässt keine halbe Datei und keinen halben Ordner unter
  echtem Namen. Solange ein Auftrag auf eine Antwort wartet, bleibt das
  Fenster weg (es nähme der Frage die Tastatur). Das Fenster hat eine feste
  Breite, damit „Abbrechen“ nicht mit den wechselnden Zahlen wandert.
- **Einhängen (E3):** Nicht eingehängte Wechseldatenträger und optische
  Medien erscheinen als Symbol; Doppelklick hängt ein und öffnet, das Menü
  bietet „Einhängen“. Ein während der Sitzung eingesteckter Datenträger wird
  eingehängt, wenn GIO es vorsieht (Automount), ohne ein Fenster zu öffnen.
  Kennwort- und Auswahlfragen stellt der Desktop selbst (nicht modal); GTKs
  eigener modaler Kennwortdialog brachte gtk4-layer-shell beim Schließen
  zum Absturz.
- **Papierkorb:** gezählt über einen Monitor ohne Abfrage im Hauptfaden
  (`papierkorb::Papierkorb`); „Papierkorb leeren“ ist ein Auftrag mit
  Fortschritt und Abbruch.
- **Überwachung** des Arbeitsflächen-Ordners über `FolderWatch` (gebündelte
  Änderungen, Warten auf einen fehlenden Ordner wie seit 0.2.5).
- **Neuer Ordner, neues Dokument, Umbenennen** als Aufträge, die nie
  ersetzen (`RENAME_NOREPLACE`).
- **Symbole ohne Vorladen (QD-106):** Arbeitsfläche, Eigenschaften und das
  Desktop-Menü laden Symbole wie das Panel ohne GTKs Vorladen im
  Thread-Pool (`symbole`, `IconLoading::WithoutPreload`). Gewollte
  Verhaltensänderung gegenüber 0.2.7; belegt mit dem Belastungslauf. Die
  eingebauten Laufwerksbilder (SVG) behält der Desktop geladen: ohne
  Vorladen hätte er sie bei jedem Neuaufbau im Hauptfaden neu geladen, nach
  einem abgebrochenen großen Auftrag bis zu 7 s ohne Reaktion.
- Programm für Ordner, Rechner, Papierkorb und Datenträger aus
  `oeffnen::FILE_MANAGERS`: Querencia Dateien zuerst, dann Querencia Files
  (neue und alte Kennung).
- Entfallen im Desktop (jetzt im Crate, Tests dort): `fileops.rs`,
  `rename.rs`, große Teile von `volumes.rs` und `icons.rs`, die vier
  Laufwerkssymbole unter `data/icons/`. Neue Module
  `desktop_view/fortschritt.rs`, `frage.rs`, `kennwort.rs`.
- Die Ablage enthält zusätzlich `querencia-dateiarbeit.mo`.

## 0.2.7 – 2026-10-23

Integrationsstand 0.2.7-2 (Tags `desktop/0.2.7-2`, `desktop/0.2.7-2-int`):
derselbe Code, gemergt auf runtime main `5a5b65c`. Programm und Kataloge
sind byte-gleich mit 0.2.7 (`desktop/0.2.7`); `desktop/0.2.7-int` wird nicht
aktiviert.

Nur ein Umbau, sichtbar ändert sich nichts gegenüber 0.2.6.

- Die Bausteine des Panels (klassische Menüs, Symbolthema, Farbschema,
  Settings-Portal) kommen aus der Bibliothek `querencia-oberflaeche` des
  Workspace statt über `#[path]` aus dem Quelltext des Panels. Die Weiche
  `cfg(querencia_desktop)` und `build.rs` entfallen.
- Das Desktop-Menü lädt seine Symbole weiter auf GTKs eigenem Weg; das
  wählt der Desktop jetzt beim Start ausdrücklich
  (`classic_menu::set_icon_loading(IconLoading::Gtk)`).
- Der Desktop bindet die Textdomäne `querencia-oberflaeche` an sein
  Katalogverzeichnis und liefert `querencia-oberflaeche.mo` mit (Tastennamen
  der Kürzel; das Desktop-Menü zeigt bisher keine).
- Die 27 Tests der geteilten Module laufen jetzt einmal in der Bibliothek
  statt zusätzlich im Desktop.

## 0.2.6-2 – 2026-10-22

Neubau von 0.2.6 auf dem Stand von main mit Panel 0.2.5. Die mit dem Panel
geteilte Menüdatei `classic_menu.rs` setzt Symbole jetzt über eine eigene
Funktion; der Desktop nimmt darin weiter GTKs eigenen Weg (`cfg(querencia_desktop)`).
Sichtbar ändert sich nichts gegenüber 0.2.6.

## 0.2.6 – 2026-10-22

**Querencia Files unter neuer Kennung.** Rechner, Persönlicher Ordner,
Papierkorb und Datenträger öffnen in Querencia Files über dessen neuen
Starter `org.querencia.Files.desktop`; fehlt er, über den bisherigen
`io.github.lgse.Strata.desktop`. Damit gehen beide Reihenfolgen beim
Einspielen von Files und Desktop. Das Log nennt beim Start, welcher Starter
gefunden wurde („Dateiverwaltung: …“).

Enthält alles aus 0.2.5.

## 0.2.5 – 2026-10-22

**Raster nach der Schrift.** Die Zellen der Arbeitsfläche richten sich nach
der Schrift der Sitzung (Name, Größe, Skalierung) statt nach festen 100 × 88
Pixeln. Bei größerer Schrift waren zweizeilige Beschriftungen höher als ihre
Zelle, und die Zeile reichte nicht für „Persönlicher“; Netzlaufwerke wie
„daten auf speicher.local“ wurden gekürzt und rückten an den Papierkorb.

- Eine Zeile fasst 14 Durchschnittszeichen, die Höhe zwei Zeilen und einen
  Abstand; bei 11 pt sind das 116 × 96 Pixel, bei 12 pt 125 × 98.
- Wechselt die Schrift im Lauf, folgt das Raster ohne Neustart. Gemerkte
  Plätze bleiben, sie werden auf die nächste Zelle gelegt.
- Lange Namen nutzen die volle Zellbreite; Wörter werden nur getrennt, wenn
  sie allein nicht in die Zeile passen.
- Ein ausgewähltes Symbol zeigt seinen ganzen Namen, wie unter GNOME 2.
- Laufwerke und Netzlaufwerke stehen weiter unter dem Papierkorb, jetzt mit
  Abstand und ungekürzt.

**Arbeitsflächen-Ordner später angelegt.** Fehlt der Ordner beim Start,
wartet der Desktop im nächsten vorhandenen Elternordner. Sobald der Ordner
entsteht (auch durch Verschieben), erscheinen seine Dateien sofort; wird er
gelöscht, wartet der Desktop wieder.

**Kein Programm für eine Datei.** Statt nur einer Logzeile erscheint ein
Hinweis mit Dateiname und Typ. „Mit Programm öffnen …“ öffnet die
Programmauswahl von GTK.

**Englisch.** Alle Texte der Arbeitsfläche (Menüs, Dialoge, Papierkorb,
Dateioperationen, Laufwerke, Eigenschaften, Namen der Sondersymbole) laufen
über gettext. Quelltexte sind englisch, Deutsch kommt aus `po/de.po` und
bleibt im Wortlaut wie bisher. Die Sprache wählt `LANGUAGE`; ohne
deutschen Katalog neben dem Programm (`share/locale`) ist die Oberfläche
englisch.

- Neue Namen für Kopien und neue Dateien folgen der Sprache („notiz (Kopie).txt“,
  „notes (copy).txt“; „Neuer Ordner“, „New Folder“).
- Der Kopf der Zustandsdatei `desktop-symbole` nennt keine interne Kennung
  mehr; Inhalt und Format sind unverändert.

## 0.2.4 – 2026-10-22

Die Beschriftung der Arbeitsflächen-Symbole steht wieder ohne Platte auf dem
Hintergrundbild (Wunsch des Besitzers vom 22.10.2026: die dunkle Platte hinter
„Rechner“, „Persönlicher Ordner“, „Papierkorb“ und den Startern stört auf dem
Bild).

- Weißer Text mit dunklem Schattenkranz: drei gestapelte Schatten, zwei
  weiche rund um die Buchstaben und einer nach rechts unten versetzt wie
  unter GNOME 2. Die 55 % schwarze Platte aus 0.2.1 entfällt.
- Ausgewählte Symbole und Ablageziele behalten die orange Platte.
- Hell und dunkel gleich; der Schatten ist in keinem Farbschema anders.

## 0.2.3 – 2026-10-21

Dateien auf der Arbeitsfläche arbeiten mit anderen Programmen zusammen
(Befund des Besitzers 21.10.2026, QD-108: Ziehen in Querencia Files, Strg+C/Strg+V
und der Papierkorb gingen nicht; das Kontextmenü bot nur „Öffnen“ und
„Umbenennen …“).

**Kontextmenü wie unter GNOME 2.** Auf Dateien, Ordnern und Startern:
„Öffnen“, „Öffnen mit“ (nur Dateien und Ordner), „Ausschneiden“, „Kopieren“,
„Einfügen in den Ordner“ (nur Ordner), „Umbenennen …“, „In den Papierkorb
verschieben“ und „Eigenschaften“.

- „Öffnen mit“ listet die Programme für den Typ der Datei. Bei leeren
  Dateien zählt wie seit 0.2.2 der Name. Darunter steht „Andere
  Anwendung …“, das GTKs Programmauswahl öffnet.
- Auf freier Fläche steht neu „Einfügen“. Eingefügte Dateien erscheinen
  dort, wo das Menü geöffnet wurde.
- Einträge, die gerade nicht gehen, sind ausgegraut: „Einfügen“ bei leerer
  Zwischenablage, „Eigenschaften“ bei mehreren ausgewählten Symbolen.

**Tastatur.**

- Strg+X, Strg+C und Strg+V arbeiten mit der Zwischenablage.
- Entf verschiebt die Auswahl in den Papierkorb.
- Umschalt+Entf löscht endgültig, erst nach einer Rückfrage. Vorgabe in der
  Rückfrage ist „Abbrechen“, Escape bricht ab.
- F2 benennt um, wie bisher.

**Zwischenablage im üblichen Format.**

- Kopierte Dateien liegen als `GdkFileList`, als `text/uri-list` und als
  `x-special/gnome-copied-files` in der Zwischenablage, beim Ausschneiden
  zusätzlich mit KDE-Markierung. Das ist dasselbe Format, das Querencia Files
  schreibt.
- Einfügen liest dieselben Formate aus jedem Programm.
- Ausgeschnittene Dateien werden erst beim Einfügen verschoben und sehen bis
  dahin blass aus.

**Ziehen und Ablegen in beide Richtungen.**

- Das Ziehen von Symbolen ist jetzt ein echtes Ziehen. Dateien kommen als
  Dateiliste in Querencia Files, GTK-Dateidialoge und andere Fenster.
- Dateien aus anderen Programmen werden in den Arbeitsflächen-Ordner kopiert
  und liegen dort, wo man losgelassen hat. Bietet die Quelle nur Verschieben
  an (Umschalt in Files), werden sie verschoben.
- Ein Ordnersymbol als Ziel nimmt die Dateien auf. Das Papierkorb-Symbol
  wirft sie weg. Das Ziel ist dabei orange hervorgehoben.
- Innerhalb der Arbeitsfläche ordnet Ziehen auf freie Fläche wie bisher nur
  die Symbole an, einzeln oder als Gruppe, mit derselben Vorschau im Raster.

**Dateioperationen.**

- Kopieren, Verschieben, Papierkorb und Löschen laufen über GIO, ohne die
  Oberfläche anzuhalten.
- Es wird nie etwas überschrieben. Eine Kopie auf einen belegten Namen heißt
  „Name (Kopie).txt“. Verschieben auf einen belegten Namen wird abgelehnt.
- Ein Ordner kommt nie in sich selbst. Verknüpfungen werden als
  Verknüpfungen behandelt, nie verfolgt.
- Was nicht klappt, zeigt ein Fehlerdialog mit Namen und Grund, statt nur im
  Protokoll zu stehen.

**Eigenschaften.** Ein kleines Fenster zeigt Name, Typ, Größe (bei Ordnern
Anzahl und Gesamtgröße), Ort und Änderungsdatum.

Unverändert: Panel und Kürzel (keine Änderung an gemeinsamen Quellen),
Beschriftung, Standardbild, Öffnen per Doppelklick, Umbenennen, Anordnen,
„Papierkorb leeren …“, Datenträger.

## 0.2.2 – 2026-10-21

Leere Dateien öffnen sich wieder (Befund vom 21.10.2026: Doppelklick auf ein
leeres `leer.txt` öffnete den Editor nicht).

- GIO nennt eine leere Datei `application/x-zerosize`; dafür gibt es kein
  Programm. Bisher stand dann nur „nicht geöffnet: Es wurde keine Anwendung
  gefunden …“ im Protokoll.
- Hat der Inhaltstyp einer Datei kein Programm, entscheidet jetzt wie in
  Nautilus der Dateiname (`.txt` → `text/plain` → Standardanwendung, hier
  Querencia Editor). Das Protokoll nennt den Weg
  (`… application/x-zerosize ohne Programm, öffne als text/plain mit …`).
- Sagt auch der Name nichts (leere Datei ohne Endung), wird nichts gestartet
  und der Fehler bleibt im Protokoll, wie bisher.
- Alles andere unverändert: Dateien mit eigenem Programm, Ordner, Starter,
  Laufwerke.

## 0.2.1 – 2026-10-21

Zwei Punkte für Alpha 2 (Zweig `desktop-0.2.1`, Hintergründe 0.2,
Entscheidung vom 21.10.2026: alle 16 neuen Hintergründe, Standard
„tiefblau“).

**Beschriftung der Symbole lesbar auf hellen Hintergründen.**

- Die Beschriftung liegt auf einer abgerundeten dunklen Hinterlegung
  (`alpha(#000000, 0.55)`, Radius 4 px, Innenabstand 1 px oben/unten und
  3 px seitlich). Der Schatten der weißen Schrift ist kräftiger
  (`1px 1px 2px #000000`, vorher 85 % Deckkraft).
- Weiß bleibt damit auch über reinem Weiß bei 4,7:1. Gemessen auf allen
  33 Hintergründen (17 alte, 16 neue) bei mindestens 5,1:1 gegen das
  hellste Pixel unter der Schrift (`huegel`). Vorher lagen die sechs
  hellen Landschaften bei 1,1–1,6:1.
- Bei Auswahl ersetzt wie bisher die orange Hinterlegung die dunkle.
  Beim Umbenennen bleibt das Feld unverändert.
- Hell und dunkel sehen gleich aus: Der dunkle Block hat keine Regel für
  die Beschriftung, und die Beschriftungszonen sind pixelgleich.
- Keine Kachel wird größer als ihre Zelle (zweizeilige Namen gemessen).
- Ein Test prüft die Deckkraft gegen 4,5:1 über Weiß.

**Standardbild „tiefblau“** (übernommen aus `standard-tiefblau` `3fafafd`).

- Ohne Bild in `desktop.ini` (`File=` leer oder fehlt) zeigt die
  Arbeitsfläche `querencia-tiefblau.png` aus `querencia/hintergruende/` der
  XDG-Datenordner (Repo und Paket `querencia-hintergruende`). Fehlt es, bleibt
  es beim bisherigen `querencia-blau.png`.
- Das Protokoll nennt das gefundene eingebaute Bild
  (`querencia-desktop: eingebautes Bild <Pfad>`).
- Farbe (`Color=`, Standard `#1c4e96`) und alles andere unverändert.

## 0.2.0 – 2026-10-21

CSS-Welle dunkel (Alpha 2, Zweig `welle-dunkel`): Die Arbeitsfläche
folgt der dunklen Fassung von Querencia-Klassisch (Thema 0.2).

- `DUNKEL_CSS`: Das Kontextmenü übernimmt Zeile für Zeile die dunklen
  Menüregeln des Panels (ein Test vergleicht sie). Das Feld „Umbenennen“
  wird dunkel (`#2b2a27`, Schrift `#f2f1f0`). Die Beschriftungen der Symbole
  bleiben weiß mit Schatten auf dem Bild.
- Schaltet über `farbschema.rs` des Panels (per `#[path]`): dunkel nur bei
  dunklem Farbschema und einem Thema mit `gtk-dark.css`. Unter Thema 0.1
  bleibt alles hell.
- Startfehler GTK 4.22: einmal nach dem Start
  `notify("gtk-interface-color-scheme")`.
- Hell unverändert, isoliert gemessen pixelgleich zum vorigen Stand.

## 0.1.0 – bis 2026-10-20

Stufen ohne eigene Versionsnummer (HMP-08 … QE-04a), zuletzt `4b45feac`;
Einzelheiten in `docs/`.
