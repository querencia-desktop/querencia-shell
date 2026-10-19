#!/bin/bash
# G2-M: legt die G2-J-Prüfwerkzeuge (tools/lautstaerke-pruefung, mit fest
# eingetragenen Pfaden des G2-J-Worktrees) als umgelenkte Kopie unter
# nachweis/gui/g2jreg/ an, damit die G2-J-Endläufe (und darüber die
# W2-Regression) mit dem G2-M-Panel laufen, ohne in den G2-J-Worktree zu
# schreiben. Aus fremden Ordnern wird nur gelesen.
#   g2jreg-anlegen.sh        danach:
#   bash nachweis/gui/g2jreg/werkzeug/w2reg-anlegen.sh
#   bash nachweis/gui/g2jreg/werkzeug/endlaeufe.sh <panel> <lautstärkeregler>
set -euo pipefail
WT=/home/user/projects/querencia-runtime-wt-netzwerk
R=$WT/nachweis/gui/g2jreg
case "$R" in "$WT"/nachweis/gui/g2jreg) ;; *) exit 2 ;; esac
[ -e "$R" ] && { chmod -R u+w "$R"; rm -rf "$R"; }
mkdir -p "$R/gui"
cp -a "$WT/tools/lautstaerke-pruefung" "$R/werkzeug"
grep -rl 'querencia-runtime-wt-lautstaerke\|\$WT/nachweis/gui\|\$WT/tools/lautstaerke-pruefung' "$R/werkzeug" | while read -r f; do
  sed -i -e "s#/querencia-runtime-wt-lautstaerke/nachweis/gui#/querencia-runtime-wt-netzwerk/nachweis/gui/g2jreg/gui#g" \
         -e 's#\$WT/tools/lautstaerke-pruefung#'"$R"'/werkzeug#g' \
         -e 's#"\$WT"/nachweis/gui/w2reg#"$WT"/nachweis/gui/g2jreg/gui/w2reg#g' \
         -e 's#\$WT/nachweis/gui#$WT/nachweis/gui/g2jreg/gui#g' \
         -e "s#querencia-runtime-wt-lautstaerke#querencia-runtime-wt-netzwerk#g" "$f"
done
# Kontrolle: kein Pfad mehr in den G2-J-Worktree, Laufzeitordner im Kopieziel
! grep -rn 'wt-lautstaerke' "$R/werkzeug" || { echo "g2jreg: Pfad in den G2-J-Worktree übrig"; exit 2; }
grep -q "g2jreg/gui/rt/g2j-" "$R/werkzeug/qeingabe.c" "$R/werkzeug/qpulse.py" "$R/werkzeug/szenario-g2j.sh"
echo "G2-J-Kopie angelegt: $R"
