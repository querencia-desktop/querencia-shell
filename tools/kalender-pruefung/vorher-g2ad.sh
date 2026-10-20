#!/bin/bash
# G2-AD: Vorbereitung im isolierten Profil vor dem Panelstart (G2M_VORHER der
# umgelenkten G2-M-Werkzeuge; läuft in szenario-g2m.sh, also nur isoliert).
#   G2AD_THEMA_START  Symbolthema, das vor dem Start als Benutzerwert gesetzt
#                     wird (gsettings im dconf des Profils), sonst keins
#   G2AD_DESKTOP      Desktop-Binary, das neben dem Panel startet (optional)
# Eigener Schreibtischordner im Profil, damit der Desktop nicht ~/Schreibtisch
# zeigt; Desktop-Log mit Uhrzeit je Zeile.
set -u
case "$XDG_RUNTIME_DIR" in */querencia-runtime-wt-g2ai/nachweis/gui/rt/g2m-*) ;; *) echo "nicht isoliert"; exit 2 ;; esac
OUT="$HMP04_PROFILE/logs"
mkdir -p "$HMP04_PROFILE/schreibtisch"
printf 'XDG_DESKTOP_DIR="%s"\n' "$HMP04_PROFILE/schreibtisch" >"$XDG_CONFIG_HOME/user-dirs.dirs"
if [ -n "${G2AD_THEMA_START:-}" ]; then
  gsettings set org.gnome.desktop.interface icon-theme "$G2AD_THEMA_START"
fi
echo "icon-theme vor dem Start: $(gsettings get org.gnome.desktop.interface icon-theme)" >"$OUT/thema-start.txt"
if [ -n "${G2AD_DESKTOP:-}" ]; then
  "$G2AD_DESKTOP" > >(python3 -I -u "$G2M_WERKZEUG/zeitstempel.py" >"$OUT/desktop.log") 2>&1 &
  sleep 3
fi
