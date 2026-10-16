# HMP-06: Klassische GNOME-2-Fensterliste

Stand: 2026-10-16. Baut auf HMP-05 auf; Fensterliste weiter über
`zwlr_foreign_toplevel_manager_v1` (labwc 0.9.8: v3) auf der vorhandenen
eigenen Wayland-Verbindung von `window_list.rs`. Keine zweite
Fensterverwaltung, keine Verbindung pro Knopf.

## Aufbau eines Knopfs

`[16-px-Anwendungssymbol] [Titel…]`, Titel linksbündig, mit Ellipse gekürzt,
voller Titel im Tooltip (bei minimierten Fenstern mit „(minimiert)“).

**Symbol** (`window_icons.rs`): App-ID des Fensters → Desktop-Eintrag aus GIO
(derselbe Katalog wie das Anwendungsmenü) → dessen Symbol über das
GTK-Symbolthema (GNOME Human, vom Panel gesetzt). Zuordnung in dieser
Reihenfolge: Desktop-ID = App-ID, dasselbe ohne Groß-/Kleinschreibung,
`StartupWMClass`, Snap-Eintrag `<snap>_<app>.desktop`, letzter Teil einer
punktgetrennten Desktop-ID, Programmname aus `Exec`. Nie nach Titel. Kein
Treffer: `application-x-executable`. Zwischenspeicher je App-ID, geleert,
sobald sich die installierten Anwendungen ändern (`AppInfoMonitor`).

## Größe

Alle Knöpfe gleich breit (`button_width`): verfügbare Breite geteilt durch
die Fensterzahl, höchstens 200 px (Referenz: ~185 px auf 1366 px), 2 px
Abstand. Mit vielen Fenstern werden sie schmaler bis auf Symbol und
Ellipse, laufen aber nicht aus dem Panel. Eigene Layout-Klasse `WindowRow`
(CSS-Name `windowlist`). Höhe: Panelhöhe minus 2 px oben und unten, die
Panelhöhe bleibt.

## Zustände

Die eckigen Klammern im bisherigen Panel bedeuteten **minimiert** (wie in
libwnck/GNOME 2). Sie entfallen; der Zustand ist jetzt am Knopf zu sehen:

| Zustand | Darstellung |
|---|---|
| inaktiv | heller Verlauf `#fdfcfb`→`#ebe8e5`, Rahmen oben `#c6c1bb`, unten `#9e9993` (gemessen in der GNOME-2-Referenz) |
| aktiv | eingedrückt: dunklerer Verlauf `#dcd6d0`→`#ebe7e3`, Innenschatten |
| minimiert | flach `#f7f6f4`, Symbol und Titel auf 55 % gedämpft |
| Titeländerung | Titel und Tooltip folgen sofort (`done` des Protokolls) |

Keine orange Farbe, keine fette Schrift, dunkle Schrift `#3c3c3c`.

## Verhalten (wie GNOME 2)

Klick auf ein minimiertes Fenster: wiederherstellen und aktivieren; auf das
aktive Fenster: minimieren; auf ein anderes: aktivieren. Mehrere Fenster
einer Anwendung bleiben getrennte Knöpfe.

## Prüfung

- Unit-Tests: Symbolzuordnung (Querencia-Programme, Firefox-Snap, Groß-/
  Kleinschreibung, WM-Klasse, unbekannte und fehlende App-ID,
  Programmnamen aus `Exec`), Knopfbreite (1/2/5/10 Fenster, schmales
  Panel), Klickverhalten, Tooltip, Text ohne Klammern, CSS.
- Isoliert mit echten Fenstern (foot mit verschiedenen App-IDs) und echtem
  Zeiger: `tools/isolierte-gui-pruefung/pruefen-fensterliste.sh <binary>
  <profil>`, Pixelauswertung `auswerten-fensterliste.py` (Anzahl, gleiche
  Breite, höchstens 200 px, Symbol in jedem Knopf, aktiver und minimierte
  Knöpfe, kein Orange).
- Mutationsläufer `tools/mutation.py`.

## Offen

- Kein Kontextmenü am Fensterknopf (Minimieren/Maximieren/Schließen).
- Keine Gruppierung bei sehr vielen Fenstern (GNOME 2 konnte gruppieren).
- Kein Hinweis „Fenster verlangt Aufmerksamkeit“: das Protokoll meldet ihn
  nicht.
