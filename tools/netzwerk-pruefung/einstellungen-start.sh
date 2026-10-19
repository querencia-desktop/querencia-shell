#!/bin/bash
# G2-M: steht in der Prüfung für querencia-einstellungen; schreibt nur die
# Argumente (ohne das erste, die Protokolldatei) mit Uhrzeit.
f=$1; shift
echo "$(date +%T.%3N) querencia-einstellungen $*" >>"$f"
