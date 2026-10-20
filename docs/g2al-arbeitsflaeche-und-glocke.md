# G2-AL – Panel: „Arbeitsfläche anzeigen“ und Glocke der Mitteilungszentrale

Stand 20.10.2026. Zweig `g2al` ab `main` `548483c` (G2-AK/G2-AI). Punkte
QD-078 (Knopf „Arbeitsfläche anzeigen“, Super+D) und QD-073 Teil 2 (Glocke
der Mitteilungszentrale).

## 1. „Arbeitsfläche anzeigen“ (QD-078)

Applet-Typ `show-desktop` („Arbeitsfläche anzeigen“), im Standardlayout
ganz links im unteren Panel vor der Fensterliste (`show-desktop-1`), wie
der Knopf im GNOME-2- und MATE-Panel. Symbol `user-desktop` aus dem
Symbolthema, Aussehen wie ein Starter (nur das Symbol, neutrale
Hervorhebung); 4 px Abstand zur Fensterliste.

- Erster Klick: minimiert alle Fenster der aktuellen Arbeitsfläche, die
  nicht schon minimiert sind, und merkt sie sich (Merkliste). Der Knopf
  bleibt gedrückt, Tooltip „Fenster wiederherstellen“.
- Zweiter Klick: stellt genau diese Fenster wieder her, in der Reihenfolge
  ihrer letzten Aktivierung; das zuletzt aktive liegt danach oben und hat
  den Fokus. Tooltip wieder „Arbeitsfläche anzeigen“.
- Die Merkliste endet, wenn inzwischen ein anderes Fenster aktiviert wurde
  (etwa ein neues Fenster oder ein Klick in der Fensterliste), ein
  gemerktes Fenster anders wiederhergestellt wurde oder die Arbeitsfläche
  wechselte. Der nächste Klick minimiert dann wieder, was offen ist.
  Geschlossene Fenster fallen aus der Merkliste.
- Ohne Fenster: nichts (Log „kein Fenster auf der Arbeitsfläche“).

Umsetzung: `arbeitsflaeche_zeigen.rs` (Merkliste als reine Logik mit
Tests, Knopf) und `window_list.rs`: die Fensterliste besitzt die
wlr-foreign-toplevel-Anbindung, führt die Merkliste aus denselben
Ereignissen nach und schickt `set_minimized`/`unset_minimized` (+
`activate` für das letzte). „Aktuelle Arbeitsfläche“ ist die Zuordnung der
Fensterliste aus G2-AK (`fensterliste.rs`): Fenster, deren Fläche die Liste
nicht kennt (vor dem Panelstart geöffnet und seitdem nie aktiviert), zählen
mit. labwc fokussiert beim Minimieren des aktiven Fensters das nächste
(`view_minimize`); solche Zwischenaktivierungen gemerkter Fenster beenden
die Merkliste nicht.

Ohne Fensterliste im Layout (oder ohne foreign-toplevel) tut der Knopf
nichts außer einer Logzeile.

### D-Bus und Super+D

Am Panel-Busnamen `org.example.Querencia.Panel`, Objekt
`/org/example/Querencia/Panel`, Schnittstelle `org.querencia.Panel` (wie
`NeuLaden`):

| Methode | Antwort | Wirkung |
|---|---|---|
| `ArbeitsflaecheUmschalten` | `(b ausgefuehrt, s meldung)` | wie ein Klick auf den Knopf; geht auch ohne Knopf, solange es eine Fensterliste gibt |

`false` heißt: keine Fensterliste oder keine Verbindung. Die labwc-Vorgabe
`session/querencia/labwc/rc.xml` bindet Super+D:

```
gdbus call --session --dest org.example.Querencia.Panel \
    --object-path /org/example/Querencia/Panel \
    --method org.querencia.Panel.ArbeitsflaecheUmschalten
```

Die Kürzel-Übersicht (`querencia-kuerzel`, ab dem nächsten Bau) beschreibt
es als „Arbeitsfläche anzeigen: alle Fenster minimieren oder
wiederherstellen“ in der Gruppe Fenster; ältere Bauten zeigen „startet
„gdbus““.

## 2. Glocke der Mitteilungszentrale (QD-073 Teil 2)

Applet-Typ `benachrichtigungen` („Mitteilungen“), ein Statussymbol wie
Lautstärke und Netzwerk (2 px Abstand, gleiche Optik), im Standardlayout
vor der Uhr (`benachrichtigungen-1`); beim Hinzufügen rechts neben
Netzwerk, sonst Lautstärke.

- Zeigt die Zahl ungelesener Mitteilungen rechts neben der Glocke (fett,
  ab 100 „99+“); mit ungelesenen Mitteilungen eigene Glocke mit orangem
  Klöppel und Punkt.
- Linksklick: `ZentraleZeigen` mit einem xdg-activation-Token des Panels;
  ist die Zentrale offen (`ZentraleOffen`), `ZentraleSchliessen`.
- Rechtsklick: „Nicht stören“ (Häkchen, Eigenschaft `NichtStoeren`), „Alle
  als gelesen markieren“ (`AlsGelesen []`), „Aus dem Panel entfernen“,
  „Verschieben“.
- „Nicht stören“ an: durchgestrichene, gedämpfte Glocke (Zahl bleibt).
- Ohne Besitzer von `org.querencia.Benachrichtigungen` (kein Dienst oder
  Benachrichtigungen vor 0.2.0): ausgegraut, keine Zahl, Menüeinträge
  gesperrt, Tooltip „Mitteilungszentrale nicht verfügbar“; das Log sagt es
  einmal je Wechsel.

Kein Polling: Der Busname wird beobachtet (`g_bus_watch_name`, nie
Autostart), beim Erscheinen einmal `Properties.GetAll`, danach
`PropertiesChanged` und das Signal `Zaehler`. Schnittstelle:
`docs/dbus.md` in querencia-benachrichtigungen 0.2.0. Logzeilen nennen nur
Zahlen, nie Inhalte von Mitteilungen.

Symbole: `crates/querencia-panel/data/symbole/glocke.svg`, `glocke-neu.svg`,
`glocke-ruhe.svg` (Querencia-Stil wie Lautstärke/Netzwerk, ins Binary
eingebaut).

## 3. panel.ini

Das Standardlayout (ohne `panel.ini`) hat beide Applets. Eine bestehende
`panel.ini` ändert das Panel nie von selbst; dafür gibt es den Schritt der
Integration:

```
querencia-panel --panel-ini ergaenzen|zuruecknehmen|pruefen <datei>
```

läuft ohne GTK und ohne Bildschirm. `ergaenzen` setzt `show-desktop-1`
direkt vor die Fensterliste und `benachrichtigungen-1` direkt vor die Uhr
(ohne Uhr ans Ende des oberen Panels), mit eigener `[Applet …]`-Gruppe; was
schon da ist, bleibt; eine belegte ID bekommt die nächste Nummer.
`zuruecknehmen` nimmt alle Applets beider Typen und ihre Gruppen heraus;
nach Ergänzen und Zurücknehmen ist die Datei byte-gleich. Kommentare und
alle anderen Gruppen bleiben (GLib-Schlüsseldatei), geschrieben wird
atomar. Danach zeigt der Befehl das Layout, wie das Panel es liest.

Ein laufendes älteres Panel (G2-AK) liest die geänderte Datei neu,
überspringt die zwei unbekannten Typen mit einer Logzeile und baut nichts
um.

## 4. Prüfung

- Unit-Tests: Merkliste (`arbeitsflaeche_zeigen.rs`), Zustand/Zähler/Menü der
  Glocke (`glocke.rs`), Ergänzung (`ergaenzung.rs`), Layout, Platzierung,
  Abstände, CSS, D-Bus-Schnittstelle, Kürzel-Beschreibung.
- Isoliert (`tools/g2al-pruefung/`, eigener headless labwc, eigener Bus, XDG-
  Profil): `laeufe-g2al.sh <panel>` mit den Arten `zeigen`, `superd`,
  `glocke`, `ergaenzung-alt`, `ergaenzung-neu`, `standard` je 1024×768 und
  1920×1080; Benachrichtigungen 0.2.0 und das G2-AK-Panel nur gelesen aus
  ihren Ablagen. `regression.sh <panel>`: G2-AK, G2-AD, G2-Y, G2-M, G2-J/W2
  mit `QS_TROCKENLAUF=1`.
- Integration: `integration/g2al/panel-integrieren.sh` (`QAL_FREIGABE=ja`),
  Probe `integration/g2al/probe.sh`.

## 5. Grenzen

- Die Reihenfolge beim Wiederherstellen folgt der letzten Aktivierung;
  labwc meldet keine Stapelreihenfolge.
- Nach einem Flächenwechsel holt der Knopf die vorher minimierten Fenster
  nicht mehr zurück (Merkliste beendet); das geht dann über die
  Fensterliste.
- Fenster, deren Arbeitsfläche die Fensterliste nicht kennt, werden auf
  jeder Fläche mit minimiert.
- Das Symbolthema Querencia zeichnet `user-desktop` als Schreibtisch-Ordner;
  ein eigenes Symbol für den Knopf wäre ein Auftrag fürs Symbole-Repo.
- Das Glockenmenü ist wie die Menüs von Lautstärke und Netzwerk ein
  GTK-Popover-Menü, nicht das klassische Menü der Hauptmenüs.
