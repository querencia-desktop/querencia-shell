# querencia-kuerzel 0.2.0 – Nachtrag der live gegangenen Kürzel

Auftrag kuerzel-reiter (21.10.2026), Zweig `kuerzel-reiter` ab `main`
`dce66e3` (G2-AL), Tag `kuerzel/0.2.0`.

## Was sich ändert

| Stelle | Vorher (0.1.0) | Jetzt (0.2.0) | Beleg |
|---|---|---|---|
| Programme → Querencia Terminal | Kopieren, Einfügen, Kontextmenü | dazu die Reiterkürzel vorn: Strg+Umschalt+T/W/Q, Strg+Bild↑/Bild↓, Strg+Umschalt+Bild↑/Bild↓, Alt+1 … Alt+9, Mittelklick auf einen Reiter, Doppelklick auf die freie Reiterleiste | `querencia-terminal-dev` Tag `terminal/0.3.0`: `src/tabs.rs` `shortcut()` (Tastenzuordnung, Tests) und `shortcut_overview()` (Reihenfolge und Wortlaut, `po/de.po`); live seit 20.10. 23:34 (`~/opt/querencia-terminal/d068d665b2043448`) |
| Programme → Bildschirmfoto | Strg+W = Schließen | Strg+W / Strg+Q = Schließen | `querencia-screenshot-dev` Tag `screenshot/0.2.8` `src/editor.rs` `ACCELS` (`win.close`), QD-051; live `49178d4f9aa60847` |
| Arbeitsfläche → Super+D | „startet „gdbus““ | „Arbeitsfläche anzeigen: alle Fenster minimieren oder wiederherstellen“ | Code aus G2-AL (`44f9f4f`), wird erst mit diesem Bau sichtbar |

Schreibweise wie im Rest von `programme.txt`: deutsche Tastennamen,
„Bild↑ / Bild↓“ (wie Texteditor und labwc-Seite), nicht „Bild auf“ aus der
Terminal-Hilfe und nie „Shift+Ctrl“ (so zeigt GTK das Menükürzel ohne
`gtk40.mo`).

## Abgeglichen, ohne Änderung

- Super+F und Alt+F11 (Vollbild, QD-065/G2-AK), Druck und Alt+Druck: stehen
  in der rc.xml und erscheinen auf der Seite „Arbeitsfläche“ schon richtig;
  neu ist nur ein Test dafür (`querencia_runtime_rc_xml`).
- Super+D steht in der labwc-Vorgabe, aber noch nicht in der Benutzerkopie
  `~/.config/querencia/labwc/rc.xml` der laufenden Sitzung; die Seite zeigt
  es nach dem Vorgaben-Abgleich der nächsten Anmeldung.
- Arbeitsfläche (Desktop), Files QF-0p, Texteditor 0.2.2, Bildbetrachter
  0.2.3, Taschenrechner 0.1.3, Nach Dateien suchen 0.1.3: Kürzel im
  Live-Tag gleich dem Stand vom 18.10.

## Sprachen

Das Kürzelprogramm ist bis jetzt nur deutsch (kein gettext, `programme.txt`
und die Beschreibungen der labwc-Seite deutsch). Der isolierte Lauf mit
`LANGUAGE=en` zeigt daher dasselbe deutsche Fenster; eine englische Fassung
gehört zur gettext-Umstellung des Crates (eigener Auftrag).

## Prüfung und Integration

- `cargo fmt --check`, `cargo clippy --all-targets -D warnings`, Tests
  (`cargo test --no-run` über den Bau-Platz, Testbinary danach ohne
  Bildschirm).
- Isolierter Lauf, Integrationsskript, Vorlage und Probe liegen außerhalb
  des Repos in `~/projects/querencia-runtime-kuerzel-0.2.0/`
  (`integration/kuerzel-020-integrieren.sh pruefen|aktivieren|zurueck`,
  `QK2_FREIGABE=ja`). Getauscht werden der Link
  `~/opt/querencia/bin/querencia-kuerzel` und der Menüeintrag
  `~/.local/share/applications/org.querencia.Kuerzel.desktop`: statt des
  festen SVG-Pfads in die Ablage 0.1.0 `Icon=org.querencia.Kuerzel` aus dem
  Symbolthema Querencia (Symbole 0.7). Das Skript setzt den Namen nur, wenn
  das eingestellte Thema das Symbol in allen Größen hat; sonst bleibt der
  Pfad. Panel und Desktop bleiben unberührt, das Skript des Vorgängers
  (`integration/kuerzel/kuerzel-integrieren.sh`) bleibt unverändert.
- Der Integrationsordner liegt byte-gleich auch unter
  `integration/kuerzel-020/` (Tag `kuerzel/0.2.0-2`; das Release-Binary der
  Vorlage ist wie üblich nicht eingecheckt, seine Summe steht in
  `integration/vorlage/SHA256SUMS`).
- Rückbau-Reihenfolge mit Symbole 0.7: erst `kuerzel-020-integrieren.sh
  zurueck`, dann `symbole-07-integrieren.sh zurueck-0.6`, sonst zeigt das Menü
  `image-missing`.
