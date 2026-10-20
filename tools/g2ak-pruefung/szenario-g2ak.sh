#!/bin/bash
# G2-AK: läuft nur in isoliert.py (eigener headless labwc, D-Bus, XDG-Profil).
# Umgebung (von pruefen-g2ak.sh):
#   G2AK_PANEL, G2AK_SZENARIO (qeingabe), G2AK_BIN, G2AK_WERKZEUG, G2M_WERKZEUG
#   G2AK_AUFLOESUNG   z. B. 1024x768 (leer: 1280x720 des headless-Ausgangs)
#   G2AK_PANELINI     Datei, die als panel.ini kopiert wird (optional)
#   G2AK_SITZUNG      pfad:     Attrappe von querencia-sitzung vorn in PATH
#                     variable: dazu QUERENCIA_SITZUNG auf eine zweite Attrappe
#                     keins:    QUERENCIA_SITZUNG auf eine fehlende Datei, also
#                               kein Sitzungsprogramm (Rückfall des Panels)
#                     Nie das echte Programm: QUERENCIA_SITZUNG bzw. die
#                     Attrappe vorn in PATH gewinnen vor ~/opt/querencia/bin.
#   G2AK_VORHER       Shell-Befehl vor dem Panelstart (optional)
# Im Szenario ersetzt: SHOT (Logordner), WERKZEUG, BIN, PROFIL, INI (panel.ini).
OUT="$HMP04_PROFILE/logs"
exec >>"$OUT/szenario.txt" 2>&1
case "$XDG_RUNTIME_DIR" in */querencia-runtime-wt-g2ak/nachweis/gui/rt/g2m-*) ;; *) echo "nicht isoliert"; exit 2 ;; esac
echo "=== $(date '+%F %T') Panel $G2AK_PANEL ($(sha256sum "$G2AK_PANEL" | cut -c1-16)), Szenario $G2AK_SZENARIO, Sitzung ${G2AK_SITZUNG:-pfad}, Auflösung ${G2AK_AUFLOESUNG:-1280x720}"
mkdir -p "$XDG_DATA_HOME/applications" "$XDG_CONFIG_HOME/querencia"
cp "$G2AK_APPS"/*.desktop "$XDG_DATA_HOME/applications/"
INI=$XDG_CONFIG_HOME/querencia/panel.ini
if [ -n "${G2AK_PANELINI:-}" ]; then
  cp "$G2AK_PANELINI" "$INI"; chmod u+w "$INI"; cp "$INI" "$OUT/panel.ini.anfang"
fi
W=1280; H=720
if [ -n "${G2AK_AUFLOESUNG:-}" ]; then
  "$G2AK_BIN/qausgabe" "$G2AK_AUFLOESUNG" && W=${G2AK_AUFLOESUNG%x*} && H=${G2AK_AUFLOESUNG#*x}
  sleep 1
fi
export QE_W=$W QE_H=$H
echo "DISPLAY=${DISPLAY:-} WAYLAND_DISPLAY=$WAYLAND_DISPLAY LABWC_PID=${LABWC_PID:-}" >"$OUT/umgebung.txt"
# Sitzungsprogramm: nur Attrappen (siehe oben)
A=$HMP04_PROFILE/attrappe
mkdir -p "$A/pfad" "$A/variable"
cp "$G2AK_WERKZEUG/sitzung-attrappe.sh" "$A/pfad/querencia-sitzung"
cp "$G2AK_WERKZEUG/sitzung-attrappe.sh" "$A/variable/querencia-sitzung"
chmod 755 "$A/pfad/querencia-sitzung" "$A/variable/querencia-sitzung"
export G2AK_AUFRUFE=$OUT/sitzung-aufrufe.txt
: >"$G2AK_AUFRUFE"
export PATH="$A/pfad:$PATH"
case "${G2AK_SITZUNG:-pfad}" in
pfad) unset QUERENCIA_SITZUNG ;;
variable) export QUERENCIA_SITZUNG="$A/variable/querencia-sitzung" ;;
keins) export QUERENCIA_SITZUNG="$A/fehlt/querencia-sitzung" ;;
*) echo "G2AK_SITZUNG unbekannt"; exit 2 ;;
esac
echo "PATH=$PATH QUERENCIA_SITZUNG=${QUERENCIA_SITZUNG:-}" >>"$OUT/umgebung.txt"
[ -n "${G2AK_VORHER:-}" ] && bash -c "$G2AK_VORHER"
"$G2AK_PANEL" > >(python3 -I -u "$G2M_WERKZEUG/zeitstempel.py" >"$OUT/panel.log") 2>&1 &
PANEL_PID=$!
sleep 4
sed -e "s#SHOT#$OUT#g" -e "s#WERKZEUG#$G2AK_WERKZEUG#g" -e "s#BIN#$G2AK_BIN#g" \
    -e "s#PROFIL#$HMP04_PROFILE#g" -e "s#INI#$INI#g" "$G2AK_SZENARIO" | "$G2AK_BIN/qeingabe"
echo "qeingabe exit $?"
sleep 1
cp "$INI" "$OUT/panel.ini.ende" 2>/dev/null
kill "$PANEL_PID" 2>/dev/null; wait "$PANEL_PID" 2>/dev/null
echo "--- Panel-Log"; cat "$OUT/panel.log"
touch "$OUT/FERTIG"
sleep 30
