#!/bin/bash
# HMP-08: isolierte Prüfung des Hintergrunds (querencia-desktop) mit Panel,
# Testfenster, Rechtsklick, vier Auflösungen, desktop.ini und Rückfall.
#   pruefen-hintergrund.sh <desktop-binary> <panel-binary> <profil>
set -euo pipefail
HIER="$(cd "$(dirname "$0")" && pwd)"
DESKTOP="${1:?Desktop-Binary}"; PANEL="${2:?Panel-Binary}"; PROFIL="${3:?Profilname}"
G="${HMP_GUI_TEST:-$HOME/projects/querencia-panel-werkstatt/gui-test-nacht}"
[ -x "$G/bin/qausgabe" ] || bash "$HIER/bauen.sh" "$G/bin"
cd "$G"; rm -rf "$PROFIL"; mkdir -p "$PROFIL/logs"
SZ="$G/$PROFIL/logs/szenario-hmp08.txt"
sed "s#QDESKTOP#$DESKTOP#g; s#QAUSGABE#$G/bin/qausgabe#g" "$HIER/szenario-hmp08.txt" > "$SZ"
HMP_PANEL="$PANEL" HMP_SZENARIO="$SZ" HMP_QEINGABE="$G/bin/qeingabe" \
  HMP_APPS="$HIER/../../session/querencia/applications" \
  /usr/bin/python3 "$HIER/isoliert.py" "$PROFIL" "$HIER/szenario.sh" 120
python3 "$HIER/auswerten-hintergrund.py" "$G/$PROFIL/logs" "$HIER/../../data/hintergruende/querencia-blau.png"
