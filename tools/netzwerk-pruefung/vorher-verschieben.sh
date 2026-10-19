#!/bin/bash
# G2-M Teil 5/6: vor dem Panelstart im Testprofil. Ein Starter für
# w2m-symbol.desktop, dessen Symbol w2m-neu es noch nicht gibt; der
# Symbolordner hicolor/scalable/apps besteht schon (der Fall von SW-6:
# neue Datei in einem bestehenden Ordner). Das Szenario legt das Symbol
# später ab (run … w2m-neu.svg).
set -eu
case "$XDG_DATA_HOME" in */nachweis/gui/*/data) ;; *) echo "nur im Testprofil"; exit 2 ;; esac
mkdir -p "$XDG_DATA_HOME/applications" "$XDG_DATA_HOME/icons/hicolor/scalable/apps"
printf '[Desktop Entry]\nType=Application\nName=Symboltest\nExec=true\nIcon=w2m-neu\nCategories=Utility;\n' >"$XDG_DATA_HOME/applications/w2m-symbol.desktop"
