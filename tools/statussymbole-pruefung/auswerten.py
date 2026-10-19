#!/usr/bin/env python3
"""G2-Y (QD-045): wertet einen Lauf von szenario-machen.py aus.

  auswerten.py <logs-ordner> statussymbole|vorher <breite> <höhe>

Misst im oberen Panel (Zeilen 0–31, System-Python mit GdkPixbuf, kein PIL):
- Rahmen: im Hover-Bild die Spalten mit dem 1-px-Hover-Rahmen #c4bdb7 über
  die ganze Panelhöhe → Knopf von–bis, Breite, Abstand der beiden Knöpfe.
- Inhalt: Spalten, die sich vom Panelgrund unterscheiden, zu Gruppen
  zusammengefasst (Lücken bis 2 px gehören dazu). Symbole sind Gruppen mit
  mindestens 14 px Höhe, Text (Uhr, Menüs) ist niedriger.
Soll G2-Y: Statusknöpfe 24 px breit (Symbol 22 px, kein Innenrand, 1 px
Rahmen), 2 px Abstand untereinander; 16 px zur Uhr und zu anderen Applets.
Soll „vorher“ (G2-M c3cda503): 28 px breit, 16 px Abstand – damit ist auch
belegt, dass die Messung den Unterschied sieht.
"""
import os
import re
import sys

import gi

gi.require_version("GdkPixbuf", "2.0")
from gi.repository import GdkPixbuf  # noqa: E402

LOGS, ART, W, H = sys.argv[1], sys.argv[2], int(sys.argv[3]), int(sys.argv[4])
RAHMEN = (0xC4, 0xBD, 0xB7)
NEU = ART != "vorher"
BREITE, ABSTAND = (24, 2) if NEU else (28, 16)
ERGEBNISSE = []


def pruefe(name, ok, info=""):
    ERGEBNISSE.append(ok)
    print(f"{'OK    ' if ok else 'FEHLER'} {name}{': ' + info if info else ''}")


class Bild:
    def __init__(self, name):
        self.name = name
        self.pb = GdkPixbuf.Pixbuf.new_from_file(os.path.join(LOGS, name))
        self.rs, self.n = self.pb.get_rowstride(), self.pb.get_n_channels()
        self.px = self.pb.get_pixels()
        self.w = self.pb.get_width()
        # Panelgrund je Zeile an einer freien Stelle zwischen System-Menü und Mitte
        frei = int(W * 0.40)
        self.grund = [self.p(frei, y) for y in range(32)]

    def p(self, x, y):
        o = y * self.rs + x * self.n
        return tuple(self.px[o:o + 3])

    def rahmen(self):
        """Spalten, die über (fast) die ganze Panelhöhe Rahmenfarbe haben."""
        spalten = [x for x in range(self.w) if sum(1 for y in range(32) if self.p(x, y) == RAHMEN) >= 28]
        return (spalten[0], spalten[-1]) if len(spalten) >= 2 else None

    def gruppen(self, x0, x1):
        """Inhaltsgruppen (von, bis, oben, unten) zwischen x0 und x1."""
        spalten = []
        for x in range(x0, x1):
            zeilen = [y for y in range(1, 31)
                      if sum(abs(a - b) for a, b in zip(self.p(x, y), self.grund[y])) > 30]
            spalten.append((x, zeilen))
        gruppen, aktuell, luecke = [], None, 0
        for x, zeilen in spalten:
            if zeilen:
                if aktuell and luecke <= 2:
                    aktuell[1] = x
                    aktuell[2] = min(aktuell[2], zeilen[0])
                    aktuell[3] = max(aktuell[3], zeilen[-1])
                else:
                    if aktuell:
                        gruppen.append(tuple(aktuell))
                    aktuell = [x, x, zeilen[0], zeilen[-1]]
                luecke = 0
            else:
                luecke += 1
        if aktuell:
            gruppen.append(tuple(aktuell))
        return gruppen

    def anordnung(self, x0, x1):
        """Folge 'S' (Symbol) / 'T' (Text) mit den Gruppen."""
        folge = []
        for g in self.gruppen(x0, x1):
            art = "S" if g[3] - g[2] + 1 >= 14 else "T"
            if folge and art == "T" and folge[-1][0] == "T" and g[0] - folge[-1][1][1] <= 8:
                folge[-1] = ("T", (folge[-1][1][0], g[1], 0, 0))   # Uhrzeit zusammenfassen
            else:
                folge.append((art, g))
        return folge


def da(name):
    return os.path.exists(os.path.join(LOGS, name))


def hover_paar(links, rechts, schritt):
    """Hover-Rahmen zweier Nachbarn: Breite und Abstand."""
    if not (da(links) and da(rechts)):
        pruefe(f"{schritt} Hover-Bilder vorhanden", False, f"{links} {rechts}")
        return
    a, b = Bild(links).rahmen(), Bild(rechts).rahmen()
    if not (a and b):
        pruefe(f"{schritt} Hover-Rahmen gefunden", False, f"{a} {b}")
        return
    wa, wb = a[1] - a[0] + 1, b[1] - b[0] + 1
    pruefe(f"{schritt} Knopf links {wa} px breit (Soll {BREITE})", wa == BREITE, f"x {a[0]}–{a[1]}")
    pruefe(f"{schritt} Knopf rechts {wb} px breit (Soll {BREITE})", wb == BREITE, f"x {b[0]}–{b[1]}")
    gap = b[0] - a[1] - 1
    pruefe(f"{schritt} Abstand der Knöpfe {gap} px (Soll {ABSTAND})", gap == ABSTAND)
    # gleiche Symbolgröße und gleicher Innenrand: Inhalt liegt im Rahmen,
    # Rahmen ohne Innenrand = Symbol 22 px
    for bild, r, seite in ((links, a, "links"), (rechts, b, "rechts")):
        g = Bild(bild).gruppen(r[0] + 1, r[1])
        if g:
            inhalt = (min(x[0] for x in g), max(x[1] for x in g))
            pruefe(f"{schritt} Symbol {seite} im Rahmen", r[0] < inhalt[0] and inhalt[1] < r[1],
                   f"Inhalt x {inhalt[0]}–{inhalt[1]}")


def statusfolge(bild, soll, schritt, bereich=None, eng=True):
    """Prüft im rechten Bereich die Folge der Symbole/Texte und die
    sichtbaren Abstände (Statussymbole nebeneinander 4–9 px, alles andere
    mindestens 16 px). eng=False: die Symbole sind kein Statuspaar (z. B.
    Statussymbol neben dem Logo von „Anwendungen“), also mindestens 16 px."""
    b = Bild(bild)
    x0, x1 = bereich or (W // 2 + 60, W)
    folge = b.anordnung(x0, x1)
    ist = "".join(a for a, _ in folge)
    pruefe(f"{schritt} Anordnung {ist} (Soll {soll})", ist == soll,
           ", ".join(f"{a} {g[0]}–{g[1]}" for a, g in folge))
    if ist != soll:
        return
    for (a, g), (c, h) in zip(folge, folge[1:]):
        luecke = h[0] - g[1] - 1
        if a == c == "S" and eng:
            ok = 4 <= luecke <= 9 if NEU else luecke >= 20
            pruefe(f"{schritt} sichtbarer Abstand Symbol–Symbol {luecke} px "
                   f"({'4–9' if NEU else '≥ 20'})", ok)
        else:
            pruefe(f"{schritt} sichtbarer Abstand {a}–{c} {luecke} px (≥ 16)", luecke >= 16)


def ini(name):
    try:
        text = open(os.path.join(LOGS, name)).read()
    except OSError:
        return {}
    oben = text.split("[Panel top]")[1].split("[")[0]
    return dict(re.findall(r"^(Start|Center|End)=(.*)$", oben, re.M))


print(f"G2-Y Statussymbole {ART} {W}x{H}: {LOGS}")
statusfolge("y00-ausgang.png", "SST", "Y00")
hover_paar("y01-hover-laut.png", "y01-hover-netz.png", "Y01")
if ART == "statussymbole":
    statusfolge("y02-ohne-netz.png", "ST", "Y02")
    statusfolge("y03-hinzugefuegt.png", "SST", "Y03")
    hover_paar("y03-hover-laut.png", "y03-hover-netz.png", "Y03")
    statusfolge("y04-uhr-links.png", "STS", "Y04")
    statusfolge("y05-netz-links.png", "SST", "Y05")
    statusfolge("y06-netz-rand.png", "ST", "Y06 rechts")
    statusfolge("y06-netz-rand.png", "SS", "Y06 links (Netzwerk, Logo)", (0, 80), eng=False)
    statusfolge("y07-netz-mitte.png", "ST", "Y07 rechts")
    statusfolge("y07-netz-mitte.png", "S", "Y07 Mitte", (W // 2 - 60, W // 2 + 60))
    statusfolge("y07-netz-mitte.png", "S", "Y07 links (nur Logo)", (0, 40))
    statusfolge("y08-netz-zurueck.png", "SST", "Y08")
    hover_paar("y08-hover-laut.png", "y08-hover-netz.png", "Y08")
    if da("nach-neustart.png"):
        statusfolge("nach-neustart.png", "SST", "Neustart")
    else:
        pruefe("Neustart-Bild vorhanden", False)
    ende = ini("panel.ini.ende")
    pruefe("panel.ini am Ende: End=volume-1;network-1;clock-1, Center leer",
           ende.get("End") == "volume-1;network-1;clock-1" and ende.get("Center", "") == "", str(ende))
    log = open(os.path.join(LOGS, "panel.log"), errors="replace").read()
    pruefe("Panel-Log ohne panicked/CRITICAL", "panicked" not in log and "CRITICAL" not in log)
ok = sum(ERGEBNISSE)
print(f"{ok}/{len(ERGEBNISSE)} Prüfungen bestanden")
