# Querencia Dateien, Ansichten, Ablauf „pruefung“ (aus szenario.sh
# eingelesen): die Ansichtsfälle aus B §5.1 (D10–D12, D16, D18, D20 und die
# Bedienung der Ansichten: Tastatur, Tippsuche, Maus, Gummiband, Zoom,
# Umbenennen). Fenster maximiert und ohne Rahmen; jeder Fall prüft
# Logzeilen der Ansichten (QUERENCIA_DATEIEN_DEBUG=1) oder das Dateisystem
# des Profils. Fallnummern wie docs/dateien-ansichten-pruefung.md.
B=$BEISPIEL
G="$B/10000 Einträge"
fall() { # <nr> <text> <befehl…>
    local nr=$1 text=$2; shift 2
    if "$@"; then ok "$nr $text"; else nok "$nr $text"; fi
}
ist_fokus() { [ "$(fokus_name)" = "$1" ]; }
auswahl_ist() { [ "$(auswahl_zahl)" = "$1" ]; }
im_log() { neu_im_log "$1"; }
tippe() { q "type $1" "wait 250"; }
taste() { local k; for k in "$@"; do q "key $k" "wait 150"; done; }
strg() { q "keydown Control_L" "type $1" "keyup Control_L" "wait 300"; }
umschalt_taste() { q "keydown Shift_L" "key $1" "keyup Shift_L" "wait 300"; }
mitte_von() { # Spalte Zeile -> x y einer Zelle (Raster aus der Lage von Objekt 0)
    echo $((X0 + $1 * ZW + ZW / 2)) $((Y0 + $2 * ZH + ZH / 3))
}
klick() { q "move $1 $2" "click" "wait 250"; }

start "$B"
taste Right Right Home
read -r X0 Y0 ZW ZH < <(fokus_lage)
info "Raster: erste Zelle $X0,$Y0, Zelle ${ZW}×$ZH"
fall A01 "Start Symbolansicht 100 %: Ordner zuerst, „10000 Einträge“ vorn" ist_fokus "10000 Einträge"
foto a01-symbole-100

# --- A02 Tastatur (Pfeile, Pos1, Ende) -------------------------------------
taste Right Right
fall A02 "Pfeil rechts zweimal: Bilder" ist_fokus "Bilder"
taste Down
fall A02 "Pfeil runter: Objekt der Zeile darunter" bash -c '[ -n "$1" ] && [ "$1" != Bilder ]' _ "$(fokus_name)"
taste End
fall A02 "Ende: letztes Objekt" ist_fokus "Übersicht Ärger Größe.pdf"
taste Home
fall A02 "Pos1: erstes Objekt" ist_fokus "10000 Einträge"
fall A02 "Pfeiltasten wählen das Objekt mit" auswahl_ist 1

# --- A03 Tippsuche ---------------------------------------------------------
marke; tippe re
fall A03 "Tippen „re“ springt zu „Rechnungen 2026“ und wählt es" ist_fokus "Rechnungen 2026"
q "wait 1500"; tippe d
fall A03 "„red“ ohne Treffer: Fokus bleibt" ist_fokus "Rechnungen 2026"
fall A03 "Kästchen rot ohne Treffer" im_log "Tippsuche Symbolansicht: „red“ → kein Treffer"
foto a03-ohne-treffer
taste Escape; tippe da
fall A03 "neu „da“ → datei1.txt" ist_fokus "datei1.txt"
taste Down; fall A03 "Pfeil runter in der Suche → datei2.txt" ist_fokus "datei2.txt"
taste Down; fall A03 "noch einmal → datei10.txt" ist_fokus "datei10.txt"
taste Up; fall A03 "Pfeil hoch → datei2.txt" ist_fokus "datei2.txt"
foto a03-tippsuche 100
taste Escape

# --- A04 Rücktaste, Eingabe ------------------------------------------------
tippe abl; taste Escape
marke; taste Return
warte_log 5 'vollständig gezeigt'; q "wait 300"
fall A04 "Eingabe auf Ordner „Ablage“: geöffnet" im_log 'aktiviert: Ablage'
marke; taste BackSpace
warte_log 5 'vollständig gezeigt'; q "wait 400"
fall A04 "Rücktaste: Elternordner, „Ablage“ gewählt" ist_fokus "Ablage"

# --- A05 Maus: Klick, Umschalt, Strg, Klick ins Leere ----------------------
read -r X Y < <(mitte_von 0 0); klick "$X" "$Y"
fall A05 "Klick wählt ein Objekt" auswahl_ist 1
read -r X Y < <(mitte_von 3 0); q "move $X $Y" "keydown Shift_L" "click" "keyup Shift_L" "wait 250"
fall A05 "Umschalt+Klick wählt den Bereich (4)" auswahl_ist 4
read -r X Y < <(mitte_von 5 0); q "move $X $Y" "keydown Control_L" "click" "keyup Control_L" "wait 250"
fall A05 "Strg+Klick nimmt eines dazu (5)" auswahl_ist 5
q "wait 800" "keydown Control_L" "click" "keyup Control_L" "wait 250"
fall A05 "Strg+Klick noch einmal nimmt es weg (4)" auswahl_ist 4
q "move $((BX / 2)) $((BY - 150))" "click" "wait 300"
fall A05 "Klick ins Leere hebt die Auswahl auf" auswahl_ist 0
tippe datei1; taste Escape; umschalt_taste Right
fall A05 "Umschalt+Pfeil erweitert die Auswahl (2)" auswahl_ist 2
marke; q "key Return" "wait 300"
warte_bis 5 test -s "$OUT/geoeffnet.log"
fall A05 "Eingabe mit zwei gewählten Textdateien öffnet beide (Prüfprogramm)" bash -c "grep -q '/datei1.txt$' '$OUT/geoeffnet.log' && grep -q '/datei2.txt$' '$OUT/geoeffnet.log'"
q "move $((BX / 2)) $((BY - 150))" "click" "wait 300"

# --- A06 Gummiband (frei, Strg, Umschalt) ----------------------------------
read -r X2 Y2 < <(mitte_von 2 0)
q "move $((BX - 100)) $((BY - 150))" "down left" "glide $((BX / 2)) $((Y2 + 60)) 300" "glide $X2 $Y2 300" "wait 300"
foto a06-gummiband 100
q "up left" "wait 300"
G1=$(auswahl_zahl)
info "A06 Gummiband von freier Fläche: $(auswahl)"
fall A06 "Gummiband wählt die überstrichenen Objekte" bash -c '[ "${1:-0}" -gt 1 ]' _ "$G1"
read -r X3 Y3 < <(mitte_von 3 0)
q "move $((X3 + ZW / 2 - 4)) $((BY - 150))" "keydown Control_L" "down left" "glide $((X0 + 6)) $Y3 300" "up left" "keyup Control_L" "wait 300"
G2=$(auswahl_zahl)
info "A06 Strg+Gummiband: $G1 → $G2 ($(auswahl))"
fall A06 "Strg+Gummiband ändert die Auswahl, ohne sie zu ersetzen" bash -c '[ "$1" != "$2" ] && [ "$2" -gt 0 ]' _ "$G1" "$G2"
q "move $((BX / 2)) $((BY - 150))" "click" "wait 300"
read -r X Y < <(mitte_von 0 0); klick "$X" "$Y"
marke; q "move $((X3 + ZW / 2 - 4)) $((BY - 150))" "keydown Shift_L" "down left" "glide $((X0 + ZW + 6)) $Y3 300" "up left" "keyup Shift_L" "wait 300"
info "A06 Umschalt+Gummiband nach Klick auf Objekt 0: $(auswahl)"
fall A06 "Umschalt+Gummiband erweitert die Auswahl" bash -c '[ "${1:-0}" -gt 1 ]' _ "$(auswahl_zahl)"
fall A06 "Umschalt+Gummiband: eigenes Band (GTK startet mit Umschalt keines)" im_log "Umschalt+Gummiband Symbolansicht"
SICHTBAR=$(grep -a 'Objekte im Modell' "$LOG" | head -1 | sed -E 's/.*: ([0-9]+) Objekte im Modell.*/\1/')
strg a
fall A06 "Strg+A wählt alles ($SICHTBAR sichtbare Objekte)" auswahl_ist "$SICHTBAR"
q "move $((BX / 2)) $((BY - 150))" "click" "wait 300"

# --- A07 Zoom (D10) --------------------------------------------------------
tippe beri; taste Escape
marke; strg +
fall A07 "Strg++: 150 % (72 px)" im_log 'Zoom Symbolansicht 150 % (72 px)'
marke; strg +
fall A07 "Strg++: 200 % (96 px)" im_log 'Zoom Symbolansicht 200 % (96 px)'
foto a07-zoom-200
marke; strg +
fall A07 "Strg++ an der Grenze: bleibt 200 %" bash -c "! tail -n +$((MARKE + 1)) '$LOG' | grep -a -q 'Zoom Symbolansicht'"
marke; strg 0
fall A07 "Strg+0: 100 %" im_log 'Zoom Symbolansicht 100 % (48 px)'
marke; strg -; strg -
fall A07 "Strg+- zweimal: 50 % (24 px)" im_log 'Zoom Symbolansicht 50 % (24 px)'
foto a07-zoom-50
marke; q "move $((BX / 2)) $((BY / 2))" "keydown Control_L" "scroll -1" "wait 300" "keyup Control_L"
fall A07 "Strg+Mausrad hoch: 66 %" im_log 'Zoom Symbolansicht 66 % (32 px)'
strg 0; q "wait 300"
fall A07 "Fokus bleibt beim Zoomen auf „bericht.final.odt“" ist_fokus "bericht.final.odt"

# --- A08 Sortierung, Fokus bleibt (D12) -----------------------------------
marke; aktion anordnen groesse; q "wait 400"
fall A08 "Nach Größe: Fokus bleibt auf „bericht.final.odt“" ist_fokus "bericht.final.odt"
taste Home; taste Right Right Right Right Right Right Right Right
fall A08 "Nach Größe: nach den acht Ordnern die größte Datei (film.mp4, 50 MB)" ist_fokus "film.mp4"
marke; aktion umgekehrt; q "wait 400"
taste Home
fall A08 "Umgekehrt: Ordner bleiben vorn, in umgekehrter Folge („Verknüpfung zu Dokumente“)" ist_fokus "Verknüpfung zu Dokumente"
aktion umgekehrt; aktion anordnen name; q "wait 400"
taste Home
fall A08 "zurück nach Name: „10000 Einträge“ vorn" ist_fokus "10000 Einträge"

# --- A09 Umbenennen (D20) ---------------------------------------------------
tippe beri; taste Escape
marke; taste F2
fall A09 "F2 auf bericht.final.odt: „bericht.final“ markiert (13 Zeichen)" im_log 'Umbenennen begonnen Symbolansicht: bericht.final.odt (markiert Some((0, 13)))'
foto a09-umbenennen
taste Escape; q "wait 300"
fall A09 "Esc bricht ab" im_log 'Umbenennen abgebrochen: bericht.final.odt'
[ -e "$B/bericht.final.odt" ] && ok "A09 Datei unverändert nach Esc" || nok "A09 Datei unverändert nach Esc"
marke; umschalt_taste F2
fall A09 "Umschalt+F2: ganzer Name markiert (17 Zeichen)" im_log 'markiert Some((0, 17)))'
taste Escape
marke; taste F2; tippe "a/b"; taste Return; q "wait 600"
fall A09 "Name mit „/“ abgelehnt mit Hinweis" im_log 'Umbenennen abgelehnt: Der Name „a/b.odt“ ist ungültig'
foto a09-ungueltig
taste Return; q "wait 400"
marke; taste F2; q "keydown Control_L" "type a" "keyup Control_L" "key BackSpace" "wait 200"; taste Return; q "wait 600"
fall A09 "leerer Name abgelehnt" im_log 'Umbenennen abgelehnt: '
taste Return; q "wait 400"
marke; taste F2; tippe "bericht.neu"; taste Return
warte_bis 5 test -e "$B/bericht.neu.odt"
fall A09 "Umbenennen „bericht.final.odt“ → „bericht.neu.odt“" test -e "$B/bericht.neu.odt"
q "wait 500"
fall A09 "Fokus auf dem umbenannten Objekt" ist_fokus "bericht.neu.odt"
marke; umschalt_taste F2; tippe "datei1.txt"; taste Return; q "wait 600"
info "A09 vorhandener Name: $(zeile 'Umbenennen abgelehnt' | sed 's/.*abgelehnt: //')"
fall A09 "vorhandener Name abgelehnt" im_log 'wird in diesem Ordner bereits verwendet'
taste Return; q "wait 300"

# --- A10 langsamer zweiter Klick ------------------------------------------
tippe noti; taste Escape; q "wait 900"
read -r FX FY FW FH < <(fokus_lage)
marke; q "move $((FX + FW / 2)) $((FY + FH - 10))" "click" "wait 1200"
fall A10 "langsamer Klick auf den Namen des gewählten Objekts: Umbenennen" im_log 'langsamer zweiter Klick: Umbenennen'
taste Escape; q "wait 300"
read -r X Y < <(mitte_von 1 0)
marke; q "move $X $Y" "dclick" "wait 900"
fall A10 "Doppelklick öffnet, ohne Umbenennen" bash -c "! tail -n +$((MARKE + 1)) '$LOG' | grep -a -q 'Umbenennen begonnen'"
marke; taste BackSpace; warte_log 5 'vollständig gezeigt'; q "wait 300"

# --- A11 mittlere Taste, „/“ -----------------------------------------------
read -r X Y < <(mitte_von 1 0)
marke; q "move $X $Y" "down middle" "up middle" "wait 400"
fall A11 "mittlere Taste auf Ordner: neues Fenster" im_log 'aktiviert (neues Fenster): Ablage'
# das neue Fenster (Gerüst) hat den Fokus; schließen, zurück ins erste
q "wait 1500"; foto a11-neues-fenster 100; strg w; q "wait 800"
q "move $((BX / 2)) $((BY - 150))" "click" "wait 300"
marke; tippe /
fall A11 "„/“ öffnet das Ortsfeld" im_log 'Ortsfeld mit „/“'
taste Escape; q "move $((BX / 2)) $((BY - 150))" "click" "wait 300"

# --- A12 Kontextmenüs (D16) -------------------------------------------------
tippe noti; taste Escape
read -r FX FY FW FH < <(fokus_lage)
marke; q "move $((FX + FW / 2)) $((FY + 20))" "rclick" "wait 600"
foto a12-menue-datei
M=$(zeile 'Kontextmenü:' | sed 's/.*Kontextmenü: //')
# gesperrte Einträge stehen in [ ]; Aktionen von B3 fehlen vor dem Zusammenbau
MO=$(printf '%s' "$M" | tr -d '[]')
info "A12 Datei: $M"
fall A12 "Datei: Öffnen mit ▸, Ausschneiden, Kopieren, Umbenennen …, Papierkorb, Eigenschaften" bash -c \
    'case $1 in *"Öffnen mit ▸"*"Ausschneiden"*"Kopieren"*"Umbenennen …"*"In den Papierkorb verschieben"*"Eigenschaften"*) exit 0 ;; esac; exit 1' _ "$MO"
taste Escape; q "wait 300"
tippe rechn; taste Escape
read -r FX FY FW FH < <(fokus_lage)
marke; q "move $((FX + FW / 2)) $((FY + 20))" "rclick" "wait 600"
M=$(zeile 'Kontextmenü:' | sed 's/.*Kontextmenü: //')
# gesperrte Einträge stehen in [ ]; Aktionen von B3 fehlen vor dem Zusammenbau
MO=$(printf '%s' "$M" | tr -d '[]')
info "A12 Ordner: $M"
fall A12 "Ordner: Öffnen, In neuem Fenster öffnen, In Ordner einfügen" bash -c \
    'case $1 in "Öffnen In neuem Fenster öffnen"*"In Ordner einfügen"*) exit 0 ;; esac; exit 1' _ "$MO"
taste Escape; q "wait 300"
read -r X Y < <(mitte_von 0 1); klick "$X" "$Y"
read -r X2 Y2 < <(mitte_von 2 1); q "move $X2 $Y2" "keydown Control_L" "click" "keyup Control_L" "wait 300"
marke; q "rclick" "wait 600"
M=$(zeile 'Kontextmenü:' | sed 's/.*Kontextmenü: //')
# gesperrte Einträge stehen in [ ]; Aktionen von B3 fehlen vor dem Zusammenbau
MO=$(printf '%s' "$M" | tr -d '[]')
info "A12 Mehrfachauswahl: $M"
fall A12 "Mehrfachauswahl: „Umbenennen …“ ausgegraut" bash -c 'case $1 in *"[Umbenennen …]"*) exit 0 ;; esac; exit 1' _ "$M"
fall A12 "Mehrfachauswahl: Auswahl bleibt (2)" auswahl_ist 2
taste Escape; q "wait 300"
marke; q "move $((BX / 2)) $((BY - 150))" "rclick" "wait 600"
foto a12-menue-hintergrund
M=$(zeile 'Kontextmenü:' | sed 's/.*Kontextmenü: //')
# gesperrte Einträge stehen in [ ]; Aktionen von B3 fehlen vor dem Zusammenbau
MO=$(printf '%s' "$M" | tr -d '[]')
info "A12 Hintergrund: $M"
fall A12 "Hintergrund: Ordner anlegen, Dokument anlegen ▸, Objekte anordnen ▸, Einfügen, Eigenschaften" bash -c \
    'case $1 in "Ordner anlegen Dokument anlegen ▸"*"Objekte anordnen ▸"*"Einfügen"*"Eigenschaften"*) exit 0 ;; esac; exit 1' _ "$MO"
fall A12 "Rechtsklick ins Leere hebt die Auswahl auf" auswahl_ist 0
taste Escape; q "wait 300"
marke; tippe noti; taste Escape; umschalt_taste F10; q "wait 500"
fall A12 "Umschalt+F10 öffnet das Menü der Auswahl" im_log 'Kontextmenü: '
taste Escape; q "wait 300"

# --- A13 Ordner anlegen und gleich umbenennen (D18) ------------------------
# „Ordner anlegen“ (win.ordner-anlegen) liefert B3; geprüft wird hier nur
# der Weg des Gerüsts: neue Datei erscheint → umbenennen_anstossen.
info "A13 Ordner anlegen: Aktion von B3, Prüfung nach dem Zusammenbau"

# --- A14 Umbenennen über das Scrollen hinweg (Mietvertrag) ----------------
marke; tippe 10000; taste Return; warte_log 10 'vollständig gezeigt'; q "wait 400"
tippe "foto 00003"; taste Escape
fall A14 "Tippsuche mit Leerzeichen und Ziffern: „Foto 00003.txt“" ist_fokus "Foto 00003.txt"
marke; taste F2; q "wait 300"
q "move $((BX / 2)) $((BY / 2))"; for i in 1 2 3 4 5 6; do q "scroll 10" "wait 80"; done; q "wait 400"
tippe x
fall A14 "Feld kommt nach dem Scrollen zurück, Text bleibt" im_log 'Umbenennen: Feld zurückgeholt, Text x'
taste Escape; q "wait 300"

# --- A15 Listenansicht (D11, D12) ------------------------------------------
marke; taste BackSpace; warte_log 5 'vollständig gezeigt'; q "wait 300"
marke; strg 2; q "wait 800"
fall A15 "Strg+2: Listenansicht, Fokus bleibt auf „10000 Einträge“" ist_fokus "10000 Einträge"
foto a15-liste
read -r KX KY KW KH < <(grep -a 'Spaltenkopf groesse bei' "$LOG" | tail -1 | sed -E 's/.* bei ([0-9-]+) ([0-9-]+) ([0-9]+) ([0-9]+)$/\1 \2 \3 \4/')
info "A15 Spaltenkopf Größe bei $KX,$KY ${KW}×$KH"
marke; klick $((KX + KW / 2)) $((KY + KH / 2)); q "wait 500"
fall A15 "Klick auf „Größe“: nach Größe, aufsteigend" im_log 'Spaltenkopf groesse: Sortierung groesse umgekehrt false'
foto a15-liste-groesse
marke; klick $((KX + KW / 2)) $((KY + KH / 2)); q "wait 500"
fall A15 "zweiter Klick: umgekehrt" im_log 'Spaltenkopf groesse: Sortierung groesse umgekehrt true'
marke; aktion anordnen name; q "wait 500"
fall A15 "Menü „Nach Name“: Pfeil folgt (kein Spaltenkopf-Ereignis)" bash -c "! tail -n +$((MARKE + 1)) '$LOG' | grep -a -q 'Spaltenkopf name:'"
aktion umgekehrt; q "wait 300"
taste Home; taste Down Down
fall A15 "Liste: Pfeil runter zweimal → Bilder" ist_fokus "Bilder"
tippe "bild."; taste Escape
fall A15 "Liste: Tippsuche „bild.“ → bild.png" ist_fokus "bild.png"
marke; taste F2
fall A15 "Liste: F2 auf bild.png markiert „bild“" im_log 'Umbenennen begonnen Listenansicht: bild.png (markiert Some((0, 4)))'
tippe "grafik"; taste Return; warte_bis 5 test -e "$B/grafik.png"
fall A15 "Liste: umbenannt in grafik.png" test -e "$B/grafik.png"
marke; strg +
fall A15 "Liste: Strg++ 66 % (32 px)" im_log 'Zoom Listenansicht 66 % (32 px)'
strg 0
if [ "$BY" -ge 1000 ]; then
    q "move $((BX / 2)) $((BY - 100))" "click" "wait 300"
    fall A15 "Liste: Klick ins Leere hebt die Auswahl auf" auswahl_ist 0
    marke; q "rclick" "wait 600"
    fall A15 "Liste: Rechtsklick ins Leere zeigt das Hintergrundmenü" im_log 'Kontextmenü: \[*Ordner anlegen'
else
    # 28 Zeilen füllen die Liste bei 768 px Höhe: keine freie Fläche
    info "A15 Liste: keine freie Fläche bei ${BX}×$BY, Auswahl per Strg+Klick aufgehoben"
    read -r FX FY FW FH < <(fokus_lage)
    q "move $((FX + 40)) $((FY + FH / 2))" "keydown Control_L" "click" "keyup Control_L" "wait 300"
    fall A15 "Liste: Strg+Klick nimmt die Zeile aus der Auswahl" auswahl_ist 0
    marke; umschalt_taste F10; q "wait 600"
    fall A15 "Liste: Umschalt+F10 ohne Auswahl zeigt das Hintergrundmenü" im_log 'Kontextmenü: \[*Ordner anlegen'
fi
taste Escape
marke; strg 1; q "wait 500"; taste Right; q "wait 300"
fall A15 "Strg+1 zurück zur Symbolansicht" im_log 'Fokus Symbolansicht: '
stopp

# --- A17 Pfeil im Spaltenkopf einer später angelegten Liste ---------------
start "$B"
marke; aktion anordnen groesse; q "wait 400"
marke; strg 2; q "wait 800"
fall A17 "Liste nach Sortierung in der Symbolansicht angelegt: Pfeil auf „Größe“" im_log 'Spaltenkopf: Pfeil groesse'
foto a17-liste-ein-pfeil
stopp

# --- A16 Zusatzzeilen und Spalten nach Einstellung -------------------------
ini "[Ansichten]" "ZoomSymbole=150" "[Anzeige]" "Zusatz=datum" "ZusatzGross=typ" \
    "[Listenspalten]" "Groesse=true" "Typ=true" "Datum=true" "Rechte=true" "Eigentuemer=true" "Gruppe=true"
start "$B"
fall A16 "Zoom 150 % aus dateien.ini" im_log 'Zoom Symbolansicht 150 %'
foto a16-zusatzzeilen-150
strg 2; q "wait 800"
fall A16 "Liste mit Zugriffsrechten, Eigentümer, Gruppe" bash -c "grep -a -q 'Spaltenkopf gruppe bei' '$LOG' && grep -a -q 'Spaltenkopf rechte bei' '$LOG'"
foto a16-liste-alle-spalten
stopp
