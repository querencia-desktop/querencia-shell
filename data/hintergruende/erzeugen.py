#!/usr/bin/env python3
"""G2-AF (QD-058): erzeugt die eingebauten Hintergründe von Querencia Desktop.

Vier Motive (Abend, Blau 2, Erde, Nacht), je 16:9 (2560×1440) und 4:3
(1600×1200), je ohne und mit Q-Logo: 16 Dateien. „Querencia Blau“ entsteht
nicht hier (HMP-08, `tools/hintergrund/erzeugen.py`) und wird nur geprüft.

Quellen sind die von pottrauschen erzeugten und sortierten Rohbilder (Ordner
`querencia-hintergruende-roh`, siehe HERKUNFT.md); jede Quelle wird über ihre
SHA256 erkannt. Die Fassungen ohne Logo sind byte-gleiche Kopien der Quellen,
mit einer Ausnahme: „Abend“ 16:9 wird aus dem logofreien ChatGPT-Original
(1672×941) auf 2560×1440 hochskaliert, weil die zuerst sortierte Datei
`querencia-abend.png` ein von der KI gezeichnetes Q enthielt (inzwischen im
Rohordner berichtigt; die eigene Hochrechnung bleibt, siehe HERKUNFT.md). Die 4:3-Dateien
sind eigene Bildausschnitte der KI, nie gestauchte 16:9-Bilder.

Das Q-Logo wird aus der SVG-Vorlage `logo/querencia-logo.svg` aufgeprägt
(nicht aus den KI-Fassungen): sichtbare Breite 1/8 der Bildhöhe, unten
rechts innerhalb des Bereichs, der bei „Bildschirm ausfüllen“ auf allen
Seitenverhältnissen sichtbar bleibt, für die die Datei gedacht ist
(16:9-Datei: 16:10 und 16:9; 4:3-Datei: 5:4, 4:3 und 3:2). Prägung: Schatten
unten rechts, Licht oben links, Fläche in Querencia-Orange, alles schwach
deckend.

Aufruf (nur python3 und GdkPixbuf über python3-gi):
  python3 -I erzeugen.py [--roh ORDNER] [--ziel ORDNER]
  python3 -I erzeugen.py --pruefen [--roh ORDNER]
`--ziel` ist standardmäßig der Ordner dieses Skripts; `--pruefen` erzeugt in
einen temporären Ordner und vergleicht mit SHA256SUMS (Exit 0 = gleich).
"""
import argparse
import hashlib
import shutil
import sys
import tempfile
from pathlib import Path

import gi

gi.require_version("GdkPixbuf", "2.0")
from gi.repository import GdkPixbuf, GLib  # noqa: E402

HIER = Path(__file__).resolve().parent
LOGO = HIER / "logo" / "querencia-logo.svg"
LOGO_SHA256 = "a17469438cb58db7ed6163547376c21e70349bf17c417a280f0a6fcb1a2b86f4"
ROH_STANDARD = Path.home() / "Bilder" / "querencia-hintergruende-roh"

GROESSE = {"16x9": (2560, 1440), "4x3": (1600, 1200)}
# Seitenverhältnisse, für die eine Datei gedacht ist (Wahl in Querencia
# Einstellungen: unter 1,555 die 4:3-Datei, sonst die 16:9-Datei).
VERHAELTNISSE = {"16x9": (16 / 10, 16 / 9), "4x3": (5 / 4, 4 / 3, 3 / 2)}

# Motiv -> Format -> (Quelle relativ zum Rohordner, SHA256 der Quelle)
QUELLEN = {
    "abend": {
        "16x9": ("chatgpt-original/Copper Ribbons in Plum Shadows.png",
                 "a4bf6a11461add9881e5dc087220d854752c22e4cfcf492d16dc71b8c65e7484"),
        "4x3": ("querencia-abend-4x3.png",
                "dc686546bb21d3c2da8b0ca1de68a707d1a5290ce563beca8f6b7b80e48279c8"),
    },
    "blau2": {
        "16x9": ("querencia-blau2.png",
                 "fc2b8085b9a2626003e423991cef8f4f7e8326371a7f25cdc58ffb451dd27a7e"),
        "4x3": ("querencia-blau2-4x3.png",
                "fd9e33e293495e76776c1c647f5a9dede84dd1e226b3eea2fbe1b73a9a8b0bc6"),
    },
    "erde": {
        "16x9": ("querencia-erde.png",
                 "d1d00e2c49a92dd083db89a1ded0d31afaadb31fdc9ae1cff5b77f760c3faef0"),
        "4x3": ("querencia-erde-4x3.png",
                "728b0105326df7481f33aad5ba0834d89ea3493cbda453857acdbba753c34689"),
    },
    "nacht": {
        "16x9": ("querencia-nacht.png",
                 "11f112f17d58f9f189598656d5f0ac7c8b42c48833c9714d1983e9ffb859ab88"),
        "4x3": ("querencia-nacht-4x3.png",
                "b8c9e17be1e1b0cd7e19600b00c4a6e92862663eded8bfaa1d06011a2a683a84"),
    },
}

# Logo: Breite und Abstände als Anteil der Bildhöhe; Deckkraft je Schicht.
LOGO_BREITE = 0.125
RAND_RECHTS = 0.052
RAND_UNTEN = 0.069
DECKKRAFT = {"schatten": 0.20, "licht": 0.07, "flaeche": 0.10}


def sha256(pfad):
    h = hashlib.sha256()
    with open(pfad, "rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def dateiname(motiv, logo, fmt):
    return f"querencia-{motiv}{'-logo' if logo else ''}{'-4x3' if fmt == '4x3' else ''}.png"


def sichtbar(fmt):
    """Bereich (x0, y0, x1, y1), der bei „Bildschirm ausfüllen“ auf jedem
    Seitenverhältnis der Datei sichtbar bleibt."""
    b, h = GROESSE[fmt]
    x0, y0, x1, y1 = 0.0, 0.0, float(b), float(h)
    for v in VERHAELTNISSE[fmt]:
        if v < b / h:
            rand = (b - h * v) / 2
            x0, x1 = max(x0, rand), min(x1, b - rand)
        else:
            rand = (h - b / v) / 2
            y0, y1 = max(y0, rand), min(y1, h - rand)
    return x0, y0, x1, y1


def alpha_rahmen(pb):
    """Kleinstes Rechteck (x0, y0, x1, y1) mit Alpha > 0."""
    b, h, rs = pb.get_width(), pb.get_height(), pb.get_rowstride()
    px = pb.get_pixels()
    xs, ys = [], []
    for y in range(h):
        zeile = px[y * rs: y * rs + b * 4]
        for x in range(b):
            if zeile[x * 4 + 3]:
                xs.append(x)
                ys.append(y)
    return min(xs), min(ys), max(xs) + 1, max(ys) + 1


def logo_pixbuf(breite):
    """Das Logo als RGBA-Pixbuf, zugeschnitten auf die Form, Formbreite `breite`."""
    probe = GdkPixbuf.Pixbuf.new_from_file_at_scale(str(LOGO), 1024, 1024, True)
    x0, _, x1, _ = alpha_rahmen(probe)
    seite = round(1024 * breite / (x1 - x0))
    pb = GdkPixbuf.Pixbuf.new_from_file_at_scale(str(LOGO), seite, seite, True)
    x0, y0, x1, y1 = alpha_rahmen(pb)
    return pb.new_subpixbuf(x0, y0, x1 - x0, y1 - y0).copy()


def eingefaerbt(pb, rgb):
    """Kopie mit gleicher Deckung, aber einheitlicher Farbe."""
    b, h, rs = pb.get_width(), pb.get_height(), pb.get_rowstride()
    px = bytearray(pb.get_pixels())
    for y in range(h):
        for x in range(b):
            i = y * rs + x * 4
            px[i:i + 3] = bytes(rgb)
    return GdkPixbuf.Pixbuf.new_from_bytes(
        GLib.Bytes.new(bytes(px)), GdkPixbuf.Colorspace.RGB, True, 8, b, h, rs)


def aufpraegen(bild, fmt):
    b, h = GROESSE[fmt]
    logo = logo_pixbuf(round(LOGO_BREITE * h))
    lb, lh = logo.get_width(), logo.get_height()
    _, _, sx1, sy1 = sichtbar(fmt)
    x = round(sx1 - RAND_RECHTS * h) - lb
    y = round(sy1 - RAND_UNTEN * h) - lh
    d = max(1, round(h / 720))
    schichten = [
        (eingefaerbt(logo, (0, 0, 0)), d, d, DECKKRAFT["schatten"]),
        (eingefaerbt(logo, (255, 255, 255)), -d, -d, DECKKRAFT["licht"]),
        (logo, 0, 0, DECKKRAFT["flaeche"]),
    ]
    for quelle, dx, dy, deck in schichten:
        quelle.composite(bild, x + dx, y + dy, lb, lh, x + dx, y + dy, 1.0, 1.0,
                         GdkPixbuf.InterpType.NEAREST, round(255 * deck))
    return (x, y, lb, lh)


def speichern(pb, ziel):
    pb.savev(str(ziel), "png", ["compression"], ["9"])


def quelle_pruefen(roh, rel, soll):
    pfad = roh / rel
    if not pfad.is_file():
        sys.exit(f"Quelle fehlt: {pfad}")
    ist = sha256(pfad)
    if ist != soll:
        sys.exit(f"Quelle {pfad}: SHA256 {ist} statt {soll}")
    return pfad


def erzeugen(roh, ziel):
    if sha256(LOGO) != LOGO_SHA256:
        sys.exit(f"Logo {LOGO}: SHA256 weicht ab")
    ziel.mkdir(parents=True, exist_ok=True)
    protokoll = []
    for motiv, formate in QUELLEN.items():
        for fmt, (rel, soll) in formate.items():
            quelle = quelle_pruefen(roh, rel, soll)
            b, h = GROESSE[fmt]
            ohne = ziel / dateiname(motiv, False, fmt)
            pb = GdkPixbuf.Pixbuf.new_from_file(str(quelle))
            if (pb.get_width(), pb.get_height()) == (b, h) and not pb.get_has_alpha():
                shutil.copyfile(quelle, ohne)
                art = "Kopie"
            else:
                pb = pb.scale_simple(b, h, GdkPixbuf.InterpType.HYPER)
                speichern(pb, ohne)
                art = f"skaliert {GdkPixbuf.Pixbuf.new_from_file(str(quelle)).get_width()}→{b}"
            protokoll.append(f"{ohne.name}: {art} aus {rel}")
            mit = GdkPixbuf.Pixbuf.new_from_file(str(ohne))
            x, y, lb, lh = aufpraegen(mit, fmt)
            speichern(mit, ziel / dateiname(motiv, True, fmt))
            protokoll.append(
                f"{dateiname(motiv, True, fmt)}: Logo {lb}×{lh} bei ({x}, {y})")
    return protokoll


def summen_lesen(datei):
    soll = {}
    for zeile in datei.read_text().splitlines():
        if zeile.strip():
            wert, name = zeile.split(maxsplit=1)
            soll[name.lstrip("*")] = wert
    return soll


def main():
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument("--roh", type=Path, default=ROH_STANDARD)
    p.add_argument("--ziel", type=Path, default=HIER)
    p.add_argument("--pruefen", action="store_true")
    a = p.parse_args()
    if not a.pruefen:
        for zeile in erzeugen(a.roh, a.ziel):
            print(zeile)
        return 0
    soll = summen_lesen(HIER / "SHA256SUMS")
    fehler = 0
    with tempfile.TemporaryDirectory(prefix="querencia-hintergruende-") as tmp:
        erzeugen(a.roh, Path(tmp))
        for neu in sorted(Path(tmp).iterdir()):
            ist = sha256(neu)
            ok = soll.get(neu.name) == ist and sha256(HIER / neu.name) == ist
            fehler += not ok
            print(f"{'OK  ' if ok else 'FAIL'} {neu.name} {ist[:16]}")
    for name in ("querencia-blau.png",):
        ok = soll.get(name) == sha256(HIER / name)
        fehler += not ok
        print(f"{'OK  ' if ok else 'FAIL'} {name} (HMP-08, nur geprüft)")
    print(f"{'PASS' if not fehler else 'FAIL'}: {fehler} Abweichung(en)")
    return 1 if fehler else 0


if __name__ == "__main__":
    sys.exit(main())
