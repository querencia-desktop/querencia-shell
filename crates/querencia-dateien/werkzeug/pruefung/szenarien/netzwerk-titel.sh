# QD-173 P-4 (Dateien 0.2.2): Fenstertitel des Netzwerks, wenn GVfs nicht
# deutsch spricht. Lauf mit LAUF_STUFE=G LAUF_SPRACHE=en: gvfsd (über den
# Bus gestartet) nimmt die englische Umgebung der Hülle, das Programm
# bekommt LANGUAGE=de. Der Titel muss „Netzwerk“ heißen.
export QUERENCIA_DATEIEN_DEBUG=1
LANGUAGE=de starte network:///
warte_log "network:///: lesen" 15
q "wait 2500"
foto n01-netzwerk
gvfs=$(gio info -a standard::display-name network:/// 2>&1 | sed -n 's/.*display-name: //p')
# Fenstertitel über wlr-foreign-toplevel (wlrctl, Exit 0 = gefunden)
wlrctl toplevel find title:Netzwerk; de=$?
wlrctl toplevel find title:Network; en=$?
echo "GVfs-Anzeigename: $gvfs; Titel Netzwerk: Exit $de; Titel Network: Exit $en" > "$OUT/netzwerk-titel.txt"
fall N01 "GVfs nennt network:/// englisch (Voraussetzung)" "$([ "$gvfs" = Network ] && echo 1)" "gio info: $gvfs"
fall N02 "Fenstertitel „Netzwerk“, nicht „Network“" "$([ $de = 0 ] && [ $en != 0 ] && echo 1)" "wlrctl: Netzwerk Exit $de, Network Exit $en"
