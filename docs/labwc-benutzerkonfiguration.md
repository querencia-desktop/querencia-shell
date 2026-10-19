# labwc-Konfiguration als Benutzerdatei (QD-046 ②, G2-AG)

Stand 20.10.2026. Entscheidung des Besitzers (19.10.): Querencia Einstellungen
schreiben die labwc-Konfiguration in eine Benutzerdatei unter
`~/.config/querencia/labwc/`, mit Vorgaben aus diesem Repo, mit Sicherung und
`labwc --reconfigure`. Dieses Dokument ist der Vertrag zwischen dem
Sitzungsstarter (`session/querencia-session`) und Querencia Einstellungen
(ab 0.0.7, Modul `labwc_konfig.rs`).

## 1. Orte

| Ort | Wer schreibt | Inhalt |
|---|---|---|
| `session/querencia/labwc/{rc.xml,autostart,environment[,menu.xml]}` | Entwickler (Git) | **Vorgaben**. labwc liest sie nicht mehr direkt (außer alter Weg, §4). |
| `~/.config/querencia/labwc/` (`$XDG_CONFIG_HOME/querencia/labwc`) | Starter, Einstellungen, Benutzer | **Benutzerkopie**; labwc läuft mit `-C` auf diesem Ordner. |
| `…/labwc/.vorgabe/<datei>` | Starter (Einstellungen nur beim Anlegen) | die Vorgabe, aus der `<datei>` zuletzt übernommen wurde (volle Kopie) |
| `…/labwc/.vorgabe/stand` | Starter | Zeit, Quelle (Commit), SHA256 je Datei in `.vorgabe/` – zum Lesen für Menschen |
| `…/labwc/<datei>.vorgabe-neu` | Starter | neuere Vorgabe, die wegen eigener Änderungen **nicht** übernommen wurde |
| `…/labwc/<datei>.vorher-<JJJJMMTT-HHMMSS>` | Starter, Einstellungen | Sicherung vor jedem Ersetzen |
| `…/labwc/environment.d/zz-querencia-einstellungen.env` | Einstellungen | eigene Umgebungswerte (Tastaturbelegung, Zeiger); labwc liest sie **nach** `environment`, sie haben Vorrang |
| `~/.config/querencia/labwc-alter-weg` | Benutzer | erzwingt den alten Weg (§4) |
| `~/.local/state/querencia/labwc-konfig.log` | Starter | was der Abgleich beim letzten Start getan hat, und welcher Ordner galt |

## 2. Abgleich (vor jedem Start, `konfig_abgleichen`)

Je Datei aus `rc.xml autostart environment menu.xml`, soweit die Vorgabe sie hat:

| Lage | Aktion |
|---|---|
| Benutzerdatei fehlt | Vorgabe kopieren, in `.vorgabe/` notieren |
| `.vorgabe/<datei>` fehlt (Datei des Benutzers oder von Einstellungen vor dem ersten Start) | Benutzerdatei **behalten**, aktuelle Vorgabe notieren |
| Vorgabe = `.vorgabe/<datei>` | nichts |
| Benutzerdatei = neue Vorgabe | nur notieren, `.vorgabe-neu` entfernen |
| Benutzerdatei = `.vorgabe/<datei>` (unverändert) | neue Vorgabe übernehmen, alte Datei als `.vorher-<Zeit>` |
| beide geändert | Benutzerdatei **behalten**, neue Vorgabe als `<datei>.vorgabe-neu`; `.vorgabe/` bleibt alt |

Begründung: Eine Benutzeränderung wird nie überschrieben. Vorgabenänderungen
(neue Tastenkürzel, neue Autostart-Programme) kommen trotzdem an, solange der
Benutzer die Datei nicht angefasst hat. In `.vorgabe/` liegt die volle alte
Vorgabe statt nur einer Prüfsumme, weil Einstellungen damit im Fall „beide
geändert“ seine eigenen Werte auf die neue `rc.xml` übertragen kann
(Dreiwegevergleich: alte Vorgabe, Benutzerdatei, neue Vorgabe; Knopf „Neue
Vorgabe übernehmen“). Alle Schreibvorgänge gehen über eine temporäre Datei
im selben Ordner und `rename`.

Die Umgebung schreibt Einstellungen bewusst **nicht** in `environment`,
sondern in `environment.d/zz-querencia-einstellungen.env`: So bleibt
`environment` unverändert, und Vorgabenänderungen daran (z. B. `LANG`,
QD-056) kommen weiter an. labwc liest im `-C`-Ordner erst `environment`, dann
`environment.d/*.env` in alphabetischer Reihenfolge (`scandir` mit
`alphasort`, also `strcoll`), jeweils mit Überschreiben; die letzte Angabe
gilt (labwc 0.9.8 `src/config/session.c`; Probe S2). Deshalb heißt die Datei
der Einstellungen `zz-…`: Sie kommt nach den Dateien anderer Querencia-Teile
(z. B. `querencia-erscheinungsbild.env` von Thema-3), die Wahl des Benutzers
gewinnt. Bis 20.10. 01:30 hieß sie `querencia-einstellungen.env` – die
sortierte vor `querencia-erscheinungsbild.env` und hätte verloren.

Von Hand nach einer Vorgabenänderung, ohne neue Anmeldung:

```bash
~/projects/querencia-runtime/session/querencia-session --labwc-konfig-abgleichen && labwc --reconfigure
```

## 3. Was wann wirkt

`labwc --reconfigure` (SIGHUP) liest `rc.xml`, `environment` und
`environment.d/*.env` neu, setzt Tastaturbelegung (`XKB_DEFAULT_*`),
Wiederholrate/-verzögerung und den eigenen Zeiger (`XCURSOR_*`) sofort.
`autostart` läuft nur bei der Anmeldung. Programme, die schon laufen (Panel
und alles, was daraus gestartet wird), behalten ihre Umgebung bis zur
nächsten Anmeldung.

## 4. Rückfall

- Ohne Skript, ab der nächsten Anmeldung: `touch ~/.config/querencia/labwc-alter-weg`
  (oder `QUERENCIA_LABWC_KONFIG=repo` in der Umgebung des Starters).
- Von selbst: Ist die Benutzerkopie nicht anlegbar oder fehlen danach
  `environment`/`autostart`, startet labwc mit der Laufzeitkopie (Zeile im
  `labwc-konfig.log`). Die Sitzung startet in jedem Fall.
- Starter zurück: `integration/labwc-benutzer/labwc-benutzer-integrieren.sh zurueck`.
- `querencia-session --exit` erkennt labwc mit beiden Ordnern.

## 5. Wer liest rc.xml sonst (Stand 19.10.)

- `querencia-kuerzel` liest das `-C` des laufenden labwc aus `/proc` → folgt von selbst.
- Einstellungen bis 0.0.6 suchen das `-C` des ersten labwc in `/proc` (nur Anzeige).
- Panel, Desktop, Sitzung: kein Zugriff auf rc.xml; Abmelden über `LABWC_PID`.
- `integration/kuerzel/kuerzel-integrieren.sh pruefen` zählt Treffer in der
  Laufzeitkopie (nur Anzeige).
- Integrationsskripte, die `session/querencia/labwc/autostart` ändern
  (z. B. Benachrichtigungen), ändern jetzt die Vorgabe: Sie kommt beim
  nächsten Start in die Benutzerkopie, solange der Benutzer `autostart` nicht
  geändert hat.
