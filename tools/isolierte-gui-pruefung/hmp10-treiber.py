#!/usr/bin/env python3
"""HMP-10: Prüftreiber in der isolierten labwc-Sitzung (pruefen-hmp10.sh).

Startet Panel und Desktop, schickt echte (virtuelle) Zeiger- und
Tastaturereignisse über qeingabe und prüft nach jedem Schritt, was wirklich
passiert ist:

- Lage der Symbole: Protokollzeile „Lage:“ des Desktops (Flächenkoordinaten)
  und Pixel im Bildschirmfoto (Symbol da / nicht da, Panels unberührt);
- Zustandsdatei desktop-symbole (Inhalt, atomar, Neustart);
- Dateisystem (Namen, Inodes, Prüfsummen: verschoben wird nie eine Datei);
- geöffnete Programme: Markerskripte schreiben nach marker.log.

Ausgabe: logs/pruefung.txt (OK/FEHLER je Prüfung), logs/bilder/*.png.
"""

import hashlib
import json
import os
import re
import signal
import subprocess
import sys
import time
import traceback

from PIL import Image

P = os.environ["HMP04_PROFILE"]
LOGS = os.path.join(P, "logs")
BILDER = os.path.join(LOGS, "bilder")
DESKTOP = os.environ["HMP10_DESKTOP"]
PANEL = os.environ["HMP_PANEL"]
QE = os.environ["HMP_QEINGABE"]
QA = os.environ["HMP_QAUSGABE"]
SCHREIBTISCH = os.path.join(P, "schreibtisch")
ZUSTAND = os.path.join(P, "state", "querencia", "desktop-symbole")
MARKER = os.path.join(P, "marker.log")

WALL = (58, 110, 165)
PANEL_BG = (0xF6, 0xF5, 0xF4)
ORANGE = (0xF0, 0x77, 0x46)
CELL = (100, 88)
MARGIN = 8

# never anywhere but in the profile
assert os.path.realpath(SCHREIBTISCH).startswith(os.path.realpath(P) + "/"), SCHREIBTISCH
assert "/hmp05-runtime-" in os.environ.get("XDG_RUNTIME_DIR", ""), "nur isoliert"

ergebnisse = []
bildnummer = [0]


def protokoll(text):
    print(time.strftime("%H:%M:%S"), text, flush=True)


def pruefe(name, ok, detail=""):
    ergebnisse.append((name, bool(ok), str(detail)))
    protokoll(f"{'OK    ' if ok else 'FEHLER'} {name} {detail}")
    return bool(ok)


class Eingabe:
    """qeingabe über eine Pipe; jeder Aufruf wartet, bis alles ausgeführt ist."""

    def __init__(self):
        self.proc = subprocess.Popen([QE], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                     text=True, bufsize=1)
        self.n = 0

    def __call__(self, *befehle):
        for b in befehle:
            self.proc.stdin.write(b + "\n")
        self.n += 1
        marke = f"SYNC{self.n}"
        self.proc.stdin.write(f"mark {marke}\n")
        self.proc.stdin.flush()
        while True:
            zeile = self.proc.stdout.readline()
            if not zeile:
                raise RuntimeError("qeingabe beendet")
            if marke in zeile:
                return


q = None


class Desktop:
    def __init__(self):
        self.proc = None
        self.log = None
        self.starts = 0

    def start(self, warte_auf=None):
        self.starts += 1
        self.log = os.path.join(LOGS, f"desktop-{self.starts}.log")
        self.proc = subprocess.Popen([DESKTOP], stdout=open(self.log, "w"), stderr=subprocess.STDOUT)
        warte(lambda: lage(), 10, "Lage nach Start")
        time.sleep(1.5)
        if warte_auf:
            warte(lambda: warte_auf(lage()), 5, "Lage nach Start")

    def stop(self):
        if self.proc and self.proc.poll() is None:
            self.proc.send_signal(signal.SIGTERM)
            try:
                self.proc.wait(5)
            except subprocess.TimeoutExpired:
                self.proc.kill()
                self.proc.wait(5)
        time.sleep(0.5)

    def text(self):
        try:
            return open(self.log, errors="replace").read()
        except OSError:
            return ""


d = Desktop()


def warte(bedingung, sekunden, was=""):
    ende = time.time() + sekunden
    while time.time() < ende:
        try:
            wert = bedingung()
        except Exception:  # noqa: BLE001 – still waiting
            wert = None
        if wert:
            return wert
        time.sleep(0.1)
    protokoll(f"Zeitlimit: {was}")
    return None


def lage():
    """Letzte Protokollzeile „Lage:“ als {Name: (x, y)} (Flächenkoordinaten)."""
    zeilen = [z for z in d.text().splitlines() if "querencia-desktop: Lage: " in z]
    if not zeilen:
        return {}
    teile = zeilen[-1].split("Lage: ", 1)[1].split("; ")
    result = {}
    for t in teile:
        name, xy = t.rsplit("=", 1)
        x, y = xy.split(",")
        result[name] = (int(x), int(y))
    return result


def flaeche():
    m = re.findall(r"Symbolfläche (\d+)x(\d+)", d.text())
    return tuple(map(int, m[-1])) if m else None


def symbole():
    m = re.findall(r"\d+ Symbole: (.*)", d.text())
    return m[-1].split(", ") if m else []


def zustand():
    try:
        return open(ZUSTAND).read()
    except OSError:
        return ""


def zustand_zeilen():
    return [z for z in zustand().splitlines() if z and not z.startswith("#")]


def marker():
    try:
        return open(MARKER).read().splitlines()
    except OSError:
        return []


def bestand():
    """{Name: (Inode, Art, sha256 oder Inhalt)} des Schreibtischs (rekursiv)."""
    result = {}
    for wurzel, ordner, dateien in os.walk(SCHREIBTISCH):
        for n in ordner + dateien:
            pfad = os.path.join(wurzel, n)
            rel = os.path.relpath(pfad, SCHREIBTISCH)
            st = os.lstat(pfad)
            sha = ""
            if os.path.isfile(pfad):
                sha = hashlib.sha256(open(pfad, "rb").read()).hexdigest()[:16]
            result[rel] = (st.st_ino, "d" if os.path.isdir(pfad) else "f", sha)
    return result


def bild(name):
    bildnummer[0] += 1
    pfad = os.path.join(BILDER, f"{bildnummer[0]:02d}-{name}.png")
    subprocess.run(["grim", pfad], check=False)
    return Image.open(pfad).convert("RGB")


def nah(p, q_, tol=30):
    return all(abs(a - b) <= tol for a, b in zip(p, q_))


oben = [32]  # Höhe des oberen Panels (gemessen)


def miss_panel(im):
    w, h = im.size
    y = 0
    while y < h // 3 and nah(im.getpixel((w // 2, y)), PANEL_BG, 14):
        y += 1
    return y


def bildschirm(x, y):
    """Flächenkoordinaten → Bildschirm."""
    return x, y + oben[0]


def mitte(pos):
    x, y = bildschirm(*pos)
    return x + 50, y + 40


def symbol_da(im, pos, schwelle=250):
    """Viele Pixel im Feld weichen deutlich vom einfarbigen Hintergrund ab."""
    x0, y0 = bildschirm(*pos)
    n = 0
    for x in range(x0 + 2, x0 + CELL[0] - 2, 2):
        for y in range(y0 + 2, y0 + CELL[1] - 2, 2):
            p = im.getpixel((x, y))
            if sum(abs(a - b) for a, b in zip(p, WALL)) > 90:
                n += 1
    return n > schwelle // 4, n


def ausgewaehlt(im, pos):
    x0, y0 = bildschirm(*pos)
    n = sum(nah(im.getpixel((x, y)), ORANGE, 40)
            for x in range(x0 + 4, x0 + 96) for y in range(y0 + 52, y0 + 86))
    return n > 40


def weiss(im, box):
    x0, y0, x1, y1 = box
    x0, y0 = max(0, x0), max(0, y0)
    return sum(im.getpixel((x, y)) == (255, 255, 255)
               for x in range(x0, x1) for y in range(y0, y1))


def panelstreifen(im):
    """Pixel des oberen und unteren Panels (ohne Uhr und Fensterliste)."""
    w, h = im.size
    # ohne Protokollzeile (Gegenprobe mit HMP-09): Panels gleich hoch
    unten = oben[0] + (flaeche() or (w, h - 2 * oben[0]))[1]
    return ([im.getpixel((x, y)) for x in range(0, w - 260, 3) for y in range(0, oben[0], 3)],
            [im.getpixel((x, y)) for x in range(w - 120, w - 60, 3) for y in range(unten, h, 3)])


def voll(l):
    """Eine echte Lage: alle Symbole (nie leer erfüllt)."""
    return len(l) >= 8


def ueberlappungsfrei(l):
    if not voll(l):
        return False, f"keine vollständige Lage: {l}"
    werte = list(l.items())
    for i, (a, pa) in enumerate(werte):
        for b, pb in werte[i + 1:]:
            if abs(pa[0] - pb[0]) < CELL[0] and abs(pa[1] - pb[1]) < CELL[1]:
                return False, f"{a} {pa} überlappt {b} {pb}"
    return True, ""


def im_bereich(l):
    if not voll(l) or not flaeche():
        return False, "keine vollständige Lage"
    w, h = flaeche()
    for n, (x, y) in l.items():
        if not (MARGIN <= x <= w - MARGIN - CELL[0] and MARGIN <= y <= h - MARGIN - CELL[1]):
            return False, f"{n} {x},{y} außerhalb {w}x{h}"
    return True, ""


def ziehen(von, nach, ms=500, zwischenbild=None):
    """Echte Maus: drücken, gleiten, loslassen (Bildschirmkoordinaten)."""
    q(f"move {von[0]} {von[1]}", "wait 150", "press", "wait 120")
    if zwischenbild:
        mx, my = (von[0] + nach[0]) // 2, (von[1] + nach[1]) // 2
        q(f"glide {mx} {my} {ms // 2}", "wait 200")
        bild(zwischenbild)
        q(f"glide {nach[0]} {nach[1]} {ms // 2}", "wait 150", "release", "wait 400")
    else:
        q(f"glide {nach[0]} {nach[1]} {ms}", "wait 150", "release", "wait 400")


def klick(pos, art="click"):
    q(f"move {pos[0]} {pos[1]}", "wait 200", art, "wait 400")


DIALOG_BG = (0xF6, 0xF5, 0xF4)


def dialog_offen():
    """Eine Meldung (hell, Bildschirmmitte) ist zu sehen."""
    pfad = os.path.join(LOGS, "dialog-probe.png")
    subprocess.run(["grim", pfad], check=False)
    im = Image.open(pfad).convert("RGB")
    w, h = im.size
    pts = [im.getpixel((x, y)) for x in range(w // 2 - 200, w // 2 + 200, 8)
           for y in range(h // 2 - 50, h // 2 + 30, 6)]
    return sum(nah(p, DIALOG_BG, 14) for p in pts) / len(pts) > 0.6


def meldung(nr, was, bildname):
    """Wartet auf die Meldung, hält sie im Bild fest, schließt sie mit Return."""
    da = warte(dialog_offen, 6, f"Meldung {was}")
    bild(bildname)
    pruefe(f"{nr} Meldung sichtbar: {was}", da)
    q("key Return", "wait 300")
    zu = warte(lambda: not dialog_offen(), 4, "Meldung zu")
    pruefe(f"{nr} Meldung mit Return geschlossen: {was}", zu)


def dialog_schliessen():
    """Return wählt „Schließen“; Escape danach schadet nicht."""
    q("key Return", "wait 600", "key Escape", "wait 400")


def neue_lage(bedingung, sekunden=5, was=""):
    return warte(lambda: bedingung(lage()) and lage(), sekunden, was) or lage()


def umbenennen_f2(name, neu, taste="Return"):
    pos = lage()[name]
    klick(mitte(pos))
    q("key F2", "wait 500")
    if neu is not None:
        q(f"type {neu}", "wait 300")
    q(f"key {taste}", "wait 900")


# ----------------------------------------------------------------------------

def test_01_auswahl():
    l = lage()
    im = bild("start")
    pruefe("01 acht Symbole im ersten Start",
           set(l) == {"Rechner", "Persönlicher Ordner", "Papierkorb", "Querencia Terminal",
                      "Apfel", "Fremder Starter", "Mein Browser", "Zebra.txt"}, sorted(l))
    pruefe("01 Starter zeigt seinen Namen (Name[de])", "Mein Browser" in l)
    alle = voll(l) and all(symbol_da(im, p)[0] for p in l.values())
    pruefe("01 alle Symbole sichtbar (Pixel)", alle)
    klick(mitte(l["Rechner"]))
    im = bild("auswahl-rechner")
    pruefe("01 Einzelklick wählt Rechner", ausgewaehlt(im, l["Rechner"]))
    pruefe("01 Einzelklick öffnet nichts", not marker(), marker())
    # kleiner Ruck unter der Schwelle: bleibt ein Klick
    x, y = mitte(l["Papierkorb"])
    q(f"move {x} {y}", "wait 150", "press", "wait 60", f"move {x + 3} {y + 2}", "wait 60",
      "release", "wait 500")
    pruefe("01 Klick mit 3 px Ruck verschiebt nichts", voll(l) and lage() == l and "verschoben" not in d.text())
    im = bild("auswahl-papierkorb")
    pruefe("01 Auswahl wechselt", ausgewaehlt(im, l["Papierkorb"]) and not ausgewaehlt(im, l["Rechner"]))


def test_02_rechts():
    global vorher_bestand, panels_vorher
    vorher_bestand = bestand()
    l = lage()
    w, h = flaeche()
    ziel = (w - 30, oben[0] + 40)
    ziehen(mitte(l["Rechner"]), ziel, 700, zwischenbild="ziehen-rechner")
    neu = neue_lage(lambda m: m.get("Rechner") != l["Rechner"], 4, "Rechner verschoben")
    erwartet = (w - MARGIN - CELL[0], MARGIN)
    pruefe("02 Rechner nach rechts oben gezogen", neu.get("Rechner") == erwartet,
           f"{neu.get('Rechner')} erwartet {erwartet}")
    im = bild("rechner-rechts-oben")
    pruefe("02 Symbol rechts oben sichtbar, links oben leer",
           symbol_da(im, erwartet)[0] and not symbol_da(im, l["Rechner"])[0],
           (symbol_da(im, erwartet)[1], symbol_da(im, l["Rechner"])[1]))
    andere = {k: v for k, v in neu.items() if k != "Rechner"}
    pruefe("02 andere Symbole unverändert", andere == {k: v for k, v in l.items() if k != "Rechner"})
    pruefe("02 keine Datei verschoben", bestand() == vorher_bestand)


def test_02b_zwischenbild():
    datei = [f for f in sorted(os.listdir(BILDER)) if "ziehen-rechner" in f][-1]
    im = Image.open(os.path.join(BILDER, datei)).convert("RGB")
    w, _ = flaeche()
    # Geisterbild in der Mitte der Strecke, Original noch am alten Platz
    l0 = (MARGIN, MARGIN)
    mx = (58 + w - 30) // 2 - 50
    geist = symbol_da(im, (mx, MARGIN))
    pruefe("02 beim Ziehen folgt ein durchscheinendes Abbild dem Zeiger",
           geist[0] and symbol_da(im, l0)[0], geist[1])


def test_03_unten():
    l = lage()
    w, h = flaeche()
    ziehen(mitte(l["Papierkorb"]), (w - 30, oben[0] + h - 20), 600)
    ziehen(mitte(l["Persönlicher Ordner"]), (40, oben[0] + h - 20), 600)
    ziehen(mitte(l["Querencia Terminal"]), (w // 2, oben[0] + h // 2), 600)
    neu = neue_lage(lambda m: m.get("Querencia Terminal") != l["Querencia Terminal"], 4)
    unten_y = h - MARGIN - CELL[1]
    pruefe("03 Papierkorb rechts unten", neu.get("Papierkorb") == (w - 108, unten_y), neu.get("Papierkorb"))
    pruefe("03 Persönlicher Ordner links unten", neu.get("Persönlicher Ordner") == (MARGIN, unten_y),
           neu.get("Persönlicher Ordner"))
    t = neu.get("Querencia Terminal")
    pruefe("03 Querencia Terminal mittig", t and abs(t[0] + 50 - w // 2) <= 2 and abs(t[1] + 40 - h // 2) <= 2, t)
    bild("ecken-und-mitte")


def test_04_panelgrenze():
    global panels_vorher
    l = lage()
    w, h = flaeche()
    ziehen(mitte(l["Apfel"]), (300, 4), 500)              # auf das obere Panel
    ziehen(mitte(l["Zebra.txt"]), (500, oben[0] + h + 20), 500)  # auf das untere Panel
    ziehen(mitte(l["Fremder Starter"]), (0, oben[0] + 300), 400)  # an den linken Rand
    neu = neue_lage(lambda m: m.get("Fremder Starter") != l["Fremder Starter"], 4)
    a, z = neu.get("Apfel"), neu.get("Zebra.txt")
    pruefe("04 oberes Panel: Symbol bleibt darunter (y = Rand)", a and a[1] == MARGIN, a)
    pruefe("04 unteres Panel: Symbol bleibt darüber", z and z[1] == h - MARGIN - CELL[1], z)
    pruefe("04 linker Bildschirmrand", neu.get("Fremder Starter", (0,))[0] == MARGIN, neu.get("Fremder Starter"))
    ok, why = im_bereich(neu)
    pruefe("04 alle Symbole in der Fläche zwischen den Panels", ok, why)
    im = bild("panelgrenzen")
    o, u = panelstreifen(im)
    pruefe("04 Panels unverdeckt (Pixel wie vor dem Ziehen)", (o, u) == panels_vorher)


def test_05_kollision():
    l = lage()
    w, _ = flaeche()
    rechner = l["Rechner"]
    ziehen(mitte(l["Mein Browser"]), mitte(rechner), 500)
    neu = neue_lage(lambda m: m.get("Mein Browser") != l["Mein Browser"], 4)
    pruefe("05 Rechner bleibt", neu.get("Rechner") == rechner)
    pruefe("05 gezogener Starter weicht auf den nächsten freien Platz aus",
           neu.get("Mein Browser") == (rechner[0], rechner[1] + CELL[1]), neu.get("Mein Browser"))
    ok, why = ueberlappungsfrei(neu)
    pruefe("05 keine Überlappung", ok, why)
    bild("kollision")


def test_06_gespeichert():
    z = zustand_zeilen()
    w, h = flaeche()
    pruefe("06 Zustandsdatei Version 1", "version\t1" in z, z[:2])
    pruefe("06 Rechner: rechts oben", "sonder\tcomputer\trechts 8\toben 8" in z)
    pruefe("06 Papierkorb: rechts unten", "sonder\ttrash\trechts 8\tunten 8" in z)
    pruefe("06 Persönlicher Ordner: links unten", "sonder\thome\tlinks 8\tunten 8" in z)
    pruefe("06 Terminal: Mitte", any(r.startswith("programm\torg.example.Querencia.Terminal.desktop\tmitte")
                                     for r in z))
    pruefe("06 Arten unterschieden (ordner, datei, starter)",
           any(r.startswith("ordner\tApfel\t") for r in z)
           and any(r.startswith("datei\tZebra.txt\t") for r in z)
           and any(r.startswith("starter\tMein Browser.desktop\t") for r in z))
    rest = [f for f in os.listdir(os.path.dirname(ZUSTAND)) if f.endswith(".neu")]
    pruefe("06 atomar geschrieben (keine Reste)", not rest, rest)


def test_07_08_neustart(nr="07", bildname="nach-neustart"):
    l = lage()
    z = zustand()
    d.stop()
    pruefe(f"{nr} Desktop beendet", d.proc.poll() is not None)
    d.start()
    neu = lage()
    pruefe(f"{nr} Positionen nach dem Neustart identisch", voll(l) and neu == l,
           {k: (l.get(k), neu.get(k)) for k in set(l) | set(neu) if l.get(k) != neu.get(k)})
    pruefe(f"{nr} Zustandsdatei durch den Neustart unverändert", bool(z) and zustand() == z)
    im = bild(bildname)
    pruefe(f"{nr} Bild: Symbole an den gespeicherten Plätzen",
           voll(neu) and all(symbol_da(im, p)[0] for p in neu.values()))
    return neu


def test_09_10_neue_datei():
    l = lage()
    with open(os.path.join(SCHREIBTISCH, "Neu von außen.txt"), "w") as f:
        f.write("heruntergeladen\n")
    neu = neue_lage(lambda m: "Neu von außen.txt" in m, 5, "neue Datei")
    pruefe("09 neue Datei erscheint (Dateimonitor)", "Neu von außen.txt" in neu)
    pruefe("10 vorhandene Positionen unverändert", voll(l) and {k: neu.get(k) for k in l} == l)
    ok, why = ueberlappungsfrei(neu)
    pruefe("10 neue Datei auf freiem Platz", ok, why)
    # Ordner über das Arbeitsflächenmenü
    frei = (420, oben[0] + 450)
    klick(frei, "rclick")
    q("wait 400")
    bild("arbeitsflaechenmenue")
    klick((frei[0] + 60, frei[1] + 15))
    neu2 = neue_lage(lambda m: "Neuer Ordner" in m, 5, "Neuer Ordner")
    pruefe("09 Ordner anlegen über das Menü", "Neuer Ordner" in neu2 and os.path.isdir(
        os.path.join(SCHREIBTISCH, "Neuer Ordner")))
    pruefe("10 Positionen nach dem Anlegen unverändert", voll(neu) and {k: neu2.get(k) for k in neu} == neu)
    ok, why = ueberlappungsfrei(neu2)
    pruefe("10 keine Überlappung", ok, why)
    bild("neue-symbole")


def test_09b_extern_umbenannt():
    with open(os.path.join(SCHREIBTISCH, "Extern.txt"), "w") as f:
        f.write("wird draußen umbenannt\n")
    l = neue_lage(lambda m: "Extern.txt" in m, 5, "Extern.txt")
    # weg vom Raster: sonst landete die „neue“ Datei zufällig im selben freien Feld
    w, h = flaeche()
    ziehen(mitte(l["Extern.txt"]), (w - 260, oben[0] + h // 2 + 70), 500)
    l = neue_lage(lambda m: m.get("Extern.txt") != l.get("Extern.txt"), 4, "Extern.txt gezogen")
    pos = l.get("Extern.txt")
    pruefe("09 Extern.txt liegt außerhalb des Rasters", pos and (pos[0] - MARGIN) % CELL[0] != 0, pos)
    os.rename(os.path.join(SCHREIBTISCH, "Extern.txt"), os.path.join(SCHREIBTISCH, "Extern umbenannt.txt"))
    neu = neue_lage(lambda m: "Extern umbenannt.txt" in m, 5, "extern umbenannt")
    pruefe("09 außerhalb umbenannte Datei behält ihren Platz", pos and neu.get("Extern umbenannt.txt") == pos,
           (pos, neu.get("Extern umbenannt.txt")))
    pruefe("10 übrige Positionen unverändert", voll(l) and {k: v for k, v in neu.items() if k != "Extern umbenannt.txt"}
           == {k: v for k, v in l.items() if k != "Extern.txt"})


def test_migration():
    """Erster Start nach HMP-09: dessen Reihenfolge wird übernommen."""
    l = lage()
    erwartet = [(MARGIN, MARGIN + i * CELL[1]) for i in range(3)]
    pruefe("M HMP-09-Reihenfolge übernommen (Papierkorb, Zebra.txt, Rechner)",
           [l.get("Papierkorb"), l.get("Zebra.txt"), l.get("Rechner")] == erwartet,
           [l.get("Papierkorb"), l.get("Zebra.txt"), l.get("Rechner")])
    pruefe("M Meldung im Protokoll", "Anordnung aus der HMP-09-Reihenfolge übernommen" in d.text())
    z = zustand_zeilen()
    pruefe("M Zustandsdatei angelegt", "sonder\ttrash\tlinks 8\toben 8" in z
           and "datei\tZebra.txt\tlinks 8\toben 96" in z, z)
    pruefe("M desktop-order bleibt lesbar für HMP-09",
           open(os.path.join(P, "state", "querencia", "desktop-order")).read().splitlines()[:3]
           == ["special:trash", "file:Zebra.txt", "special:computer"])
    bild("migration")


def test_11_f2_ordner():
    with open(os.path.join(SCHREIBTISCH, "Neuer Ordner", "inhalt.txt"), "w") as f:
        f.write("Projektinhalt bleibt\n")
    vor = bestand()
    l = lage()
    pos = l["Neuer Ordner"]
    klick(mitte(pos))
    q("key F2", "wait 600")
    im = bild("f2-bearbeitung")
    x0, y0 = bildschirm(*pos)
    pruefe("11 F2 zeigt das Bearbeitungsfeld an der Beschriftung",
           weiss(im, (x0 - 20, y0 + 50, x0 + 120, y0 + 76)) > 400 and "Umbenennen: Neuer Ordner" in d.text())
    q("type Projektunterlagen", "wait 300")
    bild("f2-getippt")
    q("key Return", "wait 1000")
    neu = neue_lage(lambda m: "Projektunterlagen" in m, 5, "umbenannt")
    nach = bestand()
    pruefe("11 Ordner im Dateisystem umbenannt",
           "Projektunterlagen" in nach and "Neuer Ordner" not in nach)
    pruefe("11 derselbe Ordner (Inode), Inhalt erhalten",
           nach.get("Projektunterlagen", (0,))[0] == vor["Neuer Ordner"][0]
           and nach.get("Projektunterlagen/inhalt.txt") == vor["Neuer Ordner/inhalt.txt"])
    pruefe("17 Position nach dem Umbenennen erhalten", neu.get("Projektunterlagen") == pos,
           (pos, neu.get("Projektunterlagen")))
    pruefe("11 Zustandsdatei: Eintrag mitgenommen",
           any(r.startswith("ordner\tProjektunterlagen\t") for r in zustand_zeilen())
           and not any("\tNeuer Ordner\t" in r for r in zustand_zeilen()))
    bild("f2-fertig")


def test_12_kontextmenue():
    vor = bestand()
    l = lage()
    pos = l["Zebra.txt"]
    m = mitte(pos)
    klick(m, "rclick")
    q("wait 400")
    bild("symbolmenue")
    klick((m[0] + 60, m[1] + 46))
    q("wait 400")
    im = bild("kontextmenue-bearbeitung")
    q("type Übersicht für Jörg", "wait 300", "key Return", "wait 1000")
    neu = neue_lage(lambda x: "Übersicht für Jörg.txt" in x, 5, "umbenannt")
    nach = bestand()
    pruefe("12 Kontextmenü „Umbenennen …“: nur der Name ohne Endung ersetzt",
           "Übersicht für Jörg.txt" in nach and "Zebra.txt" not in nach, sorted(nach))
    pruefe("12 Umlaute im Dateinamen", os.path.exists(os.path.join(SCHREIBTISCH, "Übersicht für Jörg.txt")))
    pruefe("12 dieselbe Datei, Inhalt unverändert",
           nach.get("Übersicht für Jörg.txt") == vor["Zebra.txt"])
    pruefe("17 Position erhalten", neu.get("Übersicht für Jörg.txt") == pos)


def test_13_escape():
    vor, l = bestand(), lage()
    umbenennen_f2("Neu von außen.txt", "Verworfen", "Escape")
    q("wait 500")
    pruefe("13 Escape: nichts umbenannt", bestand() == vor and voll(l) and lage() == l)
    pruefe("13 Escape protokolliert", "Umbenennen abgebrochen: Neu von außen.txt" in d.text())
    bild("escape")


def test_14_fehler():
    vor = bestand()
    l = lage()
    # Name existiert
    umbenennen_f2("Projektunterlagen", "Apfel")
    pruefe("14 Namenskonflikt: protokolliert", "Umbenennen fehlgeschlagen: Projektunterlagen → Apfel" in d.text())
    meldung("14", "Name existiert", "fehler-name-existiert")
    pruefe("14 Namenskonflikt: nichts überschrieben", bestand() == vor)
    # Schrägstrich
    umbenennen_f2("Projektunterlagen", "a/b")
    pruefe("14 Schrägstrich abgelehnt", "Umbenennen fehlgeschlagen: Projektunterlagen → a/b" in d.text()
           and bestand() == vor)
    meldung("14", "Schrägstrich", "fehler-schraegstrich")
    # leerer Name
    pos = lage()["Projektunterlagen"]
    klick(mitte(pos))
    q("key F2", "wait 500", "key BackSpace", "wait 200", "key Return", "wait 900")
    pruefe("14 leerer Name abgelehnt", "Projektunterlagen → : Der Name darf nicht leer sein" in d.text()
           and bestand() == vor)
    meldung("14", "leerer Name", "fehler-leer")
    # inzwischen gelöscht
    pos = lage()["Neu von außen.txt"]
    klick(mitte(pos))
    q("key F2", "wait 500")
    os.remove(os.path.join(SCHREIBTISCH, "Neu von außen.txt"))
    q("wait 600", "type Gerettet", "key Return", "wait 900")
    pruefe("14 Datei inzwischen gelöscht: protokolliert", "gibt es nicht mehr" in d.text()
           and not os.path.exists(os.path.join(SCHREIBTISCH, "Gerettet.txt")))
    meldung("14", "Datei gelöscht", "fehler-geloescht")
    neue_lage(lambda m: "Neu von außen.txt" not in m, 4)
    pruefe("14 gelöschte Datei verschwindet danach", "Neu von außen.txt" not in lage())
    # schreibgeschützte Datei: Umbenennen hängt am Ordner, nicht an der Datei
    sg = os.path.join(SCHREIBTISCH, "Schreibgeschützt.txt")
    with open(sg, "w") as f:
        f.write("nur lesen\n")
    os.chmod(sg, 0o444)
    neue_lage(lambda m: "Schreibgeschützt.txt" in m, 5)
    umbenennen_f2("Schreibgeschützt.txt", "Nur lesen")
    ziel = os.path.join(SCHREIBTISCH, "Nur lesen.txt")
    neue_lage(lambda m: "Nur lesen.txt" in m, 5)
    pruefe("14 schreibgeschützte Datei umbenannt, Rechte 0444 erhalten",
           os.path.exists(ziel) and (os.stat(ziel).st_mode & 0o777) == 0o444)
    # fehlende Schreibberechtigung im Ordner: Menüeintrag deaktiviert, F2 meldet
    os.chmod(SCHREIBTISCH, 0o555)
    try:
        vor = bestand()
        m = mitte(lage()["Projektunterlagen"])
        n0 = d.text().count("Umbenennen: Projektunterlagen")
        klick(m, "rclick")
        q("wait 400")
        im = bild("menue-ohne-schreibrecht")
        klick((m[0] + 60, m[1] + 46))
        q("wait 600")
        pruefe("14 ohne Schreibrecht: „Umbenennen …“ deaktiviert (Klick bewirkt nichts)",
               d.text().count("Umbenennen: Projektunterlagen") == n0, d.text().count("Umbenennen: Projektunterlagen"))
        q("key Escape", "wait 300")
        klick(m)
        q("key F2", "wait 900")
        pruefe("14 ohne Schreibrecht: F2 erklärt es", "Umbenennen nicht möglich: Projektunterlagen" in d.text())
        meldung("14", "keine Schreibberechtigung", "f2-ohne-schreibrecht")
        pruefe("14 ohne Schreibrecht: nichts verändert", bestand() == vor)
    finally:
        os.chmod(SCHREIBTISCH, 0o755)
    ok, why = ueberlappungsfrei(lage())
    pruefe("14 Lage nach allen Fehlern intakt", ok and set(lage()) >= {"Projektunterlagen", "Apfel"}, why)


def test_15_16_starter():
    pfad = os.path.join(SCHREIBTISCH, "Mein Browser.desktop")
    vor = open(pfad).read()
    modus = os.stat(pfad).st_mode & 0o777
    l = lage()
    pos = l["Mein Browser"]
    umbenennen_f2("Mein Browser", "Internet")
    neu = neue_lage(lambda m: "Internet" in m, 5, "Starter umbenannt")
    nach = open(pfad).read()
    pruefe("15 Starter heißt jetzt Internet", "Internet" in neu and "Mein Browser" not in neu)
    pruefe("15 nur Name[de] geändert, alle anderen Zeilen byte-gleich",
           nach == vor.replace("Name[de]=Mein Browser\n", "Name[de]=Internet\n"))
    pruefe("15 Dateiname und Rechte unverändert", os.path.exists(pfad) and (os.stat(pfad).st_mode & 0o777) == modus,
           oct(os.stat(pfad).st_mode & 0o777))
    pruefe("17 Position des Starters erhalten", neu.get("Internet") == pos)
    vorher = len(marker())
    klick(mitte(neu["Internet"]), "dclick")
    warte(lambda: len(marker()) > vorher, 5, "Browser-Marker")
    pruefe("16 umbenannter Starter startet dieselbe Anwendung",
           marker()[vorher:] == ["browser "], marker()[vorher:])
    # eingebauter Starter: eigener Name im Zustand
    t = neu["Querencia Terminal"]
    klick(mitte(t), "rclick")
    q("wait 400")
    klick((mitte(t)[0] + 60, mitte(t)[1] + 46))
    q("wait 400", "type Mein Terminal", "key Return", "wait 900")
    neu = neue_lage(lambda m: "Mein Terminal" in m, 5)
    pruefe("15 Querencia Terminal → Mein Terminal (Alias)", neu.get("Mein Terminal") == t
           and "name\tprogramm\torg.example.Querencia.Terminal.desktop\tMein Terminal" in zustand_zeilen())
    vorher = len(marker())
    klick(mitte(t), "dclick")
    warte(lambda: len(marker()) > vorher, 5, "Terminal-Marker")
    pruefe("16 Mein Terminal öffnet weiterhin das Terminal", marker()[vorher:] == ["terminal "], marker()[vorher:])
    # Sonderobjekt: Funktion bleibt
    r = neu["Rechner"]
    umbenennen_f2("Rechner", "Arbeitsplatz")
    neu = neue_lage(lambda m: "Arbeitsplatz" in m, 5)
    vorher = len(marker())
    klick(mitte(r), "dclick")
    warte(lambda: len(marker()) > vorher, 5, "Files-Marker")
    pruefe("15 Rechner → Arbeitsplatz öffnet weiterhin „Rechner“",
           neu.get("Arbeitsplatz") == r and marker()[vorher:] == ["files computer:///"], marker()[vorher:])
    bild("starter-umbenannt")
    # nicht vertrauenswürdiger Starter: Nachfrage, Abbrechen startet nichts
    vorher = len(marker())
    klick(mitte(neu["Fremder Starter"]), "dclick")
    pruefe("16 fremder Starter: Nachfrage sichtbar", warte(dialog_offen, 6, "Nachfrage"))
    bild("fremder-starter-nachfrage")
    q("key Escape", "wait 800")
    pruefe("16 Nachfrage mit Escape geschlossen", warte(lambda: not dialog_offen(), 4))
    pruefe("16 fremder (nicht ausführbarer) Starter: Nachfrage, Abbrechen startet nichts",
           len(marker()) == vorher and "Starter Fremder Starter nicht gestartet" in d.text())


def test_19_aufraeumen():
    vor = bestand()
    l = lage()
    frei = (420, oben[0] + 440)
    if any(abs(frei[0] - 50 - x) < 100 and abs(frei[1] - oben[0] - 40 - y) < 88 for x, y in l.values()):
        frei = (700, oben[0] + 150)
    klick(frei, "rclick")
    q("wait 400")
    klick((frei[0] + 60, frei[1] + 70))
    neu = neue_lage(lambda m: m != l, 5, "aufgeräumt")
    rang = {"Arbeitsplatz": 0, "Persönlicher Ordner": 1, "Papierkorb": 2, "Mein Terminal": 3}
    namen = sorted(neu, key=lambda n: (rang.get(n, 9), n.lower() if n not in rang else "", n))
    w, h = flaeche()
    zeilen = (h - 2 * MARGIN - CELL[1]) // CELL[1] + 1
    erwartet = {n: (MARGIN + (i // zeilen) * CELL[0], MARGIN + (i % zeilen) * CELL[1])
                for i, n in enumerate(namen)}
    pruefe("19 nach Namen aufgeräumt (feste Symbole, dann Namen)", neu == erwartet,
           {k: (neu.get(k), v) for k, v in erwartet.items() if neu.get(k) != v})
    pruefe("19 keine Datei verschoben oder umbenannt", bestand() == vor)
    z = zustand_zeilen()
    pruefe("19 neue Positionen gespeichert", "sonder\tcomputer\tlinks 8\toben 8" in z)
    bild("aufgeraeumt")


def test_20_doppelklick():
    l = lage()
    vorher = len(marker())
    klick(mitte(l["Apfel"]), "dclick")
    warte(lambda: len(marker()) > vorher, 5, "Apfel geöffnet")
    pruefe("20 Doppelklick öffnet den Ordner (Ordnerprogramm mit seinem Pfad)",
           marker()[vorher:] == [f"files {os.path.join(SCHREIBTISCH, 'Apfel')}"], marker()[vorher:])
    # Ziehen, dann sofort klicken: kein Doppelklick
    vorher = len(marker())
    a = mitte(l["Apfel"])
    q(f"move {a[0]} {a[1]}", "wait 100", "press", "wait 50", f"glide {a[0] + 30} {a[1] + 200} 150",
      "release", "wait 60", "click", "wait 1200")
    pruefe("20 Ziehen + Klick löst keinen Doppelklick aus", len(marker()) == vorher, marker()[vorher:])
    neu = lage()
    pruefe("20 das Ziehen hat verschoben", neu.get("Apfel") != l["Apfel"])
    # Klick, dann drücken und ziehen: kein Öffnen
    vorher = len(marker())
    b = mitte(neu["Apfel"])
    q(f"move {b[0]} {b[1]}", "wait 100", "click", "wait 60", "press", "wait 40",
      f"glide {b[0] + 150} {b[1]} 150", "release", "wait 1200")
    pruefe("20 Klick + Ziehen öffnet nicht", len(marker()) == vorher, marker()[vorher:])
    neu = lage()
    vorher = len(marker())
    klick(mitte(neu["Papierkorb"]), "dclick")
    warte(lambda: len(marker()) > vorher, 5)
    pruefe("20 Doppelklick Papierkorb", marker()[vorher:] == ["files trash:///"], marker()[vorher:])


def test_21_kontextmenue():
    l = lage()
    m = mitte(l["Projektunterlagen"])
    klick(m, "rclick")
    q("wait 500")
    im = bild("symbolmenue-regression")
    hell = sum(nah(im.getpixel((x, y)), (0xF5, 0xF4, 0xF2), 6)
               for x in range(m[0] + 5, m[0] + 140, 3) for y in range(m[1] + 3, m[1] + 55, 3))
    pruefe("21 Symbolmenü am Zeiger (zwei Einträge)", hell > 300, hell)
    vorher = len(marker())
    klick((m[0] + 60, m[1] + 15))
    warte(lambda: len(marker()) > vorher, 5)
    pruefe("21 „Öffnen“ öffnet", any(z.endswith("/Projektunterlagen") for z in marker()[vorher:]), marker()[vorher:])
    frei = (700, oben[0] + 160)
    klick(frei, "rclick")
    q("wait 500")
    im = bild("arbeitsflaechenmenue-regression")
    hell = sum(nah(im.getpixel((x, y)), (0xF5, 0xF4, 0xF2), 6)
               for x in range(frei[0] + 5, frei[0] + 140, 3) for y in range(frei[1] + 3, frei[1] + 110, 3))
    pruefe("21 Arbeitsflächenmenü am Zeiger", hell > 600, hell)
    q("key Escape", "wait 500")
    alle = "".join(open(os.path.join(LOGS, f), errors="replace").read()
                   for f in os.listdir(LOGS) if f.startswith("desktop-"))
    pruefe("21 keine GTK-Kritikmeldung, keine Panik (alle Desktop-Starts)",
           "CRITICAL" not in alle and "panicked" not in alle)


def test_22_arbeitsflaechen():
    l = lage()
    q("run foot -a foot -T 'Fenster auf Fläche 1' sleep 600", "wait 2500")
    im1 = bild("flaeche-1-mit-fenster")
    q("keydown Control_L", "keydown Alt_L", "key Right", "keyup Alt_L", "keyup Control_L", "wait 1200")
    im2 = bild("flaeche-2")
    w, h = im2.size
    unterschied = sum(not nah(im1.getpixel((x, y)), im2.getpixel((x, y)), 20)
                      for x in range(400, 900, 10) for y in range(200, 500, 10))
    pruefe("22 Wechsel auf Fläche 2 (Fenster weg)", unterschied > 300, unterschied)
    pruefe("22 Symbole auf Fläche 2 sichtbar", voll(l) and all(symbol_da(im2, p)[0] for p in l.values()))
    ziehen(mitte(l["Apfel"]), (900, oben[0] + 500), 500)
    neu = neue_lage(lambda m: m.get("Apfel") != l["Apfel"], 4)
    pruefe("22 Ziehen auf Fläche 2", neu.get("Apfel") != l["Apfel"])
    for _ in range(3):
        q("keydown Control_L", "keydown Alt_L", "key Right", "keyup Alt_L", "keyup Control_L", "wait 700")
    im = bild("zurueck-auf-flaeche-1")
    pruefe("22 vier Flächen: nach 4× Rechts wieder Fläche 1 (Fenster da)",
           sum(not nah(im1.getpixel((x, y)), im.getpixel((x, y)), 20)
               for x in range(400, 900, 10) for y in range(200, 500, 10)) < 60)
    pruefe("22 Position auf Fläche 1 dieselbe", voll(neu) and lage() == neu)
    subprocess.run(["pkill", "-f", "Fenster auf Fläche 1"], check=False)
    q("wait 800")


def test_23_hintergrund():
    im = bild("hintergrund")
    l = lage()
    w, h = flaeche()
    frei = [(x, y) for x in range(20, w - 20, 37) for y in range(20, h - 20, 41)
            if all(not (px - 4 <= x <= px + 104 and py - 4 <= y <= py + 92) for px, py in l.values())]
    gut = sum(nah(im.getpixel(bildschirm(x, y)), WALL, 3) for x, y in frei)
    pruefe("23 Hintergrund überall außerhalb der Symbole unverändert", gut == len(frei), f"{gut}/{len(frei)}")


def test_24_panelgrenzen():
    im = bild("panels-am-ende")
    pruefe("24 keine offene Meldung am Ende (Fensterliste leer)",
           not dialog_offen() and all(nah(im.getpixel((x, im.size[1] - 16)), PANEL_BG, 14)
                                      for x in range(20, 400, 10)))
    o, u = panelstreifen(im)
    pruefe("24 oberes Panel unverdeckt", o == panels_vorher[0])
    pruefe("24 unteres Panel unverdeckt", u == panels_vorher[1])
    ok, why = im_bereich(lage())
    pruefe("24 kein Symbol unter einem Panel", ok, why)


def test_25_aufloesungen():
    # Abnahme: Rechner (jetzt „Arbeitsplatz“) wieder nach rechts oben
    w, h = flaeche()
    ziehen(mitte(lage()["Arbeitsplatz"]), (w - 30, oben[0] + 40), 600)
    neue_lage(lambda m: m.get("Arbeitsplatz") == (w - 108, 8), 4)
    l = lage()
    z = zustand()
    w0, h0 = flaeche()
    rechner = l["Arbeitsplatz"]
    for modus, skal in (("1920x1080", None), ("1024x768", None), ("1280x720", "2")):
        args = [QA, modus] + ([skal] if skal else [])
        r = subprocess.run(args, capture_output=True, text=True)
        protokoll(r.stderr.strip())
        lw, lh = map(int, modus.split("x"))
        if skal:
            lw, lh = lw // int(skal), lh // int(skal)
        warte(lambda: flaeche() and flaeche()[0] == lw, 6, f"Fläche {modus}")
        time.sleep(1.5)
        neu = lage()
        im = bild(f"aufloesung-{modus}{'-skalierung-' + skal if skal else ''}")
        oben[0] = miss_panel(im) // (int(skal) if skal else 1)
        w, h = flaeche()
        ok1, why1 = im_bereich(neu)
        ok2, why2 = ueberlappungsfrei(neu)
        pruefe(f"25 {modus}{' ×' + skal if skal else ''}: alle Symbole sichtbar", ok1 and set(neu) == set(l), why1)
        pruefe(f"25 {modus}{' ×' + skal if skal else ''}: keine Überlappung", ok2, why2)
        if not skal:
            pruefe(f"25 {modus}: Arbeitsplatz (Rechner) bleibt rechts oben",
                   neu.get("Arbeitsplatz") == (w - MARGIN - CELL[0], MARGIN), neu.get("Arbeitsplatz"))
        pruefe(f"25 {modus}: Zustandsdatei unverändert (nur Anzeige angepasst)", zustand() == z)
    subprocess.run([QA, "1280x720", "1"], capture_output=True)
    warte(lambda: flaeche() == (w0, h0), 6, "zurück")
    time.sleep(1.5)
    im = bild("aufloesung-zurueck")
    oben[0] = miss_panel(im)
    pruefe("25 zurück auf 1280x720: alle Positionen wie vorher", voll(l) and lage() == l,
           {k: (l.get(k), lage().get(k)) for k in l if l.get(k) != lage().get(k)})
    pruefe("25 Rechner wieder an seinem Platz", lage().get("Arbeitsplatz") == rechner)


def main():
    global q, panels_vorher
    os.makedirs(BILDER, exist_ok=True)
    protokoll(f"Desktop {DESKTOP} ({hashlib.sha256(open(DESKTOP, 'rb').read()).hexdigest()[:16]})")
    protokoll(f"Panel {PANEL} ({hashlib.sha256(open(PANEL, 'rb').read()).hexdigest()[:16]})")
    panel = subprocess.Popen([PANEL], stdout=open(os.path.join(LOGS, "panel.log"), "w"), stderr=subprocess.STDOUT)
    time.sleep(3)
    q = Eingabe()
    q("move 640 700")
    if os.environ.get("HMP10_NUR") == "migration":
        os.makedirs(os.path.join(P, "state", "querencia"), exist_ok=True)
        with open(os.path.join(P, "state", "querencia", "desktop-order"), "w") as f:
            f.write("special:trash\nfile:Zebra.txt\nspecial:computer\n")
    d.start(lambda l: len(l) == 8)
    im = bild("leer-vor-den-tests")
    oben[0] = miss_panel(im)
    protokoll(f"oberes Panel {oben[0]} px, Fläche {flaeche()}")
    panels_vorher = panelstreifen(im)
    tests = [test_01_auswahl, test_02_rechts, test_02b_zwischenbild, test_05_kollision, test_03_unten,
             test_04_panelgrenze, test_06_gespeichert, test_07_08_neustart, test_09_10_neue_datei,
             test_09b_extern_umbenannt, test_11_f2_ordner, test_12_kontextmenue, test_13_escape, test_14_fehler, test_15_16_starter,
             lambda: test_07_08_neustart("18", "nach-zweitem-neustart"),
             test_19_aufraeumen, test_20_doppelklick, test_21_kontextmenue, test_22_arbeitsflaechen,
             test_23_hintergrund, test_24_panelgrenzen, test_25_aufloesungen,
             lambda: test_07_08_neustart("18", "nach-letztem-neustart")]
    if os.environ.get("HMP10_NUR") == "migration":
        tests = [test_migration, lambda: test_07_08_neustart("M", "migration-neustart")]
    for t in tests:
        name = getattr(t, "__name__", "test")
        protokoll(f"=== {name}")
        try:
            t()
        except Exception as err:  # noqa: BLE001 – record and go on
            pruefe(f"{name} ohne Ausnahme", False, f"{err!r}\n{traceback.format_exc()}")
        # nach jedem Test: keine Bearbeitung offen, keine Menüs
        q("key Escape", "wait 200")
    d.stop()
    panel.terminate()
    ok = sum(1 for _, o, _ in ergebnisse if o)
    with open(os.path.join(LOGS, "pruefung.txt"), "w") as f:
        for n, o, det in ergebnisse:
            f.write(f"{'OK    ' if o else 'FEHLER'} {n}{'  – ' + det if det and not o else ''}\n")
        f.write(f"{ok}/{len(ergebnisse)} Prüfungen bestanden\n")
    with open(os.path.join(LOGS, "ergebnis.json"), "w") as f:
        json.dump({"bestanden": ok, "gesamt": len(ergebnisse), "pruefungen": ergebnisse}, f,
                  ensure_ascii=False, indent=1)
    protokoll(f"{ok}/{len(ergebnisse)} Prüfungen bestanden")


if __name__ == "__main__":
    main()
