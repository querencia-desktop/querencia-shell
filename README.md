# Querencia Shell

Panel, Arbeitsfläche und Kürzelübersicht von Querencia Desktop: die
Bedienoberfläche im Stil von GNOME 2 auf dem Wayland-Compositor labwc. Teil von
[Querencia Desktop](https://querencialinux.org) – einer
klassischen Desktopumgebung im GNOME-2-Stil für Wayland (labwc, Rust, GTK 4).
Installiert wird es zusammen mit Querencia Desktop aus der APT-Quelle;
Anleitung auf <https://querencialinux.org>.

*English summary:* The panel, desktop surface and shortcut overview of
Querencia Desktop, modelled on GNOME 2 and running on the Wayland compositor
labwc. Alpha software; documentation is in German.

> **Alpha.** Die Bausteine sind in virtuellen Maschinen aus der APT-Quelle
> geprüft, noch nicht auf echter Hardware. Keine Sicherheits- oder
> Datenverlustgarantie.

## Zweck

- **Panel** (`querencia-panel`): oben Anwendungen, Orte, System, Uhr mit
  Kalender, Benachrichtigungsglocke sowie Lautstärke- und Netzwerk-Applet;
  unten Fensterliste, „Arbeitsfläche anzeigen“ und Arbeitsflächenumschalter.
  Beide Panels sind `wlr-layer-shell`-Ebenen und nehmen nie selbst den
  Tastaturfokus.
- **Arbeitsfläche** (`querencia-desktop`): Hintergrund, Symbole für den
  Ordner `Schreibtisch`, Laufwerke und Papierkorb, Kontextmenü, Umbenennen,
  Raster und Mehrfachauswahl.
- **Dateien** (`querencia-dateien`, Beta, seit Alpha 3 der Dateimanager): klassischer Dateimanager nach dem
  Vorbild von Nautilus 2.30 (Reiter, kompakte Ansicht, Miniaturen, Suche,
  Netzwerkorte); dazu die Bibliotheken `querencia-dateiarbeit`
  (Dateioperationen, Laufwerke, Papierkorb) und `querencia-oberflaeche`
  (gemeinsame GTK-Bausteine von Panel, Arbeitsfläche und Dateimanager).
- **Tastenkürzel** (`querencia-kuerzel`): Übersicht der aktiven labwc-Kürzel
  und der Kürzel der Querencia-Programme.
- Bibliotheken `querencia-lautstaerke` (PulseAudio-Protokoll, auch
  pipewire-pulse) und `querencia-netzwerk` (NetworkManager über D-Bus, sonst
  lesend aus dem Kernel); beide haben ein kleines Prüfprogramm gleichen Namens.
- Daten: labwc-Thema `Querencia-Klassisch` (`data/themes/`), die
  Hintergrundbilder der Fassung 0.1 (`data/hintergruende/`), der
  Sitzungsstarter der Entwicklungsmaschine (`session/`).
- Bewusst **nicht** Teil dieses Repos: der Compositor labwc (unverändert,
  wird als Prozess gestartet), das GTK-Thema (`querencia-thema`), die
  Symbole (`querencia-symbole`), die Hintergründe ab 0.2
  (`querencia-hintergruende`), die Debian-Pakete (`querencia-packaging`).

Anwendungskennungen (App-IDs): `org.querencia.Panel` (bis Panel 0.2.3
`org.example.Querencia.Panel`), `org.querencia.Desktop`,
`org.querencia.Dateien`, `org.querencia.Kuerzel` · Debian-Paket: `querencia-shell`

## Stand

| | |
|---|---|
| Version | Panel 0.3.2 (Tag `panel/0.3.2-2`), Arbeitsfläche 0.3.0 (Tag `desktop/0.3.0`), Dateien 0.2.1 Beta (Tag `dateien/0.2.1`; löst Querencia Files ab), Dateiarbeit 0.1.1 (Tag `dateiarbeit/0.1.1`), Kürzel 0.2.2 (Tag `kuerzel/0.2.2-2`) |
| Letzte Prüfung | Stand der Quelle vom 24.10.2026: fmt, Clippy und Unit-Tests der Crates; isolierte GUI-Prüfungen je Baustein mit den Werkzeugen unter `tools/` |
| Bekannte Lücken | `session/` und `tools/` enthalten Pfade der Entwicklungsmaschine (neutral `/home/user`) und sind nicht allgemein einsetzbar |
| Getestet auf | Ubuntu 26.04, labwc 0.9.8, GTK 4.22, gtk4-layer-shell 1.3, Rust 1.93 |
| Nicht getestet | andere Distributionen und Compositoren, HiDPI, Mehrmonitor |

Änderungen je Baustein: [crates/querencia-panel/CHANGELOG.md](crates/querencia-panel/CHANGELOG.md),
[crates/querencia-desktop/CHANGELOG.md](crates/querencia-desktop/CHANGELOG.md),
[crates/querencia-kuerzel/CHANGELOG.md](crates/querencia-kuerzel/CHANGELOG.md).

## Bauen

Voraussetzungen (Ubuntu 26.04):

```bash
sudo apt install build-essential pkg-config rustc cargo libgtk-4-dev libgtk4-layer-shell-dev libwayland-dev
```

```bash
cargo build --release --locked
```

Die Programme liegen danach unter `target/release/`. Das Panel braucht einen
Compositor mit `zwlr_layer_shell_v1` und labwc 0.9.4 oder neuer (Ubuntu 26.04
liefert 0.9.3; damit öffnen die Menüs, bekommen aber keine Tastatur).

## Prüfen

```bash
cargo fmt --all --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```

Diese drei Befehle laufen ohne Bildschirm und sind auch die CI
(`.github/workflows/ci.yml`). Die GUI-Prüfungen laufen in einer abgeschotteten
Sitzung (eigener labwc ohne Bildschirm, eigenes `HOME` und
`XDG_RUNTIME_DIR`), niemals in der laufenden Desktopsitzung; die Werkzeuge
liegen unter `tools/` (Einstieg `tools/isolierte-gui-pruefung/`).
Arbeitsweise insgesamt: [CONTRIBUTING.md](CONTRIBUTING.md).

## Vorbild und Abgrenzung

Vorbild für Bedienung und Aussehen sind **gnome-panel 2.30 und die
Nautilus-Arbeitsfläche aus GNOME 2 (Ubuntu 10.04)**. Querencia übernimmt davon
die Bedienideen (Menüs, Applets, Anordnung), **keinen Quellcode, keine
Texte, keine Grafiken**. Das Projekt ist mit GNOME und dessen Autoren weder
verbunden noch von ihnen unterstützt.

Abweichungen vom Vorbild: Wayland statt X11, labwc statt Metacity; die
Fensterliste spricht labwc über `wlr-foreign-toplevel-management`.
Hintergründe und Entscheidungen: Dokumente unter [docs/](docs/).

## Lizenz

GPL-3.0-or-later, siehe [LICENSE](LICENSE). Copyright 2026 Querencia Desktop
contributors; Urheberhinweis und die Lizenz der Hintergrundbilder in
[COPYRIGHT](COPYRIGHT).

Name und Logo „Querencia“ sind keine freien Inhalte: [MARKE.md](MARKE.md).

## Fehler melden

Fehler, Wünsche und Fragen zu Querencia gehen an die gemeinsame Meldestelle:
<https://github.com/querencia-desktop/querencia/issues/new/choose>.
Sicherheitslücken bitte nicht öffentlich melden, siehe [SECURITY.md](SECURITY.md).
