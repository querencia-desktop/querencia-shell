# Querencia Dateien, Ansichten, Ablauf „dauer“ (aus szenario.sh eingelesen):
# Speicher (VmRSS) beim Scrollen im Ordner „10000 Einträge“, MINUTEN Minuten
# lang: Mausrad in Schüben, Ende/Pos1, Bild↓/Bild↑, jede Minute ein
# Zoomwechsel, ein Sprung per Tippsuche und ein Wechsel zur Listenansicht
# und zurück. Messpunkte alle 30 s in dauer.csv (aus dem Prototyp).
G="$BEISPIEL/10000 Einträge"
start "$G"
P=$(pid_von)
[ -n "$P" ] || { nok "D01 Prozess nicht gefunden"; stopp; return; }
CSV=$OUT/dauer.csv
echo "sekunden;vmrss_kib" >"$CSV"
R0=$(rss "$P"); echo "0;$R0" >>"$CSV"
info "D01 Start: VmRSS $R0 KiB"
q "move $((BX / 2)) $((BY / 2))"
T0=$(date +%s); NAECHST=30; RUNDE=0
while [ $(( $(date +%s) - T0 )) -lt $((MINUTEN * 60)) ]; do
    RUNDE=$((RUNDE + 1))
    for i in $(seq 20); do q "scroll 5" "wait 40"; done
    q "key End" "wait 200" "key Home" "wait 200"
    for i in $(seq 10); do q "key Page_Down" "wait 60"; done
    for i in $(seq 20); do q "scroll -5" "wait 40"; done
    for i in $(seq 10); do q "key Page_Up" "wait 60"; done
    if [ $((RUNDE % 6)) -eq 0 ]; then
        q "keydown Control_L" "scroll -1" "wait 200" "keyup Control_L" "wait 300"
        q "keydown Control_L" "scroll 1" "wait 200" "keyup Control_L" "wait 300"
        q "type ve" "wait 300" "key Escape" "wait 100"
        q "keydown Control_L" "type 2" "keyup Control_L" "wait 500"
        q "keydown Control_L" "type 1" "keyup Control_L" "wait 500"
    fi
    S=$(( $(date +%s) - T0 ))
    if [ "$S" -ge "$NAECHST" ]; then
        kill -0 "$P" 2>/dev/null || { nok "D01 Prozess beendet nach $S s"; break; }
        echo "$S;$(rss "$P")" >>"$CSV"; NAECHST=$((NAECHST + 30))
    fi
done
kill -0 "$P" 2>/dev/null && ok "D01 Prozess läuft nach $MINUTEN min Scrollen ($RUNDE Runden)" || nok "D01 Prozess abgestürzt"
R1=$(rss "$P"); HWM=$(awk '/^VmHWM/ { print $2 }' "/proc/$P/status")
echo "$(( $(date +%s) - T0 ));$R1" >>"$CSV"
info "D01 Ende: VmRSS $R1 KiB, Spitze (VmHWM) $HWM KiB, Zuwachs $((R1 - R0)) KiB"
ZWEITE=$(sed -n 3p "$CSV" | cut -d';' -f2)
info "D01 Zuwachs ab der ersten Minute: $((R1 - ${ZWEITE:-$R0})) KiB"
stopp
