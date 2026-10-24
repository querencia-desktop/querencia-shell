# Änderungen – Querencia Dateien (`querencia-dateien`)

## 0.2.0 – 24.10.2026 (Beta, Welle C, QD-168)

Beta des eigenen Dateimanagers: Netzwerk, Miniaturen und Suche, Reiter und
Kompaktansicht. Querencia Dateien ist ab dieser Fassung das Ordnerprogramm;
die Desktop-Datei nennt `MimeType=inode/directory;`. Der Info-Dialog nennt die
Stufe „Beta“.

### Welle C1 – Netzwerk (Tag `dateien/netzwerk-1`)

- Ort „Netzwerk“ (`network:///` über GVfs) in der Seitenleiste und unter
  Gehe zu → Netzwerk; Doppelklick öffnet, nicht eingehängte Orte werden bei
  Bedarf eingehängt, Verknüpfungen im Netzwerk öffnen wie Ordner; jedes Gerät
  steht nur einmal da.
- „Mit Server verbinden …“ nach Nautilus 2.30 (Datei-Menü und Kontextmenü
  von „Netzwerk“): SFTP, SMB mit Domäne, FTP, WebDAV und WebDAVS sowie
  „Eigene Adresse“; Server, Port, Freigabe, Ordner, Benutzer und auf Wunsch
  ein Lesezeichen.
- Eigene Kennwortfrage mit Benutzer, Kennwort und Domäne; „Kennwort merken“
  nur auf Wunsch.
- Frage nach unbekanntem Serverschlüssel mit lesbarem Fingerabdruck vor dem
  Kennwort (QD-008), „Trotzdem verbinden“ nur ausdrücklich; keine doppelten
  Fragen und Meldungen.
- Eingehängte Netzorte in der Seitenleiste mit ⏏ zum Aushängen; Fehler in
  deutschen, konkreten Sätzen statt GVfs' bloßem „Verbindung fehlgeschlagen“.

### Welle C2 – Miniaturen und Suche (Tag `dateien/miniaturen-4`)

- Miniaturen (freedesktop-Cache `~/.cache/thumbnails`, derselbe wie in
  Querencia Files) in Symbol- und Listenansicht; Bilder im Programm, andere
  Typen über installierte Thumbnailer nur im Sandkasten (bwrap), ohne bwrap
  keine fremden Thumbnailer.
- Einstellungen → Vorschau: „Vorschaubilder anzeigen“ Immer / Nur lokale
  Dateien / Nie und „Nur für Dateien bis“ (Vorgabe 10 MB), wirken sofort;
  Rahmen um Fotos wie Nautilus 2.30.
- Namenssuche (Strg+F, Gehe zu → „Suchen …“, Lupe in der Werkzeugleiste):
  Leiste „Suchen nach:“, sucht ab dem gezeigten Ordner in allen
  Unterordnern (Groß- und Kleinschreibung egal, jedes Wort), Ergebnisse als
  Ansicht „Suchergebnisse“ mit Ortsspalte; Esc hält an, ein zweites Esc
  beendet; „Im Ordner anzeigen“ im Kontextmenü, „Erweiterte Suche …“ öffnet
  Querencia Dateisuche.

### Welle C3 – Reiter, Kompaktansicht, Migration (Tag `dateien/reiter-1`)

- Reiter wie Nautilus 2.30 (`src/reiter/`): eigenes Ordnermodell, eigene
  Ansicht und eigener Verlauf je Reiter, Leisten folgen dem gezeigten
  Reiter; Reiterleiste und Menü „Reiter“ ab zwei Reitern; Strg+T, Strg+W
  (letzter Reiter schließt das Fenster), Strg+Bild↑/↓, Umschalt+Strg+Bild↑/↓,
  Alt+1 … Alt+0; Mitteltaste auf Ordner, Pfadknopf und Ort öffnet einen
  Reiter dahinter (mit Umschalt ein Fenster), „In neuem Reiter öffnen“
  (Umschalt+Strg+O, Datei-Menü, Kontextmenüs), mehrere Ordner öffnen in
  Reitern; Dateien auf einen Reiterkopf ziehen legt sie dort ab.
- Kompaktansicht (Strg+3, `src/kompaktansicht.rs`): Spalten waagrecht,
  kleines Symbol mit Namen, Zoom 50–200 %, „Alle Spalten haben dieselbe
  Breite“ (Einstellungen → Anzeige).
- Migration M6 nach dem echten Files-Format: Klickwerte des benutzten
  Modus (`grid_*`, `explorer_*`, `list_*`), Einfacher Klick nur, wenn
  Ordner und Dateien mit einem Klick öffnen; der erste Start ohne Files
  schreibt `dateien.ini` sofort.
- Behoben (0.1.0): geschlossene Fenster blieben mit Ordnerüberwachung im
  Speicher; Panik beim Programmende nach Strg+W auf dem letzten Fenster;
  Umschalt+Strg+I war nicht gebunden.

## 0.1.0 – 23.10.2026 (Welle B, QD-140)

Erste Fassung des eigenen Dateimanagers nach Nautilus 2.30 (Browser-Modus),
installierbar **neben** Querencia Files; kein Ordnerprogramm
(`inode/directory`), keine Änderung an `mimeapps.list`.

- Fenster nach B §3.2: Menüleiste (Datei, Bearbeiten, Ansicht, Gehe zu,
  Lesezeichen, Hilfe; Texte und Kürzel nach B §3.3, Stufe 0.1) über
  `querencia-oberflaeche::classic_menu`, Werkzeugleiste mit Zoom und
  Ansichtswahl, Knopfpfad mit Überlauf und Ortsfeld (Strg+L, „/“, „~“,
  Tab-Vervollständigung), Seitenleiste „Orte“ (Persönlicher Ordner,
  Arbeitsfläche, Dateisystem, Netzwerk, Papierkorb; Laufwerke mit
  Einhängen und ⏏; Lesezeichen aus `~/.config/gtk-3.0/bookmarks`),
  Statusleiste wie Nautilus 2.30, Leiste „Papierkorb leeren …“ im
  Papierkorb (B1).
- Ordnermodell mit Sortierung nach Name/Größe/Typ/Datum
  (`FilenameCollationKey`, Ordner zuerst), verborgene Dateien, Lesen in
  Blöcken, Überwachung über `querencia-dateiarbeit` (B1).
- Symbolansicht (Zoom 33–400 %, Tippsuche, Gummiband, Umbenennen im Platz)
  und Listenansicht (Spalten, Sortierpfeil) als austauschbare Module gegen
  das Trait `Ansicht`, Kontextmenüs (B2).
- Ausschneiden/Kopieren/Einfügen (GNOME-, KDE- und URI-Formate), Ziehen und
  Ablegen nach Nautilus-Regel, Papierkorb, endgültiges Löschen,
  Wiederherstellen, Ordner und Dokument anlegen, Fenster
  „Dateioperationen“, Konfliktfrage, Eigenschaften (Grundlegend,
  Zugriffsrechte, Öffnen mit) über `querencia-dateiarbeit::auftraege` (B3).
- Öffnen über `plan_open` (Rückfrage bei ausführbaren Textdateien und nicht
  vertrauenswürdigen Startern), Öffnen mit ▸.
- Einstellungen mit fünf Reitern (`~/.config/querencia/dateien.ini`, wirkt
  sofort), einmalige Übernahme von sechs Werten aus Querencia Files (M6).
- Info-Dialog mit Release aus `/usr/share/querencia/release`, Hilfe-Menü
  (Online-Hilfe, Fehler melden).
- Deutsch über gettext (`po/de.po`, 294 Texte), Englisch als msgid;
  Themenfarben des Systemthemas, hell und dunkel.
