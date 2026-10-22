# Panel 0.2.3 – Absturz beim ersten Öffnen von Anwendungen, Überwachung

Stand 22.10.2026. Zweig `panel-023` (auf `panel-022`), Tag `panel/0.2.3`,
Integrationsstand `panel/0.2.3-int`. Änderungen: `crates/querencia-panel/CHANGELOG.md`.

## 1. Befund

Bei der ersten Anmeldung in einem frisch aus dem Abbild installierten System
verschwanden beide Panels beim ersten Öffnen von Anwendungen → Zubehör.
Kernel-Protokoll: `pool-4[…]: segfault at 60 … in libpangocairo-1.0.so.0.5700.0`;
`panel.log` ohne Fehler; das Panel wurde nicht neu gestartet. In zwölf
weiteren Versuchen auf derselben und einer zweiten Maschine trat es nicht
wieder auf.

Lesart: Ein Thread aus GLibs Thread-Pool (`pool-N`) griff über eine
Pango-Cairo-Fontmap auf einen Nullzeiger plus 0x60 zu. Im Panel selbst
startet kein Code Pango in einem Nebenfaden; Kandidat ist GTK, das beim
Aufklappen eines Menüs die Symbole im Thread-Pool lädt. Pango braucht dabei
nur ein SVG-Symbol mit Text: librsvg setzt Text über Pango-Cairo.

Zwei Symbole in Zubehör haben Text im SVG: `org.querencia.Rechner.svg` und
`org.querencia.Kuerzel.svg` (Querencia-Thema und, aus den Programmpaketen,
hicolor). Das Querencia-Thema liefert für beide auch 16-px-PNGs; GTK nimmt
dann die PNGs. Das SVG wird nur gerendert, wenn GTK beim ersten Aufklappen
nicht im Querencia-Thema sucht (z. B. bevor das Symbolthema der Sitzung
gesetzt ist) oder ohne die PNGs.

## 2. Versuche (isoliert, eigener labwc, Bus, HOME)

| Versuch | Ergebnis |
|---|---|
| 200 Prozesse, je 4 Fäden rendern beide SVGs gleichzeitig als erste Pango-Nutzung (GdkPixbuf/librsvg, `werkzeug/svg-faeden.py`) | 0 Abstürze |
| 30 Kaltstarts des Panels, je leerer `XDG_CACHE_HOME`, Anwendungen → Zubehör 0,5 s nach dem Start | 0 Abstürze |
| dasselbe, Rechner und Kürzel nur als SVG (PNGs im Profil entfernt) | 0 von 30 |
| dasselbe, ohne Pause, 100 Kaltstarts | 0 von 100 |

Nicht nachgestellt (0 von 160 Kaltstarts). Deshalb keine Codeänderung an der
Ursache. Vorsorge außerhalb des Panels: die beiden SVGs mit Text in Pfade
umwandeln (Symbolthema und Programmpakete); dann braucht kein Symbol Pango.

## 2a. Zweiter Absturz: live, Panel 0.2.2, 22.10. 04:07:24

Das live getauschte Panel 0.2.2 lief ab 03:56:02 und endete nach elf
Minuten. Letzte Zeile im Protokoll:
`GLib-GObject-CRITICAL: object_ref: assertion '!object_already_finalized' failed`;
Kernel: `segfault at 50 … in libgtk-4.so.1.2200.4[1c5e1e]`, Hauptthread.

Die Stelle (Disassemblierung von `libgtk-4.so.1.2200.4`, zwischen
`gtk_image_new_from_pixbuf` und `gtk_icon_theme_lookup_icon`): unter einem
globalen Mutex eine Suche in einer Hashtabelle, auf das Ergebnis
`g_object_ref`, dann ein Feld bei +0x50. Das gefundene Objekt war schon
freigegeben, `g_object_ref` lieferte NULL. Das ist der Symbol-Cache von
GTK: ein Symbol, das noch im Cache stand, obwohl es freigegeben war.

Vorher im Protokoll: dreimal „Symbolthema neu eingelesen (Anwendungen
geändert)“ (03:58–04:04, Integrationsskripte schrieben Starter vorhandener
Programme um), dazu Fenster auf und zu, Arbeitsflächenwechsel. Jedes
Neueinlesen (`gtk_icon_theme_set_search_path`) verwirft GTKs Cache,
während Bilder der Fensterliste und Menüs neu nachschlagen und GTK Symbole
im Thread-Pool lädt.

Die Speicherspitze des Scopes (989,9 MB in elf Minuten) ist nicht das Panel
allein: Der Tausch startet das Panel in einem eigenen systemd-Scope, und alle
Programme, die aus dem Panel gestartet werden (laut Protokoll u. a. Firefox
und ein Fernzugriffs-Client), liegen im selben Scope. Isoliert bleibt das
Panel über zehn Minuten mit Fensterwechseln, Menüs und Neueinlesen flach
(§4, `speicher.csv`).

Folgerung für 0.2.3: Eine Programmänderung liest das Symbolthema nur noch
neu ein, wenn ein Programmsymbol neu fehlt. Im Livefall hatten die
umgeschriebenen Starter bekannte Symbole; die drei Neueinlesungen wären
entfallen. Verdacht für den ersten Absturz (nicht belegt, das Protokoll des
Testsystems enthält keine Zeile dazu): Nach der ersten Anmeldung ändern sich
die installierten Programme mehrfach (Caches werden angelegt), das Panel las
das Thema neu ein, während GTK die Menüsymbole lud.

## 3. Überwachung `querencia-panel --ueberwacht`

Modul `ueberwachung.rs`. Der Prozess mit `--ueberwacht` startet kein GTK und
verbindet sich mit keinem Display. Er startet dasselbe Programm ohne die
Option als Kind und wartet über GLibs Child-Watch auf dessen Ende.

| Ende des Panels | Folge |
|---|---|
| Signal SIGSEGV, SIGABRT, SIGBUS, SIGILL, SIGFPE, SIGTRAP, SIGSYS oder Status 101 (Rust-Panic) | Neustart nach 1 s, höchstens 3 in 10 Minuten (gleitendes Fenster) |
| jedes andere Ende (Status 0/1, SIGTERM, SIGKILL …) | kein Neustart, Überwachung endet mit demselben Status |
| Überwachung bekommt SIGTERM, SIGINT oder SIGHUP | an das Panel weitergegeben, nach 5 s SIGKILL; kein Neustart |
| kein Wayland-Socket (`XDG_RUNTIME_DIR`/`WAYLAND_DISPLAY`) oder `LABWC_PID` beendet | kein Neustart |
| vierter Absturz im Fenster | kein Neustart, Protokollzeile „3 Neustarts in 10 Minuten erreicht“ |

Protokollzeilen (Standardfehler, also `panel.log`):

```
querencia-panel: Überwachung: aktiv (PID 752001, höchstens 3 Neustarts in 10 Minuten)
querencia-panel: Überwachung: Panel gestartet (PID 752007)
querencia-panel: Überwachung: Panel PID 752007 abgestürzt durch Signal 11 (SIGSEGV) nach 12 s – Neustart 1 von 3 in 10 Minuten
querencia-panel: Überwachung: Panel PID … abgestürzt durch Signal 11 (SIGSEGV) nach … s – 3 Neustarts in 10 Minuten erreicht, kein weiterer Neustart
```

Exit-Status der Überwachung: der des Panels, bei einem Signal 128 + Nummer.

Rusts Laufzeit fängt SIGSEGV ab, um Stapelüberläufe zu erkennen. Bei einem
anderen Speicherfehler setzt sie den Handler zurück, der Befehl fault
erneut, und der Prozess endet mit SIGSEGV. Darum sieht die Überwachung einen
echten Speicherfehler als Signal 11. Ein von außen geschicktes SIGSEGV fault
nicht erneut; die Prüfung schickt es deshalb zweimal.

### Einschalten (Sitzungsstarter)

Ohne `--ueberwacht` läuft das Panel wie bisher. Der Sitzungsstarter
(`session/querencia/labwc/autostart`, gehört der Sitzung) gibt die Option nur
an Panels, die sie kennen:

```sh
if "$QUERENCIA_PANEL" --faehigkeiten 2>/dev/null | grep -qw ueberwacht; then
    gtk_exec "$QUERENCIA_PANEL" --ueberwacht </dev/null >>"$QUERENCIA_STATE_DIR/panel.log" 2>&1 &
else
    gtk_exec "$QUERENCIA_PANEL" </dev/null >>"$QUERENCIA_STATE_DIR/panel.log" 2>&1 &
fi
```

`--faehigkeiten` antwortet ab 0.2.3 vor GTK und Portal. Die Record-Datei
nennt dann die Überwachung; der Aufräumer beim Abmelden beendet sie mit
SIGTERM, sie gibt das an das Panel weiter. Ältere Panels kennen
`--faehigkeiten` nicht und beenden sich mit einer Fehlermeldung; dann bleibt
es beim bisherigen Aufruf.

`tausch-panel.py` (Live-Tausch) erwartet genau einen Panel-Prozess der
Sitzung und verweigert, wenn eine Überwachung läuft (dann sind es zwei).

## 4. Prüfung

Arbeitsordner `~/projects/querencia-panel-023` (Kopie: `integration/panel-023/`).
Release `47ea50e6…` aus `a706716`, auf dem Merge-Stand mit main frisch gebaut
byte-gleich.

- `cargo fmt --check`, `build`, `clippy --all-targets -D warnings`, `test`
  (Workspace mit Desktop 0.2.3: 483; Panel 302, 1 ignoriert; neu 6 Tests in
  `ueberwachung.rs`, 1 in `icon_reload.rs`).
- Isoliert `ueberwacht` (20/20): Neustarts nach SIGSEGV, SIGABRT, SIGILL mit
  Protokollzeilen „Neustart 1/2/3 von 3“, beim vierten Absturz kein
  Neustart (Exit 139), SIGTERM ans Panel kein Neustart (143), SIGTERM an die
  Überwachung beendet beide (143), ohne labwc kein Neustart.
- Isoliert `absturz`: 20 Kaltstarts mit Anwendungen → Zubehör, 0 Abstürze.
- Isoliert `fotos` de hell, de dunkel, en dunkel: je 9/9 (Regression 0.2.2).
- Dauerläufe 10 min (Fenster auf/zu, Starter anlegen/entfernen, Menüs):

| Lauf | Panel | Runden | Neueinlesungen | RSS Anfang → Ende | Absturz |
|---|---|---|---|---|---|
| dauer-021 | 0.2.1 | 123 | 123 | 53,5 → 55,8 MB | nein |
| dauer-022 | 0.2.2 | 123 | 123 | 53,6 → 55,8 MB | nein |
| sturm-022 (Takt 0,3 s) | 0.2.2 | 277 | 277 | 53,5 → 54,7 MB | nein |
| dauer-023 | 0.2.3 | 123 | 0 | 53,7 → 53,6 MB | nein |
| sturm-023 (Takt 0,3 s) | 0.2.3 | 279 | 0 | 53,6 → 53,6 MB | nein |

  Kein Speicherleck; der GTK-Absturz ließ sich mit 523 Neueinlesungen unter
  Last nicht erzwingen (selten, zeitabhängig). 0.2.3 entfernt den Auslöser,
  der live vorlag, und fängt einen Absturz mit der Überwachung ab.
