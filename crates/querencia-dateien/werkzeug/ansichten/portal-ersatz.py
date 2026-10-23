#!/usr/bin/env python3
"""Dateien-Prototyp: Kopie aus Desktop 0.2.6-2, geändert nur die Schutzprüfung.
Portal-Ersatz für isolierte Läufe (Benachrichtigungen 0.2.1, CSS-Welle
dunkel): belegt org.freedesktop.portal.Desktop auf dem Wegwerf-Bus und
liefert über org.freedesktop.portal.Settings (Version 2) wie
xdg-desktop-portal-gtk org.freedesktop.appearance (color-scheme, contrast)
und org.gnome.desktop.interface (Thema, Symbole, Schrift …) aus einer
Wertedatei. Fehlt org.gnome.desktop.interface, nimmt GTK 4.22 im
Portal-Betrieb sein eingebautes Thema und nicht settings.ini (gemessen).

Aufruf:  portal-ersatz.py <farbschema> [<werte>]   0 = Standard (hell), 1 = dunkel
<werte>: Zeilen „<schema> <schlüssel> <GVariant-Text>“ wie
`gsettings list-recursively org.gnome.desktop.interface` (nur gelesen
erzeugt); color-scheme darin wird ignoriert und aus <farbschema> gesetzt.
Umschalten im Lauf (sendet SettingChanged wie xdg-desktop-portal):
  gdbus call --session --dest org.freedesktop.portal.Desktop \\
    --object-path /org/freedesktop/portal/desktop \\
    --method org.querencia.PortalErsatz.Setze 1

Nie auf dem Sitzungsbus starten: verweigert, wenn XDG_RUNTIME_DIR nicht in
der Hülle des Prüfrahmens liegt (/tmp/qd-*, QD_HUELLE).
"""

import fnmatch
import os
import sys

import gi

gi.require_version("Gio", "2.0")
from gi.repository import Gio, GLib  # noqa: E402

PFAD = "/org/freedesktop/portal/desktop"
XML = """
<node>
  <interface name="org.freedesktop.portal.Settings">
    <method name="ReadAll">
      <arg type="as" name="namespaces" direction="in"/>
      <arg type="a{sa{sv}}" name="value" direction="out"/>
    </method>
    <method name="Read">
      <arg type="s" name="namespace" direction="in"/>
      <arg type="s" name="key" direction="in"/>
      <arg type="v" name="value" direction="out"/>
    </method>
    <method name="ReadOne">
      <arg type="s" name="namespace" direction="in"/>
      <arg type="s" name="key" direction="in"/>
      <arg type="v" name="value" direction="out"/>
    </method>
    <signal name="SettingChanged">
      <arg type="s" name="namespace"/>
      <arg type="s" name="key"/>
      <arg type="v" name="value"/>
    </signal>
    <property name="version" type="u" access="read"/>
  </interface>
  <interface name="org.querencia.PortalErsatz">
    <method name="Setze">
      <arg type="u" name="farbschema" direction="in"/>
    </method>
  </interface>
</node>
"""

_rt = os.environ.get("XDG_RUNTIME_DIR", "")
if not (_rt.startswith("/tmp/qd-") and os.environ.get("QD_HUELLE")
        and os.path.exists(os.path.join(_rt, ".huelle", "konfig"))):
    sys.exit("portal-ersatz: nur in der Hülle des Prüfrahmens (XDG_RUNTIME_DIR /tmp/qd-*)")

SCHEMA_START = int(sys.argv[1]) if len(sys.argv) > 1 else 0
werte = {"org.freedesktop.appearance": {"contrast": GLib.Variant("u", 0)}}
if len(sys.argv) > 2:
    with open(sys.argv[2], encoding="utf-8") as f:
        for zeile in f:
            teile = zeile.rstrip("\n").split(" ", 2)
            if len(teile) == 3 and teile[1] != "color-scheme":
                werte.setdefault(teile[0], {})[teile[1]] = GLib.Variant.parse(None, teile[2], None, None)


def schema_setzen(schema):
    """Beide Schlüssel wie xdg-desktop-portal(-gtk); liefert die Änderungen."""
    neu = [("org.freedesktop.appearance", "color-scheme", GLib.Variant("u", schema))]
    if "org.gnome.desktop.interface" in werte:
        neu.append(("org.gnome.desktop.interface", "color-scheme",
                    GLib.Variant("s", "prefer-dark" if schema == 1 else "default")))
    for ns, schluessel, wert in neu:
        werte[ns][schluessel] = wert
    return neu


schema_setzen(SCHEMA_START)
verbindung = None


def log(text):
    print(text, flush=True)


def passt(ns, muster):
    return not muster or any(fnmatch.fnmatchcase(ns, m) or ns == m for m in muster)


def aufruf(conn, absender, pfad, schnittstelle, methode, param, antwort):
    if methode == "ReadAll":
        (muster,) = param.unpack()
        ergebnis = {ns: dict(kv) for ns, kv in werte.items() if passt(ns, muster)}
        antwort.return_value(GLib.Variant("(a{sa{sv}})", (ergebnis,)))
        log(f"ReadAll {muster} -> " + ", ".join(f"{ns} ({len(kv)})" for ns, kv in sorted(ergebnis.items())))
    elif methode in ("Read", "ReadOne"):
        ns, schluessel = param.unpack()
        wert = werte.get(ns, {}).get(schluessel)
        if wert is None:
            antwort.return_dbus_error("org.freedesktop.portal.Error.NotFound",
                                      f"{ns} {schluessel} nicht vorhanden")
            return
        if methode == "Read":
            wert = GLib.Variant("v", wert)
        antwort.return_value(GLib.Variant("(v)", (wert,)))
    elif methode == "Setze":
        (schema,) = param.unpack()
        for ns, schluessel, wert in schema_setzen(schema):
            conn.emit_signal(None, PFAD, "org.freedesktop.portal.Settings", "SettingChanged",
                             GLib.Variant("(ssv)", (ns, schluessel, wert)))
        antwort.return_value(None)
        log(f"color-scheme {schema} (SettingChanged gesendet)")


def eigenschaft(conn, absender, pfad, schnittstelle, name):
    return GLib.Variant("u", 2) if name == "version" else None


def bus_da(conn, name):
    global verbindung
    verbindung = conn
    info = Gio.DBusNodeInfo.new_for_xml(XML)
    for iface in info.interfaces:
        conn.register_object(PFAD, iface, aufruf, eigenschaft, None)


schleife = GLib.MainLoop()
Gio.bus_own_name(Gio.BusType.SESSION, "org.freedesktop.portal.Desktop",
                 Gio.BusNameOwnerFlags.NONE, bus_da,
                 lambda c, n: log(f"Name {n} belegt, color-scheme {SCHEMA_START}"),
                 lambda c, n: (log(f"Name {n} verloren"), schleife.quit()))
schleife.run()
