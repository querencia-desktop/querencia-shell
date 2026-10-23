#!/bin/bash
# Querencia Dateien, Welle C1: Fotosätze der Netzwerk-Oberfläche
# (fotos.sh) in 1024×768 und 1920×1080, hell und dunkel, dazu englisch
# 1024×768 hell. Ergebnis je Satz unter laeufe/f-<größe>-<schema>[-en]/.
set -u
HIER=$(cd "$(dirname "$(readlink -f "$0")")" && pwd)
A=${DATEIEN_ARBEIT:-$HOME/projects/dateien-netzwerk-arbeit}
ZUS=$A/laeufe/zusammenfassung-fotos.txt
: > "$ZUS"
satz() { # NAME GRÖSSE DUNKEL SPRACHE
  LAUF_WSDD=1 LAUF_SSH_HOSTKEY_FRAGE=1 LAUF_GROESSE=$2 LAUF_DUNKEL=$3 LAUF_SPRACHE=$4 \
    bash "$HIER/lauf.sh" "$1" "$HIER/szenarien/fotos.sh" > "$A/laeufe/$1.txt" 2>&1
  echo "== $1 (Hülle $?)" >> "$ZUS"
  cat "$A/laeufe/$1/ergebnis.txt" >> "$ZUS" 2>/dev/null
}
satz f-1024-hell 1024x768 0 de
satz f-1024-dunkel 1024x768 1 de
satz f-1920-hell 1920x1080 0 de
satz f-1920-dunkel 1920x1080 1 de
satz f-1024-hell-en 1024x768 0 en
echo "PASS $(grep -c '|PASS|' "$ZUS")  FAIL $(grep -c '|FAIL|' "$ZUS")" >> "$ZUS"
tail -n 1 "$ZUS"
