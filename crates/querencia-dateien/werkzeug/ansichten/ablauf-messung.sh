# Querencia Dateien, Ansichten, Ablauf „messung“ (aus szenario.sh eingelesen):
# Ordner „10000 Einträge“ (10.000 Dateien, 20 Ordner) je drei Mal kalt öffnen
# und umsortieren, in der Symbol- und in der Listenansicht; Speicher nach dem
# Öffnen. Grenzen aus dem Auftrag: Öffnen < 1 s, Umsortieren < 0,5 s.
G="$BEISPIEL/10000 Einträge"
ms() { sed -E 's/.* nach ([0-9]+) ms.*/\1/'; }
oeffnen_messen() { # Ansicht (symbole|liste) Lauf
    local umg=()
    [ "$1" = liste ] && umg=(QUERENCIA_DATEIEN_ANSICHT=liste)
    start "$G" "${umg[@]}"
    local erst ganz
    erst=$(zeile 'erstes Bild nach' | ms)
    ganz=$(zeile 'vollständig gezeigt nach' | ms)
    info "M01 $1 Lauf $2: erstes Bild $erst ms, vollständig gezeigt $ganz ms"
    [ -n "$ganz" ] && [ "$ganz" -lt 1000 ] && ok "M01 $1 Lauf $2: Öffnen von 10.020 Einträgen < 1 s ($ganz ms)" \
        || nok "M01 $1 Lauf $2: Öffnen von 10.020 Einträgen < 1 s (${ganz:-?} ms)"
    echo "$1;$2;oeffnen;$erst;$ganz" >>"$CSV"
}
umsortieren_messen() { # Ansicht Lauf Kriterium
    marke; aktion anordnen "$3"
    if warte_log 10 "Messung Umordnen .*: Bild nach"; then
        local zellen bild
        zellen=$(zeile 'Messung Umordnen .*Zellen neu nach' | ms)
        bild=$(zeile 'Messung Umordnen .*Bild nach' | ms)
        info "M02 $1 Lauf $2 nach $3: Zellen $zellen ms, Bild $bild ms"
        [ "$bild" -lt 500 ] && ok "M02 $1 Lauf $2 nach $3: Umsortieren < 0,5 s ($bild ms)" \
            || nok "M02 $1 Lauf $2 nach $3: Umsortieren < 0,5 s ($bild ms)"
        echo "$1;$2;$3;$zellen;$bild" >>"$CSV"
    else
        nok "M02 $1 Lauf $2 nach $3: keine Messzeile"
    fi
    q "wait 300"
}
CSV=$OUT/messung.csv
echo "ansicht;lauf;vorgang;ms_a;ms_b" >"$CSV"
for ansicht in symbole liste; do
    for lauf in 1 2 3; do
        oeffnen_messen "$ansicht" "$lauf"
        if [ "$lauf" = 1 ]; then
            P=$(pid_von)
            [ -n "$P" ] && info "M03 $ansicht: VmRSS nach dem Öffnen $(rss "$P") KiB"
        fi
        # Fokus auf ein Objekt in der Mitte, damit der Fokus nach dem Umordnen prüfbar ist
        q "type Rechnung 00" "wait 400" "key Escape" "wait 300"
        VOR=$(fokus_name)
        for k in groesse datum name; do umsortieren_messen "$ansicht" "$lauf" "$k"; done
        q "wait 400"
        [ "$(fokus_name)" = "$VOR" ] && ok "M04 $ansicht Lauf $lauf: Fokus bleibt nach dem Umsortieren auf „$VOR“" \
            || nok "M04 $ansicht Lauf $lauf: Fokus nach dem Umsortieren „$(fokus_name)“ statt „$VOR“"
        stopp
    done
done
