# M1.5b – System-Menü

**Datum:** 2026-10-14
**Status:** abgeschlossen. Der Platzhalter „System“ im oberen Panel ist
ein Menü mit den Untermenüs „Einstellungen“ und „Systemverwaltung“. Die
Einträge kommen aus dem vorhandenen GIO-Katalog und starten über GIO wie
im Anwendungsmenü.
**Code:** `crates/classic2-panel/src/system_menu.rs` (neu), `apps.rs`,
`app_menu.rs`, `main.rs`, `apps/tests.rs`, zwei neue Fixtures.
**Grundlage:** technischer Entwurf M1.5a (Architektur-Review freigegeben),
Code-Review des Patches (GO, keine Korrekturen).
**Belege:** `assets/m1-5b/`, Testwerkzeug in `assets/m1-5b/werkzeug/`.

## Menüstruktur

```
System
├─ Einstellungen ▸       Settings ohne System
└─ Systemverwaltung ▸    Settings mit System
```

- Ein Untermenü erscheint nur, wenn es Einträge hat. Ohne jede
  Einstellung zeigt das Menü „Keine Einstellungen“ ohne Aktion.
- Innerhalb eines Abschnitts wird nach Namen sortiert, mit der Locale des
  Prozesses (`CollationKey`), bei gleichem Namen nach Desktop-ID.
- Wie im Anwendungsmenü: `MenuButton` mit Sliding-`PopoverMenu`, keine
  Symbole, `KeyboardMode::None` unverändert.
- Sperren, Abmelden, Neustarten und Herunterfahren gehören nicht zu
  diesem Schritt.

## GIO-Klassifizierung

- Quelle ist ausschließlich `apps::Catalog`. GIO hat dort `Hidden`,
  `TryExec`/`Exec`, `NoDisplay` und `OnlyShowIn`/`NotShowIn` gegen
  `XDG_CURRENT_DESKTOP` bereits ausgewertet. Es gibt keine zweite
  Anwendungserkennung und keine festen Desktop-IDs.
- `AppEntry::system_section()` folgt der Regel aus GNOME 2
  (`settings.menu`: Preferences/Administration): `Settings` und `System`
  ergeben `Administration`, nur `Settings` ergibt `Preferences`, ohne
  `Settings` erscheint der Eintrag nicht. Die Kategorienamen sind
  groß-/kleinschreibungsgenau.
- `Catalog::system_sections()` liefert die nicht leeren Abschnitte in
  Menüreihenfolge. Die Katalogreihenfolge taugt dafür nicht, weil sie
  zuerst nach Menügruppe sortiert.
- Das Anwendungsmenü lässt Einträge mit `is_settings()` weiterhin aus;
  seit M1.5b erscheinen sie im System-Menü statt nirgends.
- Systemwerkzeuge ohne `Settings` (htop, foot) bleiben unter Anwendungen
  → Systemwerkzeuge, wie in GNOME 2.

## Gemeinsame Logik mit dem Anwendungsmenü

`app_menu.rs` stellt drei Bausteine bereit, `system_menu.rs` nutzt sie:

- `add_launch_action(button, group)`: eigene `SimpleActionGroup` je Menü
  mit der Aktion `launch` (Ziel: Desktop-ID). Das Anwendungsmenü hat
  `menu.launch`, das System-Menü `system.launch`.
- `launch_item(entry, group)`: Menüeintrag mit Beschriftung und Aktion.
- `follow_changes(button, menu, name, rebuild)`: verbindet den
  `AppInfoMonitor` mit einem Neuaufbau. Jedes Menü hat einen eigenen
  Handler und ein eigenes Merkzeichen. Ein offenes Menü wird nie umgebaut;
  die Änderung wird vorgemerkt und nach dem Schließen nachgeholt.
  Gleichzeitig wird das andere, geschlossene Menü sofort neu gebaut. Wer
  den Katalog zuerst neu liest, schaltet den Monitor für beide wieder
  scharf.

Der Start bleibt `launch_app`: Desktop-ID mit `DesktopAppInfo::new`
frisch laden, Launch-Kontext des GDK-Displays, kein Shell-Aufruf. Jedes
Menü liest den Katalog bei einer Änderung selbst neu; GIO hält die Daten
im Zwischenspeicher.

**Logzeilen** (nennen jetzt das Menü):

- `Anwendungsmenü mit N Anwendungen, M Einstellungen im Systemmenü, K ausgeschlossen`
- `Systemmenü: Einstellungen 1, Systemverwaltung 1` bzw. `Systemmenü: Keine Einstellungen`
- `Anwendungen geändert, Anwendungsmenü|Systemmenü wird neu aufgebaut`
- `Anwendungen geändert, Anwendungsmenü|Systemmenü ist offen, Aktualisierung vorgemerkt`

Die Monitor-Werkzeuge aus M1.1b-1c zählen deshalb nur noch die Zeilen des
Anwendungsmenüs; die Berichte M1.1b-1a, -1b und -1c verweisen auf den
neuen Wortlaut.

## Automatische Tests

39 Tests (33 bisherige, 6 neue). Prüfkette auf der VM: `cargo fmt --all
--check`, `cargo build`, `cargo test` (39 bestanden), `cargo clippy
--all-targets -- -D warnings` ohne Befund.

| Test | Prüft |
|---|---|
| `system_menu::settings_go_to_preferences_or_administration` | Regel, Kategoriereihenfolge, Großschreibung, Einträge ohne `Settings` |
| `system_menu::sections_become_submenus_sorted_by_name` | Untermenüs, Sortierung über Gruppen hinweg, gleicher Name nach ID, Aktion `system.launch` |
| `system_menu::empty_sections_are_left_out` | nur ein Abschnitt vorhanden |
| `system_menu::no_settings_give_a_placeholder` | „Keine Einstellungen“ ohne Aktion und ohne Untermenü |
| `system_menu::refill_replaces_the_contents_of_the_same_menu` | Neuaufbau desselben `gio::Menu`, `items-changed` |
| `apps::settings_are_split_for_the_system_menu` | Fixtures im Kindprozess mit en_US: Einstellung, Ölfarben, Papierformat / Systemeinstellung |

Erweitert: `fixtures_in_classic2_session` (zwei neue Fixtures,
`c2-einstellung-oel`, `c2-einstellung-papier`) und der Monitor-Kindprozess
(Schritt `einstellung`: eine neue Einstellung kommt mit ihrem Abschnitt im
Katalog an). Die Testhelfer des Anwendungsmenüs (`entry`, `structure`,
`group`, `label`) sind für `system_menu` mitbenutzt.

## GUI-Abnahme

labwc 0.9.8 auf tty2, Bedienung über das gehärtete QMP-Werkzeug,
Tastaturfokus über Markerbefehle im foot-Fenster `c2-fokus`. Testumgebung
`/tmp/c2menu` aus `menu-setup.sh` (M1.1b-1b, bringt „Einstellung“ mit)
und der Zusatzeintrag „Verwaltung“ (`Settings;System`). Treiber:
`werkzeug/system-matrix.sh`. Der Popover-Prüfer `probe.py` aus M1.1b-0
erkennt nur das Menü unter „Anwendungen“; `werkzeug/sysprobe.py` prüft
den Bereich unter „System“.

| Szenario | Läufe | Ergebnis |
|---|---|---|
| Maus: Einstellungen → Einstellung | 3 | Marker `einstellung` |
| Maus: Systemverwaltung → Verwaltung | 1 | Marker `verwaltung` |
| Tastatur: Einstellungen / Systemverwaltung | 3 / 3 | richtiger Marker |
| Escape, Escape im Untermenü, Klick außerhalb | 3 / 1 / 1 | erst offen, danach zu |
| System offen, neue Einstellung (`sysoffen`) | 3 | Anwendungsmenü sofort neu gebaut, System-Menü vorgemerkt (`offen-vorgemerkt.png`), alter Eintrag startet, nach dem Schließen ein Neuaufbau, neuer Eintrag startet |
| Anwendungen offen, neue Verwaltung (`appoffen`) | 3 | umgekehrt: System-Menü sofort, Anwendungsmenü nach dem Schließen |
| Leerer Katalog, Eintrag hinzu und wieder weg | 1 | „Keine Einstellungen“, Enter schließt ohne Start, Start nach Hinzufügen, wieder Platzhalter |
| Anwendungen offen, Klick auf System (`wechsel`) | 1 | Start aus System gelingt (Grenze unten) |
| Regression `menu-matrix.sh`: maus, tastatur, rechts, escape, escsub, sonstiges | 6 | unverändert |
| Regression `monitor-matrix.sh`: hinzu, offen, offenesc, leer | 4 | unverändert |
| Echte VM-Umgebung | 1 | „Keine Einstellungen“ (`keine-einstellungen-vm.png`) |

In jedem Lauf landete die Eingabe danach wieder bei foot, das Panel lief
weiter, keine Zombies. Rohzeilen: `assets/m1-5b/abnahme.csv`, gefilterte
Logs: `panel-log.txt`, Screenshots: `system-menue.png`,
`untermenue-einstellungen.png`, `untermenue-systemverwaltung.png`.

Sieben erste Läufe (escape, escsub, aussen, wechsel, sysoffen, appoffen,
leer) maßen den Popover noch mit `probe.py` und damit an der falschen
Stelle. Die Funktionsspalten waren bestanden; die Läufe wurden mit
`sysprobe.py` wiederholt und nur die Wiederholungen stehen in der Tabelle.

## Bekannte Grenzen

- **Kein Wechsel zwischen offenen Menüs:** Ein Klick auf „System“ bei
  offenem Anwendungsmenü schließt nur dieses (GTK-Popover); ein zweiter
  Klick öffnet „System“. Das gilt ebenso für „Orte“. GNOME 2 wechselte
  beim Überfahren der Menüleiste.
- **Kein echter Programmstart geprüft:** Auf der VM ist keine
  Einstellungsanwendung installiert. Kandidaten aus M1.5a: `nwg-look`
  (`nwg-look.desktop`, `Settings;DesktopSettings`) und `wdisplays`
  (`network.cycles.wdisplays.desktop`); Installation nur nach Freigabe.
- **Systemverwaltung braucht oft Root:** In der Sitzung läuft kein
  Polkit-Agent. Solche Programme können starten und dann scheitern; `Ok`
  von GIO belegt keinen Programmstart. Gehört zur Sitzungsintegration.
- **Zuordnung nur über Kategorien:** Ein Verwaltungswerkzeug ohne
  `System` (etwa nur `Settings;HardwareSettings`) landet unter
  Einstellungen. Bewusst keine Ausnahmelisten.
