#!/usr/bin/env python3
"""Legt den Beispielordner des Dateien-Prototyps an (nur unter <basis>).

Aufruf:  beispielordner.py <basis> [--neu]
<basis> ist der Ordner querencia-dateien-ansichten im HOME eines Prüfprofils
(Kopie aus dem Prototyp dateien/prototyp-2). Angelegt wird nur <basis>/beispiel; in
andere Ordner wird nichts geschrieben. Steht dort schon ein Beispiel dieser
Fassung, bleibt es unverändert (Umbenennungen bleiben erhalten), außer mit
--neu: dann wird nur <basis>/beispiel gelöscht und neu angelegt.

Inhalt: Ordner (auch mit langem Namen, versteckte), Dateien vieler Typen
(leer, nur die Endung bestimmt das Symbol), Umlaute, Zahlen im Namen
(datei2 vor datei10), verschiedene Größen und Änderungszeiten (Sortierung),
eine Sicherungsdatei, ein Starter, zwei Verknüpfungen, eine ausführbare
Datei und der Ordner „10000 Einträge“ mit 10.000 Dateien und 20 Ordnern.
"""

import os
import shutil
import sys

FASSUNG = "prototyp-beispiel-1"
WOERTER = ["Angebot", "Bericht", "Brief", "Foto", "Rechnung", "Notiz", "Protokoll",
           "Entwurf", "Liste", "Vertrag", "Urlaub", "Kalender", "Skizze", "Tabelle",
           "Zeugnis", "Garten", "Haus", "Auto", "Musik", "Video"]
ENDUNGEN = ["txt", "png", "pdf", "odt", "jpg", "mp3", "ods", "zip", "html", "c", "py", "svg"]
# feste Zeit (22.10.2026 12:00 UTC), damit die Datumssortierung gleich bleibt
ZEIT = 1792670400


def datei(pfad, groesse=0, alter_tage=0, inhalt=None, ausfuehrbar=False):
    with open(pfad, "wb") as f:
        if inhalt is not None:
            f.write(inhalt.encode())
        elif groesse:
            f.truncate(groesse)
    if ausfuehrbar:
        os.chmod(pfad, 0o755)
    t = ZEIT - alter_tage * 86400
    os.utime(pfad, (t, t))


def anlegen(ziel):
    os.makedirs(ziel)
    for name in ["Bilder", "Dokumente", "Musik", "Rechnungen 2026", "Ablage",
                 "Ein Ordner mit einem sehr langen Namen, der umbrechen muss",
                 ".versteckter Ordner"]:
        os.makedirs(os.path.join(ziel, name))
    for i in range(1, 6):
        datei(os.path.join(ziel, "Bilder", f"Urlaub {i}.jpg"), groesse=200_000 * i, alter_tage=i)
    datei(os.path.join(ziel, "Dokumente", "Lebenslauf.odt"), groesse=40_000)
    datei(os.path.join(ziel, "Rechnungen 2026", "Rechnung März.pdf"), groesse=80_000)
    datei(os.path.join(ziel, "bericht.final.odt"), groesse=52_000, alter_tage=3)
    datei(os.path.join(ziel, "notiz.txt"), inhalt="Einkaufen: Brot, Milch\n", alter_tage=1)
    datei(os.path.join(ziel, "Übersicht Ärger Größe.pdf"), groesse=1_200_000, alter_tage=40)
    datei(os.path.join(ziel, "foto.jpg"), groesse=2_400_000, alter_tage=10)
    datei(os.path.join(ziel, "bild.png"), groesse=300_000, alter_tage=11)
    datei(os.path.join(ziel, "lied.mp3"), groesse=4_000_000, alter_tage=100)
    datei(os.path.join(ziel, "film.mp4"), groesse=50_000_000, alter_tage=200)
    datei(os.path.join(ziel, "archiv.tar.gz"), groesse=7_000_000, alter_tage=300)
    datei(os.path.join(ziel, "tabelle.ods"), groesse=20_000, alter_tage=5)
    datei(os.path.join(ziel, "präsentation.odp"), groesse=900_000, alter_tage=6)
    datei(os.path.join(ziel, "seite.html"), groesse=3_000, alter_tage=7)
    datei(os.path.join(ziel, "programm.py"), inhalt="print('hallo')\n", alter_tage=8)
    datei(os.path.join(ziel, "skript.sh"), inhalt="#!/bin/sh\necho hallo\n", alter_tage=9,
          ausfuehrbar=True)
    datei(os.path.join(ziel, "datei1.txt"), groesse=10, alter_tage=12)
    datei(os.path.join(ziel, "datei2.txt"), groesse=20, alter_tage=13)
    datei(os.path.join(ziel, "datei10.txt"), groesse=30, alter_tage=14)
    datei(os.path.join(ziel, "eine-sehr-lange-datei-mit-einem-namen-der-ueber-drei-zeilen-"
                             "hinausgeht-und-deshalb-gekuerzt-werden-muss.txt"), groesse=5, alter_tage=2)
    datei(os.path.join(ziel, "leer"), alter_tage=4)
    datei(os.path.join(ziel, ".versteckt.txt"), groesse=1, alter_tage=4)
    datei(os.path.join(ziel, "sicherung.txt~"), groesse=1, alter_tage=4)
    datei(os.path.join(ziel, "Mein Starter.desktop"),
          inhalt="[Desktop Entry]\nType=Application\nName=Mein Starter\n"
                 "Exec=true\nIcon=utilities-terminal\n", alter_tage=15, ausfuehrbar=True)
    os.symlink("Dokumente", os.path.join(ziel, "Verknüpfung zu Dokumente"))
    os.symlink("gibt-es-nicht", os.path.join(ziel, "kaputte Verknüpfung"))
    gross = os.path.join(ziel, "10000 Einträge")
    os.makedirs(gross)
    for i in range(1, 21):
        os.makedirs(os.path.join(gross, f"Ordner {i:02d}"))
    for i in range(10_000):
        wort = WOERTER[i % len(WOERTER)]
        endung = ENDUNGEN[(i // len(WOERTER)) % len(ENDUNGEN)]
        datei(os.path.join(gross, f"{wort} {i:05d}.{endung}"), groesse=(i * 37) % 100_000,
              alter_tage=i % 365)
    with open(os.path.join(ziel, "." + FASSUNG), "w") as f:
        f.write("Beispielordner des Dateien-Prototyps; darf jederzeit gelöscht werden.\n")


def main():
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    basis = os.path.abspath(sys.argv[1])
    if os.path.basename(basis) != "querencia-dateien-ansichten":
        sys.exit(f"beispielordner: Basis muss …/querencia-dateien-ansichten heißen, nicht {basis}")
    ziel = os.path.join(basis, "beispiel")
    if os.path.exists(os.path.join(ziel, "." + FASSUNG)) and "--neu" not in sys.argv:
        print(f"Beispielordner vorhanden: {ziel}")
        return
    if os.path.lexists(ziel):
        shutil.rmtree(ziel)
    anlegen(ziel)
    print(f"Beispielordner angelegt: {ziel}")


if __name__ == "__main__":
    main()
