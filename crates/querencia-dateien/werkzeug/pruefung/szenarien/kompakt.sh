# Welle C3 Kompaktansicht (B §5.2 B04): Strg+3, Spalten waagrecht, Pfeile
# folgen den Spalten, Zoom, Auswahl (Umschalt+Pfeil, Strg+A, Klick ins
# Freie), Mausrad blättert waagrecht, Tippsuche, Umbenennen (F2),
# Kontextmenü, Doppelklick, „Alle Spalten haben dieselbe Breite“ im Dialog
# (Reiter „Anzeige“) sofort und in dateien.ini.
export QUERENCIA_DATEIEN_DEBUG=1
HEIM=$HOME
INI=$XDG_CONFIG_HOME/querencia/dateien.ini
OX=${LAUF_OX:-1}; OY=${LAUF_OY:-28}
MARKE=0
marke() { markiere; MARKE=$LOG_STAND; }
neu_seit() { tail -n +"$((MARKE + 1))" "$LOG" | grep -a -q -- "$1"; }
letzte() { tail -n +"$((MARKE + 1))" "$LOG" | grep -a -- "$1" | tail -n 1; }
# „Fokus Kompaktansicht: NAME | x y b h“ → NAME bzw. x y b h
fokus_name() { grep -a ': Fokus Kompaktansicht: ' "$LOG" | tail -1 | sed -E 's/.*Kompaktansicht: (.*) \| .*/\1/'; }
fokus_lage() { grep -a ': Fokus Kompaktansicht: ' "$LOG" | tail -1 | sed -E 's/.*\| ([0-9-]+) ([0-9-]+) ([0-9]+) ([0-9]+)$/\1 \2 \3 \4/'; }
nummer() { fokus_name | sed -E 's/datei-0*([0-9]+)\.txt/\1/'; }

starte "$HEIM/Viele Dateien"
warte_log "Ordner gelesen" 15 || fall K00 "Start" 0 "keine Logzeile"
marke
taste ctrl+3
warte_log "Ansicht Kompakt am Modell" 5 && warte_log "breitester Name" 5 \
  && fall K01 "Strg+3: Kompaktansicht, breitester Name gemessen" 1 "$(grep -a 'breitester Name' "$LOG" | tail -1 | cut -d: -f3-)" \
  || fall K01 "Strg+3 Kompaktansicht" 0 "$(tail -n 3 "$LOG" | tr '\n' ' ')"
grep -q "Ansicht=symbole" "$INI" 2>/dev/null || ! grep -q "Ansicht=kompakt" "$INI" 2>/dev/null \
  || fall K01b "Strg+3 ändert die Vorgabe neuer Ordner nicht" 0 "dateien.ini"
q "wait 300"
foto 01-kompakt-50

# K02 Pfeile folgen den Spalten: ↓ im Spaltenlauf, → eine Spalte weiter
q "key Home" "wait 300"
n0=$(nummer)
q "key Down" "wait 200"
n1=$(nummer); read -r x1 y1 _ _ < <(fokus_lage)
q "key Down" "wait 200"
read -r x0 y0 _ _ < <(fokus_lage)
q "key Up" "wait 200"
q "key Right" "wait 200"
n2=$(nummer); read -r x2 y2 _ _ < <(fokus_lage)
zeilen=$((n2 - n1))
q "key Left" "wait 200"
n3=$(nummer)
q "key End" "wait 300"
n4=$(nummer)
[ "$n0" = 0 ] && [ "$n1" = 1 ] && [ "$x1" = "$x0" ] && [ "$y0" -gt "$y1" ] && [ "$zeilen" -gt 3 ] \
  && [ "$y2" = "$y1" ] && [ "$x2" -gt "$x1" ] && [ "$n3" = 1 ] && [ "$n4" = 199 ] \
  && fall K02 "Pfeile: ↓ eine Zeile, → eine Spalte ($zeilen Zeilen je Spalte), ←, Ende" 1 "0 → 1 ($x1,$y1; 2 bei $x0,$y0) → $n2 ($x2,$y2) → $n3; Ende $n4" \
  || fall K02 "Pfeile in Spalten" 0 "n=$n0 $n1 $n2 $n3 $n4 lage $x0,$y0 $x1,$y1 $x2,$y2"
foto 02-ende

# K03 Zoom: Stufen 50 → 200 %, darüber nicht, Strg+0 zurück auf 50 %
marke
for _ in 1 2 3 4 5; do taste ctrl+plus; done
q "wait 300"
foto 03-kompakt-200
z=$(grep -a -c "Zoom Kompaktansicht" <(tail -n +"$((MARKE + 1))" "$LOG"))
neu_seit "Zoom Kompaktansicht 200 % (96 px)" && [ "$z" = 4 ] && ok=1 || ok=0
taste ctrl+0
q "wait 300"
neu_seit "Zoom Kompaktansicht 50 % (24 px)" && [ "$ok" = 1 ] \
  && fall K03 "Zoom: 66/100/150/200 %, kein Schritt über 200 %, Strg+0 zurück auf 50 %" 1 "4 Stufen, 200 % (96 px), 50 % (24 px)" \
  || fall K03 "Zoom" 0 "z=$z $(grep -a 'Zoom Kompakt' "$LOG" | tail -n 3 | tr '\n' ' ')"
taste ctrl+plus
taste ctrl+plus
q "wait 300"
foto 04-kompakt-100
taste ctrl+0

# K04 Auswahl: Umschalt+→ wählt bis in die nächste Spalte, Strg+A alles
q "key Home" "wait 200"
marke
q "keydown Shift_L" "key Right" "keyup Shift_L" "wait 300"
a1=$(letzte "Auswahl Kompaktansicht" | sed -E 's/.*Kompaktansicht ([0-9]+):.*/\1/')
taste ctrl+a
q "wait 300"
a2=$(letzte "Auswahl Kompaktansicht" | sed -E 's/.*Kompaktansicht ([0-9]+):.*/\1/')
[ "$a1" = "$((zeilen + 1))" ] && [ "$a2" = 200 ] \
  && fall K04 "Auswahl: Umschalt+→ eine Spalte weit ($a1), Strg+A alle ($a2)" 1 "Auswahl $a1, Auswahl $a2" \
  || fall K04 "Auswahl" 0 "a1=$a1 (erwartet $((zeilen + 1))) a2=$a2"
foto 05-alles-gewaehlt

# K05 Mausrad blättert waagrecht
q "key Home" "wait 200"
marke
q "move 500 350" "scroll 3" "wait 400"
w=$(letzte "Kompaktansicht waagrecht" | sed -E 's/.*waagrecht: ([0-9-]+).*/\1/')
[ -n "$w" ] && [ "$w" -gt 0 ] \
  && fall K05 "Mausrad (3 Rasten) blättert waagrecht" 1 "waagrecht: $w px" \
  || fall K05 "Mausrad waagrecht" 0 "w=$w"
foto 06-geblaettert
q "scroll -6" "wait 200"

# K06 Tippsuche
marke
q "type datei-150" "wait 800"
[ "$(fokus_name)" = "datei-150.txt" ] \
  && fall K06 "Tippsuche „datei-150“ springt zum Namen" 1 "Fokus datei-150.txt" \
  || fall K06 "Tippsuche" 0 "$(fokus_name)"
q "key Escape" "wait 300"

# K07 Kontextmenü (Menütaste) auf der Auswahl
marke
q "key Menu" "wait 500"
foto 07-kontextmenue
neu_seit "Kontextmenü: " \
  && fall K07 "Menütaste: Kontextmenü der Auswahl" 1 "$(letzte 'Kontextmenü: ' | cut -d: -f3- | cut -c1-90)" \
  || fall K07 "Kontextmenü" 0 "-"
q "key Escape" "wait 300"

# K08 Umbenennen im Platz (F2), Name ohne Endung markiert
marke
q "key F2" "wait 400"
foto 08-umbenennen
q "type umbenannt" "key Return" "wait 800"
[ -e "$HEIM/Viele Dateien/umbenannt.txt" ] && [ ! -e "$HEIM/Viele Dateien/datei-150.txt" ] \
  && fall K08 "F2: „datei-150.txt“ → „umbenannt.txt“ (Endung bleibt)" 1 "$(letzte 'Umbenennen: ' | cut -d: -f3-)" \
  || fall K08 "Umbenennen" 0 "$(ls "$HEIM/Viele Dateien" | grep -c .) Dateien; $(letzte 'Umbenennen' | cut -d: -f3-)"

# K09 Doppelklick öffnet einen Ordner; Klick ins Freie hebt die Auswahl auf
taste ctrl+l
q "type $HEIM/Dokumente" "key Return"
warte_log "Dokumente: lesen" 5
warte_log "Ordner gelesen" 5
q "wait 400"
foto 09-dokumente-kompakt
# die Fokuszeile nach dem Layout (die erste kommt vor der Zuteilung, 0×0)
q "key Home" "wait 300" "key Down" "wait 200" "key Up" "wait 300"
read -r rx ry rb rh < <(fokus_lage)
marke
q "move $((OX + rx + 12)) $((OY + ry + rh / 2))" "click" "wait 300"
q "move 700 500" "click" "wait 400"
a3=$(letzte "Auswahl Kompaktansicht" | sed -E 's/.*Kompaktansicht ([0-9]+):.*/\1/')
marke
q "move $((OX + rx + 12)) $((OY + ry + rh / 2))" "dclick" "wait 600"
warte_log "Dokumente/Rechnungen: lesen" 5 && [ "$a3" = 0 ] \
  && fall K09 "Klick ins Freie hebt die Auswahl auf; Doppelklick öffnet „Rechnungen“" 1 "Auswahl 0; Rechnungen: lesen" \
  || fall K09 "Klick/Doppelklick" 0 "a3=$a3 Punkt $((OX + rx + 12)) $((OY + ry + rh / 2))"
taste alt+Up
warte_log "Ordner gelesen" 5

# K10 „Alle Spalten haben dieselbe Breite“: ein Ordner mit kurzen Namen in
# Spalte 1 und 3 und langen in Spalte 2 ($zeilen Zeilen je Spalte, K02)
G="$HEIM/Gemischt"; mkdir -p "$G"
for ((i = 0; i < zeilen; i++)); do
  n=$(printf '%02d' "$i")
  : > "$G/a-$n"; : > "$G/b-$n-ein recht langer Name in der zweiten Spalte.txt"; : > "$G/c-$n"
done
spalten_abstand() { # → Abstand Spalte 1→2 und 2→3 in px (Fokuslage)
  local xa xb xc
  q "key Home" "wait 200" "key Down" "wait 150" "key Up" "wait 200"; read -r xa _ _ _ < <(fokus_lage)
  q "key Right" "wait 200"; read -r xb _ _ _ < <(fokus_lage)
  q "key Right" "wait 200"; read -r xc _ _ _ < <(fokus_lage)
  echo "$((xb - xa)) $((xc - xb))"
}
taste ctrl+l
q "type $G" "key Return"
warte_log "Gemischt: lesen" 5
warte_log "Ordner gelesen" 5
q "wait 400"
read -r g1 g2 < <(spalten_abstand)
q "move 700 600" "wait 200"
foto 10a-gemischt-gleich-breit
marke
menue bearbeiten
q "key Up" "key Return" "wait 700"
q "move 268 54" "click" "wait 300"
foto 10b-dialog-anzeige
q "move 50 300" "click" "wait 400"
grep -q "KompaktGleichBreit=false" "$INI" && neu_seit "Kompaktansicht: Spalten je nach Inhalt" \
  && fall K10 "Dialog: „Alle Spalten haben dieselbe Breite“ aus – sofort, in dateien.ini" 1 "KompaktGleichBreit=false; Spalten je nach Inhalt" \
  || fall K10 "Gleiche Breite aus" 0 "$(grep Kompakt "$INI")"
q "key Escape" "wait 400"
read -r i1 i2 < <(spalten_abstand)
q "move 700 600" "wait 300"
foto 11-gemischt-nach-inhalt
[ "$g1" = "$g2" ] && [ "$i2" -gt "$((i1 + 40))" ] \
  && fall K10c "Spaltenbreiten: gleich breit $g1/$g2 px; nach Inhalt $i1/$i2 px (lange Namen ganz)" 1 "Fokuslage Spalte 1, 2, 3" \
  || fall K10c "Spaltenbreiten" 0 "gleich $g1/$g2, Inhalt $i1/$i2"
menue bearbeiten
q "key Up" "key Return" "wait 700"
q "move 268 54" "click" "wait 300" "move 50 300" "click" "wait 400" "key Escape" "wait 300"
grep -q "KompaktGleichBreit=true" "$INI" && fall K10b "Wieder an" 1 "KompaktGleichBreit=true" || fall K10b "Wieder an" 0 "dateien.ini"
taste ctrl+l
q "type $HEIM/Viele Dateien" "key Return"
warte_log "Viele Dateien: lesen" 5
warte_log "Ordner gelesen" 5

# K11 Strg+1/Strg+2/Strg+3 schalten um, Auswahl bleibt (ein Modell)
q "type datei-010" "wait 600" "key Escape" "wait 200"
marke
taste ctrl+2
q "wait 300"
taste ctrl+3
q "wait 300"
neu_seit "Auswahl" && fall K11 "Ansichtswechsel behält die Auswahl" 0 "$(letzte Auswahl)" \
  || fall K11 "Strg+2/Strg+3: Auswahl bleibt (keine neue Auswahlzeile)" 1 "datei-010.txt"
grep -a -E "CRITICAL|panicked" "$LOG" > /dev/null && fall K12 "Keine Warnung CRITICAL, kein Absturz" 0 "$(grep -a -E 'CRITICAL|panicked' "$LOG" | head -2 | tr '\n' ' ')" \
  || fall K12 "Keine Warnung CRITICAL, kein Absturz" 1 "programm.log"
