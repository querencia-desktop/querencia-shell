# D21: Zwischenablage in beide Richtungen mit Querencia Files und Desktop 0.3.0 (isoliert).
source "$(dirname "$QD_SZ")/hilfen.sh"
S=$HOME/Schreibtisch; D=$HOME/Dokumente; T=$HOME/Test; mkdir -p "$S" "$D" "$T/ordner"
printf 'XDG_DESKTOP_DIR="$HOME/Schreibtisch"\nXDG_DOCUMENTS_DIR="$HOME/Dokumente"\n' > "$XDG_CONFIG_HOME/user-dirs.dirs"
echo d > "$S/vom-desktop.txt"; echo f > "$D/aus-files.txt"
echo z1 > "$T/zum-desktop.txt"; echo z2 > "$T/zu-files.txt"
typen() { wl-paste --list-types 2>&1 | tr '\n' ' '; }
"$QD_FILES" "$D" > "$QD_AUS/files.log" 2>&1 &
FPID=$!
eingabe <<< "wait 5000"
klick 313 210; strg c
echo "Typen nach Strg+C in Files: $(typen)" >> "$QD_AUS/innen.log"
start_app "$T"
# ordner, zu-files, zum-desktop
zklick 1; strg v
warte_bis '[ -f "$T/aus-files.txt" ]' 10
pruefe C1 "Strg+C in Files, Strg+V in Dateien" "kopiert, Original bleibt" '[ "$(cat "$T/aus-files.txt")" = f ] && [ -f "$D/aus-files.txt" ]' "$(grep -a 'Einfügen (' "$LOG" | tail -1 | cut -c1-120)"
eingabe <<< "wait 1000"
# ordner, aus-files, zu-files, zum-desktop (Ordner zuerst)
zklick 2; strg x
echo "Typen nach Strg+X in Dateien: $(typen)" >> "$QD_AUS/innen.log"
echo "GNOME: $(wl-paste -t x-special/gnome-copied-files | tr '\n' '|') KDE: $(wl-paste -t application/x-kde-cutselection)" >> "$QD_AUS/innen.log"
pruefe C2 "Formate beim Ausschneiden" "GdkFileList, uri-list, gnome-copied-files „cut“, KDE-Markierung 1" '[ "$(wl-paste -t x-special/gnome-copied-files | head -1)" = cut ] && [ "$(wl-paste -t application/x-kde-cutselection)" = 1 ] && wl-paste --list-types | grep -qx text/uri-list' "$(grep -a 'Typen nach Strg+X' "$QD_AUS/innen.log" | cut -c1-150)"
klick 1100 500; strg v
warte_bis '[ -f "$D/zu-files.txt" ]' 10
eingabe <<< "wait 800"
pruefe C3 "Strg+X in Dateien, Strg+V in Files" "verschoben" '[ "$(cat "$D/zu-files.txt")" = z2 ] && [ ! -e "$T/zu-files.txt" ]' "$(ls "$D" | tr '\n' ' ')"
kill "$FPID"; wait "$FPID" 2>/dev/null
env QUERENCIA_DESKTOP_DEBUG=1 "$QD_DESKTOP" > "$QD_AUS/desktop.log" 2>&1 &
DPID=$!
warte_bis 'grep -a -q "vom-desktop" "$QD_AUS/desktop.log"' 20
eingabe <<< "wait 1500"
klick 70 32; strg c
titel; zklick 0; strg v
warte_bis '[ -f "$T/vom-desktop.txt" ]' 10
pruefe C4 "Strg+C auf der Arbeitsfläche, Strg+V in Dateien" "kopiert, Original bleibt" '[ "$(cat "$T/vom-desktop.txt")" = d ] && [ -f "$S/vom-desktop.txt" ]' "$(ls "$T" | tr '\n' ' ')"
eingabe <<< "wait 1000"
# ordner, aus-files, vom-desktop, zum-desktop
zklick 3; strg x
klick 1200 690; strg v
warte_bis '[ -f "$S/zum-desktop.txt" ]' 10
eingabe <<< "wait 800"
foto zwischenablage-desktop
pruefe C5 "Strg+X in Dateien, Strg+V auf der Arbeitsfläche" "verschoben" '[ "$(cat "$S/zum-desktop.txt")" = z1 ] && [ ! -e "$T/zum-desktop.txt" ]' "$(ls "$S" | tr '\n' ' ')"
# In Ordner einfügen
titel; zklick 1; strg c; zklick 0; aktion in-ordner-einfuegen   # ordner, aus-files, vom-desktop
warte_bis '[ -f "$T/ordner/aus-files.txt" ]' 10
pruefe C6 "„In Ordner einfügen“" "kopiert in den gewählten Ordner" '[ -f "$T/ordner/aus-files.txt" ] && [ -f "$T/aus-files.txt" ]' "$(ls "$T/ordner" | tr '\n' ' ')"
kill "$DPID"; wait "$DPID" 2>/dev/null
