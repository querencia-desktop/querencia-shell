#!/bin/bash
# G2-R: Belegbilder Alt+Tab und Super+? in einer isolierten headless labwc
# 0.9.8 (isoliert.py: eigene Runtime, eigener D-Bus, eigenes XDG-Profil;
# qeingabe läuft nur dort). Die echte Sitzung wird nie berührt.
#
#   bash werkzeug/osd/osd-bilder.sh <variante> [desktop-binary]
#     vorher     rc.xml und Theme wie live (main 127b74e): labwc-Standard
#     klassisch  neue rc.xml + Theme (Stil classic, Vorschlag)
#     miniatur   wie klassisch, aber <osd style="thumbnail">
#     alle       wie klassisch, Alt+Tab über alle Arbeitsflächen, Editor auf 2
#     kuerzel    Super+Umschalt+ß (= Super+?) startet querencia-kuerzel
# Drei Testfenster (foot mit App-ID und Titel von Terminal, Files, Editor),
# echtes Panel (Login-Link) und Desktop. Ergebnis: Profil
# ~/.cache/g2r/gui-test/osd-<variante>/logs (Bilder, Protokolle, ergebnis.txt).
set -euo pipefail
WT="$(cd "$(dirname "$0")/../.." && pwd)"
T=$WT/tools/isolierte-gui-pruefung
V=${1:?Variante}
DESKTOP=${2:-$WT/target/debug/querencia-desktop}
KUERZEL=$WT/target/debug/querencia-kuerzel
PANEL=/home/user/opt/querencia/bin/querencia-panel
G=/home/user/.cache/g2r/gui-test
BASIS=127b74e
mkdir -p "$G"
. "$T/testprofil.sh"; NAME=osd-$V${OSD_ICON:+-icon-${OSD_ICON//[^A-Za-z0-9]/_}}
testprofil_neu "$G" "$NAME"
P=$G/$NAME
bash "$T/bauen.sh" "$G/bin" >/dev/null
mkdir -p "$P/config/labwc" "$P/data/applications" "$P/data/themes" "$P/data/icons/hicolor/scalable/apps" "$P/logs" "$P/bin"

# Theme und rc.xml
if [ "$V" = vorher ]; then
  git -C "$WT" archive "$BASIS" data/themes/Querencia-Klassisch | tar -x -C "$P" --strip-components=1 -f - 2>/dev/null ||
    { mkdir -p "$P/tmp"; git -C "$WT" archive "$BASIS" data/themes/Querencia-Klassisch | tar -x -C "$P/tmp"; mv "$P/tmp/data/themes/Querencia-Klassisch" "$P/data/themes/"; }
  [ -d "$P/themes" ] && mv "$P/themes/Querencia-Klassisch" "$P/data/themes/" && rmdir "$P/themes"
  git -C "$WT" show "$BASIS:session/querencia/labwc/rc.xml" > "$P/config/labwc/rc.xml"
else
  cp -r "$WT/data/themes/Querencia-Klassisch" "$P/data/themes/"
  cp "$WT/session/querencia/labwc/rc.xml" "$P/config/labwc/rc.xml"
fi
cp "$WT/session/querencia/labwc/environment" "$P/config/labwc/environment"
# OSD_ICON=<thema>: Symbolthema für labwc (<theme><icon>) zum Vergleich;
# OSD_ICON=- entfernt einen vorhandenen <icon>-Eintrag
if [ -n "${OSD_ICON:-}" ]; then
  sed -i '/<icon>.*<\/icon>/d' "$P/config/labwc/rc.xml"
  [ "$OSD_ICON" = - ] || sed -i "0,/<name>Querencia-Klassisch<\/name>/s##<name>Querencia-Klassisch</name>\n    <icon>$OSD_ICON</icon>#" "$P/config/labwc/rc.xml"
fi
case $V in
  miniatur) sed -i 's/style="classic"/style="thumbnail"/' "$P/config/labwc/rc.xml" ;;
  alle) python3 - "$P/config/labwc/rc.xml" <<'PY'
import sys
p = sys.argv[1]; s = open(p).read()
s = s.replace("  <keyboard>\n    <default />\n", """  <keyboard>
    <default />
    <keybind key="A-Tab"><action name="NextWindow" workspace="all" /></keybind>
    <keybind key="A-S-Tab"><action name="PreviousWindow" workspace="all" /></keybind>
""", 1)
s = s.replace("  <windowRules>\n", """  <windowRules>
    <windowRule identifier="org.querencia.Editor">
      <action name="SendToDesktop" to="2" follow="no" />
    </windowRule>
""", 1)
open(p, "w").write(s)
PY
  ;;
  kuerzel)
    printf '#!/bin/sh\necho "gestartet $$ $*" >> "%s/logs/kuerzel-marker.txt"\nexec "%s" "$@" 2>> "%s/logs/kuerzel.log"\n' "$P" "$KUERZEL" "$P" > "$P/bin/querencia-kuerzel"
    chmod 755 "$P/bin/querencia-kuerzel"
    sed -i "s#command=\"/home/user/opt/querencia/bin/querencia-kuerzel\"#command=\"$P/bin/querencia-kuerzel\"#" "$P/config/labwc/rc.xml"
    grep -q "$P/bin/querencia-kuerzel" "$P/config/labwc/rc.xml" || { echo "Super+?-Eintrag fehlt in rc.xml" >&2; exit 1; }
    ;;
esac
# Starter der Testfenster (nur Name und Symbol werden gebraucht) und Symbole
for f in org.example.Querencia.Terminal io.github.lgse.Strata org.querencia.Editor; do
  sed -e 's#^Exec=.*#Exec=/bin/true#' -e 's#^TryExec=.*#TryExec=/bin/true#' \
    "/home/user/.local/share/applications/$f.desktop" > "$P/data/applications/$f.desktop"
done
# Symbole des Editors wie live (PNG ohne 48x48 und SVG; labwc 0.9.8 ist
# ohne librsvg gebaut und zeigt nur PNG)
for d in /home/user/.local/share/icons/hicolor/*/apps; do
  [ -e "$d/org.querencia.Editor.png" ] || [ -e "$d/org.querencia.Editor.svg" ] || continue
  mkdir -p "$P/data/icons/hicolor/$(basename "$(dirname "$d")")/apps"
  cp "$d"/org.querencia.Editor.* "$P/data/icons/hicolor/$(basename "$(dirname "$d")")/apps/"
done
cp "$WT/crates/querencia-kuerzel/data/org.querencia.Kuerzel.svg" "$P/data/icons/hicolor/scalable/apps/"
sed -e "s#^Exec=.*#Exec=$P/bin/querencia-kuerzel#" -e "s#^TryExec=.*#TryExec=$P/bin/querencia-kuerzel#" \
  "$WT/crates/querencia-kuerzel/data/org.querencia.Kuerzel.desktop" > "$P/data/applications/org.querencia.Kuerzel.desktop"

Q=$G/bin/qeingabe
L=$P/logs
cat > "$L/alt-tab.txt" <<S
move 640 690
wait 500
keydown Alt_L
key Tab
wait 500
key Tab
wait 800
shot $L/alt-tab.png
keyup Alt_L
wait 700
shot $L/nach-alt-tab.png
S
cat > "$L/super-fragezeichen.txt" <<S
move 640 690
wait 300
keydown Super_L
keydown Shift_L
keydown ssharp
wait 80
keyup ssharp
keyup Shift_L
keyup Super_L
wait 3500
shot $L/kuerzel-arbeitsflaeche.png
run sh -c 'tail -1 "$L/kuerzel-marker.txt" | cut -d" " -f2 > "$L/kuerzel-pid.txt"; for p in \$(cat "$L/kuerzel-pid.txt"); do [ -d /proc/\$p ] && echo kuerzel-laeuft=ja || echo kuerzel-laeuft=nein; done >> "$L/ergebnis.txt"'
wait 500
key Escape
wait 1500
shot $L/kuerzel-nach-escape.png
S
cat > "$L/szenario.sh" <<S
#!/bin/bash
# läuft in der isolierten Sitzung (isoliert.py)
cd "$P"
update-desktop-database "\$XDG_DATA_HOME/applications" 2>/dev/null || true
echo "LABWC_PID=\$LABWC_PID labwc=\$(readlink /proc/\$LABWC_PID/exe) \$(tr '\0' ' ' < /proc/\$LABWC_PID/cmdline)" > "$L/umgebung.txt"
"$PANEL" > "$L/panel.log" 2>&1 &
"$DESKTOP" > "$L/desktop.log" 2>&1 &
sleep 3
foot -a org.example.Querencia.Terminal -T "user@testrechner: ~" > /dev/null 2>&1 &
sleep 0.7
foot -a io.github.lgse.Strata -T "Persönlicher Ordner – Querencia Files" > /dev/null 2>&1 &
sleep 0.7
foot -a org.querencia.Editor -T "notizen.md – Texteditor" > /dev/null 2>&1 &
sleep 2.5
grim "$L/drei-fenster.png"
if [ "$V" = kuerzel ]; then
  # ein qeingabe-Lauf: Super+?, Bild, Kontrolle, Esc (ein zweites
  # virtuelles Keyboard verliert seine erste Taste)
  "$Q" < "$L/super-fragezeichen.txt" > "$L/qeingabe-1.log" 2>&1
  PID=\$(cat "$L/kuerzel-pid.txt" 2>/dev/null)
  echo "kuerzel-pid=\$PID" >> "$L/ergebnis.txt"
  sleep 0.5
  [ -n "\$PID" ] && [ ! -d /proc/\$PID ] && echo "kuerzel-nach-esc-beendet=ja" >> "$L/ergebnis.txt" || echo "kuerzel-nach-esc-beendet=nein" >> "$L/ergebnis.txt"
  "$P/bin/querencia-kuerzel" --seite programme &
  sleep 3.5
  grim "$L/kuerzel-programme.png"
  "$KUERZEL" --liste > "$L/kuerzel-liste.txt" 2>&1
else
  "$Q" < "$L/alt-tab.txt" > "$L/qeingabe-1.log" 2>&1
fi
touch "$L/FERTIG"
sleep 30
S
HMP_RUNTIME_BASIS=/home/user/.cache/g2r/rt /usr/bin/python3 "$T/isoliert.py" "$P" "$L/szenario.sh" 150
ls "$L"
