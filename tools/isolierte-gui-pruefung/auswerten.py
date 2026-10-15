#!/usr/bin/env python3
"""HMP-05: wertet die Bildschirmfotos der isolierten Menüprüfung aus.

Aufruf: auswerten.py <erwartung.json> <bildordner>

Liest Pixel an festen Stellen des Standardlayouts (1280x720, Panel oben,
Anwendungen | Orte | System links):
- welches Hauptmenü offen ist (linke Kante bei x 12 / 170 / 240),
- welche Zeile des Anwendungsmenüs orange markiert ist (y 46/70/94, x 120
  oder 188: neben der weißen Schrift),
- welches Untermenü rechts daneben offen ist (Zeilenzahl in Spalte x 270:
  Zubehör 5, Internet 1, Systemwerkzeuge 4),
- ob eine Untermenüzeile markiert ist,
- dass kein Hauptmenüknopf orange ist oder blau umrahmt (Fokusring),
- dass die normale Schrift im Untermenü dunkel ist (keine weiße Schrift).
Gibt je Bild OK/FEHLER aus; Exitcode 0 nur, wenn alles stimmt.
"""
import json
import sys

from PIL import Image

ROWS = {"Zubehör": 46, "Internet": 70, "Systemwerkzeuge": 94}
SUB_ROWS = {5: "Zubehör", 1: "Internet", 4: "Systemwerkzeuge"}


def black(p):
    return sum(p) < 40


def orange(p):
    r, g, b = p
    return r > 200 and 90 < g < 180 and b < 130


def blue(p):
    r, g, b = p
    return b > 180 and r < 170 and b - r > 50


def menu_open(im):
    if not black(im.getpixel((12, 60))):
        return "anwendungen"
    if not black(im.getpixel((170, 60))):
        return "orte"
    if not black(im.getpixel((240, 60))):
        return "system"
    return "keins"


def submenu(im):
    """(top, bottom) of the light run in column x=270 below the panel."""
    run = None
    for y in range(33, 600):
        if not black(im.getpixel((270, y))):
            run = (run[0], y) if run else (y, y)
        elif run:
            break
    return run


def check(name, want, im):
    errors = []
    got_menu = menu_open(im)
    if got_menu != want["menu"]:
        errors.append(f"Menü {got_menu}, erwartet {want['menu']}")
    # main menu buttons never orange, and no blue GTK focus ring
    for x in (40, 90, 130, 175, 190, 245, 265):
        for y in (4, 15, 26):
            if orange(im.getpixel((x, y))):
                errors.append(f"Hauptmenüknopf orange bei {x},{y}")
                break
    for x in range(8, 300):
        for y in (1, 2, 28, 29, 30):
            if blue(im.getpixel((x, y))):
                errors.append(f"blauer Fokusrahmen bei {x},{y}")
                break
        else:
            continue
        break
    if got_menu == "anwendungen":
        # orange area left and right of the label (the label itself is white)
        lit = [k for k, y in ROWS.items() if orange(im.getpixel((120, y))) or orange(im.getpixel((188, y)))]
        if "aktiv" in want:
            expect = [] if want["aktiv"] == "-" else [want["aktiv"]]
            if lit != expect:
                errors.append(f"markiert {lit or '-'}, erwartet {expect or '-'}")
        run = submenu(im)
        got_sub = "-"
        if run and run[1] - run[0] > 10:
            rows = round((run[1] - run[0] - 6) / 24)
            got_sub = SUB_ROWS.get(rows, f"? ({rows} Zeilen, y {run[0]}-{run[1]})")
            # normal text in the submenu must be dark: darkest pixel of each
            # unhighlighted row
            for y in range(run[0] + 12, run[1] - 6, 24):
                if orange(im.getpixel((330, y))):
                    continue
                darkest = min(sum(im.getpixel((x, yy))) for x in range(222, 330) for yy in range(y - 6, y + 6))
                if darkest > 300:
                    errors.append(f"Untermenüzeile y {y} ohne dunkle Schrift (hellstes Dunkel {darkest})")
        if "sub" in want and got_sub != want["sub"]:
            errors.append(f"Untermenü {got_sub}, erwartet {want['sub']}")
        if "subzeile" in want:
            if not orange(im.getpixel((330, want["subzeile"]))):
                errors.append(f"Untermenüzeile bei y {want['subzeile']} nicht markiert")
    return errors


def main():
    expectations = json.load(open(sys.argv[1]))
    folder = sys.argv[2]
    failed = 0
    for name, want in expectations:
        im = Image.open(f"{folder}/{name}.png").convert("RGB")
        errors = check(name, want, im)
        print(("OK     " if not errors else "FEHLER ") + name + ("" if not errors else ": " + "; ".join(errors)))
        failed += bool(errors)
    print(f"{len(expectations) - failed}/{len(expectations)} Bilder wie erwartet")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
