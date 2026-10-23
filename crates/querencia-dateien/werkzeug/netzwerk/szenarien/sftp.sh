# B23: Mit Server verbinden → SFTP zum Test-sshd, unbekannter Schlüssel.
# Erwartung: Frage mit Fingerabdruck (= ssh-keygen -lf), „Abbrechen“ zuerst
# und vorgewählt; Ablehnen ohne Fehlermeldung; dann „Trotzdem verbinden“:
# Ordner offen, Eintrag in der Seitenleiste mit ⏏, Lesezeichen; Aushängen.
# Start mit LAUF_SSH_HOSTKEY_FRAGE=1.
source "$LAUF_NETZ/hilfen.sh"
freigabe_anlegen
SOLL=$(soll_fingerabdruck)
starte
warte_log "Ordner gelesen" 20
verbinden_dialog() {
  menue datei
  q "move 120 279" "click"  # Welle C3: zwei Einträge mehr im Menü Datei
  q "wait 800"
  q "type 127.0.0.1"
  taste alt+p; q "type 2222"
  taste alt+o; q "type $FREIGABE"
  if [ "${1:-}" = lesezeichen ]; then
    taste alt+l; taste alt+n; q "type Testfreigabe"
  fi
}
markiere
verbinden_dialog
foto_fenster 01-dialog-sftp
q "key Return"
warte_log "Frage zeigt Fingerabdruck" 20
q "wait 1200"
IST=$(grep -a "Frage zeigt Fingerabdruck" "$LOG" | tail -1 | awk '{print $NF}')
foto_fenster 02-hostkey-frage
fall S1 "Fingerabdruck gezeigt = ssh-keygen -lf" "$([ -n "$SOLL" ] && [ "$IST" = "$SOLL" ] && echo 1)" "soll $SOLL ist $IST"
fall S2 "selbst gelesen (GVfs ???)" "$(log_hat 'Fingerabdruck selbst gelesen: ja' && echo 1)" "programm.log"
q "key Return"
warte_ende 20
fall S3 "Enter = Abbrechen (sichere Wahl vorn)" "$(log_hat 'Antwort 1 (ablehnen)' && echo 1)" "programm.log"
fall S4 "Ablehnen ohne Fehlerdialog" "$(log_hat 'einhängen abgebrochen' && echo 1)" "programm.log"
foto_fenster 03-abgelehnt
markiere
verbinden_dialog lesezeichen
q "key Return"
warte_log "Frage zeigt Fingerabdruck" 20
q "wait 1200"
taste alt+t
warte_ende 30
fall S5 "Trotzdem verbinden → eingehängt" "$(log_hat 'einhängen fertig' && echo 1)" "programm.log"
warte_log "Ordner gelesen" 20
q "wait 800"
foto_fenster 04-verbunden
fall S6 "known_hosts = gezeigter Schlüssel" "$(log_hat 'known_hosts Matches' && echo 1)" "programm.log"
fall S7 "Lesezeichen angelegt (ohne Kennwort)" "$(grep -q "^sftp://127.0.0.1:2222$FREIGABE Testfreigabe\$" "$XDG_CONFIG_HOME/gtk-3.0/bookmarks" && echo 1)" "bookmarks"
fall S8 "Ordner der Freigabe gelesen" "$(grep -a 'Ordner gelesen: 3 Einträge' "$LOG" >/dev/null && echo 1)" "programm.log"
gio mount -l > "$OUT/mounts-verbunden.txt" 2>&1
fall S9 "GVfs-Mount vorhanden" "$(grep -q 'sftp://' "$OUT/mounts-verbunden.txt" && echo 1)" "mounts-verbunden.txt"
# Aushängen über ⏏ in der Seitenleiste: Fenster geht nach Hause, Mount weg
markiere
q "move 132 304" "click"
warte_log "Ordner file://" 15
q "wait 1500"
foto_fenster 05-ausgehaengt
gio mount -l > "$OUT/mounts-ausgehaengt.txt" 2>&1
fall S10 "⏏ hängt aus" "$(grep -q 'sftp://' "$OUT/mounts-ausgehaengt.txt" || echo 1)" mounts-ausgehaengt.txt
fall S11 "Fenster zeigt danach den persönlichen Ordner" "$(tail -n +"$LOG_STAND" "$LOG" | grep -a -q 'Ordner file://.*/heim: lesen' && echo 1)" programm.log
# das Lesezeichen verbindet erneut (Schlüssel jetzt bekannt: keine Frage)
FERTIG_VOR=$(grep -a -c 'einhängen fertig' "$LOG"); FRAGEN_VOR=$(grep -a -c 'Frage mit' "$LOG")
markiere
q "move 66 418" "click"
warte_ende 20
warte_log "Ordner gelesen" 20
fall S12 "Lesezeichen verbindet erneut ohne Frage" "$([ "$(grep -a -c 'einhängen fertig' "$LOG")" -gt "$FERTIG_VOR" ] && [ "$(grep -a -c 'Frage mit' "$LOG")" = "$FRAGEN_VOR" ] && echo 1)" programm.log
foto_fenster 06-lesezeichen-erneut
