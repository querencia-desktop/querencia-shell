#!/bin/bash
# Prüfrahmen Querencia Dateien: Laufwerks-Attrappen ohne sudo (C §10.2 Nr. 3).
#
#   attrappen.sh ART optionen                 Hüllenoptionen ausgeben
#   attrappen.sh ART pruefen [L|G]            belegen, was GIO sieht (Exit 0 = wie erwartet)
#   attrappen.sh ART starten [L|G] -- BEFEHL  Befehl mit dieser Attrappe in der Hülle
#
# ART:
#   eingehaengt        ganzes Dateisystem nur lesend als /media/QD-Stick (GMount);
#                      Quelle: QD_ATTRAPPE_QUELLE, sonst das erste squashfs unter
#                      /snap (ohne Untereinhängungen), sonst /boot (dann
#                      erscheint /boot/efi als zweites GMount „efi“)
#   schreibbar         ramfs als /media/QD-Schreibbar (GMount, schreibbar, ohne
#                      Größengrenze)
#   nicht-eingehaengt  fstab-Zeile noauto,user,x-gvfs-show über LIBMOUNT_FSTAB
#                      (GVolume „QD-Nicht“, can_mount = True)
#   grenze             tmpfs 1 MiB als $QD_PROFIL/grenze neben dem ext4-HOME
#                      (Dateisystemgrenze, „Platte voll“; für GIO unsichtbar)
#   alle               alle vier zusammen

set -u
HIER=$(cd "$(dirname "$(readlink -f "$0")")" && pwd)

quelle() {
  if [ -n "${QD_ATTRAPPE_QUELLE:-}" ]; then echo "$QD_ATTRAPPE_QUELLE"; return; fi
  local q
  q=$(findmnt -rn -t squashfs -o TARGET | grep -m1 '^/snap/')
  echo "${q:-/boot}"
}

optionen() {
  case $1 in
    eingehaengt) echo "--einhaengen $(quelle):QD-Stick" ;;
    schreibbar) echo "--ramfs QD-Schreibbar" ;;
    nicht-eingehaengt) echo "--fstab $HIER/attrappen/fstab --verzeichnis /media/QD-Nicht" ;;
    grenze) echo "--tmpfs grenze:1M" ;;
    alle) echo "$(optionen eingehaengt) $(optionen schreibbar) $(optionen nicht-eingehaengt) $(optionen grenze)" ;;
    *) echo "attrappen.sh: unbekannte Attrappe: $1" >&2; return 90 ;;
  esac
}

# ---------------------------------------------------------- in der Hülle
if [ "${1:-}" = "--_innen" ]; then
  ART=$2
  [ -n "${QD_HUELLE:-}" ] || { echo "nur in huelle.sh" >&2; exit 90; }
  fehl=0
  pruef() { # BESCHREIBUNG BEFEHL…
    local b=$1; shift
    if "$@" >/dev/null 2>&1; then echo "  ok    $b"; else echo "  FEHL  $b"; fehl=1; fi
  }
  sicht=$(python3 -I "$HIER/gio-sicht.py" "$HOME" ${QD_GRENZE:+"$QD_PROFIL/grenze"})
  mounts=$(gio mount -l 2>&1)
  echo "== GIO-Sicht (gio-sicht.py) =="; echo "$sicht"
  echo "== gio mount -l =="; echo "$mounts"
  echo "== Prüfungen ($ART, Stufe $QD_HUELLE) =="
  hat() { grep -a -q -- "$1" <<<"$2"; }
  hat_nicht() { ! grep -a -q -- "$1" <<<"$2"; }
  fs_home=$(sed -n "s#^pfad $HOME filesystem=\([^ ]*\) .*#\1#p" <<<"$sicht")
  case $ART in
    eingehaengt|alle)
      pruef "GMount QD-Stick unter /media/QD-Stick, aushängbar" hat "^mount QD-Stick /media/QD-Stick can_unmount=True" "$sicht"
      pruef "QD-Stick nur lesend" hat "^mount QD-Stick .* readonly=True" "$sicht"
      pruef "gio mount -l nennt QD-Stick" hat "QD-Stick -> file:///media/QD-Stick" "$mounts"
      pruef "Inhalt lesbar" ls /media/QD-Stick
      pruef "Schreiben scheitert" bash -c '! touch /media/QD-Stick/qd-probe' ;;&
    schreibbar|alle)
      pruef "GMount QD-Schreibbar, aushängbar" hat "^mount QD-Schreibbar /media/QD-Schreibbar can_unmount=True" "$sicht"
      pruef "QD-Schreibbar schreibbar" hat "^mount QD-Schreibbar .* readonly=False" "$sicht"
      echo probe > "$HOME/probe.txt"
      pruef "gio copy HOME → QD-Schreibbar" gio copy "$HOME/probe.txt" /media/QD-Schreibbar/probe.txt
      pruef "Kopie gleich" cmp "$HOME/probe.txt" /media/QD-Schreibbar/probe.txt
      fs_r=$(gio info -a id::filesystem /media/QD-Schreibbar | sed -n 's/.*id::filesystem: //p')
      pruef "anderes Dateisystem als HOME ($fs_home ≠ $fs_r)" test "$fs_home" != "$fs_r"
      pruef "Aushängen scheitert ohne root (erwartet)" bash -c '! gio mount -u /media/QD-Schreibbar' ;;&
    nicht-eingehaengt|alle)
      pruef "GVolume QD-Nicht, einhängbar, nicht eingehängt" hat "^volume QD-Nicht can_mount=True mounted=False" "$sicht"
      pruef "gio mount -l nennt Volume QD-Nicht" hat "Volume(0): QD-Nicht" "$mounts"
      pruef "kein GMount QD-Nicht" hat_nicht "^mount QD-Nicht" "$sicht"
      pruef "Einhängen scheitert ohne root (erwartet)" bash -c '! gio mount -d /dev/disk/by-label/QD-NICHT' ;;&
    grenze|alle)
      fs_g=$(sed -n "s#^pfad $QD_PROFIL/grenze filesystem=\([^ ]*\) fstype=\(.*\)#\1 \2#p" <<<"$sicht")
      pruef "grenze auf eigenem Dateisystem ($fs_home ≠ ${fs_g% *})" test -n "$fs_g" -a "$fs_home" != "${fs_g% *}"
      pruef "grenze ist tmpfs" test "${fs_g#* }" = tmpfs
      pruef "GIO blendet grenze aus (kein GMount)" hat_nicht "grenze" "$(grep -a '^mount' <<<"$sicht")"
      echo verschieben > "$HOME/wandert.txt"
      pruef "gio move über die Grenze" gio move "$HOME/wandert.txt" "$QD_PROFIL/grenze/wandert.txt"
      pruef "Quelle danach weg" test ! -e "$HOME/wandert.txt"
      pruef "Platte voll: 2 MiB scheitern" bash -c '! head -c 2M /dev/zero > "$QD_PROFIL/grenze/gross"'
      rm -f "$QD_PROFIL/grenze/gross" ;;
    eingehaengt|schreibbar|nicht-eingehaengt) ;;
    *) echo "unbekannt: $ART" >&2; exit 90 ;;
  esac
  [ "$fehl" = 0 ] && echo "ERGEBNIS $ART: wie erwartet" || echo "ERGEBNIS $ART: ABWEICHUNG"
  exit "$fehl"
fi

# ---------------------------------------------------------- außen
ART=${1:-}; BEFEHL=${2:-}
[ -n "$ART" ] && [ -n "$BEFEHL" ] || { sed -n '2,/^set -u/p' "$0" | sed '$d; s/^# \{0,1\}//'; exit 90; }
OPT=$(optionen "$ART") || exit 90
case $BEFEHL in
  optionen) echo "$OPT" ;;
  pruefen)
    STUFE=${3:-L}
    extra=()
    case $ART in grenze|alle) extra=(--setenv QD_GRENZE=1) ;; esac
    # shellcheck disable=SC2086
    exec bash "$HIER/huelle.sh" $OPT "${extra[@]}" "$STUFE" -- /bin/bash "$HIER/attrappen.sh" --_innen "$ART" ;;
  starten)
    STUFE=${3:-L}; shift 3 2>/dev/null || shift $#
    [ "${1:-}" = "--" ] || { echo "attrappen.sh ART starten L|G -- BEFEHL" >&2; exit 90; }
    shift
    # shellcheck disable=SC2086
    exec bash "$HIER/huelle.sh" $OPT "$STUFE" -- "$@" ;;
  *) echo "attrappen.sh: optionen|pruefen|starten" >&2; exit 90 ;;
esac
