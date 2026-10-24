# D32: Einstellungen wirken sofort (Einfacher Klick, Ansicht Liste), dateien.ini
# geschrieben. Mit LAUF_FILES_TOML zusätzlich Migration M6 beim ersten Start.
INI=$XDG_CONFIG_HOME/querencia/dateien.ini
starte "$HOME/Dokumente"
warte_log "Ordner gelesen" 15
if [ -n "${LAUF_M6:-}" ]; then
  grep -q "einmalig aus" "$LOG" && grep -q "AusFiles=1" "$INI" && grep -q "Ansicht=liste" "$INI" \
    && grep -q "Verborgene=true" "$INI" && grep -q "EinfacherKlick=true" "$INI" && grep -q "Anordnen=datum" "$INI" \
    && fall D32c "Migration M6 aus Files-settings.toml" 1 "dateien.ini: AusFiles=1, liste, verborgene, Einfachklick, datum" \
    || fall D32c "Migration M6" 0 "$(tr '\n' ' ' < "$INI")"
  cmp -s "$XDG_CONFIG_HOME/querencia-files/settings.toml" "$LAUF_M6" && fall D32d "Files-Datei unverändert" 1 "cmp" || fall D32d "Files-Datei unverändert" 0 "cmp"
  foto 00-nach-migration
  exit 0
fi
menue bearbeiten
q "key Up" "key Return" "wait 700"
q "move 171 54" "click" "wait 300"
foto 01-verhalten
q "move 50 151" "click" "wait 300"
grep -q "EinfacherKlick=true" "$INI" && fall D32a "Einfacher Klick in dateien.ini" 1 "EinfacherKlick=true" || fall D32a "Einfacher Klick" 0 "dateien.ini"
q "move 70 54" "click" "wait 300" "move 518 119" "click" "wait 400"
foto 02-ansicht-wahl
q "key Down" "key Return" "wait 300"
grep -q "Ansicht=liste" "$INI" && fall D32b "Neue Ordner in Listenansicht, dateien.ini" 1 "Ansicht=liste" || fall D32b "Ansicht Liste" 0 "dateien.ini"
foto 03-ansichten
q "key Escape" "wait 300"
: > "$HOME/aufrufe.log"
q "move 300 202" "click" "wait 800"
grep -q "attrappe-dokumente .*angebot.pdf" "$HOME/aufrufe.log" && fall D32e "Einfacher Klick wirkt sofort im offenen Fenster" 1 "aufrufe.log" || fall D32e "Einfacher Klick sofort" 0 "aufrufe.log"
foto 04-nach-klick
