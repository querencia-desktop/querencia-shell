# HMP-04: Menükorrekturen und Querencia-Programme im Menü

Stand: 2026-10-16. Baut auf HMP-03 (`docs/hmp-03-klassische-menues.md`) auf;
die Menüarchitektur (`classic_menu.rs`) bleibt.

## 1. Schriftfarbe in Untermenüs

**Fehlerbild:** Nicht hervorgehobene Einträge seitlicher Untermenüs (z. B.
Anwendungen › Zubehör) waren weiß auf weißem Grund.

**Ursache:** GTK gibt jedem `GtkPopover` die CSS-Klasse `background`
(`gtkpopover.c`, `gtk_popover_init`); über sie setzt das Thema die
Schriftfarbe neu (`.background { color: #2e3436; … }` im GTK-Standardthema).
`classic_menu.rs` baute die Popover mit `.css_classes([MENU_CLASS])`, und
diese Eigenschaft *ersetzt* alle Klassen. Ohne `background` erbt ein Popover
die Farbe seines CSS-Elternknotens. Ein Untermenü hängt an seiner Zeile, und
die ist beim Öffnen hervorgehoben (`color: #ffffff`): Weiß auf Weiß. Das
Hauptmenü hängt am Menüknopf und war deshalb lesbar.

**Korrektur:**
- `menu_popover()` fügt `querencia-menu` nur hinzu (`add_css_class`); die
  GTK-Klassen bleiben. Ebenso behält der Menüknopf seine GTK-Klasse `popup`.
- Das Menü-CSS setzt die Farben selbst, statt sie zu erben: normale Zeilen
  `@theme_fg_color` auf `@theme_bg_color`, Hervorhebung `#ffffff` auf
  `#f07746`, nicht wählbare Zeilen `@insensitive_fg_color`. Alle Regeln
  bleiben auf `popover.querencia-menu` bzw. `.querencia-panel` begrenzt.

Beide Maßnahmen beheben den Fehler je für sich; zusammen sichern sie ihn
gegen ein anderes Thema oder einen späteren Umbau ab.

**Prüfung:** statischer Test `menu_text_colours_are_set_not_inherited`
(`main.rs`) und der isolierte GUI-Test (Abschnitt 5), der die von GTK
berechneten Farben misst (Kontrast normal ≥ 7:1, Hervorhebung weiß,
deaktiviert grau) und die GTK-Klassen prüft. Mit wieder eingebautem
HMP-03-Fehler misst er `#ffffff` mit Kontrast 1,1 und schlägt fehl.

## 2. Hauptmenüs als Menüleiste

Wie in GNOME 2 verhalten sich Anwendungen, Orte und System eines Panels wie
eine Menüleiste:
- Ist eines offen, öffnet Überfahren eines anderen Hauptmenüknopfs dieses
  und schließt das bisherige (`switch_on_hover`). Ohne offenes Menü
  öffnet Überfahren nichts.
- Pfeil rechts auf einem Eintrag ohne Untermenü und Pfeil links in der
  obersten Ebene wechseln zum nächsten bzw. vorigen Hauptmenü, mit Umlauf
  (`neighbour`), der erste Eintrag ist dann markiert.

Die Hauptmenüs eines Panels findet `classic_menu.rs` über ein
Thread-lokales Register (schwache Referenzen) und dieselbe Wurzel (das
Panelfenster), sortiert in Widget-Reihenfolge. Keine Änderung an
`panel.rs`, Applets oder Konfiguration.

## 3. Querencia-Programme

Desktop-Dateien in `session/querencia/applications/`; Desktop-ID = App-ID,
damit labwc und die Fensterliste die Fenster zuordnen:

| Datei | Name | Symbol | Programm |
|---|---|---|---|
| `io.github.lgse.Strata.desktop` | Querencia Files | `folder` | `~/projects/querencia-files-dev/target/debug/strata %U` |
| `org.example.Querencia.Terminal.desktop` | Querencia Terminal | `utilities-terminal` | isolierter Neubau, `…/hmp-04/apps/querencia-terminal/target/debug/querencia-terminal` |
| `org.querencia.Screenshot.desktop` | Bildschirmfoto aufnehmen | `applets-screenshooter` | `~/projects/querencia-screenshot-dev/target/debug/querencia-screenshot` |

**Nachtrag 24.10.2026:** Die beiden Files-Starter (`io.github.lgse.Strata.desktop`
und das spätere `org.querencia.Files.desktop`) sind mit `session/grafik-3`
entfernt; Querencia Files ist ausgemustert, Querencia Dateien bringt
seinen eigenen Starter mit (`docs/sitzung-grafik.md`, Nachtrag).

Alle mit `Categories=GTK;Utility;…` ohne `System`, also in **Zubehör** (wie
gnome-terminal und gnome-screenshot in Ubuntu 10.04). Die Pfade sind
absolute Entwicklungspfade dieser VM, wie `session/querencia.desktop`.

Das Panel braucht dafür keinen fest verdrahteten Eintrag: Der Katalog
liest die Dateien über GIO, `AppInfoMonitor` baut die Menüs bei
Registrierung neu auf.

## 4. Orte und Ordnerprogramm

`Querencia Files` erklärt `MimeType=inode/directory`. GIO übernimmt das
aber erst über den `mimeinfo.cache` des Anwendungsverzeichnisses
(`update-desktop-database <dir>`); dann ist es ohne jeden Eintrag in
`mimeapps.list` GIOs Wahl für Ordner, solange der Benutzer kein anderes
Programm festgelegt hat. Für `computer:`, `network:` und `trash:` gibt es
keine eigenen Handler; GIO nimmt dafür den Inhaltstyp `inode/directory`.

**Ersatz ohne GIO-Zuordnung** (`directory_handler`): Kennt GIO kein
Ordnerprogramm (z. B. Desktop-Datei ohne aktualisierten Cache), nimmt das
Panel ein installiertes Programm, dessen Desktop-Datei selbst
`inode/directory` erklärt, Querencia Files zuerst, und übergibt ihm die URI
direkt (`launch_uris`). GIOs Wahl, also auch eine Vorgabe des Benutzers,
wird nie ersetzt. Echte MIME-Standardwerte werden nicht geschrieben.

Real geprüft (isoliert): `file://~`, `computer:///`,
`network:///`, `trash:///` öffnen in Querencia Files.

## 5. Isolierte GUI-Prüfung

`classic_menu/gui_test.rs`, Test `isolated_gui_menus` (`#[ignore]`), läuft
nur mit `QUERENCIA_ISOLATED_GUI=1` in einem Laufzeitverzeichnis
`…/hmp04-runtime-*`, also nur über das Werkzeug
`hmp-04/gui-test/isoliert.py` (eigener headless labwc 0.9.8, eigener D-Bus,
eigenes XDG-Profil; beendet nur die eigene Prozessgruppe).

Der Compositor hat keine Eingabegeräte: Zeiger und Tasten werden über
dieselben Methoden eingespeist, die die Ereignis-Controller aufrufen, und
Popover greifen nicht (`NO_GRAB`, nur im Test), weil ein Popup-Grab eine
Eingabe-Seriennummer braucht. Das Schließen per Klick daneben und echte
Zeigerbewegung prüft deshalb erst die GUI-Abnahme.

*Seit HMP-05:* `NO_GRAB` entfällt (Untermenüs greifen nie, nur die oberste
Ebene; im In-Prozess-Test greift auch sie nicht). Echter Zeiger und echte
Tastatur samt Grab: `tools/isolierte-gui-pruefung/`, siehe
`docs/hmp-05-menue-maus-und-optik.md`.
