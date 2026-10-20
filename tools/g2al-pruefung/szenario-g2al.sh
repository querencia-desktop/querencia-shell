#!/bin/bash
# G2-AL: läuft nur in isoliert.py (eigener headless labwc, D-Bus, XDG-Profil).
# Umgebung (von pruefen-g2al.sh):
#   G2AL_PANEL, G2AL_SZENARIO (qeingabe), G2AL_BIN, G2AL_WERKZEUG,
#   G2M_WERKZEUG, G2AL_TESTFENSTER, G2AL_APPS
#   G2AL_AUFLOESUNG  z. B. 1024x768 (leer: 1280x720 des headless-Ausgangs)
#   G2AL_PANELINI    Datei, die als panel.ini kopiert wird (leer: keine
#                    panel.ini, Standardlayout)
#   G2AL_DIENST      Querencia Benachrichtigungen (Binary, nur gelesen), das
#                    das Szenario mit „run DIENST start“ startet (dienst.sh);
#                    läuft im isolierten Bus mit dem XDG-Profil, Verlauf im
#                    Profil
#   G2AL_NEU         neues Panel-Binary für „NEU --panel-ini …“ (Ergänzung
#                    unter einem laufenden alten Panel)
# Im Szenario ersetzt: SHOT (Logordner), WERKZEUG, FENSTER (testfenster.py),
# DIENST, NEU, INI (panel.ini).
OUT="$HMP04_PROFILE/logs"
exec >>"$OUT/szenario.txt" 2>&1
case "$XDG_RUNTIME_DIR" in */querencia-runtime-wt-g2al/nachweis/gui/rt/g2m-*) ;; *) echo "nicht isoliert"; exit 2 ;; esac
case "${DBUS_SESSION_BUS_ADDRESS:-}" in unix:path=/run/user/*|"") echo "kein eigener Sitzungsbus"; exit 2 ;; esac
echo "=== $(date '+%F %T') Panel $G2AL_PANEL ($(sha256sum "$G2AL_PANEL" | cut -c1-16)), Szenario $G2AL_SZENARIO, Auflösung ${G2AL_AUFLOESUNG:-1280x720}, Dienst ${G2AL_DIENST:-–}"
mkdir -p "$XDG_DATA_HOME/applications" "$XDG_CONFIG_HOME/querencia"
cp "$G2AL_APPS"/*.desktop "$XDG_DATA_HOME/applications/"
INI=$XDG_CONFIG_HOME/querencia/panel.ini
if [ -n "${G2AL_PANELINI:-}" ]; then
  cp "$G2AL_PANELINI" "$INI"; chmod 600 "$INI"; cp "$INI" "$OUT/panel.ini.anfang"
fi
W=1280; H=720
if [ -n "${G2AL_AUFLOESUNG:-}" ]; then
  "$G2AL_BIN/qausgabe" "$G2AL_AUFLOESUNG" && W=${G2AL_AUFLOESUNG%x*} && H=${G2AL_AUFLOESUNG#*x}
  sleep 1
fi
export QE_W=$W QE_H=$H
{
  echo "DISPLAY=${DISPLAY:-} WAYLAND_DISPLAY=$WAYLAND_DISPLAY"
  echo "DBUS_SESSION_BUS_ADDRESS=$DBUS_SESSION_BUS_ADDRESS"
  echo "XDG_STATE_HOME=$XDG_STATE_HOME XDG_CONFIG_HOME=$XDG_CONFIG_HOME"
} >"$OUT/umgebung.txt"
# kein Sitzungsprogramm (G2-AL braucht keins): nie das echte
export QUERENCIA_SITZUNG="$HMP04_PROFILE/fehlt/querencia-sitzung"
DIENST="bash $G2AL_WERKZEUG/dienst.sh"
export G2AL_DIENST
NEU=${G2AL_NEU:-/bin/false}
"$G2AL_PANEL" > >(python3 -I -u "$G2M_WERKZEUG/zeitstempel.py" >"$OUT/panel.log") 2>&1 &
PANEL_PID=$!
sleep 4
sed -e "s#SHOT#$OUT#g" -e "s#WERKZEUG#$G2AL_WERKZEUG#g" -e "s#FENSTER#$G2AL_TESTFENSTER#g" \
    -e "s#DIENST#$DIENST#g" -e "s#NEU#$NEU#g" -e "s#INI#$INI#g" "$G2AL_SZENARIO" | "$G2AL_BIN/qeingabe"
echo "qeingabe exit $?"
sleep 1
cp "$INI" "$OUT/panel.ini.ende" 2>/dev/null
kill "$PANEL_PID" 2>/dev/null; wait "$PANEL_PID" 2>/dev/null
bash "$G2AL_WERKZEUG/dienst.sh" stop 2>/dev/null
echo "--- Panel-Log"; cat "$OUT/panel.log"
touch "$OUT/FERTIG"
sleep 30
