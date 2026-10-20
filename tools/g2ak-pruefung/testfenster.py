#!/usr/bin/env python3
"""G2-AK: ein GTK-4-Fenster mit fester App-ID für die Fensterlisten-Prüfung.

  testfenster.py <app-id> <titel> [<farbe>]

Läuft nur in der isolierten Sitzung (über qeingabe „run“); die App-ID
erscheint in der Logzeile der Fensterliste, die Farbe füllt das Fenster.
"""
import sys

import gi

gi.require_version("Gtk", "4.0")
from gi.repository import Gtk  # noqa: E402

APP_ID, TITEL = sys.argv[1], sys.argv[2]
FARBE = sys.argv[3] if len(sys.argv) > 3 else "#5a8fd0"


def aktivieren(app):
    fenster = Gtk.ApplicationWindow(application=app, title=TITEL)
    fenster.set_default_size(420, 300)
    css = Gtk.CssProvider()
    css.load_from_data(f"window {{ background-color: {FARBE}; }}".encode())
    fenster.get_style_context().add_provider(css, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)
    fenster.present()


app = Gtk.Application(application_id=APP_ID)
app.connect("activate", aktivieren)
app.run(None)
