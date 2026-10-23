# Fotos der Netzwerk-Oberfläche (Start mit LAUF_WSDD=1 LAUF_SSH_HOSTKEY_FRAGE=1;
# LAUF_GROESSE, LAUF_DUNKEL, LAUF_SPRACHE je Satz). Jede Station zugleich
# ein Fall.
source "$LAUF_NETZ/hilfen.sh"
freigabe_anlegen
KW=$(head -c 12 /dev/urandom | od -An -tx1 | tr -d ' \n' | head -c 12)
webdav_starten 8080 anna "$KW" || fall F0 "WebDAV-Attrappe startet" 0 webdav.log
starte
warte_log "Ordner gelesen" 20
netzwerk_oeffnen
ANZ=0
for _ in $(seq 20); do
  q "wait 2000"; markiere; q "key F5"; warte_log "Ordner gelesen" 10
  ANZ=$(grep -a "Ordner gelesen" "$LOG" | tail -1 | grep -o '[0-9]* Einträge' | cut -d' ' -f1)
  [ "${ANZ:-0}" -ge 2 ] && break
done
q "move 700 450"
foto_fenster 01-netzwerk
fall F1 "Netzwerk mit zwei Testgeräten" "$([ "${ANZ:-0}" -ge 2 ] && echo 1)" "$ANZ Einträge"
markiere
verbinden_oeffnen
dienst_waehlen 0
q "type 127.0.0.1"
taste alt+p; q "type 2222"
taste "alt+$M_ORDNER"; q "type $FREIGABE"
taste "alt+$M_LESEZ"; taste alt+n; q "type Testfreigabe"
foto_fenster 02-verbinden-dialog
q "key Return"
warte_log "Frage zeigt Fingerabdruck" 20
foto_fenster 03-hostkey-frage
fall F2 "Host-Key-Frage mit Fingerabdruck" "$(log_hat 'Frage zeigt Fingerabdruck SHA256:' && echo 1)" programm.log
taste "alt+$M_TROTZDEM"
warte_ende 20
warte_log "Ordner gelesen" 20
q "move 700 450"
foto_fenster 04-verbunden-seitenleiste
fall F3 "verbunden, Seitenleiste mit Server und Lesezeichen" "$(letztes_ende | grep -q fertig && echo 1)" programm.log
markiere
verbinden_oeffnen
dienst_waehlen 3
q "type 127.0.0.1"
taste alt+p; q "type 8080"
taste "alt+$M_BENUTZER"; q "type anna"
q "key Return"
warte_log "Kennwortfrage 1" 20
foto_fenster 05-kennwortfrage
fall F4 "Kennwortfrage mit „Kennwort merken“" "$(log_hat 'Kennwortfrage 1 (AskPasswordFlags(NEED_PASSWORD | SAVING_SUPPORTED))' && echo 1)" programm.log
q "key Escape"
warte_ende 20
markiere
verbinden_oeffnen
dienst_waehlen 0
q "type 127.0.0.1"
taste alt+p; q "type 2223"
q "key Return"
warte_ende 30
foto_fenster 06-fehlermeldung
fall F5 "Fehlermeldung verständlich" "$(grep -a 'Netzwerk: Meldung' "$LOG" | tail -1 | grep -q -i 'abgelehnt\|refused' && echo 1)" "$(grep -a 'Netzwerk: Meldung' "$LOG" | tail -1 | cut -c1-120)"
q "key Escape" "wait 500"
