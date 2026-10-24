# Abnahme 0.1, Fälle von B1, die die anderen Szenarien nicht abdecken:
# D05 (Info zweimal), D09 („~“), D28 (Orte-Reihenfolge), D29 (Lesezeichen in
# Menü und Seitenleiste), D31 (Statusleiste), D26 (Leiste im Papierkorb, G).
starte
warte_log "Ordner gelesen" 15
q "wait 400"
foto 01-start-orte
# D09: „~“ in der Ansicht öffnet das Ortsfeld mit „~“; Eingabe → persönlicher Ordner
taste ctrl+l
q "type Dokumente" "key Return"
warte_log "Dokumente: lesen" 5
warte_log "Ordner gelesen" 5
markiere
q "wait 300" "key Escape" "type ~" "wait 300"
foto 02-tilde-ortsfeld
q "key Return"
warte_log "Ordner file://$HOME: lesen" 5 && fall D09b "„~“ öffnet das Ortsfeld, Eingabe geht nach Hause" 1 "programm.log" || fall D09b "„~“" 0 "-"
warte_log "Ordner gelesen" 5
markiere
q "wait 300" "key Escape" "type /" "wait 300"
foto 03-schraegstrich-ortsfeld
q "type usr" "key Return"
warte_log "Ordner file:///usr: lesen" 5 && fall D09a "„/“ öffnet das Ortsfeld mit „/“" 1 "programm.log: /usr" || fall D09a "„/“" 0 "-"
taste alt+Home
warte_log "Ordner gelesen" 5
# D31: Statusleiste ohne Auswahl, ein Objekt, mehrere, gemischt
q "wait 300"
foto 04-status-ohne-auswahl
taste ctrl+l
q "type Dokumente" "key Return"
warte_log "Dokumente: lesen" 5; warte_log "Ordner gelesen" 5
q "wait 300" "key Escape" "type notiz" "wait 400" "key Escape"
foto 05-status-ein-objekt
taste ctrl+a
foto 06-status-gemischt
# D05: Info zweimal, zweites Mal mit Titelleiste
menue hilfe
q "key Up" "key Return" "wait 700"
foto 07-info-1
q "key Return" "wait 400"
menue hilfe
q "key Up" "key Return" "wait 700"
foto 08-info-2-mit-titelleiste
q "key Escape" "wait 300"
# D29: Lesezeichen für „Rechnungen“ (Strg+D im Ordner), Menü und Seitenleiste
taste ctrl+l
q "type $HOME/Dokumente/Rechnungen" "key Return"
warte_log "Rechnungen: lesen" 5
taste ctrl+d
q "wait 400"
grep -q "Rechnungen" "$XDG_CONFIG_HOME/gtk-3.0/bookmarks" && fall D29b "Strg+D schreibt ~/.config/gtk-3.0/bookmarks" 1 "$(tail -n 1 "$XDG_CONFIG_HOME/gtk-3.0/bookmarks")" || fall D29b "Strg+D" 0 "bookmarks"
foto 09-lesezeichen-seitenleiste
menue lesezeichen
foto 10-lesezeichen-menue
q "key Escape"
python3 -I - "$XDG_CONFIG_HOME/gtk-3.0/bookmarks" > "$OUT/gtk-lesezeichen.txt" 2>&1 <<'PY'
# eine GTK-Dateiauswahl liest dieselbe Datei: Zeilen, die GTK 3/4 als Lesezeichen nimmt
import sys
for z in open(sys.argv[1], encoding="utf-8"):
    z = z.strip()
    if z and not z.startswith("#"):
        print(z.split(" ", 1)[0])
PY
grep -q "Rechnungen" "$OUT/gtk-lesezeichen.txt" && fall D29c "Datei im GTK-Format (URI je Zeile)" 1 "gtk-lesezeichen.txt" || fall D29c "GTK-Format" 0 "-"
if [ "${LAUF_STUFE:-L}" = G ]; then
  # D26 (Teil): Papierkorb über trash:/// mit Leiste
  echo inhalt > "$HOME/weg.txt"; gio trash "$HOME/weg.txt"
  taste ctrl+l
  q "type trash:///" "key Return"
  warte_log "Ordner trash:///: lesen" 5 && warte_log "Ordner gelesen" 5 && fall D26a "trash:/// mit Leiste „Papierkorb leeren …“" 1 "programm.log" || fall D26a "trash:///" 0 "-"
  q "wait 400"
  foto 11-papierkorb-leiste
fi
