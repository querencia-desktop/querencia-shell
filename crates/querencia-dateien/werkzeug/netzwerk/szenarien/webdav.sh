# B24 (Kennwort statt SMB): WebDAV-Attrappe mit Basic-Anmeldung im Namensraum.
# Erwartung: Kennwortfrage auf Deutsch, falsches Kennwort → zweite Frage mit
# Hinweis; richtiges Kennwort ohne „merken“ → verbunden, nichts im
# Schlüsselbund; aushängen; erneut mit „merken“ → im Schlüsselbund; aushängen;
# dritter Versuch → keine Frage mehr (GVfs nimmt das gespeicherte Kennwort).
source "$LAUF_NETZ/hilfen.sh"
freigabe_anlegen
KW=$(head -c 12 /dev/urandom | od -An -tx1 | tr -d ' \n' | head -c 12)
webdav_starten 8080 anna "$KW" || fall W0 "WebDAV-Attrappe startet" 0 webdav.log
schluesselbund_starten || fall W0 "Schlüsselbund startet" 0 schluesselbund.log
starte
warte_log "Ordner gelesen" 20
webdav_dialog() {
  verbinden_oeffnen
  dienst_waehlen 3
  q "type 127.0.0.1"
  taste alt+p; q "type 8080"
  taste alt+b; q "type anna"
}
markiere
webdav_dialog
foto_fenster 01-dialog-webdav
q "key Return"
warte_log "Kennwortfrage 1" 20
foto_fenster 02-kennwortfrage
q "type falsch1234" "key Return"
warte_log "Kennwortfrage 2" 20
q "wait 1200"
fall W1 "falsches Kennwort → zweite Frage" "$(log_hat 'Kennwortfrage 2' && echo 1)" programm.log
foto_fenster 03-kennwort-falsch
q "type $KW" "key Return"
warte_ende 20
fall W2 "richtiges Kennwort → verbunden" "$(log_hat 'einhängen fertig' && echo 1)" programm.log
fall W3 "ohne „merken“ gesendet" "$(log_hat 'merken false' && echo 1)" programm.log
warte_log "Ordner gelesen" 20
foto_fenster 04-webdav-verbunden
N0=$(gespeicherte http 127.0.0.1)
fall W4 "nicht gemerkt → Schlüsselbund leer" "$([ "$N0" = 0 ] && echo 1)" "Einträge $N0"
gio mount -u "dav://anna@127.0.0.1:8080/" >> "$OUT/gio.log" 2>&1; q "wait 1500"
markiere
webdav_dialog
q "key Return"
warte_log "Kennwortfrage" 20
q "wait 1200"
taste alt+m
q "type $KW" "key Return"
warte_ende 20
fall W5 "mit „merken“ gesendet und verbunden" "$(log_hat 'merken true' && log_hat 'einhängen fertig' && echo 1)" programm.log
N1=$(gespeicherte http 127.0.0.1)
fall W6 "gemerkt → ein Eintrag im Schlüsselbund" "$([ "$N1" -ge 1 ] && echo 1)" "Einträge $N1"
gio mount -u "dav://anna@127.0.0.1:8080/" >> "$OUT/gio.log" 2>&1; q "wait 1500"
markiere
FRAGEN_VOR=$(grep -a -c "Kennwortfrage" "$LOG")
webdav_dialog
q "key Return"
warte_ende 20
FRAGEN_NACH=$(grep -a -c "Kennwortfrage" "$LOG")
fall W7 "gemerktes Kennwort → ohne Frage verbunden" "$([ "$FRAGEN_VOR" = "$FRAGEN_NACH" ] && letztes_ende | grep -q fertig && echo 1)" "Fragen $FRAGEN_VOR → $FRAGEN_NACH"
fall W8 "Kennwort nie im Programmprotokoll" "$(grep -a -q -- "$KW" "$LOG" || echo 1)" programm.log
fall W9 "Kennwort nie in den Lesezeichen" "$(grep -q -- "$KW" "$XDG_CONFIG_HOME/gtk-3.0/bookmarks" 2>/dev/null || echo 1)" bookmarks
