#!/bin/bash
# G2-M: legt in der isolierten Prüfung einen Desktop-Eintrag
# org.querencia.Einstellungen.desktop mit der Aktion „netzwerk“ an (so, wie
# G2-O ihn liefern soll). Gestartet wird nichts: Exec schreibt nur seine
# Argumente in eine Datei.
#   einstellungen-attrappe.sh <applications-ordner> <protokolldatei>
set -eu
case "$1" in */nachweis/gui/*/data/applications) ;; *) echo "nur im Testprofil"; exit 2 ;; esac
HIER="$(cd "$(dirname "$0")" && pwd)"
cat >"$1/org.querencia.Einstellungen.desktop" <<EOD
[Desktop Entry]
Type=Application
Name=Systemeinstellungen
Exec=$HIER/einstellungen-start.sh $2
Icon=preferences-system
Categories=Settings;
Actions=netzwerk;

[Desktop Action netzwerk]
Name=Netzwerk
Exec=$HIER/einstellungen-start.sh $2 --netzwerk
EOD
