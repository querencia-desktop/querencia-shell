# M1.0 – Testprotokoll erstes GTK4-Panel

**Datum:** 2026-10-13
**Umgebung:** Proxmox-Entwicklungs-VM `entwicklungs-vm`, Ubuntu 26.04.1 LTS,
labwc 0.9.3 auf tty2 als Benutzer `user`, Ausgabe `Virtual-1`
1280×800, Software-Rendering über `kms_swrast`.
**Ergebnis:** Alle acht Abnahmepunkte bestanden.

## Abhängigkeiten

| Crate | Anforderung | gesperrt |
|---|---|---|
| gtk4 | 0.11 | 0.11.5 |
| gtk4-layer-shell | 0.8.1 | 0.8.1 |
| gtk4-layer-shell-sys | transitiv | 0.6.1 |
| glib / gio | transitiv | 0.22.10 |

`Cargo.lock` enthält 73 Pakete, alle von crates.io, keine
Git-Abhängigkeiten. Systembibliotheken: GTK 4.22.4,
gtk4-layer-shell 1.3.0, GLib 2.88.0.

## Build-Prüfungen

| Befehl | Ergebnis |
|---|---|
| `cargo fmt --all --check` | ok |
| `cargo check` | ok, keine Warnungen |
| `cargo build` | ok, keine Warnungen |
| `cargo test` | ok, 0 Tests vorhanden |
| `cargo clippy -- -D warnings` | nicht ausgeführt, Clippy nicht installiert |

Log: `docs/assets/m1/m1-build.log`.

## Panel-Konfiguration

Layer `Top`, Anker oben/links/rechts, automatische Exclusive Zone,
Namespace `classic2-panel`, Tastaturmodus `None`, Höhe 34 px.
Inhalt: links `Anwendungen`, `Orte`, `System` als Labels, rechts
`Classic2 – GTK4/Wayland`.

## Grafische Abnahme

Gemessen in den QEMU-Screendumps an Pixelzeilen.

| # | Prüfung | Ergebnis | Beleg |
|---|---|---|---|
| 1 | Panel erscheint oben auf `Virtual-1` | bestanden | `m1-01-panel.png` |
| 2 | volle Bildschirmbreite | bestanden, Panelfarbe bei x=0 und x=1279 | `m1-02-maximiert.png` |
| 3 | Beschriftungen korrekt | bestanden | `m1-01-panel.png` |
| 4 | foot-Fenster parallel nutzbar | bestanden | `m1-03-normal.png` |
| 5 | maximiertes Fenster respektiert Panel | bestanden, Panel y=0–33, Fenster ab y=34 bis 799 | `m1-02-maximiert.png` |
| 6 | Panel lässt sich sauber beenden | bestanden, `gapplication action … quit` → Exit 0 | `m1-panel-run.log` |
| 7 | reservierter Bereich wird freigegeben | bestanden, maximiertes Fenster danach ab y=0 | `m1-04-freigegeben.png` |
| 8 | labwc stabil | bestanden, PID 18722 über alle Tests unverändert | – |

Zusatzprüfungen:

- **Zweitstart:** Ein zweiter Aufruf endet mit Exit 0 und erzeugt kein
  zweites Panel; es bleibt genau ein Prozess. Log:
  `m1-second-launch.log`.
- **Zentrierung:** labwc platziert ein neues normales Fenster mit Panel
  17 px tiefer als ohne Panel, also um die halbe Panelhöhe. Das bestätigt
  die reservierte Zone unabhängig vom Maximieren.
- **SIGTERM:** Abbruch per `kill` beendet das Panel mit Exit 143; die
  Zone wird trotzdem sofort freigegeben. Log: `m1-panel-sigterm.log`.

## Bekannte Probleme und Workarounds

- **EGL-Warnung im Log.** `libEGL warning: egl: failed to create dri2
  screen` erscheint zweimal beim Start. Ursache: Die VM hat `vga: virtio`
  ohne 3D-Beschleunigung, Mesa fällt auf Software-Rendering zurück. Für
  das Panel ohne Folgen. 3D wäre über `virtio-gl` auf dem Host möglich und
  ist ein eigenes Thema.
- **Kein Signal-Handler.** In gtk-rs 0.22 liegt die Unix-Signal-API im
  separaten Crate `glib-unix`. Um bei zwei direkten Abhängigkeiten zu
  bleiben, gibt es stattdessen die Aktion `app.quit`. Workaround für ein
  sauberes Ende: `gapplication action org.example.Classic2.Panel quit`.
- **`gapplication list-actions` meldet einen Fehler.** Es sucht eine
  `.desktop`-Datei zur App-ID, die es noch nicht gibt. `gapplication
  action` funktioniert trotzdem.
- **App-ID ist Platzhalter.** `org.example.Classic2.Panel` wie im
  Implementierungsplan; vor einer Veröffentlichung festlegen.
- **Keine Unit-Tests.** Der Prototyp enthält nur UI-Aufbau; `cargo test`
  baut die Testziele, führt aber 0 Tests aus.
- **Clippy fehlt.** Das Paket `rust-clippy` 1.93.1 steht in den
  Ubuntu-Quellen bereit, wurde aber nicht installiert.
- **Start von außen.** Aus SSH heraus braucht der Prozess
  `XDG_RUNTIME_DIR`, `WAYLAND_DISPLAY` und `DBUS_SESSION_BUS_ADDRESS`
  der Sitzung, siehe README.

## Testumgebung, Hinweise

- Die labwc-Sitzung läuft als transiente Unit `labwc-console.service`,
  gestartet mit `systemd-run -p PAMName=login -p TTYPath=/dev/tty2`. Sie
  ist nicht dauerhaft eingerichtet und verschwindet beim Neustart.
- Für `cargo fmt` wurde das Systempaket `rustfmt` 1.93.1 nachinstalliert.
- Screendumps entstehen auf dem Proxmox-Host per
  `qm monitor 114` → `screendump`.

## Dateien

`docs/assets/m1/`:

| Datei | Inhalt |
|---|---|
| `m1-01-panel.png` | Panel allein |
| `m1-02-maximiert.png` | Panel mit maximiertem foot |
| `m1-03-normal.png` | Panel mit normalem foot |
| `m1-04-freigegeben.png` | maximiertes foot nach Beenden des Panels |
| `m1-build.log` | fmt, check, build, test |
| `m1-panel-run.log` | Hauptlauf inkl. Exit-Code |
| `m1-second-launch.log` | Zweitstart |
| `m1-panel-sigterm.log` | SIGTERM-Test |
