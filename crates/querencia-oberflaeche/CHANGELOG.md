# Änderungen – querencia-oberflaeche

## 0.1.4 – 2026-10-28 (Symbolthema nach der Vorgabe der Sitzung)

- `icon_theme`: Ohne Nutzerwert von `org.gnome.desktop.interface
  icon-theme` zählt jetzt der wirksame Wert des Schlüssels, also die
  Schema-Vorgabe samt Override (z. B. `[org.gnome.desktop.interface:X-Querencia]
  icon-theme='Querencia'` aus dem Sitzungspaket). Er wird genommen, wenn das
  Thema installiert und keine GTK-Grundeinstellung („“, „Adwaita“,
  „hicolor“) ist; sonst wie bisher GNOME Human. Ein Nutzerwert geht weiter
  vor, ein Nutzerwert ohne installiertes Thema führt weiter zu GNOME Human.
  Ohne Schema unverändert. Befund: In einer frischen Installation (Alpha 3.5)
  zeigten Panel, Desktop und Dateien GNOME Human statt Querencia, obwohl die
  Einstellungen „Querencia“ anzeigten.
- `UserChoice::Default` trägt die Vorgabe (`Default { schema_default }`),
  neu `UserChoice::from_key`. Log z. B. „Symbolthema Querencia (Vorgabe
  „Querencia“)“ bzw. „Symbolthema gnome-human (keine Benutzereinstellung,
  Vorgabe „Adwaita“)“.

## 0.1.3 – 2026-10-27 (Panel 0.4.0, QD-164)

- `classic_menu`: Häkchen- und Radio-Einträge. Ein Eintrag mit dem
  Attribut `x-querencia-toggle` (`"check"` oder `"radio"`) und
  `x-querencia-toggled` (Boolean) zeigt in der Symbolspalte ✓ bzw. •, wenn
  er an ist, sonst die leere Spalte (wie GTK 2); Barrierefreiheit:
  Rolle `MenuItemCheckbox`/`MenuItemRadio` mit Zustand `checked`. Neu
  `Row::Entry { toggle }`, `Toggle`, `TOGGLE_ATTRIBUTE`,
  `TOGGLED_ATTRIBUTE`, CSS-Klasse `qmenu-toggle`. `PopoverMenu` nimmt den
  Zustand aus der Aktion; die klassischen Menüs können Aktionen eines
  Widgets nicht nach ihrem Zustand fragen, deshalb setzt ihn das Modell.
- Ohne die Attribute bleibt jedes Menü wie bisher (Test
  `check_and_radio_items_come_from_their_attributes`). Panel, Desktop und
  Dateien bleiben wie mit 0.1.2; nur Panel 0.4.0 nutzt die Häkchen (Menüs
  der Programme im Infobereich). Enthält 0.1.2 (Kataloge it/es/fr).

## 0.1.2 – 2026-10-26

Italienisch, Spanisch, Französisch:

- Kataloge `po/it.po`, `po/es.po`, `po/fr.po` (je 5 Tastennamen: Maiusc/Mayús/Maj usw.); Wortwahl nach der gemeinsamen Querencia-Begriffstabelle.

## 0.1.1 – 2026-10-25 (QD-176)

- `classic_menu::attach_popup` für Kontextmenüs auf einem Anker: Der Anker
  gehört nicht zur Menüleiste seines Fensters. Bisher registrierte `attach`
  jeden Knopf als Hauptmenü; in Querencia Dateien sprang ein offenes
  Kontextmenü deshalb in die Menüleiste, sobald der Zeiger irgendwo im
  Fenster unter einem Menü der Leiste stand (Fehlerbild: Zeiger Richtung
  „Umbenennen …“, „Ansicht“ klappt auf), und Links/Rechts wechselten aus
  dem Kontextmenü in die Leiste.
- Wechsel zwischen Hauptmenüs per Zeiger nur über der Knopffläche (x und y,
  `button_at` statt `span_at`). Im Panel füllen die Knöpfe die Leiste;
  dort ändert sich nichts.
- GUI-Test (`gui-test`): Kontextmenü neben den Hauptmenüs wechselt weder
  per Zeiger noch per Taste; im Zubehör wird Querencia Dateien statt Files
  erwartet.
- Panel und Desktop sind mit 0.1.0 gebaut; ihr Verhalten ändert sich erst
  mit ihrer nächsten Version.

## 0.1.0 – 2026-10-23

Erste Fassung: die GTK-Bausteine, die Panel (bis 0.3.0) und Desktop (bis
0.2.6, über `#[path]`) als Quelltext des Panels teilten, als Bibliothek des
Workspace. Code und Verhalten wie im Panel 0.3.0.

- `classic_menu`: klassische GNOME-2-Menüs (HMP-03/04/05). Neu statt der
  Weiche `cfg(querencia_desktop)`: `set_icon_loading(IconLoading)`, Standard
  ohne GTKs Vorladen (QD-106), `IconLoading::Gtk` für GTKs eigenen Weg.
  Der GUI-Test der Menüs (`classic_menu::gui_test`, Feature `gui-test`)
  nimmt Stil und Menüknöpfe vom aufrufenden Programm (`gui_test::Panel`).
- `symbole`: Symbole ohne Vorladen im Thread-Pool (QD-106), Panel 0.2.4.
- `icon_theme`: Symbolthema des Nutzers, sonst GNOME Human (G2-AD).
- `farbschema`: dunkle Regeln nur bei `dunkel` (dunkles Farbschema und Thema
  mit `gtk-dark.css`, QD-081).
- `portal`: Settings-Portal beim Start mit GSettings als Rückfall (QD-062).
- `i18n`: Textdomäne `querencia-oberflaeche` (`po/`, `werkzeug/i18n.py`);
  die Tastennamen der Kürzel mit dem bisherigen deutschen Wortlaut des
  Panels. Programme binden sie mit `i18n::bind` an ihr Katalogverzeichnis.
