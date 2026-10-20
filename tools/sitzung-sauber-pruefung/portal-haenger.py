#!/usr/bin/env python3
"""Attrappe eines stehenden Settings-Portals (QD-062): belegt
org.freedesktop.portal.Desktop auf dem Sitzungsbus und beantwortet keinen
Aufruf, wie der xdg-desktop-portal der alten Sitzung am 20.10.2026.
Nur für isolierte Busse (dbus-run-session)."""
import sys
from gi.repository import Gio, GLib

XML = """<node><interface name="org.freedesktop.portal.Settings">
<method name="ReadAll"><arg type="as" direction="in"/><arg type="a{sa{sv}}" direction="out"/></method>
<method name="Read"><arg type="s" direction="in"/><arg type="s" direction="in"/><arg type="v" direction="out"/></method>
<method name="ReadOne"><arg type="s" direction="in"/><arg type="s" direction="in"/><arg type="v" direction="out"/></method>
<property name="version" type="u" access="read"/>
</interface></node>"""
offen = []  # Aufrufe ohne Antwort am Leben halten


def aufruf(conn, sender, pfad, iface, methode, params, inv):
    print(f"haenger: {methode} von {sender} (keine Antwort)", flush=True)
    offen.append(inv)


def eigenschaft(conn, sender, pfad, iface, name):
    print(f"haenger: Eigenschaft {name} von {sender} (blockiert)", flush=True)
    GLib.usleep(60 * 1000 * 1000)
    return GLib.Variant("u", 2)


def bus_da(conn, name):
    info = Gio.DBusNodeInfo.new_for_xml(XML).interfaces[0]
    conn.register_object("/org/freedesktop/portal/desktop", info, aufruf, eigenschaft, None)


def name_da(conn, name):
    print("haenger: Name belegt", flush=True)


Gio.bus_own_name(Gio.BusType.SESSION, "org.freedesktop.portal.Desktop",
                 Gio.BusNameOwnerFlags.NONE, bus_da, name_da, lambda c, n: sys.exit(3))
GLib.MainLoop().run()
