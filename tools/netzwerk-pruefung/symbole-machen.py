#!/usr/bin/env python3
"""G2-M: erzeugt die Symbole des Netzwerk-Applets (Querencia-Stil: Grau mit
#F07746, flächig, 24x24 bzw. 16x16) in crates/querencia-netzwerk/data/symbole.

  python3 -I tools/netzwerk-pruefung/symbole-machen.py
"""
import math
import os

ZIEL = os.path.join(os.path.dirname(os.path.abspath(__file__)), "../../crates/querencia-netzwerk/data/symbole")
HEAD = '<?xml version="1.0" encoding="UTF-8"?>\n<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">\n'
H16 = '<?xml version="1.0" encoding="UTF-8"?>\n<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16">\n'
TAIL = '</svg>\n'
OR = "#F07746"; GR = "#5a5a5a"; DK = "#3c3c3c"; LG = "#c8c8c8"; MG = "#9a9a9a"


def arc(r, color, cx=12, cy=19.5, w=2.6):
    dx = r * math.cos(math.radians(45)); dy = r * math.sin(math.radians(45))
    return (f'  <path d="M {cx-dx:.2f} {cy-dy:.2f} A {r} {r} 0 0 1 {cx+dx:.2f} {cy-dy:.2f}" '
            f'fill="none" stroke="{color}" stroke-width="{w}" stroke-linecap="round"/>\n')


def fan(active, dot):
    s = ''
    for i, r in enumerate((5, 9.5, 14)):
        s += arc(r, OR if i < active else LG)
    return s + f'  <circle cx="12" cy="19.5" r="2.3" fill="{dot}"/>\n'


def badge(kind):
    s = f'  <circle cx="18" cy="18" r="5.6" fill="#ffffff"/>\n  <circle cx="18" cy="18" r="4.6" fill="{OR}"/>\n'
    if kind == "x":
        return s + '  <path d="M 16 16 L 20 20 M 20 16 L 16 20" stroke="#ffffff" stroke-width="1.6" stroke-linecap="round"/>\n'
    return s + ('  <path d="M 18 15.4 L 18 18.6" stroke="#ffffff" stroke-width="1.7" stroke-linecap="round"/>\n'
                '  <circle cx="18" cy="20.5" r="0.95" fill="#ffffff"/>\n')


def plug(body, edge, contacts):
    return (f'  <rect x="10" y="16" width="4" height="7" fill="{body}"/>\n'
            f'  <rect x="4.5" y="3.5" width="15" height="13" rx="1.5" fill="{body}" stroke="{edge}" stroke-width="1"/>\n'
            f'  <rect x="9" y="13" width="6" height="3.5" fill="{edge}"/>\n'
            + ''.join(f'  <rect x="{x}" y="5.5" width="1.6" height="4.5" fill="{contacts}"/>\n' for x in (6.6, 9.4, 12.2, 15)))


files = {
    "netzwerk-kabel.svg": plug(GR, DK, OR),
    "netzwerk-kabel-getrennt.svg": plug(MG, "#7a7a7a", "#dddddd") + badge("x"),
    "netzwerk-wlan-aus.svg": fan(0, LG)
    + '  <path d="M 4.5 3.5 L 19.5 21" stroke="#ffffff" stroke-width="5" stroke-linecap="round"/>\n'
    + f'  <path d="M 4.5 3.5 L 19.5 21" stroke="{OR}" stroke-width="2.6" stroke-linecap="round"/>\n',
    "netzwerk-verbindet.svg": f'  <circle cx="12" cy="12" r="8" fill="none" stroke="{LG}" stroke-width="3"/>\n'
    + f'  <path d="M 12 4 A 8 8 0 0 1 19.93 13.04" fill="none" stroke="{OR}" stroke-width="3" stroke-linecap="round"/>\n'
    + f'  <path d="M 4.07 10.96 A 8 8 0 0 1 6.34 6.34" fill="none" stroke="{OR}" stroke-width="3" stroke-linecap="round"/>\n'
    + f'  <circle cx="12" cy="12" r="2.6" fill="{GR}"/>\n',
    "netzwerk-kein-netz.svg": fan(0, MG) + badge("x"),
    "netzwerk-kein-dienst.svg": f'  <rect x="2.5" y="3.5" width="17" height="12" rx="1" fill="#e6e6e6" stroke="{GR}" stroke-width="1.6"/>\n'
    + f'  <rect x="9" y="15.5" width="4" height="3" fill="{GR}"/>\n  <rect x="6" y="18.5" width="10" height="1.8" rx="0.6" fill="{GR}"/>\n'
    + badge("!"),
}
for i in range(4):
    files[f"netzwerk-wlan-{i}.svg"] = fan(i, OR)


def fan_light(active):
    """Menu variant on the orange highlight: white arcs, inactive ones faint."""
    s = ''
    for i, r in enumerate((5, 9.5, 14)):
        s += arc(r, "#ffffff" if i < active else "#fbd3c2")
    return s + '  <circle cx="12" cy="19.5" r="2.3" fill="#ffffff"/>\n'


for i in range(4):
    files[f"netzwerk-wlan-{i}-hell.svg"] = fan_light(i)
for name, body in files.items():
    with open(os.path.join(ZIEL, name), "w") as f:
        f.write(HEAD + body + TAIL)
with open(os.path.join(ZIEL, "netzwerk-schloss.svg"), "w") as f:
    f.write(H16
            + f'  <path d="M 5 7.5 V 5 A 3 3 0 0 1 11 5 V 7.5" fill="none" stroke="{GR}" stroke-width="1.8"/>\n'
            + f'  <rect x="3" y="7" width="10" height="7.5" rx="1" fill="{OR}" stroke="#b36c4e" stroke-width="1"/>\n'
            + f'  <rect x="7.2" y="9.3" width="1.6" height="3" rx="0.8" fill="{DK}"/>\n' + TAIL)
for name, check in (("netzwerk-haken-an.svg", True), ("netzwerk-haken-aus.svg", False)):
    body = f'  <rect x="1.5" y="1.5" width="13" height="13" rx="1.5" fill="#ffffff" stroke="{GR}" stroke-width="1"/>\n'
    if check:
        body += f'  <path d="M 4 8.2 L 7 11 L 12.2 4.8" fill="none" stroke="{OR}" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"/>\n'
    with open(os.path.join(ZIEL, name), "w") as f:
        f.write(H16 + body + TAIL)
print("Symbole geschrieben:", ZIEL)
