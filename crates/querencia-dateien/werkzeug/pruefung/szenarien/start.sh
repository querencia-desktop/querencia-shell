# D02: Start mit Pfad und mit URI, ein Prozess (GApplication), zweiter Aufruf
# öffnet ein zweites Fenster. D01 (Teil): Start über die Desktop-Datei (gtk-launch).
starte "$HOME/Dokumente"
warte_log "Dokumente: lesen" 15 && fall D02a "Start mit Pfad" 1 "programm.log" || fall D02a "Start mit Pfad" 0 "-"
markiere
"$LAUF_PROG" "file://$HOME/Bilder" >> "$OUT/zweiter-aufruf.log" 2>&1
rc=$?
warte_log "Bilder: lesen" 10 && fall D02b "Zweiter Aufruf mit URI öffnet zweites Fenster im selben Prozess" 1 "programm.log (Log des ersten Prozesses), Exit $rc" \
  || fall D02b "Zweiter Aufruf" 0 "Exit $rc"
n=0
for p in /proc/[0-9]*; do
  [ "$(readlink "$p/exe" 2>/dev/null)" = "$LAUF_PROG" ] && n=$((n + 1))
done
[ "$n" = 1 ] && fall D02c "Ein Prozess" 1 "/proc/*/exe: $n" || fall D02c "Ein Prozess" 0 "/proc/*/exe: $n"
q "wait 500"
foto 01-zwei-fenster
