#!/bin/bash
# G2-AL: Regression mit den umgelenkten Kopien (werkzeug-anlegen.sh):
# G2-AK alle Arten (1024x768, 1920x1080), G2-AD kalender und system, G2-Y
# statussymbole, G2-M und G2-J mit W2 – über ak-werkzeug/gesamt.sh, das
# G2-AK dafür geschrieben hat. Jeder Lauf mit QS_TROCKENLAUF=1; G2-Y, G2-M,
# G2-J und W2 dazu mit QUERENCIA_SITZUNG auf einer Attrappe (gesamt.sh), so
# dass nie ein echtes Abmelden oder Ausschalten möglich ist.
#   regression.sh <panel>
# Zusammenfassung: nachweis/gui/gesamt-<sha16>.txt
set -uo pipefail
WT=/home/user/projects/querencia-runtime-wt-g2al
G=$WT/nachweis/gui
PANEL=$(realpath -e "${1:?Panel}")
bash "$WT/tools/g2al-pruefung/werkzeug-anlegen.sh" >"$G/werkzeug-anlegen.txt" 2>&1 || { echo "Werkzeuge nicht angelegt"; exit 1; }
export QS_TROCKENLAUF=1
bash "$G/ak-werkzeug/gesamt.sh" "$PANEL"
# gesamt.sh legt zum Schluss die Werkzeuge über deren (gesperrtes) Skript an;
# hier über das eigene
bash "$WT/tools/g2al-pruefung/werkzeug-anlegen.sh" >>"$G/werkzeug-anlegen.txt" 2>&1
tail -60 "$G/gesamt-$(sha256sum "$PANEL" | cut -c1-16).txt"
