#!/bin/bash
# G2-R: Cargo-Aufruf in diesem Worktree (Ziel ./target) mit höchstens zwei
# Jobs unter nice, unter dem gemeinsamen exklusiven Build-Lock der
# Werkstatt-Sitzungen. Toolchain wie die Live-Binaries: /usr/bin/cargo (1.93.1).
#   bash werkzeug/bauen.sh <cargo-befehl> [argumente …]
set -euo pipefail
WT="$(cd "$(dirname "$0")/.." && pwd)"
LOCK=/home/user/.local/state/querencia/bau.lock
export CARGO_TARGET_DIR="$WT/target"
export CARGO_BUILD_JOBS=2
cd "$WT"
SUB="$1"; shift
exec flock --verbose "$LOCK" nice -n 10 /usr/bin/cargo "$SUB" --locked --offline "$@"
