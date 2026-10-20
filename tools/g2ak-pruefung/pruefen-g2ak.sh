#!/bin/bash
# G2-AK: ein isolierter GUI-Lauf (eigener headless labwc mit Xwayland, D-Bus
# und XDG-Profil über isoliert.py der umgelenkten G2-M-Werkzeuge).
#   pruefen-g2ak.sh <panel-binary> <profilname> <szenario> [zeitlimit-s]
# Umgebung: G2AK_AUFLOESUNG (z. B. 1024x768), G2AK_PANELINI (Datei, die als
# panel.ini kopiert wird), G2AK_SITZUNG (pfad|variable|keins, siehe
# szenario-g2ak.sh), G2AK_VORHER (Shell-Befehl vor dem Panelstart).
# labwc bekommt die Vorgabe session/querencia/labwc/rc.xml dieses Worktrees
# ohne ihre Execute-Kürzel (rc-pruefung.py). Ergebnisse unter
# nachweis/gui/<profil>/logs.
set -euo pipefail
HIER="$(cd "$(dirname "$0")" && pwd)"
WT=/home/user/projects/querencia-runtime-wt-g2ak
G=$WT/nachweis/gui; M=$G/gm-werkzeug
PANEL="$(realpath -e "${1:?Panel-Binary}")"
PROFIL="${2:?Profilname}"; SZ="$(realpath -e "${3:?Szenario}")"; LIMIT="${4:-300}"
[ -x "$G/bin/qeingabe" ] && [ -x "$G/bin/qx11fenster" ] || bash "$HIER/werkzeug-anlegen.sh"
. "$M/testprofil.sh"; testprofil_neu "$G" "$PROFIL"
mkdir -p "$G/$PROFIL/config/labwc" "$G/$PROFIL/logs"
python3 -I "$HIER/rc-pruefung.py" "$WT/session/querencia/labwc/rc.xml" >"$G/$PROFIL/config/labwc/rc.xml"
cp "$G/$PROFIL/config/labwc/rc.xml" "$G/$PROFIL/logs/rc.xml"
cd "$G"
G2AK_PANEL="$PANEL" G2AK_SZENARIO="$SZ" G2AK_BIN="$G/bin" G2AK_WERKZEUG="$HIER" G2M_WERKZEUG="$M" \
  G2AK_APPS="$WT/session/querencia/applications" \
  /usr/bin/python3 "$M/isoliert.py" "$G/$PROFIL" "$HIER/szenario-g2ak.sh" "$LIMIT"
