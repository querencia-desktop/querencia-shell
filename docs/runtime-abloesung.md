# Runtime nach der Ablösung: Panel 0.3.2 (M3) und Kürzel 0.2.2

Folgezug der Welle C (Querencia Dateien löst Querencia Files ab, 08 QD-168),
vereinbart mit der Status-Session, ausdrücklich nicht Teil von C3. Zwei
kleine Stände, Zweig `runtime-abloesung`: zuerst auf main `72a7a64` (Tags
`panel/0.3.2`, `kuerzel/0.2.2` = `d15e821`), nach dem Merge von C3 auf main
`5510fc4` gemergt (Tags `panel/0.3.2-2`, `kuerzel/0.2.2-2` = `54de1de`,
Binaries byte-gleich). Aktiviert wird nur der -2-Stand über
`integration/panel-032-2/` und `integration/kuerzel-022-2/`.

## 1. Panel 0.3.2 – Migration M3 im Orte-Menü

Das Orte-Menü (`crates/querencia-panel/src/places_menu.rs`) öffnet Ordner mit
der Anwendung, die GIO für `inode/directory` wählt. Nur wenn GIO keine hat,
nimmt es selbst einen installierten Starter, der `inode/directory` in
`MimeType` nennt. Für diesen Ersatz gilt jetzt die Reihenfolge

| Rang | Merkmal | Programm |
|---|---|---|
| 0 | Kennung `org.querencia.Dateien.desktop` | Querencia Dateien |
| 1 | Kennung `org.querencia.Files.desktop` | Querencia Files (QF-0t) |
| 2 | Kennung `io.github.lgse.Strata.desktop` | alte Brücke von Files |
| 3 | Name „Querencia Dateien“ | Dateien mit fremder Kennung |
| 4 | Name „Querencia Files“ | Files mit fremder Kennung |
| – | sonst | erster Starter, der Ordner nennt |

Der englische Name von Dateien („Files“) zählt nicht, weil GNOME Dateien
(Nautilus) ihn auch trägt. Der Code stammt aus dem Patch der C3-Sitzung
(`f1a2d5a`, Autor „Claude (dateien-reiter)“), Version, CHANGELOG und
Moduldoku aus `7ab4e40`.

**Live ändert sich nichts Sichtbares.** Seit 23.10.2026 21:05 ist in
`~/.config/mimeapps.list` `inode/directory=org.querencia.Dateien.desktop`
gesetzt; damit wählt GIO, und der Ersatz greift nie. M3 wirkt nur ohne
GIO-Vorgabe und ohne `mimeinfo.cache`-Eintrag, etwa in einem frischen
Profil oder nach einer Paketinstallation, bei der der Cache noch nicht neu
geschrieben ist. Das ist gewollt: Die Vorgabe des Benutzers gewinnt immer.
`panel-032-integrieren.sh pruefen` zeigt die gesetzte Vorgabe an (nur
gelesen).

Keine neuen Nutzertexte; die vier Kataloge der Ablage sind byte-gleich mit
0.3.1. Die Protokollzeile des Panels nennt wie bisher die gewählte
Anwendung: `Ortemenü: … (Ordner öffnet <Name> [<Kennung>], Wahl von GIO |
Ersatz ohne GIO-Zuordnung)`.

## 2. Kürzel 0.2.2 – Block „Querencia Dateien“

`crates/querencia-kuerzel/data/programme.txt`: Der Block „Querencia Files“
heißt „Querencia Dateien“, die Quellzeile zeigt auf
`crates/querencia-dateien/src/menue.rs` (`KUERZEL`, `KUERZEL_IM_BEREICH`).
Grundlage ist der Vorschlag der C3-Übergabe (§6 Nr. 4), mit diesen
Entscheidungen:

- **Reihenfolge wie die Menüs** von Dateien mit C3: Datei, Bearbeiten,
  Ansicht, Gehe zu, Lesezeichen, Reiter, Hilfe; Maus und Kontextmenü an der
  Stelle, an der die Menüs denselben Befehl haben. Gegenüber dem Vorschlag
  rücken deshalb Strg+T vor Strg+N, Umschalt+Strg+W vor Strg+W, Strg+L hinter
  Alt+Pos1 und die Reiterkürzel hinter Strg+D.
- **Schreibweise wie die Menüs:** „Umschalt+Strg+N“ (so zeigt
  `classic_menu::accel_label` `<Shift><Control>n`). Die Blöcke der anderen
  Programme behalten ihre Schreibweise („Strg+Umschalt+T“ im Terminal).
- **Dazu aus main `72a7a64`:** Strg+F „Suchen“ (Welle C2, mit dem Hinweis
  auf Esc in der Suchleiste), Umschalt+F2 „Umbenennen, ganzen Namen
  markieren“, Menü/Umschalt+F10 „Kontextmenü“. „Mit Server verbinden …“
  (C1) hat kein Kürzel und steht deshalb nicht im Block. Die Tasten „/“ und
  „~“ (öffnen das Ortsfeld) passen nicht ins Format (` / ` trennt Kürzel)
  und fehlen wie bei Files.
- **Aus C3** (noch nicht in main; nur gelesen in
  `~/projects/querencia-runtime-wt-dateien-reiter`, Kopf `9167797`): Strg+T,
  Strg+W „Reiter schließen (der letzte schließt das Fenster)“,
  Strg+Bild↑/↓, Umschalt+Strg+Bild↑/↓, Alt+1 … Alt+0, Umschalt+Strg+O,
  Strg+3 Kompaktansicht, Umschalt+Strg+I Auswahl umkehren (in main nur im
  Menü gezeigt, gebunden erst mit C3). Alt+1 … Alt+0 heißt wie im
  Terminal-Block „Zu Reiter 1 … 10 wechseln“.
- „Tab“ heißt „Reiter“. Die Kürzelübersicht hat keinen Katalog: Sie bleibt
  auch mit `LANGUAGE=en` deutsch (so seit 0.1.0).

38 Kürzel. Der Test `built_in_file_is_clean` prüft Blockname, Zahl, die
ersten fünf in Menüreihenfolge, die geänderten Bedeutungen (Strg+D, F9,
Strg+3, Umschalt+Entf, Strg+W, Alt+1 … Alt+0) und dass Strg+K, Strg+B,
Strg+, und Strg+Z fehlen.

## 3. Kopplungen

| Was | Regel |
|---|---|
| Kürzel 0.2.2 ↔ Dateien mit C3 | Der Block nennt Reiter- und Kompaktkürzel. Kürzel 0.2.2 erst zusammen mit oder nach Dateien mit C3 (`dateien/reiter-1`) aktivieren; mit Dateien 0.1.0 zeigte die Übersicht Kürzel, die es dort noch nicht gibt. Rückfall von Dateien vor oder mit Kürzel 0.2.2 `zurueck`. |
| Panel 0.3.2 ↔ GIO-Vorgabe | Keine harte Kopplung. Mit Vorgabe (live) gleich 0.3.1; ohne Vorgabe Dateien vor Files.  zurueck` (Files wieder Ordnerprogramm) bleibt davon unberührt. |
| Panel-Rückfall | `panel-032 … zurueck` → 0.3.1-2 (`398577d3`), weiter `panel-031 … zurueck` → 0.3.0-2. Super+D (Kopplungsliste h) beantworten alle drei. |
| Kürzel-Rückfall | `kuerzel-022 … zurueck` → 0.2.1 (`0b7bc233`), danach `kuerzel-021 … zurueck` → 0.2.0; das 0.2.1-Skript verweigert, solange 0.2.2 verlinkt ist (Probe 6c). |

## 4. Integration

Arbeitsordner `~/projects/abloesung-arbeit` (maßgeblich, mit Binaries und
Katalogen); Kopien ohne Binaries und Kataloge in `integration/panel-032-2/`
und `integration/kuerzel-022-2/` (die ersten Fassungen `panel-032/`,
`kuerzel-022/` bleiben unverändert liegen und werden nicht aktiviert).

- `panel-032-2-integrieren.sh pruefen | panel-neustart-pruefen | aktivieren |
  panel-neustart | zurueck | panel-neustart-zurueck`
  (`PANEL_032_2_FREIGABE=ja`), Muster panel-031. Ablage
  `~/opt/querencia/panel/e6da8f43a0fe2523/` (Binary und vier Kataloge).
  `panel-neustart` tauscht das laufende Panel 0.3.1-2 oder – bis zur
  nächsten Anmeldung – das noch laufende 0.3.0-2 (`b6f9b2fa`). **Nach einem
  solchen Direkttausch 0.3.0-2 → 0.3.2 landet der Rückfall (`zurueck`, dann
  `panel-neustart-zurueck`) auf dem verlinkten 0.3.1-2**, nicht auf 0.3.0-2;
  genau 0.3.0-2 erst danach über `panel-031-integrieren.sh zurueck` und
  `panel-neustart-zurueck`. Skript und Journalzeile sagen das.
- Sperre: die -2-Fassung nimmt dieselben Sperrdateien wie panel-031 (das
  Rückfallskript) und die erste Fassung panel-032; keines läuft neben einem
  anderen. (Die erste Fassung `integration/panel-032/` hatte nur eine eigene
  Sperrdatei; sie wird nicht aktiviert.)
- `kuerzel-022-2-integrieren.sh pruefen | aktivieren | zurueck`
  (`QK222_FREIGABE=ja`), Muster kuerzel-021, gemeinsame Sperrdatei
  `kuerzel-integrieren.lock` mit allen Kürzel-Skripten. Ablage
  `~/opt/querencia/kuerzel/5372c22822acccf0/` (Programm und Symbol wie
  0.2.1); `pruefen` zeigt den Dateien-Link (nur gelesen).
- Beide starten bei `aktivieren` nichts; das Panel gilt ab der nächsten
  Anmeldung oder nach `panel-neustart`, die Übersicht beim nächsten Öffnen.

## 5. Prüfung

- `cargo test -p querencia-panel -p querencia-kuerzel`, `cargo clippy … -D
  warnings`, `cargo fmt --all --check`; Release zweimal gebaut, byte-gleich.
- Orte-Menü isoliert (`integration/panel-032/werkzeug/alle.sh`: eigenes
  HOME, XDG_RUNTIME_DIR, Bus, headless labwc; Test-Starter protokollieren
  nur): Fälle beide, GIO-Vorgabe Files bzw. Dateien, nur Files, nur Namen,
  GNOME „Files“, keiner; Gegenprobe mit 0.3.1-2. Geprüft werden die
  Protokollzeile und der echte Start über „Persönlicher Ordner“.
- Kürzelübersicht isoliert (`werkzeug/lauf-kuerzel.sh`): `--version`,
  `--liste` (38 Kürzel im Block), Fenster auf der Seite „Programme“ mit
  Fotos über den ganzen Block, de/en, 1024×768 und 1920×1080, hell und
  dunkel; Esc schließt.
- Proben der Integrationsskripte auf umgelenkten Pfaden, mit Rückbau-Kette.

Zahlen und Pfade der Läufe: .

## 6. Grenzen

- M3 ist live erst sichtbar, wenn die GIO-Vorgabe fehlt; die Gegenprobe zeigt
  den Unterschied nur im isolierten Profil.
- Ohne GVfs-Backends (isolierter Lauf mit `GIO_USE_VFS=local`) fehlen im
  Orte-Menü Computer, Netzwerk und Papierkorb; das ist Absicht der Prüfung.
- Die Kürzelübersicht hat keine englische Fassung.
- Kommentare „open in Querencia Files“ in `desktop_view.rs` bleiben bis zur
  nächsten Desktop-Version.
