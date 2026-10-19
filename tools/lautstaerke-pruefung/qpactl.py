#!/usr/bin/env python3
"""G2-J: pactl-artige Steuerung des Testservers qpulse.py (nur isoliert).

  qpactl.py info
  qpactl.py zustand                                   (JSON)
  qpactl.py list sinks short
  qpactl.py load-module module-null-sink sink_name=N [sink_properties=device.description=B]
  qpactl.py unload-module <nr>
  qpactl.py set-default-sink <name>
  qpactl.py set-sink-volume <name> <prozent>%
  qpactl.py set-sink-mute <name> 1|0
  qpactl.py spiele <anwendung> [titel]               (Wiedergabestrom anlegen)
  qpactl.py nimm-auf <anwendung> [titel]             (Aufnahmestrom anlegen)
  qpactl.py beenden

Die Befehle mit pactl-Namen verhalten sich wie pactl gegenüber einem echten
Server (module-null-sink legt ein Ausgabegerät samt Monitor an).
"""
import json
import os
import socket
import sys

RT_MARK = "/querencia-runtime-wt-lautstaerke/nachweis/gui/rt/g2j-"


def ask(request):
    rt = os.environ.get("XDG_RUNTIME_DIR", "")
    if rt.startswith("/run/user/") or RT_MARK not in rt:
        sys.exit("qpactl: nur in der isolierten Prüfumgebung")
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as s:
        s.connect(os.path.join(rt, "qpulse.ctl"))
        s.sendall(json.dumps(request).encode() + b"\n")
        return json.loads(s.makefile("r").readline())


def main(args):
    if not args:
        sys.exit(__doc__)
    cmd = args[0]
    if cmd == "info":
        a = ask({"op": "info"})
        print(f"Server: {a['server']}\nStandard-Ziel: {a['default_sink']}\nStandard-Quelle: {a['default_source']}")
        return 0
    if cmd == "zustand":
        print(json.dumps(ask({"op": "state"}), ensure_ascii=False))
        return 0
    if args[:3] == ["list", "sinks", "short"]:
        for d in ask({"op": "state"})["sinks"]:
            print(f"{d['index']}\t{d['name']}\t{d['percent']}%\t{'stumm' if d['muted'] else 'laut'}")
        return 0
    if cmd == "load-module" and args[1:2] == ["module-null-sink"]:
        opts = dict(a.split("=", 1) for a in args[2:])
        desc = opts.get("sink_properties", "").removeprefix("device.description=") or "Null Output"
        a = ask({"op": "load-null-sink", "name": opts.get("sink_name", "null"), "description": desc.strip('"')})
        print(a["module"])
        return 0
    if cmd == "unload-module":
        a = ask({"op": "unload-module", "module": int(args[1])})
    elif cmd == "set-default-sink":
        a = ask({"op": "set-default-sink", "name": args[1]})
    elif cmd == "set-sink-volume":
        a = ask({"op": "set-sink-volume", "name": args[1], "percent": int(args[2].rstrip("%"))})
    elif cmd == "set-sink-mute":
        a = ask({"op": "set-sink-mute", "name": args[1], "muted": args[2] in ("1", "true", "yes")})
    elif cmd in ("spiele", "nimm-auf"):
        a = ask({"op": "play" if cmd == "spiele" else "record", "app": args[1],
                 "title": " ".join(args[2:]), "percent": 80})
        print(a.get("index"))
        return 0
    elif cmd == "beenden":
        a = ask({"op": "quit"})
    else:
        sys.exit(f"qpactl: unbekannt: {' '.join(args)}")
    if "error" in a:
        sys.exit(f"qpactl: {a['error']}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
