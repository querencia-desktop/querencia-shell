#!/usr/bin/env python3
"""HMP-09: wertet die isolierte Prüfung der Desktop-Symbole aus
(szenario-hmp09.txt, 1280x720, oberes und unteres Panel je 32 px).

  auswerten-desktop.py <bildordner>

Bilder werden gegen h01-start (nur Hintergrund, Symbole, Panels) verglichen:
Fenster, Menüs und Dialoge ändern dort die Pixel, die Auswahl färbt die
Beschriftung orange. Dazu Protokoll (desktop.log) und Dateien (dateien.txt):
nichts überschrieben, Reihenfolge nach dem Aufräumen, desktop.ini.
"""
import os
import re
import sys

from PIL import Image

# Symbolraster: Spalte 0 bei x 8..108 (Mitte 58), Zeilen ab y = 32 + 8, 88 px hoch
COL_X = 58
ROW_Y = [32 + 8 + 88 * r for r in range(7)]
ORANGE = (0xf0, 0x77, 0x46)
MENU_BG = (0xf5, 0xf4, 0xf2)
PANEL_BG = (0xf6, 0xf5, 0xf4)


def near(p, q, tol=30):
    return all(abs(a - b) <= tol for a, b in zip(p, q))


def label_selected(im, row, col=0):
    """Orange label background under icon (col,row)."""
    x0 = 8 + 100 * col
    y0 = ROW_Y[row] + 52
    hits = sum(near(im.getpixel((x, y)), ORANGE, 40)
               for x in range(x0 + 4, x0 + 96) for y in range(y0, y0 + 32))
    return hits > 40


def differs(a, b, box, share=0.5):
    """More than `share` of the pixels in box differ clearly between a and b."""
    x0, y0, x1, y1 = box
    n = d = 0
    for x in range(x0, x1, 4):
        for y in range(y0, y1, 4):
            n += 1
            d += not near(a.getpixel((x, y)), b.getpixel((x, y)), 24)
    return d / n > share


def light_share(im, box, colour=MENU_BG, tol=8):
    """Share of the pixels in box near `colour` (menu background)."""
    x0, y0, x1, y1 = box
    pts = [im.getpixel((x, y)) for x in range(x0, x1, 3) for y in range(y0, y1, 3)]
    return sum(1 for p in pts if near(p, colour, tol)) / len(pts)


def main():
    folder = sys.argv[1]
    img = lambda n: Image.open(os.path.join(folder, n + ".png")).convert("RGB")
    log = re.sub(r"\x1b\[[0-9;]*m", "", open(os.path.join(folder, "desktop.log"), errors="replace").read())
    files = open(os.path.join(folder, "dateien.txt")).read()
    base = img("h01-start")
    checks = []

    def check(name, ok, detail=""):
        checks.append((name, bool(ok), detail))

    # H0: icons, panels free
    check("h01 sechs Symbole laut Protokoll",
          "6 Symbole: Rechner, Persönlicher Ordner, Papierkorb, Querencia Terminal, Apfel, Zebra.txt" in log)
    check("h01 Panels frei: oberes Panel über dem ersten Symbol",
          all(near(base.getpixel((x, 20)), PANEL_BG, 12) for x in (30, 58, 90))
          and not near(base.getpixel((58, 60)), PANEL_BG, 12))
    check("h01 keine Auswahl", not any(label_selected(base, r) for r in range(6)))
    # H1-H3: selection
    a, b, c = img("h02-auswahl-rechner"), img("h03-auswahl-papierkorb"), img("h04-keine-auswahl")
    check("h02 Einzelklick wählt Rechner", label_selected(a, 0) and not any(label_selected(a, r) for r in range(1, 6)))
    check("h03 Auswahl wechselt auf Papierkorb", label_selected(b, 2) and not label_selected(b, 0))
    check("h04 Klick auf freie Fläche hebt Auswahl auf", not any(label_selected(c, r) for r in range(6)))
    check("h02-h04 Einzelklick öffnet nichts", "geöffnet" not in log.split("geöffnet: Persönlicher Ordner")[0])
    # H4-H8: double click opens, Alt+F4 closes
    for shot, name in [("h05-persoenlicher-ordner", "Persönlicher Ordner"), ("h06-papierkorb", "Papierkorb"),
                       ("h07-rechner", "Rechner"), ("h08-terminal", "Querencia Terminal"), ("h09-apfel", "Apfel")]:
        check(f"{shot} Doppelklick öffnet ein Fenster",
              f"geöffnet: {name}" in log and differs(img(shot), base, (300, 200, 900, 500)))
    check("h05 Querencia Files im Persönlichen Ordner (native)", "backend=native" in log)
    check("h06 Querencia Files im Papierkorb", "backend=trash" in log)
    check("h07 Querencia Files in Rechner", "backend=computer" in log)
    check("h09b alle Fenster geschlossen", not differs(img("h09b-alle-zu"), base, (300, 200, 900, 500), 0.05))
    # H9-H10: menus at the pointer (640, 300)
    m, s = img("h10-menue"), img("h11-untermenue")
    check("h10 Arbeitsflächenmenü am Zeiger (oben links bei 640, 300)",
          light_share(m, (645, 305, 930, 410)) > 0.7 and light_share(m, (600, 250, 636, 296)) < 0.05,
          f"{light_share(m, (645, 305, 930, 410)):.2f}")
    check("h11 Dokument anlegen hervorgehoben, Untermenü rechts daneben",
          light_share(s, (650, 330, 900, 348), ORANGE, 40) > 0.4 and light_share(s, (952, 330, 1090, 372)) > 0.6)
    # H11-H15: files
    for text in ["Ordner angelegt: Neuer Ordner\n", "Ordner angelegt: Neuer Ordner 2\n",
                 "Dokument angelegt: Neues Dokument\n", "Dokument aus Vorlage angelegt: Brief.txt\n"]:
        check("Protokoll: " + text.strip(), text in log)
    listing = files.split("--- Schreibtisch danach\n")[1].split("--- desktop-order")[0]
    check("Dateien: genau Apfel, Brief.txt, Neuer Ordner, Neuer Ordner 2, Neues Dokument, Zebra.txt",
          listing.splitlines()[:7] == [".", "./Apfel", "./Brief.txt", "./Neuer Ordner", "./Neuer Ordner 2",
                                       "./Neues Dokument", "./Zebra.txt"])
    check("Zebra.txt unverändert (nichts überschrieben)",
          "e2d6b87be8b96af95690472125e7456d7dbfce48d6bb46d84acdb6a0bca64849  Zebra.txt" in files)
    check("vor dem Aufräumen: neue Symbole hinten angehängt",
          "10 Symbole: Rechner, Persönlicher Ordner, Papierkorb, Querencia Terminal, Apfel, Zebra.txt, "
          "Neuer Ordner, Neuer Ordner 2, Neues Dokument, Brief.txt" in log)
    order = files.split("--- desktop-order\n")[1].split("--- desktop.ini")[0].splitlines()
    check("Nach Namen aufräumen: feste Symbole, dann nach Namen",
          order == ["special:computer", "special:home", "special:trash", "app:org.example.Querencia.Terminal.desktop",
                    "file:Apfel", "file:Brief.txt", "file:Neuer Ordner", "file:Neuer Ordner 2",
                    "file:Neues Dokument", "file:Zebra.txt"], " | ".join(order))
    t = img("h16-aufgeraeumt")
    check("h16 zweite Spalte nach dem Aufräumen belegt (10 Symbole, 7 je Spalte)",
          differs(t, base, (120, ROW_Y[0], 196, ROW_Y[0] + 50), 0.2)
          and not differs(t, base, (120, ROW_Y[4], 196, ROW_Y[4] + 50), 0.2))
    # "Neues Dokument" (empty file) after arranging: column 1, row 1, a blank
    # white page like GNOME 2, not the theme's grey "zero size" picture
    page = [t.getpixel((x, y)) for x in range(108 + 30, 108 + 70, 2) for y in range(ROW_Y[1] + 4, ROW_Y[1] + 48, 2)]
    white = sum(1 for p in page if min(p) > 225)
    check("h16 leeres Dokument als weiße Seite", white > len(page) * 0.2, f"{white}/{len(page)} weiß")
    # H16: icon menu
    k = img("h17-symbolmenue")
    check("h17 Symbolmenü am Zeiger (55, 505)", light_share(k, (60, 508, 200, 532)) > 0.7)
    # H17: background dialog, Escape changes nothing, chosen picture shows at once
    check("h18 Dateiauswahl offen", differs(img("h18-dateiauswahl"), t, (300, 200, 900, 500), 0.3))
    check("h19 nach Escape geschlossen", not differs(img("h19-nach-dateiauswahl"), t, (300, 200, 900, 500), 0.05))
    n = img("h19c-neuer-hintergrund")
    check("h19c neues Bild sofort sichtbar (grün oben, rot unten)",
          near(n.getpixel((900, 150)), (40, 160, 60), 20) and near(n.getpixel((900, 600)), (200, 40, 40), 20))
    check("desktop.ini: gewähltes Bild, Modus und Farbe erhalten",
          "File=" in files and "bild-test.png" in files and "Mode=zoom" in files and "Color=#1c4e96" in files)
    # H18: window above icons; H19: panel menu still works
    w = img("h20-fenster-ueber-symbolen")
    check("h20 Fenster liegt über den Symbolen", all(not near(w.getpixel((COL_X, ROW_Y[r] + 20)), base.getpixel((COL_X, ROW_Y[r] + 20)), 24) for r in range(4)))
    check("h21 Anwendungen-Menü des Panels öffnet sich (über dem Fenster)",
          differs(img("h21-anwendungen"), w, (20, 40, 180, 120), 0.5))
    check("keine GTK-Kritikmeldung von querencia-desktop",
          not any("querencia-desktop" in l and "CRITICAL" in l for l in log.splitlines()))
    failed = 0
    for name, ok, detail in checks:
        print(("OK     " if ok else "FEHLER ") + name + (f" – {detail}" if detail else ""))
        failed += not ok
    print(f"{len(checks) - failed}/{len(checks)} Prüfungen bestanden")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
