#!/bin/bash
# Baut qeingabe (nur Testwerkzeug, nichts wird installiert) nach $1.
set -euo pipefail
HIER="$(cd "$(dirname "$0")" && pwd)"
OUT="${1:?Zielverzeichnis}"
mkdir -p "$OUT"
VP=$(ls -d ~/.cargo/registry/src/*/wayland-protocols-wlr-0.3.*/wlr-protocols/unstable/wlr-virtual-pointer-unstable-v1.xml | head -1)
for x in "$VP" "$HIER/virtual-keyboard-unstable-v1.xml"; do
  b=$(basename "$x" .xml)
  wayland-scanner client-header "$x" "$OUT/$b-client-protocol.h"
  wayland-scanner private-code "$x" "$OUT/$b-protocol.c"
done
gcc -O2 -Wall -Wextra -I"$OUT" -o "$OUT/qeingabe" "$HIER/qeingabe.c" "$OUT"/*-protocol.c \
  $(pkg-config --cflags --libs wayland-client xkbcommon)
echo "gebaut: $OUT/qeingabe"
