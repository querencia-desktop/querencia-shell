# D22: Ziehen in der Ansicht, auf die Seitenleiste, auf den Knopfpfad; aus dem Papierkorb nur Verschieben.
source "$(dirname "$QD_SZ")/hilfen.sh"
T=$HOME/Test
mkdir -p "$T/a" "$T/b"; echo eins > "$T/x.txt"; echo zwei > "$T/y.txt"; echo drei > "$T/z.txt"
ln -s "$QD_PROFIL/grenze" "$T/grenze"
start_app "$T"
# Reihenfolge: a, b, grenze, x.txt, y.txt, z.txt
ziehe 3 1
warte_bis '[ -f "$T/b/x.txt" ] && [ ! -e "$T/x.txt" ]' 10
pruefe Z1 "ohne Taste, gleiches Dateisystem, auf Ordner b" "verschoben" '[ -f "$T/b/x.txt" ] && [ ! -e "$T/x.txt" ]' "$(grep -a 'Ablage:' "$LOG" | tail -1 | cut -c1-200)"
n=$(geladen); eingabe <<< "wait 1200"
# a, b, grenze, y.txt, z.txt
ziehe 3 0 Control_L
warte_bis '[ -f "$T/a/y.txt" ]' 10
pruefe Z2 "Strg, gleiches Dateisystem, auf Ordner a" "kopiert" '[ -f "$T/a/y.txt" ] && [ -f "$T/y.txt" ]' "$(grep -a 'Ablage:' "$LOG" | tail -1 | cut -c1-200)"
ziehe 4 2
warte_bis '[ -f "$QD_PROFIL/grenze/z.txt" ]' 10
pruefe Z3 "ohne Taste, anderes Dateisystem (tmpfs-Grenze)" "kopiert" '[ -f "$QD_PROFIL/grenze/z.txt" ] && [ -f "$T/z.txt" ]' "$(grep -a 'Ablage:' "$LOG" | tail -1 | cut -c1-200)"
rm "$QD_PROFIL/grenze/z.txt"
ziehe 4 2 Shift_L
warte_bis '[ -f "$QD_PROFIL/grenze/z.txt" ] && [ ! -e "$T/z.txt" ]' 10
pruefe Z4 "Umschalt, anderes Dateisystem" "verschoben (kopiert, dann gelöscht)" '[ -f "$QD_PROFIL/grenze/z.txt" ] && [ ! -e "$T/z.txt" ]' "$(grep -a 'Ablage:' "$LOG" | tail -1 | cut -c1-200)"
eingabe <<< "wait 1000"
# a, b, grenze, y.txt  → y.txt auf „Persönlicher Ordner“ in der Seitenleiste
ziehe_nach 450 "$(zeile 3)" 320 260
warte_bis '[ -f "$HOME/y.txt" ] && [ ! -e "$T/y.txt" ]' 10
pruefe Z5 "auf „Persönlicher Ordner“ in der Seitenleiste" "verschoben" '[ -f "$HOME/y.txt" ] && [ ! -e "$T/y.txt" ]' "$(grep -a 'Ablage:' "$LOG" | tail -1 | cut -c1-200)"
eingabe <<< "wait 1000"
foto ziehen-nachher
# a → Knopf „heim“ im Knopfpfad (Eltern)
ziehe_nach 450 "$(zeile 0)" 285 182
warte_bis '[ -d "$HOME/a" ] && [ ! -e "$T/a" ]' 10
pruefe Z6 "Ordner auf den Knopf des Elternordners" "verschoben" '[ -d "$HOME/a" ] && [ -f "$HOME/a/y.txt" ] && [ ! -e "$T/a" ]' "$(grep -a 'Ablage:' "$LOG" | tail -1 | cut -c1-200)"
# in den eigenen Ordner: nichts
eingabe <<< "wait 1000"
vor=$(ls "$T" | tr '\n' ' ')
ziehe_nach 450 "$(zeile 1)" 800 560
eingabe <<< "wait 800"
pruefe Z7 "auf die freie Fläche des eigenen Ordners" "nichts geändert" '[ "$(ls "$T" | tr "\n" " ")" = "$vor" ]' "$(grep -a 'Ablage:\|Ziehen beendet' "$LOG" | tail -2 | tr '\n' ' ' | cut -c1-200)"
# aus dem Papierkorb: nur Verschieben (Strg wird übergangen), .trashinfo geht mit
echo papier > "$T/weg.txt"; eingabe <<< "wait 1500"
n=$(geladen)
TR=$(papierkorb_ordner)
gio trash "$T/weg.txt"
aktion gehe-zu "'$(python3 -c 'import sys,pathlib;print(pathlib.Path(sys.argv[1]).as_uri())' "$TR/files")'"
warte_ordner "$n"
ziehe_nach 450 "$(zeile 0)" 320 260 Control_L
warte_bis '[ -f "$HOME/weg.txt" ]' 10
pruefe Z8 "aus dem Papierkorb mit Strg auf „Persönlicher Ordner“" "verschoben, .trashinfo weg" '[ -f "$HOME/weg.txt" ] && [ -z "$(ls -A "$TR/files")" ] && [ -z "$(ls -A "$TR/info")" ]' "$(grep -a 'Ziehen beginnt\|Ablage:' "$LOG" | tail -2 | tr '\n' ' ' | cut -c1-250)"
