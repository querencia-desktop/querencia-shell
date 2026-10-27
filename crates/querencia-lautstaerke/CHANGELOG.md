# Änderungen – querencia-lautstaerke

Bibliothek (Tonsystem für das Panel-Applet) und Programm Lautstärkeregler
(`org.querencia.Lautstaerke`). Frühere Stände: `docs/g2j-lautstaerke-applet.md`,
`docs/panel-030-englisch.md`, Übergabe i18n Teil A.

## 0.1.2 – 2026-10-27

Lautstärkeregler übersetzt (i18n-reste, 08 QD-055):

- Fenster, Menüleiste (Datei, Hilfe), Reiter, Beschriftungen, Hinweise und
  der Info-Dialog über gettext (Domäne `querencia-lautstaerke`, msgid
  englisch, de/it/es/fr; 32 Texte statt 6). Deutsch wie bisher, außer
  „Audiodienst: keiner“ im Info-Dialog statt „Audiodienst: kein audiodienst“.
- Prozentangaben je Sprache (it „40%“, fr „40 %“ mit schmalem Leerzeichen).
- Das Programm bindet seine Domäne beim Start (`i18n::init`).
- Englische Zugriffstasten der Reiter: _Playback, _Recording, _Applications.
- `querencia-lautstaerke --zustand` (Diagnose) bleibt deutsch.
