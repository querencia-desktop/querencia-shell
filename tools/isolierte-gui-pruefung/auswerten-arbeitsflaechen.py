#!/usr/bin/env python3
"""HMP-07: wertet Bildschirmfotos des Arbeitsflächenumschalters aus.

Aufruf: auswerten-arbeitsflaechen.py <erwartung.json> <bildordner>

Sucht rechts im unteren Panel (1280x720, Zeile y=704) den Rahmen des
Umschalters und zählt die Zellen (Trennlinien #faf1e8). Aktive Zelle: hat
dunkelorangen Rand (#a5522d), cremefarbene Linie und lachsfarbenen Kern.
Prüft Anzahl, aktive Zelle, Zellbreiten (gleich, Seitenverhältnis wie der
Bildschirm 16:9 bei 22 px Höhe: 39 px), keine Ziffern in den Zellen.
"""
import json
import sys

from PIL import Image

Y = 704


def near(p, q, tol=24):
    return all(abs(a - b) <= tol for a, b in zip(p, q))


FRAME = (0xcd, 0xc6, 0xbe)
LINE = (0xfa, 0xf1, 0xe8)
TAUPE = (0xb7, 0xab, 0x9f)
DARK = (0xa5, 0x52, 0x2d)
CREAM = (0xff, 0xf2, 0xda)
SALMON = (0xfe, 0xb3, 0x96)


def pager(im):
    row = [im.getpixel((x, Y)) for x in range(im.size[0])]
    # rightmost frame pixel, then walk left over the cells to the left frame
    right = max(x for x in range(1279, 900, -1) if near(row[x], FRAME, 12) or near(row[x], TAUPE, 12) or near(row[x], DARK) or near(row[x], CREAM, 12))
    left = right
    while left > 900 and not near(row[left - 1], (0xf6, 0xf5, 0xf4), 3):
        left -= 1
    return left, right, row


def cells(im):
    left, right, row = pager(im)
    inner = list(range(left + 1, right))
    bounds, start = [], inner[0]
    for x in inner:
        if near(row[x], LINE, 6) and not near(row[x - 1], CREAM, 10) and not near(row[x + 1], CREAM, 10):
            bounds.append((start, x - 1))
            start = x + 1
    bounds.append((start, inner[-1]))
    return [(a, b) for a, b in bounds if b - a > 6]


def is_active(im, a, b):
    mid = (a + b) // 2
    col = [im.getpixel((mid, y)) for y in range(692, 718)]
    return any(near(p, DARK, 30) for p in col) and any(near(p, SALMON, 20) for p in col) and any(near(p, CREAM, 12) for p in col)


def check(want, im):
    errors = []
    found = cells(im)
    widths = [b - a + 1 for a, b in found]
    active = [n for n, (a, b) in enumerate(found) if is_active(im, a, b)]
    if len(found) != want["count"]:
        errors.append(f"{len(found)} Zellen, erwartet {want['count']}")
    if active != [want["active"]]:
        errors.append(f"aktiv {active}, erwartet [{want['active']}]")
    if widths and (max(widths) - min(widths) > 2 or not 36 <= max(widths) <= 42):
        errors.append(f"Zellbreiten {widths}")
    for n, (a, b) in enumerate(found):
        if n in active:
            continue
        dark = [im.getpixel((x, y)) for x in range(a + 2, b - 1) for y in range(696, 713)]
        if any(sum(p) < 300 for p in dark):
            errors.append(f"Zelle {n} enthält Schrift/Ziffer")
    return errors, widths


def main():
    failed = 0
    expectations = json.load(open(sys.argv[1]))
    for name, want in expectations:
        im = Image.open(f"{sys.argv[2]}/{name}.png").convert("RGB")
        errors, widths = check(want, im)
        print(("OK     " if not errors else "FEHLER ") + f"{name}: Breiten {widths}" + ("" if not errors else " – " + "; ".join(errors)))
        failed += bool(errors)
    print(f"{len(expectations) - failed}/{len(expectations)} Bilder wie erwartet")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
