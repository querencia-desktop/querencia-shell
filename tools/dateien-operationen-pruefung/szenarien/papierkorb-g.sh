# Stufe G (gvfsd im Wegwerf-Bus): derselbe Ablauf wie papierkorb.sh, aber über trash:///
# („Gehe zu → Papierkorb“), Wiederherstellen und Leeren über GVfs.
source "$(dirname "$QD_SZ")/hilfen.sh"
T=$HOME/Test; mkdir -p "$T"
for n in 1 2 3; do echo "inhalt $n" > "$T/d$n.txt"; done; echo bleibt > "$T/keep.txt"
TR=$(papierkorb_ordner)
drei_waehlen() { q "move $((950 + OX)) $(( $(zeile 0) + OY ))" "click" "wait 200" "keydown Shift_L" "move $((950 + OX)) $(( $(zeile 2) + OY ))" "click" "keyup Shift_L" "wait 300"; }
start_app "$T"
# d1, d2, d3, keep
drei_waehlen; q "key Delete" "wait 300"
warte_log 'in den Papierkorb fertig' 10
pruefe P1 "Entf auf drei Dateien" "ohne Frage im Papierkorb, .trashinfo je Datei" '! grep -a -q "Frage:" "$LOG" && [ "$(ls "$TR/files" | wc -l)" = 3 ] && [ "$(ls "$TR/info" | wc -l)" = 3 ] && [ ! -e "$T/d1.txt" ]' "$(ls "$TR/files" | tr '\n' ' ')"
# in trash:/// steht über der Liste die Leiste „Papierkorb leeren …“ (B1): Zeilen 43 px tiefer
LEISTE=0
zeile() { echo $((241 + 26 * $1 + LEISTE)); }
zum_papierkorb() { local n; n=$(geladen); aktion papierkorb-zeigen; warte_ordner "$n"; titel; LEISTE=43; }
zurueck_nach_test() { gehe_zu "$T"; LEISTE=0; }
zum_papierkorb
pruefe G0 "Stufe G" "Fenster zeigt trash:///" 'grep -a -q "Ordner trash:///: lesen" "$LOG"' "$(grep -a 'Ordner trash' "$LOG" | tail -1)"
foto papierkorb-inhalt
zklick 0; strg a; aktion wiederherstellen
warte_log 'wiederherstellen fertig' 10
pruefe P2 "Wiederherstellen aus trash:///" "an den Ursprungsort, Papierkorb leer" '[ "$(cat "$T/d2.txt")" = "inhalt 2" ] && [ -z "$(ls -A "$TR/files")" ] && [ -z "$(ls -A "$TR/info")" ]' "$(ls "$T" | tr '\n' ' ')"
zurueck_nach_test
drei_waehlen; q "key Delete" "wait 300"
warte_bis '[ "$(ls "$TR/files" | wc -l)" = 3 ]' 10
zum_papierkorb
zklick 0; q "key Delete" "wait 800"
warte_log 'Frage: endgültig löschen' 5; q "wait 800"
q "key Escape" "wait 600"
pruefe P3 "Entf im Papierkorb" "fragt nach endgültigem Löschen (Esc: bleibt)" 'grep -a -q "endgültig löschen abgebrochen" "$LOG" && [ "$(ls "$TR/files" | wc -l)" = 3 ]'
titel; klick 933 224
warte_log 'Frage: Papierkorb leeren'; q "wait 800"
foto papierkorb-leeren-frage
q "key Return" "wait 800"
warte_log 'Papierkorb leeren abgebrochen' 5
pruefe P4 "Knopf „Papierkorb leeren …“ der Leiste, Eingabe" "Vorgabe Abbrechen: Papierkorb bleibt voll" 'grep -a -q "Papierkorb leeren abgebrochen" "$LOG" && [ "$(ls "$TR/files" | wc -l)" = 3 ]' "$(grep -a 'Papierkorb leeren' "$LOG" | tail -1)"
titel; klick 933 224; warte_bis '[ "$(zahl_log "Frage: Papierkorb leeren")" -ge 2 ]' 5; q "wait 800" "key Tab" "wait 200" "key Return" "wait 800"
warte_log 'Papierkorb leeren fertig' 10
pruefe P5 "Papierkorb leeren bestätigt" "Papierkorb leer" '[ -z "$(ls -A "$TR/files")" ] && [ -z "$(ls -A "$TR/info")" ]' "$(grep -a 'Papierkorb leeren fertig' "$LOG" | tail -1)"
zurueck_nach_test
zklick 0; q "keydown Shift_L" "key Delete" "keyup Shift_L" "wait 800"
warte_bis '[ "$(zahl_log "Frage: endgültig löschen")" -ge 2 ]' 5; q "wait 800"
foto loeschen-frage
q "key Escape" "wait 600"
pruefe P6 "Umschalt+Entf, Esc" "Datei bleibt" '[ -f "$T/keep.txt" ] && [ "$(zahl_log "endgültig löschen abgebrochen")" -ge 2 ]'
titel; zklick 0; q "keydown Shift_L" "key Delete" "keyup Shift_L" "wait 1200" "key Tab" "wait 200" "key Return" "wait 800"
warte_log 'endgültig löschen fertig' 10
pruefe P7 "Umschalt+Entf bestätigt" "Datei weg, nicht im Papierkorb" '[ ! -e "$T/keep.txt" ] && [ -z "$(ls -A "$TR/files")" ]'
