#!/bin/bash
# Isolierter headless labwc mit eigenem D-Bus, eigenem XDG-Profil und
# eigenem dconf für die Prüfungen von G2-AI (QD-062). Die laufende Sitzung
# bleibt unberührt: eigenes XDG_RUNTIME_DIR (qed-runtime-*), eigener Bus,
# Profil unter dem übergebenen Ordner; GTK-Thema und Symbole der Sitzung nur
# gelesen (Verweise).
#   isoliert.sh <profil> <szenario.sh> [zeitlimit] [ausgaenge]
# Das Szenario läuft IN der Sitzung (labwc -s) und beendet sie selbst mit
# „kill $LABWC_PID“ oder durch Abmelden.
set -u
PROFIL=$(realpath -m "$1"); SZ=$(realpath "$2"); LIMIT=${3:-120}; AUSG=${4:-1}
LABWC=${QUERENCIA_LABWC:-$HOME/opt/labwc-0.9.8/bin/labwc}
case $PROFIL in "$HOME"/.config* | "$HOME"/.local* | "$HOME"/opt*) echo "Profil an verbotenem Ort: $PROFIL"; exit 2 ;; esac
rm -rf "$PROFIL"; mkdir -p "$PROFIL"/{config/labwc,data/themes,data/icons,data/dbus-1/services,state,cache,logs}
cat >"$PROFIL/config/labwc/rc.xml" <<X
<?xml version="1.0"?>
<labwc_config><core><decoration>server</decoration></core>
<theme><name>Querencia-Klassisch</name></theme></labwc_config>
X
for t in Querencia-Klassisch; do
    [ -d "$HOME/.local/share/themes/$t" ] && ln -s "$(realpath "$HOME/.local/share/themes/$t")" "$PROFIL/data/themes/$t"
done
for i in Querencia gnome-human; do
    [ -d "$HOME/.local/share/icons/$i" ] && ln -s "$(realpath "$HOME/.local/share/icons/$i")" "$PROFIL/data/icons/$i"
done
# PORTAL=echt: der echte xdg-desktop-portal wird im Wegwerfbus aktiviert.
# Er braucht das Dokument-Portal (1.21 startet sonst keine Portale); dessen
# FUSE-Mount liegt unter dem eigenen Laufzeitordner und wird beim Aufräumen
# ausgehängt (PT-1). Sonst sind beide Dienste hier nicht aktivierbar.
if [ "${PORTAL:-echt}" != echt ]; then
    for name in org.freedesktop.portal.Desktop org.freedesktop.portal.Documents; do
        printf '[D-BUS Service]\nName=%s\nExec=/bin/false\n' "$name" \
            >"$PROFIL/data/dbus-1/services/$name.service"
    done
fi
# PORTAL_CONF=<datei>: Portal-Konfiguration ins Profil (wie ~/.config/xdg-desktop-portal/)
if [ -n "${PORTAL_CONF:-}" ]; then
    mkdir -p "$PROFIL/config/xdg-desktop-portal"
    cp "$PORTAL_CONF" "$PROFIL/config/xdg-desktop-portal/"
fi
RT=$(mktemp -d /tmp/qed-runtime-XXXXXX); chmod 700 "$RT"
cleanup() {
    for p in /proc/[0-9]*; do
        tr "\0" "\n" 2>/dev/null <"$p/environ" | grep -qx "XDG_RUNTIME_DIR=$RT" && kill -TERM "${p#/proc/}" 2>/dev/null
    done
    sleep 1
    grep -q " $RT/doc " /proc/self/mounts && fusermount3 -u -z "$RT/doc"
    rm -rf "$RT"
}
trap cleanup EXIT
start=$(date +%s.%N)
env -i HOME="$HOME" USER="$USER" PATH=/usr/bin:/bin LANG=en_US.UTF-8 LANGUAGE=de \
    XDG_RUNTIME_DIR="$RT" XDG_CONFIG_HOME="$PROFIL/config" XDG_DATA_HOME="$PROFIL/data" \
    XDG_STATE_HOME="$PROFIL/state" XDG_CACHE_HOME="$PROFIL/cache" XDG_DATA_DIRS=/usr/share \
    XDG_CURRENT_DESKTOP=X-Querencia XDG_SESSION_TYPE=wayland \
    WLR_BACKENDS=headless WLR_RENDERER=pixman WLR_HEADLESS_OUTPUTS="$AUSG" WLR_LIBINPUT_NO_DEVICES=1 \
    XKB_DEFAULT_LAYOUT=de PROFIL="$PROFIL" HIER="$(dirname "$(realpath "$0")")" \
    GROESSE="${GROESSE:-1920x1080}" PRUEF_START="$start" ${PRUEF_ENV:-} \
    timeout "$LIMIT" dbus-run-session -- "$LABWC" -C "$PROFIL/config/labwc" -s "/bin/bash $SZ" \
    >"$PROFIL/logs/labwc.log" 2>&1
echo "isoliert: Status $? nach $(echo "$(date +%s.%N) - $start" | bc | cut -c1-5) s; Profil $PROFIL"
