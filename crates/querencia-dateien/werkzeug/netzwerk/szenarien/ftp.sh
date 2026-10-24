# FTP-Attrappe im Namensraum: anonym (Kennwortfrage mit „Anonym verbinden“)
# und mit Benutzer und Kennwort. Erwartung: beide Wege verbinden, Ordner
# der Freigabe wird gelesen.
source "$LAUF_NETZ/hilfen.sh"
freigabe_anlegen
KW=$(head -c 12 /dev/urandom | od -An -tx1 | tr -d ' \n' | head -c 12)
ftp_starten 2121 anna "$KW" || fall T0 "FTP-Attrappe startet" 0 ftp.log
starte
warte_log "Ordner gelesen" 20
M_ANONYM=o; [ "${LAUF_SPRACHE:-de}" = en ] && M_ANONYM=a
markiere
verbinden_oeffnen
dienst_waehlen 2
q "type 127.0.0.1"
taste alt+p; q "type 2121"
q "key Return"
warte_log "Kennwortfrage 1" 20
foto_fenster 01-ftp-anonym-frage
fall T1 "FTP fragt mit „Anonym verbinden“" "$(log_hat 'Kennwortfrage 1 (AskPasswordFlags(.*ANONYMOUS_SUPPORTED' && echo 1)" programm.log
taste "alt+$M_ANONYM"
q "wait 300" "key Return"
warte_ende 20
warte_log "Ordner gelesen" 20
foto_fenster 02-ftp-anonym-verbunden
fall T2 "anonym verbunden" "$(log_hat 'Kennwort gesendet (anonym true' && letztes_ende | grep -q fertig && echo 1)" programm.log
fall T3 "Ordner der FTP-Freigabe gelesen (LIST)" "$(grep -a 'Ordner gelesen' "$LOG" | tail -1 | grep -q '3 Einträge' && echo 1)" programm.log
gio mount -u "ftp://127.0.0.1:2121/" >> "$OUT/gio.log" 2>&1; q "wait 1500"
markiere
verbinden_oeffnen
dienst_waehlen 2
q "type 127.0.0.1"
taste alt+p; q "type 2121"
taste "alt+$M_BENUTZER"; q "type anna"
q "key Return"
warte_log "Kennwortfrage" 20
q "wait 1200" "type $KW" "key Return"
warte_ende 20
fall T4 "mit Benutzer und Kennwort verbunden" "$(letztes_ende | grep -q fertig && echo 1)" programm.log
fall T5 "Kennwort nie im Protokoll" "$(grep -a -q -- "$KW" "$LOG" "$OUT/ftp.log" || echo 1)" "programm.log, ftp.log"
