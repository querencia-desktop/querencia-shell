# B26: Server nicht erreichbar, falsche Angaben, Stopp, Zeitgrenze, doppelte
# Anfrage. Erwartung: je eine verständliche Meldung (kein Fachjargon), kein
# Hängen, Stopp bricht still ab, ein zweites Fenster fragt nicht noch einmal.
# Start mit LAUF_ZEITGRENZE=6.
source "$LAUF_NETZ/hilfen.sh"
freigabe_anlegen
starte
warte_log "Ordner gelesen" 20
# einen Typ wählen und Server/Port/Freigabe eintragen, dann Verbinden
versuch() { # TYP SERVER [PORT] [FREIGABE]
  verbinden_oeffnen
  dienst_waehlen "$1"
  q "type $2"
  [ -n "${3:-}" ] && { taste alt+p; q "type $3"; }
  [ -n "${4:-}" ] && { taste alt+f; q "type $4"; }
  q "key Return"
}
meldung() { # NR TEXT MUSTER FOTO
  warte_ende 30
  foto_fenster "$4"
  fall "$1" "$2" "$(grep -a 'Netzwerk: Meldung' "$LOG" | tail -1 | grep -q -- "$3" && echo 1)" \
    "$(grep -a 'Netzwerk: Meldung' "$LOG" | tail -1 | cut -c1-140)"
  q "key Escape" "wait 500"
}
markiere; versuch 0 127.0.0.1 2223
meldung E1 "SFTP ohne Dienst → abgelehnt" "Verbindung abgelehnt" 01-sftp-abgelehnt
markiere; versuch 0 qd-unbekannt.invalid
meldung E2 "unbekannter Rechner → nicht gefunden" "nicht gefunden" 02-unbekannt
markiere; versuch 1 127.0.0.1 "" daten
meldung E3 "SMB ohne Server → verständlich" "Server" 03-smb
markiere; versuch 2 127.0.0.1 2121
meldung E4 "FTP ohne Server → abgelehnt" "abgelehnt" 04-ftp
markiere; versuch 3 127.0.0.1 8081
meldung E5 "WebDAV ohne Server → verständlich" "Server" 05-webdav
fall E6 "kein Hängen: fünf Meldungen, Fenster bedienbar" "$([ "$(grep -a -c 'einhängen gescheitert' "$LOG")" -ge 5 ] && echo 1)" programm.log
# Eingabefehler bleiben im Dialog
verbinden_oeffnen
dienst_waehlen 0
q "key Return" "wait 400"
foto_fenster 06-kein-server
fall E7 "leerer Server → Hinweis im Dialog" "$(log_hat 'Angaben ungültig: KeinServer' && echo 1)" programm.log
q "type a b" "key Return" "wait 400"
fall E8 "Server mit Leerzeichen → Hinweis" "$(log_hat 'Angaben ungültig: ServerUngueltig' && echo 1)" programm.log
taste ctrl+a; q "type srv"
taste alt+p; q "type 99999" "key Return" "wait 400"
foto_fenster 07-port
fall E9 "Port 99999 → Hinweis" "$(log_hat 'Angaben ungültig: Port' && echo 1)" programm.log
q "key Escape" "wait 400"
verbinden_oeffnen
dienst_waehlen 5
taste alt+u; q "type nas-test/Daten" "key Return" "wait 400"
foto_fenster 08-eigene-ohne-protokoll
fall E10 "eigene Adresse ohne Protokoll → Vorschlag" "$(log_hat 'Angaben ungültig: KeinProtokoll' && echo 1)" programm.log
q "key Escape" "wait 400"
# Stopp: ein Dienst, der annimmt und schweigt
haenger_starten 2224 || fall E0 "Hänger startet" 0 haenger.log
markiere; versuch 0 127.0.0.1 2224
warte_log "einhängen sftp" 10
q "wait 1500"
foto_fenster 09-verbinden-laeuft
q "move 200 69" "click"
warte_ende 10
fall E11 "Stopp bricht still ab" "$(log_hat 'Netzwerk: Stopp' && letztes_ende | grep -q abgebrochen && echo 1)" programm.log
# Zeitgrenze (LAUF_ZEITGRENZE=6)
markiere; versuch 0 127.0.0.1 2224
warte_ende 30
foto_fenster 10-zeitgrenze
fall E12 "Zeitgrenze → Meldung" "$(log_hat 'Zeitgrenze erreicht' && grep -a 'Netzwerk: Meldung' "$LOG" | tail -1 | grep -q 'Sekunden' && echo 1)" programm.log
q "key Escape" "wait 500"
# doppelte Anfrage: zwei Fenster, ein Server, eine Kennwortfrage
KW=$(head -c 12 /dev/urandom | od -An -tx1 | tr -d ' \n' | head -c 12)
webdav_starten 8080 anna "$KW" || fall E0 "WebDAV-Attrappe startet" 0 webdav.log
markiere
"$LAUF_PROG" "dav://anna@127.0.0.1:8080/" >> "$LOG" 2>&1
warte_log "Kennwortfrage" 20
q "wait 1200"
"$LAUF_PROG" "dav://anna@127.0.0.1:8080/Berichte" >> "$LOG" 2>&1
warte_log "wartet auf laufenden Versuch" 20
foto_fenster 11-zwei-fenster
q "move 476 313" "click" "type $KW" "key Return"
warte_ende 20
q "wait 1500"
foto_fenster 12-beide-verbunden
fall E13 "zweites Fenster wartet, eine Kennwortfrage" "$(log_hat 'wartet auf laufenden Versuch' && [ "$(grep -a -c 'Kennwortfrage' "$LOG")" = 1 ] && echo 1)" "Kennwortfragen $(grep -a -c 'Kennwortfrage' "$LOG")"
fall E14 "beide Fenster zeigen den Server" "$(grep -a -c 'Ordner dav://anna@127.0.0.1:8080/' "$LOG" | awk '{exit !($1>=2)}' && echo 1)" programm.log
