#!/usr/bin/env python3
"""HMP-08: wertet die Bilder von szenario-hmp08.txt aus.

Für jedes Bild wird aus dem eingebauten Hintergrund das Sollbild berechnet
(gleiche Platzierungsregel wie config::placement) und an einem Raster von
Punkten verglichen, die weder Panel noch Testfenster sind. Damit fallen
Verzerrung, falsche Ausschnitte, falscher Modus und fehlender Hintergrund
auf. Dazu: Panels liegen über dem Hintergrund, labwcs Wurzelmenü erscheint
beim Rechtsklick (der Hintergrund fängt keine Eingaben), das Testfenster
liegt über dem Hintergrund.

  auswerten-hintergrund.py <bildordner> <hintergrund.png>
"""
import os
import sys

from PIL import Image

PANEL_TOP = 26     # oberes Panel 0..25
PANEL_BOTTOM = 40  # unteres Panel H-36..H


def placement(mode, img, out):
    iw, ih = img
    ow, oh = out
    if mode == "stretched":
        return 0, 0, ow, oh
    scale = max(ow / iw, oh / ih) if mode == "zoom" else min(ow / iw, oh / ih)
    w, h = iw * scale, ih * scale
    return (ow - w) / 2, (oh - h) / 2, w, h


def expected(src, mode, size, color):
    x, y, w, h = placement(mode, src.size, size)
    canvas = Image.new("RGB", size, color)
    scaled = src.resize((max(1, round(w)), max(1, round(h))), Image.BILINEAR)
    canvas.paste(scaled, (round(x), round(y)))
    return canvas


def is_window(p):
    # foot's dark background and its light server-side title bar/borders
    r, g, b = p
    grey = abs(r - g) < 10 and abs(g - b) < 10
    return grey and (r < 70 or r > 150)


def compare(shot, soll):
    w, h = shot.size
    good = bad = 0
    worst = []
    for yi in range(12):
        y = PANEL_TOP + 14 + yi * (h - PANEL_TOP - PANEL_BOTTOM - 28) // 11
        for xi in range(16):
            x = 6 + xi * (w - 12) // 15
            p = shot.getpixel((x, y))[:3]
            if is_window(p):
                continue
            q = soll.getpixel((x, y))
            d = max(abs(a - b) for a, b in zip(p, q))
            if d <= 14:
                good += 1
            else:
                bad += 1
                worst.append((d, (x, y), p, q))
    worst.sort(reverse=True)
    return good, bad, worst[:3]


def light(p):
    return min(p[:3]) > 190


def main():
    folder, wallpaper = sys.argv[1], sys.argv[2]
    src = Image.open(wallpaper).convert("RGB")
    blue = (0x1c, 0x4e, 0x96)
    cases = [
        ("w01-hintergrund-1280x720.png", "zoom", blue, False),
        ("w02-rechtsklick-labwc-menue.png", "zoom", blue, False),
        ("w03-fenster.png", "zoom", blue, True),
        ("w05-1920x1080.png", "zoom", blue, True),
        ("w06-1600x900.png", "zoom", blue, True),
        ("w07-1366x768.png", "zoom", blue, True),
        ("w08-1280x1024-zoom.png", "zoom", blue, True),
        ("w09-1280x1024-scaled.png", "scaled", (0x3a, 0x6e, 0xa5), True),
        ("w10-unlesbar-rueckfall.png", "zoom", blue, True),
        ("w11-1280x720-zurueck.png", "zoom", blue, True),
    ]
    sizes = {"w05": (1920, 1080), "w06": (1600, 900), "w07": (1366, 768),
             "w08": (1280, 1024), "w09": (1280, 1024), "w10": (1280, 1024)}
    results = []

    def check(name, ok, detail):
        results.append(ok)
        print(f"{'OK ' if ok else 'FEHLER'} {name}: {detail}")

    shot = Image.open(os.path.join(folder, "w00-ohne-desktop.png")).convert("RGB")
    check("w00 ohne Desktop", max(shot.getpixel((640, 360))) < 20, f"Mitte {shot.getpixel((640, 360))} (schwarz erwartet)")

    for name, mode, color, window in cases:
        shot = Image.open(os.path.join(folder, name)).convert("RGB")
        w, h = shot.size
        want = sizes.get(name[:3], (1280, 720))
        problems = []
        if (w, h) != want:
            problems.append(f"Größe {w}x{h} statt {want}")
        good, bad, worst = compare(shot, expected(src, mode, (w, h), color))
        if good < 60 or bad > good * 0.05:
            problems.append(f"Hintergrund weicht ab ({good} gleich, {bad} anders, z. B. {worst})")
        # panels above the background: their light bars at the top and bottom
        top = shot.getpixel((w // 2, 12))
        bottom = shot.getpixel((w * 3 // 4, h - 4))
        if not (light(top) and light(bottom)):
            problems.append(f"Panel verdeckt? oben {top}, unten {bottom}")
        if window:
            centre = [shot.getpixel((x, y)) for x in range(w // 2 - 100, w // 2 + 100, 20)
                      for y in range(h // 2 - 60, h // 2 + 60, 20)]
            dark = sum(1 for p in centre if max(p) < 70)
            if dark < len(centre) * 0.6:
                problems.append(f"Testfenster nicht sichtbar ({dark}/{len(centre)} dunkel)")
        if name.startswith("w02"):
            # labwc's root menu at the pointer (900, 450)
            menu = [shot.getpixel((x, y)) for x in range(905, 960, 5) for y in range(455, 490, 5)]
            n = sum(1 for p in menu if light(p))
            if n < len(menu) * 0.5:
                problems.append(f"kein labwc-Menü am Zeiger ({n}/{len(menu)} hell)")
        if name.startswith("w09"):
            bar = shot.getpixel((40, 100))
            if max(abs(a - b) for a, b in zip(bar, color)) > 6:
                problems.append(f"Rand im Modus scaled {bar} statt {color}")
        check(name, not problems, "; ".join(problems) or f"{good} Punkte wie Sollbild, {bad} abweichend")
    # The surface covers the whole output (behind the panels as well), at
    # every resolution: the desktop logs each new size of its surface.
    log = open(os.path.join(folder, "desktop.log"), encoding="utf-8", errors="replace").read()
    sizes_seen = {line.split("Fläche ")[1].strip() for line in log.splitlines() if "Fläche " in line}
    want = {"1280x720", "1920x1080", "1600x900", "1366x768", "1280x1024"}
    check("Flächengröße = Ausgang", want <= sizes_seen and sizes_seen <= want,
          f"gemeldet {sorted(sizes_seen)}, erwartet {sorted(want)}")
    print(f"ERGEBNIS {sum(results)}/{len(results)}")
    return 0 if all(results) else 1


if __name__ == "__main__":
    sys.exit(main())
