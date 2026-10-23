# Welle C3 mit C2: die Suche (Strg+F) je Reiter. Die Leiste erscheint im
# Bereich des gezeigten Reiters, ein Reiterwechsel beendet die Suche und
# zeigt die Ansicht wieder; im anderen Reiter zieht die Leiste mit.
export QUERENCIA_DATEIEN_DEBUG=1
HEIM=$HOME
MARKE=0
marke() { markiere; MARKE=$LOG_STAND; }
neu_seit() { tail -n +"$((MARKE + 1))" "$LOG" | grep -a -q -- "$1"; }
starte
warte_log "Ordner gelesen" 15
taste ctrl+t
warte_log "Reiter 2 gezeigt" 5
taste ctrl+l
q "type $HEIM/Dokumente" "key Return"
warte_log "Dokumente: lesen" 5
warte_log "Ordner gelesen" 5
marke
taste ctrl+f
warte_log "Suchleiste geöffnet" 5 && s1=1 || s1=0
q "type rechnung" "key Return"
warte_log "fertig: [1-9][0-9]* Treffer" 15 && s2=1 || s2=0
q "wait 400"
foto 01-suche-im-zweiten-reiter
[ "$s1$s2" = 11 ] && fall RS1 "Strg+F im zweiten Reiter: Leiste dort, „rechnung“ findet Treffer" 1 "Suchleiste geöffnet; $(grep -a "fertig:" "$LOG" | tail -1 | grep -o "fertig: [0-9]* Treffer")" \
  || fall RS1 "Suche im zweiten Reiter" 0 "$s1$s2"
marke
taste ctrl+Page_Up
warte_log "Reiter 1 gezeigt" 5
neu_seit "Suche beendet" && fall RS2 "Reiterwechsel beendet die Suche" 1 "Suche beendet" || fall RS2 "Reiterwechsel beendet die Suche" 0 "-"
q "wait 400"
foto 02-erster-reiter-ohne-suche
marke
taste ctrl+Page_Down
warte_log "Reiter 2 gezeigt" 5
q "key Home" "wait 200" "key Right" "wait 300"
grep -a ': Fokus Symbolansicht: ' "$LOG" | tail -1 | grep -q -E '\| [0-9]+ [0-9]+ [1-9][0-9]* [1-9][0-9]*$' \
  && fall RS3 "Zurück im zweiten Reiter: dessen Ansicht ist wieder sichtbar (Zelle mit Größe)" 1 "$(grep -a ': Fokus Symbolansicht: ' "$LOG" | tail -1 | cut -d: -f3-)" \
  || fall RS3 "Ansicht wieder sichtbar" 0 "$(grep -a ': Fokus' "$LOG" | tail -1)"
foto 03-zweiter-reiter-wieder
taste ctrl+Page_Up
warte_log "Reiter 1 gezeigt" 5
marke
taste ctrl+f
warte_log "Suchleiste geöffnet" 5
q "type ebene" "key Return"
warte_log "fertig: [1-9][0-9]* Treffer" 15 && fall RS4 "Strg+F im ersten Reiter: die Leiste zieht mit, „ebene“ findet Treffer" 1 "$(grep -a "fertig:" "$LOG" | tail -1 | grep -o "fertig: [0-9]* Treffer")" \
  || fall RS4 "Suche im ersten Reiter" 0 "$(tail -n 3 "$LOG" | tr '\n' ' ')"
q "wait 400"
foto 04-suche-im-ersten-reiter
taste ctrl+w
q "wait 600"
grep -a -E "CRITICAL|panicked|Ansichtsbereich nicht gefunden" "$LOG" > /dev/null && fall RS5 "Keine Warnung CRITICAL, kein Absturz" 0 "$(grep -a -E 'CRITICAL|panicked|Ansichtsbereich' "$LOG" | head -2 | tr '\n' ' ')" \
  || fall RS5 "Keine Warnung CRITICAL, kein Absturz" 1 "programm.log"
