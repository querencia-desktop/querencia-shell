#!/bin/bash
# G2-J: ein isolierter GUI-Lauf.
#   pruefen.sh <panel-binary> <mixer-binary> <profilname> <szenario> [zeitlimit-s]
# Umgebung: G2J_AUFLOESUNG, G2J_AUDIO, G2J_PANELINI (siehe szenario-g2j.sh).
# Ergebnisse: nachweis/gui/<profil>/logs/ (Bilder, panel.log, qpulse.log,
# szenario.txt, panel.ini.*, qpulse-ende.json).
set -euo pipefail
HIER="$(cd "$(dirname "$0")" && pwd)"
WT=/home/user/projects/querencia-runtime-wt-lautstaerke
G=$WT/nachweis/gui
PANEL="$(realpath -e "${1:?Panel-Binary}")"; MIXER="$(realpath -e "${2:?Mixer-Binary}")"
PROFIL="${3:?Profilname}"; SZ="$(realpath -e "${4:?Szenario}")"; LIMIT="${5:-300}"
[ -x "$G/bin/qeingabe" ] || bash "$HIER/bauen.sh" "$G/bin"
. "$HIER/testprofil.sh"; testprofil_neu "$G" "$PROFIL"
mkdir -p "$G/$PROFIL/config/labwc"
cd "$G"
G2J_PANEL="$PANEL" G2J_MIXER="$MIXER" G2J_SZENARIO="$SZ" G2J_BIN="$G/bin" G2J_WERKZEUG="$HIER" \
  G2J_DATA="$WT/crates/querencia-lautstaerke/data" \
  G2J_APPS="${G2J_APPS-$WT/session/querencia/applications}" \
  /usr/bin/python3 "$HIER/isoliert.py" "$PROFIL" "$HIER/szenario-g2j.sh" "$LIMIT"
