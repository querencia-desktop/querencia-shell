# Welle C3 Migration (B §4.2 M6, M1; B §5.2 B32): erster Start mit und ohne
# Querencia-Files-settings.toml. Mit LAUF_FILES_TOML (lauf.sh legt die Datei
# nach ~/.config/querencia-files/, mit LAUF_FILES_ORT=strata wie bei Files bis
# Alpha 2.1 nach ~/.config/strata/) wird einmal übernommen und vermerkt, die
# Files-Datei bleibt unverändert, ein zweiter Start übernimmt nicht noch
# einmal. Ohne sie schreibt der erste Start dateien.ini mit den Vorgaben, eine
# später auftauchende settings.toml wird nicht übernommen. M1: Lesezeichen
# aus ~/.config/gtk-3.0/bookmarks (Files' Datei) erscheinen unverändert.
INI=$XDG_CONFIG_HOME/querencia/dateien.ini
ORT=${LAUF_M6_ORT:-querencia-files}
TOML=$XDG_CONFIG_HOME/$ORT/settings.toml
BM=$XDG_CONFIG_HOME/gtk-3.0/bookmarks
zweiter_start() {
  ende
  : > "$LOG.alt"; cat "$LOG" >> "$LOG.alt"; : > "$LOG"; LOG_STAND=0
  starte "$HOME"
  warte_log "Ordner gelesen" 15
}
if [ -n "${LAUF_M6:-}" ]; then
  vorher=$(sha256sum < "$TOML")
  [ ! -e "$INI" ] || fall M6a "Vor dem ersten Start keine dateien.ini" 0 "$INI"
  starte "$HOME"
  warte_log "Ordner gelesen" 15
  [ "$ORT" = querencia-files ] || [ ! -e "$XDG_CONFIG_HOME/querencia-files" ] \
    || fall M6a "Ausgangsstand $ORT ohne querencia-files" 0 "$XDG_CONFIG_HOME/querencia-files"
  grep -a -q "Einstellungen einmalig aus .*/$ORT/settings.toml übernommen" "$LOG" \
    && grep -q "AusFiles=1" "$INI" && grep -q "Ansicht=liste" "$INI" && grep -q "Verborgene=true" "$INI" \
    && grep -q "Anordnen=datum" "$INI" && grep -q "EinfacherKlick=true" "$INI" \
    && fall M6a "Erster Start übernimmt aus Files ($ORT/settings.toml): columns → Liste, verborgene, Datum, Einfacher Klick; Vermerk AusFiles=1" 1 "programm.log, dateien.ini" \
    || fall M6a "Übernahme aus Files" 0 "$(tr '\n' ' ' < "$INI" | cut -c1-200)"
  [ "$(sha256sum < "$TOML")" = "$vorher" ] && [ -e "$TOML" ] \
    && fall M6b "Files-Datei unverändert und nicht gelöscht" 1 "sha256 gleich" || fall M6b "Files-Datei unverändert" 0 "sha256"
  q "wait 300"
  foto 01-nach-migration
  # ändert der Benutzer etwas, bleibt es; ein zweiter Start übernimmt nicht
  taste ctrl+h
  q "wait 300"
  printf 'show_hidden = true\nbrowser_mode = "icons"\n' > "$TOML"
  zweiter_start
  ! grep -a -q "einmalig aus" "$LOG" && grep -q "Verborgene=false" "$INI" && grep -q "Ansicht=liste" "$INI" \
    && fall M6c "Zweiter Start: keine zweite Übernahme, Änderung (Strg+H) bleibt" 1 "kein „einmalig“, Verborgene=false, Ansicht=liste" \
    || fall M6c "Zweiter Start" 0 "$(tr '\n' ' ' < "$INI" | cut -c1-200)"
  foto 02-zweiter-start
  exit 0
fi
# ohne Files-Einstellungen
mkdir -p "$(dirname "$BM")"
printf 'file://%s/Tief Tiefe Ordner\nfile://%s/Vorlagen\n' "$HOME" "$HOME" > "$BM"
bm_vorher=$(cat "$BM")
starte "$HOME"
warte_log "Ordner gelesen" 15
q "wait 300"
! grep -a -q "einmalig aus" "$LOG" && [ -e "$INI" ] && ! grep -q "AusFiles" "$INI" && grep -q "Ansicht=symbole" "$INI" \
  && fall M6d "Erster Start ohne Files: dateien.ini sofort mit Vorgaben, ohne Vermerk" 1 "dateien.ini da, Ansicht=symbole, kein AusFiles" \
  || fall M6d "Erster Start ohne Files" 0 "$(ls "$XDG_CONFIG_HOME/querencia" 2>&1)"
foto 03-ohne-files
menue lesezeichen
q "wait 300"
foto 04-lesezeichen-menue
q "key Escape"
taste ctrl+d
q "wait 300"
[ "$(head -n 2 "$BM")" = "$bm_vorher" ] && grep -q "^file://$HOME\$" "$BM" \
  && fall M1 "Lesezeichen: gleiche Datei gtk-3.0/bookmarks, vorhandene Einträge bleiben, Strg+D hängt an" 1 "$(wc -l < "$BM") Zeilen" \
  || fall M1 "Lesezeichen" 0 "$(tr '\n' ' ' < "$BM")"
mkdir -p "$(dirname "$TOML")"
printf 'show_hidden = true\nbrowser_mode = "list"\n' > "$TOML"
zweiter_start
! grep -a -q "einmalig aus" "$LOG" && grep -q "Verborgene=false" "$INI" && grep -q "Ansicht=symbole" "$INI" \
  && fall M6e "Später auftauchende Files-Einstellungen werden nicht übernommen" 1 "Verborgene=false, Ansicht=symbole" \
  || fall M6e "Späte settings.toml" 0 "$(tr '\n' ' ' < "$INI" | cut -c1-200)"
