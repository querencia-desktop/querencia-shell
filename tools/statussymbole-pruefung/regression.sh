#!/bin/bash
# G2-Y: Regression G2-M (rueckfall, attrappe, spaet, verschieben bei
# 1024x768 und 1920x1080) und G2-J (haupt, ohne-dienst, beide Auflösungen,
# darin W2) mit einem Panel-Binary, über die umgelenkten Kopien von
# werkzeug-anlegen.sh. Der Lautstärkeregler wird nur gelesen (Standard: die
# Live-Ablage 17c128da9c5d7304, die auch G2-M benutzt hat).
#   regression.sh <panel-binary> [<lautstärkeregler>] [nur-g2m|nur-g2j]
# Zusammenfassungen: nachweis/gui/ergebnis.txt (G2-M),
# nachweis/gui/gj/ergebnis.txt (G2-J, W2).
set -uo pipefail
WT=/home/user/projects/querencia-runtime-wt-statussymbole
G=$WT/nachweis/gui
PANEL=$(realpath -e "${1:?Panel}")
MIXER=$(realpath -e "${2:-/home/user/opt/querencia/lautstaerke/17c128da9c5d7304/querencia-lautstaerke}")
NUR=${3:-}
bash "$WT/tools/statussymbole-pruefung/werkzeug-anlegen.sh"
if [ "$NUR" != nur-g2j ]; then
  bash "$G/gm-werkzeug/laeufe.sh" "$PANEL" 1024x768 1920x1080
fi
[ "$NUR" = nur-g2m ] && exit 0
bash "$G/gj-werkzeug/w2reg-anlegen.sh" >"$G/gj/w2reg-anlegen.txt" 2>&1 || { echo "w2reg-anlegen fehlgeschlagen"; exit 1; }
bash "$G/gj-werkzeug/endlaeufe.sh" "$PANEL" "$MIXER"
