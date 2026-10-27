# Dateien 0.3.0: Rückgängig und Wiederholen (Strg+Z, Umschalt+Strg+Z) für
# Verschieben, Umbenennen, Papierkorb, Kopieren (in den Papierkorb), neuer
# Ordner; ein Stapel je Fenster über Reiter hinweg; endgültiges Löschen
# leert den Stapel (Eintrag inaktiv). Geprüft wird am Dateibaum und an den
# Logzeilen, die Fotos zeigen das Menü Bearbeiten mit der Beschreibung.
export QUERENCIA_DATEIEN_DEBUG=1
HEIM=$HOME
P="$HEIM/Probe"; Z="$P/Ziel"
mkdir -p "$Z"
for n in alpha beta gamma delta; do echo "$n" > "$P/$n.txt"; done
da() { [ -e "$1" ]; }
weg() { [ ! -e "$1" ]; }
ls_p() { (cd "$P" && find . -mindepth 1 | sort | tr '\n' ' '); }
waehle() { q "type $1" "wait 700" "key Escape" "wait 300"; }

starte "$P"
warte_log "Ordner gelesen" 15 || fall U00 "Start" 0 "keine Logzeile"
q "wait 500"
markiere
menue bearbeiten
foto 01-bearbeiten-leer
q "key Escape" "wait 300"

# U01 Verschieben: alpha.txt ausschneiden, in Ziel einfügen, Strg+Z
waehle alpha
taste ctrl+x
waehle Ziel
q "key Return" "wait 800"
warte_log "Ordner gelesen" 10
taste ctrl+v
warte_log "verschieben fertig" 10
warte_log "Rückgängig gemerkt" 5
da "$Z/alpha.txt" && weg "$P/alpha.txt" \
  && fall U01 "Verschieben nach Ziel" 1 "$(ls_p)" || fall U01 "Verschieben nach Ziel" 0 "$(ls_p)"
menue bearbeiten
foto 02-menue-rueckgaengig-verschieben
q "key Escape" "wait 300"
taste ctrl+z
warte_log "Rückgängig fertig" 10
q "wait 500"
da "$P/alpha.txt" && weg "$Z/alpha.txt" \
  && fall U02 "Strg+Z: alpha.txt wieder in Probe" 1 "$(ls_p)" || fall U02 "Strg+Z nach Verschieben" 0 "$(ls_p)"
menue bearbeiten
foto 03-menue-wiederholen-verschieben
q "key Escape" "wait 300"
taste shift+ctrl+z
warte_log "Wiederholt" 10
q "wait 500"
da "$Z/alpha.txt" && weg "$P/alpha.txt" \
  && fall U03 "Umschalt+Strg+Z: wieder in Ziel" 1 "$(ls_p)" || fall U03 "Wiederholen Verschieben" 0 "$(ls_p)"
taste ctrl+z
warte_log "Rückgängig fertig" 10
q "wait 500"
taste alt+Up
warte_log "Ordner gelesen" 10
q "wait 500"
da "$P/alpha.txt" && fall U04 "Nochmals Strg+Z, zurück in Probe" 1 "$(ls_p)" || fall U04 "Nochmals Strg+Z" 0 "$(ls_p)"

# U05 Umbenennen (F2) und zurück
waehle beta
q "key F2" "wait 400" "type neu" "key Return" "wait 800"
warte_log "Rückgängig gemerkt" 10
da "$P/neu.txt" && weg "$P/beta.txt" \
  && fall U05 "F2: beta.txt → neu.txt" 1 "$(ls_p)" || fall U05 "Umbenennen" 0 "$(ls_p)"
taste ctrl+z
warte_log "Rückgängig fertig" 10
q "wait 500"
da "$P/beta.txt" && weg "$P/neu.txt" \
  && fall U06 "Strg+Z: wieder beta.txt" 1 "$(ls_p)" || fall U06 "Strg+Z nach Umbenennen" 0 "$(ls_p)"

# U07 Papierkorb in einem zweiten Reiter, Reiter schließen, Strg+Z im ersten
taste ctrl+t
q "wait 800"
waehle gamma
q "key Delete" "wait 300"
warte_log "Rückgängig gemerkt" 10
weg "$P/gamma.txt" && fall U07 "Entf im zweiten Reiter: gamma.txt im Papierkorb" 1 "$(ls_p)" \
  || fall U07 "Papierkorb" 0 "$(ls_p)"
taste ctrl+w
q "wait 500"
menue bearbeiten
foto 04-menue-rueckgaengig-papierkorb
q "key Escape" "wait 300"
taste ctrl+z
warte_log "Rückgängig fertig" 10
q "wait 500"
da "$P/gamma.txt" && [ "$(cat "$P/gamma.txt")" = gamma ] \
  && fall U08 "Strg+Z im ersten Reiter: gamma.txt wiederhergestellt" 1 "$(ls_p)" \
  || fall U08 "Papierkorb rückgängig" 0 "$(ls_p) / $(grep -a 'Einträgen im Papierkorb' "$LOG" | tail -1)"

# U09 Kopieren: Strg+Z legt die Kopie ohne Rückfrage in den Papierkorb
TRASH="${XDG_DATA_HOME:-$HEIM/.local/share}/Trash/files"
waehle delta
taste ctrl+c
taste ctrl+v
warte_log "kopieren fertig" 10
warte_log "Rückgängig gemerkt" 5
KOPIE=$(cd "$P" && ls | grep '^delta (' | head -1)
[ -n "$KOPIE" ] && fall U09 "Kopie angelegt: $KOPIE" 1 "$(ls_p)" || fall U09 "Kopie" 0 "$(ls_p)"
menue bearbeiten
foto 05-menue-rueckgaengig-kopie
q "key Escape" "wait 300"
markiere
taste ctrl+z
warte_log "Rückgängig fertig" 10
q "wait 500"
! tail -n +"$((LOG_STAND + 1))" "$LOG" | grep -a -q "Frage" && weg "$P/$KOPIE" && da "$P/delta.txt" \
  && fall U10 "Strg+Z ohne Rückfrage: Kopie weg, Original bleibt" 1 "$(ls_p)" || fall U10 "Kopie rückgängig ohne Frage" 0 "$(ls_p)"
[ -e "$TRASH/$KOPIE" ] && [ "$(cat "$TRASH/$KOPIE")" = delta ] \
  && fall U11 "Die Kopie liegt im Papierkorb" 1 "$(ls "$TRASH" | tr '\n' ' ')" || fall U11 "Kopie im Papierkorb" 0 "$(ls "$TRASH" 2>&1 | tr '\n' ' ')"
taste shift+ctrl+z
warte_log "Wiederholt" 10
q "wait 500"
[ -n "$(cd "$P" && ls | grep '^delta (')" ] \
  && fall U12 "Wiederholen: Kopie wieder da" 1 "$(ls_p)" || fall U12 "Wiederholen Kopieren" 0 "$(ls_p)"

# U13 Neuer Ordner: Strg+Z löscht den leeren Ordner, Wiederholen legt ihn an
taste shift+ctrl+n
warte_log "Rückgängig gemerkt" 10
q "wait 500" "key Escape" "wait 500"
NEU=$(cd "$P" && find . -mindepth 1 -maxdepth 1 -type d ! -name Ziel | head -1)
[ -n "$NEU" ] && fall U13 "Neuer Ordner angelegt: $NEU" 1 "$(ls_p)" || fall U13 "Neuer Ordner" 0 "$(ls_p)"
taste ctrl+z
warte_log "Rückgängig fertig" 10
q "wait 500"
[ -z "$(cd "$P" && find . -mindepth 1 -maxdepth 1 -type d ! -name Ziel)" ] \
  && fall U14 "Strg+Z: leerer Ordner gelöscht" 1 "$(ls_p)" || fall U14 "Neuer Ordner rückgängig" 0 "$(ls_p)"
taste shift+ctrl+z
warte_log "Wiederholt" 10
q "wait 500"
[ -n "$(cd "$P" && find . -mindepth 1 -maxdepth 1 -type d ! -name Ziel)" ] \
  && fall U15 "Wiederholen: Ordner wieder da" 1 "$(ls_p)" || fall U15 "Wiederholen Neuer Ordner" 0 "$(ls_p)"

# U16 Endgültig löschen leert den Stapel: Strg+Z tut danach nichts
waehle beta
taste shift+Delete
q "key Tab" "wait 200" "key Return" "wait 300"
warte_log "Stapel geleert" 10
q "wait 500"
weg "$P/beta.txt" && fall U16 "Umschalt+Entf löscht beta.txt, Stapel geleert" 1 "$(ls_p)" \
  || fall U16 "Endgültig löschen" 0 "$(ls_p)"
menue bearbeiten
foto 06-menue-nach-loeschen
q "key Escape" "wait 300"
markiere
taste ctrl+z
q "wait 800"
VORHER=$(ls_p)
! warte_log "Rückgängig: " 1 && fall U17 "Strg+Z nach endgültigem Löschen: nichts" 1 "$VORHER" \
  || fall U17 "Strg+Z nach Löschen" 0 "$(grep -a 'Rückgängig' "$LOG" | tail -1)"
grep -a -E "CRITICAL|panicked" "$LOG" > /dev/null && fall U18 "Keine Warnung CRITICAL, kein Absturz" 0 "$(grep -a -E 'CRITICAL|panicked' "$LOG" | head -2 | tr '\n' ' ')" \
  || fall U18 "Keine Warnung CRITICAL, kein Absturz" 1 "programm.log"
