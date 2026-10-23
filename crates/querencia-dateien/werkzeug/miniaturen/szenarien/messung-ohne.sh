# Welle C2, Vergleich: dieselben Ordner mit „Miniaturen anzeigen: Nie“
# (LAUF_INI "[Vorschau]\nBilder=nie"): Speicher ohne Miniaturen.
CSV=$OUT/messung.csv; : > "$CSV"
m() { printf '%s;%s;%s;%s\n' "$1" "$2" "$3" "$4" >> "$CSV"; }
speicher() { m "$1 VmRSS" "$(awk '/VmRSS/{print $2}' /proc/$PROG_PID/status)" KiB status; m "$1 VmHWM" "$(awk '/VmHWM/{print $2}' /proc/$PROG_PID/status)" KiB status; }
starte "$HOME/Suche"
warte_log "Ordner gelesen" 15
q "wait 1500"
speicher "Start (Ordner Suche, ohne Bilder)"
taste ctrl+l; q "type $HOME/Bilder/Fotos" "key Return"; warte_log "Ordner gelesen" 15
q "wait 3000"
speicher "Fotos ohne Miniaturen"
if grep -q "Miniaturen fertig" "$LOG"; then fall N1 "Nie: keine Miniaturen" 0 "$(grep 'Miniaturen fertig' "$LOG")"; else fall N1 "Nie: keine Miniaturen erzeugt" 1 "keine Zeile „Miniaturen fertig“"; fi
foto nie-fotos
cat "$CSV"
