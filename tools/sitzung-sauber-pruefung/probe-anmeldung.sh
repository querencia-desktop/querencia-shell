#!/bin/bash
# G2-AI (QD-062): Abmelden und sofort wieder Anmelden, isoliert nachgestellt.
#
# Wie am 20.10.2026 lebt der „User-Manager“ über die Anmeldungen hinweg:
# hier ein eigener D-Bus-Daemon (Prüfbus) mit dem echten xdg-desktop-portal
# und -gtk, die er bei Bedarf startet. Jede Anmeldung ist ein Lauf der
# TESTKOPIE des Sitzungsstarters aus diesem Worktree (geändert sind genau
# zwei Zeilen: QUERENCIA_REPO → Testrepo, COMPONENTS → Prüfprogramme) mit
# headless labwc 0.9.8. Panel, Arbeitsfläche, Leerlaufdienst und
# Benachrichtigungen sind die neuen Debug-Bauten; der polkit-Agent ist ein
# Platzhalter (der echte meldet sich bei logind an). loginctl und systemctl
# sind Attrappen: systemctl wirkt nur auf die Portale des Prüfbusses.
# Nichts davon erreicht die laufende Sitzung: eigenes XDG_RUNTIME_DIR
# (qed-runtime-*), eigener Bus, eigene XDG_CONFIG/DATA/STATE_HOME.
#
#   bash probe-anmeldung.sh <panel> <desktop> <sitzung> <benachrichtigungen> <ausgabe>
#
# Fälle: V1 stehendes Portal, Abmelden über den Dialog, sofort neu anmelden;
# V2 dasselbe, aber das Portal lässt sich nicht stoppen (Wächter-Rückfall);
# V3 Reste einer früheren Sitzung (Panel, labwc) werden beendet;
# V4 ein isolierter labwc in anderem XDG_RUNTIME_DIR blockiert nicht;
# V5 fünf Log-Generationen.
set -uo pipefail
HIER=$(cd "$(dirname "$0")" && pwd)
WT=$(cd "$HIER/../.." && pwd)
P_PANEL=$(realpath "$1"); P_DESKTOP=$(realpath "$2"); P_SITZUNG=$(realpath "$3"); P_NOTIF=$(realpath "$4")
AUS=$(realpath -m "$5"); rm -rf "$AUS"; mkdir -p "$AUS"
LABWC=~/opt/labwc-0.9.8/bin/labwc
QE=${QE:-$HOME/projects/querencia-sitzung-wt-welle/werkzeug/bin/qeingabe}
T=$(mktemp -d /tmp/qed-runtime-XXXXXX); chmod 700 "$T"
RT=$T/rt; mkdir -m 0700 "$RT"
FAILS=0; N=0
ok() { N=$((N + 1)); echo "PASS $*" | tee -a "$AUS/ergebnis.txt"; }
nok() { N=$((N + 1)); FAILS=$((FAILS + 1)); echo "FAIL $*" | tee -a "$AUS/ergebnis.txt"; }
jetzt() { date +%s%N; }
ms_seit() { echo $(( ($(jetzt) - $1) / 1000000 )); }

# Laufende Portale der echten Sitzung: nachher müssen es dieselben sein.
live_portale() { for p in $(pgrep -u "$(id -u)" -f '^/usr/libexec/xdg-desktop-portal'); do
    tr '\0' '\n' 2>/dev/null <"/proc/$p/environ" | grep -qx 'XDG_RUNTIME_DIR=/run/user/1001' && echo "$p"; done | sort | tr '\n' ' '; }
LIVE_VORHER=$(live_portale)

# --- Testrepo, Prüfprogramme, Attrappen
TREPO=$T/repo; BIN=$T/bin; FAKE=$T/fakebin; X=$T/xdg
mkdir -p "$TREPO/session/querencia/labwc" "$BIN" "$FAKE" "$X"/{config,data/themes,data/icons,state}
cp "$WT"/session/querencia/labwc/{rc.xml,autostart,environment} "$TREPO/session/querencia/labwc/"
S=$TREPO/session/querencia-session
sed -e "s#^QUERENCIA_REPO=.*#QUERENCIA_REPO=$TREPO#" -e "s#^COMPONENTS=.*#COMPONENTS=$BIN#" \
    "$WT/session/querencia-session" >"$S"; chmod 0755 "$S"
diff "$WT/session/querencia-session" "$S" >"$AUS/testkopie.diff"
[ "$(grep -c '^>' "$AUS/testkopie.diff")" = 2 ] && ok "T0 Testkopie des Starters: genau 2 Zeilen geändert" || nok "T0 Testkopie: $(cat "$AUS/testkopie.diff")"
ln -s "$P_PANEL" "$BIN/querencia-panel"; ln -s "$P_DESKTOP" "$BIN/querencia-desktop"; ln -s "$P_SITZUNG" "$BIN/querencia-sitzung"
gcc -O1 -o "$BIN/querencia-polkit-agent" "$HOME/projects/querencia-sitzung-wt-g2ai/werkzeug/platzhalter.c" || nok "T0 Platzhalter"
for t in Querencia-Klassisch; do [ -d "$HOME/.local/share/themes/$t" ] && ln -s "$(realpath "$HOME/.local/share/themes/$t")" "$X/data/themes/$t"; done
for i in Querencia gnome-human; do [ -d "$HOME/.local/share/icons/$i" ] && ln -s "$(realpath "$HOME/.local/share/icons/$i")" "$X/data/icons/$i"; done
cat >"$FAKE/loginctl" <<'EOF'
#!/bin/sh
# Attrappe: Sitz seat0; Zustand „active“ nur für die eigene Sitzung.
case "$*" in
*"-p Seat"*) echo seat0 ;;
*"-p State"*) if [ "$2" = "$XDG_SESSION_ID" ]; then echo active; else echo closing; fi ;;
esac
EOF
cat >"$FAKE/systemctl" <<'EOF'
#!/bin/bash
# Attrappe des User-Managers: nur die beiden Portale des Prüfbusses.
echo "$(date +%T.%N | cut -c1-12) systemctl $*" >>"$PROBE_AUS/systemctl.log"
[ "${1-}" = --user ] || exit 1
shift; cmd=$1; shift
pids() {   # $1: Programm (pgrep -f auf den vollen Pfad; -x kennt nur 15 Zeichen)
    for p in $(pgrep -u "$(id -u)" -f "^$1( |\$)"); do
        [ "$(readlink "/proc/$p/exe" 2>/dev/null)" = "$1" ] || continue
        tr '\0' '\n' 2>/dev/null <"/proc/$p/environ" | grep -qx "DBUS_SESSION_BUS_ADDRESS=$DBUS_SESSION_BUS_ADDRESS" && echo "$p"
    done
}
prog() { case $1 in xdg-desktop-portal.service) echo /usr/libexec/xdg-desktop-portal ;; xdg-desktop-portal-gtk.service) echo /usr/libexec/xdg-desktop-portal-gtk ;; esac; }
case $cmd in
is-active) for u; do [ -n "$(pids "$(prog "$u")")" ] && echo active || echo inactive; done ;;
stop)
    [ -e "$PROBE_AUS/systemctl-taub" ] && exit 1
    for u; do for p in $(pids "$(prog "$u")"); do kill -CONT "$p"; kill -KILL "$p"; done; done ;;
*) exit 0 ;;
esac
EOF
chmod 0755 "$FAKE"/*

# --- Prüfbus (lebt über alle Anmeldungen, wie der User-Manager)
GRUND=(HOME="$HOME" USER="$USER" LOGNAME="$USER" PATH="$FAKE:/usr/bin:/bin" LANG=en_US.UTF-8 LANGUAGE=de
    XDG_RUNTIME_DIR="$RT" XDG_CONFIG_HOME="$X/config" XDG_DATA_HOME="$X/data" XDG_STATE_HOME="$X/state"
    XDG_DATA_DIRS=/usr/share PROBE_AUS="$AUS")
BUS=$(env -i "${GRUND[@]}" dbus-daemon --session --fork --print-address=1 --print-pid=3 3>"$T/bus.pid")
BUSPID=$(cat "$T/bus.pid")
GRUND+=(DBUS_SESSION_BUS_ADDRESS="$BUS")
env -i "${GRUND[@]}" gsettings set org.gnome.desktop.interface gtk-theme 'Querencia-Klassisch'
env -i "${GRUND[@]}" gsettings set org.gnome.desktop.interface font-name 'DejaVu Sans 10'
env -i "${GRUND[@]}" gsettings set org.gnome.desktop.interface icon-theme 'Querencia'

aufraeumen() {
    for p in /proc/[0-9]*; do
        tr '\0' '\n' 2>/dev/null <"$p/environ" | grep -qx "XDG_RUNTIME_DIR=$RT" && { kill -CONT "${p#/proc/}"; kill -KILL "${p#/proc/}"; } 2>/dev/null
    done
    kill "$BUSPID" 2>/dev/null; sleep 0.5
    grep -q " $RT/doc " /proc/self/mounts && fusermount3 -u -z "$RT/doc"
    rm -rf "$T"
}
trap aufraeumen EXIT
ST=$X/state/querencia
portal_pids() { for d in /proc/[0-9]*; do
    case $(readlink "$d/exe" 2>/dev/null) in /usr/libexec/xdg-desktop-portal | /usr/libexec/xdg-desktop-portal-gtk) ;; *) continue ;; esac
    tr '\0' '\n' 2>/dev/null <"$d/environ" | grep -qx "DBUS_SESSION_BUS_ADDRESS=$BUS" && echo "${d#/proc/}"
    done | tr '\n' ' '; }
# Prozesse der Prüfsitzung mit Programmnamen
sitzungsprozesse() { for p in /proc/[0-9]*; do
    tr '\0' '\n' 2>/dev/null <"$p/environ" | grep -qx "XDG_RUNTIME_DIR=$RT" || continue
    echo "${p#/proc/} $(basename "$(readlink "$p/exe")")"; done | grep -E 'querencia|labwc|polkit' | sort -n; }

# Anmeldung $1 (Sitzungs-ID): Starter im Hintergrund, misst Panel und Desktop.
LOGIN_PID=
anmelden() {
    local sid=$1 t0 pt= dt=
    t0=$(jetzt)
    env -i "${GRUND[@]}" XDG_SESSION_ID="$sid" WLR_BACKENDS=headless WLR_RENDERER=pixman \
        WLR_LIBINPUT_NO_DEVICES=1 QUERENCIA_BENACHRICHTIGUNGEN="$P_NOTIF" \
        sh "$S" >"$AUS/starter-$sid.out" 2>&1 &
    LOGIN_PID=$!
    # nur die Logs dieser Anmeldung (Kopfzeile mit der PID des Starters)
    neu() { head -1 "$ST/$1.log" 2>/dev/null | grep -q "querencia-session PID $LOGIN_PID\$"; }
    for _ in $(seq 1 200); do
        [ -z "$pt" ] && neu panel && grep -q 'Symbolthema' "$ST/panel.log" && pt=$(ms_seit "$t0")
        [ -z "$dt" ] && neu desktop && grep -q 'Symbole auf' "$ST/desktop.log" && dt=$(ms_seit "$t0")
        [ -n "$pt" ] && [ -n "$dt" ] && break
        kill -0 "$LOGIN_PID" 2>/dev/null || break
        sleep 0.05
    done
    PANEL_MS=${pt:-nie}; DESKTOP_MS=${dt:-nie}
    sleep 1   # Benachrichtigungen, Leerlauf
    cp "$ST/sitzung-start.log" "$AUS/sitzung-start-$sid.log" 2>/dev/null
    for l in panel desktop benachrichtigungen; do cp "$ST/$l.log" "$AUS/$l-$sid.log" 2>/dev/null; done
}
# labwc der laufenden Prüfsitzung
labwc_pid() { for p in $(pgrep -u "$(id -u)" -x labwc); do
    tr '\0' '\n' 2>/dev/null <"/proc/$p/environ" | grep -qx "XDG_RUNTIME_DIR=$RT" && echo "$p"; done | head -1; }
# Abmelden über den Dialog (echtes querencia-sitzung, kein Trockenlauf,
# Countdown 3 s); mit EINFRIEREN=1 werden die Portale eingefroren, sobald der
# Dialog steht (wie am 20.10.: das Portal blieb nach dem Abmelden stehen).
abmelden() {
    local sid=$1 l wd d
    l=$(labwc_pid); wd=$(tr '\0' '\n' <"/proc/$l/environ" | sed -n 's/^WAYLAND_DISPLAY=//p')
    [ -n "$wd" ] || wd=wayland-0
    # Ohne Eingabegerät hat der headless labwc keine Tastatur am Sitz und
    # kein Fenster wird aktiv; qeingabe steckt eine virtuelle an (wie in der
    # echten Sitzung die Tastatur).
    sleep 20 | env -i "${GRUND[@]}" WAYLAND_DISPLAY="$wd" "$QE" >"$AUS/qeingabe-$sid.log" 2>&1 &
    sleep 0.5
    env -i "${GRUND[@]}" WAYLAND_DISPLAY="$wd" LABWC_PID="$l" QUERENCIA_STATE_DIR="$ST" \
        "$P_SITZUNG" --abmelden --countdown 3 >"$AUS/abmelden-$sid.log" 2>&1 &
    d=$!
    sleep 1.5
    if [ "${EINFRIEREN:-}" = 1 ]; then
        EINGEFROREN=$(portal_pids)
        for p in $EINGEFROREN; do kill -STOP "$p"; done
    fi
    wait "$d"
    for _ in $(seq 1 100); do kill -0 "$LOGIN_PID" 2>/dev/null || break; sleep 0.1; done
    cp "$ST/sitzung-start.log" "$AUS/sitzung-start-$sid-ende.log" 2>/dev/null
}

# ------------------------------------------------------------------ V1
anmelden probe1
echo "probe1 panel=$PANEL_MS desktop=$DESKTOP_MS" >>"$AUS/zeiten.txt"
[ "$PANEL_MS" != nie ] && [ "$DESKTOP_MS" != nie ] && ok "V1a erste Anmeldung: Panel nach $PANEL_MS ms, Arbeitsfläche nach $DESKTOP_MS ms" || nok "V1a erste Anmeldung: Panel $PANEL_MS, Arbeitsfläche $DESKTOP_MS ($(tail -3 "$AUS/starter-probe1.out" | tr '\n' ';'))"
sitzungsprozesse >"$AUS/v1-prozesse-vorher.txt"
PORT=$(portal_pids); echo "Portale probe1: $PORT" >>"$AUS/zeiten.txt"
[ -n "$PORT" ] && ok "V1b Portale im Prüfbus aktiv ($PORT)" || nok "V1b keine Portale im Prüfbus"
EINFRIEREN=1 abmelden probe1
sitzungsprozesse >"$AUS/v1-prozesse-nachher.txt"
[ ! -s "$AUS/v1-prozesse-nachher.txt" ] && ok "V1d Abmelden: Prozessliste nachher leer (vorher: $(awk '{print $2}' "$AUS/v1-prozesse-vorher.txt" | sort | uniq -c | awk '{printf "%s×%s ",$1,$2}'))" || nok "V1d nach dem Abmelden noch: $(tr '\n' ';' <"$AUS/v1-prozesse-nachher.txt")"
grep -q 'Abmelden: SIGTERM an' "$AUS/abmelden-probe1.log" && grep -q 'Portale gestoppt (nach dem Ende' "$AUS/sitzung-start-probe1-ende.log" &&
    ok "V1e Dialog beendet die Sitzungsprogramme, Starter stoppt danach die eingefrorenen Portale ($EINGEFROREN)" || nok "V1e $(grep -h 'Abmelden\|Portale' "$AUS/abmelden-probe1.log" "$AUS/sitzung-start-probe1-ende.log" | tr '\n' ';')"
anmelden probe2
echo "probe2 panel=$PANEL_MS desktop=$DESKTOP_MS" >>"$AUS/zeiten.txt"
[ "$PANEL_MS" != nie ] && [ "$PANEL_MS" -le 5000 ] && [ "$DESKTOP_MS" != nie ] && [ "$DESKTOP_MS" -le 5000 ] &&
    ok "V1f sofort neu angemeldet: Panel nach $PANEL_MS ms, Arbeitsfläche nach $DESKTOP_MS ms (≤ 5 s); $(grep -h 'Settings-Portal' "$AUS/panel-probe2.log" | head -1)" ||
    nok "V1f Panel $PANEL_MS, Arbeitsfläche $DESKTOP_MS"
grep -q 'Settings-Portal antwortet (' "$AUS/panel-probe2.log" && ok "V1g frisches Portal: Panel nutzt es ($(grep -o 'antwortet ([0-9]* ms)' "$AUS/panel-probe2.log"))" || nok "V1g $(grep -h Portal "$AUS/panel-probe2.log" | tr '\n' ';')"
grep -q 'bereit' "$AUS/benachrichtigungen-probe2.log" && ok "V1h Benachrichtigungen bereit nach der Neuanmeldung" || nok "V1h $(tail -3 "$AUS/benachrichtigungen-probe2.log" | tr '\n' ';')"

# ------------------------------------------------------------------ V2
# Ab hier ist die Attrappe „taub“ (Datei systemctl-taub): stop scheitert,
# wie wenn der User-Manager das Portal nicht beenden kann.
touch "$AUS/systemctl-taub"
EINFRIEREN=1 abmelden probe2
timeout 3 env -i "${GRUND[@]}" busctl --user --timeout=1 call org.freedesktop.portal.Desktop /org/freedesktop/portal/desktop org.freedesktop.portal.Settings ReadOne ss org.gnome.desktop.interface gtk-theme >"$AUS/v2-portal-steht.txt" 2>&1
[ -n "$(portal_pids)" ] && grep -qi 'timed out\|Zeitüberschreitung' "$AUS/v2-portal-steht.txt" &&
    ok "V2a Portal steht vor der Anmeldung ($EINGEFROREN; Aufruf läuft in die Zeitüberschreitung)" || nok "V2a $(grep -h Portale "$AUS/sitzung-start-probe2-ende.log" | tr '\n' ';') / $(portal_pids)"
anmelden probe3
echo "probe3 panel=$PANEL_MS desktop=$DESKTOP_MS (Portal steht)" >>"$AUS/zeiten.txt"
[ "$PANEL_MS" != nie ] && [ "$PANEL_MS" -le 5000 ] && [ "$DESKTOP_MS" != nie ] && [ "$DESKTOP_MS" -le 5000 ] &&
    ok "V2b mit stehendem Portal: Panel nach $PANEL_MS ms, Arbeitsfläche nach $DESKTOP_MS ms (≤ 5 s)" || nok "V2b Panel $PANEL_MS, Arbeitsfläche $DESKTOP_MS"
grep -q 'GDK_DEBUG=no-portals' "$AUS/sitzung-start-probe3.log" && grep -q 'ohne Portal aus gsettings.*: Thema Querencia-Klassisch, Schrift DejaVu Sans 10' "$AUS/panel-probe3.log" &&
    ok "V2c Wächter-Rückfall: ohne Portal, Thema und Schrift aus gsettings ($(grep -h 'autostart' "$AUS/sitzung-start-probe3.log" | cut -d' ' -f3- | tr '\n' ';'))" ||
    nok "V2c $(grep -h 'autostart\|Portal' "$AUS/sitzung-start-probe3.log" "$AUS/panel-probe3.log" | tr '\n' ';')"
rm -f "$AUS/systemctl-taub"
for p in $(portal_pids); do kill -CONT "$p"; kill -KILL "$p"; done   # sonst wartet der Dialog selbst 25 s
abmelden probe3

# ------------------------------------------------------------------ V3/V4
# Reste: ein Panel-Platzhalter und ein labwc einer beendeten Sitzung
mkdir -p "$T/rest"; cp "$BIN/querencia-polkit-agent" "$T/rest/querencia-panel"
env -i "${GRUND[@]}" "$T/rest/querencia-panel" taub & REST1=$!
env -i "${GRUND[@]}" XDG_SESSION_ID=alt WLR_BACKENDS=headless WLR_RENDERER=pixman WLR_LIBINPUT_NO_DEVICES=1 \
    WAYLAND_DISPLAY= "$LABWC" -C "$TREPO/session/querencia/labwc" -s true >"$AUS/rest-labwc.log" 2>&1 & REST2=$!
# Köder: isolierter labwc in ANDEREM XDG_RUNTIME_DIR (wie Prüfläufe anderer Worker)
RT2=$T/rt2; mkdir -m 0700 "$RT2"
env -i HOME="$HOME" PATH=/usr/bin:/bin XDG_RUNTIME_DIR="$RT2" WLR_BACKENDS=headless WLR_RENDERER=pixman WLR_LIBINPUT_NO_DEVICES=1 \
    "$LABWC" -C "$TREPO/session/querencia/labwc" -s true >"$AUS/koeder-labwc.log" 2>&1 & KOED=$!
sleep 1.5
anmelden probe4
grep -q 'Reste einer früheren Sitzung' "$AUS/sitzung-start-probe4.log" && grep -q "labwc $REST2 gehört zu keiner laufenden Sitzung" "$AUS/sitzung-start-probe4.log" &&
    ! kill -0 $REST1 2>/dev/null && ! kill -0 $REST2 2>/dev/null &&
    ok "V3 Reste beendet statt Abbruch: Panel-Rest $REST1 (SIGTERM-taub → SIGKILL), labwc $REST2 der Sitzung „alt“" ||
    nok "V3 $(grep -h 'Rest\|beende\|SIGKILL\|labwc' "$AUS/sitzung-start-probe4.log" | tr '\n' ';')"
kill -0 $KOED 2>/dev/null && [ "$PANEL_MS" != nie ] && ok "V4 isolierter labwc in anderem XDG_RUNTIME_DIR blockiert nicht und bleibt (Panel nach $PANEL_MS ms)" || nok "V4 Köder $(kill -0 $KOED 2>/dev/null && echo lebt || echo weg), Panel $PANEL_MS"
kill $KOED 2>/dev/null
abmelden probe4

# ------------------------------------------------------------------ V5
for i in 1 2 3 4 5; do echo "alt-$i" >"$ST/panel.log.$i"; done
echo "aktuell" >"$ST/panel.log"
anmelden probe5
[ "$(head -1 "$ST/panel.log.1")" = aktuell ] && [ "$(cat "$ST/panel.log.2")" = alt-1 ] && [ "$(cat "$ST/panel.log.5")" = alt-4 ] && [ ! -e "$ST/panel.log.6" ] &&
    grep -q '^# Querencia-Sitzung ' "$ST/panel.log" &&
    ok "V5 Logs: fünf Generationen (.1 = vorige, .5 = älteste, keine .6), neue Datei mit Kopfzeile" ||
    nok "V5 $(for i in 1 2 5; do printf '%s=%s ' $i "$(head -1 "$ST/panel.log.$i")"; done)"
ls "$ST"/*.log* >"$AUS/v5-logs.txt"
abmelden probe5

[ "$(live_portale)" = "$LIVE_VORHER" ] && ok "L0 Portale der laufenden Sitzung unverändert ($LIVE_VORHER)" || nok "L0 Live-Portale vorher $LIVE_VORHER, nachher $(live_portale)"
cp "$AUS/systemctl.log" "$AUS/systemctl-aufrufe.txt" 2>/dev/null
echo "Ergebnis: $((N - FAILS)) PASS, $FAILS FAIL" | tee -a "$AUS/ergebnis.txt"
[ "$FAILS" = 0 ]
