#!/bin/bash
# G2-AD: isolierte GUI-Läufe für Kalender, System-Menü und Symbolthema mit
# den umgelenkten G2-M-Werkzeugen (werkzeug-anlegen.sh legt sie bei Bedarf an).
#   laeufe-g2ad.sh <panel> <desktop> [auflösungen …]
# LAEUFE wählt die Arten (Standard: kalender system thema thema-start).
# querencia-sitzung kommt aus PATH bzw. ~/opt/querencia/bin (nur gelesen) und
# läuft mit QS_TROCKENLAUF=1 im isolierten labwc: Abmelden und Ausschalten
# werden nur protokolliert.
# Profile nachweis/gui/ad-<art>-<breite>; Auswertungen
# nachweis/gui/ergebnis-ad-*.txt, Zusammenfassung nachweis/gui/ergebnis-g2ad.txt.
set -uo pipefail
WT=~/projects/querencia-runtime-wt-kalender
T=$WT/tools/kalender-pruefung; G=$WT/nachweis/gui; M=$G/gm-werkzeug
PANEL=$(realpath -e "${1:?Panel}"); DESKTOP=$(realpath -e "${2:?Desktop}"); shift 2
AUFL=("$@"); [ ${#AUFL[@]} -eq 0 ] && AUFL=(1024x768 1920x1080)
[ -x "$M/pruefen.sh" ] || bash "$T/werkzeug-anlegen.sh"
[ -x "$G/bin/qeingabe" ] || bash "$M/bauen.sh" "$G/bin"
SUMME=$G/ergebnis-g2ad.txt
echo "=== $(date '+%F %T') G2-AD Panel $(sha256sum "$PANEL" | cut -c1-16), Desktop $(sha256sum "$DESKTOP" | cut -c1-16)" >>"$SUMME"
for r in "${AUFL[@]}"; do
  w=${r%x*}; h=${r#*x}
  for art in ${LAEUFE:-kalender system thema thema-start}; do
    p=ad-$art-$w
    python3 -I "$T/szenario-g2ad.py" "$w" "$h" "$art" >"$G/sz-ad-$w-$art.txt"
    desktop=""; start=""; extra=()
    case $art in
      thema) desktop=$DESKTOP ;;
      thema-start) desktop=$DESKTOP; start=Adwaita; extra=("$G/ad-thema-$w/logs") ;;
    esac
    QS_TROCKENLAUF=1 G2AD_DESKTOP=$desktop G2AD_THEMA_START=$start \
    G2M_AUFLOESUNG=$r G2M_NETZ=echt G2M_PANELINI=$M/panel-mit-netzwerk.ini G2M_VORHER=$T/vorher-g2ad.sh \
      bash "$M/pruefen.sh" "$PANEL" "$p" "$G/sz-ad-$w-$art.txt" 600 >"$G/lauf-$p.txt" 2>&1
    python3 -I "$T/auswerten-g2ad.py" "$G/$p/logs" "$art" "$w" "$h" "${extra[@]}" >"$G/ergebnis-$p.txt" 2>&1
    echo "G2-AD $art $r: $(tail -1 "$G/ergebnis-$p.txt")" | tee -a "$SUMME"
  done
done
