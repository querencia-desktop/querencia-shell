#!/bin/bash
# Querencia Dateien: isolierter GUI-Lauf im Prüfrahmen (Stufe L, headless
# labwc 0.9.8, Wegwerf-HOME, eigener Bus und XDG_RUNTIME_DIR, kein Netz).
#
#   lauf.sh NAME SZENARIO.sh
#
# Umgebung: LAUF_GROESSE (1024x768), LAUF_DUNKEL (0|1), LAUF_SPRACHE (de|en),
#   LAUF_BIN (Programm, Vorgabe target/debug/querencia-dateien des Worktrees),
#   LAUF_FILES_TOML (Files-settings.toml für M6), LAUF_THEMA (Ordner des
#   GTK-Themas, Vorgabe ~/.local/share/themes/Querencia-Klassisch).
# Ausgabe: ${DATEIEN_ARBEIT:-~/projects/dateien-arbeit}/laeufe/NAME/
#   (ergebnis.txt, programm.log, fotos/, aufrufe.log, huelle.log).
# Das Szenario läuft in der Hülle mit den Hilfen aus innen.sh.
set -euo pipefail
NAME=$1; SZENARIO=$(readlink -f "$2")
HIER=$(cd "$(dirname "$(readlink -f "$0")")" && pwd)
CRATE=$(cd "$HIER/../.." && pwd); REPO=$(cd "$CRATE/../.." && pwd)
A=${DATEIEN_ARBEIT:-$HOME/projects/dateien-arbeit}
BIN=${LAUF_BIN:-$REPO/target/debug/querencia-dateien}
GROESSE=${LAUF_GROESSE:-1024x768}; DUNKEL=${LAUF_DUNKEL:-0}; SPRACHE=${LAUF_SPRACHE:-de}
THEMA=${LAUF_THEMA:-$HOME/.local/share/themes/Querencia-Klassisch}
[ -x "$BIN" ] || { echo "lauf: Programm fehlt: $BIN" >&2; exit 1; }
[ -x "$A/bin/qeingabe" ] || { echo "lauf: erst werkzeug/pruefung/bauen-eingabe.sh" >&2; exit 1; }
OUT=$A/laeufe/$NAME; PROFIL=$A/profile/$NAME
[ -d "$PROFIL" ] && chmod -R u+w "$PROFIL"
rm -rf "$OUT" "$PROFIL"; mkdir -p "$OUT/fotos" "$PROFIL"
: > "$PROFIL/.qd-profil"
mkdir -p "$PROFIL/heim" "$PROFIL/config/glib-2.0/settings" "$PROFIL/config/labwc" "$PROFIL/data/themes" \
  "$PROFIL/data/icons/hicolor/scalable/apps" "$PROFIL/locale"
# Kataloge wie in der Ablage: Programm und beide Bibliotheken
for c in querencia-dateien querencia-oberflaeche querencia-dateiarbeit; do
  (cd "$REPO/crates/$c" && python3 -I werkzeug/i18n.py mo "$PROFIL/locale" >/dev/null)
done
cp -rL --no-preserve=mode "$THEMA" "$PROFIL/data/themes/"
[ -d "$HOME/.local/share/icons/Querencia" ] && cp -rL --no-preserve=mode "$HOME/.local/share/icons/Querencia" "$PROFIL/data/icons/"
cp "$CRATE/data/icons/org.querencia.Dateien.svg" "$PROFIL/data/icons/hicolor/scalable/apps/"
SCHEMA=default; [ "$DUNKEL" = 1 ] && SCHEMA=prefer-dark
{
  printf '[org/gnome/system/wsdd]\ndisplay-mode=%s\n\n' "'disabled'"
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
  </windowRules>
</labwc_config>
XML
TOML=()
[ -n "${LAUF_FILES_TOML:-}" ] && TOML=(--files-toml "$LAUF_FILES_TOML")
python3 -I "$HIER/testbaum.py" "$PROFIL/heim" "$PROFIL/config" "$PROFIL/data" "${TOML[@]}" > "$OUT/testbaum.log"
case $SPRACHE in de) LOC=de_DE.UTF-8; LANGU=de ;; *) LOC=en_US.UTF-8; LANGU=en ;; esac
set +e
bash "$REPO/werkzeug/dateien-pruefrahmen/huelle.sh" --profil "$PROFIL" --labwc --sprache "$LOC" \
  --lesen "$BIN" --lesen "$A/bin" --lesen "$HIER" --lesen "$SZENARIO" --schreiben "$OUT" \
  --setenv "LANGUAGE=$LANGU" --setenv "QUERENCIA_DATEIEN_LOCALEDIR=$PROFIL/locale" \
  --setenv "LAUF_GROESSE=$GROESSE" --setenv "LAUF_OUT=$OUT" --setenv "LAUF_PROG=$BIN" \
  --setenv "LAUF_QE=$A/bin/qeingabe" --setenv "LAUF_QA=$A/bin/qausgabe" --setenv "LAUF_DUNKEL=$DUNKEL" \
  --setenv "LAUF_SPRACHE=$SPRACHE" --setenv "PATH=$PROFIL/heim/.local/bin:/usr/local/bin:/usr/bin:/bin" \
  --zeitlimit "${LAUF_ZEITLIMIT:-600}" --protokoll "$OUT/huelle.json" \
  L -- bash "$HIER/innen.sh" "$SZENARIO" > "$OUT/huelle.log" 2>&1
STATUS=$?
set -e
cp -f "$PROFIL/heim/aufrufe.log" "$OUT/" 2>/dev/null || true
cp -f "$PROFIL/labwc.log" "$OUT/" 2>/dev/null || true
echo "lauf $NAME: Hülle $STATUS"
[ -f "$OUT/ergebnis.txt" ] && awk -F'|' '{print $3}' "$OUT/ergebnis.txt" | sort | uniq -c
tail -n 2 "$OUT/huelle.log"
exit $STATUS
