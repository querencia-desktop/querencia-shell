#!/usr/bin/env python3
"""HMP-10a: Prüftreiber für Raster (Snap-to-Grid) und Mehrfachauswahl.

Läuft wie hmp10-treiber.py in der isolierten labwc-Sitzung
(pruefen-hmp10.sh mit HMP10_TREIBER=…/hmp10a-treiber.py) und benutzt dessen
Hilfsfunktionen (echte Eingaben über qeingabe, Lage aus dem Protokoll,
Bilder, Zustandsdatei, Dateisystem, Markerprogramme).

Phasen:
  HMP10_PHASE=1          alle Prüfungen R (Raster) und M (Mehrfachauswahl)
  HMP10_PHASE=2          erneute Anmeldung (neuer labwc, gleiches Profil)
  HMP10_NUR=migration    vorhandene HMP-10-Zustandsdatei neben dem Raster
"""

import importlib.util
import json
import math
import os
import re
import shutil
import subprocess
import time
import traceback

HIER = os.path.dirname(os.path.abspath(__file__))
_spec = importlib.util.spec_from_file_location("t", os.path.join(HIER, "hmp10-treiber.py"))
t = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(t)

P, LOGS, SCHREIBTISCH, ZUSTAND = t.P, t.LOGS, t.SCHREIBTISCH, t.ZUSTAND
MARGIN, CELL = t.MARGIN, t.CELL
d, pruefe, bild, lage, flaeche, warte = t.d, t.pruefe, t.bild, t.lage, t.flaeche, t.warte
ENDE_LAGE = os.path.join(LOGS, "lage-ende-phase1.json")
EXTRA = ["Notizen.txt", "Bilder/", "Musik/", "Ein sehr langer Dateiname mit vielen Wörtern.txt",
         "WWWWWWWWWWWWWWWWWWWWWWWW.txt", "x.txt"]


def q(*befehle):
    t.q(*befehle)


def runden(v):
    return math.floor(v + 0.5)


def raster(w=None, h=None):
    """(Spalten, Zeilen, Abstand x, Abstand y) wie layout.rs grid_axis."""
    if w is None:
        w, h = flaeche()

    def achse(laenge, kachel):
        platz = laenge - 2 * MARGIN
        n = max(1, platz // kachel)
        return n, ((platz - kachel) / (n - 1) if n > 1 else float(kachel))
    (c, px), (r, py) = achse(w, CELL[0]), achse(h, CELL[1])
    return c, r, px, py


def zelle(c, r, w=None, h=None):
    _, _, px, py = raster(w, h)
    return MARGIN + runden(c * px), MARGIN + runden(r * py)


def zelle_von(p, w=None, h=None):
    cs, rs, px, py = raster(w, h)
    return (min(max(runden((p[0] - MARGIN) / px), 0), cs - 1),
            min(max(runden((p[1] - MARGIN) / py), 0), rs - 1))


def auf_raster(l, w=None, h=None):
    falsch = {n: p for n, p in l.items() if zelle(*zelle_von(p, w, h), w, h) != p}
    return not falsch and t.voll(l), falsch


def ziel(c, r, versatz=(0, 0)):
    """Bildschirmpunkt für ein Ablegen in Zelle (c, r) mit Versatz (Griff in der Mitte)."""
    x, y = t.mitte(zelle(c, r))
    return x + versatz[0], y + versatz[1]


def auswahl():
    zeilen = [z for z in d.text().splitlines() if "querencia-desktop: Auswahl: " in z]
    if not zeilen:
        return set()
    text = zeilen[-1].split("Auswahl: ", 1)[1]
    return set(text.split("; ")) if text else set()


def neue_auswahl(erwartet, sekunden=3):
    return warte(lambda: auswahl() == set(erwartet), sekunden, f"Auswahl {sorted(erwartet)}") or False


def strg_klick(punkt):
    q(f"move {punkt[0]} {punkt[1]}", "wait 150", "keydown Control_L", "wait 80", "click", "wait 80",
      "keyup Control_L", "wait 400")


def umschalt_klick(punkt):
    q(f"move {punkt[0]} {punkt[1]}", "wait 150", "keydown Shift_L", "wait 80", "click", "wait 80",
      "keyup Shift_L", "wait 400")


def rahmen(von, nach, zwischenbild=None):
    q(f"move {von[0]} {von[1]}", "wait 150", "press", "wait 100")
    if zwischenbild:
        mx, my = (von[0] + nach[0]) // 2, (von[1] + nach[1]) // 2
        q(f"glide {mx} {my} 250", "wait 200")
        q(f"glide {nach[0]} {nach[1]} 250", "wait 250")
        bild(zwischenbild)
    else:
        q(f"glide {nach[0]} {nach[1]} 400", "wait 250")
    q("release", "wait 500")


def frei_punkt():
    """Ein Bildschirmpunkt auf freier Fläche (in keiner Kachel, 6 px Abstand),
    mit Platz für ein Menü rechts darunter."""
    l = lage()
    w, h = flaeche()
    for y in range(20, h - 230, 10):
        for x in range(20, w - 230, 10):
            if all(not (px - 6 <= x <= px + CELL[0] + 6 and py - 6 <= y <= py + CELL[1] + 6)
                   for px, py in l.values()):
                return x, y + t.oben[0]
    raise RuntimeError("kein freier Platz")


def menue(eintrag_y):
    """Arbeitsflächenmenü an einer freien Stelle, Eintrag in Höhe eintrag_y anklicken."""
    f = frei_punkt()
    t.klick(f, "rclick")
    q("wait 500")
    t.klick((f[0] + 60, f[1] + eintrag_y))
    q("wait 600")


# Einträge des Arbeitsflächenmenüs (Zeilen 24 px, Trennlinie 7 px)
ORDNER, AUFRAEUMEN, AM_RASTER, RASTER_AN_AUS = 15, 70, 94, 118


def waehle_per_rahmen(namen):
    """Wählt `namen` (eine Reihe) mit einem Rahmen aus, der in den Lücken beginnt und endet."""
    q("key Escape", "wait 200")
    l = lage()
    xs = [l[n][0] for n in namen]
    y = l[namen[0]][1] + t.oben[0]
    rahmen((min(xs) - 2, y + 40), (max(xs) + CELL[0] + 2, y + 50))
    return neue_auswahl(set(namen))


def form(l, namen):
    """Lage der Gruppe in Zellen, relativ zum ersten Symbol."""
    zellen = [zelle_von(l[n]) for n in namen]
    return [(c - zellen[0][0], r - zellen[0][1]) for c, r in zellen]


def pixelform(l, namen):
    return [(l[n][0] - l[namen[0]][0], l[n][1] - l[namen[0]][1]) for n in namen]


def gleiche_pixelform(a, b, namen):
    """Pixelabstände gleich bis auf die Rundung des Rasters (±1 px)."""
    return all(abs(p[0] - q_[0]) <= 1 and abs(p[1] - q_[1]) <= 1 for p, q_ in zip(pixelform(a, namen), pixelform(b, namen)))


def unveraendert(vor, nach, ausser=()):
    return {k: v for k, v in vor.items() if k not in ausser} == {k: v for k, v in nach.items() if k not in ausser}


# ----------------------------------------------------------------------------
# R: Raster

def test_r01_start():
    l = lage()
    im = bild("start-raster")
    pruefe("R01 14 Symbole", len(l) == 14, sorted(l))
    ok, falsch = auf_raster(l)
    pruefe("R01 alle Symbole auf Rasterzellen", ok, falsch)
    ok, why = t.ueberlappungsfrei(l)
    pruefe("R01 keine Überschneidung", ok, why)
    zu_gross = [z for z in d.text().splitlines() if "Kachel größer als die Zelle" in z]
    pruefe("R01 lange Beschriftungen bleiben in ihrer Zelle", not zu_gross, zu_gross)
    pruefe("R01 Raster gleichmäßig (12×7 bei 1280×656)", raster()[:2] == (12, 7), raster())
    pruefe("R01 alle sichtbar (Pixel)", all(t.symbol_da(im, p)[0] for p in l.values()))


def test_r02_nebeneinander():
    vor = lage()
    t.ziehen(t.mitte(vor["x.txt"]), ziel(4, 1, (30, 20)), 500)
    t.ziehen(t.mitte(vor["Notizen.txt"]), ziel(5, 1, (-35, 25)), 500)
    l = t.neue_lage(lambda m: m.get("Notizen.txt") == zelle(5, 1), 4)
    bild("zwei-nebeneinander")
    pruefe("R02 zwei Symbole rasten nebeneinander ein", l.get("x.txt") == zelle(4, 1)
           and l.get("Notizen.txt") == zelle(5, 1), (l.get("x.txt"), l.get("Notizen.txt")))
    pruefe("R02 gleiche Zeile, Abstand eine Zelle",
           l["x.txt"][1] == l["Notizen.txt"][1] and abs(l["Notizen.txt"][0] - l["x.txt"][0] - raster()[2]) <= 1)
    pruefe("R02 übrige unverändert", unveraendert(vor, l, ("x.txt", "Notizen.txt")))


def test_r03_r04_reihen():
    reihe = ["Apfel", "Zebra.txt", "Bilder", "Musik", "Mein Browser"]
    versatz = [(30, 25), (-30, -20), (25, -30), (-25, 30), (35, 10)]
    for i, (name, v) in enumerate(zip(reihe, versatz)):
        t.ziehen(t.mitte(lage()[name]), ziel(3 + i, 3, v), 450)
    l = t.neue_lage(lambda m: m.get("Mein Browser") == zelle(7, 3), 4)
    bild("fuenf-in-einer-reihe")
    ys = {l.get(n, (0, -1))[1] for n in reihe}
    xs = [l.get(n, (-1, 0))[0] for n in reihe]
    pruefe("R03 fünf Symbole in einer Reihe (gleiche Höhe)", ys == {zelle(0, 3)[1]}, [l.get(n) for n in reihe])
    abstaende = [b - a for a, b in zip(xs, xs[1:])]
    pruefe("R03 gleichmäßige Abstände", all(abs(a - raster()[2]) <= 1 for a in abstaende), abstaende)
    zweite = ["Fremder Starter", "WWWWWWWWWWWWWWWWWWWWWWWW.txt", "Ein sehr langer Dateiname mit vielen Wörtern.txt"]
    for i, (name, v) in enumerate(zip(zweite, [(-20, 30), (30, -25), (0, 35)])):
        t.ziehen(t.mitte(lage()[name]), ziel(3 + i, 4, v), 450)
    l = t.neue_lage(lambda m: m.get(zweite[2]) == zelle(5, 4), 4)
    bild("mehrere-reihen")
    pruefe("R04 zweite Reihe genau unter der ersten (gleiche Spalten)",
           all(l.get(zweite[i], (0, 0))[0] == l.get(reihe[i], (1, 1))[0] for i in range(3))
           and {l.get(n, (0, -1))[1] for n in zweite} == {zelle(0, 4)[1]}, [l.get(n) for n in zweite])
    ok, why = t.ueberlappungsfrei(l)
    pruefe("R04 keine Überschneidung", ok, why)


def test_r05_belegt():
    vor = lage()
    t.ziehen(t.mitte(vor["Rechner"]), ziel(5, 3, (10, 10)), 500)  # dort liegt „Bilder“
    l = t.neue_lage(lambda m: m.get("Rechner") != vor["Rechner"], 4)
    pruefe("R05 belegter Rasterplatz: nächste freie Zelle (5,2)", l.get("Rechner") == zelle(5, 2), l.get("Rechner"))
    pruefe("R05 Inhaber bleibt, nichts anderes bewegt", unveraendert(vor, l, ("Rechner",)))
    ok, why = t.ueberlappungsfrei(l)
    pruefe("R05 keine Überschneidung", ok, why)
    bild("belegter-platz")


def test_r06_rand():
    vor = lage()
    w, h = flaeche()
    cs, rs, _, _ = raster()
    t.ziehen(t.mitte(vor["Papierkorb"]), (1279, 719), 600)        # über den Rand und das untere Panel
    t.ziehen(t.mitte(vor["Persönlicher Ordner"]), (1270, 3), 600)  # auf das obere Panel, rechts
    l = t.neue_lage(lambda m: m.get("Persönlicher Ordner") != vor["Persönlicher Ordner"], 4)
    pruefe("R06 rechts unten: letzte Zelle, bündig am Rand",
           l.get("Papierkorb") == zelle(cs - 1, rs - 1) == (w - 108, h - 96), l.get("Papierkorb"))
    pruefe("R06 rechts oben auf dem Panel abgelegt: letzte Spalte, erste Zeile",
           l.get("Persönlicher Ordner") == zelle(cs - 1, 0) == (w - 108, 8), l.get("Persönlicher Ordner"))
    ok, why = t.im_bereich(l)
    pruefe("R06 alles in der Fläche zwischen den Panels", ok, why)
    o, u = t.panelstreifen(bild("rand"))
    pruefe("R06 Panels unverdeckt", (o, u) == t.panels_vorher)


def test_r07_neustart(nr="R07", name="neustart"):
    l = lage()
    z = t.zustand()
    d.stop()
    d.start()
    neu = lage()
    pruefe(f"{nr} Positionen nach dem Neustart identisch", t.voll(l) and neu == l,
           {k: (l.get(k), neu.get(k)) for k in set(l) | set(neu) if l.get(k) != neu.get(k)})
    pruefe(f"{nr} Zustandsdatei unverändert", bool(z) and t.zustand() == z)
    ok, _ = auf_raster(neu)
    pruefe(f"{nr} weiter auf dem Raster", ok)
    bild(name)


# ----------------------------------------------------------------------------
# M: Mehrfachauswahl

def test_m01_strg():
    l = lage()
    t.klick(t.mitte(l["x.txt"]))
    strg_klick(t.mitte(l["Notizen.txt"]))
    pruefe("M01 Strg+Klick: zwei Symbole ausgewählt", neue_auswahl({"x.txt", "Notizen.txt"}), auswahl())
    im = bild("strg-zwei")
    pruefe("M01 beide deutlich hervorgehoben (Pixel)",
           t.ausgewaehlt(im, l["x.txt"]) and t.ausgewaehlt(im, l["Notizen.txt"])
           and not t.ausgewaehlt(im, l["Apfel"]))
    pruefe("M01 nichts bewegt, nichts geöffnet", lage() == l and not [m for m in t.marker() if "Notizen" in m])


def test_m14_umschalt():
    l = lage()
    t.klick(t.mitte(l["Apfel"]))
    umschalt_klick(t.mitte(l["Bilder"]))
    a, b = sorted([l["Apfel"], l["Bilder"]])
    erwartet = {n for n, p in l.items() if a <= p <= b}
    pruefe("M14 Umschalt+Klick: Bereich in Leserichtung (Spalten)", neue_auswahl(erwartet), (sorted(auswahl()), sorted(erwartet)))
    bild("umschalt")


def test_m05_esc(nr="M05"):
    q("key Escape", "wait 400")
    pruefe(f"{nr} Esc hebt die Auswahl auf", neue_auswahl(set()), auswahl())


def test_m02_rahmen():
    l = lage()
    reihe = {"Apfel", "Zebra.txt", "Bilder", "Musik", "Mein Browser"}
    y3 = zelle(0, 3)[1] + t.oben[0]
    von = (zelle(2, 3)[0] + 60, y3 + 40)                 # freie Zelle (2,3)
    nach = (zelle(7, 3)[0] + 60, y3 + 60)
    rahmen(von, nach, zwischenbild="rahmen-beim-ziehen")
    pruefe("M02 Auswahlrechteck: genau die fünf Symbole der Reihe", neue_auswahl(reihe), sorted(auswahl()))
    pruefe("M02 Rahmen hat nichts verschoben", lage() == l)
    datei = [f for f in sorted(os.listdir(t.BILDER)) if "rahmen-beim-ziehen" in f][-1]
    im = t.Image.open(os.path.join(t.BILDER, datei)).convert("RGB")
    rand = sum(t.nah(im.getpixel((x, yy)), (0xEA, 0x70, 0x3D), 45)
               for x in range(von[0] + 2, von[0] + 150) for yy in (von[1], von[1] + 1))
    pruefe("M02 Rahmen sichtbar (Kante im Bild)", rand > 60, rand)
    bild("rahmen-fertig")
    test_m05_esc("M15a")
    # rückwärts von einer freien Stelle rechts der Reihe (Zelle 8,3)
    rahmen((zelle(8, 3)[0] + 60, y3 + 60), von)
    pruefe("M15 Rahmen in Gegenrichtung (unten rechts → oben links)", neue_auswahl(reihe), sorted(auswahl()))


def test_m03_abwaehlen():
    pruefe("M03 Vorbereitung: fünf per Rahmen", waehle_per_rahmen(["Apfel", "Zebra.txt", "Bilder", "Musik", "Mein Browser"]))
    strg_klick(t.mitte(lage()["Musik"]))
    pruefe("M03 Strg+Klick wählt ein Symbol ab", neue_auswahl({"Apfel", "Zebra.txt", "Bilder", "Mein Browser"}),
           sorted(auswahl()))
    bild("abgewaehlt")


def test_m04_alle():
    q("keydown Control_L", "wait 60", "type a", "wait 60", "keyup Control_L", "wait 500")
    alle = set(lage())
    pruefe("M04 Strg+A wählt alle Symbole", neue_auswahl(alle), len(auswahl()))
    bild("strg-a")
    test_m05_esc()


def test_m06_gemeinsam():
    zweite = ["Fremder Starter", "WWWWWWWWWWWWWWWWWWWWWWWW.txt", "Ein sehr langer Dateiname mit vielen Wörtern.txt"]
    y4 = zelle(0, 4)[1] + t.oben[0]
    rahmen((zelle(2, 4)[0] + 60, y4 + 40), (zelle(5, 4)[0] + 60, y4 + 50))
    pruefe("M06 drei Symbole ausgewählt", neue_auswahl(set(zweite)), sorted(auswahl()))
    vor = lage()
    z = t.zustand()
    # das mittlere ziehen: +4 Spalten, +1 Zeile, unsauber
    t.ziehen(t.mitte(vor[zweite[1]]), ziel(8, 5, (28, -22)), 700, zwischenbild="gruppe-beim-ziehen")
    l = t.neue_lage(lambda m: m.get(zweite[1]) != vor[zweite[1]], 4)
    zellen_vor = [zelle_von(vor[n]) for n in zweite]
    zellen = [zelle_von(l.get(n, (0, 0))) for n in zweite]
    pruefe("M06 gemeinsam verschoben (+4 Spalten, +1 Zeile)",
           zellen == [(c + 4, r + 1) for c, r in zellen_vor], zellen)
    ok, falsch = auf_raster(l)
    pruefe("M06 gemeinsam eingerastet (alle auf Rasterzellen)", ok, falsch)
    pruefe("M06 relative Anordnung erhalten (gleiche Zellabstände, Pixel ±1 durch Rasterrundung)",
           form(l, zweite) == form(vor, zweite) and gleiche_pixelform(l, vor, zweite),
           (pixelform(vor, zweite), pixelform(l, zweite)))
    pruefe("M06 nicht ausgewählte unverändert", unveraendert(vor, l, zweite))
    pruefe("M06 Auswahl bleibt bestehen", neue_auswahl(set(zweite)))
    zeilen = t.zustand_zeilen()
    pruefe("M06 alle drei Positionen gespeichert (vorn in der Datei)",
           t.zustand() != z and all(any(r.split("\t")[1] == f for r in zeilen[1:5])
                                    for f in ["Fremd.desktop", zweite[1], zweite[2]]), zeilen[:6])
    bild("gruppe-abgelegt")


def test_m07_kollision():
    zweite = ["Fremder Starter", "WWWWWWWWWWWWWWWWWWWWWWWW.txt", "Ein sehr langer Dateiname mit vielen Wörtern.txt"]
    pruefe("M07 Vorbereitung: drei per Rahmen", waehle_per_rahmen(zweite))
    vor = lage()
    # Ziel: genau auf die belegte Fünferreihe (3..5, 3)
    t.ziehen(t.mitte(vor[zweite[1]]), ziel(4, 3), 600)
    l = t.neue_lage(lambda m: m.get(zweite[1]) != vor[zweite[1]], 4)
    ok, why = t.ueberlappungsfrei(l)
    pruefe("M07 Ablegen auf belegte Plätze: keine Überlagerung", ok, why)
    pruefe("M07 andere Symbole bleiben, wo sie sind", unveraendert(vor, l, zweite))
    zellen = [zelle_von(l[n]) for n in zweite]
    pruefe("M07 Gruppe bleibt zusammen (gleiche Form)",
           [(c - zellen[0][0], r - zellen[0][1]) for c, r in zellen] == [(0, 0), (1, 0), (2, 0)], zellen)
    pruefe("M07 Gruppe auf dem nächsten freien Platz (Zeile 2 oder 4 statt 3)",
           zellen[1][1] != 3 and abs(zellen[1][1] - 3) == 1, zellen)
    bild("gruppe-kollision")


def test_m08_rand():
    zweite = ["Fremder Starter", "WWWWWWWWWWWWWWWWWWWWWWWW.txt", "Ein sehr langer Dateiname mit vielen Wörtern.txt"]
    pruefe("M08 Vorbereitung: drei per Rahmen", waehle_per_rahmen(zweite))
    vor = lage()
    cs, rs, _, _ = raster()
    t.ziehen(t.mitte(vor[zweite[1]]), (1279, 360), 600)   # weit nach rechts
    l = t.neue_lage(lambda m: m.get(zweite[1]) != vor[zweite[1]], 4)
    zellen = [zelle_von(l[n]) for n in zweite]
    pruefe("M08 am rechten Rand: Gruppe hält an (letzte Spalte)", max(c for c, _ in zellen) == cs - 1, zellen)
    pruefe("M08 Form erhalten", form(l, zweite) == [(0, 0), (1, 0), (2, 0)], form(l, zweite))
    ok, why = t.im_bereich(l)
    pruefe("M08 alles sichtbar", ok, why)
    ok, why = t.ueberlappungsfrei(l)
    pruefe("M08 keine Überlagerung", ok, why)
    pruefe("M08 andere unverändert", unveraendert(vor, l, zweite))
    bild("gruppe-rand")


def test_m10_doppelklick():
    l = lage()
    t.klick(t.mitte(l["Apfel"]))
    strg_klick(t.mitte(l["Zebra.txt"]))
    neue_auswahl({"Apfel", "Zebra.txt"})
    vorher = len(t.marker())
    t.klick(t.mitte(l["Apfel"]), "dclick")
    warte(lambda: len(t.marker()) > vorher, 5, "Apfel geöffnet")
    pruefe("M10 Doppelklick öffnet nur das angeklickte Symbol",
           t.marker()[vorher:] == [f"files {os.path.join(SCHREIBTISCH, 'Apfel')}"], t.marker()[vorher:])
    pruefe("M10 danach ist nur dieses ausgewählt", neue_auswahl({"Apfel"}), auswahl())
    pruefe("M10 nichts verschoben", lage() == l)


def test_m12_kontextmenue():
    l = lage()
    t.klick(t.mitte(l["Apfel"]))
    strg_klick(t.mitte(l["Zebra.txt"]))
    neue_auswahl({"Apfel", "Zebra.txt"})
    n0 = d.text().count("Umbenennen: ")
    m = t.mitte(l["Apfel"])
    t.klick(m, "rclick")
    q("wait 500")
    bild("menue-mehrfachauswahl")
    pruefe("M12 Rechtsklick auf ausgewähltes Symbol behält die Auswahl", neue_auswahl({"Apfel", "Zebra.txt"}))
    t.klick((m[0] + 60, m[1] + 46))     # „Umbenennen …“ (ausgegraut)
    q("wait 500")
    pruefe("M12 „Umbenennen …“ bei mehreren ausgegraut (Klick bewirkt nichts)",
           d.text().count("Umbenennen: ") == n0 and not t.dialog_offen())
    q("key Escape", "wait 300")
    q("key F2", "wait 500")
    pruefe("M12 F2 bei mehreren: kein Umbenennen", d.text().count("Umbenennen: ") == n0)
    vorher = len(t.marker())
    t.klick(m, "rclick")
    q("wait 500")
    t.klick((m[0] + 60, m[1] + 15))     # „Öffnen“
    warte(lambda: len(t.marker()) > vorher, 5)
    pruefe("M12 „Öffnen“ öffnet genau ein Symbol (keine Massenaktion)", len(t.marker()) - vorher == 1,
           t.marker()[vorher:])
    test_m05_esc("M12b")


def test_m11_einzeln():
    vor = lage()
    q("key Escape", "wait 200")
    t.ziehen(t.mitte(vor["x.txt"]), ziel(2, 0, (20, 15)), 500)
    l = t.neue_lage(lambda m: m.get("x.txt") != vor["x.txt"], 4)
    pruefe("M11 Einzelverschieben wie bisher (eingerastet)", l.get("x.txt") == zelle(2, 0), l.get("x.txt"))
    pruefe("M11 nur dieses Symbol bewegt", unveraendert(vor, l, ("x.txt",)))
    pruefe("M11 gezogenes Symbol ist ausgewählt", neue_auswahl({"x.txt"}))


def test_m13_leer():
    t.klick(frei_punkt())
    pruefe("M13 Klick auf freie Fläche hebt die Auswahl auf", neue_auswahl(set()), auswahl())


# ----------------------------------------------------------------------------

def test_r08_aufloesungen():
    l = lage()
    z = t.zustand()
    for modus, skal in (("1920x1080", None), ("1024x768", None), ("1280x720", "2")):
        r = subprocess.run([t.QA, modus] + ([skal] if skal else []), capture_output=True, text=True)
        t.protokoll(r.stderr.strip())
        lw, lh = map(int, modus.split("x"))
        if skal:
            lw, lh = lw // int(skal), lh // int(skal)
        warte(lambda: flaeche() and flaeche()[0] == lw, 6, modus)
        time.sleep(1.5)
        neu = lage()
        w, h = flaeche()
        name = f"{modus}{' ×' + skal if skal else ''}"
        bild(f"aufloesung-{modus}{'-x' + skal if skal else ''}")
        ok, falsch = auf_raster(neu, w, h)
        pruefe(f"R08 {name}: alle auf dem Raster dieser Fläche", ok and set(neu) == set(l), falsch)
        ok, why = t.ueberlappungsfrei(neu)
        pruefe(f"R08 {name}: keine Überschneidung", ok, why)
        ok, why = t.im_bereich(neu)
        pruefe(f"R08 {name}: alles sichtbar", ok, why)
        if not skal:
            pruefe(f"R08 {name}: Papierkorb bleibt in der Ecke rechts unten",
                   neu.get("Papierkorb") == (w - 108, h - 96), neu.get("Papierkorb"))
        pruefe(f"R08 {name}: Zustandsdatei unverändert", t.zustand() == z)
    subprocess.run([t.QA, "1280x720", "1"], capture_output=True)
    warte(lambda: flaeche() == (1280, 656), 6, "zurück")
    time.sleep(1.5)
    pruefe("R08 zurück auf 1280×720: alle Positionen wie vorher", t.voll(l) and lage() == l,
           {k: (l.get(k), lage().get(k)) for k in l if l.get(k) != lage().get(k)})


def test_r09_neu_und_geloescht():
    vor = lage()
    with open(os.path.join(SCHREIBTISCH, "Neu.txt"), "w") as f:
        f.write("neu\n")
    l = t.neue_lage(lambda m: "Neu.txt" in m, 5)
    cs, rs, _, _ = raster()
    erste = next(zelle(c, r) for c in range(cs) for r in range(rs)
                 if all(abs(zelle(c, r)[0] - x) >= CELL[0] or abs(zelle(c, r)[1] - y) >= CELL[1]
                        for x, y in vor.values()))
    pruefe("R09 neues Symbol: erste freie Rasterzelle", l.get("Neu.txt") == erste, (l.get("Neu.txt"), erste))
    pruefe("R09 vorhandene unverändert", unveraendert(vor, l, ("Neu.txt",)))
    alt = l["Musik"]
    os.rmdir(os.path.join(SCHREIBTISCH, "Musik"))
    l2 = t.neue_lage(lambda m: "Musik" not in m, 5)
    pruefe("R09 gelöschtes Symbol verschwindet, übrige bleiben", "Musik" not in l2 and unveraendert(l, l2, ("Musik",)))
    pruefe("R09 sein Eintrag ist aus der Zustandsdatei entfernt",
           not any(z.split("\t")[1:2] == ["Musik"] for z in t.zustand_zeilen()))
    os.mkdir(os.path.join(SCHREIBTISCH, "Musik"))
    l3 = t.neue_lage(lambda m: "Musik" in m, 5)
    pruefe("R09 neu angelegtes „Musik“ bekommt einen freien Platz, nicht den alten",
           l3.get("Musik") != alt and auf_raster(l3)[0], (alt, l3.get("Musik")))
    bild("neu-und-geloescht")


def test_r10_einstellung():
    vor = lage()
    f = frei_punkt()
    t.klick(f, "rclick")
    q("wait 500")
    bild("menue-raster")
    t.klick((f[0] + 60, f[1] + RASTER_AN_AUS))
    q("wait 600")
    pruefe("R10 „Am Raster ausrichten“ aus: gespeichert", "einstellung\traster\tnein" in t.zustand_zeilen())
    pruefe("R10 aus: Positionen bleiben (lagen schon auf dem Raster)", lage() == vor)
    p0 = lage()["x.txt"]
    zielpunkt = ziel(6, 5, (27, -19))
    t.ziehen(t.mitte(p0), zielpunkt, 500)
    l = t.neue_lage(lambda m: m.get("x.txt") != p0, 4)
    frei = (zielpunkt[0] - 50, zielpunkt[1] - 40 - t.oben[0])
    pruefe("R10 aus: pixelgenaues Ablegen wie HMP-10", l.get("x.txt") == frei, (l.get("x.txt"), frei))
    t.klick(frei_punkt(), "rclick")
    q("wait 500")
    f = frei_punkt()  # gleiche Stelle wie der Rechtsklick
    t.klick((f[0] + 60, f[1] + RASTER_AN_AUS))
    q("wait 600")
    l = t.neue_lage(lambda m: m.get("x.txt") == zelle(*zelle_von(frei)), 4)
    pruefe("R10 wieder an: Anzeige auf der nächsten Zelle", l.get("x.txt") == zelle(*zelle_von(frei)), l.get("x.txt"))
    zeile = [z for z in t.zustand_zeilen() if z.startswith("datei\tx.txt\t")]
    pruefe("R10 gespeicherte Pixelposition bleibt (nichts ungefragt verloren)", bool(zeile), zeile)
    menue(AM_RASTER)
    zeile2 = [z for z in t.zustand_zeilen() if z.startswith("datei\tx.txt\t")]
    pruefe("R10 „Symbole am Raster ausrichten“ speichert die Zellen", zeile2 and zeile2 != zeile, (zeile, zeile2))
    # Kontrolle: ohne Raster stehen jetzt alle genau auf ihren Zellen
    t.klick(frei_punkt(), "rclick")
    q("wait 500")
    f = frei_punkt()
    t.klick((f[0] + 60, f[1] + RASTER_AN_AUS))
    q("wait 600")
    ok, falsch = auf_raster(lage())
    pruefe("R10 nach dem Ausrichten: auch ohne Einrasten alles auf Zellen", ok, falsch)
    t.klick(frei_punkt(), "rclick")
    q("wait 500")
    f = frei_punkt()
    t.klick((f[0] + 60, f[1] + RASTER_AN_AUS))
    q("wait 600")
    pruefe("R10 Einstellung wieder an", "einstellung\traster\tja" in t.zustand_zeilen())


def test_r12_panels():
    im = bild("panels-am-ende")
    o, u = t.panelstreifen(im)
    pruefe("R12 oberes und unteres Panel unverdeckt", (o, u) == t.panels_vorher)
    ok, why = t.im_bereich(lage())
    pruefe("R12 kein Symbol unter einem Panel", ok, why)
    alle = "".join(open(os.path.join(LOGS, f), errors="replace").read() for f in os.listdir(LOGS)
                   if f.startswith("desktop-"))
    pruefe("R12 keine CRITICAL/Panik in allen Desktop-Starts", "CRITICAL" not in alle and "panicked" not in alle)


# ----------------------------------------------------------------------------

def test_migration():
    """Vorhandene HMP-10-Datei (wie live: Papierkorb rechts 26 oben 27, Terminal oben 230)."""
    # die vorhandene HMP-10-Datei, wie sie vor dem Start war
    vorher = open(os.environ["HMP10_ZUSTAND"]).read()
    l = lage()
    pruefe("MIG Papierkorb (rechts 26, oben 27) auf der nächsten Zelle angezeigt",
           l.get("Papierkorb") == zelle(11, 0), l.get("Papierkorb"))
    pruefe("MIG Querencia Terminal (oben 230) auf der nächsten Zelle angezeigt",
           l.get("Querencia Terminal") == zelle(0, 2), l.get("Querencia Terminal"))
    ok, falsch = auf_raster(l)
    pruefe("MIG alles auf dem Raster, ohne Überschneidung", ok and t.ueberlappungsfrei(l)[0], falsch)
    bild("migration-start")
    alte = [z for z in vorher.splitlines() if z and not z.startswith("#")]
    jetzt = open(ZUSTAND).read().splitlines()
    pruefe("MIG beim Start: alle gespeicherten Positionen unverändert (nur neue Symbole ergänzt)",
           all(z in jetzt for z in alte), [z for z in alte if z not in jetzt])
    menue(RASTER_AN_AUS)
    l2 = t.neue_lage(lambda m: m.get("Papierkorb") != l.get("Papierkorb"), 4)
    pruefe("MIG Raster aus: die alten Pixelpositionen sind unverändert da",
           l2.get("Papierkorb") == (1280 - 100 - 26, 27) and l2.get("Querencia Terminal") == (8, 230),
           (l2.get("Papierkorb"), l2.get("Querencia Terminal")))
    sicherung = ZUSTAND + ".vor-hmp10a"
    pruefe("MIG vor dem ersten Schreiben gesichert, byte-gleich mit dem Original",
           os.path.exists(sicherung) and open(sicherung).read() == vorher)
    bild("migration-raster-aus")
    menue(RASTER_AN_AUS)
    menue(AM_RASTER)
    l3 = lage()
    ok, falsch = auf_raster(l3)
    pruefe("MIG „Symbole am Raster ausrichten“: auf Zellen gespeichert", ok, falsch)
    pruefe("MIG Sicherung bleibt unverändert", open(sicherung).read() == vorher)
    test_r07_neustart("MIG", "migration-neustart")


def test_migration09():
    """Erster Start nach HMP-09 (nur desktop-order): Reihenfolge auf das Raster."""
    l = lage()
    erwartet = [zelle(0, 0), zelle(0, 1), zelle(0, 2)]
    pruefe("M9 HMP-09-Reihenfolge übernommen (Papierkorb, Zebra.txt, Rechner auf den ersten Zellen)",
           [l.get("Papierkorb"), l.get("Zebra.txt"), l.get("Rechner")] == erwartet,
           [l.get("Papierkorb"), l.get("Zebra.txt"), l.get("Rechner")])
    pruefe("M9 Meldung im Protokoll", "Anordnung aus der HMP-09-Reihenfolge übernommen" in d.text())
    ok, falsch = auf_raster(l)
    pruefe("M9 alles auf dem Raster", ok, falsch)
    z = t.zustand_zeilen()
    pruefe("M9 Zustandsdatei angelegt", "sonder\ttrash\tlinks 8\toben 8" in z
           and f"datei\tZebra.txt\tlinks 8\toben {zelle(0, 1)[1]}" in z, z)
    pruefe("M9 desktop-order bleibt lesbar für HMP-09",
           open(os.path.join(P, "state", "querencia", "desktop-order")).read().splitlines()[:3]
           == ["special:trash", "file:Zebra.txt", "special:computer"])
    pruefe("M9 keine Sicherung nötig (es gab keine HMP-10-Datei)", not os.path.exists(ZUSTAND + ".vor-hmp10a"))
    bild("migration-hmp09")
    test_r07_neustart("M9", "migration-hmp09-neustart")


def test_sicherung():
    """Review-Nachtrag 1: ohne nachgewiesene Sicherung wird nichts geschrieben."""
    original = open(os.environ["HMP10_ZUSTAND"]).read()
    sicherung = ZUSTAND + ".vor-hmp10a"
    order = os.path.join(P, "state", "querencia", "desktop-order")
    pruefe("S1 Sicherung scheitert absichtlich (Ordner im Weg)", os.path.isdir(sicherung))
    pruefe("S1 Start: Zustandsdatei unverändert (neue Symbole nicht geschrieben)",
           open(ZUSTAND).read() == original and "Sicherung fehlgeschlagen" in d.text(),
           [z for z in d.text().splitlines() if "Sicherung" in z][-1:])
    l = lage()
    zebra = l["Zebra.txt"]
    t.ziehen(t.mitte(zebra), ziel(6, 3, (15, 10)), 500)
    l2 = t.neue_lage(lambda m: m.get("Zebra.txt") != zebra, 4)
    pruefe("S2 Verschieben wirkt auf dem Bildschirm", l2.get("Zebra.txt") == zelle(6, 3), l2.get("Zebra.txt"))
    pruefe("S2 … aber die Positionsdatei wird nicht überschrieben", open(ZUSTAND).read() == original)
    pruefe("S2 … und auch keine Reihenfolge-Datei", not os.path.exists(order))
    reste = [f for f in os.listdir(os.path.dirname(ZUSTAND)) if f.endswith(".neu")]
    pruefe("S2 keine halb geschriebenen Dateien", not reste, reste)
    pruefe("S2 Grund im Protokoll", d.text().count("Sicherung fehlgeschlagen") >= 2)
    bild("sicherung-fehlgeschlagen")
    os.rmdir(sicherung)
    apfel = l2["Apfel"]
    t.ziehen(t.mitte(apfel), ziel(7, 3, (-12, 8)), 500)
    l3 = t.neue_lage(lambda m: m.get("Apfel") != apfel, 4)
    pruefe("S3 Sicherung jetzt angelegt, byte-gleich mit der Datei vorher, Marke passt",
           os.path.isfile(sicherung) and open(sicherung).read() == original and marke_gueltig(sicherung))
    jetzt = open(ZUSTAND).read()
    pruefe("S3 erst danach geschrieben: beide Verschiebungen gespeichert",
           jetzt != original and any(z.startswith("datei\tZebra.txt\t") for z in jetzt.splitlines())
           and any(z.startswith("ordner\tApfel\t") for z in jetzt.splitlines()))
    alte = [z for z in original.splitlines() if z and not z.startswith("#")]
    pruefe("S3 alle alten Einträge erhalten", all(z in jetzt.splitlines() for z in alte if "\t" in z),
           [z for z in alte if "\t" in z and z not in jetzt.splitlines()])
    test_r07_neustart("S4", "sicherung-neustart")
    pruefe("S4 nach dem Neustart: Zebra.txt und Apfel an ihren neuen Plätzen",
           lage().get("Zebra.txt") == zelle(6, 3) and lage().get("Apfel") == zelle(7, 3) and l3.get("Apfel") == zelle(7, 3))


def marke_gueltig(kopie):
    """Wie store::complete: <kopie>.sha256 = „sha256 <hex>\\nbytes <n>\\n“ des Inhalts."""
    import hashlib
    try:
        daten = open(kopie, "rb").read()
        marke = open(kopie + ".sha256").read()
    except OSError:
        return False
    return marke == f"sha256 {hashlib.sha256(daten).hexdigest()}\nbytes {len(daten)}\n"


def test_sicherung_unvollstaendig():
    """Letzter Review-Punkt: eine vorhandene, halb geschriebene Sicherung ohne Marke."""
    original = open(os.environ["HMP10_ZUSTAND"]).read()
    sicherung = ZUSTAND + ".vor-hmp10a"
    halb = original[: len(original) // 2]
    ordner = os.path.dirname(ZUSTAND)
    beiseite = [f for f in os.listdir(ordner) if ".vor-hmp10a.unvollstaendig-" in f]
    pruefe("U1 Start: unvollständige Sicherung nicht als Sicherung genommen, sondern beiseitegelegt",
           len(beiseite) == 1 and open(os.path.join(ordner, beiseite[0])).read() == halb, beiseite)
    pruefe("U1 neue Sicherung vollständig: byte-gleich mit der Datei vorher, Marke passt",
           open(sicherung).read() == original and marke_gueltig(sicherung))
    pruefe("U1 erst danach geschrieben (neue Symbole gespeichert)",
           open(ZUSTAND).read() != original and "unvollständige Sicherung beiseitegelegt" in d.text())
    reste = [f for f in os.listdir(ordner) if ".neu-" in f or f.endswith(".neu")]
    pruefe("U1 keine temporären Reste", not reste, reste)
    z = t.zustand()
    l = lage()
    t.ziehen(t.mitte(l["Zebra.txt"]), ziel(6, 3, (10, 10)), 500)
    t.neue_lage(lambda m: m.get("Zebra.txt") == zelle(6, 3), 4)
    pruefe("U2 weiteres Verschieben: Sicherung bleibt unverändert, Datei wird geschrieben",
           open(sicherung).read() == original and marke_gueltig(sicherung) and t.zustand() != z)
    test_r07_neustart("U3", "unvollstaendig-neustart")
    pruefe("U3 nach dem Neustart: Sicherung weiter gültig, nichts erneut beiseitegelegt",
           marke_gueltig(sicherung)
           and len([f for f in os.listdir(ordner) if ".vor-hmp10a.unvollstaendig-" in f]) == 1)
    bild("unvollstaendige-sicherung")


def ueberlappungen(l):
    werte = sorted(l.items())
    return {(a, b) for i, (a, pa) in enumerate(werte) for b, pb in werte[i + 1:]
            if abs(pa[0] - pb[0]) < CELL[0] and abs(pa[1] - pb[1]) < CELL[1]}


def test_platzmangel():
    """Review-Nachtrag 2: mehr Symbole als Rasterplätze; nichts wird übereinander abgelegt."""
    l0 = lage()
    z0 = t.zustand()
    r = subprocess.run([t.QA, "400x300"], capture_output=True, text=True)
    t.protokoll(r.stderr.strip())
    warte(lambda: flaeche() and flaeche()[0] == 400, 6, "400x300")
    time.sleep(1.5)
    q("extent 400 300")
    w, h = flaeche()
    cs, rs, _, _ = raster()
    pruefe("P1 kleine Fläche: 6 Rasterplätze für 14 Symbole", (cs * rs, len(lage())) == (6, 14), (w, h, cs, rs))
    vor = lage()
    vorher_paare = ueberlappungen(vor)
    bild("platzmangel-start")
    pruefe("P1 Zustandsdatei durch die kleine Fläche unverändert", t.zustand() == z0)
    # alle auswählen und ziehen
    sichtbar = zelle(1, 0)
    t.klick(t.mitte(sichtbar))
    q("keydown Control_L", "wait 60", "type a", "wait 60", "keyup Control_L", "wait 400")
    pruefe("P2 alle 14 ausgewählt", neue_auswahl(set(vor)), len(auswahl()))
    t.ziehen(t.mitte(sichtbar), t.mitte(zelle(2, 1)), 500)
    q("wait 600")
    pruefe("P2 Gruppe ohne Platz: Verschieben abgelehnt", "Verschieben abgelehnt (kein Platz für 14 Symbole)" in d.text())
    pruefe("P2 alle Symbole bleiben, wo sie waren", lage() == vor)
    pruefe("P2 Positionsdatei unverändert", t.zustand() == z0)
    bild("platzmangel-gruppe-abgelehnt")
    # zwei Symbole auf eine belegte Zelle
    q("key Escape", "wait 300")
    a, b = zelle(0, 0), zelle(1, 0)
    t.klick(t.mitte(a))
    strg_klick(t.mitte(b))
    t.ziehen(t.mitte(a), t.mitte(zelle(1, 1)), 500)
    q("wait 600")
    nach = lage()
    pruefe("P3 zwei Symbole: keine neue Überlagerung", ueberlappungen(nach) <= vorher_paare,
           ueberlappungen(nach) - vorher_paare)
    pruefe("P3 nicht ausgewählte Symbole unverändert",
           all(nach.get(n) == p for n, p in vor.items() if p not in (a, b)))
    # einzelnes Symbol auf eine belegte Zelle
    q("key Escape", "wait 300")
    vor2 = lage()
    t.ziehen(t.mitte(zelle(2, 0)), t.mitte(zelle(0, 1)), 500)
    q("wait 600")
    nach2 = lage()
    pruefe("P4 einzelnes Symbol: keine neue Überlagerung", ueberlappungen(nach2) <= ueberlappungen(vor2),
           ueberlappungen(nach2) - ueberlappungen(vor2))
    bild("platzmangel-ende")
    q("extent 1280 720")
    subprocess.run([t.QA, "1280x720"], capture_output=True)
    warte(lambda: flaeche() == (1280, 656), 6, "zurück")
    time.sleep(1.5)
    zurueck = lage()
    ok, why = t.ueberlappungsfrei(zurueck)
    pruefe("P5 zurück auf 1280x720: alles wieder frei und ohne Überschneidung", ok, why)
    bewegt = {n for n in vor if nach2.get(n) != vor.get(n)}
    pruefe("P5 keine Position verloren: jedes auf der kleinen Fläche unbewegte Symbol steht wie vorher",
           all(zurueck.get(n) == p for n, p in l0.items() if n not in bewegt),
           {n: (p, zurueck.get(n)) for n, p in l0.items() if n not in bewegt and zurueck.get(n) != p})
    bild("platzmangel-zurueck")


def phase2():
    vorher = json.load(open(ENDE_LAGE))
    l = lage()
    l = {k: tuple(v) for k, v in l.items()}
    pruefe("A2 erneute Anmeldung (neuer labwc): alle Positionen wie vor dem Abmelden",
           t.voll(l) and l == {k: tuple(v) for k, v in vorher["lage"].items()},
           {k: (vorher["lage"].get(k), l.get(k)) for k in set(l) | set(vorher["lage"])
            if tuple(vorher["lage"].get(k, ())) != tuple(l.get(k, ()))})
    pruefe("A2 Zustandsdatei unverändert", t.zustand() == vorher["zustand"])
    pruefe("A2 Einstellung „Am Raster ausrichten“ erhalten", "einstellung\traster\tja" in t.zustand_zeilen())
    ok, falsch = auf_raster(l)
    pruefe("A2 auf dem Raster", ok, falsch)
    bild("anmeldung-2")
    p0 = l["Apfel"]
    cs, rs, _, _ = raster()
    frei = next((c, r) for r in range(rs - 1, -1, -1) for c in range(2, cs)
                if all(abs(zelle(c, r)[0] - x) >= CELL[0] or abs(zelle(c, r)[1] - y) >= CELL[1]
                       for x, y in l.values()))
    t.ziehen(t.mitte(p0), ziel(*frei, (22, 18)), 500)
    l2 = t.neue_lage(lambda m: m.get("Apfel") != p0, 4)
    pruefe("A2 Verschieben nach der Anmeldung rastet ein", l2.get("Apfel") == zelle(*frei), (l2.get("Apfel"), frei))


def main():
    os.makedirs(t.BILDER, exist_ok=True)
    phase = os.environ.get("HMP10_PHASE", "1")
    nur = os.environ.get("HMP10_NUR", "")
    datei = "pruefung-phase2.txt" if phase == "2" else "pruefung.txt"
    t.protokoll(f"HMP-10a Phase {phase} {nur} Desktop {t.DESKTOP}")
    if phase == "1" and (not nur or nur == "platzmangel"):
        for n in EXTRA:
            pfad = os.path.join(SCHREIBTISCH, n.rstrip("/"))
            if n.endswith("/"):
                os.makedirs(pfad, exist_ok=True)
            else:
                with open(pfad, "w") as f:
                    f.write(n + "\n")
    if nur == "sicherung":
        os.makedirs(ZUSTAND + ".vor-hmp10a")
    if nur == "sicherung-unvollstaendig":
        inhalt = open(os.environ["HMP10_ZUSTAND"]).read()
        with open(ZUSTAND + ".vor-hmp10a", "w") as f:   # halb, ohne Marke
            f.write(inhalt[: len(inhalt) // 2])
    if nur == "migration09":
        os.makedirs(os.path.join(P, "state", "querencia"), exist_ok=True)
        with open(os.path.join(P, "state", "querencia", "desktop-order"), "w") as f:
            f.write("special:trash\nfile:Zebra.txt\nspecial:computer\n")
    panel = subprocess.Popen([t.PANEL], stdout=open(os.path.join(LOGS, f"panel-{phase}.log"), "w"),
                             stderr=subprocess.STDOUT)
    time.sleep(3)
    t.q = t.Eingabe()
    q("move 640 700")
    d.starts = 10 if phase == "2" else 0
    d.start(lambda l: len(l) >= 8)
    im = bild(f"leer-phase-{phase}")
    t.oben[0] = t.miss_panel(im)
    t.panels_vorher = t.panelstreifen(im)
    if phase == "2":
        tests = [phase2]
    elif nur == "migration":
        tests = [test_migration]
    elif nur == "migration09":
        tests = [test_migration09]
    elif nur == "sicherung":
        tests = [test_sicherung]
    elif nur == "platzmangel":
        tests = [test_platzmangel]
    elif nur == "sicherung-unvollstaendig":
        tests = [test_sicherung_unvollstaendig]
    else:
        tests = [test_r01_start, test_r02_nebeneinander, test_r03_r04_reihen, test_r05_belegt, test_r06_rand,
                 test_r07_neustart, test_m01_strg, test_m14_umschalt, test_m05_esc, test_m02_rahmen,
                 test_m03_abwaehlen, test_m04_alle, test_m06_gemeinsam, test_m07_kollision, test_m08_rand,
                 lambda: test_r07_neustart("M09", "gruppe-nach-neustart"), test_m10_doppelklick,
                 test_m12_kontextmenue, test_m11_einzeln, test_m13_leer, test_r08_aufloesungen,
                 test_r09_neu_und_geloescht, test_r10_einstellung, test_r12_panels]
    for test in tests:
        name = getattr(test, "__name__", "test")
        t.protokoll(f"=== {name}")
        try:
            test()
        except Exception as err:  # noqa: BLE001
            pruefe(f"{name} ohne Ausnahme", False, f"{err!r}\n{traceback.format_exc()}")
        q("key Escape", "wait 200")
    if phase == "1" and not nur.startswith("migration"):
        json.dump({"lage": lage(), "zustand": t.zustand()}, open(ENDE_LAGE, "w"), ensure_ascii=False)
    d.stop()
    panel.terminate()
    ok = sum(1 for _, o, _ in t.ergebnisse if o)
    with open(os.path.join(LOGS, datei), "w") as f:
        for n, o, det in t.ergebnisse:
            f.write(f"{'OK    ' if o else 'FEHLER'} {n}{'  – ' + det if det and not o else ''}\n")
        f.write(f"{ok}/{len(t.ergebnisse)} Prüfungen bestanden\n")
    t.protokoll(f"{ok}/{len(t.ergebnisse)} Prüfungen bestanden")


if __name__ == "__main__":
    main()
