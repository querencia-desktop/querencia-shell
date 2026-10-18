#!/usr/bin/env python3
"""G2-J: erzeugt die qeingabe-Szenarien für eine Auflösung.

  szenario-machen.py <breite> <höhe> haupt|ohne-dienst > szenario.txt

Alle Koordinaten sind aus der Bildschirmgröße abgeleitet (vermessen bei
1280x720, Bilder in nachweis/gui/erkunden): das Applet steht links neben
der Uhr (Mitte x = Breite - 170, y = 16), Dialoge und der
Lautstärkeregler öffnet labwc mittig (Platzierung „center“), Panelmenüs
schieben sich an den rechten Rand. SHOT, QPACTL und WERKZEUG ersetzt
szenario-g2j.sh.

Jede Marke „mark <Kennung> …“ ist ein Prüfschritt; auswerten.py ordnet
Panel-, Mixer- und Serverlog den Marken zu.
"""
import sys

W, H, ART = int(sys.argv[1]), int(sys.argv[2]), sys.argv[3]
VX, VY = W - 170, 16            # Applet
CX, CY = W // 2, H // 2          # Bildschirmmitte (Dialoge, Lautstärkeregler)
MENU_X = W - 150                 # Applet-Rechtsklickmenü (am rechten Rand)
out = []


def z(*lines):
    out.extend(lines)


def mark(key, text):
    z(f"mark {key} {text}")


def click(x, y, wait=900, button="click"):
    z(f"move {x} {y}", "wait 250", button, f"wait {wait}")


def shot(name):
    z(f"shot SHOT/{name}.png")


def menu(item_y, wait=900):
    """Rechtsklick auf das Applet, dann den Menüeintrag in Zeile item_y."""
    click(VX, VY, 900, "rclick")
    click(MENU_X, item_y, wait)


# Popup des Applets (Linksklick), vermessen bei 1280x720 relativ zum Applet
SLIDER_X = VX - 2
SLIDER_TOP, SLIDER_BOTTOM = 95, 210     # 100 % bzw. 0 %
MUTE_X, MUTE_Y = VX - 30, 241


def slider_y(percent):
    return round(SLIDER_BOTTOM - (SLIDER_BOTTOM - SLIDER_TOP) * percent / 100)


# Lautstärkeregler (Fenster 520x400, mittig), relativ zur Bildschirmmitte
M_DATEI = (CX - 233, CY - 175)
M_HILFE = (CX - 180, CY - 175)
M_TAB = {"wiedergabe": (CX - 187, CY - 135), "aufnahme": (CX - 73, CY - 135),
         "anwendungen": (CX + 50, CY - 135)}
M_GERAET = (CX + 60, CY - 81)
M_REGLER_X0, M_REGLER_X1, M_REGLER_Y = CX - 100, CX + 155, CY - 38
M_STUMM = (CX - 101, CY + 31)
M_SCHLIESSEN = (CX + 198, CY + 187)
# Anwendungen: erste Zeile (Kopf), darunter Regler (schmaler) und Stumm
M_APP_REGLER_X0, M_APP_REGLER_X1, M_APP_REGLER_Y = CX - 189, CX + 70, CY - 56


def regler_x(percent):
    return round(M_REGLER_X0 + (M_REGLER_X1 - M_REGLER_X0) * percent / 100)


def app_regler_x(percent):
    return round(M_APP_REGLER_X0 + (M_APP_REGLER_X1 - M_APP_REGLER_X0) * percent / 100)


if ART == "haupt":
    mark("H00", "Ausgangslage: Standard-panel.ini ohne Lautstärke")
    z("wait 1000"); shot("h00-ausgang")

    mark("H01", "Zum Panel hinzufügen: Rechtsklick freie Fläche oben rechts")
    click(W - 250, VY, 900, "rclick"); shot("h01-panelmenue")
    click(W - 190, 50, 1500); shot("h01-dialog")
    z("type Laut", "wait 800"); shot("h01-dialog-gefiltert")
    mark("H02", "Doppelklick auf Lautstärke: hinzufügen")
    click(CX, CY - 103, 1500, "dclick"); shot("h02-hinzugefuegt")
    mark("H03", "zweites Mal: Lautstärke gesperrt, Doppelklick fügt nichts hinzu")
    click(CX, CY - 103, 1200, "dclick"); shot("h03-gesperrt")
    click(CX + 40, CY + 203, 1000); shot("h03-dialog-zu")

    # Symbol folgt Änderungen von außen (wie pactl): Referenzen je Stufe
    for key, (cmd, name) in enumerate([
            ("set-sink-volume alsa_output.test-analog-stereo 0%", "s0"),
            ("set-sink-volume alsa_output.test-analog-stereo 20%", "s20"),
            ("set-sink-volume alsa_output.test-analog-stereo 50%", "s50"),
            ("set-sink-volume alsa_output.test-analog-stereo 90%", "s90"),
            ("set-sink-mute alsa_output.test-analog-stereo 1", "sstumm"),
            ("set-sink-mute alsa_output.test-analog-stereo 0", None),
            ("set-sink-volume alsa_output.test-analog-stereo 40%", "s40")]):
        mark(f"S{key:02d}", f"von außen: {cmd}")
        z(f"move {CX} {CY}", f"run QPACTL {cmd}", "wait 1200")
        if name:
            shot(f"{name}-symbol")

    mark("H04", "Tooltip über dem Applet")
    z(f"move {VX} {VY}", "wait 2000"); shot("h04-tooltip")

    mark("H05", "Linksklick: Regler öffnet")
    click(VX, VY, 1200); shot("h05-popup")
    mark("H06", "Klick in den Regler bei 80 %")
    click(SLIDER_X, slider_y(80), 1200); shot("h06-regler-80")
    mark("H07", "Tastatur im Regler: Bild runter (-5 %)")
    z("key Page_Down", "wait 900"); shot("h07-regler-taste")
    mark("H08", "Stumm im Regler an")
    click(MUTE_X, MUTE_Y, 1200); shot("h08-popup-stumm")
    mark("H09", "Stumm im Regler aus")
    click(MUTE_X, MUTE_Y, 1200)
    mark("H10", "Escape schließt den Regler")
    z("key Escape", "wait 900"); shot("h10-popup-zu")

    mark("H11", "Mausrad zweimal hoch (+10 %)")
    z(f"move {VX} {VY}", "wait 300", "scroll -1", "wait 500", "scroll -1", "wait 1000"); shot("h11-mausrad-hoch")
    mark("H12", "Mausrad einmal runter (-5 %)")
    z("scroll 1", "wait 1000"); shot("h12-mausrad-runter")
    mark("H13", "Mittlere Taste: stumm")
    z("mclick", "wait 1200"); shot("h13-mitte-stumm")
    mark("H14", "Mittlere Taste: wieder laut")
    z("mclick", "wait 1200")

    mark("H15", "Rechtsklick: Menü")
    click(VX, VY, 1000, "rclick"); shot("h15-menue")
    mark("H16", "Menü: Stumm")
    click(MENU_X, 37, 1200); shot("h16-menue-stumm")
    mark("H17", "Menü erneut: Häkchen bei Stumm, wieder laut")
    click(VX, VY, 1000, "rclick"); shot("h17-menue-haekchen")
    click(MENU_X, 37, 1200)

    mark("H18", "Test-Sink laden und als Standard setzen (wie pactl)")
    z("run QPACTL load-module module-null-sink sink_name=Testausgabe sink_properties=device.description=Testausgabe",
      "wait 1200", "run QPACTL set-default-sink Testausgabe", "wait 1500")
    z(f"move {VX} {VY + 1}", "wait 2000"); shot("h18-geraet-test-sink")
    mark("H19", "Mausrad auf dem Test-Sink runter (-5 %)")
    z("scroll 1", "wait 1200")

    mark("H20", "Lautstärkeregler über das Menü öffnen")
    z("run QPACTL spiele Musikspieler Lied", "run QPACTL nimm-auf Rekorder Notiz", "wait 800")
    menu(68, 3000); shot("h20-mixer")
    mark("H21", "Mixer: Ausgabegerät-Liste öffnen")
    click(*M_GERAET, 1200); shot("h21-geraeteliste")
    mark("H22", "Mixer: anderes Gerät wählen (Eingebautes Tonsystem)")
    click(M_GERAET[0], M_GERAET[1] + 37, 1500); shot("h22-geraet-gewechselt")
    mark("H23", "Mixer: Regler auf 30 %")
    click(regler_x(30), M_REGLER_Y, 1500); shot("h23-mixer-regler")
    mark("H24", "Mixer: Stumm an")
    click(*M_STUMM, 1200); shot("h24-mixer-stumm")
    mark("H25", "Mixer: Stumm aus")
    click(*M_STUMM, 1200)
    mark("H26", "Mixer: Reiter Aufnahme")
    click(*M_TAB["aufnahme"], 1200); shot("h26-aufnahme")
    mark("H27", "Mixer: Aufnahme-Regler auf 50 %")
    click(regler_x(50), M_REGLER_Y, 1200)
    mark("H28", "Mixer: Reiter Anwendungen")
    click(*M_TAB["anwendungen"], 1200); shot("h28-anwendungen")
    mark("H29", "Mixer: Musikspieler auf 60 %")
    click(app_regler_x(60), M_APP_REGLER_Y, 1500); shot("h29-anwendung-regler")
    mark("H30", "Mixer: Hilfe > Info")
    click(*M_HILFE, 900); shot("h30-hilfemenue")
    click(M_HILFE[0] + 20, M_HILFE[1] + 34, 1500); shot("h30-info")
    mark("H31", "Info: Eingabetaste löst den fokussierten Knopf Schließen aus")
    z("key Return", "wait 1200"); shot("h31-info-zu")
    mark("H32", "Mixer schließen (Knopf Schließen)")
    click(*M_SCHLIESSEN, 1500); shot("h32-mixer-zu")
    mark("H33", "Test-Sink wieder Standard, dann entladen: Applet fällt auf das eingebaute Gerät zurück")
    z("run QPACTL set-default-sink Testausgabe", "wait 1500", "run QPACTL unload-module 0", "wait 1500")
    z(f"move {VX} {VY}", "wait 2000"); shot("h33-test-sink-weg")
    mark("H34", "Aus dem Panel entfernen")
    menu(110, 1500); shot("h34-entfernt")
    mark("H35", "wieder hinzufügbar: Dialog zeigt Lautstärke frei")
    click(W - 250, VY, 900, "rclick")
    click(W - 190, 50, 1500)
    z("type Laut", "wait 800"); shot("h35-wieder-frei")
    click(CX + 40, CY + 203, 1000)
    mark("ENDE", "")
elif ART == "ohne-dienst":
    mark("K00", "Ausgangslage ohne Audiodienst")
    z("wait 1500"); shot("k00-ohne-dienst")
    mark("K01", "Tooltip: Kein Audiodienst")
    z(f"move {VX} {VY}", "wait 2000"); shot("k01-tooltip")
    mark("K02", "Linksklick: Regler ausgegraut")
    click(VX, VY, 1200); shot("k02-popup")
    z("key Escape", "wait 800")
    mark("K03", "Mausrad und mittlere Taste ohne Dienst: nichts, kein Absturz")
    z(f"move {VX} {VY}", "wait 300", "scroll -1", "wait 400", "scroll 1", "wait 400", "mclick", "wait 900")
    mark("K04", "Menü: Stumm ausgegraut, Lautstärkeregler öffnen")
    click(VX, VY, 1000, "rclick"); shot("k04-menue")
    click(MENU_X, 68, 3000); shot("k04-mixer-ohne-dienst")
    mark("K05", "Mixer: Hilfe > Info")
    click(*M_HILFE, 900)
    click(M_HILFE[0] + 20, M_HILFE[1] + 34, 1500); shot("k05-info")
    z("key Return", "wait 1000")
    mark("K06", "Audiodienst startet später: Applet und Mixer verbinden sich ohne Neustart")
    z("run python3 -I WERKZEUG/qpulse.py SHOT/qpulse-spaet.log", "wait 3000")
    shot("k06-dienst-da")
    z(f"move {VX} {VY + 1}", "wait 2000"); shot("k06-tooltip")
    mark("K07", "Audiodienst endet: wieder ausgegraut, kein Absturz")
    z("run QPACTL beenden", "wait 2000")
    z(f"move {VX} {VY}", "wait 2000"); shot("k07-dienst-weg")
    mark("K08", "Mixer schließen")
    click(*M_SCHLIESSEN, 1500); shot("k08-mixer-zu")
    mark("K09", "Applet aus panel.ini: Aus dem Panel entfernen (auch ohne Dienst)")
    click(VX, VY, 1000, "rclick"); shot("k09-menue")
    click(MENU_X, 110, 1500); shot("k09-entfernt")
    mark("ENDE", "")
else:
    sys.exit("Art: haupt oder ohne-dienst")
print("\n".join(out))
