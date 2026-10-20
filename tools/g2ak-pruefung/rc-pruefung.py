#!/usr/bin/env python3
"""G2-AK: rc.xml für die isolierte Prüfung aus der Vorgabe.

  rc-pruefung.py <vorgabe-rc.xml> > rc.xml

Übernimmt die Vorgabe (Arbeitsflächen, Kürzel, Fensterregeln) und entfernt
nur die Kürzel mit Execute (Druck, Strg+Alt+L, Strg+Alt+Entf, Super+?): sie
starten Programme der echten Installation unter ~/opt/querencia/bin.
"""
import sys
import xml.etree.ElementTree as ET

baum = ET.parse(sys.argv[1])
tastatur = baum.getroot().find("keyboard")
for kuerzel in list(tastatur.findall("keybind")):
    if any(a.get("name") == "Execute" for a in kuerzel.iter("action")):
        tastatur.remove(kuerzel)
ET.indent(baum)
sys.stdout.write('<?xml version="1.0" encoding="UTF-8"?>\n')
baum.write(sys.stdout, encoding="unicode")
sys.stdout.write("\n")
