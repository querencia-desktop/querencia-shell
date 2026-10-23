#!/usr/bin/env python3
"""wsdd-Attrappe für die Netzprüfung von Querencia Dateien (Welle C1).

gvfsd-wsdd startet „wsdd --no-host --discovery --listen SOCKET“ aus dem PATH;
lauf.sh legt diese Datei als „wsdd“ vorn in den PATH der Testsitzung. Sie
sendet nichts ins Netz, sondern beantwortet nur die API von wsdd 0.8 auf dem
Unix-Socket: „list [Typ]“ liefert zwei feste Testgeräte (Spalten wie
wsdd.get_list_reply: UUID, Name, Arbeitsgruppe, zuletzt gesehen, Adressen
als „Schnittstelle, {'Adresse'}“ wie Python sie ausgibt, Typen), dann „.“. „probe“, „start“, „stop“, „clear“ werden angenommen.
"""

import datetime
import os
import socket
import sys
import threading

GERAETE = [
    ("urn:uuid:7c1e9a52-0d1f-4b8e-9f7a-000000000001", "QD-TESTNAS", "Workgroup:WORKGROUP"),
    ("urn:uuid:7c1e9a52-0d1f-4b8e-9f7a-000000000002", "QD-PC", "Workgroup:WORKGROUP"),
]


def liste(typ):
    jetzt = datetime.datetime.now().isoformat("T", "seconds")
    zeilen = []
    for uid, name, gruppe in GERAETE:
        typen = "pub:Computer,wsdp:Device"
        if typ and typ not in typen.split(","):
            continue
        zeilen.append(f"{uid}\t{name}\t{gruppe}\t{jetzt}\tlo, {{'127.0.0.1'}}\t{typen}\n")
    return "".join(zeilen) + ".\n"


def bedienen(verb):
    with verb, verb.makefile("rw", encoding="utf-8", newline="\n") as f:
        for zeile in f:
            worte = zeile.split()
            if not worte:
                continue
            sys.stderr.write(f"wsdd-attrappe: {' '.join(worte)}\n")
            if worte[0] == "list":
                f.write(liste(worte[1] if len(worte) > 1 else None))
                f.flush()
            elif worte[0] == "quit":
                return


def main():
    argv = sys.argv[1:]
    if "--listen" not in argv:
        sys.stderr.write("wsdd-attrappe: nur mit --listen SOCKET\n")
        return 2
    pfad = argv[argv.index("--listen") + 1]
    if os.path.exists(pfad):
        os.unlink(pfad)
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.bind(pfad)
    s.listen(4)
    sys.stderr.write(f"wsdd-attrappe: lauscht auf {pfad}\n")
    while True:
        verb, _ = s.accept()
        threading.Thread(target=bedienen, args=(verb,), daemon=True).start()


if __name__ == "__main__":
    sys.exit(main())
