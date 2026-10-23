# Speicherverlauf über dreimal 200 neue Fotos (gleiche Bilder, andere Namen)
CSV=$OUT/messung.csv; : > "$CSV"
m() { printf '%s;%s;%s;%s\n' "$1" "$2" "$3" "$4" >> "$CSV"; }
speicher() { m "$1 VmRSS" "$(awk '/VmRSS/{print $2}' /proc/$PROG_PID/status)" KiB status; }
for i in 2 3; do cp -r "$HOME/Bilder/Fotos" "$HOME/Bilder/Fotos$i"; done
starte "$HOME/Suche"; warte_log "Ordner gelesen" 15; q "wait 1500"; speicher "Start"
for d in Fotos Fotos2 Fotos3; do
  taste ctrl+l; q "type $HOME/Bilder/$d" "key Return"; warte_log "Ordner gelesen" 15
  warte_log "Miniaturen fertig" 60; q "wait 1500"; speicher "nach $d"
done
q "wait 5000"; speicher "5 s später"
cat "$CSV"
