# Änderungen – Querencia Dateien (`querencia-dateien`)

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
