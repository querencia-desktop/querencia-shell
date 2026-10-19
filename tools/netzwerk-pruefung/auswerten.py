#!/usr/bin/env python3
"""G2-M: wertet einen isolierten Lauf des Netzwerk-Applets aus.

  python3 -I auswerten.py <logs-ordner> rueckfall|attrappe|spaet <breite> <höhe>

Quellen je Lauf (szenario-g2m.sh): szenario.txt (MARKE-Zeilen mit Uhrzeit),
panel.log (jede Zeile mit Uhrzeit, Menüzeilen über QUERENCIA_NETZWERK_DEBUG),
qnm.log / qnm-spaet*.log (Attrappe: jede Anfrage mit Uhrzeit),
panel.ini.anfang/.ende, einstellungen-start.txt, Bildschirmfotos. Log-
zeilen werden über die Uhrzeit dem Prüfschritt (Marke) zugeordnet.

Symbole: Das Applet-Symbol (22 px) wird im Bild gesucht (±4 px um die
erwartete Stelle) und mit allen zehn Symbolen verglichen, die aus den
SVG-Dateien des Crates mit demselben Lader (GdkPixbuf) gezeichnet und auf
die Panelfarbe gelegt werden; das ähnlichste muss das erwartete sein.

Ausgabe: je Prüfung „OK“ oder „FEHLER“, am Ende „n/m Prüfungen bestanden“;
Exit 0 nur, wenn alle bestanden.
"""
import os
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
NX, NY = W - 171, 16
CX, CY = W // 2, H // 2
SYMBOLE = Path(__file__).resolve().parent.parent.parent / "crates/querencia-netzwerk/data/symbole"
ICONS = {
    "WiredConnected": "netzwerk-kabel.svg",
    "WiredDisconnected": "netzwerk-kabel-getrennt.svg",
    "Wireless(0)": "netzwerk-wlan-0.svg",
    "Wireless(1)": "netzwerk-wlan-1.svg",
    "Wireless(2)": "netzwerk-wlan-2.svg",
    "Wireless(3)": "netzwerk-wlan-3.svg",
    "WirelessOff": "netzwerk-wlan-aus.svg",
    "Connecting": "netzwerk-verbindet.svg",
    "NoNetwork": "netzwerk-kein-netz.svg",
    "NoService": "netzwerk-kein-dienst.svg",
}
SIZE = 22
results = []


def check(name, ok, detail=""):
    results.append(bool(ok))
    print(("OK    " if ok else "FEHLER"), name, f"({detail})" if detail != "" else "")


def t(text):
    return datetime.strptime(text, "%H:%M:%S.%f")


# ------------------------------------------------------------- Marken, Logs
szenario = (LOGS / "szenario.txt").read_text(errors="replace")
marks = [(m.group(2), t(m.group(1))) for m in re.finditer(r"^MARKE (\S+) (\S+)", szenario, re.M)]
# Uhrzeiten ohne Datum: ein Lauf über Mitternacht zählt nach 0 Uhr einen Tag weiter.
DAY = timedelta(days=1)
FIRST = marks[0][1] if marks else datetime.min


def later(when):
    return when + DAY if marks and when < FIRST - timedelta(hours=12) else when


for i in range(1, len(marks)):
    marks[i] = (marks[i][0], later(marks[i][1]))
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
mock = sorted(timed("qnm.log") + timed("qnm-spaet.log") + timed("qnm-spaet-2.log"))


def found(key, pattern, lines):
    start, end = window(key)
    return [text for when, text in lines if start <= when < end and re.search(pattern, text)]


def p(key, pattern):
    return found(key, pattern, panel)


def m(key, pattern):
    return found(key, pattern, mock)


def everywhere(pattern, lines=None):
    return [text for _, text in (lines if lines is not None else panel) if re.search(pattern, text)]


def last_before(key, pattern):
    """Letzte passende Panelzeile vor dem Ende des Schritts."""
    _, end = window(key)
    hits = [text for when, text in panel if when < end and re.search(pattern, text)]
    return hits[-1] if hits else ""


def tooltip(key):
    line = last_before(key, r"Netzwerk: Tooltip „")
    mt = re.search(r"Tooltip „(.*)“$", line)
    return mt.group(1) if mt else ""


def menu_rows(key):
    """Die zuletzt im Schritt gebauten Menüzeilen."""
    rows = p(key, r"Netzwerk: Menü: ")
    return rows[-1].split("Netzwerk: Menü: ", 1)[1].split(" | ") if rows else []


# ------------------------------------------------------------- Pixel
def load(shot):
    return GdkPixbuf.Pixbuf.new_from_file(str(LOGS / f"{shot}.png"))


def pixel(pb, x, y):
    n = pb.get_n_channels()
    data = pb.get_pixels()
    i = y * pb.get_rowstride() + x * n
    return data[i], data[i + 1], data[i + 2]


REF = {}


def reference(name, bg):
    key = (name, bg)
    if key not in REF:
        src = GdkPixbuf.Pixbuf.new_from_file_at_scale(str(SYMBOLE / ICONS[name]), SIZE, SIZE, True)
        n, stride, data = src.get_n_channels(), src.get_rowstride(), src.get_pixels()
        out = []
        for y in range(SIZE):
            row = []
            for x in range(SIZE):
                i = y * stride + x * n
                a = data[i + 3] / 255 if n == 4 else 1
                row.append(tuple(round(data[i + c] * a + bg[c] * (1 - a)) for c in range(3)))
            out.append(row)
        REF[key] = out
    return REF[key]


def classify(shot):
    """(Name, Abweichung) des ähnlichsten Symbols am Applet-Platz."""
    pb = load(shot)
    bg = pixel(pb, NX - 13, 2)
    x0, y0 = NX - SIZE // 2, NY - SIZE // 2
    best = (None, 1e9)
    crops = {}
    for dx in range(-4, 5):
        for dy in range(-4, 5):
            crops[(dx, dy)] = [[pixel(pb, x0 + dx + x, y0 + dy + y) for x in range(SIZE)] for y in range(SIZE)]
    for name in ICONS:
        ref = reference(name, bg)
        for crop in crops.values():
            diff = sum(abs(crop[y][x][c] - ref[y][x][c]) for y in range(SIZE) for x in range(SIZE) for c in range(3))
            diff /= SIZE * SIZE * 3
            if diff < best[1]:
                best = (name, diff)
    return best


def icon_is(shot, expected):
    name, diff = classify(shot)
    check(f"Symbol {shot}: {expected}", name == expected and diff < 8, f"erkannt {name}, Abweichung {diff:.2f}")


def dialog_open(shot):
    """Ein mittiger Dialog deckt die schwarze Fläche in der Bildmitte ab."""
    r, g, b = pixel(load(shot), CX, CY)
    return r + g + b > 60


def menu_bottom(shot):
    """Unterkante des Linksklickmenüs in der Spalte x = Breite - 300."""
    pb = load(shot)
    bottom = 0
    for y in range(33, H - 40):
        r, g, b = pixel(pb, W - 300, y)
        if r + g + b > 300:
            bottom = y
    return bottom


def ini(name):
    path = LOGS / name
    return path.read_text() if path.exists() else ""


def choices(rows):
    """Wählbare WLAN-Zeilen (ohne die verbundene, fette)."""
    return [r for r in rows if re.match(r"\{\d\} ", r) and "*" not in r]


def has_rows(key, *expected):
    rows = menu_rows(key)
    missing = [e for e in expected if e not in rows]
    check(f"{key} Menüzeilen", rows and not missing, "fehlen: " + "; ".join(missing) if missing else " | ".join(rows))


# ------------------------------------------------------------- allgemein
if ART != "verschieben":
    check("Panel einmal gestartet (kein Neustart)", len(everywhere(r"Layout aus panel\.ini")) == 1,
          len(everywhere(r"Layout aus panel\.ini")))
bad = everywhere(r"CRITICAL|panicked|Speicherzugriffsfehler|segfault")
check("keine GLib-CRITICAL, kein Absturz", not bad, "; ".join(bad[:3]))
check("Szenario vollständig", "ENDE" in order and "qeingabe exit 0" in szenario)

if ART == "rueckfall":
    check("R00 panel.ini ohne Netzwerk", "network" not in ini("panel.ini.anfang"))
    check("R00 Rückfall gewählt", everywhere(r"Netzwerk: kein NetworkManager, nur Anzeige"))
    check("R00 Quelle Rückfall mit systemd-networkd", everywhere(r"Quelle Fallback \{ networkd: true \}"))
    added = p("R01", r"network-1 \(network\) zu Panel top hinzugefügt")
    check("R01 über den Dialog hinzugefügt (rechts)", added and "Bereich rechts" in added[0], added)
    check("R02 zweites Mal nichts hinzugefügt", not p("R02", r"zu Panel top hinzugefügt"))
    icon_is("r03-symbol", "WiredConnected")
    tip = tooltip("R04")
    check("R04 Tooltip: eth0, Adresse, nur Anzeige",
          re.match(r"Kabelnetzwerk eth0 verbunden \| Adresse: \d+\.\d+\.\d+\.\d+/\d+ \| Verwaltet von systemd-networkd, nur Anzeige$", tip), tip)
    check("R05 Menü geöffnet", p("R05", r"Netzwerk: Menü geöffnet"))
    has_rows("R05", "(Verwaltet von systemd-networkd, nur Anzeige)", "[Kabelnetzwerk]", "☐ Flugmodus –",
             "Verbindungsinformationen …", "Netzwerkeinstellungen … (kommt mit G2-O) –")
    rows = menu_rows("R05")
    eth = [r for r in rows if r.startswith("eth0*")]
    check("R05 eth0 fett mit Adresse, nicht wählbar", eth and "‹" in eth[0] and eth[0].endswith("–"), eth)
    check("R05 nichts zum Schalten (kein Trennen, kein WLAN)",
          not [r for r in rows if r.startswith(("Trennen", "Verbinden", "Mit verborgenem")) or "WLAN aktiviert" in r])
    check("R06 Tastatur: einzige wählbare Zeile", p("R06", r"Tastatur: Zeile 1 von 1 \(\d+:Verbindungsinformationen …\)"))
    check("R06 Info-Dialog geöffnet", p("R06", r"Dialog „Verbindungsinformationen“ geöffnet") and dialog_open("r06-info"))
    check("R07 Eingabetaste schließt (Fokus auf Schließen)", not dialog_open("r07-info-zu"))
    check("R08 Rechtsklickmenü", p("R08", r"Appletmenü von network-1 geöffnet"))
    check("R08 Info über Rechtsklick", p("R08", r"Dialog „Verbindungsinformationen“ geöffnet") and dialog_open("r08-info"))
    check("R09 Escape schließt", not dialog_open("r09-info-zu"))
    check("R10 entfernt, gespeichert", "network" not in ini("panel.ini.ende") and "End=volume-1;clock-1" in ini("panel.ini.ende"))
    check("R10 nie NetworkManager-Aufrufe", not everywhere(r"Netzwerk: (verbinde|trenne|WLAN (an|aus)|Flugmodus)"))
elif ART == "attrappe":
    check("M00 NetworkManager gefunden", everywhere(r"Netzwerk: NetworkManager gefunden"))
    check("M00 Passwortagent angemeldet", everywhere(r"Passwortagent angemeldet") and everywhere(r"Agent angemeldet: org\.querencia\.Panel\.Netzwerk", mock))
    icon_is("m00-kabel", "WiredConnected")
    tip = tooltip("M01")
    check("M01 Tooltip", tip == "Kabelnetzwerk „Kabelgebundene Verbindung 1“ verbunden (enp3s0) | Adresse: 192.0.2.24/24", tip)
    check("M02 Menü geöffnet, WLAN-Suche angestoßen", p("M02", r"Menü geöffnet") and m("M02", r"RequestScan"))
    has_rows("M02", "[Kabelnetzwerk]", "Kabelgebundene Verbindung 1* ‹192.0.2.24/24› –", "Trennen", "[WLAN-Netze]",
             "{3} Heimnetz 🔒", "{1} Cafe offen", "{0} Nachbar 🔒", "Mit verborgenem Netz verbinden …",
             "☑ WLAN aktiviert", "☐ Flugmodus", "Verbindungsinformationen …", "Netzwerkeinstellungen … (kommt mit G2-O) –")
    check("M03 Tastatur: Heimnetz", p("M03", r"Tastatur: Zeile 2 von 8 \(\d+:Heimnetz\)"))
    check("M03 Escape schließt", p("M03", r"Menü geschlossen"))
    check("M04 Kabel ab", m("M04", r"Befehl: kabel ab"))
    icon_is("m04-kabel-ab", "NoNetwork")
    check("M04 Tooltip", tooltip("M04") == "Keine Netzwerkverbindung", tooltip("M04"))
    has_rows("M05", "(Kabel nicht angeschlossen)")
    check("M05 Mausklick „Cafe offen“", p("M05", r"Menü → Cafe offen"))
    check("M05 AddAndActivate offen", m("M05", r"AddAndActivateConnection ssid=Cafe offen key-mgmt=keine psk=keins hidden=False"))
    icon_is("m05-verbindet", "Connecting")
    icon_is("m05-verbunden", "Wireless(1)")
    tip = tooltip("M05")
    check("M05 Tooltip WLAN", tip == "WLAN „Cafe offen“ verbunden, Signal 45 % | Adresse: 192.0.2.37/24", tip)
    icon_is("m06-signal-90", "Wireless(3)")
    icon_is("m07-signal-10", "Wireless(0)")
    icon_is("m08-signal-60", "Wireless(2)")
    has_rows("M09", "{2} Cafe offen* ‹192.0.2.37/24› –", "{3} Heimnetz 🔒", "{0} Nachbar 🔒", "Trennen")
    check("M10 Tastatur: Nachbar", p("M10", r"Tastatur: Zeile 2 von 7 \(\d+:Nachbar\)") and p("M10", r"Menü → Nachbar"))
    check("M10 eigener Passwortdialog (neues WPA3-Netz)", p("M10", r"Dialog „WLAN-Passwort“ für „Nachbar“ geöffnet$") and dialog_open("m10-passwort"))
    check("M11 zu kurz: nichts gesendet, Dialog offen",
          not p("M11", r"Passwortdialog „Nachbar“: Verbinden") and not m("M11", r"AddAndActivate") and dialog_open("m11-kurz"))
    check("M12 Passwort übergeben", p("M12", r"Passwortdialog „Nachbar“: Verbinden \(9 Zeichen\)"))
    check("M12 AddAndActivate WPA3 mit Schlüssel", m("M12", r"AddAndActivateConnection ssid=Nachbar key-mgmt=sae psk=\(9 Zeichen\)"))
    check("M12 verbunden", m("M12", r"aktiviert: Nachbar auf wlp2s0"))
    icon_is("m12-nachbar-verbunden", "Wireless(0)")
    check("M12 Tooltip", tooltip("M12").startswith("WLAN „Nachbar“ verbunden, Signal 20 %"), tooltip("M12"))
    check("M13 Heimnetz gewählt (gespeichertes Profil)", p("M13", r"Tastatur: Zeile 1 von 7 \(\d+:Heimnetz\)") and m("M13", r"ActivateConnection /org/freedesktop/NetworkManager/Settings/\d+ "))
    check("M13 NetworkManager fragt den Agenten", m("M13", r"GetSecrets an :\S+ für Heimnetz \(Flags 1\)") and p("M13", r"Passwortabfrage für „Heimnetz“$"))
    check("M13 Agenten-Dialog offen", p("M13", r"Dialog „WLAN-Passwort“ für „Heimnetz“ geöffnet$") and dialog_open("m13-agent"))
    check("M14 falsch: erneute Abfrage", m("M14", r"Schlüssel für Heimnetz falsch") and m("M14", r"GetSecrets an :\S+ für Heimnetz \(Flags 3\)")
          and p("M14", r"Dialog „WLAN-Passwort“ für „Heimnetz“ geöffnet \(erneut\)"))
    check("M15 richtig: verbunden", m("M15", r"GetSecrets beantwortet \(9 Zeichen\)") and m("M15", r"aktiviert: Heimnetz auf wlp2s0"))
    icon_is("m15-heimnetz-verbunden", "Wireless(3)")
    check("M15 Tooltip", tooltip("M15").startswith("WLAN „Heimnetz“ verbunden, Signal 82 %"), tooltip("M15"))
    check("M16 Kabel an", m("M16", r"Befehl: kabel an") and m("M16", r"aktiviert: Kabelgebundene Verbindung 1 auf enp3s0"))
    icon_is("m16-verbindet", "Connecting")
    icon_is("m16-kabel", "WiredConnected")
    tip = tooltip("M16")
    check("M16 Tooltip: Kabel zuerst, dann WLAN",
          tip.startswith("Kabelnetzwerk „Kabelgebundene Verbindung 1“ verbunden (enp3s0) | Adresse: 192.0.2.24/24 | WLAN „Heimnetz“ verbunden"), tip)
    check("M17 Ende wählt Verbindungsinformationen", p("M17", r"Tastatur: Zeile 8 von 8 \(\d+:Verbindungsinformationen …\)"))
    check("M17 Info-Dialog", p("M17", r"Dialog „Verbindungsinformationen“ geöffnet") and dialog_open("m17-info"))
    check("M18 Eingabetaste schließt (Fokus auf Schließen)", not dialog_open("m18-info-zu"))
    check("M19 Info über Rechtsklick", p("M19", r"Appletmenü von network-1 geöffnet") and dialog_open("m19-info-wlan"))
    check("M20 Flugmodus: WLAN und WWAN aus", p("M20", r"Appletmenü → Flugmodus an")
          and m("M20", r"WirelessEnabled = False") and m("M20", r"WwanEnabled = False"))
    icon_is("m20-flugmodus", "WiredConnected")
    has_rows("M20", "(Flugmodus ist an)", "☑ Flugmodus", "☐ WLAN aktiviert")
    icon_is("m21-kabel-getrennt", "WiredDisconnected")
    check("M21 Tooltip", tooltip("M21") == "Kein Netzwerkkabel angeschlossen", tooltip("M21"))
    check("M22 Flugmodus aus (Linksklickmenü, Tastatur)", p("M22", r"Tastatur: Zeile 2 von 3 \(\d+:Flugmodus\)")
          and m("M22", r"WirelessEnabled = True") and m("M22", r"WwanEnabled = True"))
    icon_is("m22-flugmodus-aus", "NoNetwork")
    check("M23 Kabel getrennt über das Menü", p("M23", r"Menü → Trennen") and m("M23", r"Disconnect /org/freedesktop/NetworkManager/Devices/1"))
    check("M23 WLAN aus (Rechtsklick)", p("M23", r"Appletmenü → WLAN aus") and m("M23", r"WirelessEnabled = False"))
    icon_is("m23-wlan-aus", "WirelessOff")
    check("M23 Tooltip", tooltip("M23") == "WLAN ist ausgeschaltet", tooltip("M23"))
    has_rows("M23", "(WLAN ist ausgeschaltet)", "☐ WLAN aktiviert", "Verbinden")
    check("M24 WLAN an", p("M24", r"Appletmenü → WLAN an") and m("M24", r"WirelessEnabled = True"))
    icon_is("m24-wlan-an", "NoNetwork")
    check("M25 Dialog verborgenes Netz", p("M25", r"Tastatur: Zeile 5 von 8 \(\d+:Mit verborgenem Netz verbinden …\)")
          and p("M25", r"Dialog „Mit verborgenem Netz verbinden“ geöffnet") and dialog_open("m25-verborgen"))
    check("M26 gesendet", p("M26", r"verborgenes Netz „Versteckt“, WPA/WPA2 Personal")
          and m("M26", r"AddAndActivateConnection ssid=Versteckt key-mgmt=wpa-psk psk=\(10 Zeichen\) hidden=True"))
    check("M26 verbunden", m("M26", r"aktiviert: Versteckt auf wlp2s0"))
    icon_is("m26-verbunden", "Wireless(2)")
    check("M26 Tooltip mit Profilname", tooltip("M26").startswith("WLAN „Versteckt“ verbunden, Signal 70 %"), tooltip("M26"))
    has_rows("M27", "{2} Versteckt* ‹192.0.2.37/24› –")
    check("M27 sieben neue Netze", len(m("M27", r"Befehl: netz-neu")) == 7)
    rows27 = menu_rows("M27")
    check("M27 acht Netze und „Weitere Netze …“", "Weitere Netze …" in rows27
          and len(choices(rows27)) == 8, " | ".join(rows27))
    rows28 = menu_rows("M28")
    check("M28 alle zehn Netze", p("M28", r"Menü → Weitere Netze …") and "Weitere Netze …" not in rows28
          and len(choices(rows28)) == 10, " | ".join(rows28))
    check("M28 Menü länger", menu_bottom("m28-alle") > menu_bottom("m27-menue"), f"{menu_bottom('m27-menue')} → {menu_bottom('m28-alle')}")
    rows29 = menu_rows("M29")
    check("M29 Menü folgt bei offenem Menü (Heimnetz weg)", m("M29", r"Befehl: netz-weg Heimnetz")
          and rows29 and not [r for r in rows29 if "Heimnetz" in r], " | ".join(rows29))
    start = (LOGS / "einstellungen-start.txt").read_text() if (LOGS / "einstellungen-start.txt").exists() else ""
    check("M30 Netzwerkeinstellungen über die Aktion netzwerk", p("M30", r"Netzwerkeinstellungen über org\.querencia\.Einstellungen\.desktop")
          and "querencia-einstellungen --netzwerk" in start, start.strip())
    check("M31 entfernt, gespeichert", "network" not in ini("panel.ini.ende"))
elif ART == "spaet":
    check("S00 Rückfall ohne NetworkManager", p("S00", r".") is not None and everywhere(r"Netzwerk: kein NetworkManager, nur Anzeige"))
    icon_is("s00-rueckfall", "WiredConnected")
    tip = tooltip("S00")
    check("S00 Tooltip eth0, kein NetworkManager", tip.startswith("Kabelnetzwerk eth0 verbunden") and tip.endswith("Kein NetworkManager gefunden, nur Anzeige"), tip)
    has_rows("S00", "(Kein NetworkManager gefunden, nur Anzeige)", "[Kabelnetzwerk]")
    check("S01 NetworkManager erscheint", p("S01", r"Netzwerk: NetworkManager gefunden") and p("S01", r"Passwortagent angemeldet"))
    icon_is("s01-attrappe", "WiredConnected")
    check("S01 Tooltip der Attrappe", "enp3s0" in tooltip("S01"), tooltip("S01"))
    has_rows("S01", "{3} Heimnetz 🔒", "Trennen")
    check("S02 NetworkManager weg: Rückfall", p("S02", r"Netzwerk: kein NetworkManager, nur Anzeige"))
    icon_is("s02-rueckfall", "WiredConnected")
    check("S02 Tooltip wieder eth0", tooltip("S02").startswith("Kabelnetzwerk eth0 verbunden"), tooltip("S02"))
    check("S03 NetworkManager ohne WLAN", p("S03", r"Netzwerk: NetworkManager gefunden"))
    rows3 = menu_rows("S03")
    check("S03 Menü nur Kabel", rows3 and "[WLAN-Netze]" not in rows3 and "Trennen" in rows3, " | ".join(rows3))
    icon_is("s04-kabel-getrennt", "WiredDisconnected")
    check("S05 Info-Dialog und Eingabe schließt", p("S05", r"Dialog „Verbindungsinformationen“ geöffnet")
          and dialog_open("s05-info") and not dialog_open("s05-info-zu"))
elif ART == "verschieben":
    def moved(key, pattern):
        check(f"{key} {pattern}", p(key, re.escape(pattern) + r"; gespeichert"), "")
    check("V01 Rechtsklick auf die Uhr öffnet ihr Menü", p("V01", r"Appletmenü von clock-1 geöffnet"))
    moved("V02", "clock-1 verschoben (Left): rechts 2 → rechts 1")
    check("V03 Orte hat jetzt ein Menü", p("V03", r"Appletmenü von places-1 geöffnet"))
    moved("V03", "places-1 verschoben (EndEdge): links 2 → rechts 3")
    moved("V04", "system-1 verschoben (Right): links 2 → links 3")
    check("V05 Ziehen mit der mittleren Taste beginnt", p("V05", r"Ziehen von launcher-1 beginnt \(mittlere Taste, links 2\)"))
    moved("V05", "launcher-1 verschoben (gezogen): links 2 → Mitte 1")
    def busy(shot):
        """Pixel, die nicht Panelfarbe sind, in der Panelmitte (±14 px)."""
        pb = load(shot)
        bg = pixel(pb, 5, 30)
        return sum(1 for x in range(CX - 14, CX + 15) for y in range(3, 29)
                   if sum(abs(a_ - b_) for a_, b_ in zip(pixel(pb, x, y), bg)) > 60)
    before, during = busy("v04-system-rechts"), busy("v05-unterwegs")
    check("V05 live: bei noch gedrückter Taste steht der Starter schon in der Mitte",
          before < 10 and during > 80, f"Mitte vorher {before}, während des Ziehens {during} Pixel")
    check("V06 Verschieben aus dem Menü der Lautstärke", p("V06", r"Verschieben von volume-1 beginnt \(rechts 2\)"))
    moved("V06", "volume-1 verschoben (Verschieben): rechts 2 → Mitte 2")
    check("V07 Rechtsklick bricht Verschieben ab, nichts gespeichert",
          p("V07", r"Verschieben von system-1 abgebrochen") and not p("V07", r"system-1 verschoben"))
    check("V08 mittlerer Klick schaltet stumm, kein Ziehen",
          p("V08", r"Lautstärke: mittlere Taste → stumm umschalten") and not p("V08", r"Ziehen von|verschoben"))
    check("V09 Befund: Alt + Links erreicht das Panel nicht als Alt (kein Ziehen)", not p("V09", r"Ziehen von"))
    check("V10 unteres Panel: Menü des Umschalters", p("V10", r"Appletmenü von workspace-switcher-1 geöffnet"))
    moved("V10", "workspace-switcher-1 verschoben (StartEdge): rechts 1 → links 1")
    moved("V11", "window-list-1 verschoben (gezogen): links 2 → rechts 1")
    check("V12 Dialog zeigt den Bereich der freien Fläche", p("V12", r"Dialog „Zum Panel hinzufügen“ für Panel top geöffnet \(Bereich links\)"))
    check("V13 Bereich im Dialog gewählt", p("V13", r"Dialog „Zum Panel hinzufügen“: Bereich rechts"))
    check("V13 hinzugefügt im gewählten Bereich", p("V13", r"places-2 \(places\) zu Panel top hinzugefügt, Bereich rechts, Position 1; gespeichert"))
    check("V14 Symbolthema neu eingelesen", p("V14", r"Symbolthema neu eingelesen \(Symbol .*w2m-neu\.svg\)"))
    def magenta(shot):
        pb = load(shot)
        return sum(1 for x in range(0, W) for y in range(2, 30)
                   if (lambda c: c[0] > 230 and c[1] < 40 and c[2] > 230)(pixel(pb, x, y)))
    a, b = magenta("v14-vorher"), magenta("v14-nachher")
    check("V14 Starter zeigt das neue Symbol ohne Neustart", a == 0 and b > 150, f"magenta vorher {a}, nachher {b}")
    end = ini("panel.ini.ende")
    for line in ("Start=applications-1;system-1", "Center=launcher-1;volume-1", "End=places-2;clock-1;places-1",
                 "Start=workspace-switcher-1", "End=window-list-1"):
        check(f"panel.ini: {line}", ("\n" + line + "\n") in ("\n" + end), "")
    check("Neustart: Panel liest die Anordnung wieder", len(everywhere(r"Layout aus panel\.ini")) == 2)
    pa, pb_ = load("vor-neustart"), load("nach-neustart")
    diff = [(x, y) for y in list(range(0, 32)) + list(range(H - 32, H)) for x in range(W)
            if pixel(pa, x, y) != pixel(pb_, x, y)]
    xs = [x for x, _ in diff]
    check("Neustart: beide Panels gleich (nur die Uhr darf abweichen)",
          not diff or (max(xs) - min(xs) < 170 and all(y < 32 for _, y in diff)),
          f"{len(diff)} Pixel" + (f", x {min(xs)}–{max(xs)}" if diff else ""))
elif ART == "symbol":
    def magenta(shot):
        pb = load(shot)
        return sum(1 for x in range(0, W) for y in range(2, 30)
                   if (lambda c: c[0] > 230 and c[1] < 40 and c[2] > 230)(pixel(pb, x, y)))
    counts = [magenta(s_) for s_ in ("y00-vorher", "y01-nach-2s", "y01-nach-12s")]
    print("MESSUNG magenta-Pixel vorher / nach 2 s / nach 12 s:", counts)
    check("Y00 vorher kein Symbol", counts[0] == 0, counts[0])
    check("Y01 Symbol nach 2 s da", counts[1] > 150, counts[1])
else:
    sys.exit("Art: rueckfall, attrappe, spaet, verschieben oder symbol")

n, ok = len(results), sum(results)
print(f"{ok}/{n} Prüfungen bestanden")
sys.exit(0 if ok == n else 1)
