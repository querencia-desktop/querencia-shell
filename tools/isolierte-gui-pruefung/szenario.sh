#!/bin/bash
# HMP-05: echtes Panel-Binary in der isolierten Sitzung mit echten
# (virtuellen) Zeiger- und Tastaturereignissen. Läuft nur in isoliert.py.
#   HMP_PANEL    Panel-Binary
#   HMP_SZENARIO Datei mit qeingabe-Befehlen (SHOT ersetzt durch Bildordner)
#   HMP_APPS     Verzeichnis mit Desktop-Dateien (optional)
OUT="$HMP04_PROFILE/logs"
exec >>"$OUT/szenario.txt" 2>&1
echo "=== $(date '+%F %T') Panel $HMP_PANEL ($(sha256sum "$HMP_PANEL" | cut -c1-16)), Szenario $HMP_SZENARIO"
if [ -n "${HMP_APPS:-}" ]; then
  mkdir -p "$XDG_DATA_HOME/applications"; cp "$HMP_APPS"/*.desktop "$XDG_DATA_HOME/applications/"
  update-desktop-database "$XDG_DATA_HOME/applications"
fi
[ -n "${HMP_WLDEBUG:-}" ] && export WAYLAND_DEBUG=client
QUERENCIA_MENU_DEBUG=1 "$HMP_PANEL" >"$OUT/panel.log" 2>&1 &
sleep 4
sed "s#SHOT#$OUT#g" "$HMP_SZENARIO" | "$HMP_QEINGABE"
echo "qeingabe exit $?"
sleep 1
echo "--- Panel-Log"; cat "$OUT/panel.log"
touch "$OUT/FERTIG"
sleep 30
