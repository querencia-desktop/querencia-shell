#!/usr/bin/env python3
"""G2-Y: fasst wettlauf.jsonl mehrerer Profile zusammen.
  zusammenfassen.py <profil-ordner> …"""
import json
import os
import statistics
import sys

for ordner in sys.argv[1:]:
    pfad = os.path.join(ordner, "logs", "wettlauf.jsonl")
    if not os.path.exists(pfad):
        print(f"{os.path.basename(ordner)}: keine Ergebnisse")
        continue
    z = [json.loads(line) for line in open(pfad)]
    name = os.path.basename(ordner)
    if not z:
        print(f"{name}: leer")
        continue
    art = z[0]["art"]
    beendet = [x["beendet_nach_s"] * 1000 for x in z]
    threads = [x["threads_bei_beendet"] for x in z]
    teil = [f"{name} ({art}): {len(z)} Durchgänge",
            f"„beendet“ nach {min(beendet):.0f}–{max(beendet):.0f} ms, dann noch {min(threads)}–{max(threads)} Threads"]
    if art == "messen":
        frei = [x["name_frei_nach_beendet_s"] * 1000 for x in z if x.get("name_frei_nach_beendet_s") is not None]
        teil.append(f"Name danach noch belegt {min(frei):.0f}–{max(frei):.0f} ms (Median {statistics.median(frei):.0f})")
    else:
        ok = sum(x["neu"] == "läuft" for x in z)
        fr = sum(x.get("failed_to_register", False) for x in z)
        teil.append(f"neues Panel läuft {ok}/{len(z)}, „Failed to register“ {fr}")
        w = [x["gewartet_s"] * 1000 for x in z if x.get("gewartet_s") is not None]
        if w:
            teil.append(f"gewartet {min(w):.0f}–{max(w):.0f} ms")
        andere = [f"{x['i']}: {x['neu']}" for x in z if x["neu"] != "läuft" and not x.get("failed_to_register")]
        if andere:
            teil.append("sonst: " + "; ".join(andere))
    if any(x.get("name_haengt") for x in z):
        teil.append("Name hing nach dem Durchgang")
    print("; ".join(teil))
