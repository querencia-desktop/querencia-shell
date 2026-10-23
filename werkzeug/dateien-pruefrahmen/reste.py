#!/usr/bin/env python3
"""Prüfrahmen Querencia Dateien: Reste eines Hüllenlaufs finden.

Aufruf:
  reste.py vorher                                   -> JSON-Schnappschuss (wsdd, fuse.portal)
  reste.py nachher --laufzeit R --profil P [--vorher DATEI] [--json DATEI]

Liest nur /proc und /proc/self/mountinfo, startet keine Prozesse und
beendet nichts. Ein Rest ist
  - ein Prozess mit XDG_RUNTIME_DIR=R in /proc/<pid>/environ,
  - ein Prozess, dessen Befehlszeile R oder P enthält (environ ist bei
    AppArmor-begrenzten Prozessen wie wsdd nicht lesbar),
  - eine Einhängung unter R oder P,
  - eine fuse.portal-Einhängung, die vorher nicht da war (nur mit --vorher).
Ausgenommen sind reste.py selbst und seine Vorfahren (die Hülle).
Neue wsdd-Prozesse ohne Bezug zu R werden als Warnung gemeldet (anderer
Worker), nicht als Rest.
Exit: 0 sauber, 1 Reste, 2 Aufruffehler.
"""

import json
import os
import sys


def lesen(pfad, binaer=False):
    try:
        with open(pfad, "rb") as f:
            daten = f.read()
    except OSError:
        return None
    return daten if binaer else daten.decode("utf-8", "replace")


def vorfahren():
    """Eigene PID und alle Vorfahren (die aufrufende Hülle trägt R/P in argv)."""
    pids, pid = set(), os.getpid()
    while pid > 1 and pid not in pids:
        pids.add(pid)
        status = lesen(f"/proc/{pid}/status") or ""
        ppid = [z.split()[1] for z in status.splitlines() if z.startswith("PPid:")]
        pid = int(ppid[0]) if ppid else 0
    return pids


def prozesse():
    ausnahmen = vorfahren()
    for name in os.listdir("/proc"):
        if not name.isdigit() or int(name) in ausnahmen:
            continue
        cmd = lesen(f"/proc/{name}/cmdline", True)
        if cmd is None:
            continue
        yield int(name), cmd.split(b"\0")


def ist_wsdd(pid, argv):
    comm = (lesen(f"/proc/{pid}/comm") or "").strip()
    return comm == "wsdd" or any(a.endswith(b"/wsdd") for a in argv[:2])


def einhaengungen():
    text = lesen("/proc/self/mountinfo") or ""
    for zeile in text.splitlines():
        teile = zeile.split(" ")
        try:
            trenner = teile.index("-")
        except ValueError:
            continue
        ziel = teile[4].encode().decode("unicode_escape")
        yield ziel, teile[trenner + 1]


def schnappschuss():
    return {
        "wsdd": sorted(pid for pid, argv in prozesse() if ist_wsdd(pid, argv)),
        "portal": sorted(z for z, typ in einhaengungen() if typ == "fuse.portal"),
    }


def unter(pfad, wurzeln):
    return any(pfad == w or pfad.startswith(w.rstrip("/") + "/") for w in wurzeln)


def nachher(laufzeit, profil, vorher):
    wurzeln = [w for w in (laufzeit, profil) if w]
    rb = [w.encode() for w in wurzeln]
    marke = f"XDG_RUNTIME_DIR={laufzeit}".encode() if laufzeit else None
    reste, warnungen = [], []
    for pid, argv in prozesse():
        befehl = b" ".join(argv).decode("utf-8", "replace")[:200]
        env = lesen(f"/proc/{pid}/environ", True)
        if marke and env is not None and marke in env.split(b"\0"):
            reste.append(f"Prozess {pid} mit eigenem XDG_RUNTIME_DIR: {befehl}")
            continue
        if any(r in a for a in argv for r in rb):
            reste.append(f"Prozess {pid} mit Bezug zum Lauf: {befehl}")
            continue
        if ist_wsdd(pid, argv) and pid not in vorher.get("wsdd", []):
            warnungen.append(f"neuer wsdd {pid} ohne Bezug zu diesem Lauf: {befehl}")
    for ziel, typ in einhaengungen():
        if unter(ziel, wurzeln):
            reste.append(f"Einhängung {ziel} ({typ})")
        elif typ == "fuse.portal" and "portal" in vorher and ziel not in vorher["portal"]:
            reste.append(f"neue fuse.portal-Einhängung {ziel}")
    return reste, warnungen


def main(argv):
    if len(argv) < 2 or argv[1] not in ("vorher", "nachher"):
        print(__doc__, file=sys.stderr)
        return 2
    if argv[1] == "vorher":
        print(json.dumps(schnappschuss()))
        return 0
    opts = {"--laufzeit": "", "--profil": "", "--vorher": "", "--json": ""}
    rest = argv[2:]
    while rest:
        if rest[0] not in opts or len(rest) < 2:
            print(f"reste.py: unbekannt: {rest[0]}", file=sys.stderr)
            return 2
        opts[rest[0]] = rest[1]
        rest = rest[2:]
    if not opts["--laufzeit"] and not opts["--profil"]:
        print("reste.py: --laufzeit oder --profil fehlt", file=sys.stderr)
        return 2
    vorher = {}
    if opts["--vorher"]:
        vorher = json.loads(lesen(opts["--vorher"]) or "{}")
    reste, warnungen = nachher(opts["--laufzeit"], opts["--profil"], vorher)
    jetzt = schnappschuss()
    bericht = {
        "reste": reste,
        "warnungen": warnungen,
        "wsdd_vorher": len(vorher.get("wsdd", [])),
        "wsdd_nachher": len(jetzt["wsdd"]),
        "portal_vorher": len(vorher.get("portal", [])),
        "portal_nachher": len(jetzt["portal"]),
    }
    if opts["--json"]:
        with open(opts["--json"], "w") as f:
            json.dump(bericht, f, ensure_ascii=False, indent=1)
    for r in reste:
        print(f"huelle: REST {r}", file=sys.stderr)
    for w in warnungen:
        print(f"huelle: Warnung {w}", file=sys.stderr)
    return 1 if reste else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
