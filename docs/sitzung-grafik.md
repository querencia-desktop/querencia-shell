# Sitzungsstarter: Software-OpenGL bei zink auf NVK (QD-169)

Stand 23.10.2026, Tag `session/grafik-1`. Betrifft
`session/querencia-session` und die Autostart-Vorgabe
`session/querencia/labwc/autostart`.

## Befund

Auf Rechnern mit NVIDIA-Karte und freiem Treiber (nouveau) läuft OpenGL
in Mesa 26 als zink auf Vulkan (NVK). Auf der Entwicklungs-VM (RTX 3060,
GA106) meldet der Renderer:

```text
zink Vulkan 1.4(NVIDIA GeForce RTX 3060 (NVK GA106) (MESA_NVK))
```

Dort stirbt der WebKit-Web-Prozess von Querencia Nachrichten beim
Bildbetrachter mit „free(): corrupted unsorted chunks“; Firefox friert ein.
Mit `LIBGL_ALWAYS_SOFTWARE=1 MESA_LOADER_DRIVER_OVERRIDE=llvmpipe
GALLIUM_DRIVER=llvmpipe` (llvmpipe, OpenGL auf der CPU) laufen beide.

Panel und Arbeitsfläche zeichnen mit GTK 4 über Vulkan (NVK) und laden kein
OpenGL (`/proc/<pid>/maps`: `libvulkan_nouveau`, kein `libgallium`); sie
sind stabil. labwc selbst setzt die Bilder mit GLES über zink zusammen
(`libgallium`, `libvulkan_nouveau`) und ist ebenfalls stabil.

## Was der Starter tut

1. **Einstellung** aus `~/.config/querencia/grafik.ini`:

   ```ini
   [Grafik]
   Software-OpenGL = automatisch
   ```

   `automatisch` (Vorgabe, auch ohne Datei), `an` (immer Software-OpenGL)
   oder `aus` (nie). Groß- und Kleinschreibung zählen nicht; der Schlüssel
   gilt nur unter `[Grafik]`. Ein unbekannter Wert gilt als `automatisch`,
   mit Hinweis im Protokoll.
2. **Vorprüfung:** Kerneltreiber der Render-Knoten
   (`/sys/class/drm/renderD*/device/driver`). Nur wenn einer `nouveau` ist,
   wird der Renderer geprüft. Bei `amdgpu`, `i915`/`xe`, `nvidia` (dem
   NVIDIA-Treiber) oder ganz ohne Render-Knoten passiert nichts und es kostet
   keine Zeit.
3. **Renderer-Prüfung** im Hintergrund, gleich nach den Eingangsprüfungen,
   während der Starter Reste früherer Sitzungen beendet, die
   labwc-Konfiguration abgleicht und die Portale zurücksetzt:
   - `querencia-sitzung --grafik-pruefen` (Querencia Sitzung ab 0.1.8): ein
     GLES-Kontext ohne Fenster (EGL, Plattform surfaceless), Ausgabe
     `renderer=…`;
   - fehlt es oder scheitert es: `eglinfo -B -p surfaceless -a gles`
     (mesa-utils).

   Der Starter wartet höchstens 2 s ab Prüfbeginn, und zwar nur auf eine
   Fertig-Datei, nie auf den Prozess selbst: eine Prüfung, die im Treiber
   hängt oder abstürzt, hält die Anmeldung nicht auf.
4. **Entscheidung** vor dem Setzen der Aktivierungsumgebung: Beginnt der
   Renderer mit `zink` und nennt NVK oder nouveau, bekommt die Sitzung

   ```text
   LIBGL_ALWAYS_SOFTWARE=1 MESA_LOADER_DRIVER_OVERRIDE=llvmpipe GALLIUM_DRIVER=llvmpipe
   ```

   Jeder andere Renderer (radeonsi, iris, llvmpipe, NVIDIA, nouveau-GL
   älterer Karten) und eine Prüfung ohne Ergebnis: nichts. Vulkan-Variablen
   (`VK_*`) setzt der Starter nie.
5. **Wohin:**
   - in die Aktivierungsumgebung von systemd und D-Bus, wie die übrigen
     Variablen der Sitzung (QD-064): Programme, die D-Bus oder der
     User-Manager starten. Nach dem Ende von labwc nimmt der Starter sie
     dort wieder heraus bzw. setzt die Werte von vorher;
   - an die Programme des Autostarts: der Starter gibt labwc nur
     `QUERENCIA_GRAFIK_UMGEBUNG` mit, der Autostart exportiert daraus genau
     diese drei Variablen. Sie gelten damit für den polkit-Agenten,
     Benachrichtigungen, Arbeitsfläche, Panel und alles, was Panel und
     Arbeitsfläche starten (Menüs, Starter, Orte).
   - **labwc selbst nicht:** es setzt die Bilder mit GLES über zink
     zusammen; `MESA_LOADER_DRIVER_OVERRIDE` träfe auch seine GBM-Puffer.
     Was labwc direkt startet (Tastenkürzel in `rc.xml`: Bildschirmfoto,
     Kürzelübersicht, Sitzungsdialoge; XWayland), läuft wie bisher.
6. **Protokoll** in `~/.local/state/querencia/sitzung-start.log`, eine Zeile:

   ```text
   querencia-session: Grafik: Software-OpenGL für die Sitzung: LIBGL_ALWAYS_SOFTWARE=1 MESA_LOADER_DRIVER_OVERRIDE=llvmpipe GALLIUM_DRIVER=llvmpipe (zink auf NVK/nouveau; Renderer „zink Vulkan 1.4(NVIDIA GeForce RTX 3060 (NVK GA106) (MESA_NVK))“ (Kerneltreiber nouveau, Prüfung 280 ms über querencia-sitzung --grafik-pruefen, gewartet 0 ms))
   ```

   oder z. B. `Grafik: OpenGL unverändert (Kerneltreiber amdgpu, kein
   nouveau: nicht geprüft)`. Hat die Benutzerkopie des Autostarts den
   Abschnitt nicht (eigene Datei), steht ein Hinweis darunter: dann gilt
   Software-OpenGL nur über die Aktivierungsumgebung.

`querencia-session --grafik-pruefen` zeigt dieselbe Zeile, ohne etwas zu
starten oder zu ändern.

## Wechsel zum NVIDIA-Treiber

Mit dem NVIDIA-Treiber heißt der Kerneltreiber `nvidia`, und OpenGL kommt
von NVIDIA, nicht von zink. Der Starter prüft dann nicht mehr und setzt
nichts; der Rückfall fällt beim nächsten Login von selbst weg. Nach einem
Neustart des Rechners hat auch der User-Manager keine alten Werte mehr.

## Messung

Prüfhülle: `env -i`, frisches HOME und XDG-Ordner, eigener
`XDG_RUNTIME_DIR`, eigener Session-Bus (`dbus-run-session`), eigener labwc
0.9.8 ohne Bildschirm. Anders als die übrigen Prüfungen zeichnet dieser
labwc mit `WLR_RENDERER=gles2` auf der GPU (wie live über zink), sonst
fehlte den Programmen linux-dmabuf und sie fielen selbst auf Software
zurück. Die laufende Sitzung, ihr Socket und ihr Bus werden nie benutzt.
Werkzeuge und Belege: `integration/session-grafik/messung.sh`,
`messung-innen.sh`, `auswerten.py`, `nachweis/session-grafik/messung/`.

Je drei Läufe ohne und mit den drei Variablen (Arbeitsfläche und Panel aus
`~/opt/querencia/bin`, Stand 23.10.2026; 1280×720, zwei `foot`-Fenster).
Zeiten aus `WAYLAND_DEBUG` der Programme selbst, Median:

| Kennzahl | ohne | mit | n |
|---|---|---|---|
| GSK-Renderer (`GSK_DEBUG=renderer`) | `GskVulkanRenderer` | `GskVulkanRenderer` | 1/1 |
| geladene Bibliotheken | `libvulkan_nouveau`, kein `libgallium` | `libvulkan_nouveau`, kein `libgallium` | 7/7 |
| Arbeitsfläche: Start bis zum ersten Bild beider Flächen | 508 ms | 512 ms | 3/3 |
| Panel: Start bis zum ersten Bild beider Panels | 590 ms | 615 ms | 3/3 |
| Panel: Klick „Anwendungen“ bis erstes Bild des Menüs | 42,5 ms | 42,6 ms | 15/15 |
| Panel: Klick Fensterliste bis erstes Panel-Bild nach dem Wechsel | 58,0 ms | 58,0 ms | 18/18 |
| Arbeitsfläche: Rechtsklick bis erstes Bild des Kontextmenüs | 14,1 ms | 9,0 ms | 15/15 |
| Arbeitsfläche RSS / PSS nach dem Start | 130 224 / 44 746 kB | 130 368 / 44 714 kB | 3/3 |
| Panel RSS / PSS nach dem Start | 149 288 / 62 784 kB | 149 236 / 62 966 kB | 3/3 |
| Arbeitsfläche RSS / PSS nach Menüs und Wechseln | 130 256 / 44 846 kB | 130 424 / 44 124 kB | 3/3 |
| Panel RSS / PSS nach Menüs und Wechseln | 149 284 / 63 084 kB | 149 240 / 62 302 kB | 3/3 |
| CPU-Zeit Arbeitsfläche / Panel bis Laufende | 470 / 820 ms | 480 / 850 ms | 3/3 |

Panel und Arbeitsfläche benutzen in beiden Fällen kein OpenGL; die drei
Variablen ändern an ihnen nichts. Die Unterschiede liegen in der Streuung
der Einzelwerte (Kontextmenü ohne: 2,9–20,0 ms, mit: 2,6–19,1 ms; Panel-Start
ohne 429–596 ms, mit 610–631 ms). Parallel lief die Prüfhülle eines anderen
Workers; absolute Zeiten sind deshalb nur untereinander vergleichbar.

**WebKit und Firefox** (nur Umgebung und `/proc/<pid>/maps`, keine
Anmeldung, keine Eingabe; Querencia Nachrichten 0.1.0, Ablage
`4c70dede91b11648`, Anmeldeseite; Firefox mit Wegwerfprofil und
`about:blank`):

| Prozess | ohne | mit |
|---|---|---|
| WebKitWebProcess (2) | `dri_gbm`, `libgallium`, `libvulkan_nouveau` (zink) | die drei Variablen; `libgallium` ohne `libvulkan_nouveau`, ohne `dri_gbm` (llvmpipe) |
| Nachrichten (Oberfläche, GTK 4) | `libgallium`, `libvulkan_nouveau` | die drei Variablen; `libgallium`, `libvulkan_nouveau` (GSK zeichnet weiter über Vulkan) |
| Firefox Hauptprozess | `dri_gbm`, `libgallium`, `libvulkan_nouveau` (zink) | die drei Variablen; keine GL-Bibliothek geladen (Firefox zeichnet in Software) |
| Firefox Inhaltsprozesse (9–10) | keine GL-Bibliothek | die drei Variablen, keine GL-Bibliothek |

Nachrichten 0.1.1 (aktiv seit 23.10. 22:39) setzt dieselben drei Variablen
für seine Web-Prozesse selbst (eigener Grafik-Rückfall); mit dem Starter
zusammen ändert das nichts (Beleg `fremd-nachrichten-011-*`).

Prüfwerkzeug (`querencia-sitzung --grafik-pruefen`, Release 0.1.8):
250–290 ms je Aufruf, fast alles in `eglInitialize` (Start von zink auf
NVK). Über den Starter (`--grafik-pruefen`): Prüfung 270–310 ms, gesamt
331–367 ms. **Im Sitzungsstart wartet der Starter 0 ms** darauf: die
Prüfung ist fertig, bevor er die Aktivierungsumgebung setzt (Probe S1, S3).

## Prüfung

`integration/session-grafik/probe-grafik.sh`: Testkopie des Starters
(drei Zeilen geändert: Laufzeitkopie, Programme, Render-Knoten), Attrappen
für Prüfwerkzeug, eglinfo, systemctl, dbus-update-activation-environment
und loginctl, headless labwc mit pixman, `env -i` ohne D-Bus.

28/28 (`nachweis/session-grafik/probe-grafik.log.txt`):

| Fall | Ergebnis |
|---|---|
| G01 zink auf NVK, nouveau | Software-OpenGL an |
| G02–G04 radeonsi (amdgpu), iris (i915), NVIDIA (nvidia) | nichts, Prüfwerkzeug gar nicht gerufen |
| G05, G06 llvmpipe bzw. nouveau-GL (NV106) trotz nouveau | nichts |
| G07 zwei Karten (amdgpu und nouveau), Renderer radeonsi | geprüft, nichts |
| G08 kein Render-Knoten | nichts |
| G09, G10 `grafik.ini` aus bzw. An (Großschreibung) | nichts bzw. an, ohne Prüfung |
| G11, G11b unbekannter Wert; Schlüssel in anderem Abschnitt | automatisch mit Hinweis; zählt nicht |
| G12, G12b Absturz (SIGSEGV) beider Werkzeuge; nur des Prüfwerkzeugs | nichts; eglinfo übernimmt |
| G13 Prüfwerkzeug hängt | Zeitgrenze 2 s, Starter nach 2,06 s weiter |
| G14a alte querencia-sitzung ohne `--grafik-pruefen` | eglinfo übernimmt |
| G14 echtes Prüfwerkzeug auf dieser Maschine, 5 Läufe | an, 331–356 ms gesamt |
| S1 voller Start (zink) | labwc ohne die drei, Panel mit; keine `VK_*`; systemd und D-Bus mit; nach dem Ende `unset-environment`; Protokollzeile |
| S2 voller Start (radeonsi) | Panel, labwc, Aktivierung ohne; Protokollzeile |
| S3 eigene autostart ohne Abschnitt | Hinweis im Protokoll, Panel ohne, Aktivierung mit |

## Integration

`integration/session-grafik/session-grafik-integrieren.sh`
(`pruefen | aktivieren | zurueck`, `QSG_FREIGABE=ja`) tauscht Starter und
Autostart-Vorgabe der Laufzeitkopie per rename und zieht, wo nötig, den
Eintrag `.vorgabe/autostart` der Benutzerkopie nach (Kopf des Skripts).
Wirkung ab der nächsten Anmeldung. Probe auf Kopien:
`probe-integrieren.sh` (34/34: aktivieren, zurueck, Nachziehen des
Protokolls und der Abgleich beim nächsten Start, Gegenprobe ohne
Nachziehen, Rückrollen an zehn Fehlerstellen und vier beim Rückfall,
Verweigerungen, Live-Stand unberührt).

## Nachtrag 24.10.2026: session/grafik-3 (Files-Reste)

Nach dem Ausmustern von Querencia Files (Beta Schritt 7, 24.10. 03:11)
entfernt `session/grafik-3` die Files-Reste aus der Laufzeitkonfiguration
(Schritt 8 des Aktivierungsplans): die Starter
`session/querencia/applications/org.querencia.Files.desktop` und
`io.github.lgse.Strata.desktop` (HMP-04, Entwicklungsbuild
querencia-files-dev) sowie „Files“ in den Kommentaren von
`labwc/rc.xml` (QD-065) und `x11-zwischenablage-helfer` (QD-155).
Starter und Autostart-Vorgabe bleiben auf grafik-2.

Die zwei Starter brauchte nichts mehr: Sie liegen nicht in
`XDG_DATA_DIRS` der Sitzung, kein Starter, Autostart, `mimeapps.list` oder
Paket nimmt sie (Querencia Dateien bringt seinen eigenen Starter mit);
nur der Panel-Test `querencia_desktop_entries_land_in_accessories` las
das Verzeichnis und prüft jetzt Terminal und Bildschirmfoto und dass
keiner dieser Starter Ordner öffnet. `rc.xml` ist Vorgabe der
Benutzerkopie: Bei der nächsten Anmeldung übernimmt der Starter die
geänderte Vorgabe selbst, solange die Benutzerkopie unverändert ist
(sonst `rc.xml.vorgabe-neu`). Den Zwischenablage-Helfer führt die
Sitzung aus `~/opt/querencia/bin` aus; diese Kopie ändert grafik-3 nicht
(nur ein Kommentar).

Integration: `integration/session-grafik-3/session-grafik-3-integrieren.sh`
(`pruefen | aktivieren | zurueck`, `QSG3_FREIGABE=ja`, Sperre gemeinsam
mit session-grafik). `aktivieren` ersetzt `rc.xml` und den Helfer der
Laufzeitkopie per rename und verschiebt die zwei Starter in die
Sicherung; `zurueck` stellt den Stand von grafik-2 (main `2280514`) wieder
her. Probe `probe.sh` auf Kopien.
