#!/bin/bash
# G2-J: legt die W2-Prüfwerkzeuge als Kopie mit umgelenkten Pfaden unter
# nachweis/gui/w2reg/ an (für endlaeufe.sh). Aus dem W2-Ordner wird nur
# gelesen: Werkzeuge und die Referenzläufe hmp11b-neu, qrel04c-dialog-alt, logo-alt-32.
set -euo pipefail
WT=/home/user/projects/querencia-runtime-wt-lautstaerke
W2=/home/user/projects/querencia-werkstatt-w2-panel
R=$WT/nachweis/gui/w2reg
case "$R" in "$WT"/nachweis/gui/w2reg) ;; *) exit 2 ;; esac
[ -e "$R" ] && { chmod -R u+w "$R"; rm -rf "$R"; }
mkdir -p "$R/gui" "$R/rt"
cp -a "$W2/nachweis/werkzeug" "$R/werkzeug"
cp -a "$W2/nachweis/gui/hmp11b-neu" "$W2/nachweis/gui/qrel04c-dialog-alt" "$W2/nachweis/gui/logo-alt-32" "$R/gui/"
grep -rl 'querencia-werkstatt-w2-panel' "$R/werkzeug" | while read -r f; do
  sed -i -e "s#$W2/nachweis/werkzeug#$R/werkzeug#g" -e "s#$W2/nachweis/gui#$R/gui#g" \
         -e "s#$W2/nachweis/rt#$R/rt#g" \
         -e "s#/querencia-werkstatt-w2-panel/nachweis/rt/w2-#/querencia-runtime-wt-lautstaerke/nachweis/gui/w2reg/rt/w2-#g" "$f"
done
O=$R/gui/bin
bash "$R/werkzeug/bauen.sh" "$O"
FT=$(ls -d ~/.cargo/registry/src/*/wayland-protocols-wlr-0.3.*/wlr-protocols/unstable/wlr-foreign-toplevel-management-unstable-v1.xml | head -1)
for x in "$FT" /usr/share/wayland-protocols/staging/ext-foreign-toplevel-list/ext-foreign-toplevel-list-v1.xml \
         /usr/share/wayland-protocols/staging/ext-workspace/ext-workspace-v1.xml \
         /home/user/src/labwc-w2-proto/protocols/labwc-toplevel-workspace-unstable-v1.xml; do
  b=$(basename "$x" .xml); wayland-scanner client-header "$x" "$O/$b-client-protocol.h"; wayland-scanner private-code "$x" "$O/$b-protocol.c"
done
gcc -O2 -Wall -Wextra -I"$O" -o "$O/qprotokoll" "$R/werkzeug/qprotokoll.c" "$O"/wlr-foreign-toplevel-management-unstable-v1-protocol.c \
  "$O"/ext-foreign-toplevel-list-v1-protocol.c "$O"/ext-workspace-v1-protocol.c "$O"/labwc-toplevel-workspace-unstable-v1-protocol.c \
  $(pkg-config --cflags --libs wayland-client)
echo "W2-Kopie angelegt: $R"
