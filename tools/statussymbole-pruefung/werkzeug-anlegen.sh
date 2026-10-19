#!/bin/bash
# G2-Y: legt die G2-M- und G2-J-Prüfwerkzeuge (tools/netzwerk-pruefung,
# tools/lautstaerke-pruefung, mit fest eingetragenen Pfaden ihrer Worktrees)
# als umgelenkte Kopien in diesem Worktree an. In fremde Worktrees wird
# nichts geschrieben, aus ihnen nichts gelesen; die Werkzeuge kommen aus den
# eingecheckten Kopien unter tools/.
#   nachweis/gui/gm-werkzeug   G2-M-Werkzeuge, Läufe in nachweis/gui/<profil>
#   nachweis/gui/gj-werkzeug   G2-J-Werkzeuge, Läufe in nachweis/gui/gj/<profil>,
#                              W2-Kopie (w2reg-anlegen.sh) in nachweis/gui/gj/w2reg
# Die G2-J-Läufe liegen eine Ebene flacher als bei G2-M (g2jreg/gui): Der
# längere Worktree-Name brächte sonst …/w2reg/rt/w2-xxxxxxxx/wayland-0 und
# …/rt/g2j-xxxxxxxx/pulse/native über die 107 Zeichen eines Unix-Sockets.
set -euo pipefail
WT=~/projects/querencia-runtime-wt-statussymbole
G=$WT/nachweis/gui
weg() { # nur die eigenen Kopien unter nachweis/gui
  case "$1" in "$G"/gm-werkzeug|"$G"/gj-werkzeug) ;; *) exit 2 ;; esac
  if [ -e "$1" ]; then chmod -R u+w "$1"; rm -rf "$1"; fi
}
mkdir -p "$G" "$G/gj"
weg "$G/gm-werkzeug"; weg "$G/gj-werkzeug"

# G2-M
M=$G/gm-werkzeug
cp -a "$WT/tools/netzwerk-pruefung" "$M"
grep -rl 'querencia-runtime-wt-netzwerk\|\$WT/tools/netzwerk-pruefung' "$M" | while read -r f; do
  sed -i -e 's#\$WT/tools/netzwerk-pruefung#'"$M"'#g' \
         -e 's#querencia-runtime-wt-netzwerk#querencia-runtime-wt-statussymbole#g' "$f"
done
rm -f "$M/integration-probe.sh" "$M/g2jreg-anlegen.sh"   # gehören zu G2-M, hier nicht gebraucht
rm -f "$M/symbole-machen.py"                             # schriebe in crates/, hier nicht gebraucht
# Der Auswerter findet die Symbole relativ zu seinem Ort (tools/…/../..);
# die Kopie liegt zwei Ebenen tiefer.
sed -i 's#^SYMBOLE = Path(__file__).resolve().parent.parent.parent / #SYMBOLE = Path("'"$WT"'") / #' "$M/auswerten.py"
grep -q "^SYMBOLE = Path(\"$WT\") / \"crates/querencia-netzwerk/data/symbole\"" "$M/auswerten.py"
sed -i -e '/g2jreg-anlegen.sh\|g2jreg\/werkzeug/d' "$M/endlaeufe.sh"

# G2-J (mit W2)
J=$G/gj-werkzeug
cp -a "$WT/tools/lautstaerke-pruefung" "$J"
grep -rl 'querencia-runtime-wt-lautstaerke\|\$WT/nachweis/gui\|\$WT/tools/lautstaerke-pruefung' "$J" | while read -r f; do
  sed -i -e "s#/querencia-runtime-wt-lautstaerke/nachweis/gui#/querencia-runtime-wt-statussymbole/nachweis/gui/gj#g" \
         -e 's#\$WT/tools/lautstaerke-pruefung#'"$J"'#g' \
         -e 's#"\$WT"/nachweis/gui#"$WT"/nachweis/gui/gj#g' \
         -e 's#\$WT/nachweis/gui#$WT/nachweis/gui/gj#g' \
         -e "s#querencia-runtime-wt-lautstaerke#querencia-runtime-wt-statussymbole#g" "$f"
done

# Kontrolle: kein Pfad in fremde Worktrees, Laufzeitordner im Kopieziel
! grep -rn 'wt-netzwerk\|wt-lautstaerke' "$M" "$J" || { echo "Pfad in einen fremden Worktree übrig"; exit 2; }
grep -q 'wt-statussymbole/nachweis/gui/rt/g2m-' "$M/qeingabe.c" "$M/szenario-g2m.sh" "$M/isoliert.py" 2>/dev/null ||
  grep -q 'wt-statussymbole/nachweis/gui/rt' "$M/isoliert.py"
grep -q 'wt-statussymbole/nachweis/gui/gj/rt/g2j-' "$J/qeingabe.c" "$J/qpulse.py" "$J/szenario-g2j.sh"
grep -q 'wt-statussymbole/nachweis/gui/gj/w2reg/rt/w2-' "$J/w2reg-anlegen.sh"
echo "Kopien angelegt: $M $J"
