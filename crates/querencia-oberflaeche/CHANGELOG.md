# Änderungen – querencia-oberflaeche

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
