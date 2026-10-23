# Welle C2: Belegfotos Miniaturen und Suche (je Lauf: Größe und hell/dunkel
# aus LAUF_GROESSE/LAUF_DUNKEL).
gehe() { taste ctrl+l; q "type $1" "key Return"; warte_log "$(basename "$1"): lesen" 10; warte_log "Ordner gelesen" 15; }
starte "$HOME/Bilder/Fotos"
warte_log "Ordner gelesen" 15; warte_log "Miniaturen fertig" 60; q "wait 600"
foto 01-fotos-symbolansicht-100
gehe "$HOME/Bilder/Gemischt"; warte_log "Miniaturen fertig" 30; q "wait 600"
foto 02-bildarten-alpha-svg-kaputt-zu-gross
gehe "$HOME/Dokumente/PDF"; warte_log "Miniaturen fertig" 30; q "wait 600"
foto 03-pdf-ohne-sandkasten-nur-symbol
gehe "$HOME/Bilder/Fotos"; q "wait 500"
taste ctrl+plus; taste ctrl+plus; q "wait 1500"
foto 04-fotos-200
taste ctrl+minus; taste ctrl+minus; taste ctrl+minus; q "wait 1500"
foto 05-fotos-66
taste ctrl+0; taste ctrl+2; q "wait 1500"
foto 06-liste-24
taste ctrl+1
gehe "$HOME/Suche"
taste ctrl+f; q "wait 300"
foto 07-suchleiste
q "type rechnung" "key Return"; warte_log "Suche [0-9]* fertig" 30; q "wait 500"
foto 08-suchergebnisse
q "move 260 199" "click" "rclick" "wait 500"
foto 09-kontextmenue-treffer
q "key Escape" "wait 300" "key Escape" "wait 300"
menue gehe; q "wait 300"
foto 10-menue-gehe-zu
q "key Escape" "wait 300"
menue bearbeiten
q "key Up" "key Return" "wait 900" "move 486 54" "click" "wait 500"
foto 11-einstellungen-vorschau
q "key Escape" "wait 300"
