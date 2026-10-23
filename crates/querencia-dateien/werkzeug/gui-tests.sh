#!/bin/bash
# Querencia Dateien: die GTK-Tests (#[ignore], brauchen eine Anzeige) isoliert
# im Prüfrahmen (werkzeug/dateien-pruefrahmen/huelle.sh, Stufe L, headless
# labwc, eigenes HOME und XDG_RUNTIME_DIR, kein Live-Bus). Nie in der
# Live-Sitzung: die Tests verweigern XDG_RUNTIME_DIR=/run/user/*.
#   werkzeug/gui-tests.sh [FILTER]        im Crate-Ordner
# Baut außen (cargo test --no-run über den Bau-Platz), startet innen nur
# das fertige Testprogramm.
set -euo pipefail
CRATE=$(cd "$(dirname "$(readlink -f "$0")")/.." && pwd)
REPO=$(cd "$CRATE/../.." && pwd)
HUELLE=$REPO/werkzeug/dateien-pruefrahmen/huelle.sh
FILTER=${1:-}
cd "$REPO"
BIN=$(bash ~/.local/state/querencia/bau-platz.sh cargo test -p querencia-dateien --no-run \
  --message-format=json 2>/dev/null | python3 -I -c '
import json, sys
for z in sys.stdin:
    m = json.loads(z)
    if m.get("reason") == "compiler-artifact" and m.get("profile", {}).get("test") \
       and m.get("target", {}).get("name") == "querencia-dateien" and m.get("executable"):
        print(m["executable"])')
[ -n "$BIN" ] || { echo "gui-tests: Testprogramm nicht gefunden" >&2; exit 1; }
exec bash "$HUELLE" --labwc --lesen "$BIN" --lesen "$CRATE" --zeitlimit 300 L -- \
  "$BIN" --ignored --test-threads=1 $FILTER
