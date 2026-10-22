# Panel 0.2.2 – Nutzertexte für Alpha 2.1

Stand 22.10.2026. Zweig `panel-022`, Code `6398fe0`, Tag `panel/0.2.2`,
Integrationsstand `panel/0.2.2-int`. Änderungen im Einzelnen:
`crates/querencia-panel/CHANGELOG.md`.

## Was sich ändert

| Stelle | Vorher | Jetzt |
|---|---|---|
| Netzwerk-Applet, Links- und Rechtsklickmenü | „Netzwerkeinstellungen … (kommt mit G2-O)“, ausgegraut | „Netzwerkeinstellungen (noch nicht verfügbar)“, ausgegraut, solange `org.querencia.Einstellungen.desktop` keine Aktion `netzwerk` hat (Einstellungen 0.1.4: keine); mit Aktion „Netzwerkeinstellungen …“ |
| „Informationen über Querencia“ | „Querencia Desktop“, „Version 0.2.1“ (Crate), Beschreibung, Lizenz | „Querencia Desktop“, Ausgabe (z. B. „Alpha 2.1“), „Panel 0.2.2“, Beschreibung, Link „querencialinux.org“, „Lizenz: GPL-3.0-or-later“ |
| System-Menü | Informationen über Querencia | dazu „Online-Hilfe“ und „Fehler melden“ im selben Abschnitt |

## Ausgabe im Info-Dialog

Format und Pfade legt die Paketierung fest
:

1. `/usr/share/querencia/release` lesbar: `VERSION` (os-release(5): Werte
   mit oder ohne doppelte Anführungszeichen, Escapes `\" \\ \` \$`,
   Kommentare und Leerzeilen übersprungen). `HOME_URL` wird der Link, wenn
   er mit `https://` beginnt und keinen Leerraum enthält, sonst
   `https://querencialinux.org`.
2. sonst `/usr/share/querencia/version`: erste Zeile, Rand abgeschnitten.
3. sonst keine Ausgabe, nur „Panel 0.2.2“ (Entwicklungsstand, Einzelpakete).

Feste Pfade, keine Umgebungsvariable. Geprüft in Unit-Tests mit einem
Testordner als Wurzel (`about::about_from`); in der isolierten GUI-Prüfung
fehlt die Datei (Fall 3).

## Adressen

| Eintrag | Adresse |
|---|---|
| Link im Info-Dialog | `HOME_URL`, sonst https://querencialinux.org |
| Online-Hilfe | https://querencialinux.org/de/docs/ bei deutscher Oberfläche, sonst https://querencialinux.org/en/docs/ |
| Fehler melden | https://github.com/querencia-desktop/querencia/issues/new/choose |

Die Sprache folgt `g_get_language_names()` (gettext-Regeln: `LANGUAGE`,
`LC_ALL`, `LC_MESSAGES`, `LANG`); in der Querencia-Sitzung (`LANG=en_US.UTF-8`,
`LANGUAGE=de`) ist das Deutsch. Geöffnet wird mit `xdg-open`, gesucht wie
jedes Programm des Panels (`XDG_OPEN`, `PATH`, `~/opt/querencia/bin`,
`/usr/bin`); fehlt es, steht nur eine Zeile im Protokoll.

## Kennungen

Alle Zeichenketten des Panels (ohne Kommentare) wurden mit einem kleinen
Scanner nach `QD-`, `G2-`, `QREL`, `HMP-`, `QE-`, `QF-`, `M1.`, `M2.`
durchsucht. Sichtbar war nur der Netzwerkeintrag; dazu kamen eine
Protokollzeile und die `<desc>` der drei Glockensymbole. Übrig ist nur der
Fenstertitel der isolierten Menüprobe (`classic_menu/gui_test.rs`, nur in
Tests übersetzt). `querencia-netzwerk` und `querencia-lautstaerke` haben
keine Kennungen in Zeichenketten.

## Prüfung

Arbeitsordner `~/projects/querencia-panel-022` (Kopie ohne Binaries und
Bilder: `integration/panel-022/`):

- `cargo fmt --check`, `build`, `clippy --all-targets -D warnings`, `test`:
  466 Tests, davon 295 Panel (1 ignoriert: isolierte Menüprobe).
- Isoliert (eigener labwc, Bus, HOME; NetworkManager-Attrappe mit
  Dokumentationsadressen; xdg-open-Attrappe): de/en × hell/dunkel je 9/9
  PASS, 8 Bilder je Lauf; Tausch 0.2.1 → 0.2.2 → 0.2.1 mit Record-Datei
  12/12 PASS.
- Probe des Integrationsskripts auf umgelenkten Kopien 40/40 PASS.
