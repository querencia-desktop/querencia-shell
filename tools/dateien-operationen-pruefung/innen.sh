#!/bin/bash
# Innen in der Hülle (huelle.sh L --labwc): Hilfen für die Szenarien,
# Programmstart, Aufräumen. Gesetzt von lauf.sh: QD_PROG, QD_SZ, QD_AUS, QD_QE.
set -u
LOG=$QD_AUS/programm.log
CSV=$QD_AUS/ergebnis.csv
echo "Nr;Fall;Erwartung;Ergebnis;Beleg" > "$CSV"
: > "$LOG"
APP=

# Wartet, bis PATTERN (grep -E) im Programmlog steht (höchstens S Sekunden).
warte_log() { # PATTERN [S]
  local bis=$(( $(date +%s) + ${2:-15} ))
  while [ "$(date +%s)" -le "$bis" ]; do
    grep -a -q -E -- "$1" "$LOG" && return 0
    sleep 0.1
  done
  echo "ZEITÜBERSCHREITUNG: $1" >> "$QD_AUS/innen.log"
  return 1
}
# Wartet, bis die Bedingung (bash-Ausdruck) wahr ist (höchstens S Sekunden).
warte_bis() { # AUSDRUCK [S]
  local bis=$(( $(date +%s) + ${2:-15} ))
  while [ "$(date +%s)" -le "$bis" ]; do
    eval "$1" && return 0
    sleep 0.1
  done
  echo "ZEITÜBERSCHREITUNG: $1" >> "$QD_AUS/innen.log"
  return 1
}
# Zählt, wie oft PATTERN im Log steht.
zahl_log() { grep -a -c -E -- "$1" "$LOG"; }
# Eingaben an qeingabe (stdin), z. B. eingabe <<< "key Delete".
eingabe() { "$QD_QE" >> "$QD_AUS/eingabe.log" 2>&1; }
foto() { grim "$QD_AUS/$1.png" 2>> "$QD_AUS/innen.log"; }
# Ergebniszeile: NR FALL ERWARTUNG BEDINGUNG BELEG
pruefe() {
  local nr=$1 fall=$2 erwartung=$3 bedingung=$4 beleg=${5:-}
  if eval "$bedingung"; then e=PASS; else e=FAIL; fi
  echo "$nr;$fall;$erwartung;$e;$beleg" >> "$CSV"
  echo "$nr $e $fall" >> "$QD_AUS/innen.log"
}
starte() { # [ARG …]
  # QD_DUNKEL=1: dunkle Fotos (Adwaita dunkel; die Programme setzen keine eigenen Farben)
  env QUERENCIA_DATEIEN_DEBUG=1 ${QD_LOCALE:+QUERENCIA_DATEIEN_LOCALEDIR=$QD_LOCALE} \
    ${QD_DUNKEL:+GTK_THEME=Adwaita:dark} \
    "$QD_PROG" "$@" >> "$LOG" 2>&1 &
  APP=$!
  echo "Programm PID $APP: $QD_PROG $*" >> "$QD_AUS/innen.log"
}
beende() {
  if [ -n "$APP" ] && kill -0 "$APP" 2>/dev/null; then
    kill "$APP"; wait "$APP" 2>/dev/null
  fi
  APP=
}
# Fenster maximieren (labwc-Vorgabe Super+A).
maximieren() {
  eingabe <<'E'
keydown Super_L
type a
keyup Super_L
wait 300
E
}
umgebung() {
  {
    echo "HOME=$HOME"; echo "XDG_RUNTIME_DIR=$XDG_RUNTIME_DIR"; echo "LANG=$LANG"
    echo "GIO_USE_VFS=${GIO_USE_VFS:-}"; echo "Papierkorb=${XDG_DATA_HOME:-$HOME/.local/share}/Trash"
    echo "grenze=$QD_PROFIL/grenze ($(stat -f -c %T "$QD_PROFIL/grenze"))"
  } >> "$QD_AUS/innen.log"
}

# Ausgabegröße: 1280x720 ist die des headless labwc, andere über qausgabe
W=${QD_AUFLOESUNG%x*}; H=${QD_AUFLOESUNG#*x}
if [ "$QD_AUFLOESUNG" != 1280x720 ]; then
  "$QD_QA" "$QD_AUFLOESUNG" >> "$QD_AUS/innen.log" 2>&1 || echo "qausgabe fehlgeschlagen" >> "$QD_AUS/innen.log"
fi
export QE_W=$W QE_H=$H
umgebung
echo "Auflösung $QD_AUFLOESUNG, Stufe ${GIO_USE_VFS:+L}${GIO_USE_VFS:-G}" >> "$QD_AUS/innen.log"
# shellcheck disable=SC1090
source "$QD_SZ"
beende
cp "$QD_PROFIL/labwc.log" "$QD_AUS/labwc.log" 2>/dev/null
exit 0
