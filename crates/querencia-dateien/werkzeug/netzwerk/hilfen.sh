# Querencia Dateien, Welle C1: Hilfen für die Netz-Szenarien (läuft in der
# Hülle; das Szenario liest sie nach innen.sh mit „source "$LAUF_NETZ/hilfen.sh"“).
#   freigabe_anlegen     Testdateien in der SFTP-Freigabe, Pfad /tmp/qd-freigabe
#   soll_fingerabdruck   SHA256 des Wegwerf-Host-Schlüssels (ssh-keygen -lf)
#   lauscht PORT         127.0.0.1:PORT nimmt Verbindungen an
#   webdav_starten PORT BENUTZER KENNWORT    WebDAV-Attrappe (Basic-Auth)
#   ftp_starten PORT BENUTZER KENNWORT       FTP-Attrappe (auch anonym)
#   haenger_starten PORT                     nimmt an und schweigt (Stopp, Zeitgrenze)
#   log_hat MUSTER       Zeile im Programmprotokoll (grep -a)
#   foto_fenster NAME    Foto nach kurzer Wartezeit (Dialog-Animation)
export PATH="$LAUF_NETZ/bin:$PATH"
FREIGABE=/tmp/qd-freigabe
SFTP_URI="sftp://127.0.0.1:2222$FREIGABE"
freigabe_anlegen() {
  ln -sfn "$QD_PROFIL/sftp-freigabe" "$FREIGABE"
  mkdir -p "$FREIGABE/Berichte" "$FREIGABE/Fotos"
  printf 'Testdatei der SFTP-Freigabe\n' > "$FREIGABE/Notizen.txt"
  printf 'Bericht\n' > "$FREIGABE/Berichte/Quartal 3.txt"
  head -c 20000 /dev/zero > "$FREIGABE/Fotos/bild.raw"
}
soll_fingerabdruck() { ssh-keygen -lf "$QD_SSH_DIR/host_ed25519.pub" | awk '{print $2}'; }
lauscht() {
  local hex
  hex=$(printf '%04X' "$1")
  grep -q " 0100007F:$hex 00000000:0000 0A " /proc/net/tcp
}
warte_port() {
  local i
  for ((i = 0; i < 100; i++)); do lauscht "$1" && return 0; q "wait 100"; done
  return 1
}
webdav_starten() {
  python3 -I "$LAUF_NETZ/webdav-attrappe.py" --port "$1" --benutzer "$2" --kennwort "$3" \
    --ordner "$FREIGABE" >> "$OUT/webdav.log" 2>&1 &
  warte_port "$1"
}
ftp_starten() {
  python3 -I "$LAUF_NETZ/ftp-attrappe.py" --port "$1" --benutzer "$2" --kennwort "$3" \
    --ordner "$FREIGABE" >> "$OUT/ftp.log" 2>&1 &
  warte_port "$1"
}
haenger_starten() {
  python3 -I -c 'import socket,sys,time
s=socket.socket(); s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
s.bind(("127.0.0.1", int(sys.argv[1]))); s.listen(4)
offen=[]
while True:
    c,_=s.accept(); offen.append(c)' "$1" >> "$OUT/haenger.log" 2>&1 &
  warte_port "$1"
}
log_hat() { grep -a -q -- "$1" "$LOG"; }
foto_fenster() { q "wait 1200"; foto "$1"; }
# Wegwerf-Schlüsselbund (gnome-keyring, nur Geheimnisse) im Test-Bus: zeigt,
# ob GVfs ein Kennwort speichert („Kennwort merken“). Kennwort des
# Schlüsselbunds ist ein Testwert ohne Bedeutung.
schluesselbund_starten() {
  printf 'qd-schluesselbund-test' | gnome-keyring-daemon --unlock --foreground --components=secrets \
    >> "$OUT/schluesselbund.log" 2>&1 &
  local i
  for ((i = 0; i < 50; i++)); do
    gdbus call --session --dest org.freedesktop.DBus --object-path /org/freedesktop/DBus \
      --method org.freedesktop.DBus.NameHasOwner org.freedesktop.secrets 2>/dev/null | grep -q true && return 0
    q "wait 100"
  done
  return 1
}
# Zahl der gespeicherten Einträge für Protokoll und Server (Attribute aus
# geheimnisse.py, die Liste landet in OUT/geheimnisse-N.txt).
GEHEIM_NR=0
gespeicherte() {
  GEHEIM_NR=$((GEHEIM_NR + 1))
  python3 -I "$LAUF_NETZ/geheimnisse.py" > "$OUT/geheimnisse-$GEHEIM_NR.txt" 2>&1
  grep -c "protocol=$1 .*server=$2" "$OUT/geheimnisse-$GEHEIM_NR.txt"
}
# Dienst-Typ im Verbinden-Dialog wählen: N-mal abwärts ab SSH (0 SSH, 1 SMB, 2 FTP, 3 WebDAV, 4 WebDAVS, 5 Eigene).
dienst_waehlen() {
  local i cmds=()
  taste alt+t
  # the dialog remembers the last type: start from the top of the list
  cmds+=("key Home")
  for ((i = 0; i < $1; i++)); do cmds+=("key Down"); done
  q "${cmds[@]}" "key Return" "wait 300"
  taste alt+s
}
# Datei → Mit Server verbinden …; unter Last geht die erste Taste manchmal
# verloren: auf die Logzeile warten, sonst ein zweites Mal.
verbinden_oeffnen() {
  local n
  for n in 1 2 3; do
    markiere
    menue datei
    q "wait 300" "move 120 231" "click"
    warte_log "Verbinden-Dialog offen" 5 && { q "wait 600"; return 0; }
    q "key Escape" "wait 500"
  done
  return 1
}
# Wartet auf das Ende eines Verbindungsversuchs (nicht auf die Startzeile).
warte_ende() { warte_log "einhängen \(fertig\|abgebrochen\|gescheitert\)" "${1:-30}"; }
letztes_ende() { grep -a "einhängen \(fertig\|abgebrochen\|gescheitert\)" "$LOG" | tail -1; }
# Zugriffstasten der Dialoge je Sprache (Ordner, Benutzer, Lesezeichen,
# Trotzdem verbinden, Kennwort merken).
if [ "${LAUF_SPRACHE:-de}" = en ]; then
  M_ORDNER=f M_BENUTZER=u M_LESEZ=b M_TROTZDEM=a M_MERKEN=r
else
  M_ORDNER=o M_BENUTZER=b M_LESEZ=l M_TROTZDEM=t M_MERKEN=m
fi
netzwerk_oeffnen() { menue gehe; q "key Up" "key Up" "key Return"; }
