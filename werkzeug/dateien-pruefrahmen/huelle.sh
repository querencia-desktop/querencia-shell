#!/bin/bash
# Prüfrahmen Querencia Dateien (QD-140 Welle A, QD-142): gehärtete Hülle für
# Tests von Dateioperationen, GVfs und Laufwerken. Bedienung: regeln.md.
#
#   huelle.sh [Optionen] L|G -- BEFEHL [ARG …]
#
# Stufe L (lokal): GIO_USE_VFS=local, GIO_USE_VOLUME_MONITOR=unix, keine
#   aktivierbaren D-Bus-Dienste.
# Stufe G (GVfs): gvfsd und gvfsd-metadata im Wegwerfbus, sonst wie L.
# Beide Stufen: env -i mit nur gesetzten Variablen, eigenes HOME und
#   XDG_*_HOME im Profil, XDG_RUNTIME_DIR=/tmp/qd-xxxxxx, privater
#   Session-Bus, Attrappen-System-Bus unter /run/dbus (QD-142), bwrap mit
#   eigenem Nutzer-, PID-, Netz- (nur lo), IPC- und UTS-Namensraum.
#   Unsichtbar: das echte HOME (außer --lesen/--schreiben), /run (Live-Bus,
#   Wayland, Portale, gvfs-fuse, Avahi), /media, /mnt, /tmp.
#   Endet der Befehl, endet der PID-Namensraum und mit ihm jeder Prozess.
#
# Optionen:
#   --profil DIR         Profil (HOME, XDG-Ordner) hier anlegen und behalten
#   --lesen PFAD         Pfad nur lesend sichtbar machen (wiederholbar)
#   --schreiben PFAD     Pfad schreibbar sichtbar machen (wiederholbar)
#   --ordner PFAD        Arbeitsordner des Befehls (Vorgabe: HOME im Profil)
#   --setenv NAME=WERT   zusätzliche Variable (wiederholbar)
#   --sprache LOCALE     LANG/LC_ALL (Vorgabe C.UTF-8)
#   --cargo              ~/.cargo/bin im PATH, CARGO_HOME und RUSTUP_HOME
#                        lesend, RUSTC_WRAPPER leer (kein sccache-Server)
#   --sshd               nur G: Test-sshd 127.0.0.1:2222 (nur SFTP,
#                        Wegwerfschlüssel), ssh-Hülle vorn im PATH
#   --labwc              headless labwc 0.9.8 in der Hülle, WAYLAND_DISPLAY
#   --einhaengen Q:NAME  ganzes Dateisystem Q nur lesend als /media/NAME
#                        (erscheint als GMount)
#   --ramfs NAME         schreibbares ramfs als /media/NAME (GMount)
#   --tmpfs PFAD[:GRÖSSE] tmpfs (für GIO unsichtbar) als Dateisystemgrenze;
#                        PFAD relativ zum Profil oder unter /media, /mnt
#   --fstab DATEI        LIBMOUNT_FSTAB (nicht eingehängte GVolume)
#   --verzeichnis PFAD   leeres Verzeichnis unter /media oder /mnt anlegen
#   --zeitlimit S        Abbruch nach S Sekunden (Vorgabe 600)
#   --protokoll DATEI    Hüllenprotokoll und Restbericht (JSON) dorthin
#   --wsdd-probe         wsdd NICHT abschalten (nur für den Selbsttest)
#
# Exit: Exit des Befehls; 90 Aufruffehler, 91 Aufbau gescheitert,
#   92 Isolation verletzt (Vorprüfung), 93 Zeitlimit, 95 Reste gefunden.

set -u
HIER=$(cd "$(dirname "$(readlink -f "$0")")" && pwd)
SELBST=$HIER/huelle.sh

melden() { echo "huelle: $*" >&2; }

# ---------------------------------------------------------------- innen
if [ "${1:-}" = "--_innen" ]; then
  # shellcheck disable=SC1090
  source "$2"
  P_LOG=$R/.huelle/protokoll
  status() { echo "$1" > "$R/.huelle/status"; }
  log() { echo "innen $(date +%T.%N | cut -c1-12) $*" >> "$P_LOG"; }

  # Vorprüfung: nur lo, kein Live-Bus, kein echtes HOME.
  netz=$(tail -n +3 /proc/net/dev | cut -d: -f1 | tr -d ' ' | sort | tr '\n' ' ')
  if [ "$netz" != "lo " ]; then status "isolation netz=$netz"; exit 92; fi
  for verboten in "/run/user/$HOST_UID/bus" "/run/user/$HOST_UID/wayland-0" \
                  /run/dbus/system_bus_socket /run/avahi-daemon/socket \
                  "$HEIM_ECHT/.ssh" "$HEIM_ECHT/.local/share/Trash"; do
    if [ -e "$verboten" ]; then status "isolation sichtbar=$verboten"; exit 92; fi
  done
  if [ "$(id -u)" != "$HOST_UID" ]; then status "isolation uid=$(id -u)"; exit 92; fi
  log "Vorprüfung ok (netz=$netz uid=$(id -u) pid=$$)"

  KIND_PIDS=()
  # Startet einen Bus, wartet auf seine Adresse (Ereignis statt sleep).
  bus_starten() { # NAME KONFIG
    local fifo=$R/.huelle/$1.fifo addr
    mkfifo "$fifo"
    dbus-daemon --nofork --config-file="$2" --print-address=5 5>"$fifo" \
      </dev/null >>"$P_LOG" 2>&1 &
    KIND_PIDS+=($!)
    if ! read -r -t 10 addr < "$fifo"; then status "aufbau bus $1"; exit 91; fi
    rm -f "$fifo"
    log "Bus $1: $addr (pid ${KIND_PIDS[-1]})"
  }

  if [ "$SSHD" = 1 ]; then
    D=$P/ssh
    mkdir -p "$D" "$P/sftp-freigabe" "$P/bin"
    ssh-keygen -q -t ed25519 -N '' -C qd-host -f "$D/host_ed25519"
    ssh-keygen -q -t ed25519 -N '' -C qd-client -f "$D/client_ed25519"
    cp "$D/client_ed25519.pub" "$D/authorized_keys"
    : > "$D/ssh_config"
    cat > "$D/sshd_config" <<EOF
# Prüfrahmen: Test-sshd, unprivilegiert, nur 127.0.0.1:2222 im eigenen Netz-Namensraum, nur SFTP.
ListenAddress 127.0.0.1
Port 2222
HostKey $D/host_ed25519
PidFile $D/sshd.pid
AuthorizedKeysFile $D/authorized_keys
StrictModes no
UsePAM no
PubkeyAuthentication yes
PasswordAuthentication yes
KbdInteractiveAuthentication no
PermitRootLogin no
AllowUsers $(id -un)
AllowTcpForwarding no
AllowAgentForwarding no
AllowStreamLocalForwarding no
X11Forwarding no
PermitTTY no
PermitTunnel no
PermitUserRC no
Subsystem sftp internal-sftp
ForceCommand internal-sftp -d $P/sftp-freigabe
LogLevel VERBOSE
EOF
    cat > "$P/bin/ssh" <<'EOF'
#!/bin/bash
# Prüfrahmen: ssh-Hülle für gvfsd-sftp (nach W4 QF-0m). Eigene Konfiguration,
# eigene known_hosts, Wegwerfschlüssel; kein ControlMaster (108-Byte-Grenze).
# QD_SSH_HOSTKEY_FRAGE=1 entfernt GVfs' NoHostAuthenticationForLocalhost.
# QD_SSH_OHNE_SCHLUESSEL=1 bietet keinen Schlüssel an.
D="${QD_SSH_DIR:?QD_SSH_DIR fehlt}"
args=()
for a in "$@"; do
  if [ -n "${QD_SSH_HOSTKEY_FRAGE:-}" ] && [ "$a" = "-oNoHostAuthenticationForLocalhost yes" ]; then continue; fi
  args+=("$a")
done
id="$D/client_ed25519"; [ -n "${QD_SSH_OHNE_SCHLUESSEL:-}" ] && id=/dev/null
echo "$(date '+%T') ssh-Hülle: ${args[*]}" >> "$D/huelle.log"
exec /usr/bin/ssh -F "$D/ssh_config" -o UserKnownHostsFile="$D/known_hosts" -o GlobalKnownHostsFile=/dev/null \
  -o IdentitiesOnly=yes -o IdentityFile="$id" -o IdentityAgent=none -o ControlMaster=no -o ControlPath=none \
  -E "$D/ssh-client.log" "${args[@]}"
EOF
    chmod 755 "$P/bin/ssh"
    export PATH="$P/bin:$PATH" QD_SSH_DIR="$D"
    /usr/sbin/sshd -D -f "$D/sshd_config" -E "$D/sshd.log" </dev/null &
    KIND_PIDS+=($!)
    # Warten, bis 127.0.0.1:2222 lauscht (0100007F:08AE, Zustand 0A).
    for _ in $(seq 100); do
      grep -q ' 0100007F:08AE 00000000:0000 0A ' /proc/net/tcp && break
      kill -0 "${KIND_PIDS[-1]}" 2>/dev/null || break
      sleep 0.05
    done
    if ! grep -q ' 0100007F:08AE 00000000:0000 0A ' /proc/net/tcp; then status "aufbau sshd"; exit 91; fi
    export QD_SSH_URI="sftp://$(id -un)@127.0.0.1:2222$P/sftp-freigabe/"
    log "sshd lauscht auf 127.0.0.1:2222 (pid ${KIND_PIDS[-1]})"
  fi

  bus_starten system "$R/.huelle/system.conf"
  bus_starten session "$R/.huelle/session.conf"

  if [ "$LABWC" = 1 ]; then
    mkdir -p "$P/config/labwc"
    [ -e "$P/config/labwc/rc.xml" ] || printf '%s\n' '<?xml version="1.0"?>' '<labwc_config>' \
      '  <core><decoration>server</decoration></core>' '</labwc_config>' > "$P/config/labwc/rc.xml"
    WLR_BACKENDS=headless WLR_RENDERER=pixman WLR_HEADLESS_OUTPUTS=1 WLR_LIBINPUT_NO_DEVICES=1 \
      XKB_DEFAULT_LAYOUT=de "$LABWC_BIN" -C "$P/config/labwc" </dev/null >>"$P/labwc.log" 2>&1 &
    KIND_PIDS+=($!)
    for _ in $(seq 200); do
      [ -S "$R/wayland-0" ] && break
      kill -0 "${KIND_PIDS[-1]}" 2>/dev/null || break
      sleep 0.05
    done
    if [ ! -S "$R/wayland-0" ]; then status "aufbau labwc"; exit 91; fi
    export WAYLAND_DISPLAY=wayland-0
    log "labwc bereit (pid ${KIND_PIDS[-1]})"
  fi

  status "laeuft"
  log "Befehl: ${BEFEHL[*]}"
  cd "$ORDNER" || { status "aufbau ordner"; exit 91; }
  "${BEFEHL[@]}"
  rc=$?
  log "Befehl beendet: $rc"
  status "befehl $rc"

  # Geordnet beenden: alle Prozesse dieses PID-Namensraums außer 1 und mir.
  ende() {
    local p n=0
    for p in /proc/[0-9]*; do
      p=${p#/proc/}
      [ "$p" = 1 ] || [ "$p" = $$ ] || [ "$p" = "$BASHPID" ] && continue
      kill "-$1" "$p" 2>/dev/null && n=$((n + 1))
    done
    echo $n
  }
  ende TERM >/dev/null
  for _ in $(seq 60); do
    [ "$(ende 0)" = 0 ] && break
    sleep 0.05
  done
  log "beim Ende noch $(ende 0) Prozesse (endet mit dem Namensraum)"
  exit 0
fi

# ---------------------------------------------------------------- mitte
# Läuft in unshare -Urm (root im eigenen Nutzer-Namensraum, eigener
# Mount-Namensraum): Attrappen vorbereiten, dann bwrap.
if [ "${1:-}" = "--_mitte" ]; then
  # shellcheck disable=SC1090
  source "$2"
  B=(--ro-bind / / --dev /dev --proc /proc
     --tmpfs /tmp --bind "$R" "$R"
     --tmpfs /run --dir /run/dbus --dir /run/user
     --tmpfs /media --tmpfs /mnt
     --tmpfs "$HEIM_ECHT" --bind "$P" "$P" --ro-bind "$HIER" "$HIER")
  for p in "${LESEN[@]}"; do B+=(--ro-bind "$p" "$p"); done
  for p in "${SCHREIBEN[@]}"; do B+=(--bind "$p" "$p"); done
  # Ganze Dateisysteme, rekursiv gebunden: im Nutzer-Namensraum sind geerbte
  # Untereinhängungen gesperrt, /boot bringt deshalb /boot/efi als zweites
  # GMount mit (auch unter einem tmpfs, GIO liest verdeckte Einträge mit).
  for e in "${EINHAENGEN[@]}"; do B+=(--ro-bind "${e%%:*}" "/media/${e#*:}"); done
  # ramfs mit Gerätenamen /dev/qd-NAME: GIO blendet Quelle „none“ und tmpfs
  # als systemintern aus, ramfs mit eigenem Namen nicht.
  for n in "${RAMFS[@]}"; do
    mkdir -p "$STAGE/r-$n"
    mount -t ramfs "/dev/qd-$n" "$STAGE/r-$n" || exit 91
    chmod 0755 "$STAGE/r-$n"
    B+=(--bind "$STAGE/r-$n" "/media/$n")
  done
  for t in "${TMPFS[@]}"; do
    ziel=${t%%:*}; groesse=${t#*:}
    if [ "$groesse" != "$t" ]; then B+=(--size "$groesse"); fi
    B+=(--tmpfs "$ziel")
  done
  for v in "${VERZEICHNISSE[@]}"; do B+=(--dir "$v"); done
  B+=(--unshare-user --uid "$HOST_UID" --gid "$HOST_GID"
      --unshare-pid --unshare-net --unshare-ipc --unshare-uts --unshare-cgroup-try
      --hostname qd-huelle --die-with-parent --new-session --chdir /)
  exec /usr/bin/bwrap "${B[@]}" /usr/bin/env -i "${UMGEBUNG[@]}" /bin/bash "$SELBST" --_innen "$2"
fi

# ---------------------------------------------------------------- außen
fehler() { melden "$*"; exit 90; }
HOST_UID=$(id -u); HOST_GID=$(id -g)
HEIM_ECHT=$(getent passwd "$HOST_UID" | cut -d: -f6)
LABWC_BIN=$HEIM_ECHT/opt/labwc-0.9.8/bin/labwc
PROFIL="" ORDNER="" SPRACHE=C.UTF-8 CARGO=0 SSHD=0 LABWC=0 FSTAB="" ZEITLIMIT=600
PROTOKOLL="" WSDD_PROBE=0
LESEN=() SCHREIBEN=() SETENV=() EINHAENGEN=() RAMFS=() TMPFS=() VERZEICHNISSE=()

absolut() { realpath -e -- "$1" 2>/dev/null || fehler "Pfad fehlt: $1"; }
name_ok() { [[ $1 =~ ^[A-Za-z0-9][A-Za-z0-9._-]{0,40}$ ]] || fehler "ungültiger Name: $1"; }

while [ $# -gt 0 ]; do
  case $1 in
    --profil) PROFIL=$(realpath -m -- "${2:?}"); shift 2 ;;
    --lesen) LESEN+=("$(absolut "${2:?}")"); shift 2 ;;
    --schreiben) SCHREIBEN+=("$(absolut "${2:?}")"); shift 2 ;;
    --ordner) ORDNER=$(realpath -m -- "${2:?}"); shift 2 ;;
    --setenv) [[ ${2:-} =~ ^[A-Za-z_][A-Za-z0-9_]*= ]] || fehler "--setenv NAME=WERT"; SETENV+=("$2"); shift 2 ;;
    --sprache) SPRACHE=${2:?}; shift 2 ;;
    --cargo) CARGO=1; shift ;;
    --sshd) SSHD=1; shift ;;
    --labwc) LABWC=1; shift ;;
    --einhaengen) q=${2%%:*}; n=${2#*:}; [ "$q" != "$2" ] || fehler "--einhaengen QUELLE:NAME"
                  name_ok "$n"; EINHAENGEN+=("$(absolut "$q"):$n"); shift 2 ;;
    --ramfs) name_ok "${2:?}"; RAMFS+=("$2"); shift 2 ;;
    --tmpfs) TMPFS+=("${2:?}"); shift 2 ;;
    --fstab) FSTAB=$(absolut "${2:?}"); shift 2 ;;
    --verzeichnis) case ${2:?} in /media/?*|/mnt/?*) VERZEICHNISSE+=("$2") ;; *) fehler "--verzeichnis nur unter /media oder /mnt" ;; esac; shift 2 ;;
    --zeitlimit) [[ ${2:-} =~ ^[0-9]+$ ]] || fehler "--zeitlimit SEKUNDEN"; ZEITLIMIT=$2; shift 2 ;;
    --protokoll) PROTOKOLL=$(realpath -m -- "${2:?}"); shift 2 ;;
    --wsdd-probe) WSDD_PROBE=1; shift ;;
    L|G) STUFE=$1; shift; break ;;
    -h|--help) sed -n '2,/^set -u/p' "$0" | sed '$d; s/^# \{0,1\}//'; exit 0 ;;
    *) fehler "unbekannt: $1 (Aufruf: huelle.sh [Optionen] L|G -- BEFEHL …)" ;;
  esac
done
[ -n "${STUFE:-}" ] || fehler "Stufe L oder G fehlt"
[ "${1:-}" = "--" ] || fehler "nach der Stufe folgt -- BEFEHL"
shift
[ $# -gt 0 ] || fehler "Befehl fehlt"
BEFEHL=("$@")
[ "$SSHD" = 0 ] || [ "$STUFE" = G ] || fehler "--sshd nur in Stufe G"
[ "$LABWC" = 0 ] || [ -x "$LABWC_BIN" ] || fehler "labwc fehlt: $LABWC_BIN"
for p in "${SCHREIBEN[@]}"; do
  case $p in /|/usr|/usr/*|/etc|/etc/*|"$HEIM_ECHT") fehler "--schreiben $p ist zu weit" ;; esac
done

# Profil: eigenes oder Wegwerfprofil auf ext4 (Dateisystemgrenze zu tmpfs).
if [ -n "$PROFIL" ]; then
  if [ -e "$PROFIL" ] && [ -n "$(ls -A "$PROFIL")" ] && [ ! -e "$PROFIL/.qd-profil" ]; then
    fehler "Profil $PROFIL ist nicht leer und kein Prüfrahmen-Profil"
  fi
  mkdir -p "$PROFIL"; P=$PROFIL; BEHALTEN=1
else
  mkdir -p "$HEIM_ECHT/.cache/qd-pruefrahmen"
  P=$(mktemp -d "$HEIM_ECHT/.cache/qd-pruefrahmen/p-XXXXXX"); BEHALTEN=0
fi
: > "$P/.qd-profil"
mkdir -p "$P/heim" "$P/config/glib-2.0/settings" "$P/data" "$P/cache" "$P/state"
chmod 700 "$P"
[ -n "$ORDNER" ] || ORDNER=$P/heim
for t in "${!TMPFS[@]}"; do
  ziel=${TMPFS[$t]%%:*}; rest=${TMPFS[$t]#"$ziel"}
  case $ziel in
    /media/?*|/mnt/?*) ;;
    /*) fehler "--tmpfs nur relativ zum Profil oder unter /media, /mnt: $ziel" ;;
    *) case $ziel in *..*) fehler "--tmpfs ohne ..: $ziel" ;; esac; ziel=$P/$ziel ;;
  esac
  if [ -n "$rest" ]; then
    g=${rest#:}; g=$(numfmt --from=iec "${g^^}" 2>/dev/null) || fehler "Größe: $rest"
    TMPFS[$t]=$ziel:$g
  else
    TMPFS[$t]=$ziel
  fi
done

# Kurzes Laufzeitverzeichnis (Unix-Sockets ≤ 108 Byte).
R=$(mktemp -d /tmp/qd-XXXXXX); STAGE=$R.a
mkdir -m 700 "$STAGE" "$R/.huelle"
chmod 700 "$R"

# gsettings im Profil (keyfile): wsdd aus (QD-041), feste Schrift (PT-1).
KEYFILE=$P/config/glib-2.0/settings/keyfile
if [ ! -e "$KEYFILE" ]; then
  {
    if [ "$WSDD_PROBE" = 0 ]; then printf '[org/gnome/system/wsdd]\ndisplay-mode=%s\n\n' "'disabled'"; fi
    printf '[org/gnome/desktop/interface]\nfont-name=%s\n' "'DejaVu Sans 12'"
  } > "$KEYFILE"
fi

# D-Bus: Session-Bus ohne Standard-Dienstordner, Attrappen-System-Bus ohne Dienste.
mkdir -p "$R/.huelle/dienste"
bus_konfig() { # TYP LISTEN DIENSTORDNER
  cat <<EOF
<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>$1</type>
  <keep_umask/>
  <listen>$2</listen>
  <auth>EXTERNAL</auth>
  <apparmor mode="disabled"/>
  ${3:+<servicedir>$3</servicedir>}
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
EOF
}
bus_konfig session "unix:path=$R/bus" "$R/.huelle/dienste" > "$R/.huelle/session.conf"
bus_konfig system "unix:path=/run/dbus/system_bus_socket" "" > "$R/.huelle/system.conf"
if [ "$STUFE" = G ]; then
  printf '[D-BUS Service]\nName=org.gtk.vfs.Daemon\nExec=/usr/libexec/gvfsd --no-fuse\n' > "$R/.huelle/dienste/org.gtk.vfs.Daemon.service"
  printf '[D-BUS Service]\nName=org.gtk.vfs.Metadata\nExec=/usr/libexec/gvfsd-metadata\n' > "$R/.huelle/dienste/org.gtk.vfs.Metadata.service"
fi

# Umgebung: nur diese Variablen (env -i).
PFAD=/usr/local/bin:/usr/bin:/bin
UMGEBUNG=(
  "HOME=$P/heim" "USER=$(id -un)" "LOGNAME=$(id -un)" "LANG=$SPRACHE" "LC_ALL=$SPRACHE" "TERM=dumb"
  "XDG_CONFIG_HOME=$P/config" "XDG_DATA_HOME=$P/data" "XDG_CACHE_HOME=$P/cache" "XDG_STATE_HOME=$P/state"
  "XDG_RUNTIME_DIR=$R" "XDG_DATA_DIRS=/usr/local/share:/usr/share" "XDG_CONFIG_DIRS=/etc/xdg"
  "DBUS_SESSION_BUS_ADDRESS=unix:path=$R/bus" "DBUS_SYSTEM_BUS_ADDRESS=unix:path=/run/dbus/system_bus_socket"
  "GSETTINGS_BACKEND=keyfile" "GIO_USE_VOLUME_MONITOR=unix" "GVFS_REMOTE_VOLUME_MONITOR_IGNORE=1" "GTK_USE_PORTAL=0" "NO_AT_BRIDGE=1" "GTK_A11Y=none"
  "QD_HUELLE=$STUFE" "QD_PROFIL=$P" "TMPDIR=/tmp"
)
[ "$STUFE" = L ] && UMGEBUNG+=("GIO_USE_VFS=local")
[ -n "$FSTAB" ] && { cp "$FSTAB" "$R/.huelle/fstab"; UMGEBUNG+=("LIBMOUNT_FSTAB=$R/.huelle/fstab"); }
if [ "$CARGO" = 1 ]; then
  for d in "$HEIM_ECHT/.cargo" "$HEIM_ECHT/.rustup"; do [ -d "$d" ] && LESEN+=("$d"); done
  PFAD=$HEIM_ECHT/.cargo/bin:$PFAD
  UMGEBUNG+=("CARGO_HOME=$HEIM_ECHT/.cargo" "RUSTUP_HOME=$HEIM_ECHT/.rustup" "RUSTC_WRAPPER=" "CARGO_NET_OFFLINE=true")
fi
UMGEBUNG+=("PATH=$PFAD")
for e in "${SETENV[@]}"; do UMGEBUNG+=("$e"); done
[ "$LABWC" = 1 ] && LESEN+=("$(dirname "$(dirname "$LABWC_BIN")")")

# Sichtbarkeit des Arbeitsordners und des Befehls prüfen.
sichtbar() {
  case $1 in "$HEIM_ECHT"/*|/tmp/*|/run/*) ;; *) return 0 ;; esac
  local w
  for w in "$P" "$R" "$HIER" "${LESEN[@]}" "${SCHREIBEN[@]}"; do
    case $1 in "$w"|"$w"/*) return 0 ;; esac
  done
  return 1
}
sichtbar "$ORDNER" || fehler "Arbeitsordner $ORDNER ist in der Hülle unsichtbar (--lesen/--schreiben)"
case ${BEFEHL[0]} in /*) sichtbar "${BEFEHL[0]}" || fehler "Befehl ${BEFEHL[0]} ist in der Hülle unsichtbar (--lesen)" ;; esac

{
  declare -p STUFE P R STAGE HIER SELBST HOST_UID HOST_GID HEIM_ECHT ORDNER SSHD LABWC LABWC_BIN
  declare -p LESEN SCHREIBEN EINHAENGEN RAMFS TMPFS VERZEICHNISSE UMGEBUNG BEFEHL
} > "$R/.huelle/konfig"
KONFIG=$R/.huelle/konfig
echo "aussen $(date +%T) Stufe $STUFE, Laufzeit $R, Profil $P, Befehl: ${BEFEHL[*]}" > "$R/.huelle/protokoll"

python3 -I "$HIER/reste.py" vorher > "$R/.huelle/vorher.json"
start=${EPOCHREALTIME/,/.}
env -i PATH=/usr/bin:/bin LANG=C.UTF-8 \
  timeout --kill-after=10 "$ZEITLIMIT" \
  /usr/bin/unshare --user --map-root-user --mount --propagation private \
  /bin/bash "$SELBST" --_mitte "$KONFIG"
rc_lauf=$?
dauer=$(awk -v a="$start" -v b="${EPOCHREALTIME/,/.}" 'BEGIN { printf "%.1f", b - a }')
status=$(cat "$R/.huelle/status" 2>/dev/null || echo "kein-status")

case $status in
  "befehl "*) rc=${status#befehl } ;;
  isolation*) melden "Isolation verletzt: $status"; rc=92 ;;
  *) if [ "$rc_lauf" = 124 ] || [ "$rc_lauf" = 137 ]; then melden "Zeitlimit $ZEITLIMIT s"; rc=93
     else melden "Aufbau gescheitert: $status (Exit $rc_lauf)"; rc=91; fi ;;
esac

# Aufräumen und Restprüfung (nur lesend über /proc und mountinfo).
rmdir "$STAGE"/* "$STAGE" 2>/dev/null
cp "$R/.huelle/protokoll" "$P/.huelle-protokoll" 2>/dev/null
python3 -I "$HIER/reste.py" nachher --laufzeit "$R" --profil "$P" --vorher "$R/.huelle/vorher.json" \
  --json "$P/.huelle-reste.json"
rc_reste=$?
[ -e "$STAGE" ] && { melden "REST Bühne $STAGE nicht leer"; rc_reste=1; }
if [ -n "$PROTOKOLL" ]; then
  mkdir -p "$(dirname "$PROTOKOLL")"
  {
    cat "$R/.huelle/protokoll"
    echo "aussen Status: $status; Exit Lauf $rc_lauf; Dauer ${dauer}s"
    echo "aussen Restbericht:"; cat "$P/.huelle-reste.json"; echo
  } > "$PROTOKOLL"
fi
if [ "$rc_reste" = 0 ]; then
  # Schreibgeschützte Dateien aus Tests (W4-Falle 10) erst freigeben.
  chmod -R u+w -- "$R" 2>/dev/null; rm -rf -- "$R"
  if [ "$BEHALTEN" = 0 ]; then chmod -R u+w -- "$P" 2>/dev/null; rm -rf -- "$P"; fi
else
  rc=95
  melden "Reste gefunden; Laufzeit $R und Profil $P bleiben zur Untersuchung"
fi
melden "Stufe $STUFE, Befehl-Status: $status, Reste: $([ "$rc_reste" = 0 ] && echo keine || echo JA), Dauer ${dauer}s, Exit $rc"
exit "$rc"
