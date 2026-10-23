#!/bin/bash
# Querencia Dateien, Welle C1 (Netzwerk): isolierter GUI-Lauf im Prüfrahmen
# Stufe G (gvfsd im Wegwerf-Bus, Attrappen-System-Bus, Netz nur lo im
# eigenen Namensraum, headless labwc 0.9.8). Nach werkzeug/pruefung/lauf.sh
# (B1), erweitert um:
#   - Test-sshd auf 127.0.0.1:2222 (huelle.sh --sshd, Wegwerfschlüssel),
#     Freigabe zusätzlich als /tmp/qd-freigabe (neutraler Pfad für Fotos);
#   - ssh-Hülle bin/ssh vor der des Prüfrahmens (Fingerabdruck-Abruf ohne -E);
#   - Test-Server nur im Namensraum: webdav-attrappe.py (Basic-Auth),
#     ftp-attrappe.py, wsdd-attrappe.py (nie echtes Netz, nie das NAS);
#   - LAUF_SSH_HOSTKEY_FRAGE=1 / LAUF_SSH_OHNE_SCHLUESSEL=1 an die ssh-Hülle,
#     LAUF_ZEITGRENZE (Sekunden) an das Programm.
#
#   lauf.sh NAME SZENARIO.sh
#
# Umgebung wie pruefung/lauf.sh (LAUF_GROESSE, LAUF_DUNKEL, LAUF_SPRACHE,
# LAUF_BIN, LAUF_THEMA), dazu LAUF_STUFE (Vorgabe G), LAUF_WSDD (1: wsdd-
# Attrappe, display-mode 'merged'). Ausgabe:
# ${DATEIEN_ARBEIT:-~/projects/dateien-netzwerk-arbeit}/laeufe/NAME/.
set -euo pipefail
NAME=$1; SZENARIO=$(readlink -f "$2")
HIER=$(cd "$(dirname "$(readlink -f "$0")")" && pwd)
CRATE=$(cd "$HIER/../.." && pwd); REPO=$(cd "$CRATE/../.." && pwd)
PRUEFUNG=$CRATE/werkzeug/pruefung
A=${DATEIEN_ARBEIT:-$HOME/projects/dateien-netzwerk-arbeit}
BIN=${LAUF_BIN:-$REPO/target/debug/querencia-dateien}
GROESSE=${LAUF_GROESSE:-1024x768}; DUNKEL=${LAUF_DUNKEL:-0}; SPRACHE=${LAUF_SPRACHE:-de}
STUFE=${LAUF_STUFE:-G}
THEMA=${LAUF_THEMA:-$HOME/.local/share/themes/Querencia-Klassisch}
[ -x "$BIN" ] || { echo "lauf: Programm fehlt: $BIN" >&2; exit 1; }
[ -x "$A/bin/qeingabe" ] || { echo "lauf: erst DATEIEN_ARBEIT=$A werkzeug/pruefung/bauen-eingabe.sh" >&2; exit 1; }
OUT=$A/laeufe/$NAME; PROFIL=$A/profile/$NAME
[ -d "$PROFIL" ] && chmod -R u+w "$PROFIL"
rm -rf "$OUT" "$PROFIL"; mkdir -p "$OUT/fotos" "$PROFIL"
: > "$PROFIL/.qd-profil"
mkdir -p "$PROFIL/heim/.local/bin" "$PROFIL/config/glib-2.0/settings" "$PROFIL/config/labwc" \
  "$PROFIL/data/themes" "$PROFIL/data/icons/hicolor/scalable/apps" "$PROFIL/locale"
for c in querencia-dateien querencia-oberflaeche querencia-dateiarbeit; do
  (cd "$REPO/crates/$c" && python3 -I werkzeug/i18n.py mo "$PROFIL/locale" >/dev/null)
done
cp -rL --no-preserve=mode "$THEMA" "$PROFIL/data/themes/"
[ -d "$HOME/.local/share/icons/Querencia" ] && cp -rL --no-preserve=mode "$HOME/.local/share/icons/Querencia" "$PROFIL/data/icons/"
cp "$CRATE/data/icons/org.querencia.Dateien.svg" "$PROFIL/data/icons/hicolor/scalable/apps/"
SCHEMA=default; [ "$DUNKEL" = 1 ] && SCHEMA=prefer-dark
WSDD_MODUS="'disabled'"
if [ "${LAUF_WSDD:-0}" = 1 ]; then
  # gvfsd-wsdd startet „wsdd“ aus dem PATH: die Attrappe, nie das echte wsdd
  install -m 755 "$HIER/wsdd-attrappe.py" "$PROFIL/heim/.local/bin/wsdd"
  WSDD_MODUS="'merged'"
fi
{
  printf '[org/gnome/system/wsdd]\ndisplay-mode=%s\n\n' "$WSDD_MODUS"
  printf '[org/gnome/system/smb]\nworkgroup=%s\n\n' "'WORKGROUP'"
  printf '[org/gnome/desktop/interface]\nfont-name=%s\ngtk-theme=%s\nicon-theme=%s\ncolor-scheme=%s\n' \
    "'DejaVu Sans 12'" "'$(basename "$THEMA")'" "'Querencia'" "'$SCHEMA'"
} > "$PROFIL/config/glib-2.0/settings/keyfile"
cat > "$PROFIL/config/labwc/rc.xml" <<'XML'
<?xml version="1.0"?>
<labwc_config>
  <core><decoration>server</decoration></core>
  <windowRules>
    <windowRule identifier="org.querencia.Dateien" type="normal">
      <action name="MoveTo" x="0" y="0"/>
    </windowRule>
    <!-- fehler.sh E13: das zweite Fenster neben dem ersten -->
    <windowRule title="Berichte">
      <action name="MoveTo" x="600" y="330"/>
    </windowRule>
  </windowRules>
</labwc_config>
XML
python3 -I "$PRUEFUNG/testbaum.py" "$PROFIL/heim" "$PROFIL/config" "$PROFIL/data" > "$OUT/testbaum.log"
case $SPRACHE in de) LOC=de_DE.UTF-8; LANGU=de ;; *) LOC=en_US.UTF-8; LANGU=en ;; esac
SSHD=(); [ "$STUFE" = G ] && SSHD=(--sshd)
EXTRA=()
[ -n "${LAUF_SSH_HOSTKEY_FRAGE:-}" ] && EXTRA+=(--setenv "QD_SSH_HOSTKEY_FRAGE=1")
[ -n "${LAUF_SSH_OHNE_SCHLUESSEL:-}" ] && EXTRA+=(--setenv "QD_SSH_OHNE_SCHLUESSEL=1")
[ -n "${LAUF_ZEITGRENZE:-}" ] && EXTRA+=(--setenv "QUERENCIA_DATEIEN_NETZ_ZEITGRENZE=$LAUF_ZEITGRENZE")
set +e
bash "$REPO/werkzeug/dateien-pruefrahmen/huelle.sh" "${SSHD[@]}" --profil "$PROFIL" --labwc --sprache "$LOC" \
  --lesen "$BIN" --lesen "$A/bin" --lesen "$PRUEFUNG" --lesen "$HIER" --lesen "$SZENARIO" --schreiben "$OUT" \
  --setenv "LANGUAGE=$LANGU" --setenv "QUERENCIA_DATEIEN_LOCALEDIR=$PROFIL/locale" \
  --setenv "LAUF_GROESSE=$GROESSE" --setenv "LAUF_OUT=$OUT" --setenv "LAUF_PROG=$BIN" \
  --setenv "LAUF_QE=$A/bin/qeingabe" --setenv "LAUF_QA=$A/bin/qausgabe" --setenv "LAUF_DUNKEL=$DUNKEL" \
  --setenv "LAUF_SPRACHE=$SPRACHE" --setenv "LAUF_STUFE=$STUFE" --setenv "LAUF_NETZ=$HIER" \
  --setenv "PATH=$PROFIL/heim/.local/bin:/usr/local/bin:/usr/bin:/bin" \
  "${EXTRA[@]}" --zeitlimit "${LAUF_ZEITLIMIT:-900}" --protokoll "$OUT/huelle.json" \
  "$STUFE" -- bash "$PRUEFUNG/innen.sh" "$SZENARIO" > "$OUT/huelle.log" 2>&1
STATUS=$?
set -e
cp -f "$PROFIL/labwc.log" "$OUT/" 2>/dev/null || true
for f in sshd.log huelle.log ssh-client.log known_hosts; do
  [ -f "$PROFIL/ssh/$f" ] && cp -f "$PROFIL/ssh/$f" "$OUT/ssh-$f"
done
echo "lauf $NAME: Hülle $STATUS"
[ -f "$OUT/ergebnis.txt" ] && awk -F'|' '{print $3}' "$OUT/ergebnis.txt" | sort | uniq -c
tail -n 2 "$OUT/huelle.log"
exit $STATUS
