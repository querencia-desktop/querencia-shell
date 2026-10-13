# Classic2

Classic2 ist eine Neuimplementierung des GNOME-2-Bedienmodells mit Rust,
GTK4 und Wayland. Die Shell läuft als eigener Prozess auf dem Compositor
labwc und nutzt `wlr-layer-shell` für Panels.

Stand: Meilenstein M1.2a. Ein Prozess zeigt zwei Panels im
GNOME-2-Layout: oben die Menüknöpfe `Anwendungen` und `Orte`, das Label
`System` und eine Uhr mit Datum, unten Platzhalter für Fensterliste und
Arbeitsflächen. Das Anwendungsmenü zeigt die installierten Anwendungen
nach GNOME-2-Kategorien und startet sie über GIO, per Maus oder Tastatur.
Es folgt installierten, entfernten und geänderten Anwendungen ohne
Neustart des Panels. Das Orte-Menü öffnet den persönlichen Ordner und die
vorhandenen XDG-Benutzerordner mit der Standardanwendung für Verzeichnisse.
Noch keine Icons im Menü, keine Fensterliste, kein System-Menü, kein
Autostart.

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
./target/debug/classic2-panel
```

Von außen (etwa per SSH) braucht der Prozess die Umgebung der
Sitzung desselben Benutzers:

```bash
export XDG_RUNTIME_DIR=/run/user/$(id -u)
export WAYLAND_DISPLAY=wayland-0
export DBUS_SESSION_BUS_ADDRESS=unix:path=$XDG_RUNTIME_DIR/bus
setsid ./target/debug/classic2-panel > ~/classic2-logs/panel.log 2>&1 < /dev/null &
```

Ein zweiter Start erzeugt keine weiteren Panels, er aktiviert nur die
laufende Instanz.

Das Anwendungsmenü liest die Anwendungen beim Start ein und danach
erneut, sobald GIO (`AppInfoMonitor`) eine Änderung meldet. Ist das Menü
gerade offen, wird es erst nach dem Schließen neu aufgebaut.
`OnlyShowIn`/`NotShowIn` werten gegen `XDG_CURRENT_DESKTOP` aus; die
Classic2-Sitzung verwendet `X-Classic2`. Ohne die Variable sind
`OnlyShowIn`-Einträge verborgen. Namen und Sortierung folgen `LANG` und
`LANGUAGE`, zum Beispiel:

```bash
XDG_CURRENT_DESKTOP=X-Classic2 LANG=en_US.UTF-8 LANGUAGE=de ./target/debug/classic2-panel
```

Das Orte-Menü liest `HOME` und die XDG-Benutzerordner nur beim Start;
Änderungen an `user-dirs.dirs` gelten erst nach einem Neustart. Ohne
`user-dirs.dirs` meldet GLib für den Schreibtisch `$HOME/Desktop`; fehlt
der Ordner, wird er ausgelassen. Zum Öffnen braucht es eine
Standardanwendung für `inode/directory` (Dateimanager), sonst steht nur
eine Fehlerzeile im Log.

## Beenden

Beide Panels gemeinsam und sauber mit Exit-Code 0 über die
GApplication-Aktion:

```bash
gapplication action org.example.Classic2.Panel quit
```

`kill` oder Strg+C funktionieren ebenfalls; der Compositor gibt die
reservierten Bereiche auch dann frei, der Exit-Code ist aber 143 bzw. 130.

## Projektstruktur

```text
classic2/
├── Cargo.toml                    Workspace, Edition 2024, MSRV 1.92
├── Cargo.lock                    versioniert
├── README.md
├── crates/
│   └── classic2-panel/
│       ├── Cargo.toml            gtk4 0.11, gtk4-layer-shell 0.8.1, gio-unix 0.22
│       ├── src/
│       │   ├── main.rs           Start, beide Panels
│       │   ├── app_menu.rs       Anwendungsmenü, Aktualisierung, Start über GIO
│       │   ├── places_menu.rs    Orte-Menü, Öffnen über GIO, Tests
│       │   ├── clock.rs          Uhr mit Minutentakt und Tests
│       │   ├── apps.rs           Anwendungskatalog über GIO
│       │   └── apps/tests.rs     Tests des Katalogs
│       └── tests/fixtures/app-catalog/  .desktop-Fixtures
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
        └── m1-2a/                Belege und Testwerkzeug M1.2a
```

## Lizenz

Vorläufig `GPL-3.0-or-later` im Cargo-Manifest. Die endgültige Lizenz
wird vor dem ersten öffentlichen Commit festgelegt.
