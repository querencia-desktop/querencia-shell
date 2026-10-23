# Welle C3 Kompaktansicht mit 10 000 Einträgen (vgl. B2-Messung 10.020
# Einträge): Start direkt in der Kompaktansicht, dann zweimal Strg+R; die
# Zeiten aus den Messzeilen des Ansichtskerns („erstes Bild“, „Ordner
# vollständig gezeigt“). Danach Ende und End-Taste (letzter Eintrag).
export QUERENCIA_DATEIEN_DEBUG=1
Z="$HOME/Zehntausend"
python3 -I -c 'import os,sys
d=sys.argv[1]; os.makedirs(d, exist_ok=True)
for i in range(10000):
    open(os.path.join(d, "eintrag-%05d mit etwas Text.txt" % i), "w").close()' "$Z"
mkdir -p "$XDG_CONFIG_HOME/querencia"
printf '[Ansichten]\nAnsicht=kompakt\n' > "$XDG_CONFIG_HOME/querencia/dateien.ini"
starte "$Z"
for n in 1 2 3; do
  warte_log "Zehntausend: Ordner vollständig gezeigt nach" 30 || { fall K13 "Messung $n" 0 "keine Messzeile"; break; }
  z=$(grep -a "Zehntausend: Ordner vollständig gezeigt nach" "$LOG" | tail -1 | sed -E 's/.*nach ([0-9]+) ms.*/\1/')
  e=$(grep -a "Zehntausend: erstes Bild nach" "$LOG" | tail -1 | sed -E 's/.*nach ([0-9]+) ms.*/\1/')
  echo "$n $e $z" >> "$OUT/messung.txt"
  [ "$n" -lt 3 ] && { q "wait 500"; taste ctrl+r; }
done
q "wait 500" "key End" "wait 800"
foto 01-zehntausend-ende
max=$(awk '{ if ($3 > m) m = $3 } END { print m }' "$OUT/messung.txt")
[ -n "$max" ] && [ "$max" -lt 3000 ] \
  && fall K13 "10 000 Einträge in der Kompaktansicht: erstes Bild / vollständig je Lauf (ms) $(awk '{printf "%s/%s ", $2, $3}' "$OUT/messung.txt")" 1 "messung.txt" \
  || fall K13 "10 000 Einträge" 0 "$(tr '\n' ' ' < "$OUT/messung.txt")"
grep -a -q "Fokus Kompaktansicht: eintrag-09999" "$LOG" && fall K13b "Ende springt zum letzten von 10 000" 1 "Fokus eintrag-09999" || fall K13b "Ende" 0 "-"
