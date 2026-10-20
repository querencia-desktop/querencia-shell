#!/bin/bash
# G2-AK: alle isolierten GUI-Läufe für eine Panel-Binary.
#   laeufe-g2ak.sh <panel> [auflösungen …]
# LAEUFE wählt die Arten (Standard: alle). Profile nachweis/gui/ak-<art>-<breite>,
# Auswertungen nachweis/gui/ergebnis-ak-*.txt, Zusammenfassung
# nachweis/gui/ergebnis-g2ak.txt. Nie das echte querencia-sitzung (Attrappen,
# szenario-g2ak.sh).
set -uo pipefail
WT=/home/user/projects/querencia-runtime-wt-g2ak
T=$WT/tools/g2ak-pruefung; G=$WT/nachweis/gui
PANEL=$(realpath -e "${1:?Panel}"); shift
AUFL=("$@"); [ ${#AUFL[@]} -eq 0 ] && AUFL=(1024x768 1920x1080)
[ -x "$G/bin/qeingabe" ] && [ -x "$G/bin/qx11fenster" ] || bash "$T/werkzeug-anlegen.sh"
SUMME=$G/ergebnis-g2ak.txt
echo "=== $(date '+%F %T') G2-AK Panel $(sha256sum "$PANEL" | cut -c1-16)" >>"$SUMME"
ARTEN=${LAEUFE:-sitzung sitzung-variable sitzung-keins fensterliste fensterliste-alle fensterliste-vorher neuladen vollbild}
for r in "${AUFL[@]}"; do
  w=${r%x*}; h=${r#*x}
  for art in $ARTEN; do
    p=ak-$art-$w
    python3 -I "$T/szenario-g2ak.py" "$w" "$h" "$art" >"$G/sz-$p.txt"
    sitzung=pfad; ini=$T/panel-standard.ini; vorher=""
    case $art in
      sitzung-variable) sitzung=variable ;;
      sitzung-keins) sitzung=keins ;;
      fensterliste-alle) ini=$T/panel-alle.ini ;;
      fensterliste-vorher)
        vorher="python3 -I $T/testfenster.py org.querencia.TestA 'Fenster A' '#5a8fd0' >/dev/null 2>&1 & sleep 4; python3 -I $T/testfenster.py org.querencia.TestB 'Fenster B' '#5ab06a' >/dev/null 2>&1 & sleep 4" ;;
    esac
    G2AK_AUFLOESUNG=$r G2AK_SITZUNG=$sitzung G2AK_PANELINI=$ini G2AK_VORHER=$vorher \
      bash "$T/pruefen-g2ak.sh" "$PANEL" "$p" "$G/sz-$p.txt" 240 >"$G/lauf-$p.txt" 2>&1
    python3 -I "$T/auswerten-g2ak.py" "$G/$p/logs" "$art" "$w" "$h" >"$G/ergebnis-$p.txt" 2>&1
    echo "G2-AK $art $r: $(tail -1 "$G/ergebnis-$p.txt")" | tee -a "$SUMME"
  done
done
