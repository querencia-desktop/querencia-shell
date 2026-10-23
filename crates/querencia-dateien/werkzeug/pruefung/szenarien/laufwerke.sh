# D30: Laufwerke in der Seitenleiste (Attrappen des Prüfrahmens): eingehängt,
# schreibbar, nicht eingehängt; Einhängen per Klick, Aushängen, belegt.
starte
warte_log "Ordner gelesen" 15
q "wait 500"
foto 01-seitenleiste-laufwerke
gio mount -l > "$OUT/gio-mount-vorher.txt" 2>&1
# Zeilen: QD-Nicht 280, QD-Schreibbar 304, QD-Stick 329 (nach Name), ⏏ bei x 132;
# in Stufe G steht „Netzwerk“ darüber (eine Zeile, 23 px tiefer)
V=0; [ "${LAUF_STUFE:-L}" = G ] && V=23
markiere
q "move 60 $((329 + V))" "click" "wait 600"
warte_log "Ordner file:///media/QD-Stick: lesen" 5 && fall D30a "Eingehängtes Laufwerk per Klick öffnen" 1 "programm.log" || fall D30a "Laufwerk öffnen" 0 "-"
foto 02-stick-offen
q "move 60 $((280 + V))" "click" "wait 1500"
foto 03-einhaengen-nicht-eingehaengt
q "key Escape" "wait 300"
( cd /media/QD-Schreibbar && exec sleep 60 ) &
BELEGT=$!
q "wait 300" "move 132 $((304 + V))" "click" "wait 1500"
foto 04-aushaengen-belegt
q "key Escape" "wait 300"
kill $BELEGT 2>/dev/null
q "move 132 $((304 + V))" "click" "wait 1500"
foto 05-aushaengen
gio mount -l > "$OUT/gio-mount-nachher.txt" 2>&1
