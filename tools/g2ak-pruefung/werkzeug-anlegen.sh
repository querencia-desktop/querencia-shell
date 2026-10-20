#!/bin/bash
# G2-AK: legt die Prüfwerkzeuge als umgelenkte Kopien unter nachweis/gui an.
#   nachweis/gui/ad-werkzeug  tools/kalender-pruefung (G2-AD), Pfade auf diesen
#                             Worktree; legt seinerseits gm-werkzeug (G2-M)
#                             und gj-werkzeug (G2-J, W2) an
#   nachweis/gui/bin          qeingabe/qausgabe (aus gm-werkzeug), qx11fenster
# Geändert gegenüber den Vorlagen (beides nur in den Kopien):
#   - qeingabe kennt bei keydown/keyup zusätzlich F11, f und l (Super+F,
#     Alt+F11, Strg+Alt+L);
#   - szenario-g2ad.py: im System-Menü steht seit G2-AK „Bildschirm sperren“
#     vor „Abmelden …“, beide Sitzungseinträge rücken eine Zeile (24 px) nach
#     unten (Abmelden y 102, Ausschalten y 126 statt 78/102).
# In fremde Worktrees wird nichts geschrieben, aus ihnen nichts gelesen.
set -euo pipefail
WT=/home/user/projects/querencia-runtime-wt-g2ak
G=$WT/nachweis/gui
A=$G/ad-werkzeug
mkdir -p "$G"
if [ -e "$A" ]; then chmod -R u+w "$A"; rm -rf "$A"; fi
cp -a "$WT/tools/kalender-pruefung" "$A"
grep -rl 'wt-kalender\|\$WT/tools/kalender-pruefung' "$A" | while read -r f; do
  sed -i -e 's#\$WT/tools/kalender-pruefung#'"$A"'#g' \
         -e 's#querencia-runtime-wt-kalender#querencia-runtime-wt-g2ak#g' \
         -e 's#wt-kalender/#wt-g2ak/#g' "$f"
done
sed -i 's#^MENU_X, ABMELDEN, AUSSCHALTEN = 300, 78, 102$#MENU_X, ABMELDEN, AUSSCHALTEN = 300, 102, 126  \# G2-AK: Sperren davor#' "$A/szenario-g2ad.py"
grep -q '^MENU_X, ABMELDEN, AUSSCHALTEN = 300, 102, 126' "$A/szenario-g2ad.py"
! grep -rn 'wt-kalender' "$A" || { echo "Pfad in einen fremden Worktree übrig"; exit 2; }
bash "$A/werkzeug-anlegen.sh"

M=$G/gm-werkzeug
sed -i 's#                     : !strcmp(arg, "F4") ? KEY_F4#                     : !strcmp(arg, "F4") ? KEY_F4 : !strcmp(arg, "F11") ? KEY_F11  /* G2-AK */\n                     : !strcmp(arg, "f") ? KEY_F : !strcmp(arg, "l") ? KEY_L#' "$M/qeingabe.c"
grep -q 'KEY_F11  /\* G2-AK' "$M/qeingabe.c"
mkdir -p "$G/bin"
bash "$M/bauen.sh" "$G/bin"
gcc -O2 -Wall -Wextra -o "$G/bin/qx11fenster" "$WT/tools/g2ak-pruefung/qx11fenster.c" -lX11
echo "Werkzeuge angelegt: $A $M $G/bin"
