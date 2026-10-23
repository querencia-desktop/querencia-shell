# Hilfen für Szenarien gegen querencia-dateien (headless labwc 1280x720).
# Thema Querencia-Klassisch (lauf.sh). Fenster 800x576 mittig: Inhalt ab (240,98);
# Titelleiste Mitte (640,85). Gemessen am 23.10. (Foto-Vermessung, B2-Ansicht):
# Listenansicht von B2 (Strg+2 beim Start): die erste Zeile liegt unter dem Spaltenkopf, den
# die Ansicht ins Log schreibt („Spaltenkopf name bei X Y B H“, Fensterkoordinaten); Zeilen 26 px;
# Ziehen und Ablegen treffen nur Symbol (x = 409) und Namen, Klicks zum Wählen rechts (x = 950).
# Seitenleiste „Orte“: Persönlicher Ordner (310,248); Knopfpfad: erster Knopf (285,176);
# Leiste im Papierkorb trash:///: „Papierkorb leeren …“ (937,214), Liste 39 px tiefer;
# Eigenschaften: Reiter Zugriffsrechte (489,228), Öffnen mit (611,228);
# Symbolansicht: Zelle i der ersten Reihe bei x = 455 + 129 i, Symbol y = 225.
PERSOENLICH="310 248"; PFAD_ERSTER="285 176"; LEEREN_KNOPF="937 214"; LEISTE_PK=39
REITER_RECHTE="489 228"; REITER_MIT="611 228"; SYM_Y=225
# Andere Auflösungen: das Fenster bleibt mittig, alle Fensterkoordinaten verschieben sich.
OX=$(( (W - 1280) / 2 )); OY=$(( (H - 720) / 2 ))
KOPF_UNTEN=219   # Bildschirm-y der Kopfunterkante bei 1280x720, aus dem Log neu gesetzt
zeile() { echo $((KOPF_UNTEN + 12 + 26 * $1 + ${LEISTE:-0})); }
kopf_lesen() {
  local z; z=$(grep -a 'Spaltenkopf name bei' "$LOG" | tail -1)
  [ -n "$z" ] && KOPF_UNTEN=$(awk '{print 98 + $(NF-2) + $NF}' <<< "$z")
  echo "Kopfunterkante $KOPF_UNTEN ($z)" >> "$QD_AUS/innen.log"
}
XZ=409
q() { printf '%s\n' "$@" | eingabe; }
strg() { q "keydown Control_L" "type $1" "keyup Control_L" "wait 400"; }
alt() { q "keydown Alt_L" "type $1" "keyup Alt_L" "wait 400"; }
klick() { q "move $(( $1 + OX )) $(( $2 + OY ))" "wait 200" "click" "wait 400"; }
zklick() { q "wait 300" "move $(( 950 + OX )) $(( $(zeile $1) + OY ))" "click" "wait 400"; }  # rechts: neben Fortschritts- und Dialogfenstern
doppel() { q "wait 300" "move $(( XZ + OX )) $(( $(zeile $1) + OY ))" "dclick" "wait 800"; }
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
    echo "move $(( $1 + OX )) $(( $2 + OY ))"; echo "wait 200"; echo "down left"; echo "wait 100"
    echo "glide $(( $1 + OX + 20 )) $(( $2 + OY + 10 )) 200"; echo "glide $(( $3 + OX )) $(( $4 + OY )) 500"; echo "wait 400"
    echo "up left"; echo "wait 400"
    [ -n "${5:-}" ] && echo "keyup $5"; } | eingabe
}
ziehe() { ziehe_nach "$XZ" "$(zeile $1)" "$XZ" "$(zeile $2)" "${3:-}"; }
papierkorb_ordner() { echo "${XDG_DATA_HOME:-$HOME/.local/share}/Trash"; }
start_app() { # ORDNER [symbole]: Listenansicht, mit „symbole“ die Symbolansicht
  starte "$1"; warte_log 'Ordner gelesen'; eingabe <<< "wait 1200"; titel
  if [ "${2:-}" != symbole ]; then strg 2; warte_log 'Spaltenkopf name bei' 5; eingabe <<< "wait 600"; kopf_lesen; fi
}
uri_von() { python3 -c 'import sys,pathlib;print(pathlib.Path(sys.argv[1]).as_uri())' "$1"; }
gehe_zu() { # PFAD
  local n; n=$(geladen); aktion gehe-zu "'$(uri_von "$1")'"; warte_ordner "$n"; titel
}
