#!/bin/bash
# Querencia Dateien, Welle C2: Testmaterial für Miniaturen und Suche
# (außerhalb der Hülle, einmal; lauf.sh kopiert es in jedes Profil).
#
#   material.sh [ZIEL]     Vorgabe: ${DATEIEN_ARBEIT:-~/projects/dateien-miniaturen-arbeit}/material
#
# Bilder/Fotos      200 JPEG 1920×1080 (ffmpeg testsrc2), je ~90 KB
# Bilder/Gemischt   PNG mit Alpha, GIF, WebP, BMP, TIFF, SVG, kaputtes PNG,
#                   ein PNG über 10 MB (Grenze „Nur für Dateien kleiner als“)
# Bilder/Neu        200 JPEG 640×480 (Abbruch beim Ordnerwechsel)
# Dokumente/PDF     drei PDF (einfacher Aufbau, eine Seite)
# Suche             10.000 Dateien in 100 Ordnern, 37 „rechnung“-Treffer,
#                   versteckte Treffer, Umlaute
set -euo pipefail
ZIEL=${1:-${DATEIEN_ARBEIT:-$HOME/projects/dateien-miniaturen-arbeit}/material}
rm -rf "$ZIEL"; mkdir -p "$ZIEL"
cd "$ZIEL"
mkdir -p Bilder/Fotos Bilder/Gemischt Bilder/Neu Dokumente/PDF
ff() { ffmpeg -hide_banner -loglevel error -y "$@"; }
ff -f lavfi -i testsrc2=size=1920x1080:rate=25 -frames:v 200 -q:v 4 Bilder/Fotos/foto-%03d.jpg
ff -f lavfi -i testsrc=size=640x480:rate=25 -frames:v 200 -q:v 5 Bilder/Neu/bild-%03d.jpg
ff -f lavfi -i "color=c=0x3465a4@0.5:size=400x300,format=rgba" -frames:v 1 Bilder/Gemischt/halbdurchsichtig.png
ff -f lavfi -i testsrc=size=320x240 -frames:v 1 Bilder/Gemischt/bewegt.gif
ff -f lavfi -i testsrc2=size=800x600 -frames:v 1 -c:v libwebp_anim Bilder/Gemischt/bild.webp
ff -f lavfi -i testsrc2=size=640x480 -frames:v 1 Bilder/Gemischt/bild.bmp
ff -f lavfi -i testsrc2=size=640x480 -frames:v 1 Bilder/Gemischt/bild.tiff
ff -f lavfi -i "nullsrc=s=2800x2800,geq=r=random(1)*255:g=random(2)*255:b=random(3)*255" -frames:v 1 Bilder/Gemischt/rauschen-gross.png
cat > Bilder/Gemischt/kreis.svg <<'SVG'
<svg xmlns="http://www.w3.org/2000/svg" width="256" height="256"><circle cx="128" cy="128" r="110" fill="#73d216" stroke="#4e9a06" stroke-width="12"/></svg>
SVG
printf '\x89PNG\r\n\x1a\nkaputt' > Bilder/Gemischt/kaputt.png
python3 -I - "$ZIEL" <<'PY'
import os, sys
ziel = sys.argv[1]

def pdf(titel):
    teile = []
    objs = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents 4 0 R "
        b"/Resources << /Font << /F1 5 0 R >> >> >>",
        None,
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
    ]
    strom = ("BT /F1 24 Tf 72 760 Td (%s) Tj ET" % titel).encode()
    objs[3] = b"<< /Length %d >>\nstream\n" % len(strom) + strom + b"\nendstream"
    out = b"%PDF-1.4\n"
    pos = []
    for i, o in enumerate(objs, 1):
        pos.append(len(out))
        out += b"%d 0 obj\n" % i + o + b"\nendobj\n"
    xref = len(out)
    out += b"xref\n0 %d\n0000000000 65535 f \n" % (len(objs) + 1)
    for p in pos:
        out += b"%010d 00000 n \n" % p
    out += b"trailer\n<< /Size %d /Root 1 0 R >>\nstartxref\n%d\n%%%%EOF\n" % (len(objs) + 1, xref)
    return out

for n in ("Rechnung März", "Angebot", "Protokoll"):
    with open(os.path.join(ziel, "Dokumente", "PDF", n + ".pdf"), "wb") as f:
        f.write(pdf(n.replace("ä", "ae")))

# 10.000 Dateien für die Suche: 100 Ordner mit je 100 Einträgen (Ordner
# zählen mit), Treffer „rechnung“ in 37 Namen, dazu versteckte
s = os.path.join(ziel, "Suche")
treffer = 0
for o in range(100):
    d = os.path.join(s, f"bereich-{o // 10:02d}", f"ordner-{o:03d}")
    os.makedirs(d, exist_ok=True)
    for i in range(100 - (2 if o % 10 == 0 else 0) - (1 if o == 0 else 0)):
        nr = o * 100 + i
        if nr % 271 == 0:
            name = f"Rechnung-{nr:05d}.pdf"
            treffer += 1
        else:
            name = f"datei-{nr:05d}.txt"
        open(os.path.join(d, name), "wb").close()
for v in (".versteckt-rechnung.txt", "bereich-00/.alt/rechnung-alt.pdf"):
    p = os.path.join(s, v)
    os.makedirs(os.path.dirname(p), exist_ok=True)
    open(p, "wb").close()
open(os.path.join(s, "Café-Übersicht.txt"), "wb").close()
gesamt = sum(len(fs) + len(ds) for _, ds, fs in os.walk(s))
print(f"material: Suche {gesamt} Einträge, {treffer} sichtbare Treffer „rechnung“")
PY
du -sh "$ZIEL"/* | sed 's/^/material: /'
