# HMP-08 – Desktop-Hintergrund (querencia-desktop)

Stand 2026-10-16. Neuer Workspace-Crate `crates/querencia-desktop`, Basis HMP-07.

## Entscheidung

- **Eigene Komponente statt Fremdprogramm.** Auf der VM gibt es weder
  `swaybg` noch `wbg`, `swww` oder `/usr/share/backgrounds`; installieren ist
  nicht erlaubt. labwc 0.9.8 zeichnet selbst keinen Hintergrund.
  `querencia-desktop` ist zugleich die Grundlage für HMP-09
  (Desktop-Symbole, Kontextmenü): Hintergrund und Symbole kommen später aus
  **einem** Prozess und **einer** Konfiguration, es gibt keine zwei
  konkurrierenden Hintergrunddienste.
- **Layer-Shell, Ebene `background`**, eine Fläche pro Monitor, an allen vier
  Kanten verankert, `exclusive_zone = -1` (deckt auch die Panelbereiche, die
  Panels liegen auf der Ebene `top` darüber), `keyboard_mode = none`,
  Namespace `querencia-desktop`.
- **Keine Eingaben in HMP-08:** Die Eingaberegion ist leer. Klicks auf den
  Desktop erreichen weiter labwc (Wurzelmenü), genau wie ohne Hintergrund.
  HMP-09 gibt dem Desktop den Zeiger nur dort, wo er ihn braucht.
- **Eine Instanz** pro Sitzung (GApplication-ID `org.querencia.Desktop`),
  ein zweiter Start beendet sich sofort.
- Gleiche Crates und Versionen wie das Panel (gtk4 0.11, gtk4-layer-shell
  0.8.1); `Cargo.lock` bekommt nur den eigenen Eintrag, kein Fremdpaket.

## Bild „Querencia Blau“

- Eigenes, reproduzierbares Bild:
  - `tools/hintergrund/erzeugen.py`, nur Python + Pillow, fester Startwert;
  - Ergebnis `data/hintergruende/querencia-blau.png`, 2560×1440, 16:9.
- Inhalt:
  - tiefes Blau `#0a2454` → `#1c4e96` (senkrechter Verlauf);
  - weiches Licht oben links;
  - drei sanfte, geschwungene Lichtbänder in der unteren Hälfte;
  - leichtes festes Rauschen gegen Farbstufen.
- Keine Schrift, keine Logos, kein historisches Bild.

## Konfiguration und Persistenz

- `~/.config/querencia/desktop.ini` (XDG-Konfigurationsordner), Gruppe
  `[Background]`:
  - `File=`: leer bedeutet das eingebaute Bild; `~/` wird aufgelöst.
  - `Mode=`: `zoom` (Standard: füllt, schneidet zu, verzerrt nie), `scaled`
    (ganzes Bild, Ränder in der Farbe) oder `stretched` (wie in GNOME 2
    wählbar, verzerrt).
  - `Color=#rrggbb`: Randfarbe und Rückfall.
- Die Datei wird überwacht. Änderungen wirken sofort, ein Neustart ist nicht
  nötig. Den späteren Menüeintrag „Hintergrund der Arbeitsfläche ändern“
  (HMP-09) schreibt nur diese Datei (`config::render`).
- Fehlerhafte Werte fallen einzeln auf den Standard zurück und werden ins Log
  geschrieben. Ein unlesbares Bild führt zu „Querencia Blau“, wenn auch das
  fehlt, zur Farbe.
- Das eingebaute Bild wird gesucht unter
  `$XDG_DATA_DIRS/querencia/hintergruende/` und danach im Quellbaum
  (Entwicklungsbuild).

## Startintegration (vorbereitet, im Baum, nicht in der Runtime)

- `session/querencia-session`: setzt `QUERENCIA_DESKTOP` und rotiert
  `desktop.log`.
- `session/querencia/labwc/autostart` startet den Hintergrund nur, wenn das
  Binary existiert. Die Sitzung läuft also auch ohne ihn.
- Bei der Übernahme in `querencia-runtime` **nur diese Zeilen** einfügen.
  Keine Dateien kopieren: Die Runtime-`environment` enthält die lokale
  Zeile `XKB_DEFAULT_LAYOUT=de`.

## Tests

- **Unit (7):** Standardwerte, Lesen/Rückfall/Gruppen, `~`, Rundreise
  `render`↔`parse`, Farben, keine Verzerrung und volle Bedeckung bei
  1920×1080, 1600×900, 1366×768, 1280×720, 1280×1024, 1080×1920,
  `scaled`/`stretched`, Suchpfade des Bildes.
- **Isoliert** mit `tools/isolierte-gui-pruefung/pruefen-hintergrund.sh`:
  - eigener labwc 0.9.8, echtes Panel, echtes Testfenster;
  - Auflösungen über das neue Testwerkzeug `qausgabe` (wlr-output-management,
    verweigert außerhalb der Isolation);
  - 11 Bilder, Auswertung mit `auswerten-hintergrund.py`: Sollbild aus dem
    eingebauten Bild nach derselben Platzierungsregel, Vergleich an bis zu 192
    Punkten außerhalb von Panels und Fenster;
  - geprüft: Panels über dem Hintergrund, Testfenster über dem Hintergrund,
    Rechtsklick öffnet labwcs Wurzelmenü, alle vier Auflösungen plus
    1280×1024, `desktop.ini` → `scaled` mit eigener Farbe sofort wirksam,
    unlesbares Bild → Rückfall.
- **Mutationen:** `hmp-08/mutationen.json` (5 Unit, 6 GUI).
