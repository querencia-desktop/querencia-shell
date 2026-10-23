#!/usr/bin/env python3
"""FTP-Attrappe für die Netzprüfung von Querencia Dateien (Welle C1).

Nur lesend, nur 127.0.0.1 im eigenen Netz-Namensraum des Prüfrahmens, nur
Python-Standardbibliothek. Gerade genug FTP für gvfsd-ftp: Anmeldung mit
Benutzer und Kennwort oder anonym (USER anonymous), PWD, CWD, TYPE, PASV,
EPSV, LIST im ls-Format, SIZE, MDTM, RETR, QUIT. Schreibende Befehle: 550.

  ftp-attrappe.py --port 2121 --benutzer anna --kennwort TEST --ordner DIR

Protokoll auf stderr: Befehle ohne Argument von PASS (nie das Kennwort).
"""

import argparse
import os
import socket
import socketserver
import stat
import sys
import time

ARGS = None


def ls_zeile(pfad, name):
    st = os.stat(pfad)
    art = "d" if stat.S_ISDIR(st.st_mode) else "-"
    rechte = stat.filemode(st.st_mode)[1:]
    zeit = time.strftime("%b %d %H:%M", time.gmtime(st.st_mtime))
    return f"{art}{rechte} 1 qd qd {st.st_size:>8} {zeit} {name}\r\n"


class Sitzung(socketserver.StreamRequestHandler):
    def senden(self, text):
        self.wfile.write((text + "\r\n").encode("utf-8"))
        self.wfile.flush()

    def pfad(self, arg):
        wurzel = os.path.realpath(ARGS.ordner)
        neu = arg if arg.startswith("/") else os.path.join(self.cwd, arg)
        neu = os.path.normpath("/" + neu.lstrip("/"))
        echt = os.path.realpath(os.path.join(wurzel, neu.lstrip("/")))
        if echt != wurzel and not echt.startswith(wurzel + os.sep):
            return None, None
        return neu, echt

    def daten(self):
        if self.passiv is None:
            return None
        verb, _ = self.passiv.accept()
        self.passiv.close()
        self.passiv = None
        return verb

    def handle(self):
        self.cwd, self.benutzer, self.drin, self.passiv = "/", None, False, None
        self.senden("220 qd-ftp-attrappe")
        for roh in self.rfile:
            zeile = roh.decode("utf-8", "replace").rstrip("\r\n")
            befehl, _, arg = zeile.partition(" ")
            befehl = befehl.upper()
            sys.stderr.write(f"ftp: {befehl} {'***' if befehl == 'PASS' else arg}\n")
            sys.stderr.flush()
            if befehl == "USER":
                self.benutzer, self.drin = arg, False
                self.senden("331 Kennwort bitte")
            elif befehl == "PASS":
                anonym = self.benutzer in ("anonymous", "ftp")
                if anonym or (self.benutzer == ARGS.benutzer and arg == ARGS.kennwort):
                    self.drin = True
                    self.senden("230 angemeldet")
                else:
                    self.senden("530 Login incorrect.")
            elif befehl == "QUIT":
                self.senden("221 tschüss")
                return
            elif not self.drin:
                self.senden("530 Please login with USER and PASS.")
            elif befehl == "SYST":
                self.senden("215 UNIX Type: L8")
            elif befehl == "FEAT":
                self.wfile.write(b"211-Features:\r\n SIZE\r\n MDTM\r\n EPSV\r\n UTF8\r\n211 End\r\n")
                self.wfile.flush()
            elif befehl == "OPTS":
                self.senden("200 ok")
            elif befehl in ("TYPE", "MODE", "STRU", "NOOP"):
                self.senden("200 ok")
            elif befehl == "PWD":
                self.senden(f'257 "{self.cwd}"')
            elif befehl in ("CWD", "CDUP"):
                neu, echt = self.pfad(".." if befehl == "CDUP" else arg)
                if echt and os.path.isdir(echt):
                    self.cwd = neu
                    self.senden("250 ok")
                else:
                    self.senden("550 No such file or directory.")
            elif befehl in ("PASV", "EPSV"):
                self.passiv = socket.socket()
                self.passiv.bind(("127.0.0.1", 0))
                self.passiv.listen(1)
                port = self.passiv.getsockname()[1]
                if befehl == "EPSV":
                    self.senden(f"229 Entering Extended Passive Mode (|||{port}|)")
                else:
                    self.senden(f"227 Entering Passive Mode (127,0,0,1,{port >> 8},{port & 255})")
            elif befehl in ("LIST", "NLST"):
                ziel = " ".join(w for w in arg.split() if not w.startswith("-")) or "."
                _, echt = self.pfad(ziel)
                if not echt or not os.path.exists(echt):
                    self.senden("550 No such file or directory.")
                    continue
                self.senden("150 Liste folgt")
                verb = self.daten()
                if verb:
                    namen = sorted(os.listdir(echt)) if os.path.isdir(echt) else [os.path.basename(echt)]
                    for n in namen:
                        p = os.path.join(echt, n) if os.path.isdir(echt) else echt
                        verb.sendall((ls_zeile(p, n) if befehl == "LIST" else n + "\r\n").encode("utf-8"))
                    verb.close()
                self.senden("226 fertig")
            elif befehl in ("SIZE", "MDTM"):
                _, echt = self.pfad(arg)
                if echt and os.path.isfile(echt):
                    st = os.stat(echt)
                    wert = st.st_size if befehl == "SIZE" else time.strftime("%Y%m%d%H%M%S", time.gmtime(st.st_mtime))
                    self.senden(f"213 {wert}")
                else:
                    self.senden("550 No such file.")
            elif befehl == "RETR":
                _, echt = self.pfad(arg)
                if not echt or not os.path.isfile(echt):
                    self.senden("550 No such file.")
                    continue
                self.senden("150 Datei folgt")
                verb = self.daten()
                if verb:
                    with open(echt, "rb") as f:
                        verb.sendall(f.read())
                    verb.close()
                self.senden("226 fertig")
            elif befehl in ("STOR", "DELE", "MKD", "RMD", "RNFR", "RNTO", "APPE", "SITE"):
                self.senden("550 Permission denied (nur lesend).")
            else:
                self.senden("502 Command not implemented.")


class Server(socketserver.ThreadingTCPServer):
    allow_reuse_address = True
    daemon_threads = True


def main():
    global ARGS
    p = argparse.ArgumentParser()
    p.add_argument("--port", type=int, required=True)
    p.add_argument("--benutzer", required=True)
    p.add_argument("--kennwort", required=True)
    p.add_argument("--ordner", required=True)
    ARGS = p.parse_args()
    with Server(("127.0.0.1", ARGS.port), Sitzung) as s:
        sys.stderr.write(f"ftp: lauscht auf 127.0.0.1:{ARGS.port}\n")
        sys.stderr.flush()
        s.serve_forever()


if __name__ == "__main__":
    main()
