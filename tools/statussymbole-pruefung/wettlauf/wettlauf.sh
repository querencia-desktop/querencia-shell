#!/bin/bash
# G2-Y: läuft in isoliert.py; Parameter über WL_ALT, WL_NEU, WL_N, WL_ART.
OUT="$HMP04_PROFILE/logs"
exec >>"$OUT/szenario.txt" 2>&1
case "$XDG_RUNTIME_DIR" in */querencia-runtime-wt-statussymbole/nachweis/gui/rt/g2m-*) ;; *) echo "nicht isoliert"; exit 2 ;; esac
mkdir -p "$XDG_CONFIG_HOME/querencia"
cp "$WL_PANELINI" "$XDG_CONFIG_HOME/querencia/panel.ini"; chmod u+w "$XDG_CONFIG_HOME/querencia/panel.ini"
python3 -I "$WL_WERKZEUG/wettlauf.py" "$WL_ALT" "$WL_NEU" "$WL_N" "$WL_ART" "$OUT"
echo "wettlauf exit $?"
touch "$OUT/FERTIG"
sleep 30
