#!/bin/bash
# Alle Szenarien der Dateioperationen nacheinander, je eine eigene Hülle.
#   gesamt.sh PROGRAMM AUSGABE [dunkel]
# Braucht: QD_LOCALE (Kataloge, i18n.py mo), für zwischenablage QD_DESKTOP
# (Desktop 0.3.0) und QD_FILES (Querencia Files), beide nur lesend eingeblendet.
# QD_SZENARIEN wählt aus, QD_STUFE (L|G) und QD_AUFLOESUNG gehen an lauf.sh;
# QD_PROG_EIGENSCHAFTEN: Bau mit b1-anhaengen.patch für das Szenario eigenschaften
# (der Dialog hängt erst nach der Integration an win.eigenschaften).
set -u
HIER=$(cd "$(dirname "$(readlink -f "$0")")" && pwd)
PROG=${1:?Programm}; AUS=${2:?Ausgabe}; MODUS=${3:-hell}
mkdir -p "$AUS"
[ "$MODUS" = dunkel ] && export QD_DUNKEL=1
SZ=${QD_SZENARIEN:-ziehen fortschritt konflikt papierkorb zwischenablage eigenschaften}
for s in $SZ; do
  extra=()
  if [ "$s" = zwischenablage ]; then
    [ -n "${QD_DESKTOP:-}" ] && [ -n "${QD_FILES:-}" ] || { echo "$s: QD_DESKTOP/QD_FILES fehlen, übersprungen"; continue; }
    extra=(--lesen "$(dirname "$QD_DESKTOP")" --lesen "$(dirname "$QD_FILES")" --setenv "QD_DESKTOP=$QD_DESKTOP" --setenv "QD_FILES=$QD_FILES")
  fi
  p=$PROG
  [ "$s" = eigenschaften ] && [ -n "${QD_PROG_EIGENSCHAFTEN:-}" ] && p=$QD_PROG_EIGENSCHAFTEN
  rm -rf "$AUS/$s"
  bash "$HIER/lauf.sh" "$p" "$HIER/szenarien/$s.sh" "$AUS/$s" "${extra[@]}" > "$AUS/$s.txt" 2>&1
  echo "$s: Exit $? – $(grep -c ';PASS;' "$AUS/$s/ergebnis.csv") PASS, $(grep -c ';FAIL;' "$AUS/$s/ergebnis.csv") FAIL"
done | tee "$AUS/summe.txt"
