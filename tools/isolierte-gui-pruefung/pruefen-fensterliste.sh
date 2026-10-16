#!/bin/bash
# HMP-06: isolierte Prüfung der Fensterliste mit echten Testfenstern (foot)
# und echtem (virtuellem) Zeiger.  pruefen-fensterliste.sh <panel-binary> <profil>
set -euo pipefail
HIER="$(cd "$(dirname "$0")" && pwd)"
PANEL="${1:?Panel-Binary}"; PROFIL="${2:?Profilname}"
G="${HMP_GUI_TEST:-$HOME/projects/querencia-panel-werkstatt/gui-test-nacht}"
[ -x "$G/bin/qeingabe" ] || bash "$HIER/bauen.sh" "$G/bin"
. "$HIER/testprofil.sh"; testprofil_neu "$G" "$PROFIL"
cd "$G"
HMP_PANEL="$PANEL" HMP_SZENARIO="$HIER/szenario-hmp06.txt" HMP_QEINGABE="$G/bin/qeingabe" \
  HMP_APPS="$HIER/../../session/querencia/applications" \
  /usr/bin/python3 "$HIER/isoliert.py" "$PROFIL" "$HIER/szenario.sh" 200
python3 "$HIER/auswerten-fensterliste.py" "$HIER/erwartung-hmp06.json" "$G/$PROFIL/logs"
