#!/bin/bash
# G2-J: alle Endläufe mit den Nachweis-Binaries, nacheinander.
#   endlaeufe.sh <panel-binary> <mixer-binary> [nur-g2j|nur-w2]
# 1. G2-J: haupt und ohne-dienst bei 1024x768 und 1920x1080.
# 2. W2-Regression (Werkzeuge aus querencia-werkstatt-w2-panel/nachweis/werkzeug,
#    umgelenkt nach nachweis/gui/w2reg, Referenzläufe nur gelesen kopiert):
#    Umschalter, Logo 32/24/48, Branding, Starter, Menü dynamisch, Fensterliste.
# Ergebnisse: nachweis/gui/ergebnis-*.txt, Zusammenfassung nachweis/gui/ergebnis.txt.
set -uo pipefail
WT=/home/user/projects/querencia-runtime-wt-lautstaerke
T=$WT/tools/lautstaerke-pruefung; G=$WT/nachweis/gui
PANEL=$(realpath -e "${1:?Panel}"); MIXER=$(realpath -e "${2:?Mixer}"); NUR=${3:-}
PY=/home/user/.werkstatt/installs/4598329b043a5290/environments/1adb572214574099b3114bf3abdc13ea/venv/bin/python3
SUMME=$G/ergebnis.txt
echo "=== $(date '+%F %T') Panel $(sha256sum "$PANEL" | cut -c1-16), Mixer $(sha256sum "$MIXER" | cut -c1-16)" >>"$SUMME"
ergebnis() { # name datei
    echo "$1: $(tail -1 "$2")" | tee -a "$SUMME"
}
if [ "$NUR" != nur-w2 ]; then
  for r in 1024x768 1920x1080; do
    w=${r%x*}; h=${r#*x}
    for art in haupt ohne-dienst; do
      p=$([ $art = haupt ] && echo h || echo k)$w
      python3 "$T/szenario-machen.py" "$w" "$h" "$art" >"$G/sz-$w-$art.txt"
      if [ $art = haupt ]; then audio=qpulse; ini=$T/panel-standard.ini; else audio=keiner; ini=$T/panel-mit-lautstaerke.ini; fi
      G2J_AUFLOESUNG=$r G2J_AUDIO=$audio G2J_PANELINI=$ini bash "$T/pruefen.sh" "$PANEL" "$MIXER" "$p" "$G/sz-$w-$art.txt" 400 >"$G/lauf-$p.txt" 2>&1
      $PY -I -W ignore "$T/auswerten.py" "$G/$p/logs" "$art" "$w" "$h" >"$G/ergebnis-$p.txt" 2>&1
      ergebnis "G2-J $art $r" "$G/ergebnis-$p.txt"
    done
  done
fi
[ "$NUR" = nur-g2j ] && exit 0
W=/home/user/projects/querencia-werkstatt-w2-panel        # nur gelesen: Szenarien der Fensterliste, Logo-Originale
R=$G/w2reg; Z=$R/werkzeug; GG=$R/gui; TT=$W/arbeit/tools/isolierte-gui-pruefung
cd "$R"
HMP_PROTOKOLL=1 HMP_NEUSTART=1 HMP_SZENARIO2=$Z/szenario-hmp11b-2.txt bash $Z/pruefen-w2.sh "$PANEL" neu-pager $Z/szenario-hmp11b.txt 200 >"$G/lauf-w2-pager.txt" 2>&1
$PY -I -W ignore $Z/auswerten-pager.py $GG hmp11b-neu neu-pager >"$G/ergebnis-w2-pager.txt" 2>&1; ergebnis "W2 Umschalter" "$G/ergebnis-w2-pager.txt"
for h in 32 24 48; do HMP_PANELHOEHE=$h HMP_VORHER=$Z/vorher-panelhoehe.sh bash $Z/pruefen-w2.sh "$PANEL" neu-logo-$h $Z/szenario-logo-$h.txt 150 >"$G/lauf-w2-logo-$h.txt" 2>&1; done
$PY -I -W ignore $Z/auswerten-logo.py $GG $W/hmp11a/logo-original neu-logo >"$G/ergebnis-w2-logo.txt" 2>&1; ergebnis "W2 Logo" "$G/ergebnis-w2-logo.txt"
bash $Z/pruefen-w2.sh "$PANEL" neu-dialog $Z/szenario-qrel04c.txt 150 >"$G/lauf-w2-dialog.txt" 2>&1
$PY -I -W ignore $Z/auswerten-branding.py $GG neu-dialog >"$G/ergebnis-w2-branding.txt" 2>&1; ergebnis "W2 Branding" "$G/ergebnis-w2-branding.txt"
HMP_VORHER=$Z/vorher-starter.sh HMP_NEUSTART=1 HMP_SZENARIO2=$Z/szenario-starter-2.txt bash $Z/pruefen-w2.sh "$PANEL" neu-starter $Z/szenario-starter.txt 240 >"$G/lauf-w2-starter.txt" 2>&1
python3 $Z/auswerten-starter.py $GG/neu-starter/logs >"$G/ergebnis-w2-starter.txt" 2>&1; ergebnis "W2 Starter" "$G/ergebnis-w2-starter.txt"
HMP_VORHER=$Z/vorher-starter.sh bash $Z/pruefen-w2.sh "$PANEL" neu-menue $Z/szenario-menue-dynamisch.txt 150 >"$G/lauf-w2-menue.txt" 2>&1
python3 $Z/auswerten-menue.py $GG/neu-menue/logs >"$G/ergebnis-w2-menue.txt" 2>&1; ergebnis "W2 Menü dynamisch" "$G/ergebnis-w2-menue.txt"
HMP_RC=1 bash $Z/pruefen-w2.sh "$PANEL" neu-fensterliste $TT/szenario-hmp06.txt 200 >"$G/lauf-w2-fensterliste.txt" 2>&1
$PY -I $Z/auswerten-fensterliste-w2.py $TT/erwartung-hmp06.json $GG/neu-fensterliste/logs >"$G/ergebnis-w2-fensterliste.txt" 2>&1; ergebnis "W2 Fensterliste" "$G/ergebnis-w2-fensterliste.txt"
echo "=== fertig $(date '+%F %T')" >>"$SUMME"
