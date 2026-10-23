#!/bin/bash
# Querencia Dateien: baut die Prüfwerkzeuge qeingabe (virtuelle Maus und
# Tastatur) und qausgabe (Auflösung des headless labwc) nach
# ${DATEIEN_ARBEIT:-~/projects/dateien-arbeit}/bin. Beide verweigern jede
# Sitzung außerhalb des Prüfrahmens (XDG_RUNTIME_DIR=/tmp/qd-*, QD_HUELLE).
set -euo pipefail
HIER="$(cd "$(dirname "$0")" && pwd)"
OUT=${DATEIEN_ARBEIT:-$HOME/projects/dateien-arbeit}/bin
mkdir -p "$OUT"
VP=$(ls -d ~/.cargo/registry/src/*/wayland-protocols-wlr-0.3.*/wlr-protocols/unstable/wlr-virtual-pointer-unstable-v1.xml | head -1)
OM=$(dirname "$VP")/wlr-output-management-unstable-v1.xml
for x in "$VP" "$HIER/virtual-keyboard-unstable-v1.xml" "$OM"; do
  b=$(basename "$x" .xml)
  wayland-scanner client-header "$x" "$OUT/$b-client-protocol.h"
  wayland-scanner private-code "$x" "$OUT/$b-protocol.c"
done
gcc -O2 -Wall -Wextra -I"$OUT" -o "$OUT/qeingabe" "$HIER/qeingabe.c" "$OUT"/virtual-*-protocol.c "$OUT"/wlr-virtual-*-protocol.c \
  $(pkg-config --cflags --libs wayland-client xkbcommon)
gcc -O2 -Wall -Wextra -I"$OUT" -o "$OUT/qausgabe" "$HIER/qausgabe.c" "$OUT/wlr-output-management-unstable-v1-protocol.c" \
  $(pkg-config --cflags --libs wayland-client)
echo "gebaut: $OUT/qeingabe $OUT/qausgabe"
