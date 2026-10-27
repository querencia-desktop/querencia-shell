# Dateien 0.3.0: Suche „Auch Dateiinhalt“. Ohne Schalter nur Namen; mit
# Schalter auch Textdateien (keine Binärdateien), Spalte „Fundstelle“,
# Statusleiste mit Zeile; eine Suche ohne Treffer meldet 0.
export QUERENCIA_DATEIEN_DEBUG=1
HEIM=$HOME
status() { grep -a ': Statusleiste: ' "$LOG" | tail -1 | sed -E 's/.*: Statusleiste: //'; }
if [ "${LAUF_SPRACHE:-de}" = en ]; then ZEILE="Line 2: Die Zahlung ist eingegangen."; else ZEILE="Zeile 2: Die Zahlung ist eingegangen."; fi
P="$HEIM/Probe"; mkdir -p "$P/unter"
printf 'Hallo,\nDie Zahlung ist eingegangen.\nGruß\n' > "$P/brief.txt"
printf '# Notiz\n\nZAHLUNG offen bis Freitag\n' > "$P/unter/notiz.md"
printf 'Zahlung\000\001\002' > "$P/bild.bin"
printf 'nichts davon\n' > "$P/anderes.txt"
head -c 2000 /dev/zero > "$P/zahlungen.ods"

starte "$P"
warte_log "Ordner gelesen" 15 || fall I00 "Start" 0 "keine Logzeile"
q "wait 500"
taste ctrl+f
warte_log "Suchleiste geöffnet" 5
q "type zahlung" "key Return"
warte_log "fertig: 1 Treffer" 15 \
  && fall I01 "Ohne Schalter: nur der Name zahlungen.ods" 1 "$(grep -a 'fertig:' "$LOG" | tail -1 | cut -c1-160)" \
  || fall I01 "Ohne Schalter: nur Namen" 0 "$(grep -a 'fertig:' "$LOG" | tail -1 | cut -c1-160)"
grep -a 'fertig:' "$LOG" | tail -1 | grep -q ' 0 Inhalte gelesen' \
  && fall I02 "Ohne Schalter wird kein Inhalt gelesen" 1 "0 Inhalte" || fall I02 "Ohne Schalter kein Inhalt" 0 "$(grep -a 'fertig:' "$LOG" | tail -1)"
foto 01-nur-namen
# Schalter: Tab vom Feld, Leertaste; die gezeigte Suche läuft neu
q "key Tab" "wait 200" "key space" "wait 300"
warte_log "Dateiinhalt an" 5 || fall I03 "Schalter" 0 "keine Logzeile"
warte_log "fertig: 3 Treffer" 15 \
  && fall I03 "Mit Schalter: 3 Treffer (Name, brief.txt, notiz.md)" 1 "$(grep -a 'fertig:' "$LOG" | tail -1 | cut -c1-160)" \
  || fall I03 "Mit Schalter: 3 Treffer" 0 "$(grep -a 'fertig:' "$LOG" | tail -1 | cut -c1-160)"
grep -a 'Fundstelle: .*brief.txt Zeile 2: Die Zahlung ist eingegangen.' "$LOG" > /dev/null \
  && grep -a 'Fundstelle: .*notiz.md Zeile 3: ZAHLUNG offen bis Freitag' "$LOG" > /dev/null \
  && ! grep -a 'Fundstelle: .*bild.bin' "$LOG" > /dev/null \
  && fall I04 "Fundstellen mit Zeile; bild.bin (Nullbyte) nicht" 1 "$(grep -a 'Fundstelle:' "$LOG" | sed 's/.*Fundstelle: //' | tr '\n' ' ' | cut -c1-200)" \
  || fall I04 "Fundstellen" 0 "$(grep -a 'Fundstelle:' "$LOG" | tr '\n' ' ' | cut -c1-200)"
foto 02-mit-inhalt
# Treffer brief.txt wählen: Statusleiste mit Zeile
taste shift+Tab
q "key Down" "wait 300" "key Down" "wait 300" "key Up" "wait 500"
[[ $(status) == *"$ZEILE"* ]] \
  && fall I05 "Auswahl brief.txt: Statusleiste mit „$ZEILE“" 1 "$(status)" \
  || fall I05 "Statusleiste mit Zeile" 0 "$(status)"
foto 03-treffer-mit-zeile
# eine Suche ohne Treffer
q "key Escape" "wait 300"
taste ctrl+f
warte_log "Suchleiste geöffnet" 5
q "type nirgends" "key Return"
warte_log "fertig: 0 Treffer" 15 \
  && fall I06 "„nirgends“ mit Schalter: 0 Treffer" 1 "$(grep -a 'fertig:' "$LOG" | tail -1 | cut -c1-160)" \
  || fall I06 "Suche ohne Treffer" 0 "$(grep -a 'fertig:' "$LOG" | tail -1 | cut -c1-160)"
grep -a 'Suche [0-9]*: „nirgends“' "$LOG" | tail -1 | grep -q 'Inhalt true' \
  && fall I07 "Schalter bleibt für die nächste Suche an" 1 "$(grep -a 'Suche [0-9]*: „nirgends“' "$LOG" | tail -1 | sed 's/.*Suche //')" \
  || fall I07 "Schalter bleibt an" 0 "$(grep -a 'Suche [0-9]*: „nirgends“' "$LOG" | tail -1)"
foto 04-ohne-treffer
q "key Escape" "wait 300"
grep -a -E "CRITICAL|panicked" "$LOG" > /dev/null && fall I08 "Keine Warnung CRITICAL, kein Absturz" 0 "$(grep -a -E 'CRITICAL|panicked' "$LOG" | head -2 | tr '\n' ' ')" \
  || fall I08 "Keine Warnung CRITICAL, kein Absturz" 1 "programm.log"
