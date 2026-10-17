#!/bin/bash
# HMP-10: isolierte Prüfung von Ziehen und Ablegen, dauerhaften Positionen und
# Umbenennen der Desktop-Symbole (querencia-desktop) mit echtem Panel, echten
# virtuellen Eingaben und eigenem Arbeitsflächenordner im Testprofil.
#   pruefen-hmp10.sh <desktop-binary> <panel-binary> <profil>
# HMP-10a:
#   HMP10_TREIBER=<datei>  anderer Treiber (hmp10a-treiber.py)
#   HMP10_RASTER=aus       „Am Raster ausrichten“ vorab aus (HMP-10-Verhalten)
#   HMP10_ZUSTAND=<datei>  vorhandene Zustandsdatei desktop-symbole vorab
#   HMP10_ANMELDUNG2=1     danach zweite Anmeldung: neuer labwc, gleiches Profil
# Ergebnis: <testordner>/<profil>/logs/pruefung.txt und Bilder in logs/bilder/.
# Der echte ~/ bleibt unberührt: Desktop-Ordner, Vorlagen, Zustand, Papierkorb
# und Programme liegen im Profil; geöffnete „Programme“ sind Markerskripte.
set -euo pipefail
HIER="$(cd "$(dirname "$0")" && pwd)"
DESKTOP="${1:?Desktop-Binary}"; PANEL="${2:?Panel-Binary}"; PROFIL="${3:?Profilname}"
G="${HMP_GUI_TEST:?HMP_GUI_TEST: Testordner}"
bash "$HIER/bauen.sh" "$G/bin" >/dev/null
. "$HIER/testprofil.sh"; testprofil_neu "$G" "$PROFIL"
P="$G/$PROFIL"
mkdir -p "$P/logs/bilder" "$P/config/labwc" "$P/apps" "$P/bin" "$P/schreibtisch/Apfel" "$P/vorlagen"
printf 'unverändert\n' > "$P/schreibtisch/Zebra.txt"
printf 'Inhalt des Apfels\n' > "$P/schreibtisch/Apfel/kern.txt"
printf 'Vorlage Brief\n' > "$P/vorlagen/Brief.txt"
printf 'XDG_DESKTOP_DIR="%s"\nXDG_TEMPLATES_DIR="%s"\n' "$P/schreibtisch" "$P/vorlagen" > "$P/config/user-dirs.dirs"
# Markerprogramme: schreiben Name und Argumente in marker.log
for m in files terminal browser fremd; do
  printf '#!/bin/sh\necho "%s $*" >> "%s/marker.log"\n' "$m" "$P" > "$P/bin/marker-$m"
  chmod 755 "$P/bin/marker-$m"
done
cat > "$P/apps/io.github.lgse.Strata.desktop" <<D
[Desktop Entry]
Type=Application
Name=Querencia Files
Exec=$P/bin/marker-files %U
Icon=system-file-manager
MimeType=inode/directory;
D
sed -e "s#^Exec=.*#Exec=$P/bin/marker-terminal#" -e "s#^TryExec=.*#TryExec=$P/bin/marker-terminal#" \
  "$HIER/../../session/querencia/applications/org.example.Querencia.Terminal.desktop" \
  > "$P/apps/org.example.Querencia.Terminal.desktop"
printf '[Default Applications]\ninode/directory=io.github.lgse.Strata.desktop\n' > "$P/config/mimeapps.list"
# Benutzereigener Starter (ausführbar = vertrauenswürdig) und ein fremder
cat > "$P/schreibtisch/Mein Browser.desktop" <<D
[Desktop Entry]
# Teststarter HMP-10: alle Felder außer Name[de] müssen erhalten bleiben
Version=1.0
Type=Application
Name=My Browser
Name[de]=Mein Browser
Name[fr]=Mon navigateur
GenericName=Web Browser
Comment=Im Internet surfen
Exec=$P/bin/marker-browser %u
TryExec=$P/bin/marker-browser
Icon=web-browser
Terminal=false
MimeType=text/html;x-scheme-handler/http;
Categories=Network;WebBrowser;
X-Querencia-Test=bleibt

[Desktop Action new-window]
Name=Neues Fenster
Exec=$P/bin/marker-browser --new-window
D
chmod 755 "$P/schreibtisch/Mein Browser.desktop"
cat > "$P/schreibtisch/Fremd.desktop" <<D
[Desktop Entry]
Type=Application
Name=Fremder Starter
Exec=$P/bin/marker-fremd
Icon=dialog-warning
D
chmod 644 "$P/schreibtisch/Fremd.desktop"
# HMP-10a: vorhandener Zustand (Migration) bzw. Raster aus
mkdir -p "$P/state/querencia"
if [ -n "${HMP10_ZUSTAND:-}" ]; then cp "$HMP10_ZUSTAND" "$P/state/querencia/desktop-symbole"; fi
if [ "${HMP10_RASTER:-}" = aus ]; then printf 'version\t1\neinstellung\traster\tnein\n' > "$P/state/querencia/desktop-symbole"; fi
# Einfarbiger Hintergrund: Symbole sind per Pixel eindeutig erkennbar
python3 -c "from PIL import Image; Image.new('RGB',(64,64),(58,110,165)).save('$P/hintergrund.png')"
mkdir -p "$P/config/querencia"
printf '[Background]\nFile=%s\nMode=stretched\nColor=#3a6ea5\n' "$P/hintergrund.png" > "$P/config/querencia/desktop.ini"
# Vier Arbeitsflächen wie live, Strg+Alt+←/→ zum Wechseln
cat > "$P/config/labwc/rc.xml" <<'X'
<?xml version="1.0"?>
<labwc_config>
  <core><decoration>server</decoration></core>
  <theme><name>Clearlooks</name></theme>
  <desktops number="4"><popupTime>0</popupTime></desktops>
  <keyboard>
    <default />
    <keybind key="C-A-Right"><action name="GoToDesktop" to="right" wrap="yes" /></keybind>
    <keybind key="C-A-Left"><action name="GoToDesktop" to="left" wrap="yes" /></keybind>
  </keyboard>
</labwc_config>
X
cat > "$P/logs/szenario-hmp10.sh" <<S
#!/bin/bash
# läuft in der isolierten Sitzung (isoliert.py)
mkdir -p "\$XDG_DATA_HOME/applications"; cp "$P/apps"/*.desktop "\$XDG_DATA_HOME/applications/"
update-desktop-database "\$XDG_DATA_HOME/applications"
HMP10_DESKTOP="$DESKTOP" HMP_PANEL="$PANEL" HMP_QEINGABE="$G/bin/qeingabe" HMP_QAUSGABE="$G/bin/qausgabe" \\
  HMP10_NUR="\${HMP10_NUR:-}" HMP10_PHASE="\${HMP10_PHASE:-1}" python3 "${HMP10_TREIBER:-$HIER/hmp10-treiber.py}" >> "$P/logs/treiber.txt" 2>&1
touch "$P/logs/FERTIG"
sleep 30
S
HMP10_PHASE=1 /usr/bin/python3 "$HIER/isoliert.py" "$P" "$P/logs/szenario-hmp10.sh" "${HMP_ZEIT:-900}"
cat "$P/logs/pruefung.txt"
if [ "${HMP10_ANMELDUNG2:-}" = 1 ]; then
  # erneute Anmeldung: neuer Compositor, neues D-Bus, gleiches Profil
  HMP10_PHASE=2 /usr/bin/python3 "$HIER/isoliert.py" "$P" "$P/logs/szenario-hmp10.sh" 300
  cat "$P/logs/pruefung-phase2.txt"
fi
