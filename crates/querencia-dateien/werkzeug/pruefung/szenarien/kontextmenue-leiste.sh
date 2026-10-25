# QD-176 (Dateien 0.2.2): ein offenes Kontextmenü springt nicht in die
# Menüleiste, die Menüleiste wechselt per Zeiger nur über ihren Knöpfen.
# Koordinaten für LAUF_GROESSE=1024x768 (Fenster bei 0,0, wie fotos.sh):
# Knöpfe der Leiste y 28–52, Datei x 3–84, Bearbeiten 84–208, Ansicht
# 208–288; notiz.txt in Dokumente bei 215,270.
# QUERENCIA_MENU_DEBUG=1 protokolliert jeden Wechsel („Hauptmenü per
# Zeiger: A -> B“, „Hauptmenü per Taste: A -> B“).
export QUERENCIA_MENU_DEBUG=1 QUERENCIA_DATEIEN_DEBUG=1
seit() { tail -n +"$((LOG_STAND + 1))" "$LOG" | grep -c -- "$1"; }
seit_zeilen() { tail -n +"$((LOG_STAND + 1))" "$LOG" | grep -- "$1" | tr '\n' ' ' | cut -c1-160; }
D=$HOME/Dokumente
starte "$D"
warte_log "Ordner gelesen" 15 || fall K00 "Start in Dokumente" 0 "keine Logzeile"
q "wait 600"

# K01 Fehlerbild von pottrauschen: Rechtsklick auf eine Datei, Zeiger im
# Kontextmenü zu „Umbenennen …“ (Weg beginnt am Klickpunkt, also im
# x-Bereich von „Ansicht“), dann Klick.
q "move 215 270" "click" "wait 200"
markiere
q "rclick" "wait 500"
warte_log "Kontextmenü: " 3
markiere
q "glide 230 300 150" "glide 260 395 250" "wait 400"
foto k01-zeiger-auf-umbenennen
n=$(seit "Hauptmenü per Zeiger")
aktiv=$(seit "aktiv=Umbenennen")
[ "$n" = 0 ] && [ "$aktiv" -ge 1 ] \
  && fall K01 "Kontextmenü Datei, Zeiger zu „Umbenennen …“: kein Wechsel, Zeile aktiv" 1 "Wechsel 0, aktiv=Umbenennen $aktiv×" \
  || fall K01 "Kontextmenü Datei, Zeiger zu „Umbenennen …“" 0 "Wechsel $n: $(seit_zeilen 'Hauptmenü per'); aktiv=Umbenennen $aktiv×"
markiere
q "click" "wait 500"
if warte_log "Umbenennen: " 3; then
  q "type umbenannt" "key Return" "wait 800"
  [ -e "$D/umbenannt.txt" ] && [ ! -e "$D/notiz.txt" ] \
    && fall K02 "Umbenennen aus dem Kontextmenü: notiz.txt → umbenannt.txt" 1 "$(ls "$D" | tr '\n' ' ' | cut -c1-120)" \
    || fall K02 "Umbenennen aus dem Kontextmenü" 0 "$(ls "$D" | tr '\n' ' ' | cut -c1-120)"
else
  fall K02 "Umbenennen aus dem Kontextmenü" 0 "kein Umbenennen-Feld; $(seit_zeilen 'Hauptmenü per')"
  q "key Escape" "wait 300" "key Escape" "wait 300"
fi

# K03 Kontextmenü Datei offen, Zeiger verlässt es nach unten in den
# x-Bereich von „Ansicht“ (Fensterfläche, nicht das Menü).
q "move 470 270" "click" "wait 200" "rclick" "wait 500"
markiere
q "glide 300 500 200" "glide 250 500 150" "wait 400"
foto k03-zeiger-unter-dem-kontextmenue
n=$(seit "Hauptmenü per Zeiger")
[ "$n" = 0 ] && fall K03 "Kontextmenü Datei, Zeiger darunter bei x von „Ansicht“: kein Wechsel" 1 "Wechsel 0" \
  || fall K03 "Kontextmenü Datei, Zeiger darunter" 0 "Wechsel $n: $(seit_zeilen 'Hauptmenü per')"
q "key Escape" "wait 300"

# K04 Kontextmenü des Hintergrunds, Zeiger nach links in den x-Bereich
# von „Ansicht“.
q "move 650 470" "click" "wait 200" "rclick" "wait 500"
markiere
q "glide 400 520 200" "glide 250 520 150" "wait 400"
foto k04-hintergrund
n=$(seit "Hauptmenü per Zeiger")
[ "$n" = 0 ] && fall K04 "Kontextmenü Hintergrund, Zeiger bei x von „Ansicht“: kein Wechsel" 1 "Wechsel 0" \
  || fall K04 "Kontextmenü Hintergrund" 0 "Wechsel $n: $(seit_zeilen 'Hauptmenü per')"
q "key Escape" "wait 300"

# K05 Kontextmenü der Seitenleiste (Musik), Zeiger in die Ansicht.
q "move 60 326" "rclick" "wait 500"
markiere
q "glide 250 530 250" "wait 400"
foto k05-seitenleiste
n=$(seit "Hauptmenü per Zeiger")
[ "$n" = 0 ] && fall K05 "Kontextmenü Seitenleiste, Zeiger bei x von „Ansicht“: kein Wechsel" 1 "Wechsel 0" \
  || fall K05 "Kontextmenü Seitenleiste" 0 "Wechsel $n: $(seit_zeilen 'Hauptmenü per')"
q "key Escape" "wait 300"

# K06 Tastatur im Kontextmenü: Rechts auf einem Eintrag ohne Untermenü und
# Links in der obersten Ebene wechseln nicht in die Menüleiste.
q "move 600 160" "click" "wait 200" "rclick" "wait 500"
markiere
q "key Down" "wait 200" "key Right" "wait 300" "key Left" "wait 300"
foto k06-tastatur
n=$(seit "Hauptmenü per")
[ "$n" = 0 ] && fall K06 "Kontextmenü: Rechts/Links wechseln nicht in die Leiste" 1 "Wechsel 0" \
  || fall K06 "Kontextmenü: Rechts/Links" 0 "Wechsel $n: $(seit_zeilen 'Hauptmenü per')"
q "key Escape" "wait 300" "key Escape" "wait 300"

# K07 Menüleiste per Zeiger: Datei offen, Zeiger über Bearbeiten und
# Ansicht wechselt (GNOME 2).
q "move 40 39" "click" "wait 500"
markiere
q "glide 140 39 200" "wait 400" "glide 250 39 200" "wait 400"
foto k07-leiste-per-zeiger
b=$(seit "Hauptmenü per Zeiger: _Datei -> _Bearbeiten"); a=$(seit "Hauptmenü per Zeiger: _Bearbeiten -> _Ansicht")
[ "$b" = 1 ] && [ "$a" = 1 ] && fall K07 "Leiste per Zeiger: Datei → Bearbeiten → Ansicht" 1 "$(seit_zeilen 'Hauptmenü per')" \
  || fall K07 "Leiste per Zeiger" 0 "$(seit_zeilen 'Hauptmenü per')"

# K08 Menü der Leiste offen (Ansicht), Zeiger unterhalb der Leiste im
# x-Bereich von „Datei“: kein Wechsel, nur über dem Knopf.
markiere
q "glide 250 530 200" "glide 40 530 250" "wait 400"
foto k08-leiste-zeiger-darunter
n=$(seit "Hauptmenü per Zeiger")
[ "$n" = 0 ] && fall K08 "Leiste: Zeiger unter der Leiste bei x von „Datei“: kein Wechsel" 1 "Wechsel 0" \
  || fall K08 "Leiste: Zeiger unter der Leiste" 0 "Wechsel $n: $(seit_zeilen 'Hauptmenü per')"
markiere
q "glide 40 39 250" "wait 400"
n=$(seit "Hauptmenü per Zeiger: _Ansicht -> _Datei")
[ "$n" = 1 ] && fall K09 "Leiste: zurück über „Datei“ wechselt" 1 "$(seit_zeilen 'Hauptmenü per')" \
  || fall K09 "Leiste: zurück über „Datei“" 0 "$(seit_zeilen 'Hauptmenü per')"
q "key Escape" "wait 300"

# K10 Menüleiste per Tastatur: Alt+D, Rechts, Rechts. Der Zeiger steht
# dabei nicht über der Leiste (wie T9 der Panel-Prüfung).
q "move 600 450" "wait 200"
markiere
menue datei
q "key Right" "wait 300" "key Right" "wait 300"
n=$(seit "Hauptmenü per Taste")
[ "$n" = 2 ] && [ "$(seit 'Hauptmenü per Taste: _Bearbeiten -> _Ansicht')" = 1 ] && fall K10 "Leiste per Tastatur: Datei → Bearbeiten → Ansicht" 1 "$(seit_zeilen 'Hauptmenü per Taste')" \
  || fall K10 "Leiste per Tastatur" 0 "$(seit_zeilen 'Hauptmenü per Taste')"
q "key Escape" "wait 300"

# K11 Umbenennen per F2 bleibt.
q "move 215 160" "click" "wait 300"
markiere
q "key F2" "wait 400" "type per-f2" "key Return" "wait 800"
ls "$D" | grep -q '^per-f2' \
  && fall K11 "F2 benennt um" 1 "$(ls "$D" | grep '^per-f2')" \
  || fall K11 "F2 benennt um" 0 "$(ls "$D" | tr '\n' ' ' | cut -c1-120)"
foto k11-ende
