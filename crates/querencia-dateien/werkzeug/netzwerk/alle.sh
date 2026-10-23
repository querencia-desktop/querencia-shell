#!/bin/bash
# Querencia Dateien, Welle C1: alle Netz-Szenarien nacheinander (je eine
# Hülle, Stufe G), Zusammenfassung in ${DATEIEN_ARBEIT}/laeufe/zusammenfassung-PRÄFIX.txt.
#   alle.sh [PRÄFIX]      Umgebung wie lauf.sh (LAUF_GROESSE, LAUF_DUNKEL, LAUF_SPRACHE, LAUF_BIN)
set -u
HIER=$(cd "$(dirname "$(readlink -f "$0")")" && pwd)
A=${DATEIEN_ARBEIT:-$HOME/projects/dateien-netzwerk-arbeit}
P=${1:-n}
ZUS=$A/laeufe/zusammenfassung-$P.txt
: > "$ZUS"
lauf() { # NAME SZENARIO [VAR=WERT …]
  local name=$1 sz=$2; shift 2
  env "$@" bash "$HIER/lauf.sh" "$P-$name" "$HIER/szenarien/$sz.sh" > "$A/laeufe/$P-$name.txt" 2>&1
  local rc=$?
  echo "== $P-$name (Hülle $rc)" >> "$ZUS"
  cat "$A/laeufe/$P-$name/ergebnis.txt" >> "$ZUS" 2>/dev/null
}
lauf sftp sftp LAUF_SSH_HOSTKEY_FRAGE=1
lauf sftp-kennwort sftp-kennwort LAUF_SSH_OHNE_SCHLUESSEL=1
lauf webdav webdav
lauf fehler fehler LAUF_ZEITGRENZE=6
lauf netzwerk netzwerk LAUF_WSDD=1
lauf ftp ftp
lauf fotos fotos LAUF_WSDD=1 LAUF_SSH_HOSTKEY_FRAGE=1
echo "PASS $(grep -c '|PASS|' "$ZUS")  FAIL $(grep -c '|FAIL|' "$ZUS")" >> "$ZUS"
tail -n 1 "$ZUS"
