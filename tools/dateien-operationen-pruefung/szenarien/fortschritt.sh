# D23: Fortschrittsfenster „Dateien werden kopiert …“, zwei Vorgänge, Abbrechen räumt auf.
source "$(dirname "$QD_SZ")/hilfen.sh"
T=$HOME/Test; mkdir -p "$T/a"
python3 - "$T" <<'P'
import os, sys
t = sys.argv[1]
for name, n in (("viele", 80000), ("viele2", 80000), ("riesig", 80000)):
    d = os.path.join(t, name); os.makedirs(d)
    for i in range(n):
        open(os.path.join(d, f"datei-{i:05d}.txt"), "w").close()
with open(os.path.join(t, "viele", "gross.bin"), "wb") as f:
    f.write(os.urandom(50 * 1024 * 1024))
P
kopieren_einfuegen() { zklick "$1"; strg c; strg v; }
abbrechen_klick() { # AUFTRAG – Fenster mittig (labwc), Titelleiste 26 px, Lage beim ersten Zeigen
  local z x y w h0 sx sy
  z=$(grep -a "Abbrechen für Auftrag $1 bei" "$LOG" | tail -1)
  x=$(sed -E 's/.* bei ([0-9-]+),([0-9-]+) .*/\1/' <<< "$z"); y=$(sed -E 's/.* bei ([0-9-]+),([0-9-]+) .*/\2/' <<< "$z")
  w=$(sed -E 's/.*Fenster ([0-9]+)x([0-9]+).*/\1/' <<< "$z"); h0=$(sed -E 's/.*Fenster ([0-9]+)x([0-9]+).*/\2/' <<< "$z")
  sx=$(( (W - w) / 2 + x )); sy=$(( (H - h0 - 26) / 2 + 26 + y ))
  echo "Abbrechen $1 → $sx,$sy" >> "$QD_AUS/innen.log"
  q "move $sx $sy" "wait 100" "click" "wait 300"
}
start_app "$T"
# a, riesig, viele, viele2 – beide Kopien in einem Zug, bevor das Fortschrittsfenster
# erscheint (es nimmt die Tastatur; ein Klick ins Hauptfenster höbe dieses darüber)
q "move $((950 + OX)) $(( $(zeile 2) + OY ))" "click" "wait 100" \
  "keydown Control_L" "type c" "type v" "keyup Control_L" "wait 50" \
  "move $((950 + OX)) $(( $(zeile 3) + OY ))" "click" "wait 100" \
  "keydown Control_L" "type c" "type v" "keyup Control_L"
warte_bis '[ "$(zahl_log "Fortschritt gezeigt")" -ge 2 ]' 20
eingabe <<< "wait 300"
foto fortschritt-zwei
pruefe F1 "zwei Kopien gleichzeitig" "Fenster „Dateien werden kopiert …“ mit zwei Zeilen" '[ "$(zahl_log "Fortschritt gezeigt")" -ge 2 ]' "$(grep -a 'Fortschritt gezeigt' "$LOG" | tr '\n' ' ' | cut -c1-200)"
warte_bis '[ "$(zahl_log "kopieren fertig: 1 erledigt")" -ge 2 ]' 180
pruefe F2 "beide Kopien fertig" "80001 bzw. 80000 Objekte, Inhalt gleich, Fenster zu" '[ "$(ls -A "$T/viele (Kopie)" | wc -l)" = 80001 ] && [ "$(ls -A "$T/viele2 (Kopie)" | wc -l)" = 80000 ] && cmp -s "$T/viele/gross.bin" "$T/viele (Kopie)/gross.bin" && grep -a -q "Fortschrittsfenster geschlossen" "$LOG"' "$(ls "$T" | tr '\n' ' ')"
eingabe <<< "wait 1500"
# a, riesig, viele, viele (Kopie), viele2, viele2 (Kopie)
kopieren_einfuegen 1
# dritter Auftrag der Warteschlange
id=3
warte_log "Abbrechen für Auftrag $id bei" 20
abbrechen_klick "$id"
warte_log 'kopieren fertig: .*abgebrochen' 30
eingabe <<< "wait 500"
reste=$(ls -A "$T" | grep -c 'querencia-teil' || true)
pruefe F3 "Abbrechen während der Kopie von „riesig“" "abgebrochen, kein Teilordner, kein „riesig (Kopie)“" 'grep -a -q "Fortschritt gezeigt: Auftrag '"$id"': Kopieren von „riesig“" "$LOG" && grep -a -q "kopieren fertig: 0 erledigt.*abgebrochen" "$LOG" && [ "$reste" = 0 ] && [ ! -e "$T/riesig (Kopie)" ]' "$(ls -A "$T" | tr '\n' ' ')"
pruefe F4 "Fenster nach Abbruch" "Fortschrittsfenster geschlossen" '[ "$(zahl_log "Fortschrittsfenster geschlossen")" -ge 2 ]'
