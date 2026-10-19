#!/bin/bash
# G2-Y: Panel-Tausch isoliert nachstellen (headless labwc, eigener Bus).
#   starten.sh <profil> <alt-binary> <neu-binary> <anzahl> tausch|warten|messen|patch
# Profil unter nachweis/gui/<profil>; Ergebnis <profil>/logs/wettlauf.jsonl.
set -euo pipefail
WT=/home/user/projects/querencia-runtime-wt-statussymbole
G=$WT/nachweis/gui; M=$G/gm-werkzeug; HIER="$(cd "$(dirname "$0")" && pwd)"
[ -x "$M/isoliert.py" ] || bash "$WT/tools/statussymbole-pruefung/werkzeug-anlegen.sh"
. "$M/testprofil.sh"; testprofil_neu "$G" "${1:?Profil}"
WL_ALT=$(realpath -e "$2") WL_NEU=$(realpath -e "$3") WL_N=$4 WL_ART=$5 WL_WERKZEUG=$HIER \
WL_PANELINI=$M/panel-mit-netzwerk.ini WL_TAUSCH=$WT/tools/statussymbole-pruefung/tausch/tausch.py \
  /usr/bin/python3 "$M/isoliert.py" "$G/$1" "$HIER/wettlauf.sh" $(( $4 * 40 + 60 ))
