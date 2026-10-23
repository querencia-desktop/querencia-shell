# Start im persönlichen Ordner, Menüs der Leiste mit Alt+Buchstabe (D03).
starte
warte_log "Ordner gelesen" 15 && fall D01a "Start im persönlichen Ordner" 1 "programm.log: Ordner gelesen" \
  || fall D01a "Start im persönlichen Ordner" 0 "keine Logzeile"
q "wait 500"
foto 01-hauptfenster
for m in datei bearbeiten ansicht gehe lesezeichen hilfe; do
  menue "$m"
  foto "menue-$m"
  q "key Escape"
done
