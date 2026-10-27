# Dateien 0.3.0: Reiter beim Start wiederherstellen (Vorgabe an). Drei
# Reiter, der zweite aktiv, Fenster schließen; Start ohne Ordner öffnet
# alle drei; ein gelöschter Ordner wird übersprungen; mit der Einstellung
# aus oder mit einer Ordnerangabe bleibt es beim gewohnten Start.
export QUERENCIA_DATEIEN_DEBUG=1
HEIM=$HOME
P="$HEIM/Probe"; mkdir -p "$P/eins" "$P/zwei" "$P/drei"
ZUSTAND="${XDG_STATE_HOME:-$HEIM/.local/state}/querencia/dateien-reiter"
INI="${XDG_CONFIG_HOME:-$HEIM/.config}/querencia/dateien.ini"
gehe() { taste ctrl+l; q "type $1" "key Return" "wait 800"; }
schluss() { # alle Fenster schließen, auf das Programmende warten
  taste shift+ctrl+w
  local i
  for ((i = 0; i < 50; i++)); do kill -0 "$PROG_PID" 2>/dev/null || break; q "wait 100"; done
  kill -0 "$PROG_PID" 2>/dev/null && { fall R00 "Programmende" 0 "läuft noch"; ende; }
  PROG_PID=
}

starte "$P/eins"
warte_log "Ordner gelesen" 15 || fall R00 "Start" 0 "keine Logzeile"
q "wait 500"
taste ctrl+t; q "wait 500"; gehe "$P/zwei"
taste ctrl+t; q "wait 500"; gehe "$P/drei"
taste ctrl+Page_Up; q "wait 500"
foto 01-drei-reiter
schluss
grep -a -q 'Reiter gemerkt: 3 (aktiv 2)' "$LOG" && [ -f "$ZUSTAND" ] \
  && fall R01 "Schließen merkt 3 Reiter, aktiv der zweite" 1 "$(grep -v '^#' "$ZUSTAND" | sed "s|$HEIM|~|" | tr '\n' ' ')" \
  || fall R01 "Reiter gemerkt" 0 "$(grep -a 'Reiter gemerkt' "$LOG" | tail -1) / $(ls "$(dirname "$ZUSTAND")" 2>&1)"

markiere
starte
warte_log "Reiter wiederhergestellt" 15 && q "wait 1000"
letzte=$(grep -a 'Reiter wiederhergestellt' "$LOG" | tail -1)
[[ $letzte == *"3 von 3 (aktiv 2)"* ]] \
  && fall R02 "Start ohne Ordner: alle drei Reiter, der zweite aktiv" 1 "${letzte#*: }" \
  || fall R02 "Wiederherstellen" 0 "${letzte:-keine Zeile}"
grep -a 'Reiter [0-9]* neu: ' "$LOG" | tail -3 | grep -c -E '/Probe/(eins|zwei|drei)$' | grep -q '^3$' \
  && fall R03 "Reihenfolge eins, zwei, drei" 1 "$(grep -a 'Reiter [0-9]* neu: ' "$LOG" | tail -3 | sed -E 's/.*neu: ([0-9/]*) .*\/(.*)$/\1 \2/' | tr '\n' ' ')" \
  || fall R03 "Reihenfolge" 0 "$(grep -a 'Reiter [0-9]* neu: ' "$LOG" | tail -3 | tr '\n' ' ')"
foto 02-wiederhergestellt
schluss

# ein Ordner fehlt beim nächsten Start: übersprungen, der Reiter davor aktiv
rmdir "$P/zwei"
markiere
starte
warte_log "Reiter wiederhergestellt" 15 && q "wait 800"
letzte=$(grep -a 'Reiter wiederhergestellt' "$LOG" | tail -1)
grep -a -q 'Reiter übersprungen (nicht erreichbar): .*/Probe/zwei' "$LOG" && [[ $letzte == *"2 von 3 (aktiv 1)"* ]] \
  && fall R04 "Gelöschter Ordner übersprungen, 2 von 3, aktiv der erste" 1 "${letzte#*: }" \
  || fall R04 "Nicht erreichbarer Ort" 0 "${letzte:-keine Zeile}"
foto 03-ohne-zwei
# Einstellungen → Verhalten zeigen den Schalter
menue bearbeiten
q "key Up" "wait 200" "key Return" "wait 1000"
q "move 171 54" "click" "wait 500"
foto 04-einstellungen-verhalten
q "key Escape" "wait 500"
schluss

# Einstellung aus: gewohnter Start im persönlichen Ordner
sed -i 's/^ReiterWiederherstellen=.*/ReiterWiederherstellen=false/' "$INI"
grep -q '^ReiterWiederherstellen=false' "$INI" || fall R05 "Einstellung in dateien.ini" 0 "$(grep Reiter "$INI")"
markiere; AB=$LOG_STAND
starte
warte_log "Ordner gelesen" 15; q "wait 800"
! tail -n +"$((AB + 1))" "$LOG" | grep -a -q 'Reiter wiederhergestellt' \
  && tail -n +"$((AB + 1))" "$LOG" | grep -a 'Reiter [0-9]* neu' | tail -1 | grep -q "1/1 $(printf 'file://%s' "$HEIM")\$" \
  && fall R05 "Einstellung aus: ein Reiter, persönlicher Ordner" 1 "$(tail -n +"$((AB + 1))" "$LOG" | grep -a 'Reiter [0-9]* neu' | tail -1 | sed "s|$HEIM|~|")" \
  || fall R05 "Einstellung aus" 0 "$(tail -n +"$((AB + 1))" "$LOG" | grep -a -E 'Reiter (wiederhergestellt|[0-9]* neu)' | tr '\n' ' ' | sed "s|$HEIM|~|g")"
schluss

# Einstellung an, aber Start mit Ordner: nur dieser Ordner
sed -i 's/^ReiterWiederherstellen=.*/ReiterWiederherstellen=true/' "$INI"
markiere; AB=$LOG_STAND
starte "$P/drei"
warte_log "Ordner gelesen" 15; q "wait 800"
! tail -n +"$((AB + 1))" "$LOG" | grep -a -q 'Reiter wiederhergestellt' \
  && fall R06 "Start mit Ordner: keine Wiederherstellung" 1 "$(tail -n +"$((AB + 1))" "$LOG" | grep -a 'Reiter [0-9]* neu' | tail -1 | sed "s|$HEIM|~|")" \
  || fall R06 "Start mit Ordner" 0 "$(grep -a 'Reiter wiederhergestellt' "$LOG" | tail -1)"
schluss
grep -a -E "CRITICAL|panicked" "$LOG" > /dev/null && fall R07 "Keine Warnung CRITICAL, kein Absturz" 0 "$(grep -a -E 'CRITICAL|panicked' "$LOG" | head -2 | tr '\n' ' ')" \
  || fall R07 "Keine Warnung CRITICAL, kein Absturz" 1 "programm.log"
