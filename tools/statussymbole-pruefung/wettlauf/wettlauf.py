#!/usr/bin/env python3
"""G2-Y: Panel-Tausch wie tausch.py nachstellen (läuft nur in isoliert.py).

  wettlauf.py <alt-binary> <neu-binary> <anzahl> tausch|warten|messen <log-ordner>

Je Durchgang: altes Panel starten und auf „Panel(s)“ warten, dann wie
tausch.py SIGTERM und im 0,1-s-Takt prüfen, ob es beendet (/proc weg oder
Zombie) ist.
- tausch:  neues Panel sofort starten (genau wie tausch.py).
- warten:  erst warten, bis NameHasOwner(org.example.Querencia.Panel) false
           meldet (Zeitlimit 5 s), dann starten (vorgeschlagene Korrektur).
- messen:  nichts starten; messen, wie lange der Name nach „beendet“ noch
           belegt ist und wie viele Threads des alten Prozesses noch leben.
- patch:   wie tausch, aber mit den Funktionen der gepatchten tausch.py
           (WL_TAUSCH): gone, wait_name_released, healthy (Log nach Bytes).
           Nur start() (systemd-run) und running_panel() bleiben außen vor.
Ergebnis je Durchgang als JSON-Zeile in <log-ordner>/wettlauf.jsonl.
"""
import json
import os
import signal
import subprocess
import sys
import time

import gi

gi.require_version("Gio", "2.0")
from gi.repository import Gio, GLib  # noqa: E402

NAME = "org.example.Querencia.Panel"
ALT, NEU, N, ART, OUT = sys.argv[1], sys.argv[2], int(sys.argv[3]), sys.argv[4], sys.argv[5]
rt = os.environ.get("XDG_RUNTIME_DIR", "")
assert "/nachweis/gui/rt/" in rt and not rt.startswith("/run/user/"), f"nicht isoliert: {rt}"
BUS = Gio.bus_get_sync(Gio.BusType.SESSION, None)
TAUSCH = None
if ART == "patch":
    import importlib.util
    spec = importlib.util.spec_from_file_location("tausch", os.environ["WL_TAUSCH"])
    TAUSCH = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(TAUSCH)


def has_owner():
    r = BUS.call_sync("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus",
                      "NameHasOwner", GLib.Variant("(s)", (NAME,)), GLib.VariantType("(b)"),
                      Gio.DBusCallFlags.NONE, 1000, None)
    return r.unpack()[0]


def gone(pid):  # wörtlich aus tausch.py
    try:
        return open(f"/proc/{pid}/stat").read().split()[2] == "Z"
    except FileNotFoundError:
        return True


def threads(pid):
    try:
        return len(os.listdir(f"/proc/{pid}/task"))
    except FileNotFoundError:
        return 0


def start(binary, logpath):
    out = open(logpath, "a")
    out.write(f"\n=== {time.strftime('%T')} Start {binary} ===\n")
    out.flush()
    return subprocess.Popen([binary], stdin=subprocess.DEVNULL, stdout=out, stderr=subprocess.STDOUT)


def ab(logpath, mark):
    """Log ab Byte mark (getsize zählt Bytes, das Log enthält Umlaute)."""
    with open(logpath, "rb") as f:
        f.seek(mark)
        return f.read().decode(errors="replace")


def aufbau(proc, logpath, mark, sekunden=20):
    ende = time.time() + sekunden
    while time.time() < ende:
        text = ab(logpath, mark)
        if "Panel(s)" in text:
            return "läuft", text
        if proc.poll() is not None:
            return f"beendet (Exit {proc.returncode})", text
        time.sleep(0.05)
    return "kein Aufbau", ab(logpath, mark)


def frei_warten(sekunden=5.0):
    t = time.monotonic()
    while has_owner():
        if time.monotonic() - t > sekunden:
            return None
        time.sleep(0.005)
    return time.monotonic() - t


def beenden(proc):
    if proc and proc.poll() is None:
        proc.send_signal(signal.SIGTERM)
    if proc:
        proc.wait(10)


ergebnisse = open(os.path.join(OUT, "wettlauf.jsonl"), "a")
for i in range(N):
    a_log = os.path.join(OUT, "alt.log")
    mark = os.path.getsize(a_log) if os.path.exists(a_log) else 0
    alt = start(ALT, a_log)
    zustand, _ = aufbau(alt, a_log, mark)
    if zustand != "läuft":
        print(f"{i}: altes Panel {zustand}", flush=True)
        beenden(alt)
        continue
    time.sleep(1.0)
    t0 = time.monotonic()
    os.kill(alt.pid, signal.SIGTERM)
    for _ in range(50):  # wie tausch.py
        if (TAUSCH.gone if TAUSCH else gone)(alt.pid):
            break
        time.sleep(0.1)
    t_gone = time.monotonic() - t0
    rest_threads = threads(alt.pid)
    zeile = {"i": i, "art": ART, "alt": os.path.basename(os.path.dirname(ALT)) or ALT,
             "neu": NEU, "beendet_nach_s": round(t_gone, 3), "threads_bei_beendet": rest_threads}
    if ART == "messen":
        frei = frei_warten()
        zeile["name_frei_nach_beendet_s"] = None if frei is None else round(frei, 4)
        beenden(alt)
    else:
        if ART == "warten":
            frei = frei_warten()
            zeile["gewartet_s"] = None if frei is None else round(frei, 4)
        if ART == "patch":
            frei = TAUSCH.wait_name_released(dict(os.environ))
            zeile["gewartet_s"] = None if frei is None else round(frei, 4)
        n_log = os.path.join(OUT, "neu.log")
        mark = os.path.getsize(n_log) if os.path.exists(n_log) else 0
        neu = start(NEU, n_log)
        if ART == "patch":
            ok, warum = TAUSCH.healthy(neu, n_log, mark)
            zustand, text = ("läuft" if ok else warum), ab(n_log, mark)
        else:
            zustand, text = aufbau(neu, n_log, mark)
        zeile["neu"] = zustand
        zeile["failed_to_register"] = "Failed to register" in text
        if zustand != "läuft":
            zeile["log"] = text.strip().splitlines()[-1:] if text.strip() else []
        alt.wait(10)
        beenden(neu)
    if frei_warten() is None:
        zeile["name_haengt"] = True
    print(json.dumps(zeile, ensure_ascii=False), file=ergebnisse, flush=True)
    print(json.dumps(zeile, ensure_ascii=False), flush=True)
