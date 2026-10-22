# Panel 0.2.4 – Absturzursache, Absturzberichte, Fenstersymbol, Kennung

Stand 22.10.2026. Zweig `panel-024` (ab main `7356f64`), Tag `panel/0.2.4`
(`a844e77`), Integrationsstand `panel/0.2.4-int`. Änderungen:
`crates/querencia-panel/CHANGELOG.md`. Vorgeschichte:
`docs/panel-023-absturz-und-ueberwachung.md`.

## 1. Ursache der Abstürze (QD-106)

Beide belegten Abstürze haben dieselbe Wurzel: GTK 4.22 lädt Symbole, die
ein `GtkImage` per Name oder GIcon zeigt, in GLibs Thread-Pool vor.

- `gtkiconhelper.c:325` (GTK 4.22.4): Sobald das Bild seine CSS-Größe kennt,
  schlägt es das Symbol mit `GTK_ICON_LOOKUP_PRELOAD` nach.
- `gtkicontheme.c:2374`: Mit diesem Schalter startet GTK eine `GTask` in
  `load_icon_thread`. Die Task hält eine Referenz auf das
  `GtkIconPaintable` und zeichnet das Symbol im Pool-Faden.

### 1.1 Absturz 2 (live 22.10. 04:07:24, Panel 0.2.2)

Ablauf im GTK-Quelltext (`gtkicontheme.c:636`, `gtkiconpaintable.c:519`):

1. Das Bild ist schon wieder weg (Menü neu gebaut, Fensterknopf neu
   beschriftet, Thema neu eingelesen), und das Symbol ist aus GTKs
   LRU-Liste (100 Plätze) gefallen. Die letzte Referenz hält die Task.
2. Der Pool-Faden gibt sie frei. Der Referenzzähler wird 0, danach erst
   nimmt `gtk_icon_paintable_finalize` die globale Cache-Sperre und trägt
   das Symbol aus der Hashtabelle aus.
3. Dazwischen schlägt der Hauptfaden dasselbe Symbol nach, findet es in der
   Hashtabelle und ruft `g_object_ref` auf. GLib 2.88 meldet
   `object_ref: assertion '!object_already_finalized' failed` und liefert
   NULL.
4. GTK prüft `icon->desired_size <= 128` auf NULL. `desired_size` liegt im
   `GtkIconPaintable` bei +0x50 (`gtkiconpaintableprivate.h`: GObject 24
   Byte, IconKey 24, `in_cache`, `icon_name`, `filename`, `loadable` je 8).

Beleg in der Live-Bibliothek: `objdump -d libgtk-4.so.1.2200.4` an
`0x1c5e1e` zeigt `cmpl $0x80,0x50(%rax)` direkt nach `call g_object_ref`
und `g_hash_table_lookup` unter `g_mutex_lock`. Das ist die Kernel-Zeile
„segfault at 50 … libgtk-4.so.1.2200.4[1c5e1e]“ von 04:07:24.

### 1.2 Absturz 1 (Testsystem 21.10., erstes Öffnen von Zubehör)

`pool-4 … segfault … in libpangocairo`: GTK 4.22 zeichnet SVG-Symbole selbst.
Ein SVG mit `<text>` (Rechner, Kürzel) setzt seinen Text im Pool-Faden über
Pango. Laufen mehrere solche Aufträge gleichzeitig, geht der Heap kaputt.
Das passt zu der Erstanmeldung, bei der alle Symbole noch zu laden waren.

### 1.3 Nachgestellt

`integration/panel-024/werkzeug/gtk-wettlauf.py` läuft im isolierten labwc
mit PyGObject und GTK 4.22.4, ohne Panel. Je Runde (alle 4 ms) zeigt es
140 von 300 SVG-Symbolen in drei Größen und verwirft das vorige Raster.

| Modus | Läufe | Abstürze | Befund |
|---|---|---|---|
| `vorladen` (GtkImage mit Namen, wie bis 0.2.3), SVG ohne Text | 8 (3 unter gdb) | 8 | gdb: Hauptfaden SIGSEGV bei `libgtk-4+0x1c5e1e`, Adresse 0x50, davor das CRITICAL; ein Pool-Faden steht in `g_object_unref` → GTK-Finalizer an der Cache-Sperre |
| `vorladen`, SVG mit Text | 10 (3 unter gdb) | 10 | gdb: Pool-Faden SIGABRT „corrupted size vs. prev_size“ bzw. „double free“ in `libpangocairo`, Aufrufer `pango_renderer_draw_layout` ← `gtk_symbolic_paintable_snapshot_symbolic` |
| `gehalten` (Paintable ohne PRELOAD, gehalten, wie 0.2.4) | 12 × 20–60 s | 0 | ~1,6 Mio. Bilder je 60-s-Lauf |
| `wechsel` (wie `gehalten`, alle 50 Runden Thema neu einlesen) | 10 × 60 s | 0 | ~0,9 Mio. Bilder je Lauf |

Die Basisadresse von `libgtk-4` unter gdb ist `0x7ffff4a00000`
(`info proc mappings`), der Absturzpunkt `0x7ffff4bc5e1e` also Offset
`0x1c5e1e` wie live.

Kernabzüge der Live-Abstürze gibt es nicht: `/var/crash` enthält keinen
Panel-Bericht. Laut `/var/log/apport.log` wurde der Absturz um 04:07:24
übergeben, aber mit „executable does not belong to a package, ignoring“
verworfen (Kernabzug-Grenze 0).

## 2. Korrektur in 0.2.4 (`symbole.rs`)

Das Panel schlägt seine Symbole selbst nach
(`IconTheme::lookup_by_gicon` ohne `PRELOAD`) und gibt dem Bild das
Paintable (`Image::set_paintable`). GTK startet dafür keine Task mehr. Das
Symbol wird beim ersten Zeichnen im Hauptfaden geladen, und die letzte
Referenz fällt immer dort.

- Paintables werden je Symbol, Größe, Skala und Richtung gehalten, bis das
  Symbolthema wechselt. Menü-Neubauten schlagen deshalb nicht erneut nach
  und drehen GTKs LRU-Liste nicht weiter.
- Bilder folgen dem Signal `changed` des Symbolthemas und Skalen- und
  Richtungswechseln. Eine eigene Textur, die ein Applet später auf dasselbe
  Bild legt, bleibt unberührt.
- Umgestellt sind alle `GtkImage`-Symbole des Panels: Menüs
  (`classic_menu.rs`), Fensterliste, Starter, „Arbeitsfläche anzeigen“,
  „Zum Panel hinzufügen“, Programmauswahl, Abmelden-Frage und die
  Ersatzsymbole von Lautstärke, Netzwerk und Glocke.
- `classic_menu.rs` teilt sich das Panel mit dem Desktop. Dort bleibt GTKs
  eigener Weg (`#[cfg(querencia_desktop)]`), weil der Desktop eigene
  Prüfungen und einen eigenen Worker hat (Folgepunkt, §7).

Belastung des echten Panels (isoliert, `G_DEBUG=fatal-criticals`,
Vorladeaufträge gezählt mit `werkzeug/qvorlade.c` über
`g_task_run_in_thread`):

| Lauf | Panel | Runden | Themenwechsel | Neueinlesungen | Vorladeaufträge | Ergebnis |
|---|---|---|---|---|---|---|
| b024-600 (600 s) | 0.2.4 `448ded49` | 230 | 32 | 26 | 0 | läuft, kein CRITICAL |
| b023-600 (600 s) | 0.2.3 `47ea50e6` | 234 | 33 | 26 | 863 | läuft, kein CRITICAL (der Wettlauf ist selten, siehe 0.2.3: 0 von 523) |

Je Runde: Starter anlegen oder entfernen (jede 4. mit neu fehlendem
Symbol), Symbol unter `icons/` anlegen oder entfernen, Thema
`gnome-human` ↔ `Querencia`, Testfenster auf und zu, Anwendungen → Zubehör
offen über die nächste Änderung hinweg.

Restrisiko: GTK-eigene Bilder mit Symbolnamen laden weiter vor, etwa das
Auge im Passwortfeld oder die Knöpfe eines Drehfelds. Sie erscheinen nur in
selten geöffneten Dialogen und sind symbolisch, also ohne Text. Der Fehler
selbst liegt in GTK und gehört als Meldung an GTK (Folgepunkt; eine
Veröffentlichung macht der Besitzer).

## 3. Absturzberichte (QD-128)

Ubuntu reicht den Kernabzug über `core_pattern` (`|/usr/share/apport/apport
…`) an apport weiter, auch bei Grenze 0. Bei einem Paket-Binary hält apport
den sterbenden Prozess fest und schreibt einen Bericht. Die Weg-B-Probe hat
gemessen: Neustart nach rund 6 s statt 1 s, rund 17 MB in `/var/crash`. Erst
danach meldet der Kernel dem Wachhund das Ende.

Varianten:

| Variante | Neustart | Bericht | Nebenwirkung |
|---|---|---|---|
| A nichts ändern | ~6 s jedes Mal (Paket) | je Absturz | `/var/crash` füllt sich |
| B `PR_SET_DUMPABLE 0` im Panel | ~1 s | keiner | `/proc/<pid>/exe` und `environ` nicht mehr lesbar; Messung, Aufräumer und Tausch arbeiten aber genau damit |
| C `RLIMIT_CORE` 0 | wie A | wie A | wirkungslos, die Pipe zu apport kennt diese Grenze nicht |
| D `RLIMIT_CORE` 1 | ~1 s | keiner | der Kernel überspringt Pipe-Abzüge bei 1 (sein Schutz gegen abstürzendes apport, `fs/coredump.c`) und schreibt eine Zeile ins Kernel-Protokoll |
| **E** D erst nach dem ersten Absturz (umgesetzt) | erster ~6 s, weitere ~1 s | genau einer | wie D |

Umgesetzt ist E, mit Schalter für A und D. Die Variable
`QUERENCIA_PANEL_ABSTURZBERICHT` hat den Wert `erster` (Vorgabe), `nie` oder
`immer`. Sie gilt nur unter `--ueberwacht`. Der Wachhund setzt die weiche
Grenze für sich selbst, die neuen Panels erben sie. Ist die harte Grenze 0,
lässt sich nichts abschalten; das meldet eine Protokollzeile.

Vorgabe `erster`, weil der erste Bericht der einzige brauchbare
Diagnosebeleg ist; zu QD-106 fehlte genau so ein Beleg. Jeder weitere
Absturz der Sitzung bringt nichts Neues, kostet aber Wartezeit und Platz.

Messung, isoliert (Entwicklungs-VM, Binary aus keinem Paket, also verwirft
apport ihn sofort; die 6 s gelten nur für das Paket):

| Lauf | Absturz 1 | Absturz 2 | Absturz 3 | apport aufgerufen |
|---|---|---|---|---|
| 0.2.3 | 1,19 s | 1,25 s | 1,22 s | 3 × |
| 0.2.4 Vorgabe `erster` | 1,18 s | 1,11 s | 1,10 s | 1 × (dann `RLIMIT_CORE` 1) |
| 0.2.4 `nie` | 1,10 s | 1,15 s | 1,12 s | 0 × |
| 0.2.4 `immer` | 1,21 s | 1,24 s | 1,24 s | 3 × |

`/var/crash` blieb in allen Läufen unverändert.

**Hinweis für die Doku und die Release-Notes:** Stürzt das Panel ab, startet
es nach etwa einer Sekunde neu. Nur beim ersten Absturz einer Anmeldung
dauert es einige Sekunden, weil Ubuntu dann einen Fehlerbericht
(`/var/crash/_usr_bin_querencia-panel.<uid>.crash`) anlegt. Mit
`QUERENCIA_PANEL_ABSTURZBERICHT=nie` (z. B. in `~/.config/environment.d/`)
entfällt auch dieser, mit `immer` wird jeder Absturz berichtet.

## 4. Fenstersymbol (QD-129)

Die Fenster des Panels (Informationen, Zum Panel hinzufügen, Abmelden-Frage)
zeigen das Querencia-Logo. Das SVG-Original `data/logo/querencia-logo.svg`
liegt als eingebaute Ressource unter dem eigenen Namen
`org.querencia.Panel-fenster`: `data/panel.gresource`, erzeugt mit
`data/ressourcen.sh`, Unit-Test auf Bytegleichheit. Es ist nur SVG
eingebaut, ohne gleichnamiges PNG, weil GTK 4.22 bei beidem abstürzt. Die
SVG-Datei enthält keinen Text.

`gtk::Window::set_default_icon_name` gilt für alle Fenster des Prozesses.
Zur App-ID gibt es keinen Starter, deshalb nimmt labwc das geschickte
Symbol. Das labwc-Protokoll (`-d`) zeigt im Lauf „loaded icon from client
buffer“. 0.2.3 zeigte im selben Lauf gar kein Symbol, auf dem Testsystem
ein allgemeines.

## 5. Kennung (QD-059)

Ergebnis der Prüfung:

| Ort | Bis 0.2.3 | Ab 0.2.4 |
|---|---|---|
| App-ID, Busname | `org.example.Querencia.Panel` | `org.querencia.Panel` |
| Objektpfad (D-Bus `NeuLaden`, `ArbeitsflaecheUmschalten`) | `/org/example/Querencia/Panel` | `/org/querencia/Panel` |
| App-ID der Panel-Fenster (Fensterliste, labwc) | alt | neu (Logo in der Fensterliste über `APP_ID`) |
| Desktop-Datei | keine | keine |
| gsettings/dconf | keine | keine |
| Pfade (`panel.ini`, `~/.local/state/querencia/panel.log`) | ohne Kennung | unverändert |

Weiterhin nutzt den alten Namen die Tastenbelegung Super+D in
`session/querencia/labwc/rc.xml`. Das betrifft die Vorgabe im Repo, im
Paket von Alpha 2/2.1 und die Benutzerkopie
`~/.config/querencia/labwc/rc.xml`. Damit sie ohne Änderung an
Nutzerdateien weiter wirkt, hält 0.2.4 den alten Namen als Zweitnamen
(`bus_own_name`, wartet in der Warteschlange, solange ein altes Panel ihn
hält) und bietet dieselbe Schnittstelle auch unter dem alten Pfad an. Das
Integrationsskript ändert keine Nutzerdatei. `pruefen` meldet nur lesend,
ob die rc.xml der Sitzung noch den alten Namen ruft.

Nutzertexte: keine interne Kennung in Menüs, Dialogen oder Tooltips; die
Prüfung „keine interne Kennung im Panel-Protokoll“ besteht.

## 6. Prüfung

Arbeitsordner `~/projects/panel-024-arbeit` (Kopie ohne Binaries und
Bilder: `integration/panel-024/`). Release `448ded49…` aus `a844e77`, nach
`cargo clean` byte-gleich neu gebaut.

- `cargo fmt --check`, `clippy --all-targets -D warnings`, `cargo test`
  (Panel 305 + 2 GUI-Tests ignoriert, Desktop 113, Workspace grün). Neu:
  `logo.rs` (Ressource), `ueberwachung.rs` (Absturzberichte, `RLIMIT_CORE`
  in einem Kindprozess), `symbole.rs` (GUI-Test im isolierten labwc,
  bestanden).
- Isoliert `fotos` 1024×768 und 1920×1080, je hell und dunkel: je 13/13
  (Info-Dialog mit Symbol, Zubehör, Fensterliste, Themenwechsel, D-Bus über
  beide Namen). Menüs und Fensterliste sind pixelgleich mit 0.2.3 (0
  abweichende Pixel, hell und dunkel).
- `ueberwacht` (Regression 0.2.3): 20/20. `absturz`: 0 von 20 Kaltstarts.
- `tausch` (0.2.3 mit Wachhund ⇄ 0.2.4, Rückfall bei kaputtem Binary, ohne
  Wachhund): 31/31.
- Probe des Integrationsskripts: 42/42.

## 7. Folgepunkte

1. Desktop: Kontextmenü und Symbole nach demselben Muster (`symbole.rs`
   ohne `cfg`), eigener Auftrag des Desktop-Workers.
2. labwc-Vorgabe `rc.xml`: Super+D auf `org.querencia.Panel`
   `/org/querencia/Panel` umstellen (Sitzung, eigener Schritt). Der
   Zweitname bleibt, bis alte Benutzerkopien abgeglichen sind.
3. GTK-Fehlermeldung (Cache-Wettlauf beim Finalisieren im Pool-Faden, Pango
   im Pool-Faden) mit `gtk-wettlauf.py` als Nachstellung (Besitzer).
4. Paket: Weg-B-Probe mit 0.2.4 misst Neustart und `/var/crash` am
   installierten Paket nach.

## 8. Integration

```bash
I=~/projects/panel-024-arbeit/integration/panel-024-integrieren.sh
bash $I pruefen
PANEL_024_FREIGABE=ja bash $I aktivieren
PANEL_024_FREIGABE=ja bash $I panel-neustart          # eigener Schritt; Wachhund bleibt
PANEL_024_FREIGABE=ja bash $I zurueck                 # Link → 0.2.3 47ea50e6
PANEL_024_FREIGABE=ja bash $I panel-neustart-zurueck
```

`panel-neustart` tauscht mit `tausch-panel-3.py`. Es läuft auch mit
Wachhund, und die Art bleibt erhalten. Gewartet wird nach PID und bis beide
Busnamen frei sind. Die Record-Datei der Sitzung wird nachgeführt.

## 9. Panel 0.2.5: Files-App-ID, korrigiertes Integrationsskript

Stand 22.10.2026, 21:43. 0.2.4 ist seit 21:22:40 aktiviert (Link), der Live-Tausch scheiterte aber an `panel-024-integrieren.sh` (siehe unten); live lief weiter 0.2.3 unter dem Wachhund. 0.2.5 setzt auf runtime main `307ff47` (Desktop 0.2.5) auf: Merge `1cf8176`, `crates/querencia-desktop` gleich main, Cargo.lock nur mit der Panel-Version, `cargo build --locked` grün. Tag `panel/0.2.5` → `1cf8176`; Release `117df958…`, auf `869ff26` und auf dem Merge-Stand nach `cargo clean` byte-gleich.

**Files-App-ID (QD-059, QF-0t).** Querencia Files bekommt `org.querencia.Files`; der alte Starter `io.github.lgse.Strata.desktop` bleibt vorerst als unsichtbare Brücke.
- Fensterliste (`window_icons.rs`): Tabelle `RENAMED` (`org.querencia.Files` → `io.github.lgse.Strata`); die neue ID findet zuerst den neuen Starter nach den Regeln 1–6, sonst den Starter der alten ID; die alte ID findet weiter den alten Starter.
- Orte (`places_menu.rs`): Querencia Files zuerst an `org.querencia.Files.desktop`, dann `io.github.lgse.Strata.desktop`, dann am Namen.
- Tests für beide Fälle; isoliert (`fotos`, Bild 12): neue ID zeigt bei nur altem Starter dasselbe Symbol wie die alte, nicht das allgemeine; geeicht mit 0.2.4 (dort allgemeines Symbol, Prüffall FAIL).
- Weitere Stellen mit der alten ID: `apps/tests.rs` (Starter der Sitzung unter `session/querencia/applications`, unverändert) und `crates/querencia-desktop/src/desktop_view.rs` `FILES_APP` (Desktop-Worker).

**Fehler in `panel-024-integrieren.sh` (gefunden von der Status-Session).** Unter `set -o pipefail` endet `laufend | grep -q …` nach dem ersten Treffer (Wachhund); `laufend` bekommt beim Ausgeben der Kind-Zeile SIGPIPE, die Pipeline 141 – `panel-neustart` verweigerte („laufendes Panel ist nicht 0.2.3“), `art()` hätte „einfach“ geliefert. Dasselbe Muster steckte in `laufend` selbst (`tr … | grep -qx`). Die Probe hatte `panel-neustart` nur im verweigerten Probemodus geprüft. `panel-024-integrieren.sh` bleibt byte-gleich (Rückfallpfad → 0.2.3).

`panel-025-integrieren.sh` (neue Datei, Ausgang „alt“ = 0.2.4 `448ded49`, Rückfall `zurueck` → 0.2.4):
- liest die Panel-Prozesse einmal in eine Variable (Umgebung und Befehlszeile je Prozess ganz gelesen, Prüfung mit `case`), keine Pipe mit `grep -q`;
- `lage()` bewertet die Liste: eine Überwachung mit höchstens einem Panel (Art `ueberwacht`, alte PID = Wachhund) oder ein Panel allein (`einfach`); alle aus einer erlaubten Fassung, sonst verweigert;
- `panel-neustart` von laufendem 0.2.3 oder 0.2.4, `panel-neustart-zurueck` von laufendem 0.2.5, jeweils mit gleicher Art;
- `panel-neustart-pruefen`: nur lesend, meldet alte PID, Art und den Aufruf von `tausch-panel-3.py` (unverändert, `5974d5a7`);
- in der Probe ersetzt `QP25_LAUFEND` die Prozessliste, und `panel-neustart(-zurueck)` ist ein Trockenlauf. Probe 60/60, darunter Wachhund mit Kind (beide Reihenfolgen), Wachhund ohne Kind, Panel allein, zwei Wachhunde, Wachhund mit zwei Panels, fremde und gemischte Fassungen, leere Liste, Trockenläufe in beide Richtungen.

Integration: `integration/panel-025/` (Kopie von `~/projects/panel-024-arbeit/integration-025`).
