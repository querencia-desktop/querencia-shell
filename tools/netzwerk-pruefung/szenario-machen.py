#!/usr/bin/env python3
"""G2-M: erzeugt die qeingabe-Szenarien des Netzwerk-Applets für eine Auflösung.

  szenario-machen.py <breite> <höhe> rueckfall|attrappe|spaet > szenario.txt

- rueckfall: echter Systembus ohne NetworkManager, echte eth0 (nur lesend);
  Panel mit Lautstärke, Netzwerk kommt über „Zum Panel hinzufügen“ dazu.
- attrappe:  eigener Systembus mit der NetworkManager-Attrappe (Kabel
  enp3s0, WLAN wlp2s0 mit Heimnetz/Cafe offen/Nachbar).
- spaet:     eigener Systembus, zuerst ohne NetworkManager (Rückfall), dann
  startet die Attrappe, endet, startet ohne WLAN – ohne Panel-Neustart.

Koordinaten aus der Bildschirmgröße (vermessen bei 1024x768, Bilder in
nachweis/gui/erkunden): Applet bei x = Breite - 171 (End=volume-1;
network-1;clock-1), sein Linksklickmenü hängt rechtsbündig darunter
(Zeilen ab y = 47), das Rechtsklickmenü (PopoverMenu) schiebt sich an den
rechten Rand, Dialoge öffnet labwc mittig. Im Linksklickmenü wird meist mit
der Tastatur gewählt (Runter/Ende/Hoch, Eingabe): die Reihenfolge der
wählbaren Zeilen steht als Kommentar dabei und im Panel-Log
(„Tastatur: Zeile i von n“). SHOT, QNMCTL, QNM, WERKZEUG und EINST ersetzt
szenario-g2m.sh.
"""
import sys

W, H, ART = int(sys.argv[1]), int(sys.argv[2]), sys.argv[3]
NX, NY = W - 171, 16            # Netzwerk-Applet
CX, CY = W // 2, H // 2          # Bildschirmmitte (Dialoge)
MENU_X = W - 300                 # Linksklickmenü (rechtsbündig unter dem Applet)
RMENU_X = W - 150                # Rechtsklickmenü am rechten Rand
R_WLAN, R_FLUG, R_INFO, R_EINST, R_WEG = 38, 68, 111, 141, 184
AWAY = (CX, CY + 250)            # Zeiger weg vom Applet (Symbol ohne Hover)
out = []


def z(*lines):
    out.extend(lines)


def mark(key, text):
    z(f"mark {key} {text}")


def click(x, y, wait=900, button="click"):
    z(f"move {x} {y}", "wait 250", button, f"wait {wait}")


def shot(name):
    z(f"shot SHOT/{name}.png")


def away(wait=700):
    z(f"move {AWAY[0]} {AWAY[1]}", f"wait {wait}")


def icon(name, wait=700):
    """Symbol ohne Hover fotografieren."""
    away(wait)
    shot(name)


def tooltip(name):
    z(f"move {NX} {NY + 1}", "wait 2200")
    shot(name)
    away(400)


def menu():
    click(NX, NY, 1200)


def keys(*names, wait=250):
    for name in names:
        z(f"key {name}", f"wait {wait}")


def rmenu(item_y, wait=1200):
    click(NX, NY, 1000, "rclick")
    click(RMENU_X, item_y, wait)


def qnm(cmd, wait=1500):
    z(f"run QNMCTL {cmd}", f"wait {wait}")


def add_network_via_dialog(prefix):
    """Rechtsklick auf freie Fläche oben rechts, Dialog, „Netz“ suchen,
    Doppelklick auf die erste Zeile, Dialog schließen."""
    click(W - 260, NY, 900, "rclick")
    shot(f"{prefix}-panelmenue")
    click(W - 190, 50, 1500)
    z("type Netz", "wait 800")
    shot(f"{prefix}-dialog")
    click(CX, CY - 103, 1800, "dclick")
    shot(f"{prefix}-hinzugefuegt")


def close_add_dialog():
    click(CX + 40, CY + 203, 1000)


if ART == "rueckfall":
    mark("R00", "Ausgangslage: Panel mit Lautstärke, ohne Netzwerk; echter Systembus ohne NetworkManager")
    z("wait 1000"); shot("r00-ausgang")
    mark("R01", "Zum Panel hinzufügen: Netzwerk (rechts neben Lautstärke, vor der Uhr)")
    add_network_via_dialog("r01")
    mark("R02", "zweites Mal: Netzwerk gesperrt")
    click(CX, CY - 103, 1200, "dclick"); shot("r02-gesperrt")
    close_add_dialog()
    mark("R03", "Symbol: Kabel verbunden (echte eth0)")
    icon("r03-symbol")
    mark("R04", "Tooltip mit Adresse und „nur Anzeige“")
    tooltip("r04-tooltip")
    mark("R05", "Linksklickmenü: Verwaltet von systemd-networkd, eth0 mit Adresse, nichts zum Schalten")
    menu(); shot("r05-menue")
    # wählbar: [Verbindungsinformationen]
    mark("R06", "Tastatur: Runter wählt Verbindungsinformationen, Eingabe öffnet den Dialog")
    keys("Down"); shot("r06-taste")
    keys("Return", wait=1500); shot("r06-info")
    mark("R07", "Info-Dialog: Eingabetaste löst Schließen aus (Tastaturfokus)")
    keys("Return", wait=1200); shot("r07-info-zu")
    mark("R08", "Rechtsklickmenü: WLAN/Flugmodus grau, Einstellungen „kommt mit G2-O“")
    click(NX, NY, 1000, "rclick"); shot("r08-rechts")
    click(RMENU_X, R_INFO, 1500); shot("r08-info")
    mark("R09", "Info-Dialog: Escape schließt")
    keys("Escape", wait=1000); shot("r09-info-zu")
    mark("R10", "Aus dem Panel entfernen")
    rmenu(R_WEG, 1500); icon("r10-entfernt")
    mark("ENDE", "")
elif ART == "attrappe":
    mark("M00", "Ausgangslage: Attrappe, Kabel verbunden")
    z("wait 800"); icon("m00-kabel")
    mark("M01", "Tooltip")
    tooltip("m01-tooltip")
    mark("M02", "Linksklickmenü per Maus: Kabel, drei WLANs mit Schloss, Schalter, Einstellungen grau")
    menu(); shot("m02-menue")
    # wählbar: [Trennen, Heimnetz, Cafe offen, Nachbar, verborgen, WLAN, Flugmodus, Info]
    mark("M03", "Tastatur: zweimal Runter hebt Heimnetz hervor, Escape schließt")
    keys("Down", "Down"); shot("m03-tasten")
    keys("Escape", wait=800)
    mark("M04", "Kabel ab (Attrappe): kein Netz")
    qnm("kabel ab"); icon("m04-kabel-ab")
    tooltip("m04-tooltip")
    mark("M05", "Menü per Maus: „Cafe offen“ (offen) verbinden")
    menu(); shot("m05-menue")
    click(MENU_X, 147, 350); shot("m05-verbindet-menue")
    away(0); shot("m05-verbindet")
    z("wait 2500"); icon("m05-verbunden", 300)
    tooltip("m05-tooltip")
    mark("M06", "Signal 90 % → vier Stufen")
    qnm("signal Cafe offen 90"); icon("m06-signal-90")
    mark("M07", "Signal 10 %")
    qnm("signal Cafe offen 10"); icon("m07-signal-10")
    mark("M08", "Signal 60 %")
    qnm("signal Cafe offen 60"); icon("m08-signal-60")
    mark("M09", "Menü: verbundenes Netz fett mit Adresse, Trennen")
    menu(); shot("m09-menue")
    # wählbar: [Heimnetz, Nachbar, Trennen, verborgen, WLAN, Flugmodus, Info]
    mark("M10", "Nachbar (WPA3, neu): Passwortdialog")
    keys("Down", "Down"); keys("Return", wait=1500); shot("m10-passwort")
    mark("M11", "zu kurzes Passwort: Verbinden grau, Eingabe tut nichts")
    z("type kurz", "wait 500"); keys("Return", wait=700); shot("m11-kurz")
    mark("M12", "richtiges Passwort, Eingabe verbindet")
    keys("Home", "Delete", "Delete", "Delete", "Delete", wait=150)
    z("type nachbar99", "wait 400"); keys("Return", wait=3200)
    icon("m12-nachbar-verbunden", 300)
    tooltip("m12-tooltip")
    mark("M13", "Heimnetz (gespeichert, Schlüssel vom Passwortagenten): Abfrage durch NetworkManager")
    menu()
    # wählbar: [Heimnetz, Cafe offen, Trennen, verborgen, WLAN, Flugmodus, Info]
    keys("Down"); keys("Return", wait=2200); shot("m13-agent")
    mark("M14", "falsches Passwort: NetworkManager fragt erneut")
    z("type falsch123", "wait 400"); keys("Return", wait=2400); shot("m14-erneut")
    mark("M15", "richtiges Passwort: verbunden")
    z("type geheim123", "wait 400"); keys("Return", wait=3000)
    icon("m15-heimnetz-verbunden", 300)
    tooltip("m15-tooltip")
    mark("M16", "Kabel an: Kabel wird Standardverbindung")
    qnm("kabel an", 400); away(0); shot("m16-verbindet")
    z("wait 2500"); icon("m16-kabel", 300)
    tooltip("m16-tooltip")
    mark("M17", "Verbindungsinformationen per Tastatur (Ende, Eingabe)")
    menu(); shot("m17-menue")
    # wählbar: [Trennen, Cafe, Nachbar, Trennen, verborgen, WLAN, Flugmodus, Info]
    keys("End"); keys("Return", wait=1500); shot("m17-info")
    mark("M18", "Info-Dialog: Eingabetaste schließt (Fokus auf Schließen)")
    keys("Return", wait=1200); shot("m18-info-zu")
    mark("M19", "Info-Dialog über Rechtsklick, Reiter WLAN")
    rmenu(R_INFO, 1500)
    click(CX - 30, CY - 249, 1000); shot("m19-info-wlan")
    keys("Escape", wait=1000)
    mark("M20", "Flugmodus über Rechtsklick: WLAN getrennt, Kabel bleibt")
    click(NX, NY, 1000, "rclick"); shot("m20-rechts")
    click(RMENU_X, R_FLUG, 1800); icon("m20-flugmodus")
    click(NX, NY, 1000, "rclick"); shot("m20-rechts-haken")
    keys("Escape", wait=800)
    menu(); shot("m20-menue"); keys("Escape", wait=800)
    mark("M21", "Kabel ab im Flugmodus: Kabel getrennt")
    qnm("kabel ab"); icon("m21-kabel-getrennt")
    tooltip("m21-tooltip")
    mark("M22", "Flugmodus aus im Linksklickmenü (Tastatur)")
    menu()
    # wählbar: [WLAN, Flugmodus, Info]
    keys("Down", "Down"); keys("Return", wait=1800); icon("m22-flugmodus-aus")
    mark("M23", "Kabel an, Kabel trennen (Menü), WLAN aus (Rechtsklick): WLAN aus")
    qnm("kabel an", 3000)
    menu()
    # wählbar: [Trennen, Heimnetz, Cafe, Nachbar, verborgen, WLAN, Flugmodus, Info]
    keys("Down"); keys("Return", wait=1500)
    rmenu(R_WLAN, 1800); icon("m23-wlan-aus")
    tooltip("m23-tooltip")
    menu(); shot("m23-menue"); keys("Escape", wait=800)
    mark("M24", "WLAN wieder an (Rechtsklick)")
    rmenu(R_WLAN, 1800); icon("m24-wlan-an")
    mark("M25", "Mit verborgenem Netz verbinden: Dialog")
    menu()
    # wählbar: [Verbinden, Heimnetz, Cafe, Nachbar, verborgen, WLAN, Flugmodus, Info]
    keys("Down", "Down", "Down", "Down", "Down"); keys("Return", wait=1500); shot("m25-verborgen")
    mark("M26", "Netzname, Passwort, Verbinden")
    z("type Versteckt", "wait 300"); keys("Tab", "Tab", wait=300)
    z("type versteckt1", "wait 500"); shot("m26-ausgefuellt")
    keys("Return", wait=3200); icon("m26-verbunden", 300)
    tooltip("m26-tooltip")
    mark("M27", "sieben weitere Netze: acht im Menü, „Weitere Netze …“")
    for i, s in enumerate((64, 58, 52, 47, 41, 36, 30)):
        qnm(f"netz-neu Gast {i + 1} {s} offen", 250)
    z("wait 1200")
    menu(); shot("m27-menue")
    # wählbar: [Verbinden, 8 Netze, Weitere, Trennen, verborgen, WLAN, Flugmodus, Info]
    mark("M28", "Weitere Netze (Tastatur: Ende, fünfmal Hoch, Eingabe): alle Netze")
    keys("End", "Up", "Up", "Up", "Up", "Up"); keys("Return", wait=1000); shot("m28-alle")
    keys("Escape", wait=800)
    mark("M29", "Netz verschwindet bei offenem Menü")
    menu()
    qnm("netz-weg Heimnetz", 1500); shot("m29-ohne-heimnetz")
    keys("Escape", wait=800)
    mark("M30", "Netzwerkeinstellungen erscheinen (G2-O-Eintrag), Start über die Aktion netzwerk")
    z("run EINST", "wait 2500")
    click(NX, NY, 1000, "rclick"); shot("m30-rechts-einstellungen")
    click(RMENU_X, R_EINST, 1500)
    mark("M31", "Aus dem Panel entfernen")
    rmenu(R_WEG, 1500); icon("m31-entfernt")
    mark("ENDE", "")
elif ART == "spaet":
    mark("S00", "Ausgangslage: eigener Systembus ohne NetworkManager (Rückfall, echte eth0)")
    z("wait 800"); icon("s00-rueckfall")
    tooltip("s00-tooltip")
    menu(); shot("s00-menue"); keys("Escape", wait=800)
    mark("S01", "NetworkManager startet: Wechsel ohne Panel-Neustart")
    z("run QNM SHOT/qnm-spaet.log", "wait 3500"); icon("s01-attrappe")
    tooltip("s01-tooltip")
    menu(); shot("s01-menue"); keys("Escape", wait=800)
    mark("S02", "NetworkManager endet: zurück zum Rückfall")
    qnm("beenden", 3000); icon("s02-rueckfall")
    tooltip("s02-tooltip")
    mark("S03", "NetworkManager startet ohne WLAN-Gerät")
    z("run QNM SHOT/qnm-spaet-2.log --ohne-wlan", "wait 3500"); icon("s03-nur-kabel")
    menu(); shot("s03-menue"); keys("Escape", wait=800)
    click(NX, NY, 1000, "rclick"); shot("s03-rechts"); keys("Escape", wait=800)
    mark("S04", "Kabel ab: Kabel getrennt (kein WLAN-Gerät)")
    qnm("kabel ab"); icon("s04-kabel-getrennt")
    mark("S05", "Verbindungsinformationen (Rechtsklick), Eingabe schließt")
    rmenu(R_INFO, 1500); shot("s05-info")
    keys("Return", wait=1200); shot("s05-info-zu")
    mark("ENDE", "")
else:
    sys.exit("Art: rueckfall, attrappe oder spaet")
print("\n".join(out))
