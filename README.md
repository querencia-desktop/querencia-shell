# Classic2

Classic2 ist eine Neuimplementierung des GNOME-2-Bedienmodells mit Rust,
GTK4 und Wayland. Die Shell läuft als eigener Prozess auf dem Compositor
labwc und nutzt `wlr-layer-shell` für Panels.

Stand: Meilenstein M1.1b-0. Ein Prozess zeigt zwei Panels im
GNOME-2-Layout: oben ein Menüknopf `Anwendungen` mit drei Testeinträgen,
die Labels `Orte` und `System` und eine Uhr mit Datum, unten Platzhalter
für Fensterliste und Arbeitsflächen. Das Menü ist per Maus und Tastatur
bedienbar und gibt den Tastaturfokus danach an das vorige Fenster
zurück. Noch kein echtes Anwendungsmenü, keine Fensterliste, kein
Autostart.

Der Tastaturfokus hängt vom Verhalten des Compositors ab. Details und
Einschränkungen: `docs/m1-1b0-menu-focus-test.md`.

## Voraussetzungen

Getestet auf Ubuntu 26.04.1 LTS (VM `entwicklungs-vm`):

| Komponente | Version |
|---|---|
| Rust / Cargo | 1.93.1 |
| GTK4 | 4.22.4 |
| gtk4-layer-shell (System) | 1.3.0 |
| labwc | 0.9.3 |

Pakete: `rustc cargo rustfmt rust-clippy libgtk-4-dev libgtk4-layer-shell-dev labwc`.
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
│       ├── Cargo.toml            gtk4 0.11, gtk4-layer-shell 0.8.1
│       └── src/
│           ├── main.rs           Start, beide Panels, Testmenü
│           └── clock.rs          Uhr mit Minutentakt und Tests
└── docs/
    ├── implementierungsplan-gnome2-gtk4-wayland.md
    ├── m1-panel-test.md          Testprotokoll M1.0
    ├── m1-1a-panel-test.md       Testprotokoll M1.1a
    ├── m1-1b0-menu-focus-test.md Testprotokoll M1.1b-0, Fokusanalyse
    └── assets/
        ├── m1/                   Screenshots und Logs M1.0
        ├── m1-1a/                Screenshots und Logs M1.1a
        └── m1-1b0/               Screenshots und Logs M1.1b-0
```

## Lizenz

Vorläufig `GPL-3.0-or-later` im Cargo-Manifest. Die endgültige Lizenz
wird vor dem ersten öffentlichen Commit festgelegt.
