# Querencia Desktop

Querencia Desktop (bis R1 „Classic2“, siehe
`docs/querencia-umbenennung.md`) ist eine Neuimplementierung des
GNOME-2-Bedienmodells mit Rust, GTK4 und Wayland. Die Shell läuft als
eigener Prozess auf dem Compositor labwc und nutzt `wlr-layer-shell` für
Panels.

Stand: Panel M1.5b, Sitzung M2.1 (Start über greetd/ReGreet, siehe
unten). Ein Prozess (`querencia-panel`) zeigt zwei Panels im
GNOME-2-Layout: oben die Menüknöpfe `Anwendungen`, `Orte` und `System`
und eine Uhr mit Datum, unten die Fensterliste und der
Arbeitsflächenumschalter. Das Anwendungsmenü zeigt die installierten
Anwendungen nach GNOME-2-Kategorien und startet sie über GIO, per Maus
oder Tastatur.
Es folgt installierten, entfernten und geänderten Anwendungen ohne
Neustart des Panels. Das Orte-Menü öffnet den persönlichen Ordner und die
vorhandenen XDG-Benutzerordner mit der Standardanwendung für Verzeichnisse.
Das System-Menü zeigt die Einstellungsanwendungen (Kategorie `Settings`)
in „Einstellungen“ und, mit zusätzlich `System`, in „Systemverwaltung“;
es startet und aktualisiert sich wie das Anwendungsmenü.
Die Fensterliste zeigt pro Fenster einen Button mit dem Titel, hebt das
aktive Fenster hervor, klammert minimierte ein und bittet labwc per Klick,
das Fenster zu aktivieren. Der Arbeitsflächenumschalter zeigt pro
Arbeitsfläche einen nummerierten Button, hebt die aktive hervor und bittet
labwc per Klick um den Wechsel.
Noch keine Icons, kein Sperren, Abmelden oder Ausschalten, kein Autostart.

Beide Panels nehmen nie selbst den Tastaturfokus
(`KeyboardMode::None`). Das geöffnete Menü bekommt die Tastatur vom
Compositor und gibt sie beim Schließen zurück. Das setzt labwc 0.9.4 oder
neuer voraus. Belege: `docs/m1-1b0-labwc-ab-test.md`.

## Voraussetzungen

Getestet auf Ubuntu 26.04.1 LTS (VM `entwicklungs-vm`):

| Komponente | Version |
|---|---|
| Rust / Cargo | 1.93.1 |
| GTK4 | 4.22.4 |
| gtk4-layer-shell (System) | 1.3.0 |
| labwc | ≥ 0.9.4, getestet 0.9.8 (Eigenbau unter `~/opt/labwc-0.9.8`) |

Pakete: `rustc cargo rustfmt rust-clippy libgtk-4-dev libgtk4-layer-shell-dev labwc`.
Ubuntu 26.04 liefert nur labwc 0.9.3. Damit öffnet das Menü zwar, bekommt
aber keine Tastatur. Die Testsitzung läuft deshalb mit dem Eigenbau 0.9.8;
das System-labwc bleibt unverändert.
Das Panel verlangt einen Compositor mit `zwlr_layer_shell_v1`. Fehlt das
Protokoll, bricht es mit einer Fehlermeldung ab, statt als normales
Fenster zu starten.

## Bauen

```bash
cd ~/projects/classic2
cargo fmt --all --check
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
```

## Starten

Innerhalb einer laufenden labwc-Sitzung, zum Beispiel aus foot:

```bash
./target/debug/querencia-panel
```

Von außen (etwa per SSH) braucht der Prozess die Umgebung der
Sitzung desselben Benutzers:

```bash
export XDG_RUNTIME_DIR=/run/user/$(id -u)
export WAYLAND_DISPLAY=wayland-0
export DBUS_SESSION_BUS_ADDRESS=unix:path=$XDG_RUNTIME_DIR/bus
setsid ./target/debug/querencia-panel > ~/classic2-logs/panel.log 2>&1 < /dev/null &
```

Ein zweiter Start erzeugt keine weiteren Panels, er aktiviert nur die
laufende Instanz.

Anwendungs- und System-Menü lesen die Anwendungen beim Start ein und
danach erneut, sobald GIO (`AppInfoMonitor`) eine Änderung meldet. Ist ein
Menü gerade offen, wird es erst nach dem Schließen neu aufgebaut; das
andere Menü ist davon unabhängig.
`OnlyShowIn`/`NotShowIn` werten gegen `XDG_CURRENT_DESKTOP` aus; die
Querencia-Sitzung verwendet `X-Querencia`, die ältere Classic2-Sitzung
`X-Classic2`. Ohne die Variable sind
`OnlyShowIn`-Einträge verborgen. Namen und Sortierung folgen `LANG` und
`LANGUAGE`, zum Beispiel:

```bash
XDG_CURRENT_DESKTOP=X-Querencia LANG=en_US.UTF-8 LANGUAGE=de ./target/debug/querencia-panel
```

Das Orte-Menü liest `HOME` und die XDG-Benutzerordner nur beim Start;
Änderungen an `user-dirs.dirs` gelten erst nach einem Neustart. Ohne
`user-dirs.dirs` meldet GLib für den Schreibtisch `$HOME/Desktop`; fehlt
der Ordner, wird er ausgelassen. Zum Öffnen braucht es eine
Standardanwendung für `inode/directory` (Dateimanager), sonst steht nur
eine Fehlerzeile im Log.

Die Fensterliste nutzt `zwlr_foreign_toplevel_manager_v1` über eine eigene
Wayland-Verbindung, deren Ereignisse im GTK-Hauptkontext verarbeitet werden.
Bietet der Compositor das Protokoll nicht an, bleibt der Bereich leer. Die
Hervorhebung zeigt nur, was labwc meldet; ein Klick ist eine Anfrage.

Der Arbeitsflächenumschalter nutzt `ext_workspace_manager_v1` ebenfalls über
eine eigene Verbindung und zeigt die Arbeitsflächen der ersten Gruppe.
labwc hat ohne Konfiguration eine Arbeitsfläche; mehr gibt es mit
`<desktops number="N" />` in der `rc.xml` der Sitzung. Die Fensterliste
zeigt weiter die Fenster aller Arbeitsflächen (Protokollgrenze unter
labwc 0.9.8).

## Beenden

Beide Panels gemeinsam und sauber mit Exit-Code 0 über die
GApplication-Aktion:

```bash
gapplication action org.querencia.Panel quit
```

Bis Panel 0.2.3 hieß die Kennung `org.example.Querencia.Panel`; ab 0.2.4
antwortet das Panel dort nur noch mit seiner D-Bus-Schnittstelle (Super+D),
siehe `docs/panel-024-absturzursache-und-kennung.md` §5.

Das ältere `classic2-panel` der Classic2-Laufzeitkopie heißt auf dem
Sitzungsbus weiterhin `org.example.Classic2.Panel`.

`kill` oder Strg+C funktionieren ebenfalls; der Compositor gibt die
reservierten Bereiche auch dann frei, der Exit-Code ist aber 143 bzw. 130.

## Eigenständige Sitzung (TTY-Start)

Hinweis R2: `session/` enthält zwei getrennte Sätze. Classic2
(`classic2-session`, `labwc/`, `classic2.desktop`) bleibt unverändert und
läuft aus der Laufzeitkopie `~/projects/classic2` mit dem
Binary `classic2-panel`. Querencia Desktop (`querencia-session`,
`querencia/labwc/`, `querencia.desktop`) läuft aus der eigenen
Laufzeitkopie `~/projects/querencia-runtime` mit
`querencia-panel`, Zustand unter `~/.local/state/querencia/`. Beide
Starter verweigern den Start, solange eine der beiden Sitzungen läuft.
Die Querencia-Laufzeitkopie und ihre Installation folgen nach der
R2-Freigabe (`docs/querencia-umbenennung.md`); die Beschreibung unten
gilt für den Classic2-Starter, der Querencia-Starter verhält sich gleich.

Der Starter `session/classic2-session` startet labwc 0.9.8 über den
absoluten Pfad `~/opt/labwc-0.9.8/bin/labwc` mit dem Konfigurationsordner
`session/labwc` (`labwc -C`). labwc liest dort `environment` und startet
über `autostart` das Panel, sobald der Wayland-Socket bereitsteht. Starter,
Konfiguration und Panel kommen immer aus dem Sitzungs-Repo
`~/projects/classic2`, nie aus einer Entwicklungskopie.

Nach Anmeldung an einer Konsole (TTY):

```bash
~/projects/classic2/session/classic2-session
```

Der Starter bricht ab, wenn er nicht aus dem Sitzungs-Repo kommt, keine
logind-Sitzung mit Sitz besteht (etwa per SSH), bereits ein labwc oder
`classic2-panel` des Benutzers läuft, die labwc-Version nicht 0.9.8 ist
oder Dateien fehlen. Logs und PID-Dateien liegen unter
`~/.local/state/classic2/` (vorheriger Lauf als `*.log.1`).

Beenden aus der Sitzung über das labwc-Wurzelmenü „Exit“ oder aus jeder
Shell des Benutzers, auch per SSH:

```bash
~/projects/classic2/session/classic2-session --exit
```

labwc leert dann die D-Bus-/systemd-Aktivierungsumgebung, das Panel endet
mit dem Verlust der Wayland-Verbindung; ein übrig gebliebenes Panel beendet
der Starter. Danach steht wieder die Konsolen-Shell bereit.

## Anmeldebildschirm (greetd + ReGreet)

Seit M2.1 ist der Desktop am grafischen Anmeldebildschirm wählbar; der
Sitzungseintrag heißt bis zur Migration R2 weiterhin „Classic2“
(`docs/querencia-umbenennung.md`).
`session/classic2.desktop` ist die Sitzungsdefinition; eine Kopie liegt
unter `/usr/share/wayland-sessions/classic2.desktop` und verweist auf
denselben, unveränderten Starter. greetd (VT 7) startet als Greeter
labwc 0.9.8 mit eigener Konfiguration unter `/etc/greetd/labwc/` und
darin ReGreet (`labwc -C /etc/greetd/labwc -S regreet`); nach der
Anmeldung läuft die Sitzung wie beim TTY-Start, nur mit
`Service=greetd`. Das Beenden mit `--exit` führt zurück zum
Anmeldebildschirm. Es gibt nur eine grafische Sitzung je Benutzer: eine
von Hand gestartete tty2-Referenz und eine Anmeldung über den Greeter
schließen sich gegenseitig aus. Einzelheiten und Installationspfade in
`docs/m2-1-display-manager-test.md`.

## Projektstruktur

```text
querencia-desktop/                 (Arbeitskopie: ~/projects/entwicklungs-vm)
├── Cargo.toml                    Workspace, Edition 2024, MSRV 1.92
├── Cargo.lock                    versioniert
├── README.md
├── crates/
│   └── querencia-panel/
│       ├── Cargo.toml            gtk4 0.11, gtk4-layer-shell 0.8.1, gio-unix 0.22,
│       │                         glib-unix 0.22, wayland-client 0.31, wayland-protocols 0.32,
│       │                         wayland-protocols-wlr 0.3
│       ├── src/
│       │   ├── main.rs           Start, beide Panels
│       │   ├── app_menu.rs       Anwendungsmenü; Aktualisierung und Start über GIO auch fürs System-Menü
│       │   ├── places_menu.rs    Orte-Menü, Öffnen über GIO, Tests
│       │   ├── system_menu.rs    System-Menü (Einstellungen, Systemverwaltung), Tests
│       │   ├── window_list.rs    Fensterliste über Wayland, Tests
│       │   ├── workspace_switcher.rs  Arbeitsflächenumschalter über Wayland, Tests
│       │   ├── clock.rs          Uhr mit Minutentakt und Tests
│       │   ├── apps.rs           Anwendungskatalog über GIO
│       │   └── apps/tests.rs     Tests des Katalogs
│       └── tests/fixtures/app-catalog/  .desktop-Fixtures
├── session/
│   ├── classic2-session          Classic2-Sitzungsstarter (TTY und greetd), --exit zum Beenden
│   ├── classic2.desktop          Classic2-Sitzungsdefinition (wayland-sessions)
│   ├── labwc/                    labwc-Konfiguration der Classic2-Sitzung (labwc -C)
│   │   ├── environment           Sitzungsvariablen
│   │   └── autostart             Panelstart
│   ├── querencia-session         Querencia-Sitzungsstarter, --exit zum Beenden
│   ├── querencia.desktop         Querencia-Sitzungsdefinition (wayland-sessions)
│   └── querencia/labwc/          labwc-Konfiguration der Querencia-Sitzung
│       ├── environment           Sitzungsvariablen (X-Querencia)
│       └── autostart             Panelstart (QUERENCIA_*)
└── docs/
    ├── implementierungsplan-gnome2-gtk4-wayland.md
    ├── m1-panel-test.md          Testprotokoll M1.0
    ├── m1-1a-panel-test.md       Testprotokoll M1.1a
    ├── m1-1b0-menu-focus-test.md Testprotokoll M1.1b-0, Fokusanalyse
    ├── m1-1b0-focus-investigation.md  Untersuchung labwc 0.9.3/0.9.4+
    ├── m1-1b0-labwc-ab-test.md        A/B-Test labwc 0.9.3/0.9.8
    ├── m1-1b0-first-click-investigation.md  Erstklick-Untersuchung
    ├── m1-1b1-anwendungsmenue-vorschlag.md  Vorschlag M1.1b-1
    ├── m1-1b1a-anwendungskatalog-test.md    Testprotokoll M1.1b-1a
    ├── m1-1b1b-anwendungsmenue-test.md      Testprotokoll M1.1b-1b
    ├── m1-1b1c-appinfo-monitor-test.md      Testprotokoll M1.1b-1c
    ├── m1-1b1d-nested-entscheidung.md       Entscheidung M1.1b-1d
    ├── m1-1b1e-icons-entscheidung.md        Entscheidung M1.1b-1e
    ├── m1-2a-orte-menue-test.md             Testprotokoll M1.2a
    ├── m1-3b-fensterliste-test.md           Testprotokoll M1.3b
    ├── m1-4b-arbeitsflaechen-test.md        Testprotokoll M1.4b
    ├── m1-5b-system-menue-test.md           Testprotokoll M1.5b
    ├── m2-0b-sitzungsstart-test.md          Testprotokoll M2.0b
    ├── m2-0c-sitzungsstart-test.md          Testprotokoll M2.0c
    ├── m2-1-display-manager-test.md         Testprotokoll M2.1
    └── assets/
        ├── m1/                   Screenshots und Logs M1.0
        ├── m1-1a/                Screenshots und Logs M1.1a
        ├── m1-1b0/               Screenshots und Logs M1.1b-0
        ├── m1-1b0-ab/            Daten und Werkzeug A/B-Test
        ├── m1-1b0-first-click/   Daten und Werkzeug Erstklick-Untersuchung
        ├── m1-1b1-vorschlag/     Wegwerf-Probe zum Vorschlag M1.1b-1
        ├── m1-1b1a/              Belege M1.1b-1a
        ├── m1-1b1b/              Belege und Testwerkzeug M1.1b-1b
        ├── m1-1b1c/              Belege und Testwerkzeug M1.1b-1c
        ├── m1-1b1e/              Screenshots der Symbol-Probe M1.1b-1e
        ├── m1-2a/                Belege und Testwerkzeug M1.2a
        ├── m1-3b/                Belege und Testwerkzeug M1.3b
        ├── m1-4b/                Belege und Testwerkzeug M1.4b
        └── m1-5b/                Belege und Testwerkzeug M1.5b
```

## Lizenz

Vorläufig `GPL-3.0-or-later` im Cargo-Manifest. Die endgültige Lizenz
wird vor dem ersten öffentlichen Commit festgelegt.
