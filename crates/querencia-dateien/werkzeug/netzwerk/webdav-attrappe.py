#!/usr/bin/env python3
"""WebDAV-Attrappe für die Netzprüfung von Querencia Dateien (Welle C1).

Nur lesend, nur 127.0.0.1 im eigenen Netz-Namensraum des Prüfrahmens, nur
Python-Standardbibliothek. Gerade genug WebDAV für gvfsd-dav (libsoup):
OPTIONS mit „DAV: 1“, PROPFIND (Tiefe 0 und 1), GET und HEAD; alles andere
403. Jede Anfrage ohne passende Basic-Anmeldung bekommt 401, so fragt GVfs
nach Benutzer und Kennwort (Kennwortdialog, „Kennwort merken“).

  webdav-attrappe.py --port 8080 --benutzer anna --kennwort TEST --ordner DIR

Protokoll auf stderr: Methode, Pfad, Status, ob angemeldet (nie das
Kennwort).
"""

import argparse
import base64
import email.utils
import html
import mimetypes
import os
import sys
import urllib.parse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

ARGS = None


def erlaubt(pfad):
    """Absoluter Pfad unterhalb des Ordners oder None (kein Ausbruch)."""
    wurzel = os.path.realpath(ARGS.ordner)
    teil = urllib.parse.unquote(urllib.parse.urlsplit(pfad).path)
    ziel = os.path.realpath(os.path.join(wurzel, teil.lstrip("/")))
    if ziel != wurzel and not ziel.startswith(wurzel + os.sep):
        return None
    return ziel


def eigenschaften(href, ziel):
    st = os.stat(ziel)
    ordner = os.path.isdir(ziel)
    name = os.path.basename(ziel.rstrip("/")) or "/"
    zeit = email.utils.formatdate(st.st_mtime, usegmt=True)
    teile = [
        f"<D:displayname>{html.escape(name)}</D:displayname>",
        "<D:resourcetype><D:collection/></D:resourcetype>" if ordner else "<D:resourcetype/>",
        f"<D:getlastmodified>{zeit}</D:getlastmodified>",
        f'<D:getetag>"{int(st.st_mtime)}-{st.st_size}"</D:getetag>',
    ]
    if not ordner:
        typ = mimetypes.guess_type(ziel)[0] or "application/octet-stream"
        teile.append(f"<D:getcontentlength>{st.st_size}</D:getcontentlength>")
        teile.append(f"<D:getcontenttype>{typ}</D:getcontenttype>")
    return (
        f"<D:response><D:href>{html.escape(href)}</D:href><D:propstat><D:prop>"
        + "".join(teile)
        + "</D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat></D:response>"
    )


class Bearbeiter(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    server_version = "qd-webdav-attrappe"

    def log_message(self, fmt, *args):  # Protokoll ohne Kopfzeilen
        sys.stderr.write("webdav: " + (fmt % args) + "\n")

    def angemeldet(self):
        kopf = self.headers.get("Authorization", "")
        if not kopf.startswith("Basic "):
            return False
        try:
            benutzer, _, kennwort = base64.b64decode(kopf[6:]).decode().partition(":")
        except ValueError:
            return False
        return benutzer == ARGS.benutzer and kennwort == ARGS.kennwort

    def antwort(self, status, text=b"", kopf=None):
        self.send_response(status)
        for k, v in (kopf or {}).items():
            self.send_header(k, v)
        self.send_header("Content-Length", str(len(text)))
        self.end_headers()
        if self.command != "HEAD":
            self.wfile.write(text)

    def koerper_lesen(self):
        n = int(self.headers.get("Content-Length") or 0)
        if n:
            self.rfile.read(n)

    def pruefen(self):
        self.koerper_lesen()
        if self.angemeldet():
            return True
        sys.stderr.write(f"webdav: {self.command} {self.path} ohne gültige Anmeldung\n")
        self.antwort(401, b"Anmeldung erforderlich\n",
                     {"WWW-Authenticate": 'Basic realm="QD-Test"', "Content-Type": "text/plain"})
        return False

    def do_OPTIONS(self):
        if self.pruefen():
            self.antwort(200, b"", {"DAV": "1", "Allow": "OPTIONS, GET, HEAD, PROPFIND",
                                    "MS-Author-Via": "DAV"})

    def do_PROPFIND(self):
        if not self.pruefen():
            return
        ziel = erlaubt(self.path)
        if ziel is None or not os.path.exists(ziel):
            self.antwort(404, b"")
            return
        tiefe = self.headers.get("Depth", "1")
        basis = urllib.parse.urlsplit(self.path).path
        if os.path.isdir(ziel) and not basis.endswith("/"):
            basis += "/"
        teile = [eigenschaften(basis, ziel)]
        if tiefe != "0" and os.path.isdir(ziel):
            for name in sorted(os.listdir(ziel)):
                kind = os.path.join(ziel, name)
                href = basis + urllib.parse.quote(name) + ("/" if os.path.isdir(kind) else "")
                teile.append(eigenschaften(href, kind))
        text = ('<?xml version="1.0" encoding="utf-8"?><D:multistatus xmlns:D="DAV:">'
                + "".join(teile) + "</D:multistatus>").encode()
        self.antwort(207, text, {"Content-Type": 'application/xml; charset="utf-8"'})

    def do_GET(self):
        if not self.pruefen():
            return
        ziel = erlaubt(self.path)
        if ziel is None or not os.path.isfile(ziel):
            self.antwort(404, b"")
            return
        with open(ziel, "rb") as f:
            text = f.read()
        self.antwort(200, text, {"Content-Type": mimetypes.guess_type(ziel)[0] or "application/octet-stream"})

    do_HEAD = do_GET

    def nur_lesend(self):
        if self.pruefen():
            self.antwort(403, b"nur lesend\n")

    do_PUT = do_DELETE = do_MKCOL = do_MOVE = do_COPY = do_PROPPATCH = do_LOCK = do_UNLOCK = nur_lesend


def main():
    global ARGS
    p = argparse.ArgumentParser()
    p.add_argument("--port", type=int, required=True)
    p.add_argument("--benutzer", required=True)
    p.add_argument("--kennwort", required=True)
    p.add_argument("--ordner", required=True)
    ARGS = p.parse_args()
    server = ThreadingHTTPServer(("127.0.0.1", ARGS.port), Bearbeiter)
    sys.stderr.write(f"webdav: lauscht auf 127.0.0.1:{ARGS.port}\n")
    sys.stderr.flush()
    server.serve_forever()


if __name__ == "__main__":
    main()
