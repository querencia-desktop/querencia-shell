# Desktop 0.2.5/0.2.6 – Raster nach der Schrift, später angelegter Ordner, Hinweis ohne Programm, Englisch, neue Files-Kennung

Stand 22.10.2026, Zweig `desktop-025` ab main `7356f64` (Desktop 0.2.4).
Nur `crates/querencia-desktop`; Panel und Kürzel unverändert. 0.2.5 (Tags
`desktop/0.2.5`, `desktop/0.2.5-int`) ist seit 22.10.2026 21:28 live; 0.2.6
enthält es und dazu §7. 0.2.6-2 ist 0.2.6 auf main mit Panel 0.2.5 (Zweig
`desktop-026`, geteilte `classic_menu.rs`).

## 1. Laufwerkssymbole im Raster (QD-091)

**Befund.** Live (DejaVu Sans 12, 1920 × 1080) zeichnet eine Kachel mit
zweizeiliger Beschriftung 92 px hoch, die Zelle war 88 px, der Zeilenabstand
des Rasters 91,2 px. Die Beschriftung hatte 92 px Breite (Zelle 100 minus
Innenabstand); „Persönlicher“ braucht in 12 pt 98 px. Folgen:

- „Persönliche | r Ordner“ (im Live-Log seit der Schriftgröße 12 belegt);
- das Netzlaufwerk unter dem Papierkorb rückte bis auf 1 px an dessen Kachel;
- „user | auf nas-test.l…“: GTK verkleinert eine umbrochene Beschriftung
  auf die schmalste Breite mit gleicher Zeilenzahl
  (`NaturalWrapMode::Word`); mit gekürzter zweiter Zeile ist das jede Breite,
  also wurden Namen gekürzt oder Wörter zerteilt, obwohl sie gepasst hätten.

**Änderung** (`desktop_view.rs`):

| Stelle | Inhalt |
|---|---|
| `LabelFont::measure` | Pango-Kontext der Symbolfläche: ungefähre Zeichenbreite, Höhe zweier Zeilen (Schrift, Größe, Skalierung der Sitzung) |
| `cell_for(icon, font)` | Breite = max(QE-04-Breite, 14 Zeichen + 8 px Innenabstand), Höhe = max(QE-04-Höhe, Symbol + zwei Zeilen + 6 px Innenabstand + 6 px Lücke) |
| `update_font`, `watch_font` | beim Start und bei `gtk-font-name`/`gtk-xft-dpi` neu messen; Zelle neu, Symbole neu aufgebaut, gemerkte Plätze bleiben (das Raster legt sie auf die nächste Zelle) |
| `icon_widget` | `NaturalWrapMode::None`: die natürliche Breite ist der ungebrochene Text, eine lange Beschriftung nimmt die ganze Zelle |
| `show_selection` | ein ausgewähltes Symbol zeigt den ganzen Namen (wie Nautilus 2) und wird zuletzt gezeichnet; `log_oversized_tiles` nimmt es aus |

Werte: 11 pt → Zelle 116 × 96, 12 pt → 125 × 98 (Test
`cells_follow_the_label_font`). `layout.rs` ist unverändert: Laufwerke kommen
weiter per `place_transient`/`below` unter den Papierkorb, jetzt eine
ganze, passende Zelle tiefer.

**Wirkung auf bestehende Anordnungen:** Die Zustandsdatei `desktop-symbole`
wird nicht umgeschrieben (nur ihr Kommentarkopf verliert die interne
Kennung). Mit größeren Zellen hat die Fläche weniger Spalten und Zeilen
(1920 × 1016 bei 12 pt: 15 × 10 statt 19 × 11); die Symbole erscheinen auf der
nächsten Zelle ihres gemerkten Platzes.

## 2. Arbeitsflächen-Ordner nach dem Start (QD-112)

`watch` überwacht den XDG-Arbeitsflächen-Ordner, wenn es ihn gibt. Fehlt er,
überwacht der Desktop den nächsten vorhandenen Elternordner
(`icons::waiting_point`) und reagiert nur auf Einträge auf dem Weg zum
Ordner (`icons::concerns_desktop`). Erscheint der Ordner (anlegen,
hineinverschieben), richtet `rewatch_soon` die Überwachung neu ein und lädt
die Symbole nach; wird er gelöscht oder verschoben, wartet der Desktop wieder
im Elternordner. Log: „Arbeitsflächen-Ordner fehlt, warte in …“,
„Arbeitsflächen-Ordner da: …“, „… weg: …“. Eine Änderung von
`user-dirs.dirs` im Lauf wird weiterhin erst beim nächsten Start gelesen.

## 3. Kein Programm für eine Datei (QD-102, Folgepunkt)

`open_file` gibt `OpenFailure::NoProgram(<typ>)` zurück, wenn weder der Typ
der Datei noch der aus dem Namen geratene Typ ein Standardprogramm hat.
`no_program` zeigt dann „„<Name>“ kann nicht geöffnet werden.“ mit
„Für Dateien vom Typ „<Beschreibung>“ (<MIME-Typ>) ist kein Programm
installiert.“ und den Knöpfen „Schließen“ und „Mit Programm öffnen …“
(GTKs Programmauswahl wie „Andere Anwendung …“).

## 4. Englisch über gettext

Nach `~/projects/querencia-editor-wt-i18n/docs/i18n-muster.md`:

- Crate `gettext-rs` 0.8 mit `gettext-system`; Modul `src/i18n.rs`
  (Domäne `querencia-desktop`, `QUERENCIA_DESKTOP_LOCALEDIR`, Kataloge neben
  der Binary unter `share/locale`, dann `<präfix>/share/locale`, dann
  `/usr/share/locale`); `i18n::init()` als erste Zeile von `main`.
- msgid englisch (Wortschatz Nautilus 2.30), `po/de.po` mit dem bisherigen
  deutschen Wortlaut (Prüfung `deutsch-gleich.py` im Arbeitsordner: 103 von
  103 deutschen Nutzertexten aus main stehen im Katalog), `po/LINGUAS`,
  `po/querencia-desktop.pot`.
- Werkzeug `crates/querencia-desktop/werkzeug/i18n.py` (Kopie aus dem Editor,
  Domäne angepasst), im Crate-Ordner aufrufen: `pot`, `update`,
  `mo <ziel>`, `pruefen`. Gegenprobe `msgfmt -c po/de.po`.
- Übersetzt: Kontextmenüs (Fläche, Symbol, Papierkorb, Laufwerk), alle
  Dialoge (Papierkorb leeren, endgültig löschen, Umbenennen, Starter,
  Eigenschaften, Datenträger belegt, Fehler der Dateioperationen, kein
  Programm), Namen der Sondersymbole, neue Namen („Neuer Ordner“, „Neues
  Dokument“, „(Kopie)“), Größen- und Datumsformat der Eigenschaften.
- Nicht übersetzt: Logzeilen auf stderr, `--datentraeger-zeigen`,
  `desktop.ini`-Diagnosen, Namen der Laufwerke (kommen von GVfs, in dessen
  Sprache), Namen von Dateien und Startern.
- Test `german_catalog_is_complete` (jede msgid in `de.po`, keine leeren
  oder fuzzy Einträge); ohne Katalog liefern die Tests die englischen msgids.

Die Ablage braucht `share/locale/de/LC_MESSAGES/querencia-desktop.mo` neben
dem Programm; ohne sie ist die Oberfläche englisch. Das
Integrationsskript legt die Ablage deshalb nur vollständig an.

## 5. Prüfung

Isoliert (eigenes HOME, Bus, labwc) im Arbeitsordner
`~/projects/querencia-desktop-025` (`werkzeug/lauf.sh`): `lage` mit der
Netzlaufwerk-Attrappe `werkzeug/qattrappe.c` (GIO-Modul, Freigaben aus einer
überwachten Datei) in 1024 × 768 und 1920 × 1080 bei Skalierung 1 und 1,5,
`ordner`, `oeffnen`, `texte` je `LANGUAGE=de` und `en`, `fotos` hell und dunkel,
`tausch`. Ergebnisse und Fotos im Bericht .

## 6. Grenzen

- Die Zellbreite hängt an der durchschnittlichen Zeichenbreite: Wörter mit
  vielen breiten Buchstaben (m, w) über etwa 13 Zeichen werden weiter
  zeichenweise getrennt, wie unter GNOME 2.
- Ein ausgewählter langer Name überdeckt die Kachel darunter, solange er
  ausgewählt ist (wie Nautilus 2).
- Englische Datums- und Zahlenformate kommen aus dem Katalog bzw. aus
  `LANG`; die Größe „1.2 kB“ formatiert GLib nach `LC_NUMERIC`.

## 7. Querencia Files unter neuer Kennung (0.2.6, QD-059)

Files bekommt mit QF-0t die Kennung `org.querencia.Files`; der alte Starter
`io.github.lgse.Strata.desktop` bleibt eine Weile als unsichtbare Brücke.
`FILES_APPS` nennt beide, `files_app()` nimmt den ersten vorhandenen
(`first_found`, Test `files_is_found_by_its_new_id_then_by_the_old_one`).
Davon hängen ab: ob Rechner und Papierkorb erscheinen (`load_items`) und
womit Rechner, Papierkorb und Datenträger geöffnet werden (`open`). Beim
Start steht „Dateiverwaltung: <starter>“ im Log.

Weitere Stellen mit der alten Kennung außerhalb des Desktops (nicht
geändert): `crates/querencia-panel/src/places_menu.rs` (`QUERENCIA_FILES_ID`),
`crates/querencia-panel/src/window_icons.rs` und `apps/tests.rs`,
`session/querencia/applications/io.github.lgse.Strata.desktop`.
