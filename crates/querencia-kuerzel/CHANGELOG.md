# Änderungen – querencia-kuerzel

## 0.2.3 – 2026-10-27

Italienisch, Spanisch, Französisch (i18n-reste, 08 QD-055):

- Alle Texte über GNU gettext, Textdomäne `querencia-kuerzel` (`po/`,
  `werkzeug/i18n.py`, `src/i18n.rs`): msgid englisch, `de.po` mit dem
  bisherigen Wortlaut, dazu `it.po`, `es.po`, `fr.po` (300 Texte) nach der
  Begriffstabelle . Kataloge neben der Binary
  (`share/locale`), sonst im Präfix, sonst `/usr/share/locale`.
- `data/programme.txt` ist jetzt englisch (Quelltext der msgids):
  Überschriften und Beschreibungen übersetzt der Katalog, Tastenfolgen mit
  dem Kontext „key“. `werkzeug/i18n.py pot` liest die Datei mit. Eine Datei
  gleichen Namens unter `$XDG_DATA_HOME`/`$XDG_DATA_DIRS` wird ebenso
  übersetzt, soweit ihre Zeilen im Katalog stehen.
- Tastennamen je Sprache wie auf der Tastatur (de Strg/Umschalt/Bild↑,
  it Maiusc/Invio/PagSu, es Mayús/Intro/RePág, fr Maj/Entrée/Pg préc).
- labwc-Beschreibungen als ganze Sätze statt aus Bausteinen (Arbeitsflächen-
  ziele, Richtungen), damit Artikel und Präpositionen in jeder Sprache
  stimmen; Deutsch unverändert.
- Desktop-Datei mit Name, GenericName, Comment, Keywords in it/es/fr.
- Tests prüfen die englischen msgids; `german_catalog_is_complete` prüft,
  dass jede msgid in `de.po` steht. Fehlermeldungen auf stderr und
  `--hilfe` bleiben deutsch (Protokoll).
- Neue Abhängigkeit `gettext-rs` 0.8 (wie Panel und Lautstärke, schon in
  Cargo.lock).

## 0.2.2 – 2026-10-24

Integrationsstand 0.2.2-2 (Tags `kuerzel/0.2.2-2`, `kuerzel/0.2.2-2-int`):
derselbe Code, gemergt auf runtime main `5510fc4` (Welle C3; Kürzel von
Dateien dort unverändert gegenüber `dateien/reiter-1`). Binary byte-gleich mit
0.2.2; aktiviert wird über `integration/kuerzel-022-2/`.

Querencia Dateien statt Querencia Files (Welle C, Folgezug „Runtime nach
der Ablösung“, 08 QD-168):

- Seite „Programme“: Der Block „Querencia Files“ heißt jetzt „Querencia
  Dateien“ und nennt dessen 38 Kürzel in der Reihenfolge seiner Menüs
  (Datei, Bearbeiten, Ansicht, Gehe zu, Lesezeichen, Reiter, Hilfe) und so,
  wie die Menüs sie schreiben (Umschalt vor Strg). Quelle:
  `crates/querencia-dateien/src/menue.rs` (`KUERZEL`, `KUERZEL_IM_BEREICH`),
  Suche Strg+F aus Welle C2, Reiter (Strg+T, Strg+W, Strg+Bild↑/↓,
  Umschalt+Strg+Bild↑/↓, Alt+1 … Alt+0, Umschalt+Strg+O), Kompaktansicht
  Strg+3 und Auswahl umkehren Umschalt+Strg+I aus Welle C3.
- Gegenüber Files geändert: Strg+1/2/3 = Symbole/Liste/Kompakt, Strg+D =
  Lesezeichen hinzufügen (Files: Duplizieren), Seitenleiste F9 (Files:
  Strg+B), Suchen Strg+F (Files: Strg+K), Umschalt+Entf = Löschen mit
  Nachfrage. Ohne Gegenstück in Dateien und deshalb nicht mehr aufgeführt:
  Strg+Z/Umschalt+Strg+Z, Strg+, (Einstellungen).
- Die Übersicht passt erst zu Querencia Dateien mit Welle C3 (Reiter);
  mit Dateien 0.1.0 fehlen die Reiter- und Kompaktkürzel noch.
- Test `built_in_file_is_clean`: Blockname, Zahl und Reihenfolge der
  Dateien-Kürzel, geänderte und entfallene Kürzel.

## 0.2.1 – 2026-10-22

Dunkelmodus (QD-109):

- Die Übersicht folgt der dunklen Darstellung: Nach dem Start meldet das
  Programm das Farbschema einmal neu (GTK 4.22 zeigte ein bei „dunkel“
  frisch gestartetes Fenster sonst hell, QD-081).
- Tastenkästchen, Quellzeile und Warnung nehmen ihre Farben aus dem Thema
  (`@borders`, `@theme_fg_color`, Mischungen aus `@theme_base_color`,
  `@theme_bg_color`, `@error_color`), ohne eigenen `@media`-Block. Hell
  bleiben Rand und Schrift gleich, das Kästchen ist #fafaf9 statt #fbfaf9,
  die Quellzeile #6d6d6c statt #6e6e6e; die Warnung ist ein dunkleres Rot
  (#672a2a statt #a0331a), das auch dunkel lesbar bleibt.

## 0.2.0 – 2026-10-21

Nachtrag der seit 0.1.0 live gegangenen Kürzel (Auftrag kuerzel-reiter):

- Seite „Programme“, Block „Querencia Terminal“: die Reiterkürzel aus
  Terminal 0.3.0 (live seit 20.10.2026 23:34) – Strg+Umschalt+T neuer
  Reiter, Strg+Umschalt+W Reiter schließen, Strg+Umschalt+Q Fenster
  schließen, Strg+Bild↑ / Strg+Bild↓ vorheriger / nächster Reiter,
  Strg+Umschalt+Bild↑ / Strg+Umschalt+Bild↓ Reiter verschieben,
  Alt+1 … Alt+9, Mittelklick auf einen Reiter, Doppelklick auf die freie
  Reiterleiste. Reihenfolge wie Hilfe → Tastenkürzel im Terminal
  (`src/tabs.rs` `shortcut_overview`, Tag `terminal/0.3.0`), Tastennamen
  wie im Rest der Datei (Bild↑ statt „Bild auf“).
- Block „Bildschirmfoto“: Strg+W / Strg+Q = Schließen (Strg+Q seit
  Screenshot 0.2.6, QD-051; live ist 0.2.8, `src/editor.rs` `ACCELS`).
  Die übrigen Blöcke stimmen mit den Live-Ständen überein (Abgleich
  21.10.2026: Desktop, Files QF-0p, Editor 0.2.2, Bilder 0.2.3, Rechner
  0.1.3, Dateisuche 0.1.3).
- Seite „Arbeitsfläche“: Super+D erscheint als „Arbeitsfläche anzeigen:
  alle Fenster minimieren oder wiederherstellen“ statt „startet „gdbus““
  (Code aus G2-AL, `44f9f4f`, erst mit diesem Bau live). Die Seite liest
  die rc.xml der laufenden Sitzung; Super+D steht dort erst nach dem
  Vorgaben-Abgleich der nächsten Anmeldung.
- Tests: Reiterkürzel des Terminals; Super+F und Alt+F11 als „Vollbild ein
  oder aus“ (QD-065, G2-AK), F11 nicht belegt.

## 0.1.0 – 2026-10-18

Erste Fassung (G2-R, Tag `kuerzel/0.1.0`): Fenster „Tastenkürzel“ mit den
Seiten „Arbeitsfläche“ (labwc-Kürzel der laufenden Sitzung) und
„Programme“ (`data/programme.txt`), `--liste`, `--seite`, `--rc`.
