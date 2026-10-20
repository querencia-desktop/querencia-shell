#!/usr/bin/env python3
"""G2-AK: Auswertung eines Laufs von szenario-g2ak.py.

  auswerten-g2ak.py <logs-ordner> <art> <breite> <höhe>

Prüft die Panel-Logzeilen je Schritt (Marken aus szenario.txt, Uhrzeit je
Logzeile), die Aufrufe der Sitzungs-Attrappe, die D-Bus-Antworten und
Bildausschnitte: Anzahl der Knöpfe in der Fensterliste, rote Pixel des
X11-Testfensters in den Panelstreifen. Letzte Zeile: „n/m bestanden“.
"""
import re
import sys
from datetime import datetime, timedelta
from pathlib import Path

import gi

gi.require_version("GdkPixbuf", "2.0")
from gi.repository import GdkPixbuf  # noqa: E402

LOGS = Path(sys.argv[1])
ART = sys.argv[2]
W, H = int(sys.argv[3]), int(sys.argv[4])
results = []


def check(name, ok, detail=""):
    results.append(bool(ok))
    print(("OK    " if ok else "FEHLER"), name, f"({detail})" if detail != "" else "")


def t(text):
    return datetime.strptime(text, "%H:%M:%S.%f")


szenario = (LOGS / "szenario.txt").read_text(errors="replace")
marks = [(m.group(2), t(m.group(1))) for m in re.finditer(r"^MARKE (\S+) (\S+)", szenario, re.M)]
FIRST = marks[0][1] if marks else datetime.min


def later(when):
    return when + timedelta(days=1) if marks and when < FIRST - timedelta(hours=12) else when


marks = [(k, later(v)) for k, v in marks]
order = [k for k, _ in marks]


def window(key):
    i = order.index(key)
    return marks[i][1], (marks[i + 1][1] if i + 1 < len(marks) else datetime.max)


def timed(name):
    path = LOGS / name
    if not path.exists():
        return []
    out = []
    for line in path.read_text(errors="replace").splitlines():
        m = re.match(r"^(\d\d:\d\d:\d\d\.\d{3}) (.*)$", line)
        if m:
            out.append((later(t(m.group(1))), m.group(2)))
    return out


panel = timed("panel.log")
aufrufe = timed("sitzung-aufrufe.txt")


def found(key, pattern, lines=None):
    s, e = window(key)
    return [x for w, x in (panel if lines is None else lines) if s <= w < e and re.search(pattern, x)]


def before_first_mark(pattern):
    return [x for w, x in panel if w < marks[0][1] and re.search(pattern, x)]


def anywhere(pattern):
    return [x for _, x in panel if re.search(pattern, x)]


def read(name):
    path = LOGS / name
    return path.read_text(errors="replace").strip() if path.exists() else "(fehlt)"


# ------------------------------------------------------------- Pixel
def load(name):
    return GdkPixbuf.Pixbuf.new_from_file(str(LOGS / f"{name}.png"))


def pixel(pb, x, y):
    n, stride, data = pb.get_n_channels(), pb.get_rowstride(), pb.get_pixels()
    o = y * stride + x * n
    return data[o], data[o + 1], data[o + 2]


def red_share(name, y0, y1):
    """Anteil rein roter Pixel (Testfenster) in den Zeilen y0..y1."""
    pb = load(name)
    red = total = 0
    for y in range(y0, y1):
        for x in range(0, W, 4):
            r, g, b = pixel(pb, x, y)
            total += 1
            red += r > 230 and g < 30 and b < 30
    return red / total


def window_buttons(name):
    """Knöpfe der Fensterliste unten links: Breite des Bereichs, der sich vom
    Panelgrund unterscheidet, geteilt durch die Knopfbreite (200 + 2)."""
    pb = load(name)
    y = H - 16
    bg = pixel(pb, W // 2, H - 2)
    right = 0
    for x in range(4, W - 160):
        if sum(abs(a - b) for a, b in zip(pixel(pb, x, y), bg)) > 24:
            right = x
    return round((right - 6) / 202) if right else 0


def last_shown(key):
    """Die sichtbaren App-IDs nach der letzten Logzeile der Fensterliste im
    Schritt `key` (None: keine Zeile)."""
    lines = found(key, r"Fensterliste: \d+ von \d+ Fenstern sichtbar")
    if not lines:
        return None
    m = re.search(r"sichtbar \((.*)\)$", lines[-1])
    return sorted(part.split(" ", 1)[1] for part in m.group(1).split(", ") if part) if m else None


A, B = "org.querencia.TestA", "org.querencia.TestB"
ATTRAPPE = r"/attrappe/(pfad|variable)/querencia-sitzung"

if ART == "sitzung":
    check("Start: Sperren über die Attrappe aus PATH",
          before_first_mark(r"Sperren über \S+/attrappe/pfad/querencia-sitzung --sperren --sofort"))
    check("Start: Ausschalten über die Attrappe",
          before_first_mark(r"Ausschalten über \S+/attrappe/pfad/querencia-sitzung --herunterfahren"))
    check("S02 Sperren startet --sperren --sofort",
          found("S02", r"attrappe/pfad/querencia-sitzung --sperren --sofort gestartet"))
    check("S02 Attrappe mit --sperren --sofort aufgerufen",
          found("S02", r"/attrappe/pfad/querencia-sitzung --sperren --sofort$", aufrufe))
    check("S03 Abmelden über die Attrappe (--abmelden)",
          found("S03", r"/attrappe/pfad/querencia-sitzung --abmelden$", aufrufe))
    check("S04 Ausschalten über die Attrappe (--herunterfahren)",
          found("S04", r"/attrappe/pfad/querencia-sitzung --herunterfahren$", aufrufe))
    check("genau drei Aufrufe, nie das echte Programm",
          len(aufrufe) == 3 and all(re.search(ATTRAPPE, x) for _, x in aufrufe),
          f"{len(aufrufe)}")
    check("kein --exit des Panels", not anywhere(r"--exit gestartet"))
elif ART == "sitzung-variable":
    check("Start: Sperren über QUERENCIA_SITZUNG (vor PATH)",
          before_first_mark(r"Sperren über \S+/attrappe/variable/querencia-sitzung --sperren"))
    check("V01 Attrappe aus QUERENCIA_SITZUNG gesperrt",
          found("V01", r"/attrappe/variable/querencia-sitzung --sperren --sofort$", aufrufe))
    check("nur diese Attrappe", len(aufrufe) == 1, f"{len(aufrufe)}")
elif ART == "sitzung-keins":
    check("Start: kein Sperren, Grund im Log",
          before_first_mark(r"kein Sperren \(QUERENCIA_SITZUNG=\S+/fehlt/querencia-sitzung: "
                            r"kein ausführbares Programm\)"))
    check("Start: Abmelden über labwc --exit (Rückfall)",
          before_first_mark(r"Abmelden über \S+labwc --exit"))
    check("Start: kein Ausschalten", before_first_mark(r"kein Ausschalten"))
    check("K02 alte Rückfrage geöffnet", found("K02", r"Abmelden: Rückfrage geöffnet"))
    check("kein --exit, keine Attrappe", not anywhere(r"--exit gestartet") and not aufrufe)
    # Dialog sichtbar: Fläche in der Mitte nicht mehr schwarz; nach Esc wieder
    def dark(name):
        pb = load(name)
        return sum(sum(pixel(pb, x, H // 2)) < 30 for x in range(W // 4, 3 * W // 4, 8))
    check("K02 Rückfrage im Bild", dark("k02-rueckfrage") < dark("k03-abgebrochen"),
          f"{dark('k02-rueckfrage')} < {dark('k03-abgebrochen')}")
elif ART in ("fensterliste", "fensterliste-alle"):
    alle = ART == "fensterliste-alle"
    check("Start: Einstellung im Log",
          before_first_mark(r"Fenster aller Arbeitsflächen \(alle_arbeitsflaechen=ja\)" if alle
                            else r"nur Fenster der aktuellen Arbeitsfläche"))
    soll = {  # Schritt: (Bild, sichtbar nur aktuelle, Knöpfe nur aktuelle, Knöpfe alle)
        "F01": ("f01-a", [A], 1, 1),
        "F02": ("f02-flaeche-2", [], 0, 1),
        "F03": ("f03-b", [B], 1, 2),
        "F04": ("f04-flaeche-1", [A], 1, 2),
        "F05": ("f05-mitgenommen", [A, B], 2, 2),
        "F06": ("f06-leer", [], 0, 2),
        "F07": ("f07-umschalter", [A, B], 2, 2),
    }
    for key, (bild, sichtbar, knoepfe, knoepfe_alle) in soll.items():
        if not alle:
            # jeder Schritt ändert die sichtbare Menge, also je eine Zeile
            ist = last_shown(key)
            check(f"{key} Log: sichtbar {sichtbar}", ist == sichtbar, f"{ist}")
        n = window_buttons(bild)
        check(f"{key} Bild: {knoepfe_alle if alle else knoepfe} Knöpfe", n == (knoepfe_alle if alle else knoepfe),
              f"{n}")
    if alle:
        check("keine Filterzeilen bei alle=ja", not anywhere(r"Fenstern sichtbar"))
    wechsel = anywhere(r"Fensterliste: Arbeitsfläche „")
    check("Flächenwechsel im Log", len(wechsel) >= 5, f"{len(wechsel)}")
elif ART == "fensterliste-vorher":
    check("P01 beide Fenster sichtbar", window_buttons("p01-start") == 2,
          f"{window_buttons('p01-start')}")
    check("P02 Fläche 2: nur das nicht zugeordnete A", last_shown("P02") == [A], f"{last_shown('P02')}")
    check("P02 Bild: 1 Knopf", window_buttons("p02-flaeche-2") == 1)
    ist = last_shown("P03")
    check("P03 nach Aktivierung ist A auf Fläche 1: Fläche 2 leer", ist == [], f"{ist}")
    check("P03 Bild: 0 Knöpfe", window_buttons("p03-flaeche-2") == 0,
          f"{window_buttons('p03-flaeche-2')}")
elif ART == "neuladen":
    check("Start: Beobachtung und D-Bus",
          before_first_mark(r"Neuladen: beobachte \S+/config/querencia/panel.ini")
          and before_first_mark(r"Neuladen: D-Bus org.querencia.Panel.NeuLaden auf "
                                r"/org/example/Querencia/Panel"))
    check("N02 Dateiänderung: unten neu, oben unverändert",
          found("N02", r"Neuladen \(Dateiänderung\): Layout aus panel.ini, 2 Panel\(s\); "
                       r"neu aufgebaut: bottom; unverändert: top; entfernt: –"))
    check("N02 genau ein Neuladen", len(found("N02", r"Neuladen \(")) == 1,
          f"{len(found('N02', r'Neuladen [(]'))}")
    check("N03 kaputte Datei: Panels bleiben",
          found("N03", r"Neuladen \(Dateiänderung\): panel.ini fehlerhaft, die Panels bleiben"))
    check("N03 nichts neu aufgebaut", not found("N03", r"neu aufgebaut"))
    check("N04 D-Bus-Antwort false", read("dbus-kaputt.txt").startswith("(false, 'panel.ini fehlerhaft"),
          read("dbus-kaputt.txt")[:60])
    check("N04 Logzeile mit Absender", found("N04", r"Neuladen \(D-Bus :\S+\): panel.ini fehlerhaft"))
    check("N05 neue Datei: beide neu, alle Arbeitsflächen",
          found("N05", r"neu aufgebaut: top, bottom; unverändert: –; entfernt: –; "
                       r"Fensterliste alle Arbeitsflächen"))
    check("N06 D-Bus unverändert", read("dbus-unveraendert.txt") == "(true, 'panel.ini unverändert')",
          read("dbus-unveraendert.txt"))
    check("N07 eigene Änderung (Uhr nach links) gespeichert",
          found("N07", r"clock-1 verschoben \(\w+\): .* gespeichert in \S+panel.ini"))
    eigene = found("N07", r"Neuladen \(")
    check("N07 eigene Änderung: panel.ini unverändert, nichts neu aufgebaut",
          eigene and all(x.endswith("panel.ini unverändert") for x in eigene), f"{eigene}")
    check("N08 gelöscht: Standardlayout, oben neu",
          found("N08", r"Neuladen \(Dateiänderung\): Layout Standardlayout \(keine panel.ini\), "
                       r"2 Panel\(s\); neu aufgebaut: top, bottom"))
    check("N09 D-Bus nach dem Löschen unverändert",
          read("dbus-geloescht.txt") == "(true, 'panel.ini unverändert')", read("dbus-geloescht.txt"))
    # Bilder: N02 ohne Umschalter (Zelle 1 = Panelgrund), N05 mit „Orte“ in der Mitte oben
    def umschalter(name):
        pb = load(name)
        return sum(abs(a - b) for a, b in zip(pixel(pb, W - 113, H - 16), pixel(pb, W // 2, H - 2))) > 40
    check("N01 Umschalter da", umschalter("n01-start"))
    check("N02 Umschalter weg", not umschalter("n02-ohne-umschalter"))
    check("N03 unverändert (kein Umschalter)", not umschalter("n03-kaputt"))
    check("N08 Standard: Umschalter wieder da", umschalter("n08-geloescht"))
    def mitte_oben(name):
        pb = load(name)
        bg = pixel(pb, W // 2 + 200, 2)
        return sum(sum(abs(a - b) for a, b in zip(pixel(pb, x, NY), bg)) > 60
                   for x in range(W // 2 - 60, W // 2 + 60))
    NY = 16
    check("N05 Orte in der Mitte oben", mitte_oben("n05-neu") > 8, f"{mitte_oben('n05-neu')}")
    check("N01 Mitte oben leer", mitte_oben("n01-start") == 0, f"{mitte_oben('n01-start')}")
    check("keine Panik", not anywhere(r"panicked"))
elif ART == "vollbild":
    def oben(name):
        return red_share(name, 2, 8)

    def unten(name):
        return red_share(name, H - 8, H - 2)

    def mitte(name):
        return red_share(name, H // 2 - 3, H // 2 + 3)

    for key, name in (("X01", "x01-vorher"), ("X02", "x02-danach")):
        check(f"{key} Vollbild: Panels ausgeblendet", oben(name) > 0.95 and unten(name) > 0.95,
              f"oben {oben(name):.2f} unten {unten(name):.2f}")
    check("X01 _NET_WM_STATE_FULLSCREEN", "FULLSCREEN" in read("xprop-vorher.txt"),
          read("xprop-vorher.txt"))
    check("X02 _NET_WM_STATE_FULLSCREEN", "FULLSCREEN" in read("xprop-danach.txt"),
          read("xprop-danach.txt"))
    n = "x03-verschoben"
    check("X03 bildschirmgroß ohne Vollbild: Panels liegen darüber (Befund Horizon)",
          oben(n) < 0.05 and unten(n) < 0.05 and mitte(n) > 0.95,
          f"oben {oben(n):.2f} unten {unten(n):.2f} mitte {mitte(n):.2f}")
    check("X03 kein Vollbild-Zustand", "FULLSCREEN" not in read("xprop-verschoben.txt"),
          read("xprop-verschoben.txt"))
    n = "x04-super-f"
    check("X04 Super+F: Panels ausgeblendet", oben(n) > 0.95 and unten(n) > 0.95,
          f"oben {oben(n):.2f} unten {unten(n):.2f}")
    check("X04 _NET_WM_STATE_FULLSCREEN nach Super+F", "FULLSCREEN" in read("xprop-super-f.txt"),
          read("xprop-super-f.txt"))
    n = "x05-alt-f11"
    check("X05 Alt+F11: Panels wieder da", oben(n) < 0.05 and unten(n) < 0.05,
          f"oben {oben(n):.2f} unten {unten(n):.2f}")
    check("X05 kein Vollbild-Zustand", "FULLSCREEN" not in read("xprop-alt-f11.txt"),
          read("xprop-alt-f11.txt"))
    for key, n in (("X06", "x06-override"), ("X07", "x07-override-super-f")):
        check(f"{key} override_redirect: unter den Panels", oben(n) < 0.05 and mitte(n) > 0.95,
              f"oben {oben(n):.2f} mitte {mitte(n):.2f}")

    def panel_da(name):
        pb = load(name)
        return sum(pixel(pb, W // 2, 2)) > 600 and sum(pixel(pb, W // 2, H - 2)) > 600
    check("X08 Wayland-Fenster: Panels da", panel_da("x08-wayland"))
    check("X08 Super+F: Panels weg (Fenster vollbild)", not panel_da("x08-wayland-super-f"))
    check("X08 Alt+F11: Panels wieder da", panel_da("x08-wayland-alt-f11"))
else:
    sys.exit(f"unbekannte Art {ART}")

print(f"{sum(results)}/{len(results)} bestanden")
