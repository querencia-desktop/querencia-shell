# Panel-Überwachung einschalten

Stand 22.10.2026. Zweig `panel-ueberwacht` (auf `panel/0.2.3-int`), Code
`7ad985a`, Tag `panel/ueberwacht-1`, Integrationsstand `panel/ueberwacht-1-int`.
Die Überwachung selbst: `docs/panel-023-absturz-und-ueberwachung.md` §3.

## Was sich ändert

Die labwc-Vorgabe `session/querencia/labwc/autostart` startet das Panel mit
`--ueberwacht`, wenn `querencia-panel --faehigkeiten` „ueberwacht“ nennt
(ab Panel 0.2.3); ein älteres Panel startet wie bisher. Die Record-Datei
nennt dann die Überwachung. Sonst ist die Datei unverändert.

| Fassung | SHA256 (Anfang) |
|---|---|
| heute (main, live) | `8e1b3b45` |
| mit Überwachung | `199d1b43` |
| zurückgenommen (heute + Kommentarzeile) | `3c462a9e` |

## Wie es live ankommt

Der Sitzungsstarter liest die Vorgabe aus dem Runtime-Checkout
(`~/projects/querencia-runtime`) und gleicht vor jeder Anmeldung die
Benutzerkopie `~/.config/querencia/labwc/autostart` ab
(`docs/labwc-benutzerkonfiguration.md` §2). Daraus folgt, belegt mit einer
Kopie des Starters in der Probe (S1–S5):

| Lage | nächste Anmeldung |
|---|---|
| `aktivieren` (Benutzerkopie mit Überwachung), Vorgabe noch alt | Benutzerkopie bleibt (S1) |
| danach Merge nach main (Vorgabe neu) | nur notiert, `.vorgabe` neu (S2) |
| `zurueck` nach dem Merge | Rücknahme bleibt (S3) |
| `zurueck` vor einer Anmeldung, dann Merge | Rücknahme bleibt, neue Vorgabe nur als `autostart.vorgabe-neu` (S4) |
| **nicht aktiviert, nur Merge nach main** | **die Anmeldung übernimmt die Überwachung von selbst** (S5) |

Der Merge von `panel-ueberwacht` nach main ist deshalb selbst eine
Aktivierung (ab der nächsten Anmeldung) und gehört hinter die Freigabe.
Die Rücknahme schreibt die alte Fassung mit einer Kommentarzeile, damit die
Benutzerkopie als „eigene Datei“ gilt und eine gemergte Vorgabe sie nicht
ersetzt.

## Tausch mit laufender Überwachung (`tausch-panel-2.py`)

Ein künftiger Panel-Tausch ersetzt Überwachung und Panel gemeinsam:

1. Erkennen: genau eine Überwachung (Befehlszeile mit `--ueberwacht`) mit
   höchstens einem Panel als Kind, oder genau ein Panel ohne Überwachung;
   alles andere wird verweigert.
2. SIGTERM an den obersten Prozess. Die Überwachung merkt sich „beendet“,
   gibt SIGTERM an ihr Panel weiter und startet es danach **nicht** neu (das
   Panel endet durch SIGTERM, kein Absturz; nach 5 s SIGKILL). Gewartet wird,
   bis beide weg sind und der Busname frei ist. Das alte Binary kann so nicht
   wiederkommen.
3. Start über den Link mit `--ueberwacht` (oder ohne), eigener systemd-Scope,
   Record-Datei auf den neuen obersten Prozess.
4. Gesund: Aufbau im Log, 4 s später leben Überwachung und Panel, kein
   Absturz; sonst Rückfall auf das alte Binary in der alten Art.

Die Überwachung startet nach einem Absturz dasselbe Binary, aus dem sie
selbst läuft (`/proc/self/exe`), nicht das hinter dem Link: Nach `aktivieren`
einer neuen Panel-Fassung ohne Tausch bleibt es bis zur Anmeldung bei der
laufenden Fassung.

## Prüfung

Arbeitsordner `~/projects/querencia-panel-ueberwacht` (Kopie:
`integration/panel-ueberwacht/`), isoliert mit den Live-Ablagen von 0.2.3
und 0.2.2:

- `autostart` (Panel-Block der Vorgabe wie vom Starter ausgeführt): neue
  Vorgabe + 0.2.3 → Überwachung, Record-Datei, Neustart nach Absturz,
  Abmelden per SIGTERM beendet beide in ~0,1 s; neue Vorgabe + 0.2.2 → wie
  bisher; alte Vorgabe + 0.2.3 → wie bisher (17/17).
- `tausch`: ohne → mit, Absturz danach, mit → mit (neue Fassung, altes Panel
  nicht neu gestartet), mit → ohne, kaputtes neues Panel → Rückfall (31/31).
- Probe des Skripts samt Starter-Abgleich S1–S5: 40/40.
