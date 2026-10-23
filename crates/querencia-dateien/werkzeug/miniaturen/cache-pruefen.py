#!/usr/bin/python3 -I
"""Prüft Miniaturen im freedesktop-Cache gegen die Spezifikation (Welle C2).

Aufruf: cache-pruefen.py CACHE-WURZEL DATEI …
Für jede DATEI: Name = MD5 der URI, Thumb::URI = URI, Thumb::MTime = mtime,
lange Kante ≤ 128 (normal) bzw. 256 (large), Rechte 0600, Ordner 0700.
Ausgabe je Datei: OK|FEHLT|FEHL-EINTRAG|FALSCH Grund; Exit 0 nur, wenn alle OK.
Mit --fehl: die Dateien müssen einen Eintrag unter fail/querencia-dateien haben.
Mit --keine: die Dateien dürfen keinen Eintrag haben.
"""
import hashlib
import os
import pathlib
import struct
import sys


def tags(daten):
    out, pos = {}, 8
    breite = hoehe = 0
    while pos + 8 <= len(daten):
        laenge, art = struct.unpack(">I4s", daten[pos:pos + 8])
        inhalt = daten[pos + 8:pos + 8 + laenge]
        if art == b"IHDR":
            breite, hoehe = struct.unpack(">II", inhalt[:8])
        if art == b"tEXt" and b"\0" in inhalt:
            k, v = inhalt.split(b"\0", 1)
            out[k.decode()] = v.decode()
        pos += 12 + laenge
        if art == b"IEND":
            break
    return out, breite, hoehe


def main():
    args = sys.argv[1:]
    modus = "ok"
    for o in ("--fehl", "--keine"):
        if o in args:
            args.remove(o)
            modus = o[2:]
    wurzel = pathlib.Path(args.pop(0))
    gut = True
    for d in args:
        p = pathlib.Path(d).resolve()
        uri = p.as_uri()
        name = hashlib.md5(uri.encode()).hexdigest() + ".png"
        mtime = int(p.stat().st_mtime)
        kandidaten = [(wurzel / e / name, k) for e, k in (("normal", 128), ("large", 256))]
        fehl = wurzel / "fail" / "querencia-dateien" / name
        da = [(c, k) for c, k in kandidaten if c.exists()]
        if modus == "keine":
            ok = not da and not fehl.exists()
            print(("OK" if ok else "FALSCH vorhanden"), p.name)
            gut &= ok
            continue
        if modus == "fehl":
            t, _, _ = tags(fehl.read_bytes()) if fehl.exists() else ({}, 0, 0)
            ok = t.get("Thumb::URI") == uri and t.get("Thumb::MTime") == str(mtime)
            print(("FEHL-EINTRAG" if ok else "FALSCH kein Fehl-Eintrag"), p.name)
            gut &= ok
            continue
        if not da:
            print("FEHLT", p.name)
            gut = False
            continue
        for c, kante in da:
            t, b, h = tags(c.read_bytes())
            gruende = []
            if t.get("Thumb::URI") != uri:
                gruende.append("URI %r" % t.get("Thumb::URI"))
            if t.get("Thumb::MTime") != str(mtime):
                gruende.append("MTime %r statt %d" % (t.get("Thumb::MTime"), mtime))
            if max(b, h) > kante or min(b, h) == 0:
                gruende.append("Größe %dx%d" % (b, h))
            if c.stat().st_mode & 0o777 != 0o600:
                gruende.append("Rechte %o" % (c.stat().st_mode & 0o777))
            if c.parent.stat().st_mode & 0o777 != 0o700:
                gruende.append("Ordner %o" % (c.parent.stat().st_mode & 0o777))
            ok = not gruende
            gut &= ok
            print(("OK" if ok else "FALSCH " + ", ".join(gruende)), p.name, c.parent.name, "%dx%d" % (b, h),
                  t.get("Thumb::Mimetype", "-"), t.get("Software", "-"))
    sys.exit(0 if gut else 1)


main()
