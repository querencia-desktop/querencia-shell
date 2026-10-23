# B25: Netzwerk durchsuchen (Start mit LAUF_WSDD=1: wsdd-Attrappe, zwei
# Testgeräte, nur Unix-Socket). Erwartung: Gehe zu → Netzwerk zeigt beide
# Rechner; Doppelklick versucht zu verbinden und meldet verständlich, dass
# der Rechner nicht erreichbar ist (kein Netz im Namensraum).
source "$LAUF_NETZ/hilfen.sh"
starte
warte_log "Ordner gelesen" 20
markiere
menue gehe
q "move 373 217" "click"
warte_log "Ordner gelesen" 20
# gvfsd-wsdd fragt die Liste erst nach 5–15 s ab (PROBE_TIMEOUT/RELOAD_TIMEOUT)
ANZ=0
for _ in $(seq 20); do
  q "wait 2000"
  markiere
  q "key F5"
  warte_log "Ordner gelesen" 10
  ANZ=$(grep -a "Ordner gelesen" "$LOG" | tail -1 | grep -o '[0-9]* Einträge' | cut -d' ' -f1)
  [ "${ANZ:-0}" -ge 2 ] && break
done
foto_fenster 01-netzwerk
fall N1 "Netzwerk zeigt die Testgeräte" "$([ "${ANZ:-0}" -ge 2 ] && echo 1)" "$ANZ Einträge"
fall N2 "Seitenleiste markiert Netzwerk, Titel Netzwerk" "1" "Foto 01-netzwerk"
markiere
q "move 215 160" "dclick"
warte_log "einhängen \(fertig\|abgebrochen\|gescheitert\)\|Ordner gelesen" 30
q "wait 2500"
foto_fenster 02-geraet-oeffnen
fall N3 "Gerät öffnen → verständliche Meldung oder Ordner" "$(tail -n +"$LOG_STAND" "$LOG" | grep -a -q 'Netzwerk: Meldung\|Ordner gelesen' && echo 1)" "$(grep -a 'Netzwerk: Meldung' "$LOG" | tail -1 | cut -c1-120)"
q "key Escape" "wait 500"
