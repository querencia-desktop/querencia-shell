#!/bin/bash
# Querencia Dateien, Welle C2 (Miniaturen und Suche): isolierter GUI-Lauf im
# Prüfrahmen Stufe L (headless labwc 0.9.8, Wegwerf-HOME, eigener Bus und
# XDG_RUNTIME_DIR, kein Netz). Abgeleitet von ../pruefung/lauf.sh (B1);
# zusätzlich Testmaterial (material.sh), ein PDF-Thumbnailer-Stellvertreter (läuft
# in der Hülle nicht: dort gibt es kein bwrap; Sandkasten-Test in erzeuger.rs),
# eine vorbelegte dateien.ini und QUERENCIA_DATEIEN_DEBUG=1.
#
#   lauf.sh NAME SZENARIO.sh
#
# Umgebung wie ../pruefung/lauf.sh (LAUF_GROESSE, LAUF_DUNKEL, LAUF_SPRACHE,
# LAUF_BIN, LAUF_THEMA, LAUF_STUFE, LAUF_ZEITLIMIT), dazu
#   LAUF_INI      Zeilen für ~/.config/querencia/dateien.ini (z. B. "[Vorschau]\nBilder=nie")
#   LAUF_OHNE_PDF 1: kein PDF-Thumbnailer im Profil (Stand der VM)
# Im Profil liegt eine Attrappe `querencia-dateisuche` (schreibt ihre Argumente
# nach ~/dateisuche-aufruf.txt).
# Ausgabe: ${DATEIEN_ARBEIT:-~/projects/dateien-miniaturen-arbeit}/laeufe/NAME/
set -euo pipefail
NAME=$1; SZENARIO=$(readlink -f "$2")
HIER=$(cd "$(dirname "$(readlink -f "$0")")" && pwd)
PRUEF=$(cd "$HIER/../pruefung" && pwd)
CRATE=$(cd "$HIER/../.." && pwd); REPO=$(cd "$CRATE/../.." && pwd)
A=${DATEIEN_ARBEIT:-$HOME/projects/dateien-miniaturen-arbeit}
BIN=${LAUF_BIN:-$REPO/target/debug/querencia-dateien}
GROESSE=${LAUF_GROESSE:-1024x768}; DUNKEL=${LAUF_DUNKEL:-0}; SPRACHE=${LAUF_SPRACHE:-de}
THEMA=${LAUF_THEMA:-$HOME/.local/share/themes/Querencia-Klassisch}
MAT=$A/material
[ -x "$BIN" ] || { echo "lauf: Programm fehlt: $BIN" >&2; exit 1; }
[ -x "$A/bin/qeingabe" ] || { echo "lauf: erst DATEIEN_ARBEIT=$A ../pruefung/bauen-eingabe.sh" >&2; exit 1; }
[ -d "$MAT/Suche" ] || { echo "lauf: erst material.sh" >&2; exit 1; }
OUT=$A/laeufe/$NAME; PROFIL=$A/profile/$NAME
[ -d "$PROFIL" ] && chmod -R u+w "$PROFIL"
rm -rf "$OUT" "$PROFIL"; mkdir -p "$OUT/fotos" "$PROFIL"
: > "$PROFIL/.qd-profil"
mkdir -p "$PROFIL/heim" "$PROFIL/config/glib-2.0/settings" "$PROFIL/config/labwc" "$PROFIL/data/themes" \
  "$PROFIL/data/icons/hicolor/scalable/apps" "$PROFIL/locale" "$PROFIL/config/querencia"
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
python3 -I "$PRUEF/testbaum.py" "$PROFIL/heim" "$PROFIL/config" "$PROFIL/data" > "$OUT/testbaum.log"
# Testmaterial (Welle C2)
cp -a "$MAT/Bilder/." "$PROFIL/heim/Bilder/"
mkdir -p "$PROFIL/heim/Dokumente/PDF"; cp -a "$MAT/Dokumente/PDF/." "$PROFIL/heim/Dokumente/PDF/"
cp -a "$MAT/Suche" "$PROFIL/heim/"
if [ "${LAUF_OHNE_PDF:-0}" != 1 ]; then
  mkdir -p "$PROFIL/heim/.local/bin" "$PROFIL/data/thumbnailers"
  cp "$HIER/qd-pdf-miniatur" "$PROFIL/heim/.local/bin/"
  printf '[Thumbnailer Entry]\nTryExec=qd-pdf-miniatur\nExec=qd-pdf-miniatur %%i %%o %%s\nMimeType=application/pdf;application/x-pdf;\n' \
    > "$PROFIL/data/thumbnailers/qd-pdf.thumbnailer"
fi
# Attrappe von Querencia Dateisuche („Erweiterte Suche …“): schreibt ihre Argumente
mkdir -p "$PROFIL/heim/.local/bin"
printf '#!/bin/sh\nprintf "%%s\\n" "$@" > "$HOME/dateisuche-aufruf.txt"\n' > "$PROFIL/heim/.local/bin/querencia-dateisuche"
chmod +x "$PROFIL/heim/.local/bin/querencia-dateisuche"
W=${GROESSE%x*}; H=${GROESSE#*x}
{
  printf '[Fenster]\nBreite=%s\nHoehe=%s\n' "$((W > 1100 ? 1100 : W - 40))" "$((H > 800 ? 760 : H - 80))"
  printf '%b\n' "${LAUF_INI:-}"
} > "$PROFIL/config/querencia/dateien.ini"
find "$PROFIL/heim" | wc -l | sed 's/^/Einträge im Testbaum: /' >> "$OUT/testbaum.log"
case $SPRACHE in de) LOC=de_DE.UTF-8; LANGU=de ;; *) LOC=en_US.UTF-8; LANGU=en ;; esac
set +e
bash "$REPO/werkzeug/dateien-pruefrahmen/huelle.sh" --profil "$PROFIL" --labwc --sprache "$LOC" \
  --lesen "$BIN" --lesen "$A/bin" --lesen "$PRUEF" --lesen "$HIER" --lesen "$SZENARIO" --schreiben "$OUT" \
  --setenv "LANGUAGE=$LANGU" --setenv "QUERENCIA_DATEIEN_LOCALEDIR=$PROFIL/locale" \
  --setenv "QUERENCIA_DATEIEN_DEBUG=1" \
  --setenv "LAUF_GROESSE=$GROESSE" --setenv "LAUF_OUT=$OUT" --setenv "LAUF_PROG=$BIN" \
  --setenv "LAUF_QE=$A/bin/qeingabe" --setenv "LAUF_QA=$A/bin/qausgabe" --setenv "LAUF_DUNKEL=$DUNKEL" \
  --setenv "LAUF_SPRACHE=$SPRACHE" --setenv "LAUF_STUFE=${LAUF_STUFE:-L}" --setenv "LAUF_HIER=$HIER" \
  --setenv "PATH=$PROFIL/heim/.local/bin:/usr/local/bin:/usr/bin:/bin" \
  --zeitlimit "${LAUF_ZEITLIMIT:-600}" --protokoll "$OUT/huelle.json" \
  "${LAUF_STUFE:-L}" -- bash "$PRUEF/innen.sh" "$SZENARIO" > "$OUT/huelle.log" 2>&1
STATUS=$?
set -e
cp -f "$PROFIL/labwc.log" "$OUT/" 2>/dev/null || true
echo "lauf $NAME: Hülle $STATUS"
[ -f "$OUT/ergebnis.txt" ] && awk -F'|' '{print $3}' "$OUT/ergebnis.txt" | sort | uniq -c
tail -n 2 "$OUT/huelle.log"
exit $STATUS
