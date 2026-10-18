#!/bin/bash
# G2-M: ein isolierter GUI-Lauf.
#   pruefen.sh <panel-binary> <profilname> <szenario> [zeitlimit-s]
# Umgebung: G2M_AUFLOESUNG, G2M_NETZ, G2M_PANELINI (siehe szenario-g2m.sh).
# Ergebnisse: nachweis/gui/<profil>/logs/ (Bilder, panel.log, qnm.log,
# szenario.txt, panel.ini.*, qnm-ende.json).
set -euo pipefail
HIER="$(cd "$(dirname "$0")" && pwd)"
WT=~/projects/querencia-runtime-wt-netzwerk
G=$WT/nachweis/gui
PANEL="$(realpath -e "${1:?Panel-Binary}")"
PROFIL="${2:?Profilname}"; SZ="$(realpath -e "${3:?Szenario}")"; LIMIT="${4:-300}"
[ -x "$G/bin/qeingabe" ] || bash "$HIER/bauen.sh" "$G/bin"
. "$HIER/testprofil.sh"; testprofil_neu "$G" "$PROFIL"
mkdir -p "$G/$PROFIL/config/labwc"
cd "$G"
G2M_PANEL="$PANEL" G2M_SZENARIO="$SZ" G2M_BIN="$G/bin" G2M_WERKZEUG="$HIER" \
  G2M_APPS="${G2M_APPS-$WT/session/querencia/applications}" \
  /usr/bin/python3 "$HIER/isoliert.py" "$G/$PROFIL" "$HIER/szenario-g2m.sh" "$LIMIT"
