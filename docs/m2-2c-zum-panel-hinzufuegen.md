# M2.2c (HMP-01) – „Zum Panel hinzufügen“ funktionsfähig

**Stand:** 2026-10-15, Basis `671297d` (M2.2b), entwickelt im isolierten
Bereich `~/projects/querencia-panel-werkstatt/arbeit`. Nicht committet,
nicht in die Runtime übernommen, noch keine GUI-Abnahme.

## Funktion

- Rechtsklick auf freie Panelfläche → „Zum Panel hinzufügen …“ öffnet den
  Dialog aus M2.2b. Die Klickposition bestimmt den Zielbereich des Panels:
  linkes Drittel → Bereich `Start`, mittleres → `Center`, rechtes → `End`
  (`Region::at`). Neue Applets kommen neben die freie Fläche: in `Start` und
  `Center` ans Ende, in `End` an den Anfang (`Region::insertion_index`).
- Im Dialog: Typ auswählen, dann „Hinzufügen“, Doppelklick oder Enter. Der
  Dialog bleibt für weitere Objekte offen (wie GNOME 2). Ein Einfachklick
  wählt nur aus.
- Typen, die nur einmal erlaubt und schon vorhanden sind (Fensterliste,
  Arbeitsflächenumschalter), erscheinen ausgegraut mit Tooltip. Der Stand
  wird nach jedem Hinzufügen und bei jeder Aktivierung des Dialogs neu
  geprüft (der Dialog des anderen Panels kann ihn verändert haben).
- Schlägt das Hinzufügen fehl, steht der Grund über den Knöpfen; Modell,
  Datei und Panel bleiben unverändert.

## Ablauf einer Änderung

1. `Layout::add_applet` (`layout.rs`) fügt auf einer Kopie des Layouts eine
   neue Instanz `<typ>-<n>` ein (kleinstes freies `n`), mit denselben Regeln
   wie `Layout::validated` (unbekannter Typ, unbekanntes Panel,
   Einzelinstanz).
2. `Editor::add` (`layout_editor.rs`) prüft die Kopie erneut mit
   `validated` und speichert sie mit `layout_store::save_checked`. Erst
   danach wird sie das aktuelle Layout. Was nicht gespeichert werden kann,
   wird nicht übernommen.
3. `panel::insert_applet` setzt ein neues Widget an dieselbe Stelle der
   laufenden Leiste. Vorhandene Applets werden nicht neu erzeugt.

## Schutz von `panel.ini`

- Gespeichert wird nur auf Benutzeraktion, atomar wie in M2.2a
  (`g_file_set_contents_full`, `CONSISTENT | DURABLE`).
- `save_checked` vergleicht die Datei vor dem Schreiben byte-genau mit dem
  Stand beim Laden bzw. letzten Speichern. Hat jemand anderes sie
  inzwischen geändert oder angelegt, wird nichts geschrieben. Eine beim
  Laden unlesbare Datei wird nie ersetzt.
- Hat die Datei mehr enthalten, als das Layout wiedergibt (verworfene
  Einträge, unbrauchbare Datei, Kommentare oder unbekannte Schlüssel), wird
  vor dem ersten Speichern eine Kopie `panel.ini.vorher-JJJJMMTT-HHMMSS`
  (Modus 0600, nie überschrieben) daneben angelegt.

## Tests

Neue Unit-Tests ohne Display in `layout.rs`, `layout_store.rs`,
`layout_editor.rs`, `add_dialog.rs` und `applets.rs`. Der GTK-Teil (Dialog,
Einfügen in die Leiste) braucht die GUI-Abnahme.
