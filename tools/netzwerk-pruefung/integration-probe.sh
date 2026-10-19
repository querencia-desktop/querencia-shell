#!/bin/bash
# G2-M: Probe des Integrationsskripts in einer Scratch-Ablage (QNW_OPT,
# QNW_STATE), nie am echten Stand. Die G2-J-Ablage wird aus der Live-Ablage
# nur gelesen und in die Scratch-Ablage kopiert.
#   integration-probe.sh <scratch-ordner>
set -uo pipefail
I=/home/user/projects/querencia-runtime-wt-netzwerk/integration/netzwerk-integrieren.sh
S=$(realpath -m "${1:?Scratch-Ordner}")
case "$S" in /home/user/opt/*) echo "nie in ~/opt"; exit 2 ;; esac
[ -e "$S" ] && { chmod -R u+w "$S"; rm -rf "$S"; }
mkdir -p "$S/opt/bin" "$S/opt/panel" "$S/state"
cp -a /home/user/opt/querencia/panel/7822605b1b90fd3c "$S/opt/panel/"
ln -s "$S/opt/panel/7822605b1b90fd3c/querencia-panel" "$S/opt/bin/querencia-panel"
ln -s /nirgends/qrel04d "$S/opt/bin/querencia-panel.vorher"
export QNW_OPT=$S/opt QNW_STATE=$S/state
n=0; ok=0
schritt() { # erwarteter-exit erwarteter-zustand beschreibung befehl…
    local soll=$1 zst=$2 text=$3; shift 3
    local aus rc
    aus=$("$@" 2>&1); rc=$?
    local ist; ist=$(bash "$I" pruefen 2>/dev/null | sed -n 's/.*Zustand: //p')
    n=$((n + 1))
    if [ "$rc" = "$soll" ] && { [ -z "$zst" ] || [ "$ist" = "$zst" ]; }; then ok=$((ok + 1)); echo "OK     $text (Exit $rc, $ist)"
    else echo "FEHLER $text (Exit $rc statt $soll, Zustand $ist statt $zst)"; echo "$aus" | sed 's/^/       /'; fi
}
schritt 0 "P=alt N=alt" "pruefen am Ausgang" bash "$I" pruefen
schritt 1 "P=alt N=alt" "aktivieren mit Fehler am Panel-Link: zurückgerollt" env QNW_PROBE=1 QNW_FEHLER=panel bash "$I" aktivieren
schritt 1 "P=alt N=alt" "aktivieren mit Fehler nach der Ablage: zurückgerollt" env QNW_PROBE=1 QNW_FEHLER=ablage bash "$I" aktivieren
schritt 1 "P=alt N=alt" "aktivieren mit Fehler am Diagnose-Link: zurückgerollt" env QNW_PROBE=1 QNW_FEHLER=diagnose bash "$I" aktivieren
schritt 0 "P=neu N=neu" "aktivieren" bash "$I" aktivieren
v=$(readlink "$S/opt/bin/querencia-panel.vorher")
n=$((n + 1)); [ "$v" = "$S/opt/panel/7822605b1b90fd3c/querencia-panel" ] && { ok=$((ok + 1)); echo "OK     .vorher zeigt auf G2-J"; } || echo "FEHLER .vorher: $v"
perm=$(stat -c %a "$(readlink "$S/opt/bin/querencia-panel")")
n=$((n + 1)); [ "$perm" = 555 ] && { ok=$((ok + 1)); echo "OK     Ablage 0555"; } || echo "FEHLER Ablage $perm"
schritt 0 "P=neu N=neu" "aktivieren noch einmal: bereits aktiv" bash "$I" aktivieren
schritt 2 "P=neu N=neu" "panel-neustart in der Scratch-Ablage verweigert" bash "$I" panel-neustart
schritt 1 "P=neu N=neu" "zurueck mit Fehler: zurückgerollt" env QNW_PROBE=1 QNW_FEHLER=diagnose-weg bash "$I" zurueck
schritt 0 "P=alt N=alt" "zurueck" bash "$I" zurueck
v=$(readlink "$S/opt/bin/querencia-panel.vorher")
n=$((n + 1)); case $v in "$S"/opt/panel/*/querencia-panel) [ "$v" != "$S/opt/panel/7822605b1b90fd3c/querencia-panel" ] && { ok=$((ok + 1)); echo "OK     .vorher zeigt auf G2-M"; } || echo "FEHLER .vorher: $v" ;; *) echo "FEHLER .vorher: $v" ;; esac
schritt 0 "P=alt N=alt" "zurueck noch einmal: bereits Ausgang" bash "$I" zurueck
schritt 0 "P=neu N=neu" "wieder aktivieren (Ablagen schon da)" bash "$I" aktivieren
rm "$S/opt/bin/querencia-netzwerk"; echo fremd >"$S/opt/bin/querencia-netzwerk"
schritt 2 "" "fremder Diagnose-Link: aktivieren verweigert" bash "$I" aktivieren
schritt 2 "" "fremder Diagnose-Link: zurueck verweigert" bash "$I" zurueck
echo "$ok/$n Probeschritte bestanden"
[ "$ok" = "$n" ]
