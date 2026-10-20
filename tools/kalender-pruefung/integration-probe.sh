#!/bin/bash
# G2-AD: Probe von integration/kalender-integrieren.sh in einer Scratch-Ablage
# (QKA_OPT, QKA_STATE, QKA_CONFIG), nie am echten Stand. Die Ausgangs-Ablagen
# Panel G2-Y 97461a61eed7ab6b und Desktop QE-04a f48795676f867b82 werden aus
# der Live-Ablage nur gelesen und kopiert; der Link querencia-sitzung der
# Scratch-Ablage muss unberührt bleiben.
#   integration-probe.sh <scratch-ordner>
set -uo pipefail
I=/home/user/projects/querencia-runtime-wt-kalender/integration/kalender-integrieren.sh
S=$(realpath -m "${1:?Scratch-Ordner}")
case "$S" in /home/user/opt/* | /home/user/.local/* | /home/user/.config/*) echo "nie in ~/opt, ~/.local oder ~/.config"; exit 2 ;; esac
[ -e "$S" ] && { chmod -R u+w "$S"; rm -rf "$S"; }
mkdir -p "$S/opt/bin" "$S/opt/panel" "$S/opt/desktop" "$S/state" "$S/config"
cp -a /home/user/opt/querencia/panel/97461a61eed7ab6b "$S/opt/panel/"
cp -a /home/user/opt/querencia/desktop/f48795676f867b82 "$S/opt/desktop/"
ln -s "$S/opt/panel/97461a61eed7ab6b/querencia-panel" "$S/opt/bin/querencia-panel"
ln -s "$S/opt/desktop/f48795676f867b82/querencia-desktop" "$S/opt/bin/querencia-desktop"
ln -s /nirgends/g2m-c3cda503 "$S/opt/bin/querencia-panel.vorher"
ln -s /nirgends/qe04-2c78da3a "$S/opt/bin/querencia-desktop.vorher"
ln -s /nirgends/sitzung-b9eb15ac "$S/opt/bin/querencia-sitzung"
export QKA_OPT=$S/opt QKA_STATE=$S/state QKA_CONFIG=$S/config
P_ALT=$S/opt/panel/97461a61eed7ab6b/querencia-panel
D_ALT=$S/opt/desktop/f48795676f867b82/querencia-desktop
n=0; ok=0
pruef() { # bedingung-ok text
    n=$((n + 1))
    if [ "$1" = 0 ]; then ok=$((ok + 1)); echo "OK     $2"; else echo "FEHLER $2"; fi
}
zustand() { bash "$I" pruefen 2>/dev/null | sed -n 's/.*Zustand: //p'; }
schritt() { # erwarteter-exit erwarteter-zustand beschreibung befehl…
    local soll=$1 zst=$2 text=$3; shift 3
    local aus rc ist
    aus=$("$@" 2>&1); rc=$?
    ist=$(zustand)
    n=$((n + 1))
    if [ "$rc" = "$soll" ] && { [ -z "$zst" ] || [ "$ist" = "$zst" ]; }; then ok=$((ok + 1)); echo "OK     $text (Exit $rc, $ist)"
    else echo "FEHLER $text (Exit $rc statt $soll, Zustand $ist statt $zst)"; echo "$aus" | sed 's/^/       /'; fi
}
vorher_ausgang() {
    [ "$(readlink "$S/opt/bin/querencia-panel.vorher")" = /nirgends/g2m-c3cda503 ] &&
        [ "$(readlink "$S/opt/bin/querencia-desktop.vorher")" = /nirgends/qe04-2c78da3a ]
    pruef $? ".vorher beider Links wie am Ausgang ($1)"
}
sitzung_unberuehrt() {
    [ "$(readlink "$S/opt/bin/querencia-sitzung")" = /nirgends/sitzung-b9eb15ac ]; pruef $? "querencia-sitzung unberührt ($1)"
}
AUS="P=alt D=alt"; NEU="P=neu D=neu"
schritt 0 "$AUS" "pruefen am Ausgang" bash "$I" pruefen
for stelle in ablage vorher panel desktop-vorher desktop; do
    schritt 1 "$AUS" "aktivieren mit Fehler an Stelle $stelle: zurückgerollt" env QKA_PROBE=1 QKA_FEHLER=$stelle bash "$I" aktivieren
done
vorher_ausgang "nach dem Rückrollen"
schritt 0 "$NEU" "aktivieren" bash "$I" aktivieren
[ "$(readlink "$S/opt/bin/querencia-panel.vorher")" = "$P_ALT" ]; pruef $? "Panel .vorher zeigt auf G2-Y 97461a61"
[ "$(readlink "$S/opt/bin/querencia-desktop.vorher")" = "$D_ALT" ]; pruef $? "Desktop .vorher zeigt auf QE-04a f4879567"
for l in querencia-panel querencia-desktop; do
    z=$(readlink "$S/opt/bin/$l"); perm=$(stat -c %a "$z"); dperm=$(stat -c %a "$(dirname "$z")")
    [ "$perm$dperm" = 555555 ]; pruef $? "Ablage $l 0555 (Datei $perm, Ordner $dperm)"
done
[ "$(ls "$S"/opt/panel/*.herkunft "$S"/opt/desktop/*.herkunft 2>/dev/null | wc -l)" = 2 ]; pruef $? "Herkunftsdateien angelegt"
grep -q 'G2-AD Kalender/Symbolthema 0.1' "$S/opt/protokoll.txt"; pruef $? "Journal-Eintrag aktivieren"
sitzung_unberuehrt aktivieren
schritt 0 "$NEU" "aktivieren noch einmal: bereits aktiv" bash "$I" aktivieren
schritt 2 "$NEU" "panel-neustart in der Scratch-Ablage verweigert" bash "$I" panel-neustart
schritt 2 "$NEU" "desktop-neustart in der Scratch-Ablage verweigert" bash "$I" desktop-neustart
for stelle in vorher panel desktop-vorher desktop; do
    schritt 1 "$NEU" "zurueck mit Fehler an Stelle $stelle: zurückgerollt" env QKA_PROBE=1 QKA_FEHLER=$stelle bash "$I" zurueck
done
schritt 0 "$AUS" "zurueck" bash "$I" zurueck
for l in panel desktop; do
    v=$(readlink "$S/opt/bin/querencia-$l.vorher")
    case $v in "$S"/opt/$l/*/querencia-$l) [ "$v" != "$P_ALT" ] && [ "$v" != "$D_ALT" ] && [ -e "$v" ] ;; *) false ;; esac
    pruef $? "$l .vorher zeigt auf die G2-AD-Ablage, die bleibt ($v)"
done
sitzung_unberuehrt zurueck
schritt 0 "$AUS" "zurueck noch einmal: bereits Ausgang" bash "$I" zurueck
schritt 2 "$AUS" "panel-neustart-zurueck in der Scratch-Ablage verweigert" bash "$I" panel-neustart-zurueck
schritt 2 "$AUS" "desktop-neustart-zurueck in der Scratch-Ablage verweigert" bash "$I" desktop-neustart-zurueck
schritt 0 "$NEU" "wieder aktivieren (Ablagen schon da)" bash "$I" aktivieren
# halber Stand (nur der Desktop zurück): beide Befehle bringen ihn in Ordnung
ln -sfn "$D_ALT" "$S/opt/bin/querencia-desktop"
[ "$(zustand)" = "P=neu D=alt" ]; pruef $? "halber Stand P=neu D=alt hergestellt"
schritt 0 "$NEU" "aktivieren vom halben Stand" bash "$I" aktivieren
ln -sfn "$D_ALT" "$S/opt/bin/querencia-desktop"
schritt 0 "$AUS" "zurueck vom halben Stand" bash "$I" zurueck
schritt 0 "$NEU" "aktivieren" bash "$I" aktivieren
exec 9>"$S/state/kalender-integrieren.lock"; flock -n 9
aus=$(bash "$I" zurueck 2>&1); rc=$?
[ "$rc" = 2 ] && echo "$aus" | grep -q 'anderer Lauf' && [ "$(zustand)" = "$NEU" ]
pruef $? "zweiter Lauf während der Sperre verweigert (Exit $rc)"
exec 9>&-
rm "$S/opt/bin/querencia-panel"; ln -s /nirgends/fremd "$S/opt/bin/querencia-panel"
schritt 2 "" "fremder Panel-Link: aktivieren verweigert" bash "$I" aktivieren
schritt 2 "" "fremder Panel-Link: zurueck verweigert" bash "$I" zurueck
[ "$(readlink "$S/opt/bin/querencia-panel")" = /nirgends/fremd ]; pruef $? "fremder Link unverändert"
rm "$S/opt/bin/querencia-panel"; ln -s "$P_ALT" "$S/opt/bin/querencia-panel"
rm "$S/opt/bin/querencia-desktop"; ln -s /nirgends/fremd "$S/opt/bin/querencia-desktop"
schritt 2 "" "fremder Desktop-Link: aktivieren verweigert" bash "$I" aktivieren
[ "$(readlink "$S/opt/bin/querencia-panel")" = "$P_ALT" ]; pruef $? "Panel-Link nach Verweigerung unverändert"
rm "$S/opt/bin/querencia-desktop"; ln -s "$D_ALT" "$S/opt/bin/querencia-desktop"
d=$(ls -d "$S"/opt/desktop/* | grep -v f48795676f867b82 | grep -v herkunft | head -1)
chmod u+w "$d" "$d/querencia-desktop"; echo x >>"$d/querencia-desktop"
schritt 2 "" "veränderte G2-AD-Desktop-Ablage: aktivieren verweigert" bash "$I" aktivieren
[ "$(readlink "$S/opt/bin/querencia-desktop")" = "$D_ALT" ]; pruef $? "Desktop-Link nach Verweigerung unverändert"
sitzung_unberuehrt Ende
echo "$ok/$n Probeschritte bestanden"
[ "$ok" = "$n" ]
