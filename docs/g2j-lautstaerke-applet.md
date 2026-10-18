# G2-J: Lautstärke-Applet und Lautstärkeregler

Stand 18.10.2026, Zweig `lautstaerke-applet`, Tag `panel/lautstaerke-0.1`.
Ausführlicher Bericht mit Bildern: .

## Bedienung (nach dem GNOME-2-Lautstärkeregler)

Das Applet zeigt die Lautstärke des Standard-Ausgabegeräts als eigenes
Symbol in vier Stufen: stumm (auch 0 %), leise (1–33 %), mittel (34–66 %),
laut (ab 67 %). Grauer Lautsprecher, Wellen in Querencia-Orange `#F07746`.

| Eingabe | Wirkung |
|---|---|
| Linksklick | Popup mit senkrechtem Regler 0–100 %, Prozentanzeige, „Stumm“; Tastatur: Pfeile ±1 %, Bild auf/ab ±5 %, Escape schließt |
| Mausrad | ±5 % je Raste, höchstens 100 % (über 100 %, von anderswo gesetzt, geht es nur abwärts) |
| mittlere Taste | stumm ein/aus |
| Rechtsklick | „Stumm“ (Häkchen), „Lautstärkeregler öffnen …“, „Aus dem Panel entfernen“ |
| Tooltip | „Lautstärke: 45 %“ bzw. „Lautstärke: stumm (45 %)“, darunter „Gerät: …“ |

Ohne Audiodienst ist das Symbol ausgegraut, der Tooltip sagt „Kein
Audiodienst“, Regler und Stumm tun nichts. Erscheint der Dienst später,
verbinden sich Applet und Regler von selbst.

## panel.ini

Applet-Typ `volume` (einmal je Layout, entfernbar, eigenes Menü). Das
Standardlayout enthält ihn nicht. Der GNOME-2-Platz links neben der Uhr:

```ini
[Panel top]
End=volume-1;clock-1

[Applet volume-1]
Type=volume
```

„Zum Panel hinzufügen …“ (Rechtsklick ins rechte Drittel des oberen Panels)
legt genau diese Einträge an. Eine zweite Instanz lehnt der Dialog ab; ein
gespeichertes Layout mit zwei Instanzen behält nur die erste.

## Technik

- Crate `crates/querencia-lautstaerke` (Bibliothek und Programm
  `querencia-lautstaerke`). Das Panel nutzt die Bibliothek.
- Audiosystem über das PulseAudio-Protokoll mit libpulse 17, zur Laufzeit
  per `dlopen("libpulse.so.0")` geöffnet: kein -dev-Paket, kein Absturz
  ohne Bibliothek oder Dienst. PipeWire spricht das Protokoll über
  `pipewire-pulse`.
- Ereignisse statt Polling: Subscribe auf Sinks, Quellen, Ströme und Server;
  je Ereignis wird nur die betroffene Liste neu gelesen. Ohne Dienst
  überwacht ein GIO-Dateimonitor `$XDG_RUNTIME_DIR/pulse/native`.
- Eine Verbindung je Prozess (`service::get`), geteilt von Applet und
  Fenstern. Logzeilen beginnen mit `querencia-panel:` bzw.
  `querencia-lautstaerke:`.
- Panel: `volume_applet.rs`; Registrierung in `applets.rs` (ein Block);
  `AppletType::menu` gibt Applets ein eigenes Rechtsklickmenü (auch für
  G2-M gedacht).

## Lautstärkeregler `querencia-lautstaerke`

Fenster mit Menüleiste (Datei: Schließen, Beenden; Hilfe: Info) und den
Reitern Wiedergabe (Ausgabegerät wählen = Standard setzen, Regler, Stumm),
Aufnahme (Eingabegeräte ohne Monitore) und Anwendungen (je Strom Regler und
Stumm). App-ID und Desktop-ID `org.querencia.Lautstaerke`, Menüeintrag
„Lautstärkeregler“ unter System → Einstellungen. `--zustand` gibt Geräte und
Ströme als Text aus (Diagnose).

## Prüfung

- `cargo test`: Panel 180 + 1 ignoriert, Lautstärke 16, Desktop 74.
- Isoliert: `tools/lautstaerke-pruefung/` (eigener headless labwc). Weil auf
  der VM kein Audiodienst installiert ist, spricht `qpulse.py` das native
  PulseAudio-Protokoll; libpulse läuft echt dagegen. `qpactl.py` steuert ihn
  wie pactl (`load-module module-null-sink`, `set-default-sink` …).
  `endlaeufe.sh <panel> <mixer>` fährt alles: G2-J bei 1024×768 und
  1920×1080 (mit Testserver und ohne Audiodienst) und die W2-Regression.

## Grenzen

- Kein echter Audiodienst auf der VM: geprüft gegen den Protokoll-Testserver,
  nicht gegen PipeWire. Live zeigt das Applet „Kein Audiodienst“, bis
  PipeWire/pipewire-pulse installiert ist.
- Keine Ports und Profile, keine Pegelanzeige, keine Balance-Regler, keine
  Verstärkung über 100 % (der Wert wird angezeigt, aber nicht gesetzt).
- Panel und Regler sprechen das Gerät des Servers an; mehrere Ausgaben
  gleichzeitig (Kombinationsgerät) gibt es nicht.
