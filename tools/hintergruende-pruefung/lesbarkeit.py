#!/usr/bin/env python3
"""G2-AF: Lesbarkeit weißer Symbolbeschriftungen am linken Rand.

Für jedes Bild in data/hintergruende und jede Bildschirmgröße wird das Bild
wie vom Desktop bei „Bildschirm ausfüllen“ skaliert und beschnitten. In der
Symbolspalte (x 0–120, unter dem oberen und über dem unteren Panel, je 32 px)
wird die relative Leuchtdichte nach WCAG 2 gemessen; ausgegeben wird der
Kontrast von Weiß gegen den hellsten Hintergrund (95. Perzentil) und gegen
den Mittelwert. Der Schatten der Beschriftung (1 px, 85 % Schwarz) bleibt
unberücksichtigt, die Werte sind also die ungünstige Seite.

Aufruf: python3 -I lesbarkeit.py [ordner] > tabelle.tsv
Exit 1, wenn ein Bild bei 4,5:1 (WCAG AA, Fließtext) im 95. Perzentil
durchfällt.
"""
import sys
from pathlib import Path

import gi

gi.require_version("GdkPixbuf", "2.0")
from gi.repository import GdkPixbuf  # noqa: E402

GROESSEN = [(1024, 768), (1920, 1080), (2560, 1440)]
SPALTE, PANEL = 120, 32


def fassung(ordner, name, b, h):
    """Wie Querencia Einstellungen: unter 1,555 die 4:3-Datei, falls vorhanden."""
    if b / h < 1.555:
        vier = ordner / name.replace(".png", "-4x3.png")
        if vier.is_file():
            return vier
    return ordner / name


def lin(c):
    c /= 255
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def leuchtdichten(datei, b, h):
    pb = GdkPixbuf.Pixbuf.new_from_file(str(datei))
    iw, ih = pb.get_width(), pb.get_height()
    s = max(b / iw, h / ih)
    sw, sh = round(iw * s), round(ih * s)
    pb = pb.scale_simple(sw, sh, GdkPixbuf.InterpType.BILINEAR)
    x0, y0 = (sw - b) // 2, (sh - h) // 2
    rs, n, px = pb.get_rowstride(), pb.get_n_channels(), pb.get_pixels()
    tabelle = [lin(i) for i in range(256)]
    werte = []
    for y in range(y0 + PANEL, y0 + h - PANEL, 2):
        for x in range(x0, x0 + SPALTE, 2):
            i = y * rs + x * n
            werte.append(0.2126 * tabelle[px[i]] + 0.7152 * tabelle[px[i + 1]]
                         + 0.0722 * tabelle[px[i + 2]])
    werte.sort()
    return werte[int(len(werte) * 0.95)], sum(werte) / len(werte)


def kontrast(l):
    return 1.05 / (l + 0.05)


def main():
    ordner = Path(sys.argv[1]) if len(sys.argv) > 1 else \
        Path(__file__).resolve().parents[2] / "data" / "hintergruende"
    namen = sorted(p.name for p in ordner.glob("querencia-*.png") if "-4x3" not in p.name)
    print("bild\tbildschirm\tdatei\tkontrast_p95\tkontrast_mittel\turteil")
    durch = 0
    for name in namen:
        for b, h in GROESSEN:
            datei = fassung(ordner, name, b, h)
            p95, mittel = leuchtdichten(datei, b, h)
            k95, km = kontrast(p95), kontrast(mittel)
            urteil = "PASS" if k95 >= 4.5 else "FAIL"
            durch += urteil == "FAIL"
            print(f"{name}\t{b}x{h}\t{datei.name}\t{k95:.1f}\t{km:.1f}\t{urteil}")
    return 1 if durch else 0


if __name__ == "__main__":
    sys.exit(main())
