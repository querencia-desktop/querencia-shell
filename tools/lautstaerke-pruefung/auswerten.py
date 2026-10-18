#!/usr/bin/env python3
"""G2-J: wertet einen isolierten Lauf aus.

  auswerten.py <logs-ordner> haupt|ohne-dienst <breite> <höhe>

Quellen je Lauf (szenario-g2j.sh): szenario.txt (MARKE-Zeilen mit Uhrzeit),
panel.log (Panel und der vom Panel gestartete Lautstärkeregler), qpulse.log
bzw. qpulse-spaet.log (Testserver: jede Anfrage eines Clients mit Uhrzeit),
panel.ini.anfang/.ende, Bildschirmfotos. Serveranfragen werden über die
Uhrzeit dem Prüfschritt (Marke) zugeordnet, in dem sie ankamen.

Symbole: Im Applet-Feld (22 px) werden die Pixel in Querencia-Orange
gezählt und als Maske mit den Referenzbildern S00–S06 desselben Laufs
verglichen (Lautstärke von außen gesetzt, wie mit pactl). Ein ausgegrautes
Symbol (Deckkraft 0,45) hat kein volles Orange.

Ausgabe: je Prüfung „OK“ oder „FEHLER“, am Ende „n/m Prüfungen bestanden“;
Exit 0 nur, wenn alle bestanden.
"""
import json
import re
import sys
from datetime import datetime
from pathlib import Path

PIL = None
try:
    from PIL import Image
    PIL = Image
except ImportError:  # System-Python ohne PIL: Pixelprüfungen entfallen sichtbar
    pass

LOGS = Path(sys.argv[1])
ART = sys.argv[2]
W, H = int(sys.argv[3]), int(sys.argv[4])
VX, VY = W - 170, 16
results = []


def check(name, ok, detail=""):
    results.append(bool(ok))
    print(("OK    " if ok else "FEHLER"), name, f"({detail})" if detail != "" else "")


def t(text):
    return datetime.strptime(text, "%H:%M:%S.%f")


# ------------------------------------------------------------- Marken
szenario = (LOGS / "szenario.txt").read_text(errors="replace")
marks = [(m.group(2), t(m.group(1))) for m in re.finditer(r"^MARKE (\S+) (\S+)", szenario, re.M)]
order = [k for k, _ in marks]


def window(key):
    i = order.index(key)
    start = marks[i][1]
    end = marks[i + 1][1] if i + 1 < len(marks) else datetime.max
    return start, end


# ------------------------------------------------------------- Logs
panel = (LOGS / "panel.log").read_text(errors="replace").splitlines()
panel = [line for line in panel if "WARNING" not in line and "libEGL" not in line]


def server_lines(name="qpulse.log"):
    path = LOGS / name
    if not path.exists():
        return []
    out = []
    for line in path.read_text(errors="replace").splitlines():
        m = re.match(r"^(\d\d:\d\d:\d\d\.\d{3}) (.*)$", line)
        if m:
            out.append((t(m.group(1)), m.group(2)))
    return out


server = server_lines()


def in_step(key, pattern, lines=None):
    start, end = window(key)
    return [text for when, text in (lines if lines is not None else server)
            if start <= when < end and re.search(pattern, text)]


def volumes(key, device, cmd="SET_SINK_VOLUME"):
    return [int(m.group(1)) for line in in_step(key, rf"BEFEHL .* {cmd} {re.escape(device)} ")
            for m in [re.search(r" (\d+) % ", line)] if m]


def panel_has(pattern):
    return [line for line in panel if re.search(pattern, line)]


OUT = "alsa_output.test-analog-stereo"
IN = "alsa_input.test-analog-stereo"

# ------------------------------------------------------------- Pixel
ORANGE = lambda p: p[0] > 200 and 90 < p[1] < 160 and p[2] < 120
GREY = lambda p: abs(p[0] - p[1]) < 12 and abs(p[1] - p[2]) < 12 and 60 < p[0] < 120


def icon_box():
    return (VX - 13, 3, VX + 13, 29)


def mask(shot):
    im = PIL.open(LOGS / f"{shot}.png").convert("RGB")
    x0, y0, x1, y1 = icon_box()
    return {(x - x0, y - y0) for x in range(x0, x1) for y in range(y0, y1) if ORANGE(im.getpixel((x, y)))}, \
        sum(1 for x in range(x0, x1) for y in range(y0, y1) if GREY(im.getpixel((x, y))))


def same(a, b):
    if not a and not b:
        return 1.0
    return len(a & b) / max(1, len(a | b))


def level_of(shot, refs):
    m, _ = mask(shot)
    best = max(refs, key=lambda r: same(m, refs[r]))
    return best, same(m, refs[best])


# ------------------------------------------------------------- Haupt
if ART == "haupt":
    ini_start = (LOGS / "panel.ini.anfang").read_text()
    ini_end = (LOGS / "panel.ini.ende").read_text() if (LOGS / "panel.ini.ende").exists() else ""
    check("H00 Ausgangs-panel.ini ohne Lautstärke", "volume" not in ini_start)
    check("H01 Panelmenü oben rechts geöffnet", panel_has(r"Kontextmenü von Panel top geöffnet \(Bereich rechts\)"))
    added = panel_has(r"volume-1 \(volume\) zu Panel top hinzugefügt, Bereich rechts, Position 1; gespeichert")
    check("H02 Lautstärke über den Dialog hinzugefügt (rechts, vor der Uhr), gespeichert", len(added) == 1, added[:1])
    check("H03 zweiter Doppelklick fügt nichts hinzu (nur einmal erlaubt)",
          len(panel_has(r"\(volume\) zu Panel .* hinzugefügt")) == 1)
    connected = [line for line in panel if line.startswith("querencia-panel: Audiodienst verbunden")]
    check("Panel verbindet sich mit dem Audiodienst", connected, connected[:1])
    check("Applet abonniert Ereignisse (SUBSCRIBE sink|source|sink-input|source-output|server)",
          any("SUBSCRIBE Maske 0x8f" in text for _, text in server))

    # Symbole: Referenzen von außen
    if PIL:
        refs = {}
        for name, ref in [("stumm", "sstumm-symbol"), ("leise", "s20-symbol"), ("mittel", "s50-symbol"),
                          ("laut", "s90-symbol")]:
            refs[name], grey = mask(ref)
            check(f"S Symbol „{name}“ von außen gesetzt: Orange vorhanden, grauer Lautsprecher",
                  len(refs[name]) >= 6 and grey > 20, f"{len(refs[name])} orange, {grey} grau")
        m0, _ = mask("s0-symbol")
        check("S 0 % zeigt das Stumm-Symbol", same(m0, refs["stumm"]) > 0.9, f"{same(m0, refs['stumm']):.2f}")
        m40, _ = mask("s40-symbol")
        check("S 40 % zeigt „mittel“", same(m40, refs["mittel"]) > 0.9, f"{same(m40, refs['mittel']):.2f}")
        names = list(refs)
        distinct = all(same(refs[a], refs[b]) < 0.8 for i, a in enumerate(names) for b in names[i + 1:])
        check("S vier verschiedene Symbole", distinct)
        check("S mehr Wellen bei mehr Lautstärke (orange: leise < mittel < laut)",
              len(refs["leise"]) < len(refs["mittel"]) < len(refs["laut"]),
              f"{len(refs['leise'])} < {len(refs['mittel'])} < {len(refs['laut'])}")
    else:
        check("Pixelprüfungen (PIL fehlt)", False, "PIL nicht verfügbar")

    check("H05 Linksklick öffnet den Regler", panel_has(r"Lautstärke: Regler geöffnet"))
    v06 = volumes("H06", OUT)
    check("H06 Klick in den Regler bei 80 % setzt 77–83 %", v06 and 77 <= v06[-1] <= 83, v06)
    v07 = volumes("H07", OUT)
    check("H07 Tastatur (Bild runter) im Regler: −5 %", v06 and v07 == [v06[-1] - 5], v07)
    check("H08 Stumm im Regler", in_step("H08", rf"SET_SINK_MUTE {OUT} stumm"))
    check("H09 Stumm im Regler aus", in_step("H09", rf"SET_SINK_MUTE {OUT} laut"))
    check("H10 Escape schließt den Regler", panel_has(r"Lautstärke: Regler geschlossen"))
    v11, v12 = volumes("H11", OUT), volumes("H12", OUT)
    check("H11 Mausrad zweimal hoch: +5 %, +5 %", v07 and v11 == [v07[-1] + 5, v07[-1] + 10], v11)
    check("H12 Mausrad runter: −5 %", v11 and v12 == [v11[-1] - 5], v12)
    check("H13 mittlere Taste: stumm", in_step("H13", rf"SET_SINK_MUTE {OUT} stumm"))
    check("H14 mittlere Taste: laut", in_step("H14", rf"SET_SINK_MUTE {OUT} laut"))
    check("H15 Rechtsklick öffnet das Appletmenü", panel_has(r"Appletmenü von volume-1 geöffnet"))
    check("H16 Menü „Stumm“: stumm", in_step("H16", rf"SET_SINK_MUTE {OUT} stumm"))
    check("H17 Menü „Stumm“ erneut: laut", in_step("H17", rf"SET_SINK_MUTE {OUT} laut"))
    if PIL:
        for shot, want, step in [("h02-hinzugefuegt", "mittel", "H02 40 %"), ("h06-regler-80", "laut", "H06 81 %"),
                                 ("h08-popup-stumm", "stumm", "H08 stumm"), ("h13-mitte-stumm", "stumm", "H13 stumm"),
                                 ("h16-menue-stumm", "stumm", "H16 stumm"), ("h12-mausrad-runter", "laut", "H12 81 %"),
                                 ("h18-geraet-test-sink", "laut", "H18 Test-Sink 100 %"),
                                 ("h23-mixer-regler", "leise", "H23 30 % aus dem Mixer")]:
            got, score = level_of(shot, refs)
            check(f"Symbol {step}: „{want}“", got == want and score > 0.85, f"{got} {score:.2f}")
    check("H18 Test-Sink (module-null-sink) geladen und Standard", in_step("H18", r"load-module module-null-sink Testausgabe")
          and in_step("H18", r"set-default-sink"))
    check("H18 Applet folgt dem Gerätewechsel von außen", panel_has(r"^querencia-panel: Ausgabe Testausgabe \(Testausgabe\): 100 %"))
    check("H19 Mausrad wirkt auf das neue Standardgerät", volumes("H19", "Testausgabe") == [95], volumes("H19", "Testausgabe"))
    check("H20 Menü öffnet den Lautstärkeregler über seinen Desktop-Eintrag",
          panel_has(r"Lautstärkeregler über org\.querencia\.Lautstaerke\.desktop")
          and panel_has(r"^querencia-lautstaerke: Fenster geöffnet"))
    check("H20 Lautstärkeregler verbindet sich als zweiter Client", in_step("H20", r"VERBINDUNG"))
    check("H22 Gerätewahl im Mixer setzt das Standardgerät", in_step("H22", rf"SET_DEFAULT_SINK {OUT}"))
    after = [i for i, line in enumerate(panel) if "Standard-Ausgabe → " + OUT in line]
    check("H22 Panel-Applet folgt der Gerätewahl im Mixer",
          after and any(line.startswith(f"querencia-panel: Ausgabe Eingebautes Tonsystem Analog Stereo ({OUT})")
                        for line in panel[after[0]:]))
    v23 = volumes("H23", OUT)
    check("H23 Mixer-Regler 30 % (27–33)", v23 and 27 <= v23[-1] <= 33, v23)
    check("H24 Mixer Stumm an", in_step("H24", rf"SET_SINK_MUTE {OUT} stumm"))
    check("H25 Mixer Stumm aus", in_step("H25", rf"SET_SINK_MUTE {OUT} laut"))
    v27 = volumes("H27", IN, "SET_SOURCE_VOLUME")
    check("H27 Aufnahme-Regler 50 % (47–53)", v27 and 47 <= v27[-1] <= 53, v27)
    v29 = [int(m.group(1)) for line in in_step("H29", r"SET_SINK_INPUT_VOLUME 0 ") for m in [re.search(r" (\d+) %", line)] if m]
    check("H29 Anwendungsstrom Musikspieler 60 % (57–63)", v29 and 57 <= v29[-1] <= 63, v29)
    check("H30 Hilfe > Info öffnet den Info-Dialog mit Fokus auf „Schließen“",
          panel_has(r"querencia-lautstaerke: Info geöffnet \(Fokus auf „Schließen“\)"))
    check("H31 Eingabetaste schließt den Info-Dialog (Tastaturfokus lag auf „Schließen“)",
          panel_has(r"querencia-lautstaerke: Info geschlossen"))
    check("H32 Knopf Schließen beendet den Lautstärkeregler (Trennung vom Server)", in_step("H32", r"TRENNUNG"))
    last_test = max([i for i, line in enumerate(panel) if line.startswith("querencia-panel: Ausgabe Testausgabe")], default=-1)
    check("H33 nach Entladen des Test-Sinks zeigt das Applet wieder das eingebaute Gerät",
          last_test >= 0 and any(line.startswith("querencia-panel: Ausgabe Eingebautes") for line in panel[last_test:]))
    check("H34 Aus dem Panel entfernt und gespeichert", panel_has(r"volume-1 \(volume\) aus dem Panel entfernt; gespeichert"))
    check("H34 panel.ini am Ende ohne Lautstärke, Uhr unverändert", "volume" not in ini_end and "End=clock-1" in ini_end)
    if PIL:
        m34, _ = mask("h34-entfernt")
        check("H34 Applet nicht mehr im Panel (kein Orange im Feld)", len(m34) == 0, len(m34))
    errors = [line for line in panel if re.search(r"panicked|CRITICAL|Segmentation|nicht entfernt|nicht hinzugefügt", line)]
    check("keine Panik, kein CRITICAL, kein Fehler beim Hinzufügen/Entfernen", not errors, errors[:2])
    check("Testserver sah keinen Protokollfehler", not any(text.startswith("FEHLER") for _, text in server))

# ------------------------------------------------------------- ohne Dienst
elif ART == "ohne-dienst":
    late = server_lines("qpulse-spaet.log")
    check("K00 Panel startet ohne Audiodienst", panel_has(r"^querencia-panel: kein Audiodienst"))
    check("K00 Panel wartet ereignisgesteuert auf den Server-Socket", panel_has(r"^querencia-panel: warte auf .*/pulse/native"))
    if PIL:
        m, grey = mask("k00-ohne-dienst")
        check("K00 Symbol ausgegraut (kein volles Orange)", len(m) == 0, len(m))
        check("K00 Symbol trotzdem sichtbar (heller Lautsprecher)", grey == 0 and
              PIL.open(LOGS / "k00-ohne-dienst.png").convert("RGB").getpixel((VX - 7, 16)) != (246, 245, 244))
    check("K03 Mausrad/mittlere Taste ohne Dienst: kein Absturz",
          panel_has(r"Lautstärke: mittlere Taste") and not panel_has(r"panicked"))
    check("K04 Lautstärkeregler auch ohne Dienst startbar", panel_has(r"^querencia-lautstaerke: Fenster geöffnet")
          and panel_has(r"^querencia-lautstaerke: kein Audiodienst"))
    check("K05 Info-Dialog mit Fokus auf „Schließen“, Eingabetaste schließt",
          panel_has(r"Info geöffnet \(Fokus") and panel_has(r"Info geschlossen"))
    check("K06 später gestarteter Dienst wird ohne Neustart gefunden (Panel)",
          panel_has(r"^querencia-panel: Audiodienst erschienen, verbinde") and panel_has(r"^querencia-panel: Audiodienst verbunden"))
    check("K06 … und vom Lautstärkeregler", panel_has(r"^querencia-lautstaerke: Audiodienst verbunden"))
    check("K06 beide verbinden sich mit dem späten Server", len([1 for _, x in late if x.startswith("VERBINDUNG")]) >= 2,
          len([1 for _, x in late if x.startswith("VERBINDUNG")]))
    if PIL:
        m, _ = mask("k06-tooltip")
        check("K06 Symbol wieder farbig (40 % = mittel, Orange vorhanden)", len(m) > 15, len(m))
        m, _ = mask("k07-dienst-weg")
        check("K07 Dienst beendet: wieder ausgegraut", len(m) == 0, len(m))
    lost = [line for line in panel if re.search(r"^querencia-panel: kein Audiodienst", line)]
    check("K07 Panel meldet den Verlust und läuft weiter", len(lost) >= 2, len(lost))
    if PIL:
        a8 = PIL.open(LOGS / "k08-mixer-zu.png").convert("RGB").crop((W // 2 - 200, H // 2 - 150, W // 2 + 200, H // 2 + 150))
        a0 = PIL.open(LOGS / "k00-ohne-dienst.png").convert("RGB").crop((W // 2 - 200, H // 2 - 150, W // 2 + 200, H // 2 + 150))
        check("K08 Lautstärkeregler geschlossen (Bildmitte wie vor dem Öffnen)", list(a8.getdata()) == list(a0.getdata()))
    check("K09 Applet aus panel.ini über das Menü entfernt und gespeichert",
          panel_has(r"volume-1 \(volume\) aus dem Panel entfernt; gespeichert"))
    ini_end = (LOGS / "panel.ini.ende").read_text() if (LOGS / "panel.ini.ende").exists() else ""
    check("K09 panel.ini am Ende ohne Lautstärke, Uhr bleibt", "volume" not in ini_end and "End=clock-1" in ini_end)
    check("keine Panik, kein CRITICAL", not panel_has(r"panicked|CRITICAL|Segmentation"))
else:
    sys.exit("Art: haupt oder ohne-dienst")

print(f"{sum(results)}/{len(results)} Prüfungen bestanden")
sys.exit(0 if all(results) else 1)
