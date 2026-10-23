# Welle C3 Reiter (B §5.2 B01/B02): Strg+T, Ort, Verlauf und Ansicht je
# Reiter, Strg+Bild↑/↓, Alt+Ziffer, Umschalt+Strg+Bild↑/↓, Mitteltaste auf
# Ordner, Pfadknopf und Ort der Seitenleiste, „In neuem Reiter öffnen“,
# Menü „Reiter“, Schließen per Knopf und Strg+W, Ordner weg im verdeckten
# Reiter, letzter Reiter schließt das Fenster; nichts bleibt übrig.
export QUERENCIA_DATEIEN_DEBUG=1
HEIM=$HOME
# Fensterinhalt auf dem Bildschirm: labwc-Rahmen (Regel MoveTo 0 0)
OX=${LAUF_OX:-1}; OY=${LAUF_OY:-28}
fokus_lage() { grep -a -E ': Fokus (Symbol|Listen|Kompakt)ansicht: ' "$LOG" | tail -1 | sed -E 's/.*\| ([0-9-]+) ([0-9-]+) ([0-9]+) ([0-9]+)$/\1 \2 \3 \4/'; }
# Bildschirmpunkt in der Mitte des Symbols der zuletzt fokussierten Zelle
fokus_punkt() { fokus_lage | awk -v ox="$OX" -v oy="$OY" '{ printf "%d %d", ox + $1 + $3 / 2, oy + $2 + $4 / 4 }'; }
# eigene Marke (warte_log setzt LOG_STAND hinter den Treffer)
MARKE=0
marke() { markiere; MARKE=$LOG_STAND; }
# Logzeile seit der Marke vorhanden?
neu_seit() { tail -n +"$((MARKE + 1))" "$LOG" | grep -a -q -- "$1"; }
letzte() { tail -n +"$((MARKE + 1))" "$LOG" | grep -a -- "$1" | tail -n 1; }

starte
warte_log "Ordner gelesen" 15 || fall R00 "Start" 0 "keine Logzeile"
q "wait 300"
foto 01-ein-reiter
grep -a -q "Reiter 1 neu: 1/1 file://$HEIM\$" "$LOG" && ! grep -a -q "Reiterleiste und Menü „Reiter“ an" "$LOG" \
  && fall R01 "Start: ein Reiter, keine Reiterleiste, kein Menü „Reiter“" 1 "Reiter 1 neu: 1/1, keine Zeile „an“" \
  || fall R01 "Start: ein Reiter ohne Leiste" 0 "$(grep -a 'Reiter' "$LOG" | head -3 | tr '\n' ' ')"

# R02 Strg+T: neuer Reiter für denselben Ort, wird gezeigt, Leiste erscheint
marke
taste ctrl+t
warte_log "Reiter 2 gezeigt: 2/2 symbole file://$HEIM\$" 5
neu_seit "Reiter 2 neu: 2/2 file://$HEIM\$" && neu_seit "Reiterleiste und Menü „Reiter“ an" \
  && fall R02 "Strg+T: Reiter 2 mit demselben Ort, gezeigt, Leiste und Menü erscheinen" 1 "Reiter 2 neu: 2/2, gezeigt 2/2, Leiste an" \
  || fall R02 "Strg+T" 0 "$(tail -n 5 "$LOG" | tr '\n' ' ')"
foto 02-zwei-reiter

# R03 Ort je Reiter
marke
taste ctrl+l
q "type $HEIM/Dokumente" "key Return"
warte_log "Ordner file://$HEIM/Dokumente: lesen" 5
warte_log "Ordner gelesen" 5
marke
taste ctrl+Page_Up
warte_log "Reiter 1 gezeigt: 1/2" 5
r1=$(letzte "Reiter 1 gezeigt")
marke
taste ctrl+Page_Down
warte_log "Reiter 2 gezeigt: 2/2" 5
r2=$(letzte "Reiter 2 gezeigt")
[[ $r1 == *"file://$HEIM" ]] && [[ $r2 == *"file://$HEIM/Dokumente" ]] \
  && fall R03 "Ort je Reiter: Reiter 1 Persönlicher Ordner, Reiter 2 Dokumente (Strg+Bild↑/↓)" 1 "${r1##*: } | ${r2##*: }" \
  || fall R03 "Ort je Reiter" 0 "$r1 | $r2"
foto 03-dokumente-im-zweiten

# R04 Verlauf je Reiter: Reiter 2 kann zurück, Reiter 1 nicht
marke
taste alt+Left
warte_log "Ordner file://$HEIM: lesen" 5 && z2=1 || z2=0
taste alt+Right
warte_log "Ordner file://$HEIM/Dokumente: lesen" 5
warte_log "Ordner gelesen" 5
taste ctrl+Page_Up
marke
taste alt+Left
q "wait 600"
neu_seit ": lesen" && z1=1 || z1=0
[ "$z2" = 1 ] && [ "$z1" = 0 ] \
  && fall R04 "Verlauf je Reiter: Alt+← wirkt in Reiter 2, Reiter 1 hat keinen Verlauf" 1 "Reiter 2 zurück und vor; Reiter 1 kein Ortswechsel" \
  || fall R04 "Verlauf je Reiter" 0 "z2=$z2 z1=$z1"

# R05 Ansicht je Reiter: Liste in Reiter 1, Reiter 2 bleibt bei Symbolen
taste ctrl+2
q "wait 300"
marke
taste ctrl+Page_Down
warte_log "Reiter 2 gezeigt" 5
a2=$(letzte "Reiter 2 gezeigt")
marke
taste ctrl+Page_Up
warte_log "Reiter 1 gezeigt" 5
a1=$(letzte "Reiter 1 gezeigt")
[[ $a2 == *" symbole file://"* ]] && [[ $a1 == *" liste file://"* ]] \
  && fall R05 "Ansicht je Reiter: Reiter 1 Liste, Reiter 2 Symbole" 1 "$(echo "$a1" | awk '{print $6}') | $(echo "$a2" | awk '{print $6}')" \
  || fall R05 "Ansicht je Reiter" 0 "$a1 | $a2"
foto 04-liste-im-ersten
taste ctrl+1
q "wait 300"

# R06 Mitteltaste auf einen Ordner: neuer Reiter dahinter, der aktuelle bleibt
q "key Escape"
marke
q "type Musik" "wait 700"
warte_log "Fokus Symbolansicht: Musik |" 5
read -r mx my < <(fokus_punkt)
q "key Escape" "wait 200"
marke
q "move $mx $my" "mclick" "wait 600"
warte_log "Reiter 3 neu: 2/3 file://$HEIM/Musik\$" 5 && ! neu_seit "Reiter 3 gezeigt" \
  && fall R06 "Mitteltaste auf Ordner „Musik“: Reiter 3 rechts vom aktuellen, bleibt dahinter" 1 "Reiter 3 neu: 2/3 …/Musik, nicht gezeigt (Punkt $mx $my)" \
  || fall R06 "Mitteltaste auf Ordner" 0 "Punkt $mx $my: $(tail -n 3 "$LOG" | tr '\n' ' ')"
foto 05-mitteltaste-ordner

# R07 Umschalt+Mitteltaste: neues Fenster statt Reiter (Nautilus 2.30)
marke
q "keydown Shift_L" "move $mx $my" "mclick" "keyup Shift_L" "wait 800"
warte_log "Ordner file://$HEIM/Musik: lesen" 5 && ! neu_seit "Reiter 4 neu: 2/" \
  && fall R07 "Umschalt+Mitteltaste: neues Fenster, kein Reiter" 1 "Reiter 4 neu: 1/1 (zweites Fenster)" \
  || fall R07 "Umschalt+Mitteltaste" 0 "$(tail -n 4 "$LOG" | tr '\n' ' ')"
taste ctrl+w
warte_log "Reiter 4 freigegeben" 5 \
  && fall R07b "Strg+W schließt das zweite Fenster (ein Reiter); Fenster und Reiter freigegeben" 1 "Reiter 4 freigegeben" \
  || fall R07b "Zweites Fenster freigegeben" 0 "$(tail -n 3 "$LOG" | tr '\n' ' ')"

# R08 Umschalt+Strg+O „In neuem Reiter öffnen“ auf der Auswahl (zwei Ordner)
marke
q "type Bilder" "wait 700"
q "key Escape" "wait 200"
taste shift+ctrl+o
warte_log "file://$HEIM/Bilder\$" 5
neu_seit "Reiter 5 neu: 2/4 file://$HEIM/Bilder" \
  && fall R08 "Umschalt+Strg+O: „Bilder“ in neuem Reiter neben dem aktuellen" 1 "Reiter 5 neu: 2/4 …/Bilder" \
  || fall R08 "Umschalt+Strg+O" 0 "$(tail -n 3 "$LOG" | tr '\n' ' ')"

# R09 Menü „Reiter“ (Alt+R): Liste der Reiter, „Nächster Reiter“
marke
taste alt+r
q "wait 400"
foto 06-menue-reiter
# „Vorheriger Reiter“ ist im ersten Reiter gesperrt: der erste Pfeil
# trifft „Nächster Reiter“
q "key Down" "key Return" "wait 400"
warte_log "Reiter 5 gezeigt: 2/4" 5 \
  && fall R09 "Menü „Reiter“ → Nächster Reiter" 1 "Reiter 5 gezeigt: 2/4" \
  || fall R09 "Menü Reiter" 0 "$(tail -n 3 "$LOG" | tr '\n' ' ')"

# R10 Alt+Ziffer
marke
taste alt+4
warte_log "Reiter 2 gezeigt: 4/4" 5 && ok1=1 || ok1=0
taste alt+1
warte_log "Reiter 1 gezeigt: 1/4" 5 && ok2=1 || ok2=0
marke
taste alt+9
q "wait 400"
neu_seit "gezeigt" && ok3=0 || ok3=1
[ "$ok1$ok2$ok3" = 111 ] && fall R10 "Alt+4, Alt+1; Alt+9 ohne neunten Reiter wirkt nicht" 1 "4/4, 1/4, kein Wechsel" \
  || fall R10 "Alt+Ziffer" 0 "$ok1$ok2$ok3"

# R11 Grenzen: Strg+Bild↑ im ersten Reiter wechselt nicht (kein Umlauf)
marke
taste ctrl+Page_Up
q "wait 400"
neu_seit "gezeigt" && fall R11 "Kein Umlauf am Anfang" 0 "$(tail -n 1 "$LOG")" \
  || fall R11 "Strg+Bild↑ im ersten Reiter: kein Umlauf" 1 "kein Wechsel"

# R12 Umschalt+Strg+Bild↓ verschiebt den Reiter nach rechts
marke
taste shift+ctrl+Page_Down
warte_log "Reiter 1 verschoben: 2/4" 5 \
  && fall R12 "Umschalt+Strg+Bild↓: Reiter 1 nach rechts (Position 2)" 1 "Reiter 1 verschoben: 2/4" \
  || fall R12 "Reiter verschieben" 0 "$(tail -n 2 "$LOG" | tr '\n' ' ')"
taste shift+ctrl+Page_Up
foto 07-vier-reiter

# R13 Ordner weg in einem verdeckten Reiter: der Reiter geht zum Elternordner
mkdir -p "$HEIM/Weg/innen"
marke
taste ctrl+t
warte_log "Reiter 6 gezeigt" 5
taste ctrl+l
q "type $HEIM/Weg/innen" "key Return"
warte_log "Weg/innen: lesen" 5
warte_log "Ordner gelesen" 5
taste alt+1
warte_log "Reiter 1 gezeigt" 5
marke
rm -rf "$HEIM/Weg/innen"
warte_log "Ordner file://$HEIM/Weg: lesen" 8 \
  && fall R13 "Ordner weg im verdeckten Reiter: der Reiter zeigt den Elternordner" 1 "Ordner …/Weg: lesen" \
  || fall R13 "Ordner weg im verdeckten Reiter" 0 "$(tail -n 3 "$LOG" | tr '\n' ' ')"

# R14 Strg+W schließt den aktuellen Reiter, der Reiter wird freigegeben
n=$(grep -a -c "Reiter [0-9]* neu" "$LOG")
marke
taste ctrl+w
warte_log "Reiter 1 geschlossen, 4 offen" 5 && w1=1 || w1=0
warte_log "Reiter 1 freigegeben" 5 && w2=1 || w2=0
[ "$w1$w2" = 11 ] && fall R14 "Strg+W schließt den aktuellen Reiter; Reiter freigegeben" 1 "Reiter 1 geschlossen, 4 offen; freigegeben" \
  || fall R14 "Strg+W" 0 "$w1$w2 $(tail -n 3 "$LOG" | tr '\n' ' ')"
foto 08-nach-strg-w

# R15 Schließen-Knopf im Reiterkopf (bei zwei Reitern: rechts im ersten Kopf)
taste ctrl+w
taste ctrl+w
q "wait 300"
grep -a -q "Reiter [0-9]* geschlossen, 2 offen" "$LOG" || fall R15a "Zwei Reiter übrig" 0 "$(tail -n 2 "$LOG" | tr '\n' ' ')"
foto 09-zwei-uebrig
marke
q "move ${LAUF_KNOPF:-457 145}" "click" "wait 600"
warte_log "geschlossen, 1 offen" 5 && k1=1 || k1=0
neu_seit "Reiterleiste und Menü „Reiter“ aus" && k2=1 || k2=0
[ "$k1$k2" = 11 ] && fall R15 "Schließen-Knopf im Kopf: Reiter zu, Leiste und Menü verschwinden bei einem Reiter" 1 "geschlossen, 1 offen; Leiste aus" \
  || fall R15 "Schließen-Knopf" 0 "$k1$k2 $(tail -n 3 "$LOG" | tr '\n' ' ')"
foto 10-wieder-einer

# R16 Mitteltaste auf Pfadknopf und auf einen Ort der Seitenleiste
marke
q "move ${LAUF_PFADKNOPF:-48 105}" "mclick" "wait 600"
warte_log "neu: 2/2 file://" 5 && p1=1 || p1=0
marke
q "move ${LAUF_ORT:-60 304}" "mclick" "wait 600"
warte_log "neu: 2/3 file://$HEIM/Musik\$" 5 && p2=1 || p2=0
[ "$p1$p2" = 11 ] && fall R16 "Mitteltaste auf Pfadknopf und auf „Musik“ in der Seitenleiste: je ein Reiter" 1 "neu 2/2, neu 2/3 …/Musik" \
  || fall R16 "Mitteltaste Pfadknopf/Seitenleiste" 0 "$p1$p2 $(grep -a 'neu:' "$LOG" | tail -n 2 | tr '\n' ' ')"
foto 11-pfad-und-ort

# R17 Der letzte Reiter schließt das Fenster; alle Reiter freigegeben
taste ctrl+w
taste ctrl+w
q "wait 300"
marke
taste ctrl+w
q "wait 1500"
lebt=0; kill -0 "$PROG_PID" 2>/dev/null && lebt=1
neu=$(grep -a -c "Reiter [0-9]* neu" "$LOG"); frei=$(grep -a -c "Reiter [0-9]* freigegeben" "$LOG")
[ "$lebt" = 0 ] && [ "$neu" = "$frei" ] \
  && fall R17 "Strg+W auf dem letzten Reiter schließt das Fenster; Programm endet, $frei von $neu Reitern freigegeben" 1 "Prozess beendet, neu=$neu frei=$frei" \
  || fall R17 "Letzter Reiter" 0 "lebt=$lebt neu=$neu frei=$frei"
grep -a -E "CRITICAL|panicked" "$LOG" > /dev/null && fall R18 "Keine Warnung CRITICAL, kein Absturz" 0 "$(grep -a -E 'CRITICAL|panicked' "$LOG" | head -2 | tr '\n' ' ')" \
  || fall R18 "Keine Warnung CRITICAL, kein Absturz" 1 "programm.log"
