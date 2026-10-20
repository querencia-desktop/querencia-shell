#!/bin/bash
# Läuft IN der isolierten Sitzung (isoliert.sh). Startet Panel und Desktop
# wie der Autostart und misst, wann beide sichtbar arbeiten.
#   PORTAL=echt|fehlt|haengt  PANEL=<binary> DESKTOP=<binary> QA=<qausgabe>
set -u
L=$PROFIL/logs; Z=$L/zeiten.txt
seit() { echo "$(date +%s.%N) - $1" | bc | sed 's/^\./0./' | cut -c1-5; }
[ -n "${QA:-}" ] && "$QA" "$GROESSE" >"$L/qausgabe.log" 2>&1 && sleep 1
if [ -n "${INI_SCHRIFT:-}" ]; then
    # wie Prüfumgebungen nach PT-1: Schrift aus settings.ini, dconf leer
    # (INI_SCHRIFT mit _ statt Leerzeichen, z. B. DejaVu_Serif_9)
    mkdir -p "$XDG_CONFIG_HOME/gtk-4.0"
    printf '[Settings]\ngtk-font-name=%s\n' "${INI_SCHRIFT//_/ }" >"$XDG_CONFIG_HOME/gtk-4.0/settings.ini"
else
    gsettings set org.gnome.desktop.interface gtk-theme 'Querencia-Klassisch'
    gsettings set org.gnome.desktop.interface font-name 'DejaVu Sans 10'
    gsettings set org.gnome.desktop.interface cursor-theme 'DMZ-White'
    gsettings set org.gnome.desktop.interface cursor-size 24
    gsettings set org.gnome.desktop.interface icon-theme 'Querencia'
fi
if [ "$PORTAL" = haengt ]; then
    python3 -I "$HIER/portal-haenger.py" >"$L/haenger.log" 2>&1 &
    sleep 1
fi
t0=$(date +%s.%N)
[ -n "${PRUEF_GDK_DEBUG:-}" ] && export GDK_DEBUG=$PRUEF_GDK_DEBUG
"$DESKTOP" </dev/null >"$L/desktop.log" 2>&1 &
"$PANEL" </dev/null >"$L/panel.log" 2>&1 &
unset GDK_DEBUG
pt= dt=
for i in $(seq 1 $(( ${WARTEN:-60} * 10 ))); do
    [ -z "$pt" ] && grep -q 'Symbolthema' "$L/panel.log" && pt=$(seit "$t0")
    [ -z "$dt" ] && grep -q 'Symbole auf' "$L/desktop.log" && dt=$(seit "$t0")
    [ -n "$pt" ] && [ -n "$dt" ] && break
    sleep 0.1
done
echo "portal=$PORTAL panel=${pt:-nie} desktop=${dt:-nie}" >"$Z"
sleep 1.5
grim "$L/bild-$PORTAL.png"
# Änderung zur Laufzeit: Schrift
gsettings set org.gnome.desktop.interface font-name 'DejaVu Serif 12'
sleep 1.5
grim "$L/bild-$PORTAL-serif.png"
grep -h 'font-name' "$L/panel.log" "$L/desktop.log" >"$L/aenderung.txt"
tr '\0' '\n' </proc/$(pgrep -n -f "^$PANEL")/environ 2>/dev/null | grep '^GDK_DEBUG' >"$L/panel-env.txt"
echo fertig >>"$Z"
kill "$LABWC_PID"
