#!/bin/bash
# G2-Y: Probe des Integrationsskripts in einer Scratch-Ablage (QSY_OPT,
# QSY_STATE), nie am echten Stand. Die G2-M-Ablage c3cda503421d7e9f wird aus
# der Live-Ablage nur gelesen und in die Scratch-Ablage kopiert; der Link
# querencia-netzwerk der Scratch-Ablage muss unberührt bleiben.
#   integration-probe.sh <scratch-ordner>
set -uo pipefail
I=/home/user/projects/querencia-runtime-wt-statussymbole/integration/statussymbole-integrieren.sh
S=$(realpath -m "${1:?Scratch-Ordner}")
case "$S" in /home/user/opt/* | /home/user/.local/*) echo "nie in ~/opt oder ~/.local"; exit 2 ;; esac
[ -e "$S" ] && { chmod -R u+w "$S"; rm -rf "$S"; }
mkdir -p "$S/opt/bin" "$S/opt/panel" "$S/state"
cp -a /home/user/opt/querencia/panel/c3cda503421d7e9f "$S/opt/panel/"
ln -s "$S/opt/panel/c3cda503421d7e9f/querencia-panel" "$S/opt/bin/querencia-panel"
ln -s /nirgends/g2j-7822605b "$S/opt/bin/querencia-panel.vorher"
ln -s /nirgends/netzwerk-83ec9eca "$S/opt/bin/querencia-netzwerk"
export QSY_OPT=$S/opt QSY_STATE=$S/state
ALT=$S/opt/panel/c3cda503421d7e9f/querencia-panel
n=0; ok=0
pruef() { # bedingung-ok text
    n=$((n + 1))
    if [ "$1" = 0 ]; then ok=$((ok + 1)); echo "OK     $2"; else echo "FEHLER $2"; fi
}
schritt() { # erwarteter-exit erwarteter-zustand beschreibung befehl…
    local soll=$1 zst=$2 text=$3; shift 3
    local aus rc
    aus=$("$@" 2>&1); rc=$?
    local ist; ist=$(bash "$I" pruefen 2>/dev/null | sed -n 's/.*Zustand: //p')
    n=$((n + 1))
    if [ "$rc" = "$soll" ] && { [ -z "$zst" ] || [ "$ist" = "$zst" ]; }; then ok=$((ok + 1)); echo "OK     $text (Exit $rc, $ist)"
    else echo "FEHLER $text (Exit $rc statt $soll, Zustand $ist statt $zst)"; echo "$aus" | sed 's/^/       /'; fi
}
netz_unberuehrt() {
    [ "$(readlink "$S/opt/bin/querencia-netzwerk")" = /nirgends/netzwerk-83ec9eca ]; pruef $? "querencia-netzwerk unberührt ($1)"
}
schritt 0 "P=alt" "pruefen am Ausgang" bash "$I" pruefen
schritt 1 "P=alt" "aktivieren mit Fehler nach der Ablage: zurückgerollt" env QSY_PROBE=1 QSY_FEHLER=ablage bash "$I" aktivieren
schritt 1 "P=alt" "aktivieren mit Fehler nach .vorher: zurückgerollt" env QSY_PROBE=1 QSY_FEHLER=vorher bash "$I" aktivieren
[ "$(readlink "$S/opt/bin/querencia-panel.vorher")" = /nirgends/g2j-7822605b ]; pruef $? ".vorher nach Rückrollen wie vorher"
schritt 1 "P=alt" "aktivieren mit Fehler am Panel-Link: zurückgerollt" env QSY_PROBE=1 QSY_FEHLER=panel bash "$I" aktivieren
schritt 0 "P=neu" "aktivieren" bash "$I" aktivieren
[ "$(readlink "$S/opt/bin/querencia-panel.vorher")" = "$ALT" ]; pruef $? ".vorher zeigt auf G2-M c3cda503"
perm=$(stat -c %a "$(readlink "$S/opt/bin/querencia-panel")"); dperm=$(stat -c %a "$(dirname "$(readlink "$S/opt/bin/querencia-panel")")")
[ "$perm$dperm" = 555555 ]; pruef $? "Ablage 0555 (Datei $perm, Ordner $dperm)"
ls "$S"/opt/panel/*.herkunft >/dev/null 2>&1; pruef $? "Herkunftsdatei angelegt"
grep -q 'G2-Y Statussymbole 0.1' "$S/opt/protokoll.txt"; pruef $? "Journal-Eintrag aktivieren"
netz_unberuehrt aktivieren
schritt 0 "P=neu" "aktivieren noch einmal: bereits aktiv" bash "$I" aktivieren
schritt 2 "P=neu" "panel-neustart in der Scratch-Ablage verweigert" bash "$I" panel-neustart
schritt 1 "P=neu" "zurueck mit Fehler an .vorher: zurückgerollt" env QSY_PROBE=1 QSY_FEHLER=vorher bash "$I" zurueck
schritt 1 "P=neu" "zurueck mit Fehler am Panel-Link: zurückgerollt" env QSY_PROBE=1 QSY_FEHLER=panel bash "$I" zurueck
schritt 0 "P=alt" "zurueck" bash "$I" zurueck
v=$(readlink "$S/opt/bin/querencia-panel.vorher")
case $v in "$S"/opt/panel/*/querencia-panel) [ "$v" != "$ALT" ] ;; *) false ;; esac; pruef $? ".vorher zeigt auf G2-Y ($v)"
[ -e "$v" ]; pruef $? "G2-Y-Ablage bleibt nach zurueck"
netz_unberuehrt zurueck
schritt 0 "P=alt" "zurueck noch einmal: bereits Ausgang" bash "$I" zurueck
schritt 2 "P=alt" "panel-neustart-zurueck in der Scratch-Ablage verweigert" bash "$I" panel-neustart-zurueck
schritt 0 "P=neu" "wieder aktivieren (Ablage schon da)" bash "$I" aktivieren
exec 9>"$S/state/statussymbole-integrieren.lock"; flock -n 9
aus=$(bash "$I" zurueck 2>&1); rc=$?
[ "$rc" = 2 ] && echo "$aus" | grep -q 'anderer Lauf' && [ "$(bash "$I" pruefen 2>/dev/null | sed -n 's/.*Zustand: //p')" = P=neu ]
pruef $? "zweiter Lauf während der Sperre verweigert (Exit $rc)"
exec 9>&-
rm "$S/opt/bin/querencia-panel"; ln -s /nirgends/fremd "$S/opt/bin/querencia-panel"
schritt 2 "" "fremder Panel-Link: aktivieren verweigert" bash "$I" aktivieren
schritt 2 "" "fremder Panel-Link: zurueck verweigert" bash "$I" zurueck
[ "$(readlink "$S/opt/bin/querencia-panel")" = /nirgends/fremd ]; pruef $? "fremder Link unverändert"
rm "$S/opt/bin/querencia-panel"; ln -s "$ALT" "$S/opt/bin/querencia-panel"
p=$(ls -d "$S"/opt/panel/* | grep -v c3cda503 | grep -v herkunft | head -1)
chmod u+w "$p" "$p/querencia-panel"; echo x >>"$p/querencia-panel"
schritt 2 "" "veränderte G2-Y-Ablage: aktivieren verweigert" bash "$I" aktivieren
[ "$(readlink "$S/opt/bin/querencia-panel")" = "$ALT" ]; pruef $? "Panel-Link nach Verweigerung unverändert"
echo "$ok/$n Probeschritte bestanden"
[ "$ok" = "$n" ]
