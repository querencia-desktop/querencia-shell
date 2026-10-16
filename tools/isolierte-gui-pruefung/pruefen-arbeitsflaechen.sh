#!/bin/bash
# HMP-07: isolierte Prüfung des Arbeitsflächenumschalters mit N Arbeitsflächen
# (nur im Test-rc.xml des isolierten Profils) und echtem (virtuellem) Zeiger.
#   pruefen-arbeitsflaechen.sh <panel-binary> <profil> <anzahl> <szenario> [erwartung]
set -euo pipefail
HIER="$(cd "$(dirname "$0")" && pwd)"
PANEL="${1:?Panel-Binary}"; PROFIL="${2:?Profilname}"; N="${3:?Anzahl}"; SZ="${4:?Szenario}"
G="${HMP_GUI_TEST:-$HOME/projects/querencia-panel-werkstatt/gui-test-nacht}"
[ -x "$G/bin/qeingabe" ] || bash "$HIER/bauen.sh" "$G/bin"
cd "$G"; rm -rf "$PROFIL"; mkdir -p "$PROFIL/config/labwc"
cat > "$PROFIL/config/labwc/rc.xml" <<RC
<?xml version="1.0"?>
<labwc_config>
  <core><decoration>server</decoration></core>
  <theme><name>Clearlooks</name></theme>
  <desktops number="$N" popupTime="0" />
</labwc_config>
RC
HMP_PANEL="$PANEL" HMP_SZENARIO="$SZ" HMP_QEINGABE="$G/bin/qeingabe" \
  HMP_APPS="$HIER/../../session/querencia/applications" \
  /usr/bin/python3 "$HIER/isoliert.py" "$PROFIL" "$HIER/szenario.sh" 120
if [ -n "${5:-}" ]; then python3 "$HIER/auswerten-arbeitsflaechen.py" "$5" "$G/$PROFIL/logs"; fi
