#!/usr/bin/env python3
"""QE-01: Prüftreiber für Hintergrund „nur Farbe“, „Zentrieren“, „Kacheln“
und den Menüpunkt „Hintergrund der Arbeitsfläche ändern …“.

Läuft wie hmp10a-treiber.py in der isolierten labwc-Sitzung
(pruefen-hmp10.sh mit HMP10_TREIBER=…/qe01-treiber.py) und benutzt dessen
Hilfsfunktionen (echte Eingaben über qeingabe, Lage der Symbole aus dem
Protokoll, Bilder über grim, Ausgabegröße über qausgabe).

Geprüft wird am Bildschirmfoto gegen ein Sollbild (gleiche Rechnung wie
config::placement bzw. modell.rs::platzierung in Querencia Einstellungen):
an einem Punktraster außerhalb von Panels und Symbolen und pixelgenau an den
Bild- und Kachelkanten. desktop.ini wird wie von Querencia Einstellungen
geschrieben (Zwischendatei + rename); der Desktop folgt ohne Neustart.

Ausgabe: logs/pruefung.txt, logs/bilder/*.png.
"""

import importlib.util
import os
import shutil
import subprocess
import time
import traceback

HIER = os.path.dirname(os.path.abspath(__file__))
_spec = importlib.util.spec_from_file_location("a", os.path.join(HIER, "hmp10a-treiber.py"))
a = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(a)
t = a.t

P, LOGS = t.P, t.LOGS
d, pruefe, bild, lage, flaeche, warte, nah = t.d, t.pruefe, t.bild, t.lage, t.flaeche, t.warte, t.nah
Image = t.Image
INI = os.path.join(P, "config", "querencia", "desktop.ini")
MUSTER = os.path.join(P, "muster-300x200.png")
GROSS = os.path.join(P, "muster-1600x1200.png")
FARBE = (0x55, 0x22, 0x77)
FARBE_TEXT = "#552277"
# Viertel des Musters: links oben, rechts oben, links unten, rechts unten
VIERTEL = ((200, 40, 40), (40, 160, 60), (220, 200, 40), (120, 60, 160))
HINTERGRUND_MENUE = 149     # „Hintergrund der Arbeitsfläche ändern …“ im Arbeitsflächenmenü
EINSTELLUNGEN_LOG = os.path.join(P, "einstellungen.log")


def q(*befehle):
    t.q(*befehle)


def muster(w, h):
    im = Image.new("RGB", (w, h))
    for i, farbe in enumerate(VIERTEL):
        x0, y0 = (i % 2) * (w // 2), (i // 2) * (h // 2)
        im.paste(farbe, (x0, y0, x0 + w // 2, y0 + h // 2))
    return im


def schreibe(text):
    """desktop.ini wie Querencia Einstellungen: Zwischendatei, dann rename."""
    neu = INI + ".neu"
    with open(neu, "w") as f:
        f.write(text)
    os.replace(neu, INI)


def ini(typ=None, datei="", modus="zoom", farbe=FARBE_TEXT):
    return "[Background]\n" + (f"Type={typ}\n" if typ else "") + f"File={datei}\nMode={modus}\nColor={farbe}\n"


def log_seit(n):
    return d.text()[n:]


def lade(text):
    """desktop.ini schreiben und warten, bis der Desktop sie neu gelesen und
    gezeichnet hat (eine neue Protokollzeile „Hintergrund …“)."""
    n = len(d.text())
    schreibe(text)
    ok = warte(lambda: "querencia-desktop: Hintergrund" in log_seit(n), 8, "desktop.ini neu gelesen")
    time.sleep(0.8)
    return bool(ok), n


# ----------------------------------------------------------------------------
# Sollbild und Vergleich

def soll(art, groesse, skal=1, quelle=None, farbe=FARBE):
    """Sollbild in Bildschirmpixeln. art: farbe | centered | tiled | zoom |
    scaled | stretched. Zentrieren und Kacheln: ein Bildpixel je Bildschirmpixel."""
    w, h = groesse
    canvas = Image.new("RGB", (w, h), farbe)
    if art == "farbe":
        return canvas
    iw, ih = quelle.size
    if art == "centered":
        canvas.paste(quelle, ((w - iw) // 2, (h - ih) // 2))
    elif art == "tiled":
        for y in range(0, h, ih):
            for x in range(0, w, iw):
                canvas.paste(quelle, (x, y))
    else:
        if art == "stretched":
            x, y, sw, sh = 0, 0, w, h
        else:
            f = max(w / iw, h / ih) if art == "zoom" else min(w / iw, h / ih)
            sw, sh = iw * f, ih * f
            x, y = (w - sw) / 2, (h - sh) / 2
        canvas.paste(quelle.resize((round(sw), round(sh)), Image.NEAREST), (round(x), round(y)))
    return canvas


def frei(im, skal):
    """Bildschirmpunkte außerhalb der Panels und der Symbolkacheln."""
    w, h = im.size
    oben = t.miss_panel(im)
    l = lage()
    punkte = []
    for y in range(oben + 4, h - oben - 4, 7 * skal):
        for x in range(3, w - 3, 9 * skal):
            fx, fy = x / skal, y / skal - oben / skal
            if all(not (px - 6 <= fx <= px + 106 and py - 6 <= fy <= py + 94) for px, py in l.values()):
                punkte.append((x, y))
    return punkte


def rechteck(art, groesse, quelle):
    """Bildrechteck in Bildschirmpixeln (wie config::placement, ein Bildpixel
    je Bildschirmpixel bei centered/tiled)."""
    w, h = groesse
    iw, ih = quelle.size
    if art in ("centered", "tiled"):
        return ((w - iw) // 2, (h - ih) // 2, iw, ih) if art == "centered" else (0, 0, iw, ih)
    if art == "stretched":
        return 0, 0, w, h
    f = max(w / iw, h / ih) if art == "zoom" else min(w / iw, h / ih)
    return (w - iw * f) / 2, (h - ih * f) / 2, iw * f, ih * f


def kante(art, groesse, quelle, x, y):
    """Liegt (x, y) auf weniger als 3 px an einer Bild- oder Viertelkante?
    (Dort mischt die Filterung zwei Farben.)"""
    if art == "farbe":
        return False
    w, h = groesse
    x0, y0, rw, rh = rechteck(art, groesse, quelle)
    if art == "tiled":
        xs = [k * rw / 2 for k in range(0, 2 * int(w // rw) + 3)]
        ys = [k * rh / 2 for k in range(0, 2 * int(h // rh) + 3)]
    else:
        xs, ys = [x0, x0 + rw / 2, x0 + rw], [y0, y0 + rh / 2, y0 + rh]
    return min(abs(x - v) for v in xs) < 3 or min(abs(y - v) for v in ys) < 3


def vergleiche(name, im, art, skal=1, quelle=None, farbe=FARBE, tol=8):
    s = soll(art, im.size, skal, quelle, farbe)
    gut, schlecht, beispiele = 0, 0, []
    for x, y in frei(im, skal):
        if kante(art, im.size, quelle, x, y):
            continue
        p, e = im.getpixel((x, y)), s.getpixel((x, y))
        if nah(p, e, tol):
            gut += 1
        else:
            schlecht += 1
            if len(beispiele) < 4:
                beispiele.append(((x, y), p, e))
    return pruefe(f"{name}: Bild wie Sollbild ({art})", gut >= 150 and schlecht == 0,
                  f"{gut} gleich, {schlecht} anders {beispiele}")


def punkt_ist(im, xy, farbe, tol=10):
    w, h = im.size
    x, y = xy
    return 0 <= x < w and 0 <= y < h and nah(im.getpixel((x, y)), farbe, tol)


def ecken_zentriert(name, im, quelle, farbe=FARBE):
    """Pixelgenau: erstes und letztes Bildpixel, je ein Pixel davor/dahinter."""
    w, h = im.size
    iw, ih = quelle.size
    x0, y0 = (w - iw) // 2, (h - ih) // 2
    x1, y1 = x0 + iw - 1, y0 + ih - 1
    falsch = []
    for xy, soll_farbe in (((x0, y0), VIERTEL[0]), ((x0 - 1, y0), farbe), ((x0, y0 - 1), farbe),
                           ((x1, y1), VIERTEL[3]), ((x1 + 1, y1), farbe), ((x1, y1 + 1), farbe),
                           ((x1, y0), VIERTEL[1]), ((x0, y1), VIERTEL[2])):
        if not punkt_ist(im, xy, soll_farbe):
            falsch.append((xy, im.getpixel(xy), soll_farbe))
    return pruefe(f"{name}: Bildkanten pixelgenau ({iw}×{ih} bei ({x0}, {y0}))", not falsch, falsch)


def kachelkanten(name, im, quelle, skal=1):
    """Pixelgenau an freien Kachelecken: letztes Pixel der einen, erstes der
    nächsten Kachel; dazu muss die erste Kachel bei (0, 0) beginnen."""
    w, h = im.size
    iw, ih = quelle.size
    l = lage()
    oben = t.miss_panel(im)

    def ist_frei(x, y):
        fx, fy = x / skal, (y - oben) / skal
        return oben + 4 < y < h - oben - 4 and all(
            not (px - 8 <= fx <= px + 108 and py - 8 <= fy <= py + 96) for px, py in l.values())

    geprueft, falsch = 0, []
    for ky in range(1, h // ih + 1):
        for kx in range(1, w // iw + 1):
            x, y = kx * iw, ky * ih
            if x + 1 >= w or y + 1 >= h or not (ist_frei(x - 1, y - 1) and ist_frei(x, y)):
                continue
            geprueft += 1
            for xy, farbe in (((x - 1, y - 1), VIERTEL[3]), ((x, y), VIERTEL[0]),
                              ((x - 1, y), VIERTEL[1]), ((x, y - 1), VIERTEL[2])):
                if not punkt_ist(im, xy, farbe):
                    falsch.append((xy, im.getpixel(xy), farbe))
    return pruefe(f"{name}: Kachelkanten pixelgenau ({iw}×{ih} ab links oben)", geprueft >= 2 and not falsch,
                  f"{geprueft} Kachelecken, falsch {falsch[:4]}")


def warte_bild(bedingung, sekunden=8):
    """Bildschirmfoto, bis bedingung(im) gilt (der Desktop lädt asynchron)."""
    pfad = os.path.join(LOGS, "warte-bild.png")
    ende = time.time() + sekunden
    while time.time() < ende:
        subprocess.run(["grim", pfad], check=False)
        try:
            if bedingung(Image.open(pfad).convert("RGB")):
                return True
        except Exception:  # noqa: BLE001 – still waiting
            pass
        time.sleep(0.3)
    return False


def mitte_ist(farbe, dx=0, dy=0):
    return lambda im: punkt_ist(im, (im.size[0] // 2 + dx, im.size[1] // 2 + dy), farbe, 10)


def ausgabe(modus, skal="1"):
    r = subprocess.run([t.QA, modus, skal], capture_output=True, text=True)
    t.protokoll(r.stderr.strip())
    w, h = map(int, modus.split("x"))
    lw = w // int(skal)
    warte(lambda: flaeche() and flaeche()[0] == lw, 8, modus)
    time.sleep(1.5)


# ----------------------------------------------------------------------------
# A1: nur Farbe

def test_q01_farbe():
    da, n = lade(ini("color", MUSTER, "centered"))
    im = bild("q01-farbe-mit-datei")
    pruefe("Q01 Type=color (File gesetzt, Mode=centered): Farbe erscheint ohne Neustart", da)
    vergleiche("Q01 Type=color", im, "farbe")
    neu = log_seit(n)
    pruefe("Q01 Protokoll: nur Farbe, kein Bild geladen",
           f"Hintergrund nur Farbe {FARBE_TEXT} (Type=color)" in neu and "Modus centered" not in neu, neu.strip())
    viertel = sum(1 for x, y in frei(im, 1) if any(nah(im.getpixel((x, y)), v, 20) for v in VIERTEL))
    pruefe("Q01 keine Bildreste des Musters", viertel == 0, f"{viertel} Musterpixel")


def test_q02_farbe_ohne_datei():
    da, n = lade(ini("color", "", "zoom", "#204a87"))
    im = bild("q02-farbe-ohne-datei")
    pruefe("Q02 Type=color ohne File: Farbe statt eingebautem Bild", da)
    vergleiche("Q02 Type=color ohne File", im, "farbe", farbe=(0x20, 0x4A, 0x87), tol=3)
    neu = log_seit(n)
    pruefe("Q02 eingebautes Bild nicht geladen", "querencia-blau" not in neu and "nur Farbe #204a87" in neu,
           neu.strip())


# ----------------------------------------------------------------------------
# A2: Zentrieren, Kacheln, alte Modi, Auflösungen, Skalierung

def zentriert(name, quelle, datei, skal=1):
    da, _ = lade(ini(None, datei, "centered"))
    im = bild(f"{name[:3].lower()}-zentriert-{quelle.size[0]}x{quelle.size[1]}")
    w, h = im.size
    pruefe(f"{name}: Bild erscheint ohne Neustart", da)
    vergleiche(name, im, "centered", skal, quelle)
    if quelle.size[0] < w:
        ecken_zentriert(name, im, quelle)
    else:
        # größer als der Monitor: mittig beschnitten, keine Farbe sichtbar
        cx, cy = w // 2, h // 2
        falsch = [(xy, im.getpixel(xy), f) for xy, f in (((cx - 1, cy - 1), VIERTEL[0]), ((cx, cy - 1), VIERTEL[1]),
                                                         ((cx - 1, cy), VIERTEL[2]), ((cx, cy), VIERTEL[3]))
                  if not punkt_ist(im, xy, f)]
        pruefe(f"{name}: Bildmitte = Bildschirmmitte, beschnitten", not falsch, falsch)
        farbe = sum(1 for xy in frei(im, skal) if nah(im.getpixel(xy), FARBE, 10))
        pruefe(f"{name}: keine Farbfläche (Bild größer als der Monitor)", farbe == 0, f"{farbe} Farbpunkte")
    return im


def gekachelt(name, quelle, datei, skal=1):
    da, _ = lade(ini(None, datei, "tiled"))
    im = bild(f"{name[:3].lower()}-gekachelt")
    pruefe(f"{name}: Kacheln erscheinen ohne Neustart", da)
    vergleiche(name, im, "tiled", skal, quelle)
    kachelkanten(name, im, quelle, skal)
    farbe = sum(1 for xy in frei(im, skal) if nah(im.getpixel(xy), FARBE, 10))
    pruefe(f"{name}: keine Farbfläche (ganze Fläche gekachelt)", farbe == 0, f"{farbe} Farbpunkte")
    return im


def test_q03_zentriert():
    zentriert("Q03 Mode=centered 1280×720", muster(300, 200), MUSTER)


def test_q04_gross():
    zentriert("Q04 Mode=centered, Bild 1600×1200 > 1280×720", muster(1600, 1200), GROSS)


def test_q05_gekachelt():
    gekachelt("Q05 Mode=tiled 1280×720", muster(300, 200), MUSTER)


def test_q06_wechsel():
    """color → image → color ohne Neustart; derselbe Prozess."""
    pid = d.proc.pid
    starts = d.starts
    lade(ini("color", MUSTER, "centered"))
    a1 = mitte_ist(FARBE)(bild("q06-1-farbe"))
    lade(ini(None, MUSTER, "centered"))
    a2 = mitte_ist(VIERTEL[3], 10, 10)(bild("q06-2-bild"))
    lade(ini("color", MUSTER, "centered"))
    im = bild("q06-3-farbe")
    a3 = mitte_ist(FARBE)(im)
    pruefe("Q06 color → image → color ohne Neustart", a1 and a2 and a3 and d.proc.poll() is None
           and d.proc.pid == pid and d.starts == starts, (a1, a2, a3))
    vergleiche("Q06 zurück auf Farbe", im, "farbe")


def test_q07_aufloesungen():
    m = muster(300, 200)
    for modus, skal in (("1024x768", 1), ("1920x1080", 1), ("1920x1080", 2)):
        ausgabe(modus, str(skal))
        name = f"{modus}{' ×2' if skal == 2 else ''}"
        zentriert(f"Q07 centered {name}", m, MUSTER, skal)
        gekachelt(f"Q07 tiled {name}", m, MUSTER, skal)
        lade(ini("color", MUSTER, "tiled"))
        vergleiche(f"Q07 color {name}", bild(f"q07-farbe-{modus}-x{skal}"), "farbe", skal)
    ausgabe("1280x720", "1")


def test_q08_unbekannter_typ():
    da, n = lade("[Background]\nType=muster\nFile=" + MUSTER + "\nMode=centered\nColor=" + FARBE_TEXT + "\n")
    im = bild("q08-unbekannter-typ")
    pruefe("Q08 Type=muster: Bild wird gezeigt (image)", da)
    vergleiche("Q08 Type=muster", im, "centered", 1, muster(300, 200))
    neu = log_seit(n)
    pruefe("Q08 Logzeile „unbekannter Typ“", "unbekannter Typ „muster“, image" in neu and "Modus centered" in neu,
           neu.strip())


def test_q09_alte_modi():
    m = muster(300, 200)
    for modus in ("zoom", "scaled", "stretched"):
        lade(ini(None, MUSTER, modus))
        vergleiche(f"Q09 Mode={modus} unverändert", bild(f"q09-{modus}"), modus, 1, m, tol=14)


# ----------------------------------------------------------------------------
# --faehigkeiten

def test_q10_faehigkeiten():
    vorher = d.text()
    env = {k: v for k, v in os.environ.items() if k not in ("WAYLAND_DISPLAY", "DISPLAY")}
    start = time.time()
    r = subprocess.run([t.DESKTOP, "--faehigkeiten"], capture_output=True, text=True, timeout=10, env=env)
    dauer = time.time() - start
    pruefe("Q10 --faehigkeiten ohne Display: eine Zeile, Exit 0",
           r.returncode == 0 and r.stdout == "type-color mode-centered mode-tiled\n" and not r.stderr,
           (r.returncode, r.stdout, r.stderr, f"{dauer:.2f} s"))
    r2 = subprocess.run([t.DESKTOP, "--faehigkeiten"], capture_output=True, text=True, timeout=10)
    time.sleep(1)
    pruefe("Q10 --faehigkeiten neben laufender Instanz: gleiche Ausgabe, Instanz unberührt",
           r2.returncode == 0 and r2.stdout == r.stdout and d.proc.poll() is None and d.text() == vorher,
           (r2.returncode, r2.stdout, d.text()[len(vorher):]))


# ----------------------------------------------------------------------------
# A3: Menüpunkt

def einstellungen_skript(pfad, wer, shebang="#!/bin/sh"):
    os.makedirs(os.path.dirname(pfad), exist_ok=True)
    with open(pfad, "w") as f:
        f.write(f'{shebang}\necho "{wer} $* token=${{XDG_ACTIVATION_TOKEN:+ja}}" >> "{EINSTELLUNGEN_LOG}"\n')
    os.chmod(pfad, 0o755)


def mit_umgebung(path_vorne, home):
    """Desktop neu starten, mit PATH-Vorsatz und eigenem HOME (nur hier)."""
    d.stop()
    alt = dict(os.environ)
    if path_vorne:
        os.environ["PATH"] = path_vorne + ":" + alt["PATH"]
    os.environ["HOME"] = home
    try:
        d.start(lambda l: len(l) >= 8)
    finally:
        os.environ.clear()
        os.environ.update(alt)


def fremdfenster(im, vorher):
    """Anteil der Punkte um die Bildschirmmitte, die nicht mehr wie vorher sind."""
    w, h = im.size
    pts = [(x, y) for x in range(w // 2 - 250, w // 2 + 250, 10) for y in range(h // 2 - 150, h // 2 + 150, 10)]
    return sum(1 for p in pts if not nah(im.getpixel(p), vorher.getpixel(p), 12)) / len(pts)


def menuepunkt(name):
    vorher_ini = open(INI).read()
    vorher_bild = bild(f"{name}-vorher")
    n = len(d.text())
    a.menue(HINTERGRUND_MENUE)
    q("wait 800")
    return vorher_ini, vorher_bild, n


def einstellungen_zeilen():
    try:
        return open(EINSTELLUNGEN_LOG).read().splitlines()
    except OSError:
        return []


def test_a3():
    heim_opt = os.path.join(P, "heim-mit-opt")
    heim_leer = os.path.join(P, "heim-leer")
    os.makedirs(heim_leer, exist_ok=True)
    einstellungen_skript(os.path.join(heim_opt, "opt/querencia/bin/querencia-einstellungen"), "opt")
    einstellungen_skript(os.path.join(P, "einst-bin", "querencia-einstellungen"), "path")
    einstellungen_skript(os.path.join(P, "kaputt-bin", "querencia-einstellungen"), "kaputt", "#!/gibt/es/nicht/sh")
    lade(ini(None, MUSTER, "tiled"))

    # a) PATH und ~/opt vorhanden: PATH gewinnt
    mit_umgebung(os.path.join(P, "einst-bin"), heim_opt)
    vor_ini, vor_bild, n = menuepunkt("a3a")
    z = warte(lambda: einstellungen_zeilen(), 5, "Einstellungen gestartet")
    time.sleep(1)
    im = bild("a3a-nach-menue")
    pruefe("A3a PATH zuerst: querencia-einstellungen --hintergrund gestartet",
           z == ["path --hintergrund token=ja"], (z, log_seit(n).strip()))
    pruefe("A3a kein Dateidialog, desktop.ini nicht geschrieben",
           fremdfenster(im, vor_bild) < 0.05 and open(INI).read() == vor_ini, f"{fremdfenster(im, vor_bild):.2f}")
    pruefe("A3a Protokoll", "Hintergrund-Einstellungen gestartet: " in log_seit(n), log_seit(n).strip())

    # b) nur ~/opt/querencia/bin
    os.remove(EINSTELLUNGEN_LOG)
    mit_umgebung(None, heim_opt)
    vor_ini, vor_bild, n = menuepunkt("a3b")
    z = warte(lambda: einstellungen_zeilen(), 5, "Einstellungen gestartet")
    time.sleep(1)
    im = bild("a3b-nach-menue")
    pruefe("A3b ~/opt/querencia/bin/querencia-einstellungen --hintergrund gestartet",
           z == ["opt --hintergrund token=ja"], (z, log_seit(n).strip()))
    pruefe("A3b kein Dateidialog, desktop.ini nicht geschrieben",
           fremdfenster(im, vor_bild) < 0.05 and open(INI).read() == vor_ini, f"{fremdfenster(im, vor_bild):.2f}")
    os.remove(EINSTELLUNGEN_LOG)

    # c) nicht installiert: Dateidialog wie bisher, Esc ändert nichts
    mit_umgebung(None, heim_leer)
    vor_ini, vor_bild, n = menuepunkt("a3c")
    offen = warte(lambda: fremdfenster(bild_jetzt(), vor_bild) > 0.5, 6, "Dateidialog")
    bild("a3c-dateidialog")
    pruefe("A3c nicht installiert: Dateidialog öffnet sich", offen and not einstellungen_zeilen())
    q("key Escape", "wait 800")
    zu = warte(lambda: fremdfenster(bild_jetzt(), vor_bild) < 0.05, 5, "Dateidialog zu")
    pruefe("A3c Esc schließt den Dialog, desktop.ini unverändert", zu and open(INI).read() == vor_ini)

    # d) installiert, aber nicht startbar: Rückfall auf den Dateidialog
    mit_umgebung(os.path.join(P, "kaputt-bin"), heim_leer)
    vor_ini, vor_bild, n = menuepunkt("a3d")
    offen = warte(lambda: fremdfenster(bild_jetzt(), vor_bild) > 0.5, 6, "Dateidialog")
    bild("a3d-dateidialog")
    pruefe("A3d Start scheitert: Logzeile und Dateidialog",
           offen and "nicht gestartet" in log_seit(n) and "Dateiauswahl" in log_seit(n), log_seit(n).strip())
    q("key Escape", "wait 800")
    warte(lambda: fremdfenster(bild_jetzt(), vor_bild) < 0.05, 5, "Dateidialog zu")

    # e) Rückfall-Dialog nach Type=color: gewähltes Bild wird gezeigt (Type entfällt)
    mit_umgebung(None, heim_leer)
    lade(ini("color", "", "centered"))
    vor_ini, vor_bild, n = menuepunkt("a3e")
    offen = warte(lambda: fremdfenster(bild_jetzt(), vor_bild) > 0.5, 6, "Dateidialog")
    # „/“ öffnet im GTK-Dateidialog die Pfadeingabe; ganzer Pfad, dann Return
    q(f"type {MUSTER}", "wait 800", "key Return", "wait 1500")
    da = warte_bild(mitte_ist(VIERTEL[3], 10, 10))
    im = bild("a3e-bild-gewaehlt")
    text = open(INI).read()
    pruefe("A3e Rückfall-Dialog nach Type=color: Bild gewählt und gezeigt",
           offen and da and text == ini(None, MUSTER, "centered"), repr(text))
    vergleiche("A3e gewähltes Bild zentriert", im, "centered", 1, muster(300, 200))

    # wieder ohne Sonder-Umgebung
    mit_umgebung(None, os.environ["HOME"])


def bild_jetzt():
    pfad = os.path.join(LOGS, "jetzt.png")
    subprocess.run(["grim", pfad], check=False)
    return Image.open(pfad).convert("RGB")


def test_q11_neustart():
    """Einstellung bleibt über einen Neustart (Kacheln) – liest desktop.ini beim Start."""
    lade(ini(None, MUSTER, "tiled"))
    d.stop()
    d.start(lambda l: len(l) >= 8)
    time.sleep(1)
    gekachelt("Q11 nach Neustart Mode=tiled", muster(300, 200), MUSTER)
    lade(ini("color", MUSTER, "tiled"))
    d.stop()
    d.start(lambda l: len(l) >= 8)
    time.sleep(1)
    im = bild("q11-farbe-nach-neustart")
    vergleiche("Q11 nach Neustart Type=color", im, "farbe")
    pruefe("Q11 nach Neustart: nur Farbe geladen", "Hintergrund nur Farbe" in d.text() and "Modus" not in
           "\n".join(z for z in d.text().splitlines() if "Hintergrund" in z), "")


def main():
    os.makedirs(t.BILDER, exist_ok=True)
    t.protokoll(f"QE-01 Desktop {t.DESKTOP}")
    muster(300, 200).save(MUSTER)
    muster(1600, 1200).save(GROSS)
    shutil.copy(INI, os.path.join(LOGS, "desktop-ini-start.txt"))
    panel = subprocess.Popen([t.PANEL], stdout=open(os.path.join(LOGS, "panel-1.log"), "w"),
                             stderr=subprocess.STDOUT)
    time.sleep(3)
    t.q = t.Eingabe()
    q("move 640 700")
    d.start(lambda l: len(l) >= 8)
    im = bild("start")
    t.oben[0] = t.miss_panel(im)
    tests = [test_q10_faehigkeiten, test_q01_farbe, test_q02_farbe_ohne_datei, test_q03_zentriert, test_q04_gross,
             test_q05_gekachelt, test_q06_wechsel, test_q08_unbekannter_typ, test_q09_alte_modi,
             test_q07_aufloesungen, test_q11_neustart, test_a3]
    for test in tests:
        name = getattr(test, "__name__", "test")
        t.protokoll(f"=== {name}")
        try:
            test()
        except Exception as err:  # noqa: BLE001
            pruefe(f"{name} ohne Ausnahme", False, f"{err!r}\n{traceback.format_exc()}")
        q("key Escape", "wait 200")
    d.stop()
    panel.terminate()
    ok = sum(1 for _, o, _ in t.ergebnisse if o)
    with open(os.path.join(LOGS, "pruefung.txt"), "w") as f:
        for n, o, det in t.ergebnisse:
            f.write(f"{'OK    ' if o else 'FEHLER'} {n}{'  – ' + det if det and not o else ''}\n")
        f.write(f"{ok}/{len(t.ergebnisse)} Prüfungen bestanden\n")
    t.protokoll(f"{ok}/{len(t.ergebnisse)} Prüfungen bestanden")


if __name__ == "__main__":
    main()
