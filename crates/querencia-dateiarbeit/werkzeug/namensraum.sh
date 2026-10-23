#!/bin/sh
# Minimale Prüfhülle von querencia-dateiarbeit (bis der Prüfrahmen ihre
# Aufgabe übernimmt): führt BEFEHL in eigenem Benutzer- und
# Einhänge-Namensraum aus (unshare -Urm, ohne sudo; Recherche C §10.2).
# Darin, nur für diesen Prozessbaum sichtbar und mit ihm verschwunden:
#   BASIS/grenze           tmpfs 64 MiB  – anderes Dateisystem neben BASIS
#   BASIS/voll             tmpfs 1 MiB   – Ziel voll
#   BASIS/ohne-papierkorb  tmpfs 4 MiB mit einer Datei .Trash-0 – dort kann
#                          GLib keinen Papierkorb anlegen
#   /media                 tmpfs, darin QDTEST = rbind /boot (eingehängtes
#                          Laufwerk, nur lesbar)
#   LIBMOUNT_FSTAB=BASIS/fstab für GIO_USE_VOLUME_MONITOR=unix: QDTEST mit
#                          der Quelle von /boot (so hat der Mount ein
#                          Volume), QDFSTAB als tmpfs (nicht eingehängt,
#                          mount(8) hängt es im Namensraum ein). GIO übergeht
#                          bind-Zeilen der fstab (gemessen 23.10.).
# Aufruf: namensraum.sh BASIS BEFEHL [ARG…]
# Die Umgebung kommt vom Aufrufer (env -i bzw. env_clear); die Hülle setzt
# nur QD_NAMENSRAUM_INNEN und LIBMOUNT_FSTAB.
set -eu
if [ "${QD_NAMENSRAUM_INNEN:-}" != 1 ]; then
  export QD_NAMENSRAUM_INNEN=1
  exec unshare -Urm -- /bin/sh "$0" "$@"
fi
basis=$1
shift
mkdir -p "$basis/grenze" "$basis/voll" "$basis/ohne-papierkorb"
mount -t tmpfs -o size=64m tmpfs "$basis/grenze"
mount -t tmpfs -o size=1m tmpfs "$basis/voll"
mount -t tmpfs -o size=4m tmpfs "$basis/ohne-papierkorb"
: > "$basis/ohne-papierkorb/.Trash-0"
mount -t tmpfs tmpfs /media
mkdir /media/QDTEST /media/QDFSTAB
mount --rbind /boot /media/QDTEST
quelle=$(findmnt -n -o SOURCE /boot)
typ=$(findmnt -n -o FSTYPE /boot)
cat > "$basis/fstab" <<EOF
$quelle /media/QDTEST $typ noauto,user,x-gvfs-show,x-gvfs-name=QDTEST 0 0
tmpfs /media/QDFSTAB tmpfs noauto,user,x-gvfs-show,x-gvfs-name=QDFSTAB,size=1m 0 0
EOF
LIBMOUNT_FSTAB="$basis/fstab"
export LIBMOUNT_FSTAB
exec "$@"
