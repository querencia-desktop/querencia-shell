# Erster Blick: Fotos-Ordner, Miniaturen, Suche
starte "$HOME/Bilder/Fotos"
warte_log "Ordner gelesen" 15
warte_log "Miniaturen fertig" 60 && fall E1 "Miniaturen fertig" 1 "$(grep 'Miniaturen fertig' "$LOG" | tail -1)" || fall E1 "Miniaturen fertig" 0 "-"
foto 01-fotos
grep -E "VmRSS|VmHWM" /proc/$PROG_PID/status > "$OUT/speicher-1.txt"
taste ctrl+l
q "type $HOME/Bilder/Gemischt" "key Return"
warte_log "Ordner gelesen" 10
warte_log "Miniaturen fertig" 30
q "wait 500"
foto 02-gemischt
taste ctrl+l
q "type $HOME/Dokumente/PDF" "key Return"
warte_log "Ordner gelesen" 10
warte_log "Miniaturen fertig" 30
q "wait 500"
foto 03-pdf
taste ctrl+l
q "type $HOME/Suche" "key Return"
warte_log "Ordner gelesen" 10
taste ctrl+f
q "type rechnung" "key Return"
warte_log "Suche .* fertig" 30
q "wait 500"
foto 04-suche
grep -E "VmRSS|VmHWM" /proc/$PROG_PID/status > "$OUT/speicher-2.txt"
