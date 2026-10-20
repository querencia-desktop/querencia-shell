#!/usr/bin/env python3
"""G2-AK: qeingabe-Szenarien der isolierten Prüfung.

  szenario-g2ak.py <breite> <höhe> <art> > szenario.txt

Arten (mit G2AK_SITZUNG bzw. G2AK_PANELINI aus laeufe-g2ak.sh):
- sitzung:          Attrappe in PATH; System-Menü, „Bildschirm sperren“,
                    „Abmelden …“, „Ausschalten …“ rufen die Attrappe
- sitzung-variable: QUERENCIA_SITZUNG auf eine zweite Attrappe; Sperren
- sitzung-keins:    kein Sitzungsprogramm; Menü ohne Sperren/Ausschalten,
                    „Abmelden …“ öffnet die alte Rückfrage des Panels, Esc
- fensterliste:     zwei Testfenster auf zwei Flächen, Wechsel per Strg+Alt+
                    Pfeil, Mitnehmen per Strg+Alt+Umschalt+→, Umschalter-Klick
- fensterliste-alle: dasselbe mit alle_arbeitsflaechen=ja
- fensterliste-vorher: Fenster vor dem Panelstart (G2AK_VORHER), Grenze
- neuladen:         panel.ini ändern, kaputt schreiben, D-Bus NeuLaden,
                    eigene Änderung des Panels, Datei löschen
- vollbild:         X11-Testfenster (vorher, danach, verschoben, override),
                    Super+F und Alt+F11, ein Wayland-Fenster mit Super+F

Koordinaten vermessen bei 1024x768 (ak-erkunden*): System x 269, Menüzeilen
y 78 (Sperren), 102 (Abmelden), 126 (Ausschalten); Uhr W - 76, Rechtsklick-
menü der Uhr „Nach links“ (W - 110, 111); Umschalterzellen W - 113, W - 84.
"""
import sys

W, H, ART = int(sys.argv[1]), int(sys.argv[2]), sys.argv[3]
NY = 16
SYSTEM, MENU_X = 269, 300
SPERREN, ABMELDEN, AUSSCHALTEN = 78, 102, 126
UHR = W - 76
ZELLE = {1: W - 113, 2: W - 84}
AWAY = (W // 2, H // 2 + 150)
out = []


def z(*lines):
    out.extend(lines)


def mark(key, text):
    z(f"mark {key} {text}")


def click(x, y, wait=900, button="click"):
    z(f"move {x} {y}", "wait 250", button, f"wait {wait}")


def shot(name):
    z(f"shot SHOT/{name}.png")


def away(wait=500):
    z(f"move {AWAY[0]} {AWAY[1]}", f"wait {wait}")


def chord(*keys, wait=1500):
    """Tasten nacheinander drücken, rückwärts loslassen."""
    z(*[f"keydown {k}" for k in keys], *[f"keyup {k}" for k in reversed(keys)], f"wait {wait}")


def run(command, wait):
    z(f"run {command}", f"wait {wait}")


def fenster(app_id, titel, farbe, wait=4000):
    run(f'python3 -I WERKZEUG/testfenster.py {app_id} "{titel}" "{farbe}"', wait)


def system_menu():
    click(SYSTEM, NY, 1200)


if ART == "sitzung":
    mark("S01", "System-Menü mit Bildschirm sperren und Strg+Alt+L")
    system_menu(); shot("s01-system")
    mark("S02", "Bildschirm sperren")
    click(MENU_X, SPERREN, 1500); shot("s02-gesperrt-attrappe")
    mark("S03", "Abmelden")
    system_menu(); click(MENU_X, ABMELDEN, 1500)
    mark("S04", "Ausschalten")
    system_menu(); click(MENU_X, AUSSCHALTEN, 1500)
    mark("ENDE", "")
elif ART == "sitzung-variable":
    mark("V01", "System-Menü, Sperren über QUERENCIA_SITZUNG")
    system_menu(); shot("v01-system")
    click(MENU_X, SPERREN, 1500)
    mark("ENDE", "")
elif ART == "sitzung-keins":
    mark("K01", "System-Menü ohne Sitzungsprogramm")
    system_menu(); shot("k01-system")
    mark("K02", "Abmelden: alte Rückfrage des Panels")
    # ohne Sperren steht „Abmelden …“ in der ersten Sitzungszeile
    click(MENU_X, SPERREN, 1500); shot("k02-rueckfrage")
    mark("K03", "Esc bricht ab")
    z("key Escape", "wait 1200"); shot("k03-abgebrochen")
    mark("ENDE", "")
elif ART in ("fensterliste", "fensterliste-alle"):
    mark("F01", "Fenster A auf Fläche 1")
    fenster("org.querencia.TestA", "Fenster A", "#5a8fd0"); away(); shot("f01-a")
    mark("F02", "Strg+Alt+→: Fläche 2")
    chord("Control_L", "Alt_L", "Right"); shot("f02-flaeche-2")
    mark("F03", "Fenster B auf Fläche 2")
    fenster("org.querencia.TestB", "Fenster B", "#5ab06a"); away(); shot("f03-b")
    mark("F04", "Strg+Alt+←: Fläche 1")
    chord("Control_L", "Alt_L", "Left"); shot("f04-flaeche-1")
    mark("F05", "Strg+Alt+Umschalt+→: A mitnehmen")
    chord("Control_L", "Alt_L", "Shift_L", "Right"); shot("f05-mitgenommen")
    mark("F06", "Strg+Alt+←: Fläche 1 leer")
    chord("Control_L", "Alt_L", "Left"); shot("f06-leer")
    mark("F07", "Umschalter: Klick auf Fläche 2")
    click(ZELLE[2], H - 16, 1500); away(); shot("f07-umschalter")
    mark("ENDE", "")
elif ART == "fensterliste-vorher":
    # G2AK_VORHER öffnete A, dann B (B aktiv), beide auf Fläche 1
    mark("P01", "Fenster vor dem Panelstart")
    away(1500); shot("p01-start")
    mark("P02", "Fläche 2: B ist zugeordnet, A noch nicht")
    chord("Control_L", "Alt_L", "Right"); shot("p02-flaeche-2")
    mark("P03", "Fläche 1, Alt+Tab aktiviert A, Fläche 2")
    chord("Control_L", "Alt_L", "Left")
    z("keydown Alt_L", "wait 100", "key Tab", "wait 400", "keyup Alt_L", "wait 1200")
    chord("Control_L", "Alt_L", "Right"); shot("p03-flaeche-2")
    mark("ENDE", "")
elif ART == "neuladen":
    ini = "INI"
    dbus = ("gdbus call --session --dest org.example.Querencia.Panel "
            "--object-path /org/example/Querencia/Panel --method org.querencia.Panel.NeuLaden")
    ersetzen = "python3 -I WERKZEUG/ersetzen.py"
    mark("N01", "Ausgangslage")
    away(1000); shot("n01-start")
    mark("N02", "panel.ini überschrieben: unten ohne Umschalter")
    run(f"cp WERKZEUG/panel-ohne-umschalter.ini {ini}", 2000); shot("n02-ohne-umschalter")
    mark("N03", "panel.ini kaputt (atomar ersetzt)")
    run(f"{ersetzen} WERKZEUG/panel-kaputt.ini {ini}", 2000); shot("n03-kaputt")
    mark("N04", "D-Bus NeuLaden bei kaputter Datei")
    run(f"{dbus} > SHOT/dbus-kaputt.txt 2>&1", 2000)
    mark("N05", "neue panel.ini: oben Orte in der Mitte, alle Arbeitsflächen")
    run(f"{ersetzen} WERKZEUG/panel-neu.ini {ini}", 2500); shot("n05-neu")
    mark("N06", "D-Bus NeuLaden ohne Änderung")
    run(f"{dbus} > SHOT/dbus-unveraendert.txt 2>&1", 2000)
    mark("N07", "eigene Änderung: Uhr nach links")
    click(UHR, NY, 1200, "rclick"); click(W - 110, 111, 2500); away(); shot("n07-uhr-links")
    mark("N08", "panel.ini gelöscht: Standardlayout")
    run(f"rm {ini}", 2500); shot("n08-geloescht")
    mark("N09", "D-Bus NeuLaden nach dem Löschen")
    run(f"{dbus} > SHOT/dbus-geloescht.txt 2>&1", 2000)
    mark("ENDE", "")
elif ART == "vollbild":
    x11 = "BIN/qx11fenster"
    def x11fenster(modus, sekunden, key, text, name, wait=3000):
        mark(key, text)
        run(f"{x11} {modus} {sekunden} SHOT/x11-{modus}.id > SHOT/x11-{modus}.txt 2>&1", wait)
        shot(name)
        run(f"xprop -id $(cat SHOT/x11-{modus}.id) _NET_WM_STATE > SHOT/xprop-{modus}.txt 2>&1", 800)
    x11fenster("vorher", 6, "X01", "X11 mit _NET_WM_STATE_FULLSCREEN vor dem Abbilden", "x01-vorher")
    z("wait 4000")
    x11fenster("danach", 7, "X02", "X11, Vollbild per _NET_WM_STATE-Nachricht nach 1 s", "x02-danach")
    z("wait 4500")
    x11fenster("verschoben", 14, "X03", "X11 bildschirmgroß an 0,0 ohne Vollbild (wie Horizon)",
               "x03-verschoben")
    mark("X04", "Super+F: labwc schaltet Vollbild ein")
    chord("Super_L", "f"); shot("x04-super-f")
    run("xprop -id $(cat SHOT/x11-verschoben.id) _NET_WM_STATE > SHOT/xprop-super-f.txt 2>&1", 800)
    mark("X05", "Alt+F11: Vollbild wieder aus")
    chord("Alt_L", "F11"); shot("x05-alt-f11")
    run("xprop -id $(cat SHOT/x11-verschoben.id) _NET_WM_STATE > SHOT/xprop-alt-f11.txt 2>&1", 800)
    z("wait 6000")
    x11fenster("override", 7, "X06", "X11 override_redirect bildschirmgroß", "x06-override")
    mark("X07", "Super+F wirkt nicht auf unverwaltete Fenster")
    chord("Super_L", "f"); shot("x07-override-super-f")
    z("wait 5000")
    mark("X08", "Wayland-Fenster: Super+F, Alt+F11")
    fenster("org.querencia.TestV", "Fenster V", "#5a8fd0"); shot("x08-wayland")
    chord("Super_L", "f"); shot("x08-wayland-super-f")
    chord("Alt_L", "F11"); shot("x08-wayland-alt-f11")
    mark("ENDE", "")
else:
    sys.exit(f"unbekannte Art {ART}")

print("\n".join(out))
