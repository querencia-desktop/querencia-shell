# SFTP mit Kennwort (Start mit LAUF_SSH_OHNE_SCHLUESSEL=1): Benutzer „anna“
# gibt es auf dem Test-sshd nicht, jedes Kennwort ist falsch (sshd ohne PAM
# kann ohnehin keine Kennwörter prüfen). Erwartung: Kennwortfrage auf
# Deutsch mit „Kennwort merken“ (aus), zweite Frage mit Hinweis, nach dem
# letzten Versuch von GVfs eine klare Meldung; Abbrechen sagt nichts.
source "$LAUF_NETZ/hilfen.sh"
freigabe_anlegen
starte
warte_log "Ordner gelesen" 20
sftp_anna() {
  verbinden_oeffnen
  dienst_waehlen 0
  q "type 127.0.0.1"
  taste alt+p; q "type 2222"
  taste alt+o; q "type $FREIGABE"
  taste alt+b; q "type anna"
  q "key Return"
}
markiere
sftp_anna
warte_log "Kennwortfrage 1" 20
foto_fenster 01-sftp-kennwort
fall K1 "Kennwortfrage mit „merken“" "$(log_hat 'Kennwortfrage 1 (AskPasswordFlags(NEED_PASSWORD | SAVING_SUPPORTED))' && echo 1)" programm.log
for n in 1 2 3 4 5 6; do
  q "type falsch$n" "key Return"
  i=0
  while ((i < 100)); do
    tail -n +"$((LOG_STAND + 1))" "$LOG" | grep -a -q "Kennwortfrage $((n + 1))\|einhängen \(fertig\|abgebrochen\|gescheitert\)" && break
    q "wait 100"; i=$((i + 1))
  done
  [ "$n" = 1 ] && foto_fenster 02-sftp-kennwort-falsch
  tail -n +"$((LOG_STAND + 1))" "$LOG" | grep -a -q "einhängen \(fertig\|abgebrochen\|gescheitert\)" && break
done
foto_fenster 03-sftp-abgewiesen
fall K2 "zweite Frage nach falschem Kennwort" "$(log_hat 'Kennwortfrage 2' && echo 1)" programm.log
fall K3 "am Ende klare Meldung" "$(grep -a 'Netzwerk: Meldung' "$LOG" | tail -1 | grep -q 'nicht angenommen' && echo 1)" "$(grep -a 'Netzwerk: Meldung' "$LOG" | tail -1 | cut -c1-120)"
q "key Escape" "wait 500"
markiere
sftp_anna
warte_log "Kennwortfrage" 20
q "wait 1200"
q "key Escape"
warte_ende 20
fall K4 "Abbrechen der Kennwortfrage sagt nichts" "$(letztes_ende | grep -q abgebrochen && echo 1)" programm.log
fall K5 "keine Fehlermeldung nach Abbrechen" "$(tail -n +"$((LOG_STAND + 1))" "$LOG" | grep -a -q 'Netzwerk: Meldung' || echo 1)" programm.log
