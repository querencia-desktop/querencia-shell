#!/usr/bin/env python3
"""G2-AD: Auswertung eines Laufs von szenario-g2ad.py.

  auswerten-g2ad.py <logs-ordner> <art> <breite> <höhe> [<logs-ordner thema>]

Prüft die Panel- (und Desktop-)Protokolle je Schritt (Marken aus
szenario.txt, Uhrzeit je Logzeile) und vergleicht Bildausschnitte:
Kalenderfläche unter der Uhr, Desktop-Symbol „Rechner“, Symbolspalte des
Orte-Menüs, Dialogfläche. Bei thema-start vergleicht der fünfte Parameter
mit dem Adwaita-Bild des Laufs „thema“. Letzte Zeile: „n/m bestanden“.
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
THEMA = Path(sys.argv[5]) if len(sys.argv) > 5 else None
results = []

# Bildausschnitte (x0, y0, x1, y1), vermessen bei 1024x768 (ad-erkunden*)
KALENDER = (W - 282, 46, W - 2, 274)
RECHNER = (30, 38, 86, 90)
ORTE_SYMBOLE = (176, 38, 196, 160)
FLAECHE = (0, 40, W, H - 40)        # ohne die beiden Panels (Uhr!)


def check(name, ok, detail=""):
    results.append(bool(ok))
    print(("OK    " if ok else "FEHLER"), name, f"({detail})" if detail != "" else "")


def t(text):
    return datetime.strptime(text, "%H:%M:%S.%f")


szenario = (LOGS / "szenario.txt").read_text(errors="replace")
marks = [(m.group(2), t(m.group(1))) for m in re.finditer(r"^MARKE (\S+) (\S+)", szenario, re.M)]
DAY = timedelta(days=1)
FIRST = marks[0][1] if marks else datetime.min


def later(when):
    return when + DAY if marks and when < FIRST - timedelta(hours=12) else when


marks = [(k, later(v)) for k, v in marks]
order = [k for k, _ in marks]
start = re.search(r"^=== (\d{4})-(\d\d)-(\d\d) ", szenario, re.M)
YEAR, MONTH = int(start.group(1)), int(start.group(2))


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
desktop = timed("desktop.log")


def found(key, pattern, lines=None):
    s, e = window(key)
    return [x for w, x in (panel if lines is None else lines) if s <= w < e and re.search(pattern, x)]


def before_first_mark(pattern, lines):
    return [x for w, x in lines if w < marks[0][1] and re.search(pattern, x)]


def month(offset_months=0, offset_years=0):
    m = MONTH - 1 + offset_months
    return f"{m % 12 + 1:02d}/{YEAR + offset_years + m // 12}"


# ------------------------------------------------------------- Pixel
def load(path):
    return GdkPixbuf.Pixbuf.new_from_file(str(path))


def differing(a, b, box, count=False):
    """Anteil (mit count: Anzahl) der Pixel im Ausschnitt, die sich um mehr
    als 24 (Summe RGB) unterscheiden."""
    pa, pb = load(a), load(b)
    n, sa, sb = pa.get_n_channels(), pa.get_rowstride(), pb.get_rowstride()
    da, db = pa.get_pixels(), pb.get_pixels()
    nb = pb.get_n_channels()
    x0, y0, x1, y1 = box
    diff = total = 0
    for y in range(y0, y1):
        for x in range(x0, x1):
            i, j = y * sa + x * n, y * sb + x * nb
            total += 1
            if sum(abs(da[i + c] - db[j + c]) for c in range(3)) > 24:
                diff += 1
    if count:
        return diff
    return diff / total if total else 0.0


def shot(name, logs=LOGS):
    return logs / f"{name}.png"


def same(a, b, box, name, logs_b=LOGS):
    d = differing(shot(a), shot(b, logs_b), box)
    check(name, d < 0.002, f"{d:.4f} abweichend")


def unlike(a, b, box, name, at_least=0.02):
    d = differing(shot(a), shot(b), box)
    check(name, d >= at_least, f"{d:.4f} abweichend")


# ------------------------------------------------------------- allgemein
for log, lines in (("panel.log", panel), ("desktop.log", desktop)):
    if lines or log == "panel.log":
        bad = [x for _, x in lines if re.search(r"panicked|CRITICAL|Failed to register", x)]
        check(f"{log} ohne panicked/CRITICAL", not bad, "; ".join(bad[:2]))

if ART == "kalender":
    check("Kalender mit deutschem Zeitformat erzeugt",
          before_first_mark(r"Uhr: Kalender mit Zeitformat de_DE\.UTF-8", panel))
    check("K01 Klick öffnet den laufenden Monat", found("K01", rf"Kalender geöffnet \({month()}\)"), month())
    unlike("k01-offen", "k00-ausgang", KALENDER, "K01 Kalenderfläche sichtbar", 0.2)
    check("K02 Monat vor", found("K02", rf"Kalender zeigt {month(1)}$"), month(1))
    check("K03 Jahr vor", found("K03", rf"Kalender zeigt {month(1, 1)}$"), month(1, 1))
    check("K04 Esc schließt", found("K04", r"Kalender geschlossen"))
    same("k04-esc", "k00-ausgang", KALENDER, "K04 Kalenderfläche wieder wie vorher")
    check("K05 wieder geöffnet beim laufenden Monat", found("K05", rf"Kalender geöffnet \({month()}\)"))
    check("K06 zweiter Klick schließt", found("K06", r"Kalender geschlossen"))
    check("K06 und öffnet nicht wieder", not found("K06", r"Kalender geöffnet"))
    same("k06-zweiter-klick", "k00-ausgang", KALENDER, "K06 Kalenderfläche wieder wie vorher")
    k07 = found("K07", r"Kalender (zeigt|geöffnet|geschlossen)")
    shown = [x.rsplit(" ", 1)[1] for x in k07 if "zeigt" in x]
    check("K07 Tastatur erreicht den Kalender (Strg+→ nächster Monat, Strg+← zurück)",
          shown[:2] == [month(1), month()], " ".join(shown))
    unlike("k07-strg-rechts", "k07-zurueck", KALENDER, "K07 Strg+→ zeigt einen anderen Monat", 0.02)
    unlike("k07-auswahl", "k07-zurueck", KALENDER, "K07 Pfeil ab und Leertaste wählen einen anderen Tag", 0.002)
    check("K07 Esc schließt", k07 and k07[-1].endswith("geschlossen"))
    check("K08 Klick daneben schließt",
          found("K08", r"Kalender geöffnet") and found("K08", r"Kalender geschlossen"))
    same("k08-daneben", "k00-ausgang", KALENDER, "K08 Kalenderfläche wieder wie vorher")
    check("K09 Rechtsklick öffnet keinen Kalender", not found("K09", r"Kalender geöffnet"))
    unlike("k09-rechtsklick", "k10-ende", (W - 300, 32, W, 200), "K09 Applet-Menü sichtbar", 0.01)

elif ART == "system":
    sitz = r"/querencia-sitzung"
    check("Systemmenü bietet Abmelden und Ausschalten über querencia-sitzung",
          before_first_mark(rf"Abmelden über \S*{sitz} --abmelden, Ausschalten über \S*{sitz} --herunterfahren", panel))
    unlike("s00-menue", "s01-abgebrochen", (235, 32, 500, 120), "S00 Menü offen", 0.05)
    for key, arg, name in (("S01", "--abmelden", "Abmelden"), ("S02", "--herunterfahren", "Ausschalten")):
        check(f"{key} {name} startet querencia-sitzung {arg}", found(key, rf"{sitz} {arg} gestartet"))
        check(f"{key} Dialog beendet (Esc)", found(key, rf"{sitz} {arg}( beendet|: )"))
        check(f"{key} kein Abmelden/Ausschalten nach Esc", not found(key, r"TROCKENLAUF"))
    unlike("s01-abmelden", "s01-abgebrochen", FLAECHE, "S01 Dialog sichtbar", 0.01)
    # Nur die Zahl im Countdown ändert sich: wenige Dutzend Pixel, bei jeder
    # Auflösung gleich viele, deshalb eine absolute Schwelle.
    n = differing(shot("s01-abmelden"), shot("s01-abmelden-3s"), FLAECHE, count=True)
    check("S01 Countdown läuft (Text ändert sich)", n >= 20, f"{n} Pixel abweichend")
    unlike("s02-ausschalten", "s02-abgebrochen", FLAECHE, "S02 Dialog sichtbar", 0.01)
    check("S03 Countdown Abmelden bis 0: labwc --exit (Trockenlauf)",
          found("S03", r"TROCKENLAUF: \S*labwc\S* --exit"))
    check("S03 querencia-sitzung beendet", found("S03", rf"{sitz} --abmelden( beendet|: )"))
    unlike("s03-start", "s03-nach-countdown", FLAECHE, "S03 Dialog nach dem Countdown weg", 0.01)
    check("S04 Countdown Ausschalten bis 0: logind nicht aufgerufen (Trockenlauf)",
          found("S04", r"TROCKENLAUF: logind"))
    check("S04 querencia-sitzung beendet", found("S04", rf"{sitz} --herunterfahren( beendet|: )"))
    # Jede ausgeführte Aktion der Sitzung („Aktion: …“) muss ein Trockenlauf
    # sein; ein echtes labwc --exit oder ein logind-Aufruf stünde ohne
    # TROCKENLAUF im Log.
    actions = [x for _, x in panel if re.search(r"querencia-sitzung: Aktion: ", x)]
    dry = [x for _, x in panel if re.search(r"querencia-sitzung: TROCKENLAUF: ", x)]
    real = [x for _, x in panel if re.search(r"querencia-sitzung: (\S*labwc\S* --exit beendet|logind (PowerOff|Reboot)\()", x)]
    check("nirgends wirklich abgemeldet oder ausgeschaltet",
          len(actions) == len(dry) == 2 and not real, f"{len(actions)} Aktionen, {len(dry)} Trockenläufe")

elif ART == "thema":
    for prog, lines in (("Panel", panel), ("Desktop", desktop)):
        check(f"{prog} startet mit GNOME Human (keine Benutzereinstellung)",
              before_first_mark(r"Symbolthema gnome-human \(keine Benutzereinstellung\)", lines))
        for key, text, pattern in (
                ("T01", "Adwaita", r"Symbolthema Adwaita \(Benutzereinstellung „Adwaita“\)"),
                ("T02", "Humanity", r"Symbolthema Humanity \(Benutzereinstellung „Humanity“\)"),
                ("T03", "nicht installiert → GNOME Human",
                 r"Symbolthema gnome-human \(Benutzereinstellung „Gibtsnicht“\)"),
                ("T04", "zurückgesetzt → GNOME Human",
                 r"Symbolthema gnome-human \(keine Benutzereinstellung\)")):
            check(f"{key} {prog}: {text}", found(key, pattern, lines))
    unlike("t01-adwaita", "t00-ausgang", RECHNER, "T01 Desktop-Symbol Rechner gewechselt", 0.1)
    unlike("t01-orte", "t00-orte", ORTE_SYMBOLE, "T01 Symbole im Orte-Menü gewechselt", 0.1)
    unlike("t02-humanity", "t01-adwaita", RECHNER, "T02 Rechner wieder anders als Adwaita", 0.1)
    same("t04-reset", "t00-ausgang", RECHNER, "T04 Rechner wieder wie am Anfang")
    same("t04-orte", "t00-orte", ORTE_SYMBOLE, "T04 Orte-Menü wieder wie am Anfang")

elif ART == "thema-start":
    check("Benutzerwert vor dem Start gesetzt",
          "'Adwaita'" in (LOGS / "thema-start.txt").read_text(errors="replace"))
    for prog, lines in (("Panel", panel), ("Desktop", desktop)):
        check(f"{prog} startet mit Adwaita (Benutzereinstellung)",
              before_first_mark(r"Symbolthema Adwaita \(Benutzereinstellung „Adwaita“\)", lines))
        check(f"{prog} setzt kein GNOME Human",
              not [x for _, x in lines if "Symbolthema gnome-human" in x])
    if THEMA:
        same("t10-start", "t01-adwaita", RECHNER, "Rechner wie zur Laufzeit gewechseltes Adwaita", THEMA)
        same("t10-orte", "t01-orte", ORTE_SYMBOLE, "Orte-Menü wie zur Laufzeit gewechseltes Adwaita", THEMA)
else:
    sys.exit(f"unbekannte Art {ART}")

print(f"{sum(results)}/{len(results)} bestanden")
sys.exit(0 if all(results) else 1)
