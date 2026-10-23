#!/usr/bin/env python3
"""Testbaum für die isolierte Prüfung von Querencia Dateien (B §5):
XDG-Ordner mit deutschen Namen, Dokumente mit Umlauten, Leerzeichen,
langen Namen, versteckten Dateien, Verweis, ausführbarer Textdatei,
200 Dateien, eine 50-MB-Datei, ein 12 Ebenen tiefer Pfad, Vorlagen,
Attrappen-Programme (schreiben ihre Argumente nach ~/aufrufe.log) mit
mimeapps.list, optional eine Files-settings.toml (Migration M6).

  testbaum.py HOME XDG_CONFIG_HOME XDG_DATA_HOME [--files-toml DATEI]
Nur im Profil des Prüfrahmens aufrufen (prüft, dass HOME kein echtes ist).
"""
import os
import sys

PNG_1x1 = (b"\x89PNG\r\n\x1a\n" + b"\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01\x08\x02\x00\x00\x00\x90wS\xde"
           + b"\x00\x00\x00\x0cIDATx\x9cc\xf8\xcf\xc0\x00\x00\x03\x01\x01\x00\xc9\xfe\x92\xef"
           + b"\x00\x00\x00\x00IEND\xaeB`\x82")
PDF = b"%PDF-1.4\n1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\n2 0 obj<</Type/Pages/Kids[]/Count 0>>endobj\ntrailer<</Root 1 0 R>>\n%%EOF\n"

XDG = {
    "DESKTOP": "Arbeitsfläche", "DOCUMENTS": "Dokumente", "DOWNLOAD": "Downloads",
    "MUSIC": "Musik", "PICTURES": "Bilder", "PUBLICSHARE": "Öffentlich",
    "TEMPLATES": "Vorlagen", "VIDEOS": "Videos",
}

ATTRAPPEN = {
    # Desktop-ID: (Name, MIME-Typen)
    "attrappe-editor": ("Attrappe Editor", ["text/plain"]),
    "attrappe-bilder": ("Attrappe Bilder", ["image/png"]),
    "attrappe-dokumente": ("Attrappe Dokumente", ["application/pdf", "application/vnd.oasis.opendocument.text"]),
    "attrappe-zweit": ("Attrappe Zweitprogramm", ["text/plain", "image/png"]),
    "attrappe-browser": ("Attrappe Browser", ["x-scheme-handler/https", "x-scheme-handler/http", "text/html"]),
}


def schreiben(pfad, inhalt=b"", modus=None):
    os.makedirs(os.path.dirname(pfad), exist_ok=True)
    with open(pfad, "wb") as f:
        f.write(inhalt if isinstance(inhalt, bytes) else inhalt.encode())
    if modus is not None:
        os.chmod(pfad, modus)


def main():
    home, config, data = sys.argv[1:4]
    if not os.path.exists(os.path.join(os.path.dirname(home), ".qd-profil")):
        sys.exit("testbaum: kein Prüfrahmen-Profil")
    files_toml = sys.argv[sys.argv.index("--files-toml") + 1] if "--files-toml" in sys.argv else None
    zeilen = []
    for k, name in XDG.items():
        os.makedirs(os.path.join(home, name), exist_ok=True)
        zeilen.append(f'XDG_{k}_DIR="$HOME/{name}"')
    schreiben(os.path.join(config, "user-dirs.dirs"), "\n".join(zeilen) + "\n")
    d = os.path.join(home, "Dokumente")
    schreiben(os.path.join(d, "notiz.txt"), "Eine Notiz.\n")
    schreiben(os.path.join(d, "angebot.pdf"), PDF)
    schreiben(os.path.join(d, "bericht.final.odt"), b"PK\x03\x04odt")
    schreiben(os.path.join(d, "Brief an Oma.odt"), b"PK\x03\x04odt")
    schreiben(os.path.join(d, "Übersicht März.txt"), "Umlaute\n")
    schreiben(os.path.join(d, "ein sehr langer Dateiname für die Beschriftung unter dem Symbol.txt"), "lang\n")
    schreiben(os.path.join(d, ".versteckt.txt"), "versteckt\n")
    schreiben(os.path.join(d, "sicherung.txt~"), "alt\n")
    schreiben(os.path.join(d, "skript.sh"), "#!/bin/sh\necho ausgeführt >> \"$HOME/aufrufe.log\"\n", 0o755)
    os.makedirs(os.path.join(d, "Rechnungen"), exist_ok=True)
    for i in range(1, 13):
        schreiben(os.path.join(d, "Rechnungen", f"rechnung-{i}.txt"), f"Rechnung {i}\n" * i)
    if not os.path.lexists(os.path.join(d, "verweis-auf-notiz.txt")):
        os.symlink("notiz.txt", os.path.join(d, "verweis-auf-notiz.txt"))
    schreiben(os.path.join(home, "Bilder", "foto.png"), PNG_1x1)
    viele = os.path.join(home, "Viele Dateien")
    for i in range(200):
        schreiben(os.path.join(viele, f"datei-{i:03d}.txt"), f"{i}\n")
    gross = os.path.join(home, "Groß", "gross-50mb.bin")
    os.makedirs(os.path.dirname(gross), exist_ok=True)
    with open(gross, "wb") as f:
        block = os.urandom(1 << 20)
        for _ in range(50):
            f.write(block)
    tief = os.path.join(home, "Tief")
    for i in range(1, 13):
        tief = os.path.join(tief, f"ebene-{i:02d}")
    os.makedirs(tief, exist_ok=True)
    schreiben(os.path.join(tief, "ganz-unten.txt"), "unten\n")
    v = os.path.join(home, "Vorlagen")
    schreiben(os.path.join(v, "Brief.odt"), b"PK\x03\x04vorlage")
    schreiben(os.path.join(v, "Notiz.txt"), "Vorlage\n")
    # Attrappen-Programme
    apps = os.path.join(data, "applications")
    binordner = os.path.join(home, ".local", "bin")
    for ident, (name, typen) in ATTRAPPEN.items():
        prog = os.path.join(binordner, ident)
        schreiben(prog, f'#!/bin/sh\necho "{ident} $*" >> "$HOME/aufrufe.log"\n', 0o755)
        schreiben(os.path.join(apps, ident + ".desktop"),
                  "[Desktop Entry]\nType=Application\n"
                  f"Name={name}\nExec={prog} %u\nMimeType={';'.join(typen)};\nNoDisplay=false\n")
    standard = {
        "text/plain": "attrappe-editor", "image/png": "attrappe-bilder",
        "application/pdf": "attrappe-dokumente",
        "application/vnd.oasis.opendocument.text": "attrappe-dokumente",
        "x-scheme-handler/https": "attrappe-browser", "x-scheme-handler/http": "attrappe-browser",
        "text/html": "attrappe-browser",
    }
    schreiben(os.path.join(config, "mimeapps.list"),
              "[Default Applications]\n" + "".join(f"{t}={a}.desktop\n" for t, a in standard.items())
              + "[Added Associations]\ntext/plain=attrappe-editor.desktop;attrappe-zweit.desktop;\n"
              + "image/png=attrappe-bilder.desktop;attrappe-zweit.desktop;\n")
    if files_toml:
        ziel = os.path.join(config, "querencia-files", "settings.toml")
        os.makedirs(os.path.dirname(ziel), exist_ok=True)
        with open(files_toml, "rb") as q, open(ziel, "wb") as z:
            z.write(q.read())
    print("testbaum: angelegt in", home)


if __name__ == "__main__":
    main()
