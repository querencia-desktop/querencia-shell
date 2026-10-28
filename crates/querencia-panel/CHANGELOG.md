# Änderungen – querencia-panel

## 0.4.1 – 2026-10-28

Symbolthema nach der Vorgabe der Sitzung (querencia-oberflaeche 0.1.4):

- Ohne eigene Wahl des Symbolthemas nimmt das Panel die Vorgabe der Sitzung
  (in der Querencia-Sitzung „Querencia“ aus dem Sitzungspaket) statt immer
  GNOME Human. Wer ein Thema gewählt hat, sieht keinen Unterschied.

## 0.4.0 – 2026-10-27

Infobereich (QD-164, `docs/panel-040-infobereich.md`):

- Neues Status-Applet `notification-area` („Infobereich“, „Notification
  Area“): Symbole anderer Programme über das StatusNotifierItem-Protokoll
  (Webex, Nextcloud, Discord, Programme mit libappindicator …). Im
  Standardlayout oben rechts vor der Glocke; höchstens einer je Layout.
- Das Panel bietet `org.kde.StatusNotifierWatcher` an, solange niemand
  sonst ihn anbietet (Warteschlange, Übernahme nach dessen Ende), mit dem
  Alias `org.freedesktop.StatusNotifierWatcher`; es ist immer Host.
- Je Item: Symbol (Themenname, eigener Symbolordner, Datei, Pixmap),
  Overlay, Aufmerksamkeit (hervorgehoben), Passive ausgeblendet, Tooltip;
  Linksklick `Activate` (mit xdg-activation-Token), mittlere Taste
  `SecondaryActivate`, Mausrad `Scroll`, Rechtsklick das Menü des Programms
  (com.canonical.dbusmenu als klassisches Querencia-Menü mit Untermenüs,
  Häkchen, Radio, grauen Einträgen, Symbolen, Kürzeln) bzw.
  `ContextMenu`. Rechtsklick auf den Rand des Infobereichs: Appletmenü.
- `--panel-ini infobereich-ergaenzen|infobereich-zuruecknehmen <datei>`:
  Infobereich vor die Glocke setzen bzw. nur ihn herausnehmen (für die
  Integration; das Panel ändert die Benutzerdatei nie von selbst).
- Braucht querencia-oberflaeche 0.1.3 (Häkchen und Radio in
  `classic_menu`); Kataloge: „Infobereich“, „Statussymbole laufender
  Programme“ in de, it, es, fr. Enthält 0.3.3 (it/es/fr).

## 0.3.3 – 2026-10-26

Italienisch, Spanisch, Französisch:

- Kataloge `po/it.po`, `po/es.po`, `po/fr.po` (je 151 Texte): Menüs Anwendungen/Orte/System, Applets, „Zum Panel hinzufügen“, Netzwerk-, Lautstärke- und Mitteilungsmenüs; Uhr numerisch wie im Deutschen; Wortwahl nach der gemeinsamen Querencia-Begriffstabelle.

## 0.3.2 – 2026-10-24

Integrationsstand 0.3.2-2 (Tags `panel/0.3.2-2`, `panel/0.3.2-2-int`):
derselbe Code, gemergt auf runtime main `5510fc4` (Welle C3). Binary und
Kataloge sind byte-gleich mit 0.3.2; `integration/panel-032/` wird nicht
aktiviert, sondern `integration/panel-032-2/`.

Migration M3 der Welle C (Querencia Dateien löst Querencia Files ab,
Folgezug „Runtime nach der Ablösung“, 08 QD-168):

- Das Orte-Menü kennt Querencia Dateien: Hat GIO für `inode/directory`
  keine Anwendung, nimmt das Panel unter den installierten Einträgen, die
  selbst `inode/directory` nennen, zuerst `org.querencia.Dateien.desktop`,
  dann Querencia Files (`org.querencia.Files.desktop`, danach die Brücke
  `io.github.lgse.Strata.desktop`). Ohne passende Kennung zählt der Name
  „Querencia Dateien“ vor „Querencia Files“; der englische Name „Files“
  zählt nicht, weil GNOME Dateien (Nautilus) ihn auch trägt.
- Die Wahl von GIO gewinnt weiter immer. Live ist `inode/directory` seit
  23.10.2026 21:05 auf `org.querencia.Dateien.desktop` gesetzt; dort ändert
  0.3.2 nichts Sichtbares. Die neue Reihenfolge wirkt nur ohne GIO-Vorgabe
  (z. B. frisches Profil, Paketinstallation ohne `mimeinfo.cache`).
- Tests: Dateien vor Files, Rangfolge nach Kennung und Name, „Files“ von
  GNOME zählt nicht (`cargo test -p querencia-panel places`).

## 0.3.1 – 2026-10-23

Integrationsstand 0.3.1-2 (Tags `panel/0.3.1-2`, `panel/0.3.1-2-int`):
derselbe Code, gemergt auf runtime main `5a5b65c` (Dateiarbeit 0.1.1,
Prüfrahmen 0.1.1). Binary und Kataloge sind byte-gleich mit 0.3.1
(`panel/0.3.1`); `panel/0.3.1-int` wird nicht aktiviert.

Nur ein Umbau, sichtbar ändert sich nichts gegenüber 0.3.0.

- Die Bausteine `classic_menu`, `symbole`, `icon_theme`, `farbschema` und
  `portal` stehen jetzt in der Bibliothek `querencia-oberflaeche` des
  Workspace (`crates/querencia-oberflaeche`), die auch der Desktop und der
  kommende Dateimanager nutzen. Das Panel bindet sie über `use` ein; die
  Menüs laden ihre Symbole weiter ohne GTKs Vorladen (QD-106).
- Die Tastennamen der Kürzel (`Strg`, `Umschalt`, `Entf`, `Eingabe`,
  `Druck`) kommen aus der Textdomäne `querencia-oberflaeche`; das Panel
  bindet sie wie Lautstärke und Netzwerk an sein Katalogverzeichnis und
  liefert `querencia-oberflaeche.mo` mit. In `po/` des Panels fallen die
  fünf Einträge weg. Ohne diesen Katalog stünde im System-Menü
  „Ctrl+Alt+L“ statt „Strg+Alt+L“.
- Der GUI-Test der Menüs liegt in der Bibliothek
  (`classic_menu::gui_test`, Feature `gui-test`); das Panel ruft ihn mit
  seinen eigenen Menüknöpfen auf (`src/menu_gui_test.rs`, ignoriert, nur
  isoliert).

## 0.3.0 – 2026-10-22

Integrationsstand 0.3.0-2 (Tags `panel/0.3.0-2`, `panel/0.3.0-2-int`):
derselbe Code, gemergt auf runtime main `52935d0` (Desktop 0.2.6-2). Binary
und Kataloge sind byte-gleich mit 0.3.0; aktiviert wird über
`integration/panel-030-2/`, `integration/panel-030/` wurde nie aktiviert.

Englisch per gettext (QD-055), Muster wie Editor 0.2.0 und Desktop 0.2.5.

- Alle sichtbaren Texte über GNU gettext: msgid englisch, Deutsch aus
  `po/de.po` mit exakt dem bisherigen Wortlaut (Prüfung gegen panel/0.2.5:
  alle 91 alten deutschen Nutzertexte stehen im Katalog). Menüs
  Anwendungen/Orte/System mit Kategorien und Orten, Uhr (Format übersetzbar:
  deutsch `%d.%m.%Y %H:%M`, englisch wie GNOME 2 „Thu Oct 22, 10:08 PM“),
  Glocke (Pluralformen), Lautstärke, Netzwerk (Menü, Netzliste, Dialoge),
  „Arbeitsfläche anzeigen“, Fensterliste, Arbeitsflächen, „Zum Panel
  hinzufügen“, Programmauswahl, Kontextmenüs, Info-Dialog, Abmelden-Frage,
  Tastennamen der Kürzel (`Strg`/`Ctrl`).
- Modul `i18n.rs` (Textdomäne `querencia-panel`): Kataloge neben der Binary
  (`share/locale`), dann im Präfix, dann `/usr/share/locale`;
  `QUERENCIA_PANEL_LOCALEDIR` für Tests. Sprache wie üblich über `LANGUAGE`
  vor `LANG`; die Sitzung setzt `LANGUAGE=de`.
- Texte aus `querencia-lautstaerke` und `querencia-netzwerk` (Tooltips,
  Netzliste, Verbindungsinformationen) haben eigene Textdomänen gleichen
  Namens; das Panel liefert deren Kataloge mit und bindet sie.
- `po/`, `werkzeug/i18n.py` (pot, update, mo, pruefen) je Crate; Tests
  `german_catalog_is_complete`, `german_wording_is_the_old_one`.
- Ohne Katalog (z. B. `LANGUAGE=fr`) erscheint Englisch. Protokollzeilen
  bleiben deutsch.

## 0.2.5 – 2026-10-22

Enthält 0.2.4 (nicht einzeln aktiviert) und die App-ID von Querencia Files
(QD-059, Entscheidung der Startsession zu QF-0t).

- Fensterliste (`window_icons.rs`): Ein Fenster mit der App-ID
  `org.querencia.Files` bekommt das Symbol des Starters
  `org.querencia.Files.desktop`; solange nur der alte Starter
  `io.github.lgse.Strata.desktop` (Brücke, `NoDisplay`) installiert ist,
  dessen Symbol (Tabelle `RENAMED`). Die alte App-ID findet weiter den alten
  Starter.
- Orte (`places_menu.rs`): Ohne Wahl von GIO öffnet Querencia Files
  Ordner, erkannt zuerst an `org.querencia.Files.desktop`, dann an
  `io.github.lgse.Strata.desktop`, dann am Namen „Querencia Files“.

## 0.2.4 – 2026-10-22

Ursache der Abstürze QD-106 behoben, Absturzberichte (QD-128), Fenstersymbol
(QD-129), App-ID `org.querencia.Panel` (QD-059).

- Symbole ohne GTKs Vorladen im Thread-Pool (neues Modul `symbole.rs`):
  Ein `GtkImage` mit Symbolname oder GIcon lässt GTK 4.22 das Symbol in
  GLibs Thread-Pool laden (`GTK_ICON_LOOKUP_PRELOAD`). Daraus kamen beide
  Abstürze: (1) Gibt der Pool-Faden die letzte Referenz auf das Symbol frei,
  findet der Hauptfaden es noch in GTKs Symbol-Cache, `g_object_ref` meldet
  `object_already_finalized` und liefert NULL, GTK liest NULL+0x50 – live
  22.10. 04:07:24 (`libgtk-4.so.1.2200.4[1c5e1e]`). (2) Ein SVG-Symbol mit
  Text wird im Pool-Faden über Pango gezeichnet, der Heap geht kaputt –
  `pool-4 … libpangocairo` beim ersten Öffnen von Zubehör (21.10.). Beides
  ist mit `integration/panel-024/werkzeug/gtk-wettlauf.py` in 18 von 18
  Läufen nachgestellt (Stapel im gdb identisch mit dem Live-Absturz). Das Panel schlägt seine
  Symbole jetzt selbst ohne Vorladen nach, gibt dem Bild das Paintable und
  hält es bis zum nächsten Themenwechsel; GTK lädt sie dann beim ersten
  Zeichnen im Hauptfaden. Bilder folgen Themen-, Skalen- und
  Richtungswechseln wie bisher; eigene Texturen der Applets bleiben
  unberührt. Betrifft Menüs, Fensterliste, Starter, „Arbeitsfläche
  anzeigen“, Dialoge und die Ersatzsymbole der Applets. Gegenprobe: 0 von 22
  Läufen; im 10-Minuten-Belastungslauf des Panels 0 statt 863
  Vorladeaufträge.
- Absturzberichte unter `--ueberwacht` (QD-128): Umgebungsvariable
  `QUERENCIA_PANEL_ABSTURZBERICHT` = `erster` (Vorgabe) | `nie` | `immer`.
  Bei `erster` bekommt apport nur den ersten Absturz (ein Bericht zum
  Nachsehen); danach startet die Überwachung weitere Panels mit
  `RLIMIT_CORE` 1, der Kernel reicht dann keinen Kernabzug mehr an apport
  (kein Warten von rund 6 s vor dem Neustart, kein weiterer Bericht von
  rund 17 MB in `/var/crash`). `/proc/<pid>/exe` und `environ` des Panels
  bleiben lesbar.
- Fenstersymbol (QD-129): Info-Dialog und die anderen Fenster des Panels
  zeigen das Querencia-Logo statt eines allgemeinen Symbols (eingebaute
  Ressource `data/panel.gresource`, nur SVG, Name
  `org.querencia.Panel-fenster`).
- App-ID und Busname `org.querencia.Panel`, Objektpfad
  `/org/querencia/Panel` (QD-059). Der alte Name
  `org.example.Querencia.Panel` mit `/org/example/Querencia/Panel` bleibt als
  Zweitname mit derselben Schnittstelle, damit Super+D aus der labwc-Vorgabe
  von Alpha 2/2.1 und aus Benutzerkopien weiter wirkt.

## 0.2.3 – 2026-10-22

Absturz beim ersten Öffnen von Anwendungen (QD-106) und Neustart-Schutz.

- Neuer Modus `querencia-panel --ueberwacht` (Modul `ueberwachung.rs`): ein
  kleiner Elternprozess ohne GTK und ohne Display startet das Panel als
  Kind und startet es nach einem Absturz neu – höchstens 3-mal in
  10 Minuten, nach 1 s Pause, je mit einer Zeile im Protokoll
  („Überwachung: Panel PID … abgestürzt durch Signal 11 (SIGSEGV) nach … s –
  Neustart 1 von 3 in 10 Minuten“). Als Absturz gelten SIGSEGV, SIGABRT,
  SIGBUS, SIGILL, SIGFPE, SIGTRAP, SIGSYS und Status 101 (Rust-Panic).
  Kein Neustart bei jedem anderen Ende (kein Layer Shell, zweite Instanz,
  SIGTERM), wenn die Überwachung selbst SIGTERM/SIGINT/SIGHUP bekommt (wird
  ans Panel weitergegeben, nach 5 s SIGKILL) oder wenn die Sitzung vorbei ist
  (kein Wayland-Socket, `LABWC_PID` beendet). Exit-Status der Überwachung =
  der des Panels (Signal: 128 + Nummer).
- `querencia-panel --faehigkeiten` gibt `ueberwacht` aus, damit der
  Sitzungsstarter `--ueberwacht` nur an Panels gibt, die es kennen.
- Ohne `--ueberwacht` startet das Panel wie bisher; eingeschaltet wird der
  Schutz im Sitzungsstarter (`session/`, eigener Schritt).
- Symbolthema seltener neu einlesen (`icon_reload.rs`): Eine Änderung der
  installierten Programme liest das Thema nur noch neu ein, wenn ein
  Programmsymbol fehlt, das nach dem letzten Einlesen noch nicht fehlte;
  sonst nur eine Protokollzeile. Änderungen unter `~/.local/share/icons`
  lösen wie bisher aus. Grund: Panel 0.2.2 stürzte live am 22.10. um 04:07
  im Symbol-Cache von GTK ab (`object_ref` auf ein freigegebenes Symbol,
  Lesen an NULL+0x50 in `libgtk-4`), Minuten nach drei Neueinlesungen, die
  nur vom Umschreiben vorhandener Starter kamen; jedes Neueinlesen verwirft
  GTKs Cache, während Bilder und Ladefäden ihn noch benutzen.
- Absturz beim ersten Öffnen von Anwendungen (Thread-Pool, `libpangocairo`):
  in 160 isolierten Kaltstarts nicht nachgestellt; Befund, Verdacht und
  Zusammenhang in `docs/panel-023-absturz-und-ueberwachung.md`.

## 0.2.2 – 2026-10-22

Nutzertexte für Alpha 2.1 (QD-110/QD-115, QD-116).

- Netzwerk-Applet: Solange Querencia Einstellungen keine Netzwerkseite hat
  (Desktop-Aktion `netzwerk`; Einstellungen 0.1.4 hat keine), heißt der
  ausgegraute Eintrag „Netzwerkeinstellungen (noch nicht verfügbar)“ statt
  „Netzwerkeinstellungen … (kommt mit G2-O)“, im Linksklick- und im
  Rechtsklickmenü. Gibt es die Aktion, bleibt es „Netzwerkeinstellungen …“.
- „Informationen über Querencia“: Produktname „Querencia Desktop“, darunter
  die Ausgabe aus `/usr/share/querencia/release` (`VERSION`, Format
  os-release(5)), sonst aus der ersten Zeile von `/usr/share/querencia/version`;
  ohne beide (Entwicklungsstand, Einzelpakete) keine Ausgabe. Immer
  „Panel <Version>“ aus `Cargo.toml`, die Beschreibung, der Link
  „querencialinux.org“ (`HOME_URL` aus der Datei, nur `https://`, sonst die
  feste Adresse; geöffnet mit `xdg-open`) und „Lizenz: GPL-3.0-or-later“.
  Feste Pfade, keine Umgebungsvariable (Vorgabe der Paketierung).
- System-Menü: nach „Informationen über Querencia“ die Einträge
  „Online-Hilfe“ (https://querencialinux.org/de/docs/ bei deutscher
  Oberfläche, sonst /en/docs/; Sprache nach den Regeln von gettext) und
  „Fehler melden“ (Auswahlseite für neue Meldungen im GitHub-Repository
  querencia-desktop/querencia), beide mit `xdg-open` (gesucht wie alle
  Programme des Panels: `XDG_OPEN`, `PATH`, `~/opt/querencia/bin`,
  `/usr/bin`). Symbole `help-browser` und `tools-report-bug` (Ersatz
  `dialog-warning`).
- Interne Kennungen aus allen sichtbaren Texten des Panels entfernt (auch
  aus einer Protokollzeile und den Beschreibungen der Glockensymbole).
- Neues Modul `links.rs` (Adressen, Sprache der Hilfe, Öffnen).

## 0.2.1 – 2026-10-21

Statussymbole auf dem dunklen Panel (Entscheidung vom 21.10.2026:
Variante A).

- Neues Modul `symbol_farbe.rs`: Solange `farbschema::dunkel` gilt, zeichnet
  das Panel seine eigenen Symbole für Lautstärke, Netzwerk (Applet und
  Netzwerkmenü) und Glocke mit umgetauschten Grautönen (`UMFAERBUNG`).
  - Fläche `#5a5a5a` → `#bdb7b1`, Umriss `#3c3c3c` → `#23221f`.
  - Blasse „leere“ Balken bleiben blass (`#5f5d57`), ausgegraute Symbole
    bleiben ausgegraut.
  - Orange, Weiß und die orangen Zeilenvarianten bleiben.
- Beim Wechsel des Farbschemas oder Themas werden die Symbole sofort neu
  gezeichnet.
- Die SVG-Quellen und die Crates `querencia-lautstaerke` und
  `querencia-netzwerk` bleiben unverändert. Das Panel nutzt deren
  öffentliches `svg()`. Hell zeichnen die Crates wie bisher (pixelgleich).
- Tests: Jede Graufarbe der Applet-Symbole hat ein dunkles Gegenstück, und
  Orange bleibt an derselben Stelle.

## 0.2.0 – 2026-10-21

CSS-Welle dunkel (Alpha 2, Zweig `welle-dunkel`): Das Panel folgt
der dunklen Fassung von Querencia-Klassisch (Thema 0.2).

- Dunkle Gegenstücke zu den eigenen Regeln in `PANEL_CSS` als ein Block
  `@media (prefers-color-scheme: dark)` (`PANEL_CSS_DUNKEL`):
  Anwendungen/Orte/System und alle Schriften des Panels in `#dfdbd2`,
  Arbeitsflächenumschalter, Fensterliste (normal, überfahren, aktiv,
  minimiert), Knöpfe (Starter, Statussymbole, „Arbeitsfläche anzeigen“),
  Netzwerkmenü, klassische Menüs (`#353430`, Rand `#2b2a27`, Pfeile und
  Kürzel `#a5a09a`). Werte aus der Zuordnung des Themas. Orange-Hervorhebung
  mit weißer Schrift bleibt. Hell ist unverändert, isoliert gemessen
  pixelgleich zum vorigen Stand.
- Neues Modul `farbschema.rs` (auch vom Desktop benutzt): Die eigene CSS
  wird dunkel, genau wenn das Farbschema dunkel ist **und** das GTK-Thema
  eine dunkle Fassung hat (`gtk-4.0/gtk-dark.css`, wie Einstellungen 0.1.2).
  Grundlage ist die isolierte Messung: Ohne Bindung greift der Dunkelblock
  nie. Mit reiner Bindung an `gtk-interface-color-scheme` wird die CSS
  unter Thema 0.1 dunkel, obwohl das Thema hell bleibt. Folgt
  Farbschema, `gtk-application-prefer-dark-theme` (Betrieb ohne Portal)
  und Themenname im Lauf. Logzeile „Farbschema hell|dunkel (Start/Wechsel
  …)“.
- Startfehler GTK 4.22: einmal nach dem Start
  `notify("gtk-interface-color-scheme")`, damit das Thema unter „dunkel“
  nicht hell bleibt.
- Tests: Dunkelblock nur aus Selektoren der hellen Regeln; jede dunkle
  Schrift `#3c3c3c` hat ein helles Gegenstück; Hover gewinnt nie über
  aktive Zelle oder minimierten Knopf; eingebautes Thema hat Dunkel.

## 0.1.0 – bis 2026-10-20

Stufen ohne eigene Versionsnummer (M1 … HMP-11), zuletzt `57a01524`
(Arbeitsfläche anzeigen, Glocke der Mitteilungszentrale); Einzelheiten in
`docs/`.
