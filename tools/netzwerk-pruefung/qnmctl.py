#!/usr/bin/env python3
"""G2-M: steuert die NetworkManager-Attrappe qnm.py (Testbefehle, nur Prüfung).

  python3 -I qnmctl.py kabel ab | kabel an | signal Heimnetz 30 | netz-neu Gast 60 offen
                       netz-weg Gast | wlan-schalter aus | zustand | beenden

Spricht nur den Bus aus DBUS_SYSTEM_BUS_ADDRESS an, und nur, wenn er im
isolierten Laufzeitordner liegt.
"""
import os
import sys

import gi

gi.require_version("Gio", "2.0")
from gi.repository import Gio, GLib  # noqa: E402

address = os.environ.get("DBUS_SYSTEM_BUS_ADDRESS", "")
rt = os.environ.get("XDG_RUNTIME_DIR", "")
if "/nachweis/gui/rt/g2m-" not in rt or rt not in address:
    sys.exit("qnmctl: nur mit eigenem Bus im isolierten Laufzeitordner")
bus = Gio.DBusConnection.new_for_address_sync(
    address, Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, None, None)
try:
    reply = bus.call_sync("org.freedesktop.NetworkManager", "/org/querencia/Attrappe", "org.querencia.Attrappe",
                          "Befehl", GLib.Variant("(s)", (" ".join(sys.argv[1:]),)), GLib.VariantType("(s)"),
                          Gio.DBusCallFlags.NO_AUTO_START, 5000, None)
except GLib.Error as e:
    sys.exit(f"qnmctl: {e.message}")
print(reply.unpack()[0])
