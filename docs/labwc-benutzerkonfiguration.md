# labwc-Konfiguration als Benutzerdatei (QD-046 ②, G2-AG)

Stand 21.10.2026 (§1, §6, §7 gleichgezogen mit Einstellungen 0.1.2, Fassung in deren `docs/`). Entscheidung des Besitzers (19.10.): Querencia Einstellungen
schreiben die labwc-Konfiguration in eine Benutzerdatei unter
`~/.config/querencia/labwc/`, mit Vorgaben aus diesem Repo, mit Sicherung und
`labwc --reconfigure`. Dieses Dokument ist der Vertrag zwischen dem
Sitzungsstarter (`session/querencia-session`) und Querencia Einstellungen
(ab 0.0.7, Modul `labwc_konfig.rs`).

## 1. Orte

| Ort | Wer schreibt | Inhalt |
|---|---|---|
| `session/querencia/labwc/{rc.xml,autostart,environment[,menu.xml]}` im Runtime-Checkout, im Paket `/etc/querencia/labwc/` | Entwickler (Git) bzw. Paket | **Vorgaben**. labwc liest sie nicht mehr direkt (außer alter Weg, §4). Wie Einstellungen sie finden: §7. |
| `~/.config/querencia/labwc/` (`$XDG_CONFIG_HOME/querencia/labwc`) | Starter, Einstellungen, Benutzer | **Benutzerkopie**; labwc läuft mit `-C` auf diesem Ordner. |
| `…/labwc/.vorgabe/<datei>` | Starter (Einstellungen nur beim Anlegen) | die Vorgabe, aus der `<datei>` zuletzt übernommen wurde (volle Kopie) |
| `…/labwc/.vorgabe/stand` | Starter | Zeit, Quelle (Commit), SHA256 je Datei in `.vorgabe/`; Einstellungen ab 0.1.2 lesen daraus die Zeile `quelle <ordner> (commit …)` (§7) |
| `…/labwc/<datei>.vorgabe-neu` | Starter | neuere Vorgabe, die wegen eigener Änderungen **nicht** übernommen wurde |
| `…/labwc/<datei>.vorher-<JJJJMMTT-HHMMSS>` | Starter (Einstellungen bis 0.1.0) | Sicherung vor jedem Ersetzen; Einstellungen ab 0.1.1 löschen sie nie |
| `…/labwc/<datei>.einstellungen-vorher-<JJJJMMTT-HHMMSS>` | Einstellungen ab 0.1.1 | Sicherung vor dem ersten Schreiben je Programmlauf, höchstens fünf je Datei (§6) |
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

## 6. Rückstellung und Sicherungen der Einstellungen (ab 0.1.1, QD-080)

Anlass: Werkstatt QREL-06 A17.6 (20.10.) – Ändern und Zurückstellen ließ
ausdrückliche Standardwerte in `rc.xml` stehen (dann ist die Datei für den
Abgleich „geändert“, neue Vorgaben kommen nur noch als `.vorgabe-neu`), und
jedes „Übernehmen“ legte eine `rc.xml.vorher-<Zeit>` an; 0.1.0 räumte dabei
auch Sicherungen des Starters weg.

- **Rückstellung byte-gleich.** Ein Wert, den „Übernehmen“ ändert und der
  danach dem Wert der Basis gleicht, wird so geschrieben, wie die Basis ihn
  hat: deren Text, oder gar nicht, wenn die Basis ihn nicht angibt (dann gilt
  labwcs eingebauter Wert, wie ihn die Bereiche anzeigen). Basis ist
  `.vorgabe/rc.xml`, sonst die aktuelle Vorgabe – genau die Datei, mit der der
  Starter vergleicht. Behälter, die Einstellungen dafür angelegt hatten
  (`<libinput><device category="default">`, `<font place="…">`), gehen mit,
  wenn sie leer werden und die Basis sie nicht hat. Bleibt danach nur
  Leerraum anders als in der Basis, wird die Basis byte-gleich geschrieben.
  Eigene Werte, fremde Blöcke und Kommentare bleiben unberührt.
- **Umgebung.** `environment.d/zz-querencia-einstellungen.env` verschwindet
  wieder, wenn nur noch ihr eigener Kopf übrig ist; Zeilen, die nicht von
  Einstellungen stammen, halten sie.
- **Sicherungen.** Eigenes Muster `<datei>.einstellungen-vorher-<Zeit>`
  (bei gleicher Sekunde `-2`, `-3` …). Eine Sicherung je Datei und
  Programmlauf: der Stand vor dem ersten Schreiben; eine weitere nur, wenn
  ein anderer die Datei inzwischen geändert hat. Gleicht eine eigene
  Sicherung danach der Datei, ist sie überflüssig und wird entfernt (Ändern
  und Zurückstellen hinterlässt keine). Höchstens fünf je Datei, die ältesten
  gehen. `<datei>.vorher-*` (Starter und Einstellungen bis 0.1.0, nicht zu
  unterscheiden) löschen Einstellungen nie.

## 7. Wo Einstellungen die Vorgabe finden (ab 0.1.2, G1a)

Bis 0.1.1 stand der Pfad des Runtime-Checkouts der Entwicklungs-VM fest im
Programm. Ab 0.1.2 ermittelt `labwc_konfig::vorgabe_ermitteln` ihn zur
Laufzeit, in dieser Reihenfolge (die erste passende Angabe gilt):

| # | Quelle | Fall |
|---|---|---|
| 1 | `QE_LABWC_VORGABE=<ordner>` | Prüfungen (isolierte Läufe, Tests) |
| 2 | `$QUERENCIA_REPO/session/querencia/labwc` | Entwicklung mit einem Runtime-Checkout; gleicher Name wie im Starter |
| 3 | Zeile `quelle <ordner> (commit …)` in `~/.config/querencia/labwc/.vorgabe/stand`, wenn dort eine `rc.xml` liegt | Sitzung: der Starter hat die Benutzerkopie aus diesem Ordner abgeglichen (Entwicklungs-VM: Runtime-Checkout) |
| 4 | `/etc/querencia/labwc` | Paket |

Leere und relative Angaben zählen nicht. `querencia-einstellungen
--labwc-zeigen` nennt Ordner und Herkunft (`vorgabe=`, `vorgabe-herkunft=`
`Umgebung|Repo|Stand|Paket`). Für den Starter heißt das: Die Zeile `quelle`
in `.vorgabe/stand` bleibt in dieser Form (absoluter Ordner, danach
` (commit …)`); ein Paket-Starter schreibt dort `/etc/querencia/labwc`.

**Was der Starter schreibt** (`session/querencia-session`, `konfig_abgleichen`):
- Nach jedem Abgleich, der etwas übernommen hat, und wenn `stand` noch fehlt,
  schreibt der Starter `.vorgabe/stand` neu, atomar über `stand.neu-<pid>` und
  `mv`:
  - zwei Kommentarzeilen,
  - `zeit <JJJJ-MM-TT HH:MM:SS>`,
  - `quelle <VORGABE_DIR> (commit <kurz>|unbekannt)`,
  - je Datei in `.vorgabe/` eine `sha256sum`-Zeile.
- `VORGABE_DIR` ist `$QUERENCIA_REPO/session/querencia/labwc`; auf der
  Entwicklungs-VM ist das der Runtime-Checkout.
- **Vertrag:** Die `quelle`-Zeile bleibt in dieser Form (ein absoluter Ordner,
  dann ` (commit …)`). Einstellungen ab 0.1.2 finden die Vorgabe darüber (Fall 3).
- Ein Paket-Starter (G2-F) schreibt dort `/etc/querencia/labwc` oder setzt
  `QUERENCIA_REPO`. Ändert sich das Format, müssen Starter und Einstellungen
  gemeinsam geändert werden.

## 8. Software-OpenGL im Autostart (QD-169, 23.10.2026)

Die Autostart-Vorgabe exportiert seit `session/grafik-1` die Variablen aus
`QUERENCIA_GRAFIK_UMGEBUNG` (nur `LIBGL_ALWAYS_SOFTWARE`,
`MESA_LOADER_DRIVER_OVERRIDE`, `GALLIUM_DRIVER`), die der Starter bei zink
auf NVK setzt. Eine eigene Benutzerdatei `autostart` ohne diesen Abschnitt
bleibt beim Abgleich erhalten; der Starter schreibt dann einen Hinweis ins
Protokoll, und Software-OpenGL gilt nur über die Aktivierungsumgebung.
Einzelheiten: [sitzung-grafik.md](sitzung-grafik.md).

