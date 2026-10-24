#!/usr/bin/env python3
"""Prüfrahmen Querencia Dateien: zeigt, was GIO an Laufwerken sieht.

Aufruf (nur in der Hülle): gio-sicht.py [PFAD …]
Ausgabe je Zeile, sortiert:
  monitor <Klasse>
  volume <Name> can_mount=… mounted=… can_eject=…
  mount <Name> <Pfad> can_unmount=… readonly=…
  pfad <Pfad> filesystem=<id::filesystem> fstype=<filesystem::type>
"""

import os
import sys

import gi

gi.require_version("Gio", "2.0")
from gi.repository import Gio  # noqa: E402


def nur_lesend(datei):
    try:
        info = datei.query_filesystem_info("filesystem::readonly", None)
        return info.get_attribute_boolean("filesystem::readonly")
    except Exception as fehler:  # noqa: BLE001 - Anzeige, kein Abbruch
        return f"?({fehler.__class__.__name__})"


def main(pfade):
    if not os.environ.get("QD_HUELLE"):
        print("gio-sicht.py: nur in huelle.sh aufrufen", file=sys.stderr)
        return 2
    monitor = Gio.VolumeMonitor.get()
    zeilen = []
    for v in monitor.get_volumes():
        zeilen.append(f"volume {v.get_name()} can_mount={v.can_mount()} "
                      f"mounted={v.get_mount() is not None} can_eject={v.can_eject()}")
    for m in monitor.get_mounts():
        wurzel = m.get_root()
        zeilen.append(f"mount {m.get_name()} {wurzel.get_path()} can_unmount={m.can_unmount()} "
                      f"readonly={nur_lesend(wurzel)}")
    for pfad in pfade:
        datei = Gio.File.new_for_path(pfad)
        info = datei.query_info("id::filesystem", Gio.FileQueryInfoFlags.NONE, None)
        fs = datei.query_filesystem_info("filesystem::type", None)
        zeilen.append(f"pfad {pfad} filesystem={info.get_attribute_string('id::filesystem')} "
                      f"fstype={fs.get_attribute_string('filesystem::type')}")
    print(f"monitor {type(monitor).__name__}")
    for z in sorted(zeilen):
        print(z)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
