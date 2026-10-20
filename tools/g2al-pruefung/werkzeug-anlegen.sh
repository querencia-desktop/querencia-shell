#!/bin/bash
# G2-AL: legt alle Prüfwerkzeuge als umgelenkte Kopien unter nachweis/gui an,
# flach und ohne die verschachtelten Anlage-Skripte der Vorlagen (deren
# eingetragene Pfade zeigten auf main-Stand teils in fremde Worktrees, etwa
# tools/kalender-pruefung auf querencia-runtime-wt-g2ai).
#   nachweis/gui/gm-werkzeug  tools/netzwerk-pruefung (G2-M: isoliert.py,
#                             testprofil.sh, qeingabe, pruefen.sh …)
#   nachweis/gui/gj-werkzeug  tools/lautstaerke-pruefung (G2-J, W2)
#   nachweis/gui/ad-werkzeug  tools/kalender-pruefung (G2-AD, G2-Y)
#   nachweis/gui/ak-werkzeug  tools/g2ak-pruefung (G2-AK)
#   nachweis/gui/bin          qeingabe/qausgabe, qx11fenster
# Gegenüber den Vorlagen geändert (nur in den Kopien), wie dort beschrieben:
#   - G2-AD: Symbolpfad und Zweitauswertung G2-Y im Auswerter, endlaeufe.sh
#     ohne g2jreg (kalender-pruefung/werkzeug-anlegen.sh);
#   - G2-AK: Menüzeilen von Abmelden/Ausschalten eine Zeile tiefer
#     (g2ak-pruefung/werkzeug-anlegen.sh);
#   - qeingabe kennt bei keydown/keyup zusätzlich F11, f, l (G2-AK) und d
#     (G2-AL, Super+D).
# Die Anlage-Skripte in den Kopien sind durch einen Stopp ersetzt: Neu
# angelegt wird nur über dieses Skript. Zum Schluss darf in keiner Kopie ein
# Pfad in einen fremden Worktree stehen.
set -euo pipefail
WT=/home/user/projects/querencia-runtime-wt-g2al
G=$WT/nachweis/gui
M=$G/gm-werkzeug J=$G/gj-werkzeug A=$G/ad-werkzeug K=$G/ak-werkzeug
weg() { # nur die eigenen Kopien unter nachweis/gui
  case "$1" in "$M" | "$J" | "$A" | "$K") ;; *) exit 2 ;; esac
  if [ -e "$1" ]; then chmod -R u+w "$1"; rm -rf "$1"; fi
}
stopp() { # ersetzt ein Anlage-Skript der Kopie
  cat >"$1" <<'X'
#!/bin/bash
echo "G2-AL: Werkzeuge nur über tools/g2al-pruefung/werkzeug-anlegen.sh anlegen" >&2
exit 2
X
}
umlenken() { # <ordner> <alter-tools-ordner> <neuer-ort> [weitere sed-Ausdrücke]
  local dir=$1 tools=$2 ziel=$3; shift 3
  grep -rlE 'querencia-runtime-wt-|\$WT/tools/|\$WT/nachweis/gui|"\$WT"/nachweis/gui' "$dir" | while read -r f; do
    sed -i -E -e 's#\$WT/tools/'"$tools"'#'"$ziel"'#g' "$@" \
      -e 's#querencia-runtime-wt-(netzwerk|lautstaerke|statussymbole|kalender|g2ai|g2ak)([/"'"'"' ]|$)#querencia-runtime-wt-g2al\2#g' \
      -e 's#wt-(netzwerk|lautstaerke|statussymbole|kalender|g2ai|g2ak)/nachweis#wt-g2al/nachweis#g' "$f"
  done
}
mkdir -p "$G" "$G/gj"
for d in "$M" "$J" "$A" "$K"; do weg "$d"; done

# G2-M (wie kalender-pruefung/werkzeug-anlegen.sh)
cp -a "$WT/tools/netzwerk-pruefung" "$M"
umlenken "$M" netzwerk-pruefung "$M"
rm -f "$M/integration-probe.sh" "$M/g2jreg-anlegen.sh" "$M/symbole-machen.py"
sed -i 's#^SYMBOLE = Path(__file__).resolve().parent.parent.parent / #SYMBOLE = Path("'"$WT"'") / #' "$M/auswerten.py"
grep -q "^SYMBOLE = Path(\"$WT\") / \"crates/querencia-netzwerk/data/symbole\"" "$M/auswerten.py"
sed -e 's#^    bg = pixel(pb, NX - 13, 2)$#    bg = pixel(pb, NX - 9, 2)   \# G2-Y: im 24-px-Statusknopf#' "$M/auswerten.py" >"$M/auswerten-g2y.py"
[ "$(diff "$M/auswerten.py" "$M/auswerten-g2y.py" | grep -c '^[<>]')" = 2 ]
sed -i -e '/g2jreg-anlegen.sh\|g2jreg\/werkzeug/d' "$M/endlaeufe.sh"
# Szenarien drücken Super+F, Alt+F11, Strg+Alt+L (G2-AK) und Super+D (G2-AL);
# die Vorlage kennt diese Buchstaben- und F-Tasten nicht
sed -i 's#                     : !strcmp(arg, "F4") ? KEY_F4#                     : !strcmp(arg, "F4") ? KEY_F4 : !strcmp(arg, "F11") ? KEY_F11  /* G2-AK */\n                     : !strcmp(arg, "f") ? KEY_F : !strcmp(arg, "l") ? KEY_L : !strcmp(arg, "d") ? KEY_D  /* G2-AL */#' "$M/qeingabe.c"
grep -q 'KEY_D  /\* G2-AL' "$M/qeingabe.c"

# G2-J mit W2 (wie kalender-pruefung/werkzeug-anlegen.sh)
cp -a "$WT/tools/lautstaerke-pruefung" "$J"
grep -rl 'querencia-runtime-wt-lautstaerke\|\$WT/nachweis/gui\|\$WT/tools/lautstaerke-pruefung' "$J" | while read -r f; do
  sed -i -e "s#/querencia-runtime-wt-lautstaerke/nachweis/gui#/querencia-runtime-wt-g2al/nachweis/gui/gj#g" \
         -e 's#\$WT/tools/lautstaerke-pruefung#'"$J"'#g' \
         -e 's#"\$WT"/nachweis/gui#"$WT"/nachweis/gui/gj#g' \
         -e 's#\$WT/nachweis/gui#$WT/nachweis/gui/gj#g' \
         -e "s#querencia-runtime-wt-lautstaerke#querencia-runtime-wt-g2al#g" "$f"
done

# G2-AD/G2-Y
cp -a "$WT/tools/kalender-pruefung" "$A"
umlenken "$A" kalender-pruefung "$A"
stopp "$A/werkzeug-anlegen.sh"
rm -f "$A/integration-probe.sh"   # Probe der G2-AD-Integration, hier nicht gebraucht
# regression.sh legte die Kopien neu an: hier schon geschehen
sed -i 's#^bash "'"$A"'/werkzeug-anlegen.sh"$#: \# G2-AL: Werkzeuge von tools/g2al-pruefung/werkzeug-anlegen.sh#' "$A/regression.sh"
grep -q '^: # G2-AL: Werkzeuge' "$A/regression.sh"
# G2-AK: im System-Menü steht „Bildschirm sperren“ vor „Abmelden …“
sed -i 's#^MENU_X, ABMELDEN, AUSSCHALTEN = 300, 78, 102$#MENU_X, ABMELDEN, AUSSCHALTEN = 300, 102, 126  \# G2-AK: Sperren davor#' "$A/szenario-g2ad.py"
grep -q '^MENU_X, ABMELDEN, AUSSCHALTEN = 300, 102, 126' "$A/szenario-g2ad.py"

# G2-AK
cp -a "$WT/tools/g2ak-pruefung" "$K"
umlenken "$K" g2ak-pruefung "$K" -e 's#\$WT/tools/kalender-pruefung#'"$A"'#g'
stopp "$K/werkzeug-anlegen.sh"

# Kontrolle: kein Pfad in einen fremden Worktree, Laufzeitordner im Kopieziel
fremd=$(grep -rnE 'querencia-runtime-wt-' "$M" "$J" "$A" "$K" | grep -v 'querencia-runtime-wt-g2al' || true)
[ -z "$fremd" ] || { echo "Pfad in einen fremden Worktree übrig:"; echo "$fremd"; exit 2; }
fremd=$(grep -rnE 'wt-[a-z0-9]+/nachweis' "$M" "$J" "$A" "$K" | grep -v 'wt-g2al/nachweis' || true)
[ -z "$fremd" ] || { echo "Laufzeitordner außerhalb:"; echo "$fremd"; exit 2; }
grep -q '/querencia-runtime-wt-g2al/nachweis/gui/rt/g2m-' "$M/qeingabe.c"
grep -q '^RT_BASE = "/home/user/projects/querencia-runtime-wt-g2al/nachweis/gui/rt"$' "$M/isoliert.py"
grep -q '\[ "$basis" = /home/user/projects/querencia-runtime-wt-g2al/nachweis/gui \]' "$M/testprofil.sh"

mkdir -p "$G/bin"
bash "$M/bauen.sh" "$G/bin"
gcc -O2 -Wall -Wextra -o "$G/bin/qx11fenster" "$WT/tools/g2ak-pruefung/qx11fenster.c" -lX11
echo "Werkzeuge angelegt: $M $J $A $K $G/bin"
