#!/usr/bin/env python3
"""Listet im Test-Bus die Einträge des Secret Service mit ihren Attributen
(nie die Geheimnisse selbst), eine Zeile je Eintrag: Pfad und Attribute.
Nur für den Wegwerf-Schlüsselbund der Netzprüfung (hilfen.sh)."""
import gi
gi.require_version("Gio", "2.0")
from gi.repository import Gio, GLib

bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
def eigenschaft(pfad, schnittstelle, name):
    v = bus.call_sync("org.freedesktop.secrets", pfad, "org.freedesktop.DBus.Properties", "Get",
                      GLib.Variant("(ss)", (schnittstelle, name)), None, 0, 5000, None)
    return v.unpack()[0]
for sammlung in eigenschaft("/org/freedesktop/secrets", "org.freedesktop.Secret.Service", "Collections"):
    for eintrag in eigenschaft(sammlung, "org.freedesktop.Secret.Collection", "Items"):
        attr = eigenschaft(eintrag, "org.freedesktop.Secret.Item", "Attributes")
        print(eintrag, " ".join(f"{k}={v}" for k, v in sorted(attr.items())))
