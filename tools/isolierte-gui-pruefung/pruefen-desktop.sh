#!/bin/bash
# HMP-09: isolierte Prüfung der Desktop-Symbole und des Desktop-Kontextmenüs
# (querencia-desktop) mit Panel, echten virtuellen Eingaben und eigenem
# Arbeitsflächen- und Vorlagenordner im Testprofil (der echte ~/ bleibt unberührt).
#   pruefen-desktop.sh <desktop-binary> <panel-binary> <profil> [szenario]
set -euo pipefail
HIER="$(cd "$(dirname "$0")" && pwd)"
DESKTOP="${1:?Desktop-Binary}"; PANEL="${2:?Panel-Binary}"; PROFIL="${3:?Profilname}"
SZQ="${4:-$HIER/szenario-hmp09.txt}"
G="${HMP_GUI_TEST:-$HOME/projects/querencia-panel-werkstatt/gui-test-nacht}"
bash "$HIER/bauen.sh" "$G/bin" >/dev/null
. "$HIER/testprofil.sh"; testprofil_neu "$G" "$PROFIL"
cd "$G"; mkdir -p "$PROFIL/logs" "$PROFIL/config" "$PROFIL/apps"
P="$G/$PROFIL"
# Eigene XDG-Benutzerordner im Profil (absolute Pfade sind in user-dirs.dirs erlaubt)
mkdir -p "$P/schreibtisch/Apfel" "$P/vorlagen"
printf 'unverändert\n' > "$P/schreibtisch/Zebra.txt"
printf 'Vorlage Brief\n' > "$P/vorlagen/Brief.txt"
printf 'XDG_DESKTOP_DIR="%s"\nXDG_TEMPLATES_DIR="%s"\n' "$P/schreibtisch" "$P/vorlagen" > "$P/config/user-dirs.dirs"
# HMP-10a: HMP10_RASTER=aus schaltet das Raster vorab aus (HMP-09/10-Pixellagen)
if [ "${HMP10_RASTER:-}" = aus ]; then
  mkdir -p "$P/state/querencia"; printf 'version\t1\neinstellung\traster\tnein\n' > "$P/state/querencia/desktop-symbole"
fi
# Programme wie in der Live-Sitzung: Querencia Files QF-0f, Querencia Terminal
cat > "$P/apps/io.github.lgse.Strata.desktop" <<D
[Desktop Entry]
Type=Application
Name=Querencia Files
Exec=/home/user/projects/querencia-files-werkstatt/qf-0f/live/strata %U
Icon=system-file-manager
MimeType=inode/directory;
D
cp "$HIER/../../session/querencia/applications/org.example.Querencia.Terminal.desktop" "$P/apps/"
# Testbild für „Hintergrund ändern“: grün oben, rot unten (eindeutig erkennbar)
python3 -c "from PIL import Image; im=Image.new('RGB',(320,180),(40,160,60)); im.paste((200,40,40),(0,90,320,180)); im.save('$P/bild-test.png')"
SZ="$P/logs/szenario-hmp09.txt"
sed "s#QDESKTOP#$DESKTOP#g; s#QBILD#$P/bild-test.png#g" "$SZQ" > "$SZ"
HMP_PANEL="$PANEL" HMP_SZENARIO="$SZ" HMP_QEINGABE="$G/bin/qeingabe" HMP_APPS="$P/apps" \
  /usr/bin/python3 "$HIER/isoliert.py" "$PROFIL" "$HIER/szenario.sh" "${HMP_ZEIT:-180}"
{ echo "--- Schreibtisch danach"; (cd "$P/schreibtisch" && find . | sort && cat Zebra.txt && sha256sum Zebra.txt);
  echo "--- desktop-order"; cat "$P/state/querencia/desktop-order" 2>/dev/null;
  echo "--- desktop.ini"; cat "$P/config/querencia/desktop.ini" 2>/dev/null || echo "(keine)"; } > "$P/logs/dateien.txt"
cat "$P/logs/dateien.txt"
