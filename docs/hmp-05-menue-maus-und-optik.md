# HMP-05: Hängende Menüs und GNOME-2-Optik

Stand: 2026-10-16. Baut auf HMP-04 (`docs/hmp-04-menues-und-querencia-programme.md`)
auf; die Menüarchitektur (`classic_menu.rs`) bleibt.

## 1. Hängendes Menü

**Fehlerbild (Besitzer):** Anwendungen öffnen, auf „Systemwerkzeuge“ verweilen,
bis das Untermenü offen ist, dann auf „Zubehör“ oder „Internet“: Die alte
Zeile bleibt orange, ihr Untermenü offen, die neue Kategorie reagiert nicht.

**Reproduziert** mit echtem (virtuellem) Zeiger im isolierten labwc
(`tools/isolierte-gui-pruefung`), mit dem unveränderten HMP-04-Binary: nach
0,5 / 2 / 5 / 10 s Verweilen bleibt „Systemwerkzeuge“ markiert und offen. Ein
Wechsel *vor* Ablauf der 225 ms (Untermenü noch zu) funktioniert. Die Dauer
ist also nicht die Ursache; entscheidend ist, ob das Untermenü schon offen ist.

**Ursache:** Jedes Untermenü war ein *greifender* Popover (`autohide`).
GTK 4 ruft für einen greifenden Popover `gtk_grab_add()` auf
(`gtkpopover.c`), und `gtk_main_do_event()` (`gtkmain.c`) gibt dann jedes
Zeigerereignis, dessen Ziel nicht im greifenden Widget liegt, an dieses
weiter. Das Menü darüber liegt nicht im Untermenü: Alle Bewegungen über dem
Hauptmenü landeten beim Untermenü, mit Koordinaten der falschen Oberfläche.
Das Hauptmenü bekam keine Bewegung mehr, also blieb die alte Zeile aktiv und
ihr Untermenü offen. Die Zeitgeber (225 ms Öffnen, 300 ms Zielsperre) waren
nicht schuld; sie liefen korrekt ab.

Mitbetroffen: Die Tastatur. Der Tastaturfokus wanderte ins greifende
Untermenü bzw. lag nach einem Klick auf keiner Zeile; Escape und Pfeiltasten
kamen dann nicht an (im HMP-04-Binary nachgewiesen).

**Korrektur** (wie die Menüleiste von GTK 2: nur die oberste Ebene greift):
- Untermenüs greifen nie (`set_autohide(false)`). Nur der Popover des
  Hauptmenüs greift; Untermenüs hängen als Kinder in seinem Widget-Baum, also
  innerhalb des Grabs, und bekommen ihre Ereignisse normal.
- Klick außerhalb: der Grab der obersten Ebene schließt das Menü (xdg_popup
  `popup_done`), `connect_closed` schließt alle Untermenüs.
- Menüleiste: Über dem Panel umgeleitete Bewegungen landen beim greifenden
  Hauptmenü; ein Bewegungs-Controller dort wertet die Position im Panel aus
  (`hover_panel`, `span_at`) und wechselt zu Orte/System.
- Tastatur: ein Tasten-Controller am (langlebigen) Hauptmenü-Popover, der
  die Taste an die innerste Ebene mit markierter Zeile gibt
  (`keyboard_level`); Untermenüs leiten ebenfalls dorthin weiter. Der
  GTK-Tastaturfokus bleibt in der obersten Ebene.
- Escape schließt erst das innerste Untermenü, dann das Menü (GNOME 2).
- Tastatursperre (`key_hold`): Nach einer Taste wiederholt der Compositor
  beim Verschwinden eines Untermenüs die Zeigerposition. Diese
  Wiederholung ist keine Bewegung und darf z. B. ein mit Escape
  geschlossenes Untermenü nicht wieder öffnen. Erst eine echte Bewegung
  (mehr als 3 px) gibt den Zeiger wieder frei.
- Zeitgeber: unverändert 225 ms und 300 ms; jeder Zeilenwechsel bricht die
  ausstehenden ab, und der Öffnungszeitgeber prüft beim Ablauf zusätzlich
  die aktuelle Zeile.

**Protokoll für die Abnahme:** `QUERENCIA_MENU_DEBUG=1` schreibt jeden
Zustandswechsel mit Uhrzeit nach stderr (Zeile, offenes Untermenü,
Zielsperre, Tasten, Hauptmenüwechsel), z. B.
`querencia-panel: menü 01:00:19.735: Ebene 0: Untermenü Zubehör geöffnet (Tastatur): [aktiv=Zubehör offen=Zubehör] > [aktiv=- offen=-]`.

## 2. Optik

Ambiance-GTK2-Daten liegen auf der VM nicht vor (`/usr/share/themes` hat
nur Default/Emacs). Referenz ist ein Bildschirmfoto eines GNOME-2-Menüs mit
Ambiance (`~/MATE-classic-gnome2-ubuntu.jpg`), Farben mit PIL gemessen:

| Element | gemessen | CSS |
|---|---|---|
| Menügrund | `#f5f4f2` | `#f5f4f2` |
| Text | `#41403e` (JPEG) | `#3c3c3c` (Ambiance `text_color`) |
| Hervorhebung Verlauf | `#f8875d` → `#ea703d`, Mitte `#f07846` | `linear-gradient(to bottom, #f8875d, #ea703d)` |
| Hervorhebung Oberkante | `#fd9975` (1 px) | `box-shadow: inset 0 1px #fd9975` |
| Hervorhebung Rahmen | `#b36c4e` (1 px) | `border-color: #b36c4e` |
| Untermenüpfeil | `#6f6b6a` (JPEG) | `#686463` |
| Menürahmen | `#c3c2c0` | `#c7c2be` |
| offener Hauptmenüknopf | Grund `#ede6e0`, Rand `#c4bdb7` | `#ebe7e3`, `#c4bdb7` |

HMP-03/04 hatte die Hervorhebung als flache Fläche in `#f07746` (Ambiance
`selected_bg_color`, die Mitte des Verlaufs). Sie wirkte dadurch kräftiger
als das Original.

**Hauptmenüknöpfe** Anwendungen | Orte | System: nie orange. Dunkle Schrift
`#3c3c3c`, ohne Rahmen; beim Überfahren und bei offenem Menü nur eine
etwas dunklere Panelfläche mit 1 px Rand wie im Original, kein blauer
GTK-Fokusring (`outline: none`).

Weiter gilt (HMP-04): Farben werden gesetzt, nicht geerbt; nicht wählbare
Zeilen `@insensitive_fg_color`; alle Regeln nur für `.querencia-panel` und
`popover.querencia-menu`.

## 3. Prüfung

- Unit-Tests: `span_at` (Hauptmenü unter dem Zeiger), `hold_step`
  (Tastatursperre), CSS-Tests für Menüfarben und neutrale Hauptmenüknöpfe.
- In-Prozess-GUI-Test `classic_menu/gui_test.rs` (isoliert): kein Untermenü
  greift, Fokus bleibt oben, veralteter Öffnungszeitgeber öffnet nichts,
  Wechsel nach 0,5 s und 2 s Verweilen schließt das alte Untermenü.
- Echter Zeiger und echte Tastatur: `tools/isolierte-gui-pruefung/pruefen.sh
  <panel-binary> <profil>`. Eigener headless labwc, eigener D-Bus, virtuelle
  Geräte über `qeingabe` (verweigert den Start außerhalb der Isolierung).
  37 Bildschirmfotos, `auswerten.py` prüft Pixel gegen
  `erwartung-hmp05.json`: offenes Menü, markierte Zeile, offenes Untermenü,
  markierte Untermenüzeile, dunkle Schrift, kein Orange und kein Fokusring auf
  den Knöpfen.
