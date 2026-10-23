# Desktop 0.3.0: Dateiarbeit mit Fortschritt, Abbruch und Konfliktfrage

**Stand:** Entwurf vom 23.10.2026 (Worker desktop-030, Welle A von „Querencia
Dateien“, QD-140). Dieser Entwurf entstand vor dem Merge der Crates
`querencia-oberflaeche` und `querencia-dateiarbeit`. Abschnitte mit „(offen)“
werden nach dem Merge mit der echten API und den Messwerten gefüllt.

## 1. Ausgangslage

| Was | Stand |
|---|---|
| Live | Desktop 0.2.6-2, Ablage `a9e3600107c1cd53` (PID 2556, gemessen über `/proc/<pid>/exe`, 23.10.) |
| Code | main `a1b3e53`; Desktop-Crate unverändert seit `desktop/0.2.6-2` |
| Dateioperationen | `fileops.rs` blockierend im Arbeitsthread (`gio::spawn_blocking`), 12 GIO-Aufrufe mit `Cancellable::NONE`, kein Fortschritt, kein Abbruch |
| Konflikte | Kopie auf belegten Namen: still freier Name („(Kopie)“); Verschieben auf belegten Namen: abgelehnt („nie überschreiben“, Test `a_move_never_overwrites…`) |
| Laufwerke | nur eingehängte Mounts als Symbol; Einhängen gibt es nicht |
| Papierkorb | Zähler `trash::item-count` synchron im Hauptthread, Monitor auf `trash:///` |
| Überwachung | `desktop_view.rs` `watch`/`rewatch_soon` mit eigener Entprellung und Warten auf den Arbeitsflächen-Ordner (QD-112) |
| Geteilte GTK-Bausteine | `classic_menu.rs`, `farbschema.rs`, `icon_theme.rs`, `portal.rs` über `#[path]` aus dem Panel |
| GUI-Regression | `gesamt` (48 Fälle) lief zuletzt mit 0.2.3; die 0.2.6-Werkzeuge kannten sie nicht (QD-144) |

## 2. Entscheidungen, die gelten

- **E2:** Konfliktregel „fragen“: Ersetzen, Überspringen, Umbenennen, „Für alle
  übernehmen“; Vorgabeknopf **Überspringen**. Ersetzt wird atomar (erst unter
  Hilfsnamen kopieren, dann umbenennen), wo das Crate es anbietet.
- **E3:** Einhängen und Automount gehören in den Desktop. Ein Doppelklick auf
  ein nicht eingehängtes Laufwerk hängt es ein; Rückfragen (Kennwort,
  Verschlüsselung) über `gtk4::MountOperation`.
- **E8:** Rückgängig kommt erst nach der Beta.
- Fortschritt nach Variante A der Inventur: Bibliothek im Prozess, ein
  eigenes Fenster je Programm; ein gemeinsamer D-Bus-Dienst folgt in Welle C.

## 3. Umbau

Reihenfolge nach dem Merge (oberflaeche → dateiarbeit):

1. **Crates einbinden.** `Cargo.toml` des Desktops: `querencia-oberflaeche`
   und `querencia-dateiarbeit` als `path`-Abhängigkeit (Workspace). Die vier
   `#[path]`-Module und `cfg(querencia_desktop)` in `build.rs` entfallen. Hat
   der oberflaeche-Merge den Desktop schon umgestellt (0.2.7), setzt 0.3.0
   darauf auf und doppelt nichts.
2. **`fileops.rs` ersetzen.** URI-Liste, Zwischenablage-Formate, Kopienamen,
   `tally`, `size_text` und Fehlertexte kommen aus dem Crate (`uri`, `namen`,
   `auftraege`). Im Desktop bleibt nur, was die Arbeitsfläche betrifft
   (Ablegen an der Stelle des Loslassens, `place_arrived`).
3. **Jobs über `auftraege::Queue`.** `transfer.rs` `start_job`/`job_done`
   werden zu: Job an die Queue geben, Ereignisse abonnieren. `Finished` löst
   wie bisher `place_arrived`, `reload`, Papierkorb-Aktualisierung und den
   Fehlerdialog aus. Abgebrochene Jobs zeigen keinen Fehlerdialog.
4. **Fortschrittsfenster** (neues Modul `desktop_view/fortschritt.rs`):
   eigenes Toplevel „Dateioperationen“ wie Nautilus 2.30; je Job eine Zeile
   mit Text („Kopieren von 3 Dateien nach »Schreibtisch«“), Fortschrittsbalken,
   aktuellem Dateinamen, „12,4 MB von 100 MB – 1 Minute verbleibend“ und
   Knopf **Abbrechen**. Erscheint erst nach 1 s Laufzeit (kurze Jobs ohne
   Fenster, wie 2.30), schließt sich, wenn kein Job mehr läuft. QD-076: beim
   Schließen zerstören, beim nächsten Job neu bauen.
5. **Konfliktdialog** (neues Modul `desktop_view/konflikt.rs`): Wortlaut nach
   Nautilus 2.30 de: „Datei »x« ersetzen?“ bzw. „Ordner »x« zusammenführen?“,
   Erklärzeile mit Größe und Änderungsdatum beider Dateien, Knöpfe
   **Abbrechen**, **Überspringen** (Vorgabe, Eingabetaste und Escape-freie
   Wahl), **Umbenennen** (Namensfeld vorbelegt mit freiem Namen), **Ersetzen**
   bzw. **Zusammenführen**; Kontrollkästchen „Diese Aktion auf alle Dateien
   anwenden“. Escape = Abbrechen des ganzen Jobs. Die Antwort geht über den
   `Reply` des `Ask`-Ereignisses an den Job zurück.
6. **Duplizieren bleibt still.** Einfügen in denselben Ordner (Quelle =
   Ziel-Ordner) erzeugt wie bisher „notiz (Kopie).txt“ ohne Frage, wie 2.30.
7. **`volumes.rs` ersetzen.** Einordnung, Bildwahl, Aushängen/Auswerfen,
   „Programme halten Dateien offen“ aus `laufwerke`. Die Desktop-Einstellung
   `shown()` (welche Typen gezeigt werden) und die Platzierung bleiben im
   Desktop.
8. **Einhängen (E3).** Nicht eingehängte Wechsel- und optische Datenträger
   (`can_mount`, Typ nach `laufwerke`) erscheinen als Symbol, eingebaute
   Partitionen nicht (sonst stünden Systempartitionen auf der Fläche).
   Doppelklick oder Menü „Einhängen“ hängt ein und öffnet danach im
   Ordnerprogramm. Automount neu eingesteckter Wechseldatenträger
   (`volume-added` mit `should_automount`), ohne Ordner zu öffnen (E10: „beim
   Einhängen öffnen“ entfällt).
9. **Papierkorb-Zähler** über `papierkorb::Papierkorb` (Signal statt
   synchroner Abfrage im Hauptthread). „Papierkorb leeren“ läuft als Job
   (Fortschritt und Abbruch), Fehlerliste wie bisher.
10. **Überwachung** über `ueberwachung::FolderWatch` mit `warten = true`
    (QD-112); die eigene Entprellung in `watch`/`rewatch_soon` entfällt.
11. **`icons.rs`/`rename.rs`:** `type_from_name`, `split_extension`,
    `unique_name`, `check_file_name`, `selected_chars` aus dem Crate; Reihenfolge,
    Vorlagen, `Kind` und das Ändern von `Name[xx]` im Starter bleiben.
12. **Texte:** neue msgids englisch, `po/de.po` vollständig (Test
    `german_catalog_is_complete`); Texte der Crates in deren Domänen.
13. **Version und Doku:** 0.3.0, CHANGELOG-Abschnitt „Verhaltensänderung“,
    Release-Notes-Satz (§6), Doc-Map.

## 4. Testplan

### 4.1 Unit-Tests

| Bereich | vorher (0.2.6-2) | nachher |
|---|---|---|
| Desktop-Binary gesamt | 120 (93 eigene + 27 geteilte) | (offen) |
| fileops | 8 | ins Crate; im Desktop bleiben die Tests zur Ablage |
| volumes | 11 | ins Crate bis auf `shown()` |
| icons / rename | 9 / 7 | anteilig ins Crate |
| neu | – | Konfliktantwort → Crate-Antwort, „für alle“, Duplizieren ohne Frage, Fortschrittstext (Zahlen, Restzeit), Einhängen-Entscheidung |

Der alte Test „Verschieben überschreibt nie“ wird ersetzt durch: ohne Antwort
(Vorgabe Überspringen) bleibt das Ziel unverändert.

### 4.2 GUI-Regression isoliert (`werkzeug/lauf.sh`)

Ablauf **gesamt** (aus 0.2.3 portiert, 48 PASS bei 0.2.3) plus neue Fälle:

| Nr. | Fall | Erwartung |
|---|---|---|
| N1 | große Kopie (Ordner mit 200 MB in vielen Dateien) Arbeitsfläche → Ordner | Fortschrittsfenster erscheint nach ~1 s, Balken wächst, Dateiname wechselt, Fenster schließt sich am Ende |
| N2 | Abbruch mitten im Baum | Knopf Abbrechen: Job endet, Log „abgebrochen“, kein Fehlerdialog, teilweise kopierte Datei nicht halb liegen gelassen, Quelle unverändert |
| N3 | Konflikt Ersetzen | Ziel hat danach den Inhalt der Quelle |
| N4 | Konflikt Überspringen (Eingabetaste = Vorgabe) | Ziel unverändert, Quelle unverändert |
| N5 | Konflikt Umbenennen | Kopie unter dem eingegebenen Namen, Ziel unverändert |
| N6 | Konflikt „für alle“ + Überspringen bei drei Konflikten | eine Frage, drei übersprungen, Nicht-Konflikte kopiert |
| N7 | Konflikt Abbrechen (Escape) | Job endet, nichts ersetzt |
| N8 | Verschieben mit Konflikt (Strg+X, Strg+V) Ersetzen | Quelle weg, Ziel neu |
| N9 | Ordner zusammenführen | Inhalt beider Ordner im Ziel |
| N10 | Einhängen per Doppelklick (Laufwerks-Attrappe aus `werkzeug/dateien-pruefrahmen`) | Volume eingehängt, Symbol wechselt auf eingehängt |
| N11 | Einhängen mit Rückfrage (Attrappe verlangt Kennwort) | MountOperation-Dialog erscheint; Abbrechen lässt es ausgehängt |
| N12 | Papierkorb leeren als Job | Zähler fällt auf 0, Symbol leer |

Weitere Abläufe wie 0.2.6-2: `files`, `tausch` (0.2.6-2 → 0.3.0 → 0.2.6-2),
`texte` de/en, `lage`, `fotos`; Fotos hell und dunkel je 1920×1080 und
1024×768 (Fortschrittsfenster und Konfliktdialog eingeschlossen).

### 4.3 Isolation der Läufe

Seit 0.3.0 setzt `isoliert.py` `GVFS_MOUNTABLE_DIR` auf eine Kopie der
gvfs-Mountbeschreibungen ohne `network`, `wsdd`, `smb-browse`, `dns-sd` und
`afp-browse`: Dateidialog und Files starten im Wegwerf-Bus keine Netzsuche
(kein wsdd). Der Papierkorb bleibt.

## 5. Integration

Nach dem Muster 0.2.6-2: Arbeitsordner `~/projects/querencia-desktop-030`,
Skript `desktop-030-integrieren.sh` (`pruefen | aktivieren | desktop-neustart |
zurueck | desktop-neustart-zurueck`, `DESKTOP_030_FREIGABE=ja`), Ablage mit
Programm und `share/locale/de`, Rückfall 0.2.6-2 `a9e3600107c1cd53`, Tausch
mit `tausch-desktop.py`, Probe auf umgelenkten Kopien. Tags `desktop/0.3.0`
(Code) und `desktop/0.3.0-int` (Integrationsstand).

## 6. Verhaltensänderung (für CHANGELOG und Release Notes)

> Kopieren und Verschieben auf der Arbeitsfläche zeigen jetzt ein
> Fortschrittsfenster mit „Abbrechen“ und fragen bei einem vorhandenen Namen
> nach (Ersetzen, Überspringen, Umbenennen); bisher wurde nie überschrieben.

## 7. Risiken

| Risiko | Gegenmaßnahme |
|---|---|
| Datenverlust beim Ersetzen über Dateisysteme oder GVfs | Vorgabe Überspringen; Ersetzen nur nach ausdrücklicher Antwort; Prüffälle N3/N8/N9 mit Inhaltsvergleich |
| Konfliktdialog blockiert den Job dauerhaft (Desktop-Neustart mitten in der Frage) | Job-Abbruch beim Beenden; Tausch-Ablauf mit offenem Dialog prüfen |
| Fortschrittsfenster verliert die Titelleiste (QD-076) | Fenster beim Schließen zerstören, neu bauen |
| Symbole im Fortschrittsfenster (QD-106) | nur Symbole über `symbole` aus dem GUI-Crate |
| Einhängen in der Prüfung trifft echte Platten | Läufe mit gesperrten GVfs-Volume-Monitoren, nur Attrappe |
