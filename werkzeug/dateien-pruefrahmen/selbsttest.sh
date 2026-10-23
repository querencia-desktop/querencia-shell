#!/bin/bash
# Prüfrahmen Querencia Dateien: Selbsttest von huelle.sh, attrappen.sh und reste.py.
#
#   selbsttest.sh [PROTOKOLL]
#
# Jeder Fall läuft in einer frischen Hülle. Ausgabe: eine Zeile je Fall,
# am Ende Summe und Restprüfung des ganzen Laufs. Das Protokoll enthält
# keine Heimnetzdaten: die Gateway-Adresse des Hosts geht nur als Variable
# in die Hülle und wird nie ausgegeben.
# Exit: 0 alle Fälle grün und nichts hinterlassen, sonst 1.

set -u
HIER=$(cd "$(dirname "$(readlink -f "$0")")" && pwd)
H=$HIER/huelle.sh
A=$HIER/attrappen.sh
PROTOKOLL=${1:-}
ARBEIT=$(mktemp -d "${TMPDIR:-/tmp}/qd-selbsttest-XXXXXX")
HEIM_ECHT=$(getent passwd "$(id -u)" | cut -d: -f6)
N=0 OK=0 FEHL=()
AUS=$ARBEIT/ausgabe

[ -n "$PROTOKOLL" ] && exec > >(tee "$PROTOKOLL") 2>&1

qd_tmp() { find /tmp -maxdepth 1 -name 'qd-??????*' ! -name 'qd-selbsttest-*' -newer "$ARBEIT/start" 2>/dev/null | sort; }
profile() { find "$HEIM_ECHT/.cache/qd-pruefrahmen" -maxdepth 1 -name 'p-*' -newer "$ARBEIT/start" 2>/dev/null | sort; }
# wsdd ist ein Python-Skript: comm ist „python3“, erkannt wird es an argv (wie reste.py).
wsdd_zahl() { python3 -I "$HIER/reste.py" vorher | python3 -I -c 'import json, sys; print(len(json.load(sys.stdin)["wsdd"]))'; }
lauscht_2222() { grep -c ' 0100007F:08AE 00000000:0000 0A ' /proc/net/tcp; }

touch "$ARBEIT/start"
WSDD_VORHER=$(wsdd_zahl)
PORTAL_VORHER=$(findmnt -rn -t fuse.portal | wc -l)
echo "Selbsttest Prüfrahmen $(date '+%F %T'), $(git -C "$HIER" rev-parse --short HEAD 2>/dev/null) $(sha256sum "$H" | cut -c1-12)"
echo "Host: $(uname -r), bwrap $(bwrap --version | cut -d' ' -f2), $(dbus-daemon --version | head -1 | sed 's/.* //'), gvfs $(dpkg-query -W -f='${Version}' gvfs-daemons 2>/dev/null), userns-Einschränkung $(sysctl -n kernel.apparmor_restrict_unprivileged_userns)"
echo "Ausgang: wsdd $WSDD_VORHER, fuse.portal $PORTAL_VORHER"
echo

# fall NAME BESCHREIBUNG FUNKTION – FUNKTION liefert 0 bei Erfolg.
fall() {
  N=$((N + 1))
  local t0=${EPOCHREALTIME/,/.} rc
  : > "$AUS"
  "$3" > "$AUS" 2>&1
  rc=$?
  local dauer
  dauer=$(awk -v a="$t0" -v b="${EPOCHREALTIME/,/.}" 'BEGIN { printf "%.1f", b - a }')
  if [ "$rc" = 0 ]; then
    OK=$((OK + 1)); printf 'ok    %02d %-28s %s (%ss)\n' "$N" "$1" "$2" "$dauer"
  else
    FEHL+=("$N $1"); printf 'FEHL  %02d %-28s %s (%ss)\n' "$N" "$1" "$2" "$dauer"
    sed 's/^/        | /' "$AUS" | tail -25
  fi
}
hat() { grep -a -q -- "$1" "$AUS.l"; }

# ------------------------------------------------------------------ Fälle
t_l_startet() { "$H" L -- true; }
t_exit_durch() { "$H" L -- sh -c 'exit 7'; [ $? = 7 ]; }
t_aufruf() {
  "$H" X -- true; [ $? = 90 ] || return 1
  "$H" L true; [ $? = 90 ] || return 1
  "$H" --sshd L -- true; [ $? = 90 ] || return 1
  "$H" --schreiben / L -- true; [ $? = 90 ]
}
t_umgebung() {
  QUERENCIA_DESKTOP_RECORD=/tmp/live WAYLAND_DISPLAY=wayland-0 DISPLAY=:0 SSH_AUTH_SOCK=/tmp/x \
    "$H" L -- env > "$AUS.l" || return 1
  cat "$AUS.l"
  ! grep -a -q -E '^(QUERENCIA_|WAYLAND_DISPLAY|DISPLAY|SSH_AUTH_SOCK|SESSION_MANAGER|XAUTHORITY|DBUS_STARTER)' "$AUS.l" || return 1
  local erwartet="DBUS_SESSION_BUS_ADDRESS DBUS_SYSTEM_BUS_ADDRESS GIO_USE_VFS GIO_USE_VOLUME_MONITOR GSETTINGS_BACKEND GTK_A11Y GTK_USE_PORTAL GVFS_REMOTE_VOLUME_MONITOR_IGNORE HOME LANG LC_ALL LOGNAME NO_AT_BRIDGE OLDPWD PATH PWD QD_HUELLE QD_PROFIL SHLVL TERM TMPDIR USER XDG_CACHE_HOME XDG_CONFIG_DIRS XDG_CONFIG_HOME XDG_DATA_DIRS XDG_DATA_HOME XDG_RUNTIME_DIR XDG_STATE_HOME _"
  [ "$(cut -d= -f1 "$AUS.l" | LC_ALL=C sort | tr '\n' ' ')" = "$(tr ' ' '\n' <<<"$erwartet" | LC_ALL=C sort | tr '\n' ' ')" ]
}
t_home() {
  "$H" L -- sh -c 'echo "HOME=$HOME"; ls -A "'"$HEIM_ECHT"'"; for d in .ssh .config .local .mozilla Schreibtisch Desktop; do [ -e "'"$HEIM_ECHT"'/$d" ] && echo "SICHTBAR $d"; done; touch "$HOME/qd-heimprobe"; echo ok' > "$AUS.l" || return 1
  cat "$AUS.l"
  hat "^HOME=$HEIM_ECHT/.cache/qd-pruefrahmen/p-.*/heim$" && ! hat SICHTBAR && hat '^ok$' && [ ! -e "$HEIM_ECHT/qd-heimprobe" ]
}
t_profil() {
  local p=$ARBEIT/profil-behalten
  "$H" --profil "$p" L -- sh -c 'echo inhalt > "$HOME/bleibt.txt"' || return 1
  [ -f "$p/heim/bleibt.txt" ] && [ -f "$p/.huelle-reste.json" ] || return 1
  "$H" --profil "$p" L -- sh -c 'cat "$HOME/bleibt.txt"' | grep -q inhalt || return 1
  mkdir -p "$ARBEIT/fremd"; echo x > "$ARBEIT/fremd/datei"
  "$H" --profil "$ARBEIT/fremd" L -- true; [ $? = 90 ] || return 1
  [ -z "$(profile)" ]   # Wegwerfprofile der Fälle bis hier sind weg
}
t_bus_privat() {
  "$H" L -- sh -c 'echo "$DBUS_SESSION_BUS_ADDRESS"; gdbus call --timeout 10 --session -d org.freedesktop.DBus -o /org/freedesktop/DBus -m org.freedesktop.DBus.ListActivatableNames; ls /run/user/; [ -e /run/user/'"$(id -u)"'/bus ] && echo LIVEBUS; true' > "$AUS.l" || return 1
  cat "$AUS.l"
  hat '^unix:path=/tmp/qd-[A-Za-z0-9]\{6\}/bus$' && hat "^(\['org.freedesktop.DBus'\],)$" && ! hat LIVEBUS
}
t_systembus() {
  "$H" G -- sh -c 'gdbus call --timeout 10 --system -d org.freedesktop.DBus -o /org/freedesktop/DBus -m org.freedesktop.DBus.ListActivatableNames; for n in org.freedesktop.Avahi org.freedesktop.UDisks2 org.freedesktop.NetworkManager org.freedesktop.login1; do gdbus call --timeout 10 --system -d org.freedesktop.DBus -o /org/freedesktop/DBus -m org.freedesktop.DBus.NameHasOwner $n; done; ls -l /run/dbus' > "$AUS.l" || return 1
  cat "$AUS.l"
  hat "^(\['org.freedesktop.DBus'\],)$" && [ "$(grep -a -c '^(false,)$' "$AUS.l")" = 4 ] && ! hat '^(true,)$'
}
t_laufzeit_kurz() {
  "$H" G -- sh -c 'echo "$XDG_RUNTIME_DIR"; stat -c %a "$XDG_RUNTIME_DIR"' > "$AUS.l" || return 1
  cat "$AUS.l"
  local r; r=$(head -1 "$AUS.l")
  [[ $r =~ ^/tmp/qd-[A-Za-z0-9]{6}$ ]] && [ $(( ${#r} + 30 )) -lt 108 ] && hat '^700$' && [ ! -e "$r" ]
}
t_tmp_privat() {
  local m; m=$(mktemp /tmp/qd-selbsttest-sichtbar-XXXXXX)
  "$H" L -- sh -c "ls -A /tmp; [ -e '$m' ] && echo SICHTBAR; touch /tmp/innen" > "$AUS.l"; local rc=$?
  rm -f "$m"; cat "$AUS.l"
  [ $rc = 0 ] && ! hat SICHTBAR && [ ! -e /tmp/innen ]
}
t_netz_lo() {
  "$H" G -- sh -c 'tail -n +3 /proc/net/dev | cut -d: -f1; cat /proc/net/route | wc -l; hostname' > "$AUS.l" || return 1
  cat "$AUS.l"
  [ "$(sed -n 1p "$AUS.l" | tr -d ' ')" = lo ] && [ "$(sed -n 2p "$AUS.l")" = 1 ] && hat '^qd-huelle$'
}
t_heimnetz() {
  # Gateway des Hosts (Heimnetz) nur als Variable, nie ausgegeben.
  local gw
  gw=$(awk 'NR > 1 && $2 == "00000000" { print $3; exit }' /proc/net/route)
  [ -n "$gw" ] || { echo "kein Standard-Gateway auf dem Host"; return 1; }
  "$H" --setenv "QD_ZIEL_HEX=$gw" G -- python3 -I -c '
import os, socket, struct
nur_lo = [z.split(":")[0].strip() for z in open("/proc/net/dev").read().splitlines()[2:]] == ["lo"]
if not nur_lo:
    print("ABBRUCH: mehr als lo sichtbar, kein Verbindungsversuch"); raise SystemExit(3)
ziel = socket.inet_ntoa(struct.pack("<I", int(os.environ["QD_ZIEL_HEX"], 16)))
ergebnis = []
for port in (445, 80, 22):
    s = socket.socket(); s.settimeout(3)
    try:
        s.connect((ziel, port)); ergebnis.append("VERBUNDEN")
    except OSError as e:
        ergebnis.append(e.errno)
    s.close()
u = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
try:
    u.sendto(b"probe", ("239.255.255.250", 3702)); ergebnis.append("GESENDET")
except OSError as e:
    ergebnis.append(e.errno)
try:
    socket.getaddrinfo("example.org", 80); ergebnis.append("AUFGELOEST")
except OSError as e:
    ergebnis.append("dns-" + type(e).__name__)
print("Gateway des Hosts (Adresse nicht protokolliert): tcp/445 tcp/80 tcp/22 udp-multicast dns ->", ergebnis)
raise SystemExit(0 if ergebnis[:4] == [101, 101, 101, 101] and ergebnis[4].startswith("dns-") else 1)
'
}
t_gvfsd() {
  "$H" G -- sh -c 'gio list trash:/// && echo LISTE-OK; gdbus call --timeout 10 --session -d org.freedesktop.DBus -o /org/freedesktop/DBus -m org.freedesktop.DBus.NameHasOwner org.gtk.vfs.Daemon; for p in /proc/[0-9]*; do cat $p/comm; done | sort -u | tr "\n" " "' > "$AUS.l" || return 1
  cat "$AUS.l"
  hat LISTE-OK && hat '^(true,)$' && hat 'gvfsd ' && hat 'gvfsd-trash'
}
t_papierkorb() {
  local echt=$HEIM_ECHT/.local/share/Trash vorher nachher
  vorher=$( { ls -A "$echt/files" 2>/dev/null | wc -l; stat -c %Y "$echt/files" "$echt/info" 2>/dev/null; } | tr '\n' ' ')
  "$H" G -- sh -c 'echo weg > "$HOME/qd-papierkorb-probe.txt"; gio trash "$HOME/qd-papierkorb-probe.txt" && gio list trash:/// && ls "$XDG_DATA_HOME/Trash/files" "$XDG_DATA_HOME/Trash/info"; gio trash --empty; ls -A "$XDG_DATA_HOME/Trash/files" | wc -l' > "$AUS.l" || return 1
  nachher=$( { ls -A "$echt/files" 2>/dev/null | wc -l; stat -c %Y "$echt/files" "$echt/info" 2>/dev/null; } | tr '\n' ' ')
  cat "$AUS.l"; echo "echter Papierkorb vorher/nachher gleich: $([ "$vorher" = "$nachher" ] && echo ja || echo NEIN)"
  [ "$(grep -a -c 'qd-papierkorb-probe.txt' "$AUS.l")" -ge 2 ] && hat 'qd-papierkorb-probe.txt.trashinfo' && [ "$(tail -1 "$AUS.l")" = 0 ] && [ "$vorher" = "$nachher" ]
}
t_l_ohne_gvfs() {
  "$H" L -- sh -c 'echo "GIO_USE_VFS=$GIO_USE_VFS"; gio list trash:/// ; echo "trash-exit $?"; gdbus call --timeout 10 --session -d org.freedesktop.DBus -o /org/freedesktop/DBus -m org.freedesktop.DBus.NameHasOwner org.gtk.vfs.Daemon' > "$AUS.l" 2>&1 || return 1
  cat "$AUS.l"
  hat '^GIO_USE_VFS=local$' && ! hat '^trash-exit 0$' && hat '^(false,)$'
}
t_wsdd_aus() {
  "$H" G -- sh -c 'gsettings get org.gnome.system.wsdd display-mode; gsettings get org.gnome.desktop.interface font-name; timeout 20 gio list network:///; echo "network-exit $?"; for p in /proc/[0-9]*; do [ "$(tr "\0" "\n" < $p/cmdline 2>/dev/null | sed -n 2p)" = /usr/bin/wsdd ] && echo "WSDD ${p#/proc/}"; done; true' > "$AUS.l" 2>&1 || return 1
  cat "$AUS.l"
  hat "^'disabled'$" && hat "^'DejaVu Sans 12'$" && hat '^network-exit 0$' && ! hat '^WSDD'
}
t_wsdd_probe() {
  local vorher; vorher=$(wsdd_zahl)
  "$H" --protokoll "$ARBEIT/wsdd-probe.log" --setenv GVFS_DEBUG=1 --wsdd-probe G -- sh -c 'gsettings get org.gnome.system.wsdd display-mode; timeout 6 gio list wsdd:/// ; echo "wsdd-list-exit $?"; for p in /proc/[0-9]*; do cat $p/comm; done | sort -u | tr "\n" " "' > "$AUS.l" 2>&1
  local rc=$?
  cat "$AUS.l"; grep -a -E 'Spawning our own wsdd|wsdd daemon' "$ARBEIT/wsdd-probe.log" "$AUS.l" | sed 's/^[^:]*://' | sort -u
  echo "wsdd auf dem Host vorher $vorher, nachher $(wsdd_zahl)"
  [ $rc = 0 ] && hat "^'merged'$" && hat 'gvfsd-wsdd' && grep -a -q 'Spawning our own wsdd daemon' "$AUS.l" "$ARBEIT/wsdd-probe.log" && [ "$(wsdd_zahl)" -le "$vorher" ]
}
t_sftp() {
  local vorher; vorher=$(lauscht_2222)
  "$H" --sshd G -- bash -c 'printf "Testinhalt SFTP\n" > "$QD_PROFIL/sftp-freigabe/hallo.txt"
    timeout 20 gio mount "$QD_SSH_URI" </dev/null; echo "mount-exit $?"
    gio mount -l | grep -a "sftp://"
    gio cat "${QD_SSH_URI}hallo.txt"
    gio copy "${QD_SSH_URI}hallo.txt" "$HOME/kopie.txt" && cat "$HOME/kopie.txt"
    gio list "sftp://$(id -un)@127.0.0.1:2222/home/"
    timeout 20 gio mount -u "$QD_SSH_URI"; echo "umount-exit $?"
    grep -a -c "ssh-Hülle" "$QD_SSH_DIR/huelle.log"; ls -A "$QD_SSH_DIR"' > "$AUS.l" 2>&1 || return 1
  cat "$AUS.l"; echo "Host 127.0.0.1:2222 lauscht vorher $vorher, nachher $(lauscht_2222)"
  hat '^mount-exit 0$' && hat 'sftp://.*@127.0.0.1:2222/' && [ "$(grep -a -c '^Testinhalt SFTP$' "$AUS.l")" = 2 ] && hat '^umount-exit 0$' && [ "$(lauscht_2222)" = "$vorher" ]
}
t_att_eingehaengt() { "$A" eingehaengt pruefen L; }
t_att_schreibbar() { "$A" schreibbar pruefen L; }
t_att_nicht() { "$A" nicht-eingehaengt pruefen L; }
t_att_grenze() { "$A" grenze pruefen L; }
t_att_alle_g() { "$A" alle pruefen G; }
t_aufraeumen() {
  # Hintergrundprozesse, auch mit setsid abgelöst, enden mit dem Namensraum.
  "$H" --protokoll "$ARBEIT/aufraeumen.log" G -- sh -c 'sleep 300 & setsid sh -c "sleep 301" & nohup sleep 302 >/dev/null 2>&1 & gio list trash:/// >/dev/null; echo gestartet; exit 3'
  [ $? = 3 ] || return 1
  grep -a -A12 'Restbericht' "$ARBEIT/aufraeumen.log"
  grep -a -q '"reste": \[\]' "$ARBEIT/aufraeumen.log" && [ -z "$(qd_tmp)" ]
}
t_zeitlimit() {
  "$H" --zeitlimit 3 L -- sleep 60; local rc=$?
  echo "Exit $rc"; [ $rc = 93 ] && [ -z "$(qd_tmp)" ] && [ -z "$(profile)" ]
}
t_isolation_92() {
  # Live-Laufzeitordner absichtlich freigegeben: die Vorprüfung muss vor dem Befehl abbrechen.
  "$H" --lesen "/run/user/$(id -u)" L -- sh -c 'echo BEFEHL-LIEF'; local rc=$?
  echo "Exit $rc"; [ $rc = 92 ] && ! grep -a -q BEFEHL-LIEF "$AUS"
}
t_reste_95() {
  # Ein Fremdprozess mit Bezug zum Profil (draußen gestartet) muss als Rest gelten.
  local p=$ARBEIT/profil-rest koeder rc r
  mkdir -p "$p"; : > "$p/.qd-profil"
  ( "$H" --profil "$p" --protokoll "$ARBEIT/rest.log" L -- sh -c 'while [ ! -e "$QD_PROFIL/koeder-da" ]; do sleep 0.1; done' ) &
  local huelle_pid=$!
  bash -c 'touch "$1/koeder-da"; while :; do sleep 1; done' koeder "$p" &
  koeder=$!
  wait "$huelle_pid"; rc=$?
  kill "$koeder"; wait "$koeder" 2>/dev/null
  echo "Exit $rc"; grep -a 'REST\|"reste"' "$ARBEIT/rest.log" | head -3
  r=$(sed -n '1s/.*Laufzeit \(\/tmp\/qd-[A-Za-z0-9]*\),.*/\1/p' "$ARBEIT/rest.log")
  # Der Lauf hat R zur Untersuchung stehen lassen: jetzt wegräumen.
  [ -n "$r" ] && [ -d "$r/.huelle" ] && rm -rf -- "$r"
  [ $rc = 95 ] && grep -a -q 'Prozess .* mit Bezug zum Lauf' "$ARBEIT/rest.log" && [ ! -e "$r" ]
}
t_reste_py() {
  # reste.py allein: Prozess mit fremdem XDG_RUNTIME_DIR wird erkannt.
  local r; r=$(mktemp -d /tmp/qd-selbsttest-rest-XXXXXX)
  env XDG_RUNTIME_DIR="$r" sleep 60 & local pid=$!
  python3 -I "$HIER/reste.py" nachher --laufzeit "$r"; local rc1=$?
  kill "$pid"; wait "$pid" 2>/dev/null
  python3 -I "$HIER/reste.py" nachher --laufzeit "$r"; local rc2=$?
  rmdir "$r"; echo "mit Prozess $rc1, ohne $rc2"
  [ $rc1 = 1 ] && [ $rc2 = 0 ]
}
t_labwc() {
  "$H" --labwc L -- python3 -I -c '
import os, gi
gi.require_version("Gtk", "4.0"); gi.require_version("Gdk", "4.0")
from gi.repository import Gtk, Gdk
Gtk.init()
d = Gdk.Display.get_default()
print(type(d).__name__, os.environ["WAYLAND_DISPLAY"], os.path.exists(os.path.join(os.environ["XDG_RUNTIME_DIR"], "wayland-0")), os.path.exists("/run/user/%d/wayland-0" % os.getuid()))
' > "$AUS.l" || return 1
  cat "$AUS.l"; hat '^GdkWaylandDisplay wayland-0 True False$'
}

fall l-startet          "Stufe L startet, Exit 0"                       t_l_startet
fall exit-durchgereicht "Exit des Befehls wird durchgereicht (7)"      t_exit_durch
fall aufruffehler       "falscher Aufruf -> 90"                         t_aufruf
fall umgebung           "env -i: nur gesetzte Variablen, kein QUERENCIA_*/WAYLAND" t_umgebung
fall home-isoliert      "HOME im Profil, echtes HOME unsichtbar"        t_home
fall profil             "--profil bleibt, Wegwerfprofil geht, Schutz"   t_profil
fall bus-privat         "Session-Bus privat, nichts aktivierbar (L)"    t_bus_privat
fall system-bus         "Attrappen-System-Bus ohne Avahi/UDisks (QD-142)" t_systembus
fall laufzeit-kurz      "XDG_RUNTIME_DIR /tmp/qd-xxxxxx, 0700, danach weg" t_laufzeit_kurz
fall tmp-privat         "/tmp in der Hülle privat"                      t_tmp_privat
fall netz-nur-lo        "nur lo, keine Route, eigener Hostname"         t_netz_lo
fall heimnetz           "Gateway des Hosts unerreichbar (G)"            t_heimnetz
fall gvfsd              "G: gvfsd im Wegwerfbus, trash:/// geht"        t_gvfsd
fall papierkorb         "G: Papierkorb nur im Profil, echter unberührt" t_papierkorb
fall l-ohne-gvfs        "L: GIO_USE_VFS=local, kein gvfsd"              t_l_ohne_gvfs
fall wsdd-aus           "G: wsdd abgeschaltet, network:/// ohne wsdd"   t_wsdd_aus
fall wsdd-probe         "G: wsdd erlaubt -> endet in der Hülle, kein Rest" t_wsdd_probe
fall sftp               "G --sshd: SFTP über 127.0.0.1:2222 im Namensraum" t_sftp
fall attrappe-eingeh    "Attrappe eingehängt (GMount, nur lesend)"      t_att_eingehaengt
fall attrappe-schreib   "Attrappe schreibbar (ramfs-GMount)"            t_att_schreibbar
fall attrappe-nicht     "Attrappe nicht eingehängt (GVolume, fstab)"    t_att_nicht
fall attrappe-grenze    "Attrappe Dateisystemgrenze (tmpfs 1 MiB)"      t_att_grenze
fall attrappen-alle-g   "alle Attrappen zusammen in Stufe G"            t_att_alle_g
fall aufraeumen         "Hintergrund-/setsid-Prozesse enden mit dem Lauf" t_aufraeumen
fall zeitlimit          "--zeitlimit -> 93, nichts bleibt"              t_zeitlimit
fall isolation-92       "Live-Laufzeit sichtbar -> 92 vor dem Befehl"   t_isolation_92
fall reste-95           "Rest mit Bezug zum Profil -> 95"               t_reste_95
fall reste-py           "reste.py erkennt fremdes XDG_RUNTIME_DIR"      t_reste_py
fall labwc              "--labwc: eigener Wayland-Socket, Live unsichtbar" t_labwc

# ---------------------------------------------------------- Gesamtreste
echo
rm -rf -- "$ARBEIT/profil-behalten" "$ARBEIT/profil-rest" "$ARBEIT/fremd"
WSDD_NACHHER=$(wsdd_zahl)
PORTAL_NACHHER=$(findmnt -rn -t fuse.portal | wc -l)
LT=$(qd_tmp); PR=$(profile)
echo "Ende: wsdd $WSDD_VORHER -> $WSDD_NACHHER, fuse.portal $PORTAL_VORHER -> $PORTAL_NACHHER, /tmp/qd-* übrig: ${LT:-keine}, Profile übrig: ${PR:-keine}"
SAUBER=1
[ "$WSDD_NACHHER" -le "$WSDD_VORHER" ] && [ "$PORTAL_NACHHER" -le "$PORTAL_VORHER" ] && [ -z "$LT" ] && [ -z "$PR" ] || SAUBER=0
rm -rf -- "$ARBEIT"
echo "Ergebnis: $OK/$N Fälle bestanden, Aufräumen $([ $SAUBER = 1 ] && echo sauber || echo NICHT SAUBER)"
[ ${#FEHL[@]} = 0 ] || echo "Fehlgeschlagen: ${FEHL[*]}"
[ "$OK" = "$N" ] && [ $SAUBER = 1 ]
