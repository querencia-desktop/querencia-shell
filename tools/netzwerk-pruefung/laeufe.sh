#!/bin/bash
# G2-M: die isolierten GUI-Läufe des Netzwerk-Applets mit einem Panel-Binary.
#   laeufe.sh <panel-binary> [auflösungen …]   (Standard: 1024x768 1920x1080)
# Je Auflösung: rueckfall (echte eth0, echter Systembus ohne NetworkManager,
# nur lesend), attrappe (eigener Bus mit qnm.py), spaet (Attrappe startet
# später). Auswertungen: nachweis/gui/ergebnis-<art><breite>.txt,
# Zusammenfassung in nachweis/gui/ergebnis.txt.
set -uo pipefail
WT=~/projects/querencia-runtime-wt-netzwerk
T=$WT/tools/netzwerk-pruefung; G=$WT/nachweis/gui
PANEL=$(realpath -e "${1:?Panel}"); shift
AUFL=("$@"); [ ${#AUFL[@]} -eq 0 ] && AUFL=(1024x768 1920x1080)
SUMME=$G/ergebnis.txt
echo "=== $(date '+%F %T') G2-M Panel $(sha256sum "$PANEL" | cut -c1-16)" >>"$SUMME"
for r in "${AUFL[@]}"; do
  w=${r%x*}; h=${r#*x}
  for art in rueckfall attrappe spaet; do
    p=${art:0:1}$w
    case $art in
      rueckfall) netz=echt; ini=$T/panel-mit-lautstaerke.ini ;;
      attrappe) netz=attrappe; ini=$T/panel-mit-netzwerk.ini ;;
      spaet) netz=spaet; ini=$T/panel-mit-netzwerk.ini ;;
    esac
    python3 -I "$T/szenario-machen.py" "$w" "$h" "$art" >"$G/sz-$w-$art.txt"
    G2M_AUFLOESUNG=$r G2M_NETZ=$netz G2M_PANELINI=$ini bash "$T/pruefen.sh" "$PANEL" "$p" "$G/sz-$w-$art.txt" 500 >"$G/lauf-$p.txt" 2>&1
    python3 -I "$T/auswerten.py" "$G/$p/logs" "$art" "$w" "$h" >"$G/ergebnis-$p.txt" 2>&1
    echo "G2-M $art $r: $(tail -1 "$G/ergebnis-$p.txt")" | tee -a "$SUMME"
  done
done
