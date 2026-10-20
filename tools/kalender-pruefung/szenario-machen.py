#!/usr/bin/env python3
# G2-AD: Kopie aus tools/statussymbole-pruefung (unverändert bis auf Pfade).
"""G2-Y (QD-045): qeingabe-Szenario für die Abstände der Statussymbole.

  szenario-machen.py <breite> <höhe> statussymbole|vorher > szenario.txt

Läuft mit den G2-M-Werkzeugen (nachweis/gui/gm-werkzeug/pruefen.sh,
G2M_NETZ=echt, G2M_PANELINI=panel-mit-netzwerk.ini: End=volume-1;network-1;
clock-1, G2M_NEUSTART=1). SHOT ersetzt szenario-g2m.sh.

- vorher:         nur Ausgangslage und Hover über beiden Statussymbolen
                  (Belegbilder des Ausgangs, Koordinaten des G2-M-Panels).
- statussymbole:  Ausgangslage, Hover, Entfernen, „Zum Panel hinzufügen“,
                  „Nach links“ (Uhr, Netzwerk), „An den linken Rand“ (des
                  Panels), „Verschieben“ in die Mitte und zurück neben die
                  Lautstärke, danach Neustart.

Koordinaten aus der Bildschirmbreite W (vermessen bei 1024x768,
nachweis/gui/y-erkunden): Die Uhr endet bei W - 11 und ist etwa 132 px
breit; davor 16 px Abstand. Neu: Statusknöpfe 24 px breit, 2 px Abstand
untereinander; G2-M: 28 px, 16 px. Rechtsklickmenüs am rechten Rand,
Zeilen (y): Netzwerk Entfernen 184, Verschieben 227, Nach links 257,
An den linken Rand 317; Uhr Nach links 111. Menüs anderswo hängen unter dem
Zeiger (x + 60).
"""
import sys

W, H, ART = int(sys.argv[1]), int(sys.argv[2]), sys.argv[3]
NY = 16
CX, CY = W // 2, H // 2
RMENU_X = W - 150
AWAY = (CX, CY + 250)
UHR = W - 76
if ART == "vorher":
    LAUT, NETZ = W - 215, W - 171          # G2-M: 28 px, 16 px Abstand
else:
    LAUT, NETZ = W - 196, W - 170          # G2-Y: 24 px, 2 px Abstand
N_WEG, N_VERSCHIEBEN, N_LINKS, N_RAND = 184, 227, 257, 317
LINKS = 22                                 # Applet am linken Panelrand
U_LINKS = 111
out = []


def z(*lines):
    out.extend(lines)


def mark(key, text):
    z(f"mark {key} {text}")


def click(x, y, wait=900, button="click"):
    z(f"move {x} {y}", "wait 250", button, f"wait {wait}")


def shot(name):
    z(f"shot SHOT/{name}.png")


def icon(name, wait=700):
    """Panel ohne Hover fotografieren."""
    z(f"move {AWAY[0]} {AWAY[1]}", f"wait {wait}")
    shot(name)


def hover(x, name):
    z(f"move {x} {NY}", "wait 900")
    shot(name)


mark("Y00", "Ausgangslage: End=volume-1;network-1;clock-1")
z("wait 1500")
icon("y00-ausgang", 1000)
mark("Y01", "Hover: Lautstärke, dann Netzwerk (Rahmen zeigen Knopfbreite und Abstand)")
hover(LAUT, "y01-hover-laut")
hover(NETZ, "y01-hover-netz")
if ART == "statussymbole":
    mark("Y02", "Netzwerk: Aus dem Panel entfernen")
    click(NETZ, NY, 1000, "rclick")
    shot("y02-netz-menue")
    click(RMENU_X, N_WEG, 1500)
    icon("y02-ohne-netz")
    mark("Y03", "Zum Panel hinzufügen: Netzwerk kommt rechts neben die Lautstärke")
    click(W - 260, NY, 900, "rclick")
    click(W - 190, 50, 1500)
    z("type Netz", "wait 800")
    shot("y03-dialog")
    click(CX, CY - 103, 1800, "dclick")
    click(CX + 40, CY + 203, 1000)        # Dialog schließen
    icon("y03-hinzugefuegt")
    hover(LAUT, "y03-hover-laut")
    hover(NETZ, "y03-hover-netz")
    mark("Y04", "Uhr: Nach links (zwischen die Statussymbole)")
    click(UHR, NY, 1000, "rclick")
    click(RMENU_X, U_LINKS, 1200)
    icon("y04-uhr-links")
    mark("Y05", "Netzwerk (rechts außen): Nach links, wieder neben die Lautstärke")
    click(W - 22, NY, 1000, "rclick")
    click(RMENU_X, N_LINKS, 1200)
    icon("y05-netz-links")
    mark("Y06", "Netzwerk: An den linken Rand (des Panels, vor „Anwendungen“)")
    click(NETZ, NY, 1000, "rclick")
    click(RMENU_X, N_RAND, 1200)
    icon("y06-netz-rand")
    mark("Y07", "Netzwerk: Verschieben vom linken Rand in die Mitte, Linksklick setzt")
    click(LINKS, NY, 1000, "rclick")
    click(LINKS + 60, N_VERSCHIEBEN, 900)
    z(f"glide {CX} {NY} 500", "wait 400", "click", "wait 1200")
    icon("y07-netz-mitte")
    mark("Y08", "Netzwerk: Verschieben zurück zwischen Lautstärke und Uhr")
    click(CX, NY, 1000, "rclick")
    click(CX + 60, N_VERSCHIEBEN, 900)
    z(f"glide {W - 130} {NY} 600", "wait 400", "click", "wait 1200")
    icon("y08-netz-zurueck")
    hover(LAUT, "y08-hover-laut")
    hover(NETZ, "y08-hover-netz")
    icon("y09-ende")                      # Zeiger weg vor dem Neustart
mark("ENDE", "")
print("\n".join(out))
