# HMP-03: Klassische GNOME-2-Menüs (eigenes Menü-Widget)

Stand: 2026-10-15. Baut auf HMP-02 (`docs/hmp-02-gnome2-menues.md`) auf.

## Entscheidung

HMP-02 hatte Inhalt und Optik der drei Hauptmenüs an Ubuntu 10.04
angeglichen, aber die Darstellung blieb GTKs `PopoverMenu`. Das kann zwei
Kernmerkmale der GNOME-2-Menüs nicht (geprüft im Quelltext von GTK 4.22.4):

- keine Symbole neben Menüeinträgen mit Text (`gtkmodelbutton.c`);
- Untermenüs gleiten an Ort und Stelle ein („‹ Zubehör“) oder öffnen mit
  NESTED daneben über einen 80-ms-Zeitgeber, der schräge Mausbewegungen
  verliert (M1.1b-1d, `docs/m1-1b1d-nested-entscheidung.md`).

Der Besitzer hat ein eigenes Menü-Widget beschlossen (HMP-03). Die
`gio::Menu`-Modelle aus `app_menu.rs`, `places_menu.rs` und `system_menu.rs`
bleiben die einzige Datenquelle; ersetzt ist nur die Anzeige. Die
Entscheidung M1.1b-1e (keine Symbole) ist damit für die drei Hauptmenüs
überholt.

## Aufbau (`crates/querencia-panel/src/classic_menu.rs`)

- `attach(&MenuButton, &MenuModel)` gibt dem Panelknopf ein eigenes
  `Popover` (ohne Pfeil, linksbündig, Klasse `querencia-menu`). Die Zeilen
  werden bei jedem Öffnen aus dem Modell neu aufgebaut.
- Jede Zeile: 16-px-Symbol, Text, bei Untermenüs `▸` (`pan-end-symbolic`).
  Abschnitte des Modells werden zu Trennlinien, nur zwischen nicht leeren
  Abschnitten. Einträge ohne Aktion und leere Untermenüs sind ausgegraut.
  Fehlt ein Themensymbol, steht `application-x-executable` da, damit die
  Symbolspalte bündig bleibt.
- Ein Untermenü ist ein eigenes `Popover` an seiner Zeile: rechts daneben,
  oben bündig (`valign` Start, Versatz um Rand und Innenabstand), am
  Bildschirmrand klappt GTK es nach links.
- Maus: Untermenü öffnet nach 225 ms Verweilen (`gtk-menu-popup-delay` von
  GTK 2), beim Klick sofort. Bewegt sich der Zeiger von der Zeile eines
  offenen Untermenüs auf dieses zu (Dreieck zwischen letztem Zeigerpunkt und
  der nahen Kante des Untermenüs, `aiming`), übernehmen überquerte Zeilen
  höchstens 300 ms lang nicht.
- Tastatur: Pfeil hoch/runter und Tab (mit Umlauf), Pos1/Ende; Pfeil rechts,
  Enter oder Leertaste öffnen ein Untermenü und markieren dessen ersten
  Eintrag; Pfeil links schließt ein Untermenü; Escape schließt die Ebene
  (GTK-Popover); Enter oder Leertaste aktivieren einen Eintrag.
- Aktivieren schließt alle Menüebenen und löst die Aktion des Eintrags mit
  ihrem Ziel an der Zeile aus; die Aktionsgruppen (`apps`, `places`,
  `session`) hängen wie bisher am Panelknopf.

## Symbole

| Bereich | Symbolnamen (alle in der Kette gnome-human → gnome-colors-common → gnome) |
|---|---|
| Anwendungsgruppen | `applications-accessories`, `applications-science` (Bildung), `applications-games`, `applications-graphics`, `applications-internet`, `applications-office`, `applications-development`, `applications-multimedia`, `applications-system`, `preferences-desktop-accessibility`, `applications-other` |
| Anwendungen | `Icon=` der Desktop-Datei, sonst `application-x-executable` |
| Orte | `user-home`, `user-desktop`, `folder-documents`, `folder-music`, `folder-pictures`, `folder-videos`, `folder-download`, `computer`, `network-workgroup`, `user-trash` |
| System | `preferences-desktop`, `preferences-system`, `help-about`, `system-log-out` |

## Optik (Ubuntu 10.04, Thema Ambiance)

Eckiges Menü mit 1-px-Rahmen ohne Schatten, 22-px-Zeilen, 1-px-Trennlinien,
Hervorhebung in `#f07746` (selected_bg_color von Ambiance) mit weißer
Schrift. Der Knopf des offenen Hauptmenüs wird ebenso hervorgehoben. Alle
Regeln sind auf `.querencia-panel` und `popover.querencia-menu` begrenzt
(Test `menu_style_is_scoped_to_the_panel_and_its_menus`).

## Bewusst offen

- Kein Tastenkürzel zum Öffnen des Anwendungsmenüs (GNOME 2: Alt+F1); das
  bräuchte eine labwc-Tastenbindung in der Runtime-Konfiguration.
- ~~Kein Wechsel zwischen den drei Hauptmenüs …~~ – seit HMP-04 umgesetzt
  (`docs/hmp-04-menues-und-querencia-programme.md`).
- Keine Tastenkürzel-Buchstaben (Mnemonics) in den Einträgen.
- Kontextmenü und „Zum Panel hinzufügen“ sind unverändert.
