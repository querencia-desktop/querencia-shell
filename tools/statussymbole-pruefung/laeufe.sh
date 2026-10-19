#!/bin/bash
# G2-Y (QD-045): isolierte GUI-Läufe der Statussymbole, mit den umgelenkten
# G2-M-Werkzeugen (werkzeug-anlegen.sh vorher).
#   laeufe.sh <panel-neu> [<panel-vorher>] [auflösungen …]
# Je Auflösung: „vorher“ mit dem Ausgangs-Panel (Standard: Live-Ablage
# G2-M c3cda503421d7e9f, nur gelesen), „statussymbole“ mit dem neuen Panel.
# Profile nachweis/gui/yv<breite> und y<breite>; Auswertungen
# nachweis/gui/ergebnis-y*.txt, Zusammenfassung nachweis/gui/ergebnis-g2y.txt.
set -uo pipefail
WT=/home/user/projects/querencia-runtime-wt-statussymbole
T=$WT/tools/statussymbole-pruefung; G=$WT/nachweis/gui; M=$G/gm-werkzeug
NEU=$(realpath -e "${1:?Panel neu}"); shift
ALT=/home/user/opt/querencia/panel/c3cda503421d7e9f/querencia-panel
if [ $# -gt 0 ] && [ -f "$1" ]; then ALT=$(realpath -e "$1"); shift; fi
AUFL=("$@"); [ ${#AUFL[@]} -eq 0 ] && AUFL=(1024x768 1920x1080)
[ -x "$M/pruefen.sh" ] || bash "$T/werkzeug-anlegen.sh"
SUMME=$G/ergebnis-g2y.txt
echo "=== $(date '+%F %T') G2-Y neu $(sha256sum "$NEU" | cut -c1-16), vorher $(sha256sum "$ALT" | cut -c1-16)" >>"$SUMME"
for r in "${AUFL[@]}"; do
  w=${r%x*}; h=${r#*x}
  for art in ${LAEUFE:-vorher statussymbole}; do
    if [ "$art" = vorher ]; then p=yv$w; panel=$ALT; else p=y$w; panel=$NEU; fi
    python3 -I "$T/szenario-machen.py" "$w" "$h" "$art" >"$G/sz-$w-$art.txt"
    G2M_AUFLOESUNG=$r G2M_NETZ=echt G2M_PANELINI=$M/panel-mit-netzwerk.ini G2M_NEUSTART=$([ "$art" = vorher ] || echo 1) \
      bash "$M/pruefen.sh" "$panel" "$p" "$G/sz-$w-$art.txt" 300 >"$G/lauf-$p.txt" 2>&1
    python3 -I "$T/auswerten.py" "$G/$p/logs" "$art" "$w" "$h" >"$G/ergebnis-$p.txt" 2>&1
    echo "G2-Y $art $r: $(tail -1 "$G/ergebnis-$p.txt")" | tee -a "$SUMME"
  done
done
