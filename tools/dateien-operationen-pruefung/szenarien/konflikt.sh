# D24: jede Konfliktantwort (E2), „für alle“, Ordner zusammenführen.
source "$(dirname "$QD_SZ")/hilfen.sh"
T=$HOME/Test; mkdir -p "$T/a/m" "$T/b/m"
echo neu > "$T/a/x.txt"; echo neu > "$T/a/y.txt"; echo neu > "$T/a/z.txt"
echo neu-m > "$T/a/m/x.txt"; echo nur-a > "$T/a/m/q.txt"
zuruecksetzen() { rm -rf "$T/b"; mkdir -p "$T/b/m"; echo alt > "$T/b/x.txt"; echo alt > "$T/b/y.txt"; echo alt > "$T/b/z.txt"; echo alt-m > "$T/b/m/x.txt"; }
zuruecksetzen
hoch() { local n; n=$(geladen); q "keydown Alt_L" "key Up" "keyup Alt_L"; warte_ordner "$n"; }
hinein() { local n; n=$(geladen); doppel "$1"; warte_ordner "$n"; }
einfuegen_frage() { local n; n=$(fragen); zklick 0; strg v; warte_frage "$n"; }
start_app "$T"
hinein 0; zklick 1; strg c          # a: m, x, y, z → x.txt
hoch; hinein 1                      # b
einfuegen_frage
foto konflikt-datei
q "key Return" "wait 800"; warte_log 'kopieren fertig: 0 erledigt, 1 übersprungen'
pruefe K1 "Eingabe (Vorgabeknopf)" "Überspringen: Ziel bleibt" 'grep -a -q "Antwort zu Auftrag [0-9]*: Überspringen$" "$LOG" && [ "$(cat "$T/b/x.txt")" = alt ]' "$(grep -a 'Antwort zu' "$LOG" | tail -1)"
titel; einfuegen_frage; alt e; warte_bis '[ "$(cat "$T/b/x.txt")" = neu ]' 10
pruefe K2 "Alt+E" "Ersetzen: neuer Inhalt, kein Bauname übrig" '[ "$(cat "$T/b/x.txt")" = neu ] && [ -z "$(ls -A "$T/b" | grep querencia-teil)" ]' "$(grep -a 'Antwort zu' "$LOG" | tail -1)"
echo alt > "$T/b/x.txt"
titel; einfuegen_frage; alt u; warte_bis '[ -f "$T/b/x (Kopie).txt" ]' 10
pruefe K3 "Alt+U" "Umbenennen: „x (Kopie).txt“, Ziel bleibt" '[ "$(cat "$T/b/x (Kopie).txt")" = neu ] && [ "$(cat "$T/b/x.txt")" = alt ]' "$(grep -a 'Antwort zu' "$LOG" | tail -1)"
titel; einfuegen_frage; q "key Escape" "wait 800"; warte_log 'kopieren fertig: .*abgebrochen'
pruefe K4 "Esc" "Abbrechen: nichts geändert" 'grep -a -q "Antwort zu Auftrag [0-9]*: Abbrechen" "$LOG" && [ "$(cat "$T/b/x.txt")" = alt ] && [ "$(ls "$T/b" | wc -l)" = 5 ]' "$(ls "$T/b" | tr '\n' ' ')"
titel; hoch; hinein 0
strg a; q "keydown Control_L" "move $((450 + OX)) $(( $(zeile 0) + OY ))" "click" "keyup Control_L" "wait 400"; strg c
hoch; hinein 1
n=$(fragen); zklick 0; strg v; warte_frage "$n"
alt d; q "key Return" "wait 1000"; warte_log '0 erledigt, 3 übersprungen' 10
pruefe K5 "„Diese Aktion auf alle Dateien anwenden“ + Überspringen" "eine Frage, alles übersprungen" 'grep -a -q "Überspringen (für alle)" "$LOG" && [ "$(fragen)" = $((n + 1)) ] && [ "$(cat "$T/b/y.txt")" = alt ]' "$(grep -a 'fertig' "$LOG" | tail -1)"
titel; gehe_zu "$T"; zuruecksetzen; eingabe <<< "wait 1000"
gehe_zu "$T/a"; zklick 0; strg c; gehe_zu "$T/b"
einfuegen_frage
foto konflikt-ordner
alt z; n=$(fragen); warte_frage "$((n - 1))"
q "key Return" "wait 800"; warte_log 'kopieren fertig' 10
pruefe K6 "Zusammenführen, darin Überspringen" "q.txt kommt dazu, x.txt bleibt" '[ "$(cat "$T/b/m/q.txt")" = nur-a ] && [ "$(cat "$T/b/m/x.txt")" = alt-m ]' "$(grep -a 'Frage zu\|Antwort zu' "$LOG" | tail -4 | tr '\n' ' ' | cut -c1-200)"
