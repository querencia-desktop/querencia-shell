#!/bin/bash
# G2-AL: ein isolierter GUI-Lauf (eigener headless labwc mit Xwayland, D-Bus
# und XDG-Profil über isoliert.py der umgelenkten G2-M-Werkzeuge).
#   pruefen-g2al.sh <panel-binary> <profilname> <szenario> [zeitlimit-s]
# Umgebung (siehe szenario-g2al.sh): G2AL_AUFLOESUNG, G2AL_PANELINI,
# G2AL_DIENST, G2AL_NEU.
# labwc bekommt die Vorgabe session/querencia/labwc/rc.xml dieses Worktrees,
# von ihren Execute-Kürzeln nur Super+D (gdbus an den Panel-Busnamen des
# isolierten Busses; rc-pruefung-g2al.py). Ergebnisse unter
# nachweis/gui/<profil>/logs.
set -euo pipefail
HIER="$(cd "$(dirname "$0")" && pwd)"
WT=/home/user/projects/querencia-runtime-wt-g2al
G=$WT/nachweis/gui; M=$G/gm-werkzeug
PANEL="$(realpath -e "${1:?Panel-Binary}")"
PROFIL="${2:?Profilname}"; SZ="$(realpath -e "${3:?Szenario}")"; LIMIT="${4:-300}"
[ -x "$G/bin/qeingabe" ] && [ -f "$M/isoliert.py" ] || bash "$HIER/werkzeug-anlegen.sh"
# der Lauf beginnt im Profilordner: Pfade absolut
for v in G2AL_PANELINI G2AL_DIENST G2AL_NEU; do
  if [ -n "${!v:-}" ]; then export "$v=$(realpath -e "${!v}")"; fi
done
. "$M/testprofil.sh"; testprofil_neu "$G" "$PROFIL"
mkdir -p "$G/$PROFIL/config/labwc" "$G/$PROFIL/logs"
python3 -I "$HIER/rc-pruefung-g2al.py" "$WT/session/querencia/labwc/rc.xml" >"$G/$PROFIL/config/labwc/rc.xml"
cp "$G/$PROFIL/config/labwc/rc.xml" "$G/$PROFIL/logs/rc.xml"
cd "$G"
G2AL_PANEL="$PANEL" G2AL_SZENARIO="$SZ" G2AL_BIN="$G/bin" G2AL_WERKZEUG="$HIER" G2M_WERKZEUG="$M" \
  G2AL_TESTFENSTER="$WT/tools/g2ak-pruefung/testfenster.py" G2AL_APPS="$WT/session/querencia/applications" \
  /usr/bin/python3 "$M/isoliert.py" "$G/$PROFIL" "$HIER/szenario-g2al.sh" "$LIMIT"
