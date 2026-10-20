#!/bin/bash
# G2-AL: alle isolierten GUI-Läufe für eine Panel-Binary.
#   laeufe-g2al.sh <panel> [auflösungen …]
# LAEUFE wählt die Arten (Standard: alle). Profile nachweis/gui/al-<art>-<breite>,
# Auswertungen nachweis/gui/ergebnis-al-*.txt, Zusammenfassung
# nachweis/gui/ergebnis-g2al.txt, Bilder zusätzlich nach .
# Benachrichtigungen 0.2.0 und das alte Panel (G2-AK) werden nur gelesen.
set -uo pipefail
WT=/home/user/projects/querencia-runtime-wt-g2al
T=$WT/tools/g2al-pruefung; G=$WT/nachweis/gui
PANEL=$(realpath -e "${1:?Panel}"); shift
AUFL=("$@"); [ ${#AUFL[@]} -eq 0 ] && AUFL=(1024x768 1920x1080)
DIENST=${G2AL_DIENST:-/home/user/opt/querencia/benachrichtigungen/eb7dda6b1ab25601/querencia-benachrichtigungen}
ALT=${G2AL_ALT:-/home/user/opt/querencia/panel/5deb1e2e9de11f3e/querencia-panel}
BILDER=${G2AL_BILDER:-
[ -x "$G/bin/qeingabe" ] && [ -f "$G/gm-werkzeug/isoliert.py" ] || bash "$T/werkzeug-anlegen.sh"
SUMME=$G/ergebnis-g2al.txt
echo "=== $(date '+%F %T') G2-AL Panel $(sha256sum "$PANEL" | cut -c1-16), Dienst $(sha256sum "$DIENST" | cut -c1-16), alt $(sha256sum "$ALT" | cut -c1-16)" >>"$SUMME"
ARTEN=${LAEUFE:-zeigen superd glocke ergaenzung-alt ergaenzung-neu standard}
mkdir -p "$BILDER"
for r in "${AUFL[@]}"; do
  w=${r%x*}; h=${r#*x}
  for art in $ARTEN; do
    p=al-$art-$w
    python3 -I "$T/szenario-g2al.py" "$w" "$h" "$art" >"$G/sz-$p.txt"
    panel=$PANEL; ini=$T/panel-g2al.ini; neu=""
    case $art in
      ergaenzung-alt) panel=$ALT; ini=$T/panel-live.ini; neu=$PANEL ;;
      ergaenzung-neu) ini=$T/panel-live.ini; neu=$PANEL ;;
      standard) ini="" ;;
    esac
    G2AL_AUFLOESUNG=$r G2AL_PANELINI=$ini G2AL_DIENST=$DIENST G2AL_NEU=$neu \
      bash "$T/pruefen-g2al.sh" "$panel" "$p" "$G/sz-$p.txt" 300 >"$G/lauf-$p.txt" 2>&1
    python3 -I "$T/auswerten-g2al.py" "$G/$p/logs" "$art" "$w" "$h" >"$G/ergebnis-$p.txt" 2>&1
    echo "G2-AL $art $r: $(tail -1 "$G/ergebnis-$p.txt")" | tee -a "$SUMME"
    for f in "$G/$p/logs"/*.png; do [ -e "$f" ] && cp "$f" "$BILDER/$p-$(basename "$f")"; done
  done
done
