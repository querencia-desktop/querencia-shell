#!/usr/bin/env python3
"""G2-AK: ersetzt <ziel> atomar durch den Inhalt von <quelle>, wie GLib und
Querencia Einstellungen schreiben (neue Datei daneben, dann umbenennen).

  ersetzen.py <quelle> <ziel>
"""
import os
import sys

quelle, ziel = sys.argv[1], sys.argv[2]
with open(quelle, "rb") as f:
    daten = f.read()
neu = f"{ziel}.{os.getpid()}.neu"
with open(neu, "wb") as f:
    f.write(daten)
    f.flush()
    os.fsync(f.fileno())
os.replace(neu, ziel)
