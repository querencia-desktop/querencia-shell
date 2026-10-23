# Eigenschaften (Grundlegend mit Ordnergröße, Zugriffsrechte, Öffnen mit) über win.eigenschaften mit
# b1-anhaengen.patch (Integrationsprobe), Anlegen.
source "$(dirname "$QD_SZ")/hilfen.sh"
T=$HOME/Test; mkdir -p "$T/ordner/unter"
for i in $(seq 1 40); do head -c 1000 /dev/zero > "$T/ordner/unter/f$i"; done
printf '#!/bin/sh\necho hallo\n' > "$T/skript.sh"; chmod 644 "$T/skript.sh"
XD=${XDG_DATA_HOME:-$HOME/.local/share}; mkdir -p "$XD/applications"
printf '[Desktop Entry]\nType=Application\nName=Testanzeige\nExec=true %%u\nMimeType=text/plain;application/x-shellscript;\n' > "$XD/applications/testanzeige.desktop"
# GIO liest Starter eines Benutzerordners nur über dessen mimeinfo.cache (gemessen)
printf '[MIME Cache]\napplication/x-shellscript=testanzeige.desktop;\ntext/plain=testanzeige.desktop;\n' > "$XD/applications/mimeinfo.cache"
VORLAGEN=$HOME/Vorlagen; mkdir -p "$VORLAGEN"; echo "Sehr geehrte" > "$VORLAGEN/Brief.txt"
grep -q TEMPLATES "$XDG_CONFIG_HOME/user-dirs.dirs" 2>/dev/null || echo 'XDG_TEMPLATES_DIR="$HOME/Vorlagen"' >> "$XDG_CONFIG_HOME/user-dirs.dirs"
start_app "$T"
# ordner, skript.sh
zklick 0; q "keydown Alt_L" "key Return" "keyup Alt_L" "wait 800"
warte_log 'Ordnergröße: ' 10; eingabe <<< "wait 400"
foto eigenschaften-ordner
pruefe E1 "Alt+Eingabe auf einem Ordner" "Inhalt gezählt: 41 Objekte" 'grep -a -q "Ordnergröße: 41 Objekte, 4[0-9]* Bytes" "$LOG"' "$(grep -a 'Ordnergröße' "$LOG" | tail -1)"
q "key Escape" "wait 500"
titel; zklick 1; q "keydown Alt_L" "key Return" "keyup Alt_L" "wait 800"
warte_log 'Eigenschaften: skript.sh' 10; eingabe <<< "wait 400"
foto eigenschaften-datei
klick 552 221; foto eigenschaften-rechte
alt a
pruefe E2 "Zugriffsrechte: „Datei als Programm ausführen“" "Modus 644 → 755" '[ "$(stat -c %a "$T/skript.sh")" = 755 ]' "$(grep -a 'Zugriffsrechte' "$LOG" | tail -1 | cut -c1-150)"
klick 663 221; foto eigenschaften-oeffnen-mit
klick 430 346
pruefe E3 "Öffnen mit: Vim statt Testanzeige" "wird Standard (mimeapps.list)" 'grep -q "application/x-shellscript=vim.desktop" "${XDG_CONFIG_HOME:-$HOME/.config}/mimeapps.list"' "$(grep -a 'Standardprogramm' "$LOG" | tail -1)"
q "key Escape" "wait 500"
titel; zklick 0; q "keydown Control_L" "keydown Shift_L" "type n" "keyup Shift_L" "keyup Control_L" "wait 800"
warte_bis '[ -d "$T/Neuer Ordner" ]' 10
pruefe N1 "Umschalt+Strg+N" "„Neuer Ordner“" '[ -d "$T/Neuer Ordner" ]' "$(ls "$T" | tr '\n' ' ')"
q "key Escape" "wait 400"
titel; aktion dokument-anlegen "''"
warte_bis '[ -f "$T/Neues Dokument" ]' 10
pruefe N2 "Dokument anlegen ▸ Leere Datei" "„Neues Dokument“, leer" '[ -f "$T/Neues Dokument" ] && [ ! -s "$T/Neues Dokument" ]' "$(ls "$T" | tr '\n' ' ')"
q "key Escape" "wait 400"
titel; aktion dokument-anlegen "'$(uri_von "$VORLAGEN/Brief.txt")'"
warte_bis '[ -f "$T/Brief.txt" ]' 10
pruefe N3 "Dokument anlegen ▸ Vorlage Brief" "Kopie der Vorlage" '[ "$(cat "$T/Brief.txt")" = "Sehr geehrte" ]' "$(ls "$T" | tr '\n' ' ')"
q "key Escape" "wait 400"
titel; aktion ordner-anlegen
warte_bis '[ -d "$T/Neuer Ordner 2" ]' 10
pruefe N4 "zweiter neuer Ordner" "freier Name „Neuer Ordner 2“" '[ -d "$T/Neuer Ordner 2" ]' "$(ls "$T" | tr '\n' ' ')"
