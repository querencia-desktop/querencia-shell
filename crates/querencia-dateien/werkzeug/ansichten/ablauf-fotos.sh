# Querencia Dateien, Ansichten, Ablauf „fotos“ (aus szenario.sh eingelesen):
# Belegfotos mit labwc-Rahmen wie live, Fenster maximiert (B §5.3 Nr. 1, 2,
# 7, 8 und die Bedienung der Ansichten). Mausziele aus den Fokuszeilen
# (Fensterkoordinaten) plus Höhe der Titelleiste T.
B=$BEISPIEL
T=26
taste() { local k; for k in "$@"; do q "key $k" "wait 150"; done; }
tippe() { q "type $1" "wait 250"; }
strg() { q "keydown Control_L" "type $1" "keyup Control_L" "wait 400"; }
start "$B"
tippe beri; taste Escape
foto f01-symbolansicht 600
read -r FX FY FW FH < <(fokus_lage)
q "move $((FX + FW / 2)) $((FY + T + 20))" "rclick" "wait 600"
foto f02-kontextmenue-datei
taste Escape; q "wait 300"
q "move $((BX / 2 + 200)) $((BY - 200))" "rclick" "wait 600"
foto f03-kontextmenue-hintergrund
taste Escape; q "wait 300"
tippe beri; taste Escape F2; q "wait 400"
foto f04-umbenennen
taste Escape; q "wait 300"
tippe da
foto f05-tippsuche 100
taste Escape
read -r X0 Y0 ZW ZH < <(grep -a ': Fokus Symbolansicht: ' "$LOG" | head -1 | sed -E 's/.*\| ([0-9-]+) ([0-9-]+) ([0-9]+) ([0-9]+)$/\1 \2 \3 \4/')
q "move $((BX - 80)) $((BY - 150))" "down left" "glide $((X0 + 2 * ZW + ZW / 2)) $((Y0 + T + ZH / 2)) 400" "wait 300"
foto f06-gummiband 100
q "up left" "wait 300"
q "move $((BX / 2)) $((BY - 150))" "click" "wait 300"
strg + ; strg +
foto f07-zoom-200
strg 0
aktion anordnen groesse; q "wait 400"
strg 2; q "wait 800"
foto f08-liste-nach-groesse
aktion anordnen name; strg 1; q "wait 400"
stopp
info "$(ls "$FOTOS" | grep -c png) Fotos"
