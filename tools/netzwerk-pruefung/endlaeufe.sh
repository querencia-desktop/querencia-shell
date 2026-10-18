#!/bin/bash
# G2-M: alle Endläufe mit den Nachweis-Binaries, nacheinander.
#   endlaeufe.sh <panel-binary> <lautstärkeregler-binary> [nur-g2m|nur-reg]
# 1. G2-M: rueckfall, attrappe, spaet bei 1024x768 und 1920x1080 (laeufe.sh).
# 2. G2-J-Regression mit dem G2-M-Panel (Kopie der G2-J-Werkzeuge unter
#    nachweis/gui/g2jreg, g2jreg-anlegen.sh): haupt und ohne-dienst bei
#    beiden Auflösungen, darin die W2-Regression (Umschalter, Logo, Branding,
#    Starter, Menü dynamisch, Fensterliste).
# Zusammenfassungen: nachweis/gui/ergebnis.txt (G2-M) und
# nachweis/gui/g2jreg/gui/ergebnis.txt (G2-J, W2).
set -uo pipefail
WT=/home/user/projects/querencia-runtime-wt-netzwerk
T=$WT/tools/netzwerk-pruefung
PANEL=$(realpath -e "${1:?Panel}"); MIXER=$(realpath -e "${2:?Lautstärkeregler}"); NUR=${3:-}
if [ "$NUR" != nur-reg ]; then
  bash "$T/laeufe.sh" "$PANEL" 1024x768 1920x1080
fi
[ "$NUR" = nur-g2m ] && exit 0
bash "$T/g2jreg-anlegen.sh"
bash "$WT/nachweis/gui/g2jreg/werkzeug/w2reg-anlegen.sh" >"$WT/nachweis/gui/g2jreg/w2reg-anlegen.txt" 2>&1
bash "$WT/nachweis/gui/g2jreg/werkzeug/endlaeufe.sh" "$PANEL" "$MIXER"
