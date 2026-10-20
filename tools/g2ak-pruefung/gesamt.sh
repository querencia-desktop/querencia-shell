#!/bin/bash
# G2-AK: alle isolierten GUI-Prüfungen für eine Panel-Binary, nacheinander.
#   gesamt.sh <panel>
# 1. G2-AK (laeufe-g2ak.sh, 1024x768 und 1920x1080)
# 2. G2-AD kalender und system (umgelenkte Kopie, Abmelden/Ausschalten eine
#    Zeile tiefer, siehe werkzeug-anlegen.sh); querencia-sitzung echt aus
#    PATH bzw. ~/opt/querencia/bin, nur gelesen, mit QS_TROCKENLAUF=1 im
#    isolierten labwc wie bei G2-AD
# 3. G2-Y statussymbole, G2-M und G2-J mit W2 (regression.sh); dabei zeigt
#    QUERENCIA_SITZUNG auf eine Attrappe, kein Lauf erreicht das echte
#    Programm (W2 klickt im System-Menü noch auf die alte Abmelden-Zeile,
#    dort steht jetzt „Bildschirm sperren“)
# Zusammenfassung: nachweis/gui/gesamt-<sha16>.txt
set -uo pipefail
WT=~/projects/querencia-runtime-wt-g2ak
T=$WT/tools/g2ak-pruefung; G=$WT/nachweis/gui; A=$G/ad-werkzeug
PANEL=$(realpath -e "${1:?Panel}")
SHA=$(sha256sum "$PANEL" | cut -c1-16)
OUT=$G/gesamt-$SHA.txt
DESKTOP=~/opt/querencia/desktop/4b45feac67ef2e22/querencia-desktop
echo "=== $(date '+%F %T') Gesamtlauf Panel $SHA" >>"$OUT"
bash "$T/laeufe-g2ak.sh" "$PANEL" 1024x768 1920x1080 >>"$OUT" 2>&1
[ -x "$A/laeufe-g2ad.sh" ] || bash "$T/werkzeug-anlegen.sh" >/dev/null 2>&1
LAEUFE="kalender system" bash "$A/laeufe-g2ad.sh" "$PANEL" "$DESKTOP" 1024x768 1920x1080 >>"$OUT" 2>&1
R=$G/attrappe-regression
mkdir -p "$R"
cp "$T/sitzung-attrappe.sh" "$R/querencia-sitzung"; chmod 755 "$R/querencia-sitzung"
export QUERENCIA_SITZUNG=$R/querencia-sitzung G2AK_AUFRUFE=$R/aufrufe-$SHA.txt
LAEUFE=statussymbole bash "$A/laeufe.sh" "$PANEL" >>"$OUT" 2>&1
bash "$A/regression.sh" "$PANEL" >>"$OUT" 2>&1
echo "--- G2-M"; tail -12 "$G/ergebnis.txt" >>"$OUT" 2>/dev/null
echo "--- G2-J/W2"; tail -20 "$G/gj/ergebnis.txt" >>"$OUT" 2>/dev/null
# Werkzeuge mit den G2-AK-Tasten wieder herstellen (regression.sh legt gm-werkzeug neu an)
bash "$T/werkzeug-anlegen.sh" >/dev/null 2>&1
echo "=== $(date '+%F %T') fertig" >>"$OUT"
