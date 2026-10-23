#!/bin/bash
# Isolierter GUI-Lauf der Dateioperationen (Querencia Dateien, B3) im
# Prüfrahmen Stufe L mit headless labwc. Nie in der Live-Sitzung.
#
#   lauf.sh PROGRAMM SZENARIO AUSGABE [--sprache de_DE.UTF-8] [weitere huelle-Optionen]
#
# PROGRAMM  Binary (querencia-dateien oder die Wegwerf-Probe)
# SZENARIO  Bash-Datei, innen mit den Hilfen aus innen.sh ausgeführt
# AUSGABE   Ordner für Log, Fotos und Ergebnis-CSV (wird angelegt)
# Umgebung: QD_STUFE (L|G, Vorgabe L), QD_AUFLOESUNG (Vorgabe 1280x720; sonst
#           über qausgabe, z. B. 1024x768, 1920x1080), QD_DUNKEL=1 (dunkel),
#           QD_LOCALE (Kataloge), QD_QEINGABE/QD_QAUSGABE (Werkzeuge)
set -u
HIER=$(cd "$(dirname "$(readlink -f "$0")")" && pwd)
RAHMEN=$(cd "$HIER/../../werkzeug/dateien-pruefrahmen" && pwd)
PROG=$(readlink -f "${1:?Programm}"); SZ=$(readlink -f "${2:?Szenario}"); AUS=${3:?Ausgabe}
shift 3
mkdir -p "$AUS"; AUS=$(readlink -f "$AUS")
QE=${QD_QEINGABE:-$HOME/projects/dateien-operationen-arbeit/bin/qeingabe}
QA=${QD_QAUSGABE:-$(dirname "$QE")/qausgabe}
[ -x "$QE" ] || { echo "qeingabe fehlt: $QE (bauen.sh)"; exit 90; }
SPRACHE=de_DE.UTF-8
if [ "${1:-}" = "--sprache" ]; then SPRACHE=$2; shift 2; fi
bash "$RAHMEN/huelle.sh" --labwc --sprache "$SPRACHE" \
  --lesen "$HIER" --lesen "$(dirname "$PROG")" --lesen "$(dirname "$QE")" --lesen "$(dirname "$SZ")" \
  ${QD_LOCALE:+--lesen "$QD_LOCALE"} \
  --schreiben "$AUS" --tmpfs grenze:64M \
  --setenv "QD_PROG=$PROG" --setenv "QD_SZ=$SZ" --setenv "QD_AUS=$AUS" --setenv "QD_QE=$QE" --setenv "QD_QA=$QA" \
  --setenv "QD_AUFLOESUNG=${QD_AUFLOESUNG:-1280x720}" \
  ${QD_LOCALE:+--setenv "QD_LOCALE=$QD_LOCALE"} ${QD_DUNKEL:+--setenv "QD_DUNKEL=1"} \
  --protokoll "$AUS/huelle.json" --zeitlimit "${QD_ZEITLIMIT:-600}" "$@" \
  "${QD_STUFE:-L}" -- bash "$HIER/innen.sh"
rc=$?
echo "Lauf $(basename "$SZ"): Exit $rc, Ergebnis $AUS/ergebnis.csv"
[ -f "$AUS/ergebnis.csv" ] && column -t -s ';' "$AUS/ergebnis.csv" | tail -n +1
exit $rc
