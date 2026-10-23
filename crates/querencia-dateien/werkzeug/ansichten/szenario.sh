#!/bin/bash
# Läuft IN der Hülle des Prüfrahmens (huelle.sh --labwc L): eigener Bus,
# eigenes XDG_RUNTIME_DIR /tmp/qd-*, eigenes HOME im Profil, headless labwc.
# Querencia Dateien, Ansichten; Abläufe siehe lauf.sh. Gerüst aus dem
# Prototyp dateien/prototyp-2 (werkzeug/szenario.sh).
set -u
. "$QD_PROFIL/env.sh"
mkdir -p "$OUT"
R=$OUT/ergebnis.txt; : >"$R"
ok() { echo "PASS $*" | tee -a "$R"; }
nok() { echo "FAIL $*" | tee -a "$R"; }
info() { echo "INFO $*" | tee -a "$R"; }
case $XDG_RUNTIME_DIR in /tmp/qd-*) ;; *) echo "nicht in der Hülle" >>"$R"; exit 2 ;; esac
[ -n "${QD_HUELLE:-}" ] && [ -e "$XDG_RUNTIME_DIR/.huelle/konfig" ] || { echo "keine Hülle" >>"$R"; exit 2; }
case ${DBUS_SESSION_BUS_ADDRESS:-} in unix:path=/tmp/qd-*) ;; *) echo "kein eigener Bus" >>"$R"; exit 2 ;; esac
[ "$HOME" = "$QD_PROFIL/heim" ] && ok "eigenes HOME im Profil" || { nok "HOME ist nicht das Profil"; exit 1; }
env | grep -q '^QUERENCIA_' && { nok "QUERENCIA_-Variable geerbt"; exit 1; }
[ -d /run/user ] && [ -n "$(ls -A /run/user 2>/dev/null)" ] && { nok "/run/user sichtbar"; exit 1; }
export QE_W=$BX QE_H=$BY
. "$W/eingabe.sh"
LOG=$OUT/dateien.log
foto() { q "wait ${2:-400}"; grim "$FOTOS/$1.png"; }
hat_besitzer() { gdbus call --timeout 10 --session --dest org.freedesktop.DBus --object-path /org/freedesktop/DBus --method org.freedesktop.DBus.NameHasOwner "$1" 2>&1 | grep -q true; }
warte_bis() { # Sekunden Befehl…
    local n=$(($1 * 10)); shift
    while [ $n -gt 0 ]; do "$@" && return 0; sleep 0.1; n=$((n - 1)); done
    return 1
}
# Logzeilen ab einer Marke: marke; …; warte_log 5 'Muster'
MARKE=0
marke() { MARKE=$(wc -l <"$LOG"); }
neu_im_log() { tail -n +$((MARKE + 1)) "$LOG" | grep -a -q -- "$1"; }
warte_log() { warte_bis "$1" neu_im_log "$2"; }
zeile() { tail -n +$((MARKE + 1)) "$LOG" | grep -a -- "$1" | tail -1; }
# letzte Fokuszeile: Name und Lage (Fensterkoordinaten = Bildschirm ohne Rahmen)
# Fokuszeilen: „Fokus <Ansicht>: Name | x y b h“ (GTK meldet einen neuen
# Fokus) und „Fokus nach Umordnen <Ansicht>: Name (Position)“
fokus_name() { grep -a -E ': Fokus (nach Umordnen )?(Symbol|Listen)ansicht: ' "$LOG" | tail -1 \
    | sed -E -e 's/.*ansicht: //' -e 's/ \| [0-9 -]+$//' -e 's/ \([0-9]+\)$//'; }
fokus_lage() { grep -a -E ': Fokus (Symbol|Listen)ansicht: ' "$LOG" | tail -1 | sed -E 's/.*\| ([0-9-]+) ([0-9-]+) ([0-9]+) ([0-9]+)$/\1 \2 \3 \4/'; }
fokus_mitte() { fokus_lage | awk '{ printf "%d %d", $1 + $3 / 2, $2 + $4 / 3 }'; }
# letzte Auswahlzeile der sichtbaren Ansicht: „<Zahl>: <bis zu sechs Namen>“
auswahl() { grep -a 'Auswahl [A-Za-z]*ansicht [0-9]*:' "$LOG" | tail -1 | sed -E 's/.*Auswahl [A-Za-z]+ //'; }
auswahl_zahl() { auswahl | cut -d: -f1; }
aktion() { # Name [Zeichenkettenwert]: Fensteraktion über org.gtk.Actions (wie das Menü)
    local p='[]'; [ $# -gt 1 ] && p="[<'$2'>]"
    gdbus call --timeout 10 --session --dest org.querencia.Dateien \
        --object-path /org/querencia/Dateien/window/1 \
        --method org.gtk.Actions.Activate "$1" "$p" '{}' >/dev/null 2>>"$OUT/gdbus.log"
}
pid_von() { for p in /proc/[0-9]*; do [ "$(readlink "$p/exe" 2>/dev/null)" = "$BIN" ] && echo "${p#/proc/}"; done | head -1; }
rss() { awk '/^VmRSS/ { print $2 }' "/proc/$1/status"; }

while read -r schema schluessel wert; do
    gsettings set "$schema" "$schluessel" "$wert" 2>>"$OUT/gsettings.log"
done <"$PW"
python3 -I "$PE" "$DUNKEL" "$PW" >"$OUT/portal.log" 2>&1 &
warte_bis 5 hat_besitzer org.freedesktop.portal.Desktop && ok "Portal-Ersatz" || nok "Portal-Ersatz fehlt"
"$QA" "${BX}x$BY" 1 2>>"$OUT/qausgabe.log"; sleep 1
info "querencia-dateien $(sha256sum "$BIN" | cut -c1-16), ${BX}x$BY, dunkel $DUNKEL, Ablauf $ABLAUF"

# dateien.ini des Profils (Einstellungen des Gerüsts, konfig.rs)
ini() { mkdir -p "$XDG_CONFIG_HOME/querencia"; printf '%s\n' "$@" >"$XDG_CONFIG_HOME/querencia/dateien.ini"; }
start() { # [Ordner] [Umgebung…]
    local ordner=${1:-$BEISPIEL}; shift || true
    START_NR=$((${START_NR:-0} + 1)); LOG=$OUT/dateien-$START_NR.log
    : >"$LOG"; MARKE=0
    # GTK_USE_PORTAL der Hülle (0) weg: GTK 4 liest sonst das Settings-Portal
    # (Farbschema, Schrift) nicht, auch nicht mit leerem Wert
    env -u GTK_USE_PORTAL "$@" PATH="$PBIN:$PATH" QUERENCIA_DATEIEN_DEBUG=1 \
        QUERENCIA_DATEIEN_LOCALEDIR="$QD_PROFIL/locale" \
        "$BIN" "$ordner" >"$LOG" 2>&1 8>&- &
    PID=$!
    warte_log 20 'vollständig gezeigt' && warte_log 5 ': Fokus [A-Za-z]*ansicht: ' \
        && ok "Start: $(zeile 'Objekte im Modell' | sed 's/.*Messung //')" \
        || nok "Start ohne Messzeile"
    hat_besitzer org.querencia.Dateien || nok "App-ID nicht auf dem Bus"
    # die erste Taste nach dem Anstecken der virtuellen Tastatur geht verloren
    q "keydown Shift_L" "keyup Shift_L" "wait 300"
}
stopp() { kill "$PID" 2>/dev/null; wait "$PID" 2>/dev/null; }

case $ABLAUF in
erkunden)
    start
    foto e01-start 800
    for k in Right Right Down Left Home; do
        q "key $k" "wait 300"
        info "nach $k: Fokus $(fokus_name) bei $(fokus_lage); Auswahl $(auswahl)"
    done
    foto e02-rechts
    q "keydown Control_L" "type 2" "keyup Control_L" "wait 600"
    q "key Down" "wait 300"
    info "Liste: Fokus $(fokus_name) bei $(fokus_lage)"
    foto e03-liste
    q "keydown Control_L" "type 1" "keyup Control_L" "wait 600"
    q "type beri" "key Escape" "key F2" "wait 600"
    foto e04-umbenennen
    q "key Escape" "wait 300"
    stopp
    ;;
*)
    . "$W/ablauf-$ABLAUF.sh"
    ;;
esac
[ -n "$(pid_von)" ] && { nok "Programm läuft noch"; kill "$(pid_von)"; }
info "Ende: $(grep -c '^PASS' "$R") PASS, $(grep -c '^FAIL' "$R") FAIL"
exit 0
