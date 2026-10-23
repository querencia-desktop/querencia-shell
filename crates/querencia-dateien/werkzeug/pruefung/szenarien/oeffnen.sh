# Öffnen über Tippsuche und Eingabe (D14), ausführbare Textdatei (D15),
# Öffnen mit (D17). Die Tippsuche der Ansicht wählt das Objekt.
waehle() { q "key Escape" "wait 200" "type $1" "wait 400"; }
starte "$HOME/Dokumente"
warte_log "Ordner gelesen" 15
: > "$HOME/aufrufe.log"
q "wait 500"
foto 01-dokumente
waehle angebot
q "key Return" "wait 800"
grep -q "attrappe-dokumente .*angebot.pdf" "$HOME/aufrufe.log" && fall D14a "PDF öffnet im Standardprogramm" 1 "aufrufe.log" || fall D14a "PDF" 0 "aufrufe.log"
waehle notiz
q "key Return" "wait 800"
grep -q "attrappe-editor .*notiz.txt" "$HOME/aufrufe.log" && fall D14b "Textdatei öffnet im Editor" 1 "aufrufe.log" || fall D14b "Text" 0 "aufrufe.log"
waehle skript
q "key Return" "wait 800"
foto 02-ausfuehren-frage
q "key Escape" "wait 500"
grep -q "ausgeführt" "$HOME/aufrufe.log" && fall D15a "Esc führt nicht aus" 0 "aufrufe.log" || fall D15a "Frage, Esc führt nicht aus" 1 "aufrufe.log ohne Eintrag"
waehle skript
q "key Return" "wait 800" "key Tab" "key Return" "wait 800"
grep -q "ausgeführt" "$HOME/aufrufe.log" && fall D15b "Ausführen führt aus" 1 "aufrufe.log: ausgeführt" || fall D15b "Ausführen" 0 "aufrufe.log"
waehle notiz
q "key Escape" "wait 200"
menue datei
q "key Down" "key Down" "key Down" "key Down" "key Down" "key Right" "wait 400"
foto 03-oeffnen-mit
q "key Down" "key Return" "wait 800"
grep -q "attrappe-zweit .*notiz.txt" "$HOME/aufrufe.log" && fall D17a "Öffnen mit anderem Programm" 1 "aufrufe.log" || fall D17a "Öffnen mit" 0 "aufrufe.log"
grep -q "^text/plain=attrappe-editor.desktop" "$XDG_CONFIG_HOME/mimeapps.list" && fall D17b "Standard bleibt" 1 "mimeapps.list unverändert" || fall D17b "Standard bleibt" 0 "mimeapps.list"
cp "$HOME/aufrufe.log" "$OUT/aufrufe-innen.log"
