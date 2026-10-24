# Navigation über Tastatur: Ort (Strg+L, Tab, Esc, „/“), tiefer Pfad,
# Verlauf, Seitenleiste F9, verborgene Dateien, Zoom, Hilfe, Info, Lesezeichen.
HEIM=$HOME
starte
warte_log "Ordner gelesen" 15 || fall D01 "Start" 0 "keine Logzeile"
foto 01-start
markiere
taste ctrl+l
q "type Dok" "key Tab" "wait 200"
foto 02-ort-tab
q "key Return"
if warte_log "Ordner file://$HEIM/Dokumente: lesen" 5; then
  fall D08 "Strg+L, Tab-Vervollständigung, Eingabe" 1 "programm.log: Dokumente lesen"
else
  fall D08 "Strg+L, Tab-Vervollständigung, Eingabe" 0 "kein Ortswechsel"
fi
warte_log "Ordner gelesen" 5
taste ctrl+l
q "type xyz" "key Escape" "wait 200"
foto 03-ort-esc
q "type /" "wait 200"
foto 04-schraegstrich
q "key Escape"
markiere
taste ctrl+l
q "type $HEIM/Tief/ebene-01/ebene-02/ebene-03/ebene-04/ebene-05/ebene-06/ebene-07/ebene-08/ebene-09/ebene-10/ebene-11/ebene-12" "key Return"
warte_log "ebene-12: lesen" 5 && fall D07a "Tiefer Pfad über das Textfeld" 1 "programm.log" || fall D07a "Tiefer Pfad über das Textfeld" 0 "kein Wechsel"
foto 05-tiefer-pfad
markiere
taste alt+Up
warte_log "ebene-11: lesen" 5 && fall D06a "Alt+↑ nach oben" 1 "programm.log ebene-11" || fall D06a "Alt+↑ nach oben" 0 "-"
foto 06-nach-oben-kind-markiert
markiere
taste alt+Left
warte_log "ebene-12: lesen" 5 && fall D06b "Alt+← zurück" 1 "programm.log ebene-12" || fall D06b "Alt+← zurück" 0 "-"
markiere
taste alt+Right
warte_log "ebene-11: lesen" 5 && fall D06c "Alt+→ vor" 1 "programm.log ebene-11" || fall D06c "Alt+→ vor" 0 "-"
markiere
taste alt+Home
warte_log "Ordner file://$HEIM: lesen" 5 && fall D06d "Alt+Pos1 persönlicher Ordner" 1 "programm.log" || fall D06d "Alt+Pos1" 0 "-"
warte_log "Ordner gelesen" 5
q "key F9" "wait 300"
foto 07-ohne-seitenleiste
grep -q "Seitenleiste=false" "$XDG_CONFIG_HOME/querencia/dateien.ini" && fall D28a "F9 aus, in dateien.ini" 1 "Seitenleiste=false" || fall D28a "F9 aus" 0 "dateien.ini"
q "key F9" "wait 300"
taste ctrl+h
q "wait 300"
foto 08-verborgene
grep -q "Verborgene=true" "$XDG_CONFIG_HOME/querencia/dateien.ini" && fall D13a "Strg+H gemerkt" 1 "Verborgene=true" || fall D13a "Strg+H gemerkt" 0 "dateien.ini"
taste ctrl+h
taste ctrl+plus
foto 09-zoom-150
taste ctrl+minus
taste ctrl+minus
foto 10-zoom-66
taste ctrl+0
foto 11-zoom-100
taste ctrl+2
foto 12-liste
taste ctrl+1
q "key F1" "wait 1000"
grep -q "attrappe-browser https://querencialinux.org/de/docs/" "$HOME/aufrufe.log" \
  && fall D04a "F1 Online-Hilfe" 1 "aufrufe.log: /de/docs/" || fall D04a "F1 Online-Hilfe" 0 "aufrufe.log"
menue hilfe
q "key Down" "key Down" "key Return" "wait 500"
grep -q "attrappe-browser https://github.com/querencia-desktop/querencia/issues/new/choose" "$HOME/aufrufe.log" \
  && fall D04b "Fehler melden" 1 "aufrufe.log: Meldestelle" || fall D04b "Fehler melden" 0 "aufrufe.log"
menue hilfe
q "key Up" "key Return" "wait 700"
foto 13-info
q "key Return" "wait 300"
foto 14-info-zu
taste ctrl+d
q "wait 300"
grep -q "file://$HEIM" "$XDG_CONFIG_HOME/gtk-3.0/bookmarks" 2>/dev/null \
  && fall D29a "Strg+D Lesezeichen in bookmarks" 1 "$(tail -n 1 "$XDG_CONFIG_HOME/gtk-3.0/bookmarks")" || fall D29a "Strg+D" 0 "bookmarks"
menue lesezeichen
foto 15-lesezeichen-menue
q "key Escape"
