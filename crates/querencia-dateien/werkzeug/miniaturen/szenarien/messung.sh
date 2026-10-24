# Welle C2, Messung: Miniaturen für 200 Fotos (neu, aus dem Cache, Zoom,
# Liste), Abbruch beim Ordnerwechsel, Suche über 10.000 Einträge (3 Läufe),
# Speicher (VmRSS/VmHWM). Zeilen in messung.csv: art;wert;einheit;beleg
CSV=$OUT/messung.csv; : > "$CSV"
m() { printf '%s;%s;%s;%s\n' "$1" "$2" "$3" "$4" >> "$CSV"; }
speicher() { m "$1 VmRSS" "$(awk '/VmRSS/{print $2}' /proc/$PROG_PID/status)" KiB status; m "$1 VmHWM" "$(awk '/VmHWM/{print $2}' /proc/$PROG_PID/status)" KiB status; }
fertig() { # Name: wartet auf „Miniaturen fertig“ und notiert die Zeile
  if warte_log "Miniaturen fertig" "${2:-60}"; then
    local z; z=$(grep "Miniaturen fertig" "$LOG" | tail -1)
    m "$1" "$(sed -E 's/.* in ([0-9]+) ms.*/\1/' <<<"$z")" ms "${z#*: }"
  else
    m "$1" "-" ms "keine Zeile"
  fi
}
gehe() { taste ctrl+l; q "type $1" "key Return"; warte_log "Ordner gelesen" 15; }
starte "$HOME/Suche"
warte_log "Ordner gelesen" 15
q "wait 1500"
speicher "Start (Ordner Suche, ohne Bilder)"
gehe "$HOME/Bilder/Fotos"
fertig "200 Fotos neu (1920x1080 JPEG, 100 %)" 120
q "wait 1000"
speicher "nach 200 Miniaturen"
gehe "$HOME/Suche"
q "wait 500"
gehe "$HOME/Bilder/Fotos"
q "wait 500"
z=$(tail -n 5 "$LOG" | grep -c "Miniaturen fertig")
m "200 Fotos erneut im selben Prozess (aus dem Speicher)" "$z" "Zeilen „fertig“ (0: keine Aufträge)" "-"
kill "$PROG_PID"; wait "$PROG_PID" 2>/dev/null
markiere
starte "$HOME/Bilder/Fotos"
fertig "200 Fotos aus dem Cache (neuer Prozess, 100 %)" 60
q "wait 1000"
speicher "neuer Prozess nach 200 Miniaturen aus dem Cache"
taste ctrl+plus; taste ctrl+plus
fertig "Zoom 200 % (128 px, aus dem Cache normal)" 60
taste ctrl+0
taste ctrl+2
fertig "Listenansicht (24 px)" 60
taste ctrl+1
q "wait 500"
speicher "nach Zoom und Liste"
# Abbruch: 200 neue Fotos (Kopie, noch ohne Miniatur), 150 ms nach „Ordner gelesen“ Alt+↑
cp -r "$HOME/Bilder/Fotos" "$HOME/Bilder/Abbruch"
markiere
gehe "$HOME/Bilder/Abbruch"
q "wait 150"
taste alt+Up
q "wait 2000"
z=$(tail -n +"$((LOG_STAND + 1))" "$LOG" | grep "nach Ordnerwechsel" | tail -1)
m "Abbruch (Alt+↑ 150 ms nach dem Lesen)" "$(sed -E 's/.*: ([0-9]+) zurückgenommen.*/\1/' <<<"$z")" "zurückgenommen" "${z#*: }"
n=$(find "$XDG_CACHE_HOME/thumbnails/normal" -name '*.png' | wc -l)
m "Cache-Einträge normal nach dem Abbruch (200 Fotos + Abbruch)" "$n" Dateien "-"
speicher "nach dem Abbruch"
gehe "$HOME/Suche"
# Suche über 10.000 Einträge
for i in 1 2 3; do
  markiere
  taste ctrl+f
  q "key ctrl+a" "type rechnung" "key Return"
  if warte_log "Suche [0-9]* fertig" 30; then
    z=$(grep "Suche [0-9]* fertig" "$LOG" | tail -1)
    m "Suche „rechnung“ Lauf $i (Faden)" "$(sed -E 's/.*Faden ([0-9]+) ms.*/\1/' <<<"$z")" ms "${z#*: }"
    m "Suche „rechnung“ Lauf $i (bis zur Anzeige)" "$(sed -E 's/.*Anzeige ([0-9]+) ms.*/\1/' <<<"$z")" ms "-"
  fi
  q "key Escape" "wait 300"
done
markiere
taste ctrl+f
q "type datei" "key Return"
if warte_log "Suche [0-9]* fertig" 60; then
  z=$(grep "Suche [0-9]* fertig" "$LOG" | tail -1)
  m "Suche „datei“ (fast alle Einträge, Faden)" "$(sed -E 's/.*Faden ([0-9]+) ms.*/\1/' <<<"$z")" ms "${z#*: }"
  m "Suche „datei“ (bis zur Anzeige)" "$(sed -E 's/.*Anzeige ([0-9]+) ms.*/\1/' <<<"$z")" ms "-"
fi
q "wait 1000"
speicher "nach Suche „datei“"
q "key Escape" "wait 500"
foto messung-ende
cat "$CSV"
