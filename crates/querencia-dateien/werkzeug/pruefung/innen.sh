#!/bin/bash
# Hilfen für Szenarien von lauf.sh (läuft IN der Hülle). Das Szenario wird
# danach eingelesen und kann nutzen:
#   q "BEFEHL" …      qeingabe-Befehle (move/click/dclick/rclick/key/type/
#                     keydown/keyup/scroll/wait); wartet, bis alle ausgeführt sind
#   starte [ARG …]    Querencia Dateien im Hintergrund (Log: programm.log)
#   warte_log MUSTER [SEK]   wartet auf eine neue Logzeile (Vorgabe 10 s)
#   foto NAME         Bildschirmfoto fotos/NAME.png
#   fall NR TEXT OK BELEG    Ergebniszeile (OK: 1 = PASS, sonst FAIL)
#   taste TASTEN      z. B. "ctrl+l", "alt+d", "shift+ctrl+n", "F2"
#   menue NAME        datei|bearbeiten|ansicht|gehe|lesezeichen|hilfe öffnen
set -u
OUT=$LAUF_OUT; W=${LAUF_GROESSE%x*}; H=${LAUF_GROESSE#*x}
LOG=$OUT/programm.log; : > "$LOG"; : > "$OUT/ergebnis.txt"
"$LAUF_QA" "$LAUF_GROESSE" 2>>"$OUT/qausgabe.log" || echo "innen: qausgabe fehlgeschlagen" >&2
export QE_W=$W QE_H=$H
coproc QE { "$LAUF_QE" 2>>"$OUT/qeingabe.log"; }
q() {
  local c line
  for c in "$@"; do printf '%s\n' "$c" >&"${QE[1]}"; done
  printf 'mark SYNC\n' >&"${QE[1]}"
  while IFS= read -r -t 60 line <&"${QE[0]}"; do
    [[ $line == *SYNC* ]] && return 0
  done
  echo "innen: qeingabe antwortet nicht" >&2; return 1
}
PROG_PID=
starte() {
  "$LAUF_PROG" "$@" >>"$LOG" 2>&1 &
  PROG_PID=$!
}
LOG_STAND=0
warte_log() { # MUSTER [SEK]: neue Zeile seit dem letzten warte_log/markiere
  local muster=$1 sek=${2:-10} i
  for ((i = 0; i < sek * 10; i++)); do
    if tail -n +"$((LOG_STAND + 1))" "$LOG" | grep -q -- "$muster"; then
      LOG_STAND=$(grep -n -- "$muster" "$LOG" | tail -n 1 | cut -d: -f1)
      return 0
    fi
    q "wait 100"
  done
  return 1
}
markiere() { LOG_STAND=$(wc -l < "$LOG"); }
foto() { q "wait 200" "shot $OUT/fotos/$1.png"; }
fall() { local ok=FAIL; [ "$3" = 1 ] && ok=PASS; printf '%s|%s|%s|%s\n' "$1" "$2" "$ok" "$4" >> "$OUT/ergebnis.txt"; }
taste() { # ctrl+l, alt+d, shift+ctrl+n, F2, ctrl+plus
  local teile k m i mods=()
  IFS=+ read -ra teile <<< "$1"
  k=${teile[-1]}
  for m in "${teile[@]:0:${#teile[@]}-1}"; do
    case $m in ctrl) mods+=(Control_L) ;; alt) mods+=(Alt_L) ;; shift) mods+=(Shift_L) ;; super) mods+=(Super_L) ;; esac
  done
  local cmds=()
  for m in "${mods[@]}"; do cmds+=("keydown $m"); done
  case $k in
    ?) cmds+=("type $k") ;;
    plus) cmds+=("type +") ;;
    minus) cmds+=("type -") ;;
    *) cmds+=("key $k") ;;
  esac
  for ((i = ${#mods[@]} - 1; i >= 0; i--)); do cmds+=("keyup ${mods[i]}"); done
  # a menu or dialog opened by the keys needs a moment before it has the keyboard
  q "${cmds[@]}" "wait 300"
}
# Menü der Leiste über seine Zugriffstaste (de: D B A G L H, en: F E V G B H)
menue() {
  local de=(datei:d bearbeiten:b ansicht:a gehe:g lesezeichen:l hilfe:h)
  local en=(datei:f bearbeiten:e ansicht:v gehe:g lesezeichen:b hilfe:h)
  local liste=("${de[@]}") e
  [ "${LAUF_SPRACHE:-de}" = en ] && liste=("${en[@]}")
  for e in "${liste[@]}"; do [ "${e%%:*}" = "$1" ] && taste "alt+${e#*:}"; done
}
ende() {
  if [ -n "$PROG_PID" ] && kill -0 "$PROG_PID" 2>/dev/null; then kill "$PROG_PID"; wait "$PROG_PID" 2>/dev/null; fi
}
trap ende EXIT
# shellcheck disable=SC1090
source "$1"
