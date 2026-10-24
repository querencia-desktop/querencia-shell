#!/bin/bash
# Querencia Dateien, Ansichten: Release-Bau über die Bau-Plätze
# (~/.local/state/querencia/bau-platz.sh, nice 10) ins target/ des
# Worktrees, für die isolierten Läufe (lauf.sh). Nichts wird abgelegt oder
# aktiviert.
set -euo pipefail
HIER=$(cd "$(dirname "$(readlink -f "$0")")" && pwd)
WT=$(cd "$HIER/../../../.." && pwd)
cd "$WT"
bash "$HOME/.local/state/querencia/bau-platz.sh" cargo build --release -p querencia-dateien
sha256sum "$WT/target/release/querencia-dateien"
