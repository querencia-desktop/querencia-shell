#!/bin/bash
# G2-J: läuft nur in isoliert.py (eigener headless labwc, D-Bus, XDG-Profil).
# Umgebung (von pruefen.sh):
#   G2J_PANEL, G2J_MIXER   Binaries; G2J_SZENARIO Eingabeskript für qeingabe
#   G2J_BIN                qeingabe, qausgabe; G2J_WERKZEUG dieser Ordner
#   G2J_AUFLOESUNG         z. B. 1024x768 (leer: 1280x720 des headless-Ausgangs)
#   G2J_AUDIO              qpulse (Testserver mit einem Gerät), leer (qpulse
#                          ohne Geräte), keiner (kein Audiodienst)
#   G2J_PANELINI           Datei, die als panel.ini kopiert wird (optional)
#   G2J_APPS               Desktop-Dateien der Querencia-Programme (Menü)
OUT="$HMP04_PROFILE/logs"
exec >>"$OUT/szenario.txt" 2>&1
case "$XDG_RUNTIME_DIR" in */querencia-runtime-wt-lautstaerke/nachweis/gui/rt/g2j-*) ;; *) echo "nicht isoliert"; exit 2 ;; esac
echo "=== $(date '+%F %T') Panel $G2J_PANEL ($(sha256sum "$G2J_PANEL" | cut -c1-16)), Mixer $G2J_MIXER ($(sha256sum "$G2J_MIXER" | cut -c1-16)), Szenario $G2J_SZENARIO, Audio ${G2J_AUDIO:-keiner}, Auflösung ${G2J_AUFLOESUNG:-1280x720}"
# Menüeinträge: Querencia-Programme und der Lautstärkeregler (Exec auf das Prüf-Binary)
mkdir -p "$XDG_DATA_HOME/applications" "$XDG_DATA_HOME/icons/hicolor/scalable/apps" "$XDG_CONFIG_HOME/querencia"
[ -n "${G2J_APPS:-}" ] && cp "$G2J_APPS"/*.desktop "$XDG_DATA_HOME/applications/"
sed -e "s#^Exec=.*#Exec=$G2J_MIXER#" -e "s#^TryExec=.*#TryExec=$G2J_MIXER#" \
    "$G2J_DATA/org.querencia.Lautstaerke.desktop" >"$XDG_DATA_HOME/applications/org.querencia.Lautstaerke.desktop"
cp "$G2J_DATA/symbole/org.querencia.Lautstaerke.svg" "$XDG_DATA_HOME/icons/hicolor/scalable/apps/"
if [ -n "${G2J_PANELINI:-}" ]; then
  cp "$G2J_PANELINI" "$XDG_CONFIG_HOME/querencia/panel.ini"; chmod u+w "$XDG_CONFIG_HOME/querencia/panel.ini"
  cp "$G2J_PANELINI" "$OUT/panel.ini.anfang"
fi
W=1280; H=720
if [ -n "${G2J_AUFLOESUNG:-}" ]; then
  "$G2J_BIN/qausgabe" "$G2J_AUFLOESUNG" && W=${G2J_AUFLOESUNG%x*} && H=${G2J_AUFLOESUNG#*x}
  sleep 1
fi
export QE_W=$W QE_H=$H
wayland-info 2>/dev/null | grep -A3 'wl_output' | head -8 >"$OUT/ausgabe.txt"
export G2J_QPACTL="python3 -I $G2J_WERKZEUG/qpactl.py"
case "${G2J_AUDIO:-keiner}" in
qpulse) python3 -I "$G2J_WERKZEUG/qpulse.py" "$OUT/qpulse.log" & sleep 1 ;;
leer) python3 -I "$G2J_WERKZEUG/qpulse.py" "$OUT/qpulse.log" --leer & sleep 1 ;;
esac
"$G2J_PANEL" >"$OUT/panel.log" 2>&1 &
PANEL_PID=$!
sleep 4
sed -e "s#SHOT#$OUT#g" -e "s#QPACTL#$G2J_QPACTL#g" -e "s#WERKZEUG#$G2J_WERKZEUG#g" "$G2J_SZENARIO" | "$G2J_BIN/qeingabe"
echo "qeingabe exit $?"
sleep 1
[ -S "$XDG_RUNTIME_DIR/qpulse.ctl" ] && $G2J_QPACTL zustand >"$OUT/qpulse-ende.json"
cp "$XDG_CONFIG_HOME/querencia/panel.ini" "$OUT/panel.ini.ende" 2>/dev/null
kill "$PANEL_PID" 2>/dev/null; wait "$PANEL_PID" 2>/dev/null
[ -S "$XDG_RUNTIME_DIR/qpulse.ctl" ] && $G2J_QPACTL beenden >/dev/null
echo "--- Panel-Log"; cat "$OUT/panel.log"
touch "$OUT/FERTIG"
sleep 30
