# Dateien 0.2.4, QD-173 P-3: Statusleiste mit der Suche. Während Treffer
# gezeigt werden „N Treffer“, bei Auswahl „„name“ ausgewählt (Größe)“ bzw.
# „N Objekte ausgewählt (…)“; nach Esc oder Strg+F wieder die Ordnerzeile
# (vorher blieb sie leer, L1-09).
export QUERENCIA_DATEIEN_DEBUG=1
HEIM=$HOME
MARKE=0
marke() { markiere; MARKE=$LOG_STAND; }
letzte() { tail -n +"$((MARKE + 1))" "$LOG" | grep -a -- "$1" | tail -n 1; }
status() { grep -a ': Statusleiste: ' "$LOG" | tail -1 | sed -E 's/.*: Statusleiste: //'; }
if [ "${LAUF_SPRACHE:-de}" = en ]; then
  ORDNER="5 items, Free space: "; T1="1 match"; T4="4 matches"
  S1="“querencia-glut.png” selected (3.8 MB)"; S4="4 items selected (3.8 MB)"
else
  ORDNER="5 Objekte, freier Speicherplatz: "; T1="1 Treffer"; T4="4 Treffer"
  S1="„querencia-glut.png“ ausgewählt (3,8 MB)"; S4="4 Objekte ausgewählt (3,8 MB)"
fi
P="$HEIM/Probe"; mkdir -p "$P"
truncate -s 3797000 "$P/querencia-glut.png"
for n in querencia-abend.png querencia-blau-2.png querencia-tiefblau.png streifen.png; do head -c 1000 /dev/zero > "$P/$n"; done

starte "$P"
warte_log "Ordner gelesen" 15 || fall SS0 "Start" 0 "keine Logzeile"
warte_log "Statusleiste: $ORDNER" 10 \
  && fall SS1 "Ordnerzeile vor der Suche" 1 "$(status)" || fall SS1 "Ordnerzeile vor der Suche" 0 "$(status)"
foto 01-ordner
marke
taste ctrl+f
warte_log "Suchleiste geöffnet" 5
q "type glut" "key Return"
warte_log "fertig: 1 Treffer" 15
q "wait 300"
[ "$(status)" = "$T1" ] && fall SS2 "Suche „glut“: Statusleiste „$T1“" 1 "$(status)" || fall SS2 "Suche: Trefferzahl" 0 "$(status)"
foto 02-ein-treffer
q "key Down" "wait 300"
taste ctrl+a
q "wait 300"
[ "$(status)" = "$S1" ] && fall SS3 "Auswahl im Suchergebnis: „$S1“" 1 "$(status)" || fall SS3 "Auswahl im Suchergebnis" 0 "$(status)"
foto 03-treffer-ausgewaehlt
marke
q "key Escape" "wait 500"
letzte "Suche beendet" > /dev/null && [ "$(status)" != "" ] && [[ $(status) == "$ORDNER"* ]] \
  && fall SS4 "Esc schließt die Suche: wieder die Ordnerzeile (L1-09)" 1 "$(status)" || fall SS4 "Esc: Ordnerzeile" 0 "„$(status)“"
foto 04-nach-esc
marke
taste ctrl+f
warte_log "Suchleiste geöffnet" 5
q "type querencia" "key Return"
warte_log "fertig: 4 Treffer" 15
q "wait 300"
[ "$(status)" = "$T4" ] && fall SS5 "Suche „querencia“: „$T4“" 1 "$(status)" || fall SS5 "Suche: vier Treffer" 0 "$(status)"
q "key Down" "wait 300"
taste ctrl+a
q "wait 300"
[ "$(status)" = "$S4" ] && fall SS6 "Alle Treffer ausgewählt: „$S4“" 1 "$(status)" || fall SS6 "Mehrfachauswahl im Suchergebnis" 0 "$(status)"
foto 05-vier-ausgewaehlt
marke
taste ctrl+f
q "wait 500"
letzte "Suche beendet" > /dev/null && [[ $(status) == "$ORDNER"* ]] \
  && fall SS7 "Strg+F schließt die Suche: wieder die Ordnerzeile" 1 "$(status)" || fall SS7 "Strg+F: Ordnerzeile" 0 "„$(status)“"
foto 06-nach-strg-f
# im Ordner: Auswahl wieder wie gewohnt
q "type streifen" "wait 700" "key Escape" "wait 300"
[[ $(status) == *streifen.png* ]] \
  && fall SS8 "Danach im Ordner: Auswahl in der Statusleiste" 1 "$(status)" || fall SS8 "Auswahl im Ordner" 0 "$(status)"
grep -a -E "CRITICAL|panicked" "$LOG" > /dev/null && fall SS9 "Keine Warnung CRITICAL, kein Absturz" 0 "$(grep -a -E 'CRITICAL|panicked' "$LOG" | head -2 | tr '\n' ' ')" \
  || fall SS9 "Keine Warnung CRITICAL, kein Absturz" 1 "programm.log"
