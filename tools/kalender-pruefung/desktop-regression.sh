#!/bin/bash
# G2-AD: Desktop-Regression wie QE-04a (nachweis/qe04/regression-lauf.sh), mit
# eigenen Ordnern ~/.cache/g2x/gui-test-g2ad und rt-g2ad (testprofil.sh lässt
# nur ~/.cache/g2x/gui-test* zu; G2-Xs eigene Profile bleiben unberührt)
# und dem Panel des Ausgangs (G2-Y 97461a61eed7ab6b, nur gelesen).
#   bash tools/kalender-pruefung/desktop-regression.sh <desktop-binary> [suite …]
# Standard-Suiten wie bei QE-04a: qe02 qe01 qe03-ohne-loop hmp10a-voll hmp09-raster-aus.
set -uo pipefail
WT=~/projects/querencia-runtime-wt-kalender
B=$(realpath -e "${1:?Desktop-Binary}"); shift
SUITEN=("$@"); [ ${#SUITEN[@]} -eq 0 ] && SUITEN=(qe02 qe01 qe03-ohne-loop hmp10a-voll hmp09-raster-aus)
AUS=$WT/nachweis/gui/desktop-regression; mkdir -p "$AUS"
export G2R_GUI_TEST=~/.cache/g2x/gui-test-g2ad G2R_RT=~/.cache/g2x/rt-g2ad
export G2R_PANEL=~/opt/querencia/panel/97461a61eed7ab6b/querencia-panel QE02_BIN=$B
for s in "${SUITEN[@]}"; do
  echo "== $(date '+%F %T') $s mit $(sha256sum "$B" | cut -c1-16)"
  bash "$WT/nachweis/regression.sh" "$s" >"$AUS/$s.log" 2>&1
  echo "exit=$? $(grep -a -E 'ERGEBNIS|Prüfungen bestanden|[0-9]+/[0-9]+' "$AUS/$s.log" | tail -2 | tr '\n' ' ')"
done | tee -a "$AUS/uebersicht.txt"
