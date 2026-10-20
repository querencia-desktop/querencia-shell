#!/bin/bash
# G2-AL: Querencia Benachrichtigungen im isolierten Lauf starten und beenden
# (über qeingabe „run DIENST start|stop“). Nur mit dem Laufzeitordner von
# isoliert.py; beendet wird nur der hier gestartete Prozess, und nur wenn er
# diesen Laufzeitordner hat (nie der Dienst der Sitzung).
#   dienst.sh start|stop
case "$XDG_RUNTIME_DIR" in */querencia-runtime-wt-g2al/nachweis/gui/rt/g2m-*) ;; *) echo "nicht isoliert"; exit 2 ;; esac
OUT=$HMP04_PROFILE/logs
case "${1:-}" in
start)
  [ -x "${G2AL_DIENST:-}" ] || { echo "kein Dienst"; exit 2; }
  echo "$(date +%T.%3N) start $G2AL_DIENST" >>"$OUT/dienst.log"
  "$G2AL_DIENST" >>"$OUT/dienst.log" 2>&1 &
  echo $! >"$OUT/dienst.pid"
  ;;
stop)
  pid=$(cat "$OUT/dienst.pid" 2>/dev/null) || exit 0
  if tr '\0' '\n' <"/proc/$pid/environ" 2>/dev/null | grep -qx "XDG_RUNTIME_DIR=$XDG_RUNTIME_DIR"; then
    echo "$(date +%T.%3N) stop $pid" >>"$OUT/dienst.log"
    kill "$pid"
  fi
  rm -f "$OUT/dienst.pid"
  ;;
*) exit 2 ;;
esac
