#!/bin/bash
# HMP-05: komplette isolierte Menüprüfung mit echtem (virtuellem) Zeiger und
# echter Tastatur. Nie in der laufenden Sitzung: isoliert.py startet einen
# eigenen headless labwc mit eigenem D-Bus und XDG-Profil; qeingabe und die
# Tests verweigern sonst den Start.
#   pruefen.sh <panel-binary> <profilname>
set -euo pipefail
HIER="$(cd "$(dirname "$0")" && pwd)"
PANEL="${1:?Panel-Binary}"
PROFIL="${2:?Profilname}"
# Arbeitsordner für Profile, Bilder und das gebaute qeingabe (außerhalb des Baums)
G="${HMP_GUI_TEST:-$HOME/projects/querencia-panel-werkstatt/hmp-05/gui-test}"
[ -x "$G/bin/qeingabe" ] || bash "$HIER/bauen.sh" "$G/bin"
cd "$G"
rm -rf "$PROFIL"
HMP_PANEL="$PANEL" HMP_SZENARIO="$HIER/szenario-hmp05.txt" HMP_QEINGABE="$G/bin/qeingabe" \
  HMP_APPS="$HIER/../../session/querencia/applications" \
  /usr/bin/python3 "$HIER/isoliert.py" "$PROFIL" "$HIER/szenario.sh" 400
python3 "$HIER/auswerten.py" "$HIER/erwartung-hmp05.json" "$G/$PROFIL/logs"
