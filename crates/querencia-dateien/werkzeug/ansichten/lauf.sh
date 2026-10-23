#!/bin/bash
# Querencia Dateien, Ansichten (Worker B2): ein isolierter GUI-Lauf in der
# Hülle des Prüfrahmens, Stufe L (werkzeug/dateien-pruefrahmen/huelle.sh:
# env -i, bwrap mit eigenem PID-/Netz-Namensraum, privater Bus, echtes HOME
# und /run unsichtbar, headless labwc 0.9.8). Gerüst aus dem Prototyp
# dateien/prototyp-2 (werkzeug/lauf.sh), dort aus Desktop 0.2.6-2.
#   bash werkzeug/ansichten/lauf.sh <tag> <ablauf>
# ablauf  erkunden  Start, ein Foto, Fokus- und Messzeilen (Eichung)
#         pruefung  Ansichtsfälle aus B §5.1 mit Log-Belegen (ohne Rahmen:
#                   Fensterkoordinaten = Bildschirmkoordinaten)
#         messung   10.020 Einträge: Öffnen und Umsortieren, je drei Mal
#         fotos     Belegfotos mit labwc-Rahmen wie live
#         dauer     Speicher beim Scrollen in „10000 Einträge“ (LAUF_MINUTEN)
# Umgebung: LAUF_GROESSE 1920x1080|1024x768, LAUF_DUNKEL 0|1.
# Programm: target/release/querencia-dateien des Worktrees (bauen.sh).
# Ergebnis: nachweis/ansichten/laeufe/<tag>/, Fotos nachweis/ansichten/fotos/<tag>/.
set -euo pipefail
W=$(cd "$(dirname "$(readlink -f "$0")")" && pwd)
K=$(cd "$W/../.." && pwd)
WT=$(cd "$K/../.." && pwd)
TAG=$1 ABLAUF=$2
case $TAG in '' | .* | */* | *[!A-Za-z0-9._-]*) echo "Tag?" >&2; exit 2 ;; esac
case $ABLAUF in erkunden | pruefung | messung | fotos | dauer) ;; *) echo "Ablauf?" >&2; exit 2 ;; esac
GROESSE=${LAUF_GROESSE:-1920x1080} DUNKEL=${LAUF_DUNKEL:-0} MINUTEN=${LAUF_MINUTEN:-5}
case $GROESSE in 1920x1080 | 1024x768) ;; *) echo "Größe?" >&2; exit 2 ;; esac
case $DUNKEL in 0 | 1) ;; *) echo "Schema?" >&2; exit 2 ;; esac
case $MINUTEN in *[!0-9]* | '') echo "Minuten?" >&2; exit 2 ;; esac
BIN=$WT/target/release/querencia-dateien
HUELLE=$WT/werkzeug/dateien-pruefrahmen/huelle.sh
TQ=$(readlink -f "$HOME/.local/share/themes/Querencia-Klassisch")
[ -x "$BIN" ] && [ -x "$HUELLE" ] && [ -f "$TQ/gtk-4.0/gtk.css" ] && [ -x "$W/bin/qeingabe" ] \
    || { echo "fehlt: $BIN, $HUELLE, $TQ oder bin/qeingabe (bauen-eingabe.sh)" >&2; exit 2; }
BASIS=$HOME/.cache/querencia-dateien-ansichten/pruefung
PROFIL=$BASIS/profil-$TAG
OUT=$K/nachweis/ansichten/laeufe/$TAG
FOTOS=$K/nachweis/ansichten/fotos/$TAG
case $PROFIL in "$HOME"/.cache/querencia-dateien-ansichten/pruefung/profil-*) ;; *) exit 2 ;; esac
if [ -d "$PROFIL" ]; then chmod -R u+w "$PROFIL"; rm -rf "$PROFIL"; fi
rm -rf "$OUT" "$FOTOS"
mkdir -p "$PROFIL/data/themes" "$PROFIL/data/icons" "$PROFIL/config/gtk-4.0" "$PROFIL/config/labwc" \
    "$PROFIL/heim" "$PROFIL/bin" "$OUT" "$FOTOS"
# ein Profil des Prüfrahmens (huelle.sh --profil nimmt nur leere oder markierte)
: >"$PROFIL/.qd-profil"
cp -rL "$TQ" "$PROFIL/data/themes/"
cp -rL "$HOME/.local/share/icons/Querencia" "$HOME/.local/share/icons/hicolor" "$PROFIL/data/icons/"
chmod -R u+w "$PROFIL/data"
# Testbaum im HOME des Profils (dort darf umbenannt werden); XDG-Ordner wie
# eine frische deutsche Sitzung, Vorlagen für „Dokument anlegen“
python3 -I "$W/beispielordner.py" "$PROFIL/heim/querencia-dateien-ansichten" >"$OUT/beispiel.txt"
mkdir -p "$PROFIL/heim/Vorlagen" "$PROFIL/heim/Schreibtisch" "$PROFIL/config"
printf 'Brief\n' >"$PROFIL/heim/Vorlagen/Brief.odt"
printf 'XDG_DESKTOP_DIR="$HOME/Schreibtisch"\nXDG_TEMPLATES_DIR="$HOME/Vorlagen"\n' >"$PROFIL/config/user-dirs.dirs"
# Kataloge (de) des Programms und der beiden Bibliotheken
for c in querencia-dateien querencia-oberflaeche querencia-dateiarbeit; do
    (cd "$WT/crates/$c" && python3 -I werkzeug/i18n.py mo "$PROFIL/locale" >/dev/null)
done
# Testprogramm für Textdateien (Prüfrahmen-Regel 10: eigene mimeapps.list,
# nichts Echtes startet): schreibt nur auf, was es öffnen soll
mkdir -p "$PROFIL/data/applications"
printf '#!/bin/sh\nfor f in "$@"; do printf "%%s\\n" "$f" >>"%s/geoeffnet.log"; done\n' "$OUT" >"$PROFIL/bin/qd-oeffnen"
chmod +x "$PROFIL/bin/qd-oeffnen"
printf '[Desktop Entry]\nType=Application\nName=Prüfprogramm\nExec=qd-oeffnen %%F\nMimeType=text/plain;\n' \
    >"$PROFIL/data/applications/qd-pruef.desktop"
printf '[Default Applications]\ntext/plain=qd-pruef.desktop\n' >"$PROFIL/config/mimeapps.list"
# xdg-open-Attrappe (nichts startet wirklich)
printf '#!/bin/sh\nprintf "%%s\\n" "$*" >>"%s/xdg-open.log"\n' "$OUT" >"$PROFIL/bin/xdg-open"
chmod +x "$PROFIL/bin/xdg-open"
SCHRIFT=$(gsettings get org.gnome.desktop.interface font-name | tr -d "'")
printf '[Settings]\ngtk-theme-name=Querencia-Klassisch\ngtk-icon-theme-name=Querencia\ngtk-font-name=%s\ngtk-enable-animations=0\ngtk-cursor-blink=0\n' \
    "$SCHRIFT" >"$PROFIL/config/gtk-4.0/settings.ini"
BX=${GROESSE%x*} BY=${GROESSE#*x}
case $ABLAUF in fotos) DEKO='' ;; *) DEKO='<action name="SetDecorations" decorations="none" />' ;; esac
cat >"$PROFIL/config/labwc/rc.xml" <<X
<?xml version="1.0"?>
<labwc_config>
  <core><decoration>server</decoration></core>
  <theme><name>Querencia-Klassisch</name><icon>Querencia</icon></theme>
  <windowRules>
    <windowRule identifier="org.querencia.Dateien">
      $DEKO
      <action name="Maximize" />
    </windowRule>
  </windowRules>
</labwc_config>
X
gsettings list-recursively org.gnome.desktop.interface | grep -v -E ' (color-scheme|enable-animations|cursor-blink) ' >"$PROFIL/portal-werte.txt"
printf 'org.gnome.desktop.interface enable-animations false\norg.gnome.desktop.interface cursor-blink false\n' >>"$PROFIL/portal-werte.txt"
cat >"$PROFIL/env.sh" <<X
TAG=$TAG
ABLAUF=$ABLAUF
BIN=$BIN
DUNKEL=$DUNKEL
BX=$BX
BY=$BY
MINUTEN=$MINUTEN
OUT=$OUT
FOTOS=$FOTOS
PBIN=$PROFIL/bin
BEISPIEL=$PROFIL/heim/querencia-dateien-ansichten/beispiel
W=$W
Q=$W/bin/qeingabe
QA=$W/bin/qausgabe
PE=$W/portal-ersatz.py
PW=$PROFIL/portal-werte.txt
X
# Kopie: Bash liest Skripte stückweise, eine Änderung im Lauf träfe sonst den Lauf
cp "$W/szenario.sh" "$PROFIL/szenario.sh"
GRENZE=$((MINUTEN * 60 + 900))
set +e
bash "$HUELLE" --profil "$PROFIL" --labwc --sprache de_DE.UTF-8 --zeitlimit "$GRENZE" \
    --setenv LANGUAGE=de --setenv GSK_RENDERER=cairo --setenv GDK_DISABLE=gl,vulkan \
    --lesen "$WT/target/release" --lesen "$W" --schreiben "$OUT" --schreiben "$FOTOS" \
    --protokoll "$OUT/huelle.txt" L -- /bin/bash "$PROFIL/szenario.sh"
RC=$?
set -e
echo "Hülle: Exit $RC" >>"$OUT/ergebnis.txt"
[ "$RC" = 95 ] && { echo "Reste – Profil $PROFIL bleibt" >&2; exit 95; }
chmod -R u+w "$PROFIL"; rm -rf "$PROFIL"
rmdir "$FOTOS" 2>/dev/null || true
grep -a -E '^INFO Ende' "$OUT/ergebnis.txt" || echo "kein Ende in ergebnis.txt"
exit "$RC"
