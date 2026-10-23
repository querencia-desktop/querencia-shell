# Hilfen für Szenarien gegen querencia-dateien (headless labwc 1280x720).
# Fenster 800x576 mittig: Inhalt ab (240,98); Titelleiste Mitte (640,85).
# Ersatzansicht bzw. Liste: Zeile n bei y = 229 + 52 n, x = 450.
# Seitenleiste „Orte“: Persönlicher Ordner (320,260), Dateisystem (320,285), Papierkorb (315,310).
# Knopfpfad: erster Knopf (285,182), zweiter (367,182).
zeile() { echo $((229 + 52 * $1)); }
q() { printf '%s\n' "$@" | eingabe; }
strg() { q "keydown Control_L" "type $1" "keyup Control_L" "wait 400"; }
alt() { q "keydown Alt_L" "type $1" "keyup Alt_L" "wait 400"; }
klick() { q "move $1 $2" "wait 200" "click" "wait 400"; }
zklick() { q "wait 300" "move 950 $(zeile $1)" "click" "wait 400"; }  # rechts: neben Fortschritts- und Dialogfenstern
doppel() { q "wait 300" "move 450 $(zeile $1)" "dclick" "wait 800"; }
titel() { klick 640 85; }
# Fensteraktion über den privaten Sitzungsbus der Hülle (derselbe Weg wie Menü und Tasten)
aktion() { # NAME [GVARIANT-PARAMETER]
  local p='[]'; [ -n "${2:-}" ] && p="[<$2>]"
  gdbus call --session --dest org.querencia.Dateien --object-path /org/querencia/Dateien/window/1 \
    --method org.gtk.Actions.Activate "$1" "$p" '{}' >> "$QD_AUS/innen.log" 2>&1
  eingabe <<< "wait 500"
}
geladen() { zahl_log 'Ordner gelesen'; }
# Wartet, bis das Fenster den Ordner neu gelesen hat (nach Wechseln).
warte_ordner() { # ANZAHL_VORHER
  warte_bis '[ "$(geladen)" -gt '"$1"' ]' 15; eingabe <<< "wait 600"
}
fragen() { zahl_log "Frage zu Auftrag"; }
warte_frage() { # ANZAHL_VORHER
  warte_bis '[ "$(fragen)" -gt '"$1"' ]' 10; q "wait 800"
}
ziehe_nach() { # X1 Y1 X2 Y2 [Taste]
  { [ -n "${5:-}" ] && echo "keydown $5"
    echo "move $1 $2"; echo "wait 200"; echo "down left"; echo "wait 100"
    echo "glide $(( $1 + 20 )) $(( $2 + 10 )) 200"; echo "glide $3 $4 500"; echo "wait 400"
    echo "up left"; echo "wait 400"
    [ -n "${5:-}" ] && echo "keyup $5"; } | eingabe
}
ziehe() { ziehe_nach 450 "$(zeile $1)" 450 "$(zeile $2)" "${3:-}"; }
papierkorb_ordner() { echo "${XDG_DATA_HOME:-$HOME/.local/share}/Trash"; }
start_app() { # ORDNER
  starte "$1"; warte_log 'Ordner gelesen'; eingabe <<< "wait 1200"; titel
}
uri_von() { python3 -c 'import sys,pathlib;print(pathlib.Path(sys.argv[1]).as_uri())' "$1"; }
gehe_zu() { # PFAD
  local n; n=$(geladen); aktion gehe-zu "'$(uri_von "$1")'"; warte_ordner "$n"; titel
}
