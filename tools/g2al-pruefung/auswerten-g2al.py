#!/usr/bin/env python3
"""G2-AL: Auswertung eines Laufs von szenario-g2al.py.

  auswerten-g2al.py <logs-ordner> <art> <breite> <höhe>

Prüft die Panel-Logzeilen je Schritt (Marken aus szenario.txt, Uhrzeit je
Logzeile), die Antworten von gdbus und der panel.ini-Ergänzung und
Bildausschnitte:
- Testfenster A/B/C über ihre Füllfarben (Raster 4 px unter dem oberen und
  über dem unteren Panel);
- Knopf „Arbeitsfläche anzeigen“: Symbol bei (23, H - 16), gedrückt, wenn
  bei (12, H - 4) der Knopfgrund #dcd6d0 statt des Panelgrunds liegt;
- Glocke: die Symbole oben rechts vor der Uhr (Uhr ab W - 140) als
  zusammenhängende Spalten (statusgruppen); die letzte Gruppe ist die
  Glocke, eine schmale
  Gruppe dicht dahinter die Zahl. Grau: dunkelster Pixel heller als 400
  (Summe RGB), normal: dunkler als 250; „neu“: orange Pixel im Symbol.
Letzte Zeile: „n/m bestanden“.
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


def found(key, pattern):
    s, e = window(key)
    return [x for w, x in panel if s <= w < e and re.search(pattern, x)]


def before_first_mark(pattern):
    return [x for w, x in panel if w < marks[0][1] and re.search(pattern, x)]


def anywhere(pattern):
    return [x for _, x in panel if re.search(pattern, x)]


def read(name):
    path = LOGS / name
    return path.read_text(errors="replace").strip() if path.exists() else "(fehlt)"


# ------------------------------------------------------------- Pixel
_bilder = {}


def load(name):
    if name not in _bilder:
        _bilder[name] = GdkPixbuf.Pixbuf.new_from_file(str(LOGS / f"{name}.png"))
    return _bilder[name]


def pixel(pb, x, y):
    n, stride, data = pb.get_n_channels(), pb.get_rowstride(), pb.get_pixels()
    o = y * stride + x * n
    return data[o], data[o + 1], data[o + 2]


def near(p, q, tol=12):
    return sum(abs(a - b) for a, b in zip(p, q)) < tol


FARBEN = {"A": (0x5A, 0x8F, 0xD0), "B": (0x5A, 0xB0, 0x6A), "C": (0xD0, 0xA0, 0x5A)}


def fenster(name):
    """Sichtbare Testfenster (Name: Rasterpunkte ihrer Farbe)."""
    pb = load(name)
    cnt = {k: 0 for k in FARBEN}
    for y in range(40, H - 40, 4):
        for x in range(0, W, 4):
            p = pixel(pb, x, y)
            for k, c in FARBEN.items():
                if near(p, c):
                    cnt[k] += 1
    return cnt


def sichtbar(name):
    return sorted(k for k, n in fenster(name).items() if n > 20)


def knopf_da(name):
    r, g, b = pixel(load(name), 23, H - 16)
    return r > 200 and 100 < g < 170 and b < 90


def knopf_gedrueckt(name):
    pb = load(name)
    return near(pixel(pb, 12, H - 4), (0xDC, 0xD6, 0xD0), 20) and not near(
        pixel(pb, 12, H - 4), pixel(pb, W // 2, H - 2), 8)


def statusgruppen(name):
    """Spaltengruppen der Symbole vor der Uhr: [(x0, x1)]. Schwelle 60 über
    dem Panelgrund, damit der helle Grund eines hervorgehobenen Knopfs
    (#ebe7e3, nach einem Menü) nicht zählt; ein bis zwei Spalten breite
    Gruppen sind dessen Rahmen und fallen weg."""
    pb = load(name)
    bg = pixel(pb, W // 2, 2)
    cols = [x for x in range(W - 300, W - 141)
            if any(sum(abs(a - b) for a, b in zip(pixel(pb, x, y), bg)) > 60 for y in range(4, 28))]
    gruppen = []
    for x in cols:
        if gruppen and x - gruppen[-1][1] <= 3:
            gruppen[-1][1] = x
        else:
            gruppen.append([x, x])
    return [tuple(g) for g in gruppen if g[1] - g[0] >= 2]


def glocke(name):
    """(Zahl da, dunkelster Pixel, orange Pixel, Anzahl Symbolgruppen ohne Zahl)."""
    gruppen = statusgruppen(name)
    if not gruppen:
        return None
    zahl = len(gruppen) >= 2 and gruppen[-1][1] - gruppen[-1][0] <= 12 and \
        gruppen[-1][0] - gruppen[-2][1] <= 10
    x0, x1 = gruppen[-2] if zahl else gruppen[-1]
    pb = load(name)
    punkte = [pixel(pb, x, y) for x in range(x0, x1 + 1) for y in range(4, 28)]
    dunkel = min(sum(p) for p in punkte)
    orange = sum(1 for r, g, b in punkte if r > 200 and 90 < g < 160 and b < 120)
    return zahl, dunkel, orange, len(gruppen) - (1 if zahl else 0)


def glocke_check(key, name, zahl, art):
    g = glocke(name)
    if g is None:
        check(f"{key} Bild: Glocke", False, "keine Symbole")
        return
    hat_zahl, dunkel, orange, _ = g
    ok = hat_zahl == zahl
    if art == "grau":
        ok = ok and dunkel > 400
    elif art == "normal":
        ok = ok and dunkel < 250 and orange < 3
    elif art == "neu":
        ok = ok and dunkel < 250 and orange >= 10
    elif art == "ruhe":
        ok = ok and 250 <= dunkel <= 450 and orange == 0
    check(f"{key} Bild: Glocke {art}, {'mit' if zahl else 'ohne'} Zahl", ok,
          f"Zahl {hat_zahl}, dunkel {dunkel}, orange {orange}")


def keine_panik():
    check("keine Panik, kein CRITICAL", not anywhere(r"panicked|CRITICAL"))


TA, TB, TC = "org.querencia.TestA", "org.querencia.TestB", "org.querencia.TestC"
DREI = rf"1 {TA}, 2 {TB}, 3 {TC}"

if ART == "zeigen":
    check("Start: D-Bus ArbeitsflaecheUmschalten angeboten",
          before_first_mark(r"Arbeitsfläche anzeigen: D-Bus org.querencia.Panel.ArbeitsflaecheUmschalten "
                            r"auf /org/example/Querencia/Panel"))
    check("Z01 Bild: Knopf links unten, nicht gedrückt",
          knopf_da("z01-start") and not knopf_gedrueckt("z01-start"))
    check("Z02 Bild: drei Fenster", sichtbar("z02-drei") == ["A", "B", "C"], f"{fenster('z02-drei')}")
    check("Z04 Log: 3 Fenster minimiert (Knopf)", found("Z04", rf"3 Fenster minimiert \(Knopf: {DREI}\)"))
    check("Z04 Bild: kein Fenster, Knopf gedrückt",
          sichtbar("z04-minimiert") == [] and knopf_gedrueckt("z04-minimiert"),
          f"{fenster('z04-minimiert')}")
    check("Z05 Log: 3 wiederhergestellt, C zuletzt",
          found("Z05", rf"3 Fenster wiederhergestellt \(Knopf: {DREI}\)"))
    f5 = fenster("z05-wieder")
    check("Z05 Bild: drei Fenster, C oben (größte Fläche), Knopf frei",
          sichtbar("z05-wieder") == ["A", "B", "C"] and f5["C"] > max(f5["A"], f5["B"])
          and not knopf_gedrueckt("z05-wieder"), f"{f5}")
    check("Z06 Log: Merkliste endet mit dem Klick in der Fensterliste",
          found("Z06", r"3 Fenster minimiert") and found("Z06", r"Merkliste verworfen"))
    check("Z06 Bild: nur B", sichtbar("z06-b") == ["B"], f"{fenster('z06-b')}")
    check("Z07 Log: nur B minimiert und zurück",
          found("Z07", rf"1 Fenster minimiert \(Knopf: 2 {TB}\)")
          and found("Z07", rf"1 Fenster wiederhergestellt \(Knopf: 2 {TB}\)"))
    check("Z07 Bild: B weg, dann B da",
          sichtbar("z07-b-weg") == [] and sichtbar("z07-b-wieder") == ["B"])
    check("Z08 Bild: alle drei wieder (Fensterliste)", sichtbar("z08-alle") == ["A", "B", "C"],
          f"{fenster('z08-alle')}")
    check("Z09 Log: minimiert, Flächenwechsel verwirft, Fläche 2 leer",
          found("Z09", r"3 Fenster minimiert") and found("Z09", r"Merkliste verworfen")
          and found("Z09", r"kein Fenster auf der Arbeitsfläche \(Knopf\)"))
    check("Z09 Bild: Fläche 2 ohne Fenster, Knopf frei",
          sichtbar("z09-flaeche-2") == [] and not knopf_gedrueckt("z09-flaeche-2"))
    check("Z10 Log: auf Fläche 1 nichts wiederherzustellen (Grenze)",
          found("Z10", r"kein Fenster auf der Arbeitsfläche \(Knopf\)"))
    check("Z10 Bild: Fenster bleiben minimiert", sichtbar("z10-flaeche-1") == [])
    n = len(anywhere(r"Arbeitsfläche anzeigen: \d+ Fenster (minimiert|wiederhergestellt)"))
    check("sechs Schaltvorgänge im Log", n == 6, f"{n}")
    keine_panik()
elif ART == "superd":
    check("D01 Bild: drei Fenster", sichtbar("d01-drei") == ["A", "B", "C"])
    check("D02 Super+D: über D-Bus minimiert",
          found("D02", rf"3 Fenster minimiert \(D-Bus :\S+: {DREI}\)"))
    check("D02 Bild: kein Fenster, Knopf gedrückt",
          sichtbar("d02-minimiert") == [] and knopf_gedrueckt("d02-minimiert"))
    check("D03 Super+D: wiederhergestellt", found("D03", r"3 Fenster wiederhergestellt \(D-Bus :\S+: "))
    check("D03 Bild: drei Fenster, Knopf frei",
          sichtbar("d03-wieder") == ["A", "B", "C"] and not knopf_gedrueckt("d03-wieder"))
    d1, d2 = read("dbus-1.txt"), read("dbus-2.txt")
    check("D04 gdbus-Antwort: minimiert", d1.startswith("(true, '3 Fenster minimiert (D-Bus :"), d1[:70])
    check("D04 Bild: kein Fenster", sichtbar("d04-dbus-minimiert") == [])
    check("D05 gdbus-Antwort: wiederhergestellt", d2.startswith("(true, '3 Fenster wiederhergestellt"),
          d2[:70])
    check("D05 Bild: drei Fenster", sichtbar("d05-dbus-wieder") == ["A", "B", "C"])
    check("D06 Super+D, dann Knopf: dieselbe Merkliste",
          found("D06", r"3 Fenster minimiert \(D-Bus") and found("D06", r"3 Fenster wiederhergestellt \(Knopf"))
    check("D06 Bild: drei Fenster", sichtbar("d06-knopf") == ["A", "B", "C"])
    rc = read("rc.xml")
    check("rc.xml der Prüfung: Super+D aus der Vorgabe, sonst kein Execute",
          rc.count('name="Execute"') == 1 and 'key="W-d"' in rc and "ArbeitsflaecheUmschalten" in rc)
    keine_panik()
elif ART == "glocke":
    ohne = anywhere(r"Glocke: org.querencia.Benachrichtigungen ohne Besitzer")
    check("Log: ohne Dienst genau zweimal (Start, Beenden), keine Wiederholung",
          len(ohne) == 2 and before_first_mark(r"Glocke: \S+ ohne Besitzer") and found("G11", r"ohne Besitzer"),
          f"{len(ohne)}")
    glocke_check("G01", "g01-ohne", False, "grau")
    check("G02 Klick ohne Dienst: nur Log", found("G02", r"Glocke: Klick, keine Mitteilungszentrale"))
    check("G03 Dienst gefunden, 0 ungelesen",
          found("G03", r"Glocke: Mitteilungszentrale org.querencia.Benachrichtigungen \(:\S+\): 0 ungelesen"))
    glocke_check("G03", "g03-null", False, "normal")
    check("G04 Zähler 1, 2, 3", all(found("G04", rf"Glocke: {n} ungelesen von {n},") for n in (1, 2, 3)))
    check("G04 Get Ungelesen = 3", read("get-ungelesen-3.txt") == "(<uint32 3>,)", read("get-ungelesen-3.txt"))
    glocke_check("G04", "g04-drei", True, "neu")
    check("G05 Menü geöffnet, Nicht stören an",
          found("G05", r"Appletmenü von benachrichtigungen-1 geöffnet")
          and found("G05", r"Menü → Nicht stören an") and found("G05", r"3 ungelesen von 3, Nicht stören an"))
    check("G05 Get NichtStoeren = true", read("get-ruhe-an.txt") == "(<true>,)", read("get-ruhe-an.txt"))
    glocke_check("G05", "g05-ruhe", True, "ruhe")
    check("G06 bei Nicht stören gezählt", found("G06", r"4 ungelesen von 4, Nicht stören an"))
    glocke_check("G06", "g06-vier", True, "ruhe")
    check("G07 Nicht stören aus", found("G07", r"Menü → Nicht stören aus")
          and read("get-ruhe-aus.txt") == "(<false>,)")
    glocke_check("G07", "g07-laut", True, "neu")
    check("G08 Klick: ZentraleZeigen, Zentrale offen",
          found("G08", r"Klick → ZentraleZeigen \(mit Token\)") and found("G08", r"Zentrale offen")
          and read("get-offen.txt") == "(<true>,)")
    pb = load("g08-zentrale")
    hell = sum(1 for y in range(60, H - 60, 8) for x in range(W // 4, 3 * W // 4, 8)
               if sum(pixel(pb, x, y)) > 600)
    check("G08 Bild: ein helles Fenster in der Mitte (Zentrale)", hell > 300, f"{hell}")
    check("G09 Klick: ZentraleSchliessen, zu",
          found("G09", r"Klick → ZentraleSchliessen") and read("get-zu.txt") == "(<false>,)")
    check("G10 Alle als gelesen: 0 von 4",
          found("G10", r"Menü → alle als gelesen") and found("G10", r"0 ungelesen von 4")
          and read("get-ungelesen-0.txt") == "(<uint32 0>,)")
    glocke_check("G10", "g10-gelesen", False, "normal")
    glocke_check("G11", "g11-beendet", False, "grau")
    check("G12 Dienst wieder da, Verlauf 4, 0 ungelesen",
          found("G12", r"Mitteilungszentrale \S+ \(:\S+\): 0 ungelesen") and found("G12", r"0 ungelesen von 4"))
    glocke_check("G12", "g12-wieder", False, "normal")
    check("Dienst nur isoliert (Verlauf im Profil)",
          "/nachweis/gui/" in read("dienst.log").split("Verlauf ", 1)[-1].split(":", 1)[0])
    keine_panik()
elif ART == "ergaenzung-alt":
    check("E01 altes Panel ohne Knopf", not knopf_da("e01-alt"))
    e = read("ergaenzen.txt")
    check("E02 Ergänzung: beide eingetragen und geschrieben",
          "show-desktop-1 vor der Fensterliste" in e and "benachrichtigungen-1 vor der Uhr" in e
          and "geschrieben:" in e, e.splitlines()[:2])
    check("E02 Ergänzung: neues Layout ohne Problem",
          "End=volume-1;network-1;benachrichtigungen-1;clock-1" in e
          and "Start=show-desktop-1;window-list-1" in e and "Problem:" not in e)
    check("E02 altes Panel: neu gelesen, nichts neu aufgebaut",
          found("E02", r"Neuladen \(Dateiänderung\): Layout aus panel.ini, 2 Panel\(s\); neu aufgebaut: –; "
                       r"unverändert: top, bottom"))
    check("E02 altes Panel: die zwei neuen Typen nur übersprungen",
          len(found("E02", r"unbekannter Applet-Typ")) == 2)
    check("E02 Bild: Panels wie vorher (kein Knopf, gleiche Symbole)",
          not knopf_da("e02-ergaenzt") and statusgruppen("e02-ergaenzt") == statusgruppen("e01-alt"))
    e2 = read("ergaenzen-2.txt")
    check("E03 zweites Ergänzen: unverändert, nicht geschrieben",
          e2.count("schon im Panel, unverändert") == 2 and "geschrieben:" not in e2)
    check("E03 kein Neuladen", not found("E03", r"Neuladen \("))
    z = read("zuruecknehmen.txt")
    check("E04 Zurücknehmen: beide entfernt", "benachrichtigungen-1 entfernt" in z
          and "show-desktop-1 entfernt" in z and "geschrieben:" in z)
    check("E04 altes Panel: nichts neu aufgebaut",
          found("E04", r"neu aufgebaut: –; unverändert: top, bottom"))
    check("panel.ini nach Ergänzen und Zurücknehmen byte-gleich",
          (LOGS / "panel.ini.anfang").read_bytes() == (LOGS / "panel.ini.ende").read_bytes())
    check("panel.ini ergänzt: Typen eingetragen",
          "Type=show-desktop" in read("panel.ini.ergaenzt") and "Type=benachrichtigungen" in read("panel.ini.ergaenzt"))
    keine_panik()
elif ART == "ergaenzung-neu":
    check("N01 ohne Knopf, ohne Glocke (zwei Statussymbole)",
          not knopf_da("n01-ohne") and glocke("n01-ohne")[3] == 2, f"{statusgruppen('n01-ohne')}")
    check("N02 Neuladen: beide Panels neu",
          found("N02", r"Neuladen \(Dateiänderung\): Layout aus panel.ini, 2 Panel\(s\); neu aufgebaut: top, bottom"))
    check("N02 Bild: Knopf und Glocke (drei Statussymbole)",
          knopf_da("n02-mit") and glocke("n02-mit")[3] == 3, f"{statusgruppen('n02-mit')}")
    glocke_check("N02", "n02-mit", False, "grau")
    d = read("dbus-leer.txt")
    check("N03 D-Bus ohne Fenster", d.startswith("(true, 'kein Fenster auf der Arbeitsfläche (D-Bus :"), d)
    check("N04 zurückgenommen: wieder ohne",
          found("N04", r"neu aufgebaut: top, bottom") and not knopf_da("n04-ohne")
          and glocke("n04-ohne")[3] == 2)
    d = read("dbus-ohne-knopf.txt")
    check("N05 D-Bus ohne Knopf: die Fensterliste minimiert",
          d.startswith("(true, '1 Fenster minimiert (D-Bus :") and TA in d, d)
    check("N05 Bild: kein Fenster", sichtbar("n05-minimiert") == [])
    keine_panik()
elif ART == "standard":
    check("Start: Standardlayout", before_first_mark(r"Layout Standardlayout \(keine panel.ini\), 2 Panel\(s\)"))
    check("S01 Bild: Knopf und Glocke (einziges Statussymbol)",
          knopf_da("s01-standard") and glocke("s01-standard")[3] == 1, f"{statusgruppen('s01-standard')}")
    glocke_check("S01", "s01-standard", False, "grau")
    check("S02 Dienst: 1 ungelesen", found("S02", r"Glocke: 1 ungelesen von 1"))
    glocke_check("S02", "s02-eins", True, "neu")
    keine_panik()
else:
    sys.exit(f"unbekannte Art {ART}")

print(f"{sum(results)}/{len(results)} bestanden")
