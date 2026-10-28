# Querencia Oberfläche – gemeinsame GTK-Bausteine

Stand 23.10.2026. Zweig `oberflaeche`, Bibliothek `querencia-oberflaeche`
0.1.0 mit Panel 0.3.1 und Desktop 0.2.7 (Welle A von „Querencia Dateien“,
QD-140). Änderungen: `crates/querencia-oberflaeche/CHANGELOG.md`.

## 1. Zweck

Panel und Desktop teilten bis Panel 0.3.0 und Desktop 0.2.6 fünf Module als
Quelltext des Panels: Der Desktop band `classic_menu.rs`, `farbschema.rs`,
`icon_theme.rs` und `portal.rs` über `#[path]` ein, eine Weiche
`cfg(querencia_desktop)` (aus `build.rs` des Desktops) ließ in
`classic_menu.rs` den Symbol-Schutz QD-106 und den GUI-Test des Panels weg.
`symbole.rs` (QD-106) nutzte nur das Panel.

Jetzt stehen alle fünf in der Bibliothek `crates/querencia-oberflaeche`
(GPL-3.0-or-later aus `[workspace.package]`, Muster wie
`querencia-lautstaerke`/`querencia-netzwerk`). Panel, Desktop und der
kommende Dateimanager hängen über eine `path`-Abhängigkeit daran. Code und
Verhalten sind die von Panel 0.3.0; neu sind nur die Stellen in §3 und §4.

## 2. Aufbau

| Modul | Inhalt |
|---|---|
| `classic_menu` | GNOME-2-Menüs aus `gio::MenuModel` (HMP-03/04/05): Symbole, Kürzel, Untermenüs mit Zielkegel, GTK-2-Tastatur, Menüleiste |
| `classic_menu::gui_test` | isolierter GUI-Test der Menüs, nur mit Feature `gui-test` |
| `symbole` | Themensymbole ohne GTKs Vorladen im Thread-Pool (QD-106) |
| `icon_theme` | Symbolthema des Nutzers, sonst die Vorgabe der Sitzung (ab 0.1.4), sonst GNOME Human, Wechsel zur Laufzeit (G2-AD) |
| `farbschema` | dunkle Regeln der Programm-CSS nur bei `dunkel` (QD-081) |
| `portal` | Settings-Portal beim Start, GSettings als Rückfall (QD-062) |
| `i18n` | Textdomäne `querencia-oberflaeche` |

Abhängigkeiten: nur `gtk4 0.11` (`v4_10`) und `gettext-rs 0.8`
(`gettext-system`), dieselben Versionen wie im Panel; `Cargo.lock` bekommt
nur den Eintrag des neuen Crates.

## 3. Was sich an den Nahtstellen ändert

- **Symbolweg statt `cfg`:** `classic_menu::set_icon_loading(IconLoading)`.
  Standard ist `IconLoading::WithoutPreload` (über `symbole`, QD-106).
  Der Desktop wählt beim Start ausdrücklich `IconLoading::Gtk`, damit sein
  Menü wie bis 0.2.6 lädt. Ein Wechsel des Desktops auf den Standard ist
  eine Verhaltensänderung und gehört in Desktop 0.3.0.
- **GUI-Test:** `classic_menu::gui_test::isolated_gui_menus(&Panel)` mit
  Stil, drei Hauptmenüknöpfen und einem Menüknopf-Bauer des Programms. Das
  Panel ruft ihn aus `src/menu_gui_test.rs` auf (ignoriert, nur isoliert),
  über `[dev-dependencies]` mit Feature `gui-test`; der Release-Bau enthält
  keinen Testcode. Der Test läuft jetzt auch unter den Laufzeitordnern der
  Panel-Prüfung ab 0.2.4 (`qp24-runtime-`).
- **Einbindung:** `use querencia_oberflaeche::{classic_menu, …};` in
  `main.rs` von Panel und Desktop; die Pfade `crate::symbole::…` und
  `crate::portal::…` im übrigen Code gelten damit unverändert.

## 4. Übersetzungen

Entscheidung: **eigene Textdomäne `querencia-oberflaeche`** mit `po/`,
`werkzeug/i18n.py` und Tests (`german_catalog_is_complete`,
`german_wording_is_the_old_one`), wie Lautstärke und Netzwerk.

Verworfen wurde ein Übersetzer, den das Programm setzt. Die Bausteine haben
heute nur fünf Texte (Tastennamen `Strg`, `Umschalt`, `Entf`, `Eingabe`,
`Druck` in `classic_menu::accel_label`, Kontext `keyboard key`). Ein
gesetzter Übersetzer hätte diese Texte im Katalog jedes Programms verlangt:
im Panel, im Desktop und im Dateimanager je dieselben fünf Einträge.
Fehlt einer, erscheint das Englische, und kein Test im Crate merkt es. Mit
eigener Domäne liegt der Wortlaut einmal vor und wird im Crate geprüft. Der
Preis ist eine `.mo` mehr in jeder Ablage. Das ist dasselbe Muster, das
Panel 0.3.0 schon für zwei Bibliotheken nutzt.

- Übersetzt wird mit `dgettext` über die eigene Domäne
  (`i18n::pgettext(ctxt, msgid)`), unabhängig von der Hauptdomäne.
- Jedes Programm bindet die Domäne an sein Katalogverzeichnis:
  `querencia_oberflaeche::i18n::bind(dir)` in seinem `i18n::init`. Es
  liefert `share/locale/<lang>/LC_MESSAGES/querencia-oberflaeche.mo` neben
  der Binary mit (Panel und Desktop ab 0.3.1/0.2.7).
- Der deutsche Wortlaut ist der des Panels 0.3.0; die fünf Einträge fallen
  aus `crates/querencia-panel/po/` weg.
- **Ohne die `.mo` in der Ablage zeigt das Panel „Ctrl+Alt+L“** statt
  „Strg+Alt+L“ neben „Bildschirm sperren“. Die Integrationsskripte prüfen
  deshalb alle Kataloge der Ablage.

Pflege wie in `docs/panel-030-englisch.md` §3, im Crate-Ordner:

```bash
python3 werkzeug/i18n.py pot       # po/querencia-oberflaeche.pot
python3 werkzeug/i18n.py update    # po/de.po nachziehen
python3 werkzeug/i18n.py pruefen
python3 werkzeug/i18n.py mo <ziel> # <ziel>/de/LC_MESSAGES/querencia-oberflaeche.mo
```

## 5. Nutzung in einem neuen Programm

Reihenfolge beim Start, wie Panel und Desktop:

```rust
use querencia_oberflaeche::{classic_menu, farbschema, icon_theme, portal};

i18n::init(); // eigene Domäne binden, dazu querencia_oberflaeche::i18n::bind(dir)
let mode = portal::prepare("querencia-dateien");          // vor gtk4::init
gtk4::init()?;
portal::follow_gsettings("querencia-dateien", mode);
// in activate:
icon_theme::follow("querencia-dateien");
let css = gtk4::CssProvider::new();
farbschema::folgen(&css, "querencia-dateien");            // vor load_from_data
css.load_from_data(STIL);                                 // eigene Menü-CSS
farbschema::startfehler_beheben();
```

- **Menüleiste:** je Menü ein `gtk4::MenuButton` mit Beschriftung in einer
  waagerechten Box des Fensters, `classic_menu::attach(&knopf, &modell)`.
  Alle mit `attach` angehängten Knöpfe desselben Fensters bilden eine
  Menüleiste in Widget-Reihenfolge: Überfahren eines Knopfs wechselt,
  solange eins offen ist (der Zeiger zählt nur über der Knopffläche, x und
  y, seit 0.1.1); Links und Rechts wechseln, mit Umlauf. Einträge mit `action`/`target` lösen die
  Aktion am Knopf aus, also auch `win.`- und `app.`-Aktionen. Ein Attribut
  `accel` (GTK-Schreibweise) zeigt das Kürzel rechts an; binden muss das
  Programm die Taste selbst.
- **Kontextmenü:** ein leerer, nicht anklickbarer Anker-`MenuButton` mit
  `attach_popup` (seit 0.1.1); beim Klick wird sein Menü auf die
  Klickstelle gesetzt und geöffnet (so macht es der Desktop,
  `desktop_view.rs`, `popup_menu`). Nie `attach`: dann gehört der Anker zur
  Menüleiste des Fensters, und das offene Kontextmenü wechselt per Zeiger
  oder Links/Rechts in ein Menü der Leiste (QD-176, Dateien 0.2.1).
- **Symbole:** `symbole::image(&icon, größe)`, `symbole::image_named`,
  `symbole::set(&image, &icon)`, `symbole::set_named`. Pflicht für jedes
  Bild mit Themensymbol in Listen und Rastern. `Image::from_gicon`,
  `set_icon_name` und `set_from_gicon` laden über GTKs Vorladen im
  Thread-Pool und können GTK 4.22 zum Absturz bringen (QD-106,
  `docs/panel-024-absturzursache-und-kennung.md`).
- **Thema:** `icon_theme::follow`, `farbschema::folgen`/`dunkel`;
  eigene Farben nur dunkel, wenn `farbschema::dunkel` gilt.
- **Portal für Kinder:** `portal::for_children(&kontext)` bzw.
  `portal::launcher_for_children(&launcher)` vor jedem Programmstart.

Die Menü-CSS liegt heute im Programm. Panel und Desktop haben dieselben
Regeln je als eigene Kopie (`popover.querencia-menu …`); ein neues Programm
übernimmt sie.

## 6. Grenzen

- Die Debug-Zeilen der Menüs (`QUERENCIA_MENU_DEBUG=1`) beginnen auch im
  Desktop mit `querencia-panel: menü` (wie bisher).
- Keine Zugriffstasten in der Menüleiste (Alt+D, F10). GNOME 2 hatte sie;
  für den Dateimanager kommen sie als Erweiterung dazu.
- Die Menü-CSS ist nicht Teil der Bibliothek (§5).

## 7. Prüfung

Siehe : Testnamen-Abgleich vorher/nachher
(die 27 geteilten Tests laufen jetzt einmal im Crate statt in Panel und
Desktop), GUI-Regression von Panel und Desktop, Fotos hell/dunkel,
Symbol-Belastung QD-106, isolierte GUI-Tests der Menüs und der Symbole.

## 8. Integration

Maßgeblich sind die Arbeitsordner
`~/projects/oberflaeche-arbeit/integration-panel-031/` und
`~/projects/oberflaeche-arbeit/integration-desktop-027/`; im Repo liegen
Kopien ohne Binaries und Kataloge (`integration/panel-031/`,
`integration/desktop-027/`). Panel und Desktop sind unabhängig: jedes
Programm hat seine eigene Ablage mit seiner Kopie von
`querencia-oberflaeche.mo`; die Reihenfolge ist beliebig.

```bash
P=~/projects/oberflaeche-arbeit/integration-panel-031/panel-031-integrieren.sh
bash $P pruefen                                 # P=alt (0.3.0-2 b6f9b2fa)
PANEL_031_FREIGABE=ja bash $P aktivieren        # Ablage mit vier Katalogen + Link
PANEL_031_FREIGABE=ja bash $P panel-neustart    # eigener Schritt, Wachhund bleibt
PANEL_031_FREIGABE=ja bash $P zurueck           # Link → 0.3.0-2
PANEL_031_FREIGABE=ja bash $P panel-neustart-zurueck
D=~/projects/oberflaeche-arbeit/integration-desktop-027/desktop-027-integrieren.sh
bash $D pruefen                                 # D=alt (0.2.6-2 a9e36001)
DESKTOP_027_FREIGABE=ja bash $D aktivieren      # Ablage mit zwei Katalogen + Link
DESKTOP_027_FREIGABE=ja bash $D desktop-neustart
DESKTOP_027_FREIGABE=ja bash $D zurueck         # Link → 0.2.6-2
DESKTOP_027_FREIGABE=ja bash $D desktop-neustart-zurueck
```

**Integrationsstand:** `panel/0.3.1-2-int` und `desktop/0.2.7-2-int`.
Während der Prüfung rückte main auf `5a5b65c` (Dateiarbeit 0.1.1,
Prüfrahmen 0.1.1). Der Merge `0f4b6ef` (Tags `panel/0.3.1-2`,
`desktop/0.2.7-2`) baut Programme und Kataloge byte-gleich mit
`87ceaab` (`panel/0.3.1`, `desktop/0.2.7`), aus dem die Vorlagen stammen
(`QUELLE_COMMIT=87ceaab`). Die Tags `panel/0.3.1-int` und
`desktop/0.2.7-int` bleiben stehen, sie werden nicht aktiviert.

Die Tausch-Skripte (`tausch-panel-3.py`, `tausch-desktop.py`) sind
byte-gleich mit denen von Panel 0.3.0-2 und Desktop 0.2.6-2. Das
Desktop-Skript liest die Prozessliste jetzt ohne `grep -q` hinter einer
Pipe (wie das Panel-Skript seit 0.2.5).
