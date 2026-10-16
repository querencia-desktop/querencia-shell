#!/usr/bin/env python3
"""HMP-06: wertet Bildschirmfotos der Fensterliste (unteres Panel) aus.

Aufruf: auswerten-fensterliste.py <erwartung.json> <bildordner>

Liest die Knöpfe der Fensterliste aus den Pixeln des unteren Panels
(1280x720, Panel 32 px hoch ab y 688, Knöpfe y 690..717): Rahmenspalten in
Zeile y=692 trennen die Knöpfe. Je Knopf: Breite, ob gedrückt (aktiv: dunkler
Verlauf oben), ob minimiert (flacher heller Grund oben wie unten), ob ein
Symbol links sitzt (mindestens 25 Pixel weichen im Symbolfeld deutlich vom
Knopfgrund ab). Prüft gegen
die Erwartung: Anzahl, aktiver Knopf, minimierte Knöpfe, gleiche Breiten,
höchstens 200 px, alle Knöpfe mit Symbol, nichts orange.
"""
import json
import sys

from PIL import Image

TOP, MID, LOW = 692, 703, 715


def lum(p):
    return sum(p)


def is_border(p):
    return lum(p) < 620 and max(p) - min(p) < 30


def border_runs(im, xs):
    runs, start, prev = [], None, None
    for x in xs:
        if not is_border(im.getpixel((x, LOW - 2))):
            continue
        if start is None:
            start = prev = x
        elif abs(x - prev) == 1:
            prev = x
        else:
            runs.append((min(start, prev), max(start, prev)))
            start = prev = x
    if start is not None:
        runs.append((min(start, prev), max(start, prev)))
    return runs


def switcher_left(im):
    """Left edge of the workspace switcher: the second border from the right."""
    runs = border_runs(im, range(1279, 900, -1))
    return runs[1][0] if len(runs) > 1 else 1280


def buttons(im):
    """(left, right) of each window button, left of the workspace switcher."""
    edges = [x for x in range(0, switcher_left(im)) if is_border(im.getpixel((x, LOW - 2)))]
    runs, start, prev = [], None, None
    for x in edges:
        if start is None:
            start = prev = x
        elif x == prev + 1:
            prev = x
        else:
            runs.append((start, prev))
            start = prev = x
    if start is not None:
        runs.append((start, prev))
    found = []
    for (_, a), (b, _) in zip(runs, runs[1:]):
        if b - a > 12 and lum(im.getpixel(((a + b) // 2, LOW - 2))) > 620:
            found.append((a + 1, b - 1))
    return found


def orange(p):
    r, g, b = p
    return r > 200 and 90 < g < 180 and b < 130


def describe(im, left, right):
    mid = (left + right) // 2
    top = lum(im.getpixel((mid, TOP + 1)))
    low = lum(im.getpixel((mid, LOW)))
    # icon box left of the title; reference: the button's own ground
    # in the same row, left of the icon
    marked = 0
    for y in range(MID - 7, MID + 8):
        ground = lum(im.getpixel((left + 1, y)))
        marked += sum(abs(lum(im.getpixel((x, y))) - ground) > 40 for x in range(left + 4, left + 22))
    icon_dark = 0 if marked >= 25 else 1000
    return {
        "breite": right - left + 1,
        "aktiv": top < 700,
        "minimiert": low > 734 and abs(top - low) < 8,
        "symbol": icon_dark < 450,
        "symbol_blass": False,
        "orange": any(orange(im.getpixel((x, y))) for x in range(left, right, 3) for y in (TOP, LOW)),
    }


def check(want, im):
    errors = []
    found = buttons(im)
    info = [describe(im, a, b) for a, b in found]
    if "count" in want and len(found) != want["count"]:
        errors.append(f"{len(found)} Knöpfe, erwartet {want['count']}")
    widths = [i["breite"] for i in info]
    if widths and max(widths) - min(widths) > 3:
        errors.append(f"ungleich breit: {widths}")
    if widths and max(widths) > 202:
        errors.append(f"breiter als 200 px: {max(widths)}")
    for n, i in enumerate(info):
        if not (i["symbol"] or (i["minimiert"] and i["symbol_blass"])):
            errors.append(f"Knopf {n} ohne Symbol")
        if i["orange"]:
            errors.append(f"Knopf {n} orange")
    active = [n for n, i in enumerate(info) if i["aktiv"]]
    if "active" in want and active != [want["active"]]:
        errors.append(f"aktiv {active}, erwartet [{want['active']}]")
    for n in want.get("not_active", []):
        if n in active:
            errors.append(f"Knopf {n} aktiv, erwartet nicht")
    if "minimized" in want:
        mins = [n for n, i in enumerate(info) if i["minimiert"]]
        if mins != want["minimized"]:
            errors.append(f"minimiert {mins}, erwartet {want['minimized']}")
    return errors, widths


def main():
    expectations = json.load(open(sys.argv[1]))
    failed = 0
    for name, want in expectations:
        im = Image.open(f"{sys.argv[2]}/{name}.png").convert("RGB")
        errors, widths = check(want, im)
        state = "OK     " if not errors else "FEHLER "
        print(f"{state}{name}: Breiten {widths}" + ("" if not errors else " – " + "; ".join(errors)))
        failed += bool(errors)
    print(f"{len(expectations) - failed}/{len(expectations)} Bilder wie erwartet")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
