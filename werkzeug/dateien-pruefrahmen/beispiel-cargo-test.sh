#!/bin/bash
# Prüfrahmen Querencia Dateien: Beispiel „cargo test in der Hülle“.
#
#   beispiel-cargo-test.sh WORKSPACE [L|G] [PAKET] [-- TESTARGUMENTE]
#
# Bauen draußen (Bau-Platz, sccache, nice), Testen drinnen: Jedes
# Testbinary von PAKET (Vorgabe querencia-dateiarbeit) läuft einzeln in
# huelle.sh, mit dem Paketordner als Arbeitsordner (wie cargo test) und
# WORKSPACE nur lesend. Die Tests schreiben nur in HOME/XDG-Ordner des
# Profils, /tmp der Hülle und CARGO_TARGET_TMPDIR (target/tmp). Doctests
# laufen hier nicht (rustdoc), die laufen ohne Dateizugriff weiter draußen.
# CARGO_TARGET_DIR wird beachtet (liegt er außerhalb von WORKSPACE, wird
# er zusätzlich nur lesend freigegeben).
# Exit: 0 alle Binaries grün; sonst der erste Fehler-Exit (95 = Reste).

set -u
HIER=$(cd "$(dirname "$(readlink -f "$0")")" && pwd)
WS=$(realpath -e "${1:?WORKSPACE fehlt}") || exit 90
STUFE=${2:-L}; PAKET=${3:-querencia-dateiarbeit}
shift $(( $# < 3 ? $# : 3 ))
[ "${1:-}" = "--" ] && shift
TARGET=${CARGO_TARGET_DIR:-$WS/target}
PLATZ=$HOME/.local/state/querencia/bau-platz.sh
LOG=$(mktemp "${TMPDIR:-/tmp}/qd-cargo-XXXXXX.json")
trap 'rm -f "$LOG"' EXIT

echo "== Bauen draußen: cargo test -p $PAKET --no-run (Ziel $TARGET)"
PATH=$HOME/.cargo/bin:$PATH CARGO_TARGET_DIR=$TARGET bash "$PLATZ" \
  cargo test --manifest-path "$WS/Cargo.toml" -p "$PAKET" --no-run --offline \
  --message-format=json-render-diagnostics > "$LOG" || { echo "Bau gescheitert"; exit 1; }

mapfile -t BINS < <(python3 -I -c '
import json, os, sys
for zeile in open(sys.argv[1]):
    try:
        m = json.loads(zeile)
    except ValueError:
        continue
    if m.get("reason") == "compiler-artifact" and m.get("executable") and m["profile"]["test"]:
        print(m["executable"] + "\t" + os.path.dirname(m["manifest_path"]))
' "$LOG")
[ ${#BINS[@]} -gt 0 ] || { echo "keine Testbinaries für $PAKET"; exit 1; }

FREI=(--lesen "$WS")
case $TARGET in "$WS"/*) ;; *) FREI+=(--lesen "$TARGET") ;; esac
# CARGO_TARGET_TMPDIR (target/tmp) ist cargos Platz für Testdateien: schreibbar.
mkdir -p "$TARGET/tmp"
FREI+=(--schreiben "$TARGET/tmp")
gesamt=0
for b in "${BINS[@]}"; do
  exe=${b%%$'\t'*}; paket_dir=${b#*$'\t'}
  echo "== Testen drinnen (Stufe $STUFE): ${exe#"$TARGET"/}"
  bash "$HIER/huelle.sh" "${FREI[@]}" --ordner "$paket_dir" \
    --setenv "CARGO_MANIFEST_DIR=$paket_dir" --setenv "CARGO_PKG_NAME=$PAKET" \
    --setenv RUST_BACKTRACE=1 "$STUFE" -- "$exe" "$@"
  rc=$?
  [ "$rc" = 0 ] || [ "$gesamt" != 0 ] || gesamt=$rc
done
echo "== Ergebnis $PAKET Stufe $STUFE: Exit $gesamt"
exit "$gesamt"
