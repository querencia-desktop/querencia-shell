# Einstellungen (fünf Reiter), Eigenschaften, Info; Fotos.
starte
warte_log "Ordner gelesen" 15
foto 01-hauptfenster
menue bearbeiten
q "key Up" "key Return" "wait 700"
foto 02-einstellungen-ansichten
# Reiter anklicken (Dialog oben links, labwc-Regel MoveTo 0 0)
for n in 171:03-verhalten 265:04-anzeige 373:05-listenspalten 486:06-vorschau; do
  q "move ${n%%:*} 54" "click"
  foto "${n#*:}"
done
q "key Escape" "wait 300"
q "key Down" "key Down"
taste alt+Return
q "wait 700"
foto 07-eigenschaften
q "key Escape" "wait 300"
menue hilfe
q "key Up" "key Return" "wait 700"
foto 08-info
q "key Escape"
