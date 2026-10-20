# G2-AK – Panel: Programmsuche, Bildschirm sperren, Fensterliste je Arbeitsfläche, panel.ini neu laden, Vollbild-Kürzel

Stand 20.10.2026. Zweig `g2ak` (ab `64f587a`, G2-AD v1), für die Aktivierung
gebaut aus `g2ak-g2ai` (= `g2ai` + `g2ak`). Punkte QD-069 (Pflichtprobe B10,
B11), QD-046 ⑥ (Panel-Seite) und QD-065.

## 1. Programmsuche (QD-069, B10)

`sitzung_start.rs` sucht jedes Programm, das das Panel selbst startet
(`querencia-sitzung`, der Lautstärkeregler `querencia-lautstaerke`), in
dieser Reihenfolge:

1. Umgebungsvariable aus dem Programmnamen (`querencia-sitzung` →
   `QUERENCIA_SITZUNG`, `querencia-lautstaerke` → `QUERENCIA_LAUTSTAERKE`),
   ein absoluter Pfad. Ist sie gesetzt (und nicht leer), zählt nur sie; ein
   unbrauchbarer Wert heißt „kein Programm“ mit Grund im Log, nie ein
   anderes Programm an seiner Stelle.
2. absolute Einträge von `PATH`, der Reihe nach;
3. `~/opt/querencia/bin` (Entwicklungsweg der Integrationsskripte);
4. `/usr/bin` (Paket), auch wenn `PATH` es nicht nennt.

Gefunden wird nur eine ausführbare reguläre Datei (wie `which`). Ohne
Sitzungsprogramm bleibt die alte Zwei-Knopf-Rückfrage des Panels für
„Abmelden …“ (labwc `--exit`); „Bildschirm sperren“ und „Ausschalten …“
fehlen dann. Logzeile beim Menüaufbau: `Sperren über <pfad> --sperren
--sofort` bzw. `kein Sperren (<grund>)`.

Befund zur Pflichtprobe: Der Paketsatz (G2-F, `quellen.conf`) baute das Panel
aus `52f177c`, also vor G2-AD; dort gab es noch keine Sitzungssuche. Schon
die G2-AD-Suche über `PATH` hätte `/usr/bin` gefunden.

## 2. Bildschirm sperren

System-Menü in GNOME-2-Reihenfolge: „Bildschirm sperren“, „Abmelden …“,
„Ausschalten …“. Sperren startet `querencia-sitzung --sperren --sofort`,
denselben Befehl wie Strg+Alt+L in der labwc-Vorgabe (ein Unit-Test liest
`rc.xml` mit). Das Menü zeigt das Kürzel rechts als „Strg+Alt+L“: Das
klassische Menü (`classic_menu.rs`) liest das GTK-Attribut `accel` eines
Eintrags und schreibt es deutsch (`accel_label`, Strg/Alt/Umschalt/Super,
Entf/Esc/Druck), nur zur Anzeige; die Taste bindet labwc. Farben
`.qmenu-accel` nur im Panel (`#7a7671`, weiß in der Hervorhebung).

## 3. Fensterliste nur der aktuellen Arbeitsfläche (B11)

`panel.ini`:

```ini
[window-list]
alle_arbeitsflaechen=nein
```

`nein` (Standard, auch ohne Gruppe: bestehende Nutzer bekommen den neuen
Standard) zeigt wie GNOME 2 nur Fenster der aktuellen Fläche, `ja` alle.
`true`/`false` werden auch gelesen; anderes ist ein Problem (Log) und lässt
den Standard. Das Panel schreibt den Schlüssel beim Speichern nur zurück,
wenn die Datei ihn hatte.

labwc 0.9.8 meldet keine Arbeitsfläche je Fenster. Die Fensterliste bindet
`ext_workspace_manager_v1` deshalb auf ihrer eigenen Verbindung (gleiche
Reihenfolge wie die Fensterereignisse) und ordnet zu (`fensterliste.rs`):
neues Fenster → aktuelle Fläche; aktiviertes Fenster → aktuelle Fläche;
behält das aktive Fenster beim Flächenwechsel den Fokus, ist es mitgegangen
(Strg+Alt+Umschalt+←/→), verliert es ihn innerhalb von 300 ms, blieb es
zurück. Grenzen: Fenster, die schon vor dem Panelstart offen waren, stehen
auf allen Flächen, bis sie einmal aktiv waren; `SendToDesktop follow="no"`
(in Querencia nicht belegt) bleibt bis zur nächsten Aktivierung an der alten
Stelle; Fenster auf allen Flächen erscheinen dort, wo sie zuletzt aktiv
waren. Ohne das Protokoll werden alle Fenster gezeigt. Ein Klick auf Fenster
anderer Flächen entfällt damit; Alt+Tab bleibt. Logzeilen
`Fensterliste: Arbeitsfläche „…“` und `Fensterliste: n von m Fenstern
sichtbar (<schlüssel> <app-id>, …)` (nie Fenstertitel).

## 4. panel.ini neu laden (QD-046 ⑥, Schnittstelle für Querencia Einstellungen)

- **Datei:** `$XDG_CONFIG_HOME/querencia/panel.ini`, Format wie bisher
  (`layout_store.rs`) plus `[window-list]`. Atomar schreiben (neue Datei
  daneben, umbenennen), wie GLib `g_file_set_contents`.
- **Dateibeobachtung:** GFileMonitor auf die Datei; Änderungen werden 250 ms
  gesammelt, dann einmal gelesen. Ein Aufruf ist nicht nötig.
- **D-Bus:** auf dem Busnamen des Panels `org.example.Querencia.Panel`,
  Objekt `/org/example/Querencia/Panel`, Schnittstelle `org.querencia.Panel`,
  Methode `NeuLaden() → (b geladen, s meldung)`; liest sofort.
  ```bash
  gdbus call --session --dest org.example.Querencia.Panel \
    --object-path /org/example/Querencia/Panel --method org.querencia.Panel.NeuLaden
  ```
  `(true, 'panel.ini unverändert')`, `(true, 'Layout aus panel.ini, 2 Panel(s); neu aufgebaut: bottom; …')`,
  `(false, 'panel.ini fehlerhaft, die Panels bleiben wie sie sind: …')`.
- **Verhalten:** gleich einem Neustart (`Editor::load`): keine Datei →
  Standardlayout; einzelne kaputte Einträge → verworfen, Log. Unbrauchbare
  Datei (nicht lesbar, keine Schlüsseldatei, kein Panel) → alte Konfiguration
  bleibt, `geladen=false`. Inhalt gleich dem zuletzt gelesenen oder vom Panel
  selbst geschriebenen → nichts geschieht. Unveränderte Panels bleiben
  stehen (offene Menüs bleiben offen), geänderte, neue und entfernte werden
  neu gebaut bzw. entfernt; eine geänderte Fensterlisten-Einstellung baut
  die Panels mit Fensterliste neu. Eine Logzeile je Neuladen
  (`Neuladen (Dateiänderung|D-Bus <absender>): …`).
- Nach dem Neuladen speichert „Zum Panel hinzufügen“ wieder normal (vorher:
  „seit dem Laden verändert, Panel neu starten“).

## 5. Vollbild-Kürzel (QD-065)

Vorgabe `session/querencia/labwc/rc.xml`: `ToggleFullscreen` auf **Super+F**
und **Alt+F11**, nicht F11: labwc-Kürzel gehen jedem Programm vor, F11
gehörte sonst nicht mehr den Programmen, die es selbst nutzen (Querencia
Bilder, Firefox, entfernte Sitzungen). Die Kürzel-Übersicht
(`querencia-kuerzel`) zeigt beide automatisch unter „Fenster“ („Vollbild ein
oder aus“), weil sie die rc.xml des laufenden labwc liest. Wirkt nach dem
Merge nach `main` beim nächsten Vorgaben-Abgleich (Anmeldung).

Befund Xwayland (isoliert, X11-Testfenster `tools/g2ak-pruefung/qx11fenster.c`):
labwc 0.9.8 blendet die Top-Ebene (Panels) aus, wenn ein Vollbild-Fenster
oben liegt – auch für X11 mit `_NET_WM_STATE_FULLSCREEN`. Über den Panels
bleibt ein Fenster nur ohne diesen Zustand: bildschirmgroß an 0,0 per
ConfigureRequest (wie Horizon „FullScreen“) oder `override_redirect`
(unverwaltete X11-Fenster liegen in labwc unter der Top-Ebene). Zudem
setzte labwc in 3 von 20 Fällen den vor dem Abbilden gesetzten Vollbild-
Zustand nicht um (zeitabhängig). Abhilfe: Super+F oder Alt+F11 (nicht für
unverwaltete Fenster), Alt+Leertaste → Vollbild, im Programm „Fenster“
statt „Vollbild“. Keine labwc-Änderung.

## 6. Prüfung

`tools/g2ak-pruefung/` (isolierter headless labwc mit Xwayland, Attrappe
statt `querencia-sitzung`, `laeufe-g2ak.sh`, `gesamt.sh`), Unit-Tests in
`sitzung_start.rs`, `fensterliste.rs`, `neuladen.rs`, `layout_store.rs`,
`window_list.rs`, `system_menu.rs`, `classic_menu.rs`. Ergebnisse:
.
