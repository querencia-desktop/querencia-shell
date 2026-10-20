#!/usr/bin/env python3
"""G2-AL: rc.xml für die isolierte Prüfung aus der Vorgabe.

  rc-pruefung-g2al.py <vorgabe-rc.xml> > rc.xml

Wie rc-pruefung.py (G2-AK): übernimmt die Vorgabe und entfernt die Kürzel
mit Execute, die Programme der echten Installation starten. Behalten wird
nur Super+D: gdbus an den Busnamen des Panels, im isolierten Lauf also an
das Prüfpanel auf dem eigenen Bus. Fehlt Super+D, bricht es ab.
"""
import sys
import xml.etree.ElementTree as ET

SUPER_D = ("gdbus call --session --dest org.example.Querencia.Panel "
           "--object-path /org/example/Querencia/Panel "
           "--method org.querencia.Panel.ArbeitsflaecheUmschalten")

baum = ET.parse(sys.argv[1])
tastatur = baum.getroot().find("keyboard")
behalten = 0
for kuerzel in list(tastatur.findall("keybind")):
    befehle = [a.get("command") for a in kuerzel.iter("action") if a.get("name") == "Execute"]
    if not befehle:
        continue
    if kuerzel.get("key") == "W-d" and befehle == [SUPER_D]:
        behalten += 1
        continue
    tastatur.remove(kuerzel)
if behalten != 1:
    sys.exit("Super+D fehlt in der Vorgabe")
ET.indent(baum)
sys.stdout.write('<?xml version="1.0" encoding="UTF-8"?>\n')
baum.write(sys.stdout, encoding="unicode")
sys.stdout.write("\n")
