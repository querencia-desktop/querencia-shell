# Dateien 0.2.4, QD-173 P-2: Die Kompaktansicht kürzte gewöhnliche Namen
# („querencia-…“ bei 50 %). Die Grenze gleich breiter Spalten hängt nicht
# mehr vom Zoom ab (42 Zeichen der Sitzungsschrift, höchstens die sichtbare
# Breite); Namen darunter stehen in 50, 100 und 200 % ganz, längere werden
# mit „…“ gekürzt und stehen ganz im Tooltip.
export QUERENCIA_DATEIEN_DEBUG=1
HEIM=$HOME
OX=${LAUF_OX:-1}; OY=${LAUF_OY:-28}
MARKE=0
marke() { markiere; MARKE=$LOG_STAND; }
letzte() { tail -n +"$((MARKE + 1))" "$LOG" | grep -a -- "$1" | tail -n 1; }
fokus_lage() { grep -a ': Fokus Kompaktansicht: ' "$LOG" | tail -1 | sed -E 's/.*\| ([0-9-]+) ([0-9-]+) ([0-9]+) ([0-9]+)$/\1 \2 \3 \4/'; }
fokus_name() { grep -a ': Fokus Kompaktansicht: ' "$LOG" | tail -1 | sed -E 's/.*Kompaktansicht: (.*) \| .*/\1/'; }
# „Spaltenbreite W px (breitester Name B px, Grenze G px, Zoom Z %, …)“ → W B G Z
breiten() { letzte "Spaltenbreite" | sed -E 's/.*Spaltenbreite ([0-9]+) px \(breitester Name ([0-9]+) px, Grenze ([0-9]+) px, Zoom ([0-9]+) %.*/\1 \2 \3 \4/'; }

N="$HEIM/Namen"; mkdir -p "$N"
# die Namen aus Weg B Alpha 3 (L1-05) und ein längerer
for n in querencia-abend.png querencia-blau-2.png querencia-glut.png querencia-tiefblau.png \
  streifen.png querencia-abend-16x9-logo.png; do head -c 2000 /dev/zero > "$N/$n"; done
LN="$HEIM/Sehr lange Namen"; mkdir -p "$LN"
LANG_NAME="querencia-abend-16x9-mit-logo-fassung-fuer-sehr-breite-monitore.png"
for n in kurz.png querencia-tiefblau.png "$LANG_NAME"; do head -c 2000 /dev/zero > "$LN/$n"; done

starte "$N"
warte_log "Ordner gelesen" 15 || fall KN0 "Start" 0 "keine Logzeile"
marke
taste ctrl+3
warte_log "Ansicht Kompakt am Modell" 5
q "wait 800"
# Stufe ZOOM: Spalte so breit wie der breiteste Name (nicht an der Grenze),
# die Zelle des längsten Namens mindestens Name + Symbol
stufe() {
  local nr=$1 zoom=$2 symbol=$3 w b g z x y zb zh
  q "type querencia-abend-16" "wait 700" "key Escape" "wait 300"
  read -r w b g z < <(breiten)
  read -r x y zb zh < <(fokus_lage)
  eval "G$zoom=$g"
  [ "$z" = "$zoom" ] && [ "$(fokus_name)" = querencia-abend-16x9-logo.png ] && [ "$w" = "$b" ] \
    && [ "$b" -lt "$g" ] && [ "$zb" -ge "$((b + symbol))" ] \
    && fall "$nr" "Zoom $zoom %: Spalte = breitester Name ($b px < Grenze $g px), Zelle $zb px ≥ Name + Symbol, nichts gekürzt" 1 "Spaltenbreite $w, Zelle $x $y $zb $zh" \
    || fall "$nr" "Zoom $zoom %: nichts gekürzt" 0 "w=$w b=$b g=$g z=$z zelle=$x $y $zb $zh fokus=$(fokus_name)"
  q "move 900 700" "wait 200"
}
stufe KN1 50 24
foto 01-namen-50
marke
taste ctrl+plus; taste ctrl+plus
warte_log "Zoom Kompaktansicht 100 %" 5
q "wait 500"
stufe KN2 100 48
foto 02-namen-100
marke
taste ctrl+plus; taste ctrl+plus
warte_log "Zoom Kompaktansicht 200 %" 5
q "wait 500"
stufe KN3 200 96
foto 03-namen-200
[ -n "${G50:-}" ] && [ "$G50" = "${G100:-}" ] && [ "$G50" = "${G200:-}" ] \
  && fall KN4 "Grenze unabhängig vom Zoom: $G50 px in 50, 100 und 200 %" 1 "Grenze $G50/$G100/$G200" \
  || fall KN4 "Grenze unabhängig vom Zoom" 0 "${G50:-} ${G100:-} ${G200:-}"

# Sehr lange Namen: nur der Name über der Grenze wird gekürzt, Tooltip ganz
taste ctrl+0
q "wait 300"
marke
taste ctrl+l
q "type $LN" "key Return"
warte_log "Sehr lange Namen: lesen" 5
warte_log "Ordner gelesen" 5
q "wait 800"
read -r w b g z < <(breiten)
[ "$w" = "$g" ] && [ "$b" -gt "$g" ] \
  && fall KN5 "Längerer Name als die Grenze: Spalte = Grenze ($g px < $b px)" 1 "Spaltenbreite $w, Zoom $z %" \
  || fall KN5 "Spalte an der Grenze" 0 "w=$w b=$b g=$g z=$z"
q "type querencia-abend-16" "wait 700" "key Escape" "wait 300"
read -r x y zb zh < <(fokus_lage)
q "move $((OX + x + zb / 2)) $((OY + y + zh / 2))" "wait 300" "move $((OX + x + zb / 2 + 2)) $((OY + y + zh / 2))" "wait 1800"
foto 04-tooltip-langer-name
q "move 900 700" "wait 300"
q "type kurz" "wait 700" "key Escape" "wait 300"
foto 05-lange-namen-50
grep -a -E "CRITICAL|panicked" "$LOG" > /dev/null && fall KN6 "Keine Warnung CRITICAL, kein Absturz" 0 "$(grep -a -E 'CRITICAL|panicked' "$LOG" | head -2 | tr '\n' ' ')" \
  || fall KN6 "Keine Warnung CRITICAL, kein Absturz" 1 "programm.log"
