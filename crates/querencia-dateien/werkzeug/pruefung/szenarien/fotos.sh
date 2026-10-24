# Belegbilder nach B §5.3 (Stufe G für den Papierkorb): 01 Hauptfenster
# Symbolansicht, 02 Liste nach Größe, 04–06 Menüs Datei/Bearbeiten/Ansicht,
# 07/08 Kontextmenü Datei/Hintergrund, 09 Papierkorb mit Leiste und Frage,
# 13 Einstellungen (fünf Reiter), 14 Info, 15 Knopfpfad mit Überlauf.
# Fortschritt (10), Konflikt (11) und Eigenschaften (12) kommen aus den
# Läufen von B3 (tools/dateien-operationen-pruefung).
starte
warte_log "Ordner gelesen" 15
q "wait 600"
foto 01-hauptfenster-symbolansicht
for m in datei:04 bearbeiten:05 ansicht:06; do
  menue "${m%%:*}"
  foto "${m#*:}-menue-${m%%:*}"
  q "key Escape" "wait 200"
done
taste ctrl+l
q "type Dokumente" "key Return"
warte_log "Dokumente: lesen" 5; warte_log "Ordner gelesen" 5
taste ctrl+2
menue ansicht
q "key Up" "key Up" "key Up" "key Up" "key Up" "key Up" "key Right" "wait 300" "key Down" "key Return" "wait 400"
foto 02-liste-nach-groesse
menue ansicht
q "key Up" "key Up" "key Up" "key Up" "key Up" "key Up" "key Right" "wait 300" "key Return" "wait 300"
taste ctrl+1
q "wait 400" "move 215 270" "click" "wait 200" "rclick" "wait 500"
foto 07-kontextmenue-datei
q "key Escape" "wait 300" "move 650 470" "click" "wait 200" "rclick" "wait 500"
foto 08-kontextmenue-hintergrund
q "key Escape" "wait 300"
echo weg > "$HOME/Dokumente/alt.txt"; gio trash "$HOME/Dokumente/alt.txt"
taste ctrl+l
q "type trash:///" "key Return"
warte_log "trash:///: lesen" 5; warte_log "Ordner gelesen" 5
q "wait 500" "move 700 143" "click" "wait 800"
foto 09-papierkorb-leiste-frage
q "key Escape" "wait 300"
menue bearbeiten
q "key Up" "key Return" "wait 700"
foto 13a-einstellungen-ansichten
for n in 171:13b-verhalten 265:13c-anzeige 373:13d-listenspalten 486:13e-vorschau; do
  q "move ${n%%:*} 54" "click"
  foto "${n#*:}"
done
q "key Escape" "wait 300"
menue hilfe
q "key Up" "key Return" "wait 700"
foto 14-info
q "key Escape" "wait 300"
taste ctrl+l
q "type $HOME/Tief/ebene-01/ebene-02/ebene-03/ebene-04/ebene-05/ebene-06/ebene-07/ebene-08/ebene-09/ebene-10/ebene-11/ebene-12" "key Return"
warte_log "ebene-12: lesen" 5; warte_log "Ordner gelesen" 5
q "wait 500"
foto 15-knopfpfad-ueberlauf
