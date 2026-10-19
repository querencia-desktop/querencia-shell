# G2-Y – Statussymbole im Panel eng setzen (QD-045)

Stand 19.10.2026, Zweig `statussymbole`, Tag `panel/statussymbole-0.1`.

## Befund

Der Besitzer meldete am 19.10. (Screenshot), dass Lautstärke- und
Netzwerk-Symbol oben rechts „zu weit auseinander“ stehen. Gemessen am
G2-M-Panel `c3cda503` (1024×768, `End=volume-1;network-1;clock-1`):

| | G2-M (vorher) | G2-Y (nachher) |
|---|---|---|
| Knopfbreite Lautstärke/Netzwerk | 28 px (22 Symbol + 2×2 Innenrand + 2×1 Rahmen) | 24 px (22 Symbol + 2×1 Rahmen) |
| Abstand der Knöpfe | 16 px (`SPACING`) | 2 px (`STATUS_SPACING`) |
| Abstand der Symbolflächen (22 px) | 22 px | 4 px |
| sichtbarer Abstand Lautstärke → Kabelsymbol | 25 px | 7 px |
| Abstand zur Uhr | 16 px | 16 px (unverändert) |

Ursache: `panel.rs` `space_out` gab jedem Applet eines Bereichs 16 px
Abstand, nur zwischen zwei Startern 2 px; dazu je 2 px Innenrand links und
rechts an beiden Knöpfen. Der in QD-045 genannte 6-px-Rand in
`volume_applet.rs` (Zeilen 260–263) gehört zum Inhalt des Regler-Popups,
nicht zum Knopf, und ist unverändert.

## Umsetzung

- `applets.rs`: neues Feld `AppletType::status`. Gesetzt für `volume` und
  `network`; ein künftiges Status-Applet setzt es ebenso.
- `panel.rs`: `applet_widget` gibt Status-Applets die CSS-Klasse
  `querencia-status` (`STATUS_CLASS`), bevor das Widget in eine Zeile kommt.
  `space_out` ordnet jedes Applet einer Art zu (`Kind`: Starter, Status,
  sonstiges) und rechnet die Ränder der ganzen Zeile neu (`margins`):
  - Starter–Starter 2 px (HMP-11, unverändert),
  - Status–Status 2 px (`STATUS_SPACING`),
  - alles andere 16 px (`SPACING`), also auch Status–Uhr, Status–Starter.
- `space_out` läuft nach jedem Einfügen, Verschieben und Entfernen
  (`place`, `take`); „Zum Panel hinzufügen“, „Nach links/rechts“, „An den
  Rand“, „Verschieben“ und Ziehen mit der mittleren Taste setzen die
  Abstände damit ohne eigenen Code richtig.
- `main.rs`: eine Regel für alle Status-Applets
  (`.querencia-panel button.querencia-status`: kein Innenrand, 1 px
  Hover-Rahmen, neutrale Hervorhebung, gedämpft ohne Dienst) statt je einer
  für Lautstärke und Netzwerk. Gleicher Innenrand und gleiche Symbolgröße
  (beide `logo::size_for_panel`, 22 px bei 32 px Panelhöhe).
- `panel.ini` bleibt im Format gleich, es gibt keine neuen Schlüssel.

## Prüfung

- Unit-Tests: `panel::tests` (`gap`, `margins` für Ausgang, Entfernen,
  Hinzufügen, Verschieben, drittes Status-Applet, Starter neben Status),
  `applets::tests::only_volume_and_network_are_status_icons`,
  `tests::status_applets_share_one_compact_look` (CSS). Panel 209 + 1
  ignoriert.
- Isolierte GUI (headless labwc 0.9.8, eigener D-Bus, eigenes Profil):
  `tools/statussymbole-pruefung/` (siehe unten), 1024×768 und 1920×1080,
  vorher mit dem G2-M-Panel, nachher mit dem neuen. Messung über Pixel
  (Hover-Rahmen und Inhalt), nicht über Augenschein.
- Regression G2-M (rueckfall, attrappe, spaet, verschieben) und G2-J
  (haupt, ohne-dienst, darin W2) über umgelenkte Kopien der Werkzeuge.

Ergebnisse (Release-Bau `97461a61eed7ab6b`, Auswertungen in
`nachweis/ergebnisse/g2y/`):

| Lauf | 1024×768 | 1920×1080 |
|---|---|---|
| G2-Y vorher (G2-M `c3cda503`) | 8/8 | 8/8 |
| G2-Y statussymbole | 45/45 | 45/45 |
| G2-M rueckfall | 22/22 | 22/22 |
| G2-M attrappe | 69/74 (Zweitauswertung 74/74) | 69/74 (Zweitauswertung 74/74) |
| G2-M spaet | 18/18 | 18/18 |
| G2-M verschieben | 30/30 | 30/30 |
| G2-J haupt | 56/56 | 56/56 |
| G2-J ohne-dienst | 17/17 | 17/17 |

W2: Umschalter 56/56, Logo 21/21, Branding 34/35 (dieselbe Zeile wie beim
Ausgang G2-M, bekannt), Starter 19/19, Menü 3/3, Fensterliste 10/10.
Integrationsprobe 28/28.

G2-M attrappe: Die fünf Abweichungen sind Bilder, in denen der
Netzwerk-Knopf hervorgehoben ist (nach dem Menü). Der G2-M-Auswerter misst
den Hintergrund bei `NX − 13`; das lag im 28-px-Knopf im Rahmen, im
24-px-Knopf daneben. Das Symbol wird in allen fünf richtig erkannt, nur die
Abweichung steigt auf 8–10 (Schwelle 8). `auswerten-g2y.py` (eine Zeile:
`NX − 9`, von `werkzeug-anlegen.sh` erzeugt) wertet dieselben Bilder mit
74/74 aus; alle anderen Zeilen sind gleich.

Werkzeuge:

```bash
bash tools/statussymbole-pruefung/werkzeug-anlegen.sh        # Kopien der G2-M/G2-J-Werkzeuge
bash tools/statussymbole-pruefung/laeufe.sh <panel-neu>      # G2-Y vorher/nachher, beide Auflösungen
bash tools/statussymbole-pruefung/regression.sh <panel-neu>  # G2-M + G2-J + W2 (~1 h)
bash tools/statussymbole-pruefung/integration-probe.sh <scratch-ordner>
```

Die G2-J-Läufe liegen in `nachweis/gui/gj/` statt (wie bei G2-M)
`nachweis/gui/g2jreg/gui/`: Mit dem längeren Worktree-Namen kämen
`…/w2reg/rt/w2-xxxxxxxx/wayland-0` und `…/rt/g2j-xxxxxxxx/pulse/native`
sonst über die 107 Zeichen eines Unix-Socket-Pfads.

## Integration

`integration/statussymbole-integrieren.sh pruefen|aktivieren|panel-neustart|zurueck|panel-neustart-zurueck`,
Muster `netzwerk-integrieren.sh`. Ausgang und Rückfall: G2-M
`c3cda503421d7e9f`. Vorlage `integration/vorschlag-statussymbole/`
(Release-Bau, Binary nicht im Git, `SHA256SUMS` im Git).

## Grenzen

- Das Panel ist der erste Release-Bau, der live gehen soll (bisher
  Debug-Workspace-Builds). Release-Builds sind ebenfalls pfadgebunden
  (Panic-Orte), Neubau nur im selben Ordner bit-gleich.
- Mehr als zwei Status-Applets gibt es noch nicht; der Abstand ist für
  beliebig viele festgelegt und im Unit-Test geprüft.
- Die Lage der Symbole ändert sich gegenüber G2-M, wenn beide nebeneinander
  stehen: Die Mitte der Lautstärke rückt um 20 px nach rechts, die des
  Netzwerks um 2 px. Fremde Prüfskripte mit festen Klickkoordinaten auf die
  Lautstärke neben dem Netzwerk müssen das berücksichtigen (die G2-M- und
  G2-J-Szenarien treffen weiterhin, siehe Bericht).

## Live (19.10.2026 13:01)

Aktiviert von der Status-Session: Panel `97461a61eed7ab6b`, `.vorher` G2-M
`c3cda503421d7e9f`. Rückfall in dieser Reihenfolge:
`statussymbole-integrieren.sh zurueck`, `panel-neustart-zurueck`; erst
danach `netzwerk-integrieren.sh` (verweigert sonst mit `P=fremd`).

Der erste Tausch (11:33) scheiterte an `querencia-panel-werkstatt/live/tausch.py`:
Es startete das neue Panel, sobald der Hauptthread des alten Zombie war;
der Busname `org.example.Querencia.Panel` blieb aber noch bis 66 ms
vergeben, das neue Panel wurde Zweitinstanz („Failed to register …
NoReply“). Nachstellung: `tools/statussymbole-pruefung/wettlauf/`
(ohne Warten 0/40, mit Patch 40/40). Seit 13:01 wartet tausch.py dauerhaft
auf `NameHasOwner = false` und liest Logs nach Bytes
(`nachweis/g2y/tausch-namenswarten.patch`).
