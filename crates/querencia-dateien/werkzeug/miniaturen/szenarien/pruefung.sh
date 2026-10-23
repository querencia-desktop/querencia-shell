# Welle C2: Prüffälle Miniaturen (M) und Suche (S) im Prüfrahmen Stufe L.
# Fotos unter fotos/, Ergebniszeilen in ergebnis.txt (fall NR TEXT 0|1 BELEG).
CP="python3 -I $LAUF_HIER/cache-pruefen.py $XDG_CACHE_HOME/thumbnails"
ok() { "$@" > "$OUT/beleg-$NR.txt" 2>&1; }
gehe() { taste ctrl+l; q "type $1" "key Return"; warte_log "$(basename "$1"): lesen" 10; warte_log "Ordner gelesen" 15; }
# the line warte_log found last
gefunden() { sed -n "${LOG_STAND}p" "$LOG"; }
zeile() { tail -n +"$((LOG_STAND + 1))" "$LOG" | grep -E "$1" | tail -1; }
suchen() { taste ctrl+a; q "type $1" "key Return"; warte_log "Suche [0-9]* fertig" 30; }
treffer() { gefunden | sed -E 's/.*fertig: ([0-9]+) Treffer.*/\1/'; }

starte "$HOME/Bilder/Fotos"
warte_log "Ordner gelesen" 15
NR=M01; warte_log "Miniaturen fertig" 60
z=$(grep "Miniaturen fertig" "$LOG" | tail -1)
[[ $z == *"200 erzeugt, 0 fehlgeschlagen"* ]] && fall M01 "200 Fotos bekommen Miniaturen (Symbolansicht 100 %, 64 px)" 1 "${z#*: }" || fall M01 "200 Fotos" 0 "$z"
q "wait 400"; foto M01-fotos-symbolansicht
NR=M02; ok $CP "$HOME"/Bilder/Fotos/*.jpg && fall M02 "Cache nach Spezifikation: normal/MD5(URI).png, Thumb::URI, Thumb::MTime, ≤128 px, 0600/0700" 1 "beleg-M02.txt (200 OK)" || fall M02 "Cache nach Spezifikation" 0 "beleg-M02.txt"
markiere; gehe "$HOME/Bilder/Gemischt"; warte_log "Miniaturen fertig" 30; q "wait 400"
foto M03-gemischt
NR=M03; ok $CP "$HOME"/Bilder/Gemischt/{bewegt.gif,bild.bmp,bild.tiff,bild.webp,halbdurchsichtig.png,kreis.svg} \
  && fall M03 "GIF, BMP, TIFF, WebP, PNG mit Alpha, SVG über GdkPixbuf/Thumbnailer" 1 "beleg-M03.txt" || fall M03 "Bildarten" 0 "beleg-M03.txt"
NR=M04; ok $CP --keine "$HOME/Bilder/Gemischt/rauschen-gross.png" && fall M04 "Datei über 10 MB (Vorgabe „Nur für Dateien kleiner als“) bekommt keine Miniatur" 1 "beleg-M04.txt, $(stat -c %s "$HOME/Bilder/Gemischt/rauschen-gross.png") Bytes" || fall M04 "Größengrenze" 0 "beleg-M04.txt"
NR=M05; ok $CP --fehl "$HOME/Bilder/Gemischt/kaputt.png" && fall M05 "Kaputtes Bild: Eintrag unter fail/querencia-dateien mit URI und MTime" 1 "beleg-M05.txt" || fall M05 "fail-Eintrag" 0 "beleg-M05.txt"
markiere; gehe "$HOME/Dokumente/PDF"; warte_log "Miniaturen fertig" 30; q "wait 400"
foto M06-pdf
NR=M06; ok $CP "$HOME"/Dokumente/PDF/*.pdf && fall M06 "PDF über einen Thumbnailer (%i %o %s; Stellvertreter qd-pdf.thumbnailer)" 1 "beleg-M06.txt" || fall M06 "PDF" 0 "beleg-M06.txt"
# Fremde Thumbnailer: außerhalb in bwrap (Unit-Test), in der Prüfhülle
# (kein verschachtelter Namensraum) in deren eigenem Sandkasten
NR=M11; z=$(grep "Thumbnailer (" "$LOG" | head -1)
[[ $z == *"Prüfhülle ohne eigenes bwrap"* ]] && fall M11 "Thumbnailer laufen nur im Sandkasten (hier: Prüfhülle ohne Netz; bwrap-Weg im Unit-Test)" 1 "${z#*: }" || fall M11 "Sandkasten" 0 "${z:-keine Zeile}"
# Zoom und Liste
markiere; gehe "$HOME/Bilder/Fotos"; q "wait 500"
taste ctrl+plus; q "wait 1500"; taste ctrl+plus; q "wait 1500"
NR=M07; z=$(grep "Miniaturen fertig" "$LOG" | tail -1)
[[ $z == *" 0 erzeugt"* && $z != *" 0 aus dem Cache"* ]] && fall M07 "Zoom 200 %: 128 px aus dem Cache, nichts neu erzeugt" 1 "${z#*: }" || fall M07 "Zoom 200 %" 0 "$z"
q "wait 400"; foto M07-zoom-200
taste ctrl+0; markiere; taste ctrl+2
NR=M08; warte_log "Miniaturen fertig" 30; z=$(gefunden)
[[ $z == *" 0 erzeugt"* ]] && fall M08 "Listenansicht: Miniaturen in Symbolgröße (24 px) aus dem Cache" 1 "${z#*: }" || fall M08 "Listenansicht" 0 "$z"
q "wait 400"; foto M08-liste
taste ctrl+1
# Abbruch beim Ordnerwechsel
cp -r "$HOME/Bilder/Fotos" "$HOME/Bilder/Abbruch"
markiere; gehe "$HOME/Bilder/Abbruch"; q "wait 150"; taste alt+Up; q "wait 1500"
NR=M09; z=$(zeile "nach Ordnerwechsel"); n=$(find "$XDG_CACHE_HOME/thumbnails/normal" -name '*.png' | wc -l)
zur=$(sed -E 's/.*: ([0-9]+) zurückgenommen.*/\1/' <<<"$z")
[[ -n $z && ${zur:-0} -gt 0 ]] && fall M09 "Ordnerwechsel nimmt offene Aufträge zurück" 1 "${z#*: }; normal/ hat $n Dateien" || fall M09 "Abbruch" 0 "${z:-keine Zeile}; $n"
# Einstellung zur Laufzeit: Vorschau → Miniaturen anzeigen: Nie (Tastatur im Dialog)
gehe "$HOME/Bilder/Fotos"; q "wait 600"
markiere
menue bearbeiten
q "key Up" "key Return" "wait 900"
q "move 486 54" "click" "wait 400"
foto M10a-einstellungen-vorschau
q "move ${M10_X:-560} ${M10_Y:-119}" "click" "wait 500"
foto M10b-einstellungen-wahl
q "key Down" "key Down" "key Down" "key Return" "wait 600"
foto M10c-einstellungen-nie
NR=M10; z=$(zeile "Regel Nie")
[[ -n $z ]] && fall M10 "Einstellung „Nie“ wirkt sofort auf offene Fenster" 1 "${z#*: }" || fall M10 "Einstellung zur Laufzeit" 0 "keine Zeile „Regel Nie“"
q "key Escape" "wait 500"
foto M10d-fotos-ohne-miniaturen
# Suche
gehe "$HOME/Suche"
markiere; taste ctrl+f
NR=S01; warte_log "Suchleiste geöffnet" 3 && fall S01 "Strg+F öffnet die Suchleiste über der Ansicht" 1 "Logzeile" || fall S01 "Strg+F" 0 "-"
foto S01-suchleiste
suchen rechnung; q "wait 400"
NR=S02; t=$(treffer); [ "$t" = 37 ] && fall S02 "„rechnung“: 37 Treffer in 10.092 Einträgen, Ortsspalte" 1 "$(gefunden | sed 's/^[^:]*: //')" || fall S02 "Treffer" 0 "$t"
foto S02-suchergebnisse
q "key Down" "key Delete" "wait 300" "key ctrl+c" "wait 200"
NR=S03; [ -e "$HOME/Suche/bereich-00/ordner-000/Rechnung-00000.pdf" ] && [ "$(find "$HOME/.local/share/Trash" -type f 2>/dev/null | wc -l)" = 0 ] \
  && fall S03 "Entf in den Suchergebnissen löscht nichts (Tasten des Ordners gesperrt)" 1 "Datei da, Papierkorb leer" || fall S03 "Entf gesperrt" 0 "-"
q "key Tab" "wait 100"
taste ctrl+f; taste ctrl+f
suchen "cafe"
NR=S04; t=$(treffer); [ "$t" = 1 ] && fall S04 "„cafe“ findet „Café-Übersicht.txt“ (NFD, wie 2.30)" 1 "$t Treffer" || fall S04 "Akzente" 0 "$t"
suchen "RECHNUNG 0027"
NR=S05; t=$(treffer); [ "$t" = 1 ] && fall S05 "Mehrere Wörter, Groß/klein egal: „RECHNUNG 0027“ → Rechnung-00271.pdf" 1 "$t Treffer" || fall S05 "Wörter" 0 "$t"
q "key Escape" "wait 300"
# verborgene nach Einstellung des Fensters
taste ctrl+h; q "wait 300"; taste ctrl+f
suchen rechnung
NR=S06; t=$(treffer); [ "$t" = 39 ] && fall S06 "Verborgene Dateien angezeigt (Strg+H): 39 Treffer mit .versteckt-…" 1 "$t" || fall S06 "Verborgene" 0 "$t"
q "key Escape" "wait 300"; taste ctrl+h
# Abbrechen: Esc gleich nach dem Start
taste ctrl+f
markiere; q "key ctrl+a" "type datei" "key Return" "key Escape"
warte_log "Suche [0-9]* fertig" 10
NR=S07; z=$(gefunden)
[[ $z == *angehalten* ]] && fall S07 "Esc hält eine laufende Suche an, Treffer bis dahin bleiben" 1 "${z#*: }" || fall S07 "Esc hält an" 0 "${z:-keine Zeile} (Suche vielleicht schneller als Esc)"
foto S07-angehalten
markiere; q "key Escape" "wait 400"
NR=S08; warte_log "Suche beendet" 3 && fall S08 "Zweites Esc beendet die Suche, die Ansicht kommt zurück" 1 "Logzeile" || fall S08 "Esc beendet" 0 "-"
foto S08-nach-esc
# Abbruch bei jeder Eingabe: Suche starten, sofort weitertippen
taste ctrl+f
markiere; q "key ctrl+a" "type datei" "key Return" "type x"
NR=S15; if warte_log "Suche bei Eingabe angehalten" 5; then
  warte_log "Suche [0-9]* fertig" 10; z=$(gefunden)
  [[ $z == *angehalten* ]] && fall S15 "Jede Eingabe ins Suchfeld hält die laufende Suche an" 1 "${z#*: }" || fall S15 "Eingabe hält an" 0 "$z"
else fall S15 "Eingabe hält an" 0 "keine Zeile (Suche vielleicht schneller als die Eingabe)"; fi
q "key Escape" "wait 300" "key Escape" "wait 300"
# Treffer öffnen: Ordner-Treffer mit Doppelklick
taste ctrl+f; suchen "ordner-05"
q "wait 300" "move 260 199" "dclick" "wait 300"
NR=S09; warte_log "ordner-050: lesen" 5 && fall S09 "Doppelklick auf einen Ordner-Treffer öffnet ihn (Suche endet)" 1 "Logzeile ordner-050: lesen" || fall S09 "Öffnen" 0 "-"
q "wait 500"
# Im Ordner anzeigen (Kontextmenü)
gehe "$HOME/Suche"
taste ctrl+f; suchen "Rechnung-00271"
q "wait 300" "move 260 199" "rclick" "wait 500"
foto S10a-kontextmenue
q "key Down" "key Down" "key Return" "wait 300"
NR=S10; warte_log "ordner-002: lesen" 5 && fall S10 "Kontextmenü „Im Ordner anzeigen“ öffnet den Ordner des Treffers" 1 "Logzeile ordner-002: lesen" || fall S10 "Im Ordner anzeigen" 0 "-"
q "wait 600"; foto S10b-im-ordner
# Werkzeugleiste und Menü
markiere; q "move 341 69" "click" "wait 300"
NR=S11; warte_log "Suchleiste geöffnet" 3 && fall S11 "Knopf „Suchen“ der Werkzeugleiste öffnet die Leiste" 1 "Logzeile" || fall S11 "Knopf" 0 "-"
foto S11-knopf
q "move 341 69" "click" "wait 300"
NR=S12; warte_log "Suche beendet" 3 && fall S12 "Zweiter Klick schließt sie" 1 "Logzeile" || fall S12 "Knopf zu" 0 "-"
menue gehe
q "wait 300"; foto S13-menue-gehe-zu
q "key Up" "key Return" "wait 300"
NR=S13; warte_log "Suchleiste geöffnet" 3 && fall S13 "Gehe zu → Suchen … öffnet die Leiste" 1 "Logzeile" || fall S13 "Menü" 0 "-"
q "key Escape" "wait 300"
# Erweiterte Suche: Querencia Dateisuche (Attrappe) mit Ordner und Name
gehe "$HOME/Suche"
taste ctrl+f; q "type rechnung"; taste alt+e; q "wait 800"
NR=S14; a=$(tr '\n' ' ' < "$HOME/dateisuche-aufruf.txt" 2>/dev/null)
[[ $a == "--ordner $HOME/Suche --name *rechnung* " ]] && fall S14 "„Erweiterte Suche …“ startet Querencia Dateisuche mit Ordner und Name" 1 "Aufruf: $a" || fall S14 "Erweiterte Suche" 0 "${a:-kein Aufruf}"
foto S14-erweiterte-suche
q "key Escape" "wait 300"
