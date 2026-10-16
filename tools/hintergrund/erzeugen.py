#!/usr/bin/env python3
"""HMP-08: erzeugt das Querencia-Standardhintergrundbild „Querencia Blau“.

Eigenes, reproduzierbares Bild (kein fremdes Material): tiefes klassisches
Blau als senkrechter Verlauf, ein weiches Licht oben links und drei sanfte,
geschwungene Lichtbänder, wie es zu GNOME-2-Desktops passt. Keine Schrift,
keine Logos. Ein leichtes festes Rauschen (Startwert 2026) verhindert
Farbstufen im Verlauf.

Aufruf: erzeugen.py <ziel.png> [breite höhe]   (Standard 2560 x 1440)
Benötigt nur Python 3 und Pillow.
"""
import math
import random
import sys

from PIL import Image

TOP = (10, 36, 84)        # tiefes Blau oben
BOTTOM = (28, 78, 150)    # klassisches GNOME-2-Blau unten
GLOW = (120, 170, 235)    # weiches Licht
BAND = (150, 195, 245)    # Lichtbänder


def mix(a, b, t):
    return tuple(a[i] + (b[i] - a[i]) * t for i in range(3))


def render(width, height):
    rnd = random.Random(2026)
    noise = [rnd.uniform(-1.2, 1.2) for _ in range(4096)]
    # bands: (relative height, amplitude, wavelength, phase, thickness, strength)
    bands = [
        (0.62, 0.09, 1.25, 0.4, 0.060, 0.22),
        (0.70, 0.07, 0.95, 1.7, 0.035, 0.16),
        (0.78, 0.05, 1.60, 2.9, 0.025, 0.12),
    ]
    img = Image.new("RGB", (width, height))
    px = img.load()
    for y in range(height):
        v = y / (height - 1)
        base = mix(TOP, BOTTOM, v ** 0.9)
        for x in range(width):
            u = x / (width - 1)
            # soft glow, upper left
            d = math.hypot((u - 0.22) * 1.6, (v - 0.18) * 1.0)
            glow = 0.28 * math.exp(-(d * d) / 0.10)
            c = mix(base, GLOW, glow)
            # gentle bands across the lower half
            for (h, amp, wave, phase, thick, strength) in bands:
                centre = h + amp * math.sin(2 * math.pi * u / wave + phase)
                dist = (v - centre) / thick
                c = mix(c, BAND, strength * math.exp(-dist * dist))
            n = noise[(x * 7919 + y * 104729) % 4096]
            px[x, y] = tuple(max(0, min(255, int(round(ch + n)))) for ch in c)
    return img


def main():
    target = sys.argv[1]
    width, height = (int(sys.argv[2]), int(sys.argv[3])) if len(sys.argv) > 3 else (2560, 1440)
    render(width, height).save(target, optimize=True)
    print(f"{target}: {width}x{height}")


if __name__ == "__main__":
    main()
