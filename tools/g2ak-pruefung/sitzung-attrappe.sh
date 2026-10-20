#!/bin/bash
# G2-AK: Attrappe von querencia-sitzung für die isolierte Prüfung. Schreibt
# Aufruf und Argumente in $G2AK_AUFRUFE (sonst nach stderr) und endet mit 0;
# sperrt, meldet ab und schaltet nichts aus.
echo "$(date +%T.%3N) $0 $*" >>"${G2AK_AUFRUFE:-/dev/stderr}"
