#!/usr/bin/env python3
"""G2-AL: qeingabe-Szenarien der isolierten Prüfung.

  szenario-g2al.py <breite> <höhe> <art> > szenario.txt

Arten (panel.ini und Dienst aus laeufe-g2al.sh):
- zeigen:          drei Testfenster; „Arbeitsfläche anzeigen“ minimiert und
                   stellt wieder her, Merkliste endet mit einem Klick in der
                   Fensterliste und mit einem Flächenwechsel
- superd:          dasselbe über Super+D (labwc-Vorgabe) und gdbus
- glocke:          ohne Dienst grau, Dienst startet, drei Mitteilungen,
                   „Nicht stören“ an/aus, Zentrale öffnen/schließen, „Alle als
                   gelesen“, Dienst beendet und wieder gestartet
- ergaenzung-alt:  laufendes altes Panel (G2-AK 5deb1e2e), panel.ini wird mit
                   dem neuen Binary ergänzt und zurückgenommen
- ergaenzung-neu:  neues Panel mit der Live-panel.ini, Ergänzung wirkt sofort
- standard:        ohne panel.ini: Standardlayout mit Knopf und Glocke

Koordinaten vermessen bei 1024x768 (al-erkunden-*): Knopf x 13–34 (Mitte
23), Fensterliste ab x 42 (Knöpfe 200 breit, 2 Abstand), Glocke rechtsbündig
vor der Uhr (Symbol W - 191 … W - 175, Zahl bis W - 162; Klick W - 174),
Glockenmenü: „Nicht stören“ y 38, „Alle als gelesen markieren“ y 68 (x W - 170).
"""
import sys

W, H, ART = int(sys.argv[1]), int(sys.argv[2]), sys.argv[3]
NY, UY = 16, H - 16
KNOPF = 23
GLOCKE = W - 174
MENU_X, NICHT_STOEREN, ALLE_GELESEN = W - 170, 38, 68
AWAY = (W // 2, H // 2 + 200)
FARBEN = {"A": "#5a8fd0", "B": "#5ab06a", "C": "#d0a05a"}
DBUS = ("gdbus call --session --dest org.example.Querencia.Panel "
        "--object-path /org/example/Querencia/Panel "
        "--method org.querencia.Panel.ArbeitsflaecheUmschalten")
GET = ("gdbus call --session --dest org.querencia.Benachrichtigungen "
       "--object-path /org/freedesktop/Notifications "
       "--method org.freedesktop.DBus.Properties.Get org.querencia.Benachrichtigungen")
out = []


def z(*lines):
    out.extend(lines)


def mark(key, text):
    z(f"mark {key} {text}")


def click(x, y, wait=1200, button="click"):
    z(f"move {x} {y}", "wait 250", button, f"wait {wait}")


def shot(name):
    z(f"shot SHOT/{name}.png")


def away(wait=500):
    z(f"move {AWAY[0]} {AWAY[1]}", f"wait {wait}")


def chord(*keys, wait=1500):
    z(*[f"keydown {k}" for k in keys], *[f"keyup {k}" for k in reversed(keys)], f"wait {wait}")


def run(command, wait):
    z(f"run {command}", f"wait {wait}")


def fenster(name, wait=4000):
    run(f'python3 -I FENSTER org.querencia.Test{name} "Fenster {name}" "{FARBEN[name]}"', wait)


def liste(i):
    """Mitte des i-ten Knopfs der Fensterliste."""
    return 42 + i * 202 + 100


def knopf(wait=1500):
    click(KNOPF, UY, wait)


def drei_fenster():
    for name in "ABC":
        fenster(name)
    away(800)


if ART == "zeigen":
    mark("Z01", "Start: Knopf links unten")
    away(1500); shot("z01-start")
    mark("Z02", "drei Testfenster, C aktiv")
    drei_fenster(); shot("z02-drei")
    mark("Z03", "Tooltip über dem Knopf")
    z(f"move {KNOPF} {UY}", "wait 2500"); shot("z03-tooltip")
    mark("Z04", "Klick: alle minimiert")
    z("click", "wait 1500"); shot("z04-minimiert")
    z("wait 1500"); shot("z04-tooltip")
    mark("Z05", "Klick: alle wieder da, C oben")
    knopf(); away(); shot("z05-wieder")
    mark("Z06", "Klick, dann B aus der Fensterliste: Merkliste endet")
    knopf(); click(liste(1), UY, 1500); away(); shot("z06-b")
    mark("Z07", "Klick minimiert nur B, nächster Klick holt nur B")
    knopf(); away(); shot("z07-b-weg")
    knopf(); away(); shot("z07-b-wieder")
    mark("Z08", "A und C über die Fensterliste")
    click(liste(0), UY, 1500); click(liste(2), UY, 1500); away(); shot("z08-alle")
    mark("Z09", "Klick, Flächenwechsel: Merkliste endet; Fläche 2 leer")
    knopf(); chord("Control_L", "Alt_L", "Right"); knopf(); away(); shot("z09-flaeche-2")
    mark("Z10", "zurück auf Fläche 1: nichts mehr zum Wiederherstellen")
    chord("Control_L", "Alt_L", "Left"); knopf(); away(); shot("z10-flaeche-1")
    mark("ENDE", "")
elif ART == "superd":
    mark("D01", "drei Testfenster")
    drei_fenster(); shot("d01-drei")
    mark("D02", "Super+D: alle minimiert")
    chord("Super_L", "d", wait=2000); shot("d02-minimiert")
    mark("D03", "Super+D: alle wieder da")
    chord("Super_L", "d", wait=2000); shot("d03-wieder")
    mark("D04", "gdbus: minimiert")
    run(f"{DBUS} > SHOT/dbus-1.txt 2>&1", 2000); shot("d04-dbus-minimiert")
    mark("D05", "gdbus: wieder da")
    run(f"{DBUS} > SHOT/dbus-2.txt 2>&1", 2000); shot("d05-dbus-wieder")
    mark("D06", "Super+D, dann der Knopf: wieder da")
    chord("Super_L", "d", wait=2000); knopf(); away(); shot("d06-knopf")
    mark("ENDE", "")
elif ART == "glocke":
    mark("G01", "ohne Dienst: Glocke grau")
    away(1500); shot("g01-ohne")
    mark("G02", "Klick ohne Dienst")
    click(GLOCKE, NY); away()
    mark("G03", "Dienst startet: Glocke ohne Zahl")
    run("DIENST start", 3500); shot("g03-null")
    mark("G04", "drei Mitteilungen")
    for n in ("Erste", "Zweite", "Dritte"):
        run(f'notify-send -a Pruefung "{n}" "Mitteilung {n}"', 900)
    z("wait 6000"); shot("g04-drei")
    run(f"{GET} Ungelesen > SHOT/get-ungelesen-3.txt 2>&1", 800)
    mark("G05", "Rechtsklick, Nicht stören an")
    click(GLOCKE, NY, 1200, "rclick"); shot("g05-menu")
    click(MENU_X, NICHT_STOEREN, 1500); away(); shot("g05-ruhe")
    run(f"{GET} NichtStoeren > SHOT/get-ruhe-an.txt 2>&1", 800)
    mark("G06", "Mitteilung bei Nicht stören: gezählt, kein Popup")
    run('notify-send -a Pruefung "Vierte" "Mitteilung Vierte"', 2500); shot("g06-vier")
    mark("G07", "Nicht stören aus")
    click(GLOCKE, NY, 1200, "rclick"); shot("g07-menu")
    click(MENU_X, NICHT_STOEREN, 1500); away(); shot("g07-laut")
    run(f"{GET} NichtStoeren > SHOT/get-ruhe-aus.txt 2>&1", 800)
    mark("G08", "Klick: Zentrale öffnet")
    click(GLOCKE, NY, 2500); shot("g08-zentrale")
    run(f"{GET} ZentraleOffen > SHOT/get-offen.txt 2>&1", 800)
    mark("G09", "Klick: Zentrale schließt")
    click(GLOCKE, NY, 2000); away(); shot("g09-zu")
    run(f"{GET} ZentraleOffen > SHOT/get-zu.txt 2>&1", 800)
    mark("G10", "Alle als gelesen")
    click(GLOCKE, NY, 1200, "rclick")
    click(MENU_X, ALLE_GELESEN, 1500); away(); shot("g10-gelesen")
    run(f"{GET} Ungelesen > SHOT/get-ungelesen-0.txt 2>&1", 800)
    mark("G11", "Dienst beendet: wieder grau")
    run("DIENST stop", 2500); shot("g11-beendet")
    mark("G12", "Dienst wieder da: Verlauf mit 4 Einträgen, 0 ungelesen")
    run("DIENST start", 3500); shot("g12-wieder")
    mark("ENDE", "")
elif ART == "ergaenzung-alt":
    mark("E01", "altes Panel mit der Live-panel.ini")
    away(1500); shot("e01-alt")
    mark("E02", "ergänzen mit dem neuen Binary")
    # qeingabe liest Zeilen bis 511 Zeichen: je Befehl eine Zeile
    run("NEU --panel-ini ergaenzen INI > SHOT/ergaenzen.txt 2>&1", 1500)
    run("cp INI SHOT/panel.ini.ergaenzt", 1500)
    shot("e02-ergaenzt")
    mark("E03", "zweites Ergänzen ändert nichts")
    run("NEU --panel-ini ergaenzen INI > SHOT/ergaenzen-2.txt 2>&1", 2500)
    mark("E04", "zurücknehmen")
    run("NEU --panel-ini zuruecknehmen INI > SHOT/zuruecknehmen.txt 2>&1", 3000); shot("e04-zurueck")
    mark("ENDE", "")
elif ART == "ergaenzung-neu":
    mark("N01", "neues Panel mit der Live-panel.ini: ohne Knopf und Glocke")
    away(1500); shot("n01-ohne")
    mark("N02", "ergänzen: beide Panels neu, mit Knopf und Glocke")
    run("NEU --panel-ini ergaenzen INI > SHOT/ergaenzen.txt 2>&1", 3000); shot("n02-mit")
    mark("N03", "D-Bus ohne Fenster")
    run(f"{DBUS} > SHOT/dbus-leer.txt 2>&1", 2000)
    mark("N04", "zurücknehmen: wieder ohne")
    run("NEU --panel-ini zuruecknehmen INI > SHOT/zuruecknehmen.txt 2>&1", 3000); shot("n04-ohne")
    mark("N05", "D-Bus ohne Knopf (die Fensterliste kann es)")
    fenster("A"); run(f"{DBUS} > SHOT/dbus-ohne-knopf.txt 2>&1", 2000); shot("n05-minimiert")
    mark("ENDE", "")
elif ART == "standard":
    mark("S01", "ohne panel.ini: Standardlayout mit Knopf und Glocke")
    away(1500); shot("s01-standard")
    mark("S02", "Dienst: Glocke mit Zahl")
    run("DIENST start", 3500)
    run('notify-send -a Pruefung "Erste" "Mitteilung Erste"', 4000); shot("s02-eins")
    mark("ENDE", "")
else:
    sys.exit(f"unbekannte Art {ART}")

print("\n".join(out))
