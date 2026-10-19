# G2-R: Papierkorb voll/leer, Tastenkürzel-Übersicht, Alt+Tab-Optik

Stand 18.10.2026, Zweig `kleinigkeiten` (Basis `main` 127b74e). Nichts davon
ist live. Bericht mit allen Zahlen: ,
Bilder: .

## 1. Desktop QE-02: Papierkorb-Symbol (Tag `desktop/qe-02`)

- Seit HMP-10b zeigte der Desktop `user-trash` bzw. `user-trash-full`, baute dafür
  aber bei jedem Ereignis des Papierkorb-Monitors **alle** Symbole neu auf.
- QE-02: Der GIO-Monitor auf `trash:///` tauscht nur noch das Bild des
  Papierkorb-Symbols (`show_trash_state`). Ereignisse werden 120 ms gebündelt;
  es gibt keinen Zeitgeber, der den Papierkorb abfragt.
- Nach „Papierkorb leeren“ wechselt das Symbol sofort (ohne auf den Monitor zu
  warten). Öffnet man das Kontextmenü, gleicht dieselbe Zählung Symbol und
  Menüpunkt ab; „Papierkorb leeren…“ bleibt bei leerem Korb ausgegraut.
- Symbole: die des Themas. Der Desktop wählt das klassische Symbolthema
  (Humanity), dessen leerer und voller Papierkorb zu Rechner und Persönlichem
  Ordner passen; eigene Querencia-Varianten sind deshalb nicht nötig.
- Prüfhilfe: Mit `QUERENCIA_DESKTOP_DEBUG=1` protokolliert jede Zählung
  („Papierkorb gezählt: …“) und jeder Wechsel („Papierkorb jetzt voll/leer“).
- Prüffall `tools/isolierte-gui-pruefung/qe02-treiber.py` (29 Prüfungen, nur der
  isolierte Papierkorb des Testprofils); Aufruf
  `QE02_BIN=<binary> bash nachweis/regression.sh qe02`.

## 2. querencia-kuerzel 0.1.0 (Tag `kuerzel/0.1.0`)

- GTK4-Fenster „Tastenkürzel“ (App-ID `org.querencia.Kuerzel`), zwei Seiten:
  „Arbeitsfläche“ (labwc) und „Programme“.
- rc.xml der laufenden Sitzung: `LABWC_PID` → `/proc/<pid>/cmdline` → `-C <ordner>`
  bzw. `-c <datei>`; ohne diese Angaben der XDG-Suchpfad wie bei labwc. Eigener
  kleiner XML-Leser (`xml.rs`), keine neue Abhängigkeit.
- labwc-0.9.8-Regeln: `<default />` setzt die Standardbelegung ein; eine spätere
  Belegung derselben Tasten ersetzt die frühere; `None` entfernt sie.
- Gruppen: Fenster, Arbeitsflächen, Bildschirmfoto, Sitzung, Weitere. Tastennamen
  deutsch (Strg, Umschalt, Entf, Druck, Bild↑ …); `W-S-question` erscheint als „Super+?“.
- Programmkürzel: `crates/querencia-kuerzel/data/programme.txt` (eingebaut; eine
  Datei gleichen Namens unter `$XDG_DATA_HOME/querencia-kuerzel/` ersetzt sie).
- `querencia-kuerzel --liste` gibt alles als Text aus (ohne Bildschirm).
- Symbol `data/org.querencia.Kuerzel.svg` (nur SVG), Desktop-Datei
  `Name=Tastenkürzel`, `Categories=GTK;Utility;`.

## 3. Alt+Tab (Theme `theme/querencia-klassisch-osd`, rc.xml-Vorschlag)

- Theme: `osd.*` am Ende von `data/themes/Querencia-Klassisch/labwc/themerc`
  (heller Kasten, grauer 1-px-Rahmen, orange Auswahl, Miniatur-Werte).
- rc.xml: `<windowSwitcher>` Stil classic, Felder Symbol/Titel/Arbeitsfläche,
  Anzeige auf dem fokussierten Bildschirm; `<icon>gnome-human</icon>` (labwc 0.9.8
  ist ohne librsvg gebaut und zeigt nur PNG-Symbole); OSD-Schrift Adwaita Sans 11;
  Super+? → `~/opt/querencia/bin/querencia-kuerzel`.
- Belegbilder: `werkzeug/osd/osd-bilder.sh <variante>` (isolierter headless labwc,
  drei Testfenster).

## Integration (vorbereitet, nicht ausgeführt)

Unter `integration/`: `kuerzel/kuerzel-integrieren.sh`, `theme/theme-osd-integrieren.sh`
(je mit `probe.sh`), `rc.xml-alt-tab.patch`, `rc.xml-super-fragezeichen.patch`,
`rc.xml-g2r-gesamt.patch`, `themerc-osd.patch`. Reihenfolge und Rückfall stehen
in .
