#!/bin/bash
# G2-M: läuft nur in isoliert.py (eigener headless labwc, D-Bus, XDG-Profil).
# Umgebung (von pruefen.sh):
#   G2M_PANEL              Binary; G2M_SZENARIO Eingabeskript für qeingabe
#   G2M_BIN                qeingabe, qausgabe; G2M_WERKZEUG dieser Ordner
#   G2M_AUFLOESUNG         z. B. 1024x768 (leer: 1280x720 des headless-Ausgangs)
#   G2M_NETZ               echt:     echter Systembus, nur lesend (kein
#                                    NetworkManager auf der VM: Rückfall mit eth0)
#                          attrappe: eigener Systembus im Laufzeitordner mit
#                                    der NetworkManager-Attrappe qnm.py
#                          spaet:    eigener Systembus, Attrappe startet erst im Szenario
#   G2M_PANELINI           Datei, die als panel.ini kopiert wird (optional)
#   G2M_APPS               Desktop-Dateien der Querencia-Programme (Menü)
OUT="$HMP04_PROFILE/logs"
exec >>"$OUT/szenario.txt" 2>&1
case "$XDG_RUNTIME_DIR" in */querencia-runtime-wt-netzwerk/nachweis/gui/rt/g2m-*) ;; *) echo "nicht isoliert"; exit 2 ;; esac
echo "=== $(date '+%F %T') Panel $G2M_PANEL ($(sha256sum "$G2M_PANEL" | cut -c1-16)), Szenario $G2M_SZENARIO, Netz ${G2M_NETZ:-echt}, Auflösung ${G2M_AUFLOESUNG:-1280x720}"
mkdir -p "$XDG_DATA_HOME/applications" "$XDG_CONFIG_HOME/querencia"
[ -n "${G2M_APPS:-}" ] && cp "$G2M_APPS"/*.desktop "$XDG_DATA_HOME/applications/"
if [ -n "${G2M_PANELINI:-}" ]; then
  cp "$G2M_PANELINI" "$XDG_CONFIG_HOME/querencia/panel.ini"; chmod u+w "$XDG_CONFIG_HOME/querencia/panel.ini"
  cp "$G2M_PANELINI" "$OUT/panel.ini.anfang"
fi
W=1280; H=720
if [ -n "${G2M_AUFLOESUNG:-}" ]; then
  "$G2M_BIN/qausgabe" "$G2M_AUFLOESUNG" && W=${G2M_AUFLOESUNG%x*} && H=${G2M_AUFLOESUNG#*x}
  sleep 1
fi
export QE_W=$W QE_H=$H
wayland-info 2>/dev/null | grep -A3 'wl_output' | head -8 >"$OUT/ausgabe.txt"
QNMCTL="python3 -I $G2M_WERKZEUG/qnmctl.py"
QNM="python3 -I $G2M_WERKZEUG/qnm.py"
case "${G2M_NETZ:-echt}" in
attrappe|spaet)
  export DBUS_SYSTEM_BUS_ADDRESS="unix:path=$XDG_RUNTIME_DIR/systembus"
  dbus-daemon --session --nofork --nopidfile --address="$DBUS_SYSTEM_BUS_ADDRESS" >"$OUT/systembus.log" 2>&1 &
  sleep 0.7
  [ "$G2M_NETZ" = attrappe ] && { $QNM "$OUT/qnm.log" & sleep 1.5; }
  ;;
esac
# Teil 5/6: Vorbereitung im Profil vor dem Panelstart (optional)
[ -n "${G2M_VORHER:-}" ] && bash "$G2M_VORHER"
# Einstellungen mit Netzwerkseite (G2-O) erst, wenn das Szenario sie anlegt
# („run EINST“): Desktop-Eintrag mit Aktion netzwerk, Exec protokolliert nur.
EINST="bash $G2M_WERKZEUG/einstellungen-attrappe.sh $XDG_DATA_HOME/applications $OUT/einstellungen-start.txt"
# Panel-Log mit Uhrzeit je Zeile (der Auswerter ordnet sie den Marken zu);
# die Menüzeilen schreibt das Panel mit QUERENCIA_NETZWERK_DEBUG=1.
QUERENCIA_NETZWERK_DEBUG=1 "$G2M_PANEL" > >(python3 -I -u "$G2M_WERKZEUG/zeitstempel.py" >"$OUT/panel.log") 2>&1 &
PANEL_PID=$!
sleep 4
sed -e "s#SHOT#$OUT#g" -e "s#QNMCTL#$QNMCTL#g" -e "s#QNM#$QNM#g" -e "s#WERKZEUG#$G2M_WERKZEUG#g" \
    -e "s#EINST#$EINST#g" "$G2M_SZENARIO" | "$G2M_BIN/qeingabe"
echo "qeingabe exit $?"
sleep 1
if [ "${G2M_NEUSTART:-}" = 1 ]; then
  # Teil 5: Panel neu starten, die Anordnung muss aus panel.ini wiederkommen
  cp "$XDG_CONFIG_HOME/querencia/panel.ini" "$OUT/panel.ini.vor-neustart" 2>/dev/null
  grim "$OUT/vor-neustart.png"
  kill "$PANEL_PID" 2>/dev/null; wait "$PANEL_PID" 2>/dev/null
  echo "MARKE $(date +%T.%3N) NEUSTART Panel neu gestartet"
  QUERENCIA_NETZWERK_DEBUG=1 "$G2M_PANEL" > >(python3 -I -u "$G2M_WERKZEUG/zeitstempel.py" >>"$OUT/panel.log") 2>&1 &
  PANEL_PID=$!
  sleep 5
  grim "$OUT/nach-neustart.png"
fi
[ -S "$XDG_RUNTIME_DIR/systembus" ] && $QNMCTL zustand >"$OUT/qnm-ende.json" 2>/dev/null
cp "$XDG_CONFIG_HOME/querencia/panel.ini" "$OUT/panel.ini.ende" 2>/dev/null
kill "$PANEL_PID" 2>/dev/null; wait "$PANEL_PID" 2>/dev/null
[ -S "$XDG_RUNTIME_DIR/systembus" ] && $QNMCTL beenden >/dev/null 2>&1
echo "--- Panel-Log"; cat "$OUT/panel.log"
touch "$OUT/FERTIG"
sleep 30
