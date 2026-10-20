#!/bin/bash
# Portal-Läufe (QD-062): Panel und Desktop mit echtem, fehlendem und
# hängendem Settings-Portal, je isoliert.
#   laeufe-portal.sh <panel> <desktop> <ausgabeordner> [arten]
set -u
H=$(dirname "$(realpath "$0")")
PANEL=$(realpath "$1"); DESKTOP=$(realpath "$2"); OUT=$(realpath -m "$3")
QA=${QA:-$HOME/projects/querencia-sitzung-wt-welle/werkzeug/bin/qausgabe}
mkdir -p "$OUT"
for art in ${4:-echt fehlt haengt}; do
    PORTAL=$art PRUEF_ENV="PORTAL=$art PANEL=$PANEL DESKTOP=$DESKTOP QA=$QA WARTEN=${WARTEN:-60} PRUEF_GDK_DEBUG=${PRUEF_GDK_DEBUG:-}" \
        bash "$H/isoliert.sh" "$OUT/$art" "$H/szenario-portal.sh" "${LIMIT:-90}"
    cat "$OUT/$art/logs/zeiten.txt" 2>/dev/null || echo "portal=$art: keine Zeiten"
done
