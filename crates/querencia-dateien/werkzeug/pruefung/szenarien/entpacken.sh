# Dateien 0.3.0: „Hier entpacken“ im Kontextmenü von Archiven (mit
# LAUF_ARCHIV=<querencia-archiv>). Ein zip mit zwei Dateien bekommt einen
# Ordner mit seinem Namen; das Menü einer Textdatei hat den Eintrag nicht.
export QUERENCIA_MENU_DEBUG=1 QUERENCIA_DATEIEN_DEBUG=1
HEIM=$HOME
if [ "${LAUF_SPRACHE:-de}" = en ]; then EINTRAG="Extract Here"; else EINTRAG="Hier entpacken"; fi
P="$HEIM/Probe"; mkdir -p "$P"
python3 -I -c '
import sys, zipfile
with zipfile.ZipFile(sys.argv[1] + "/paket.zip", "w") as z:
    z.writestr("eins.txt", "eins\n")
    z.writestr("zwei.txt", "zwei\n")
' "$P"
echo "notiz" > "$P/notiz.txt"
waehle() { q "type $1" "wait 700" "key Escape" "wait 300"; }
menue_zeile() { grep -a 'Kontextmenü: ' "$LOG" | tail -1 | sed 's/.*Kontextmenü: //'; }

starte "$P"
warte_log "Ordner gelesen" 15 || fall E00 "Start" 0 "keine Logzeile"
q "wait 500"
[ -n "${QUERENCIA_ARCHIV:-}" ] || fall E00 "LAUF_ARCHIV gesetzt" 0 "ohne Archivprogramm"

# E01 Textdatei: kein „Hier entpacken“
waehle notiz
markiere
q "key Menu" "wait 500"
warte_log "Kontextmenü: " 3
[[ $(menue_zeile) != *"$EINTRAG"* ]] \
  && fall E01 "Textdatei: kein „$EINTRAG“" 1 "$(menue_zeile | cut -c1-160)" || fall E01 "Textdatei ohne Eintrag" 0 "$(menue_zeile)"
q "key Escape" "wait 300"

# E02 zip: der Eintrag nach der Öffnen-Gruppe
waehle paket
markiere
q "key Menu" "wait 500"
warte_log "Kontextmenü: " 3
[[ $(menue_zeile) == *"$EINTRAG"* ]] \
  && fall E02 "zip: „$EINTRAG“ im Kontextmenü" 1 "$(menue_zeile | cut -c1-200)" || fall E02 "zip mit Eintrag" 0 "$(menue_zeile)"
foto 01-kontextmenue-zip
# zum Eintrag gehen (höchstens acht Schritte): das Menü protokolliert bei
# jeder Taste den Zustand davor, also steht „aktiv=…“ erst beim nächsten
# Schritt im Log – dann einen zurück, Enter
for ((i = 0; i < 8; i++)); do
  q "key Down" "wait 200"
  tail -n +"$((LOG_STAND + 1))" "$LOG" | grep -a -q "aktiv=$EINTRAG" && { q "key Up" "wait 200"; break; }
done
q "key Return" "wait 300"
warte_log "Hier entpacken: " 5
ok=0
for ((i = 0; i < 100; i++)); do
  [ -f "$P/paket/eins.txt" ] && [ -f "$P/paket/zwei.txt" ] && { ok=1; break; }
  q "wait 100"
done
[ "$ok" = 1 ] && [ "$(cat "$P/paket/zwei.txt")" = zwei ] \
  && fall E03 "Enter: Querencia Archiv entpackt nach Probe/paket/" 1 "$(cd "$P" && find . -mindepth 1 | sort | tr '\n' ' ')" \
  || fall E03 "Entpacken" 0 "$(cd "$P" && find . -mindepth 1 | sort | tr '\n' ' ') / $(grep -a 'Hier entpacken' "$LOG" | tail -2 | tr '\n' ' ')"
for ((i = 0; i < 100; i++)); do grep -a -q 'Hier entpacken: fertig' "$LOG" && break; q "wait 100"; done
grep -a -q 'Hier entpacken: fertig' "$LOG" \
  && fall E04 "Archiv endet mit Erfolg" 1 "$(grep -a 'Hier entpacken: ' "$LOG" | tail -1)" \
  || fall E04 "Archiv-Ende" 0 "$(grep -a 'Hier entpacken' "$LOG" | tail -1)"
q "wait 800"
foto 02-entpackt
grep -a -E "CRITICAL|panicked" "$LOG" > /dev/null && fall E05 "Keine Warnung CRITICAL, kein Absturz" 0 "$(grep -a -E 'CRITICAL|panicked' "$LOG" | head -2 | tr '\n' ' ')" \
  || fall E05 "Keine Warnung CRITICAL, kein Absturz" 1 "programm.log"
