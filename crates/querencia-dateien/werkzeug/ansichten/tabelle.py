#!/usr/bin/env python3
"""Ergebnisse isolierter Läufe als Markdown-Tabelle (für das Prüfprotokoll).

Aufruf:  tabelle.py <ergebnis.txt> [<ergebnis.txt> …]
Je Fallnummer (A01, M02 …) eine Zeile: Text, dann je Lauf PASS/FAIL. Ein
Fall, der in einem Lauf mehrfach vorkommt, zählt als PASS nur, wenn jedes
Vorkommen PASS ist; INFO-Zeilen und Zeilen ohne Fallnummer bleiben draußen.
"""

import os
import re
import sys

FALL = re.compile(r"^(PASS|FAIL) ([A-Z]\d\d) (.*)$")


def lesen(pfad):
    faelle = {}
    with open(pfad, encoding="utf-8", errors="replace") as f:
        for zeile in f:
            m = FALL.match(zeile.rstrip("\n"))
            if not m:
                continue
            ergebnis, nr, text = m.groups()
            schluessel = (nr, text)
            alt = faelle.get(schluessel)
            faelle[schluessel] = ergebnis if alt in (None, "PASS") else alt
    return faelle


def main():
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    laeufe = [(os.path.basename(os.path.dirname(p)), lesen(p)) for p in sys.argv[1:]]
    reihen = []
    for _, faelle in laeufe:
        for k in faelle:
            if k not in reihen:
                reihen.append(k)
    kopf = "| Nr. | Fall | " + " | ".join(n for n, _ in laeufe) + " |"
    print(kopf)
    print("|" + "---|" * (2 + len(laeufe)))
    for nr, text in reihen:
        zellen = [f.get((nr, text), "–") for _, f in laeufe]
        print(f"| {nr} | {text.replace('|', '/')} | " + " | ".join(zellen) + " |")


if __name__ == "__main__":
    main()
