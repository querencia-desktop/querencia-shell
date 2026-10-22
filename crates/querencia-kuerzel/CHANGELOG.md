# Änderungen – querencia-kuerzel

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
