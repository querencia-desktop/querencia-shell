# Querencia Dateien – Dateioperationen (Welle B3): Aufbau und Prüfprotokoll

**Stand:** 23.10.2026, Zweig `dateien-operationen` (Tag `dateien/operationen-1`),
Basis Gerüst `dateien/geruest-1` und Zweig `dateien` (a63d07b, mit den
Ansichten von B2, `dateien/ansichten-1`). Auftrag QD-140
Welle B, Worker B3. Schnittstelle: `docs/dateien.md` §3.

## 1. Was B3 beiträgt

| Datei | Inhalt |
|---|---|
| `crates/querencia-dateien/src/operationen.rs` | `registrieren()` (Haken `anwendung`, `fenster`, `neue_ansicht`, `ablage`), `Dienst` (folgt `App::queue`: Fortschritt, Fragen und Fehler für **jeden** Auftrag, auch das Umbenennen des Fensters), `Operationen` je Fenster mit den B3-Aktionen aus §3.6 |
| `zwischenablage.rs` | Schreiben: `GdkFileList`, `text/uri-list`, `x-special/gnome-copied-files` (`copy`/`cut`), bei Ausschneiden `application/x-kde-cutselection` = `1`. Lesen in dieser Reihenfolge: GNOME-Text, `GdkFileList`, URI-Liste (mit KDE-Markierung). Nach dem Einfügen ausgeschnittener Dateien wird die Zwischenablage geleert |
| `ziehen.rs` | `DragSource`/`DropTargetAsync` an jeder Ansicht (über `eintrag_bei`, Markierung über `ablage_markieren`) und an Orten/Pfadknöpfen (Haken `ablage`); Regel `entscheiden` nach Nautilus 2.30 |
| `fortschritt.rs` | Fenster „Dateioperationen“: Kopie aus Desktop 0.3.0 (desktop-030 7ae54ad) mit Vermerk; Fenstertitel nennt, was läuft („Dateien werden kopiert …“) |
| `konflikt.rs` | Konfliktfrage (E2): Kopie aus Desktop 0.3.0 mit Vermerk; Vorgabeknopf hat die Tastatur vor dem Zeigen, kein „Ersetzen“ beim Wiederherstellen, Fenster gehört zum auslösenden Fenster |
| `eigenschaften_dialog.rs` | `pub fn zeigen(fenster: &Rc<Fenster>)` für B1s `win.eigenschaften` (Anhängen: `tools/dateien-operationen-pruefung/b1-anhaengen.patch`): Grundlegend (Ordnergröße in eigenem Faden, laufend), Zugriffsrechte (Klapplisten wie Nautilus 2.30, „Datei als Programm ausführen“, sofort geschrieben, nur für den Besitzer), Öffnen mit (Radioknöpfe, Wahl = Standard) |
| `po/de.operationen.po` | Teilkatalog (93 Texte); msgids, die B1 schon hat, stehen nicht darin |
| `tools/dateien-operationen-pruefung/` | isolierte GUI-Prüfung (siehe §4) |

### 1.1 Aktionen

| Aktion | Wirkung |
|---|---|
| `win.ausschneiden`, `win.kopieren`, `win.einfuegen`, `win.in-ordner-einfuegen` | Zwischenablage; Einfügen ausgeschnittener Dateien = Verschieben |
| `win.papierkorb` | ohne Frage in den Papierkorb; **im Papierkorb** fragt es wie `win.loeschen` |
| `win.loeschen` | Frage „endgültig löschen“, Vorgabe Abbrechen, Esc bricht ab |
| `win.papierkorb-leeren` | Frage wie im Desktop, Vorgabe Abbrechen |
| `win.wiederherstellen` | nur im Papierkorb; an den Ursprungsort, nie ersetzen |
| `win.ordner-anlegen`, `win.dokument-anlegen` (s: URI der Vorlage, `""` = leere Datei) | danach `Fenster::nach_erscheinen_umbenennen` |
Öffnen, Öffnen mit, Eigenschaften, Umbenennen und die Tasten gehören dem
Fenster (B1, §3.6); B3 registriert dort nichts. Der Eigenschaften-Dialog ist
eine Funktion, die B1 an `win.eigenschaften` hängt; „Mit anderer Anwendung
öffnen …“ kommt mit der Beta (B §1.10). Die
Aktionen sind nur aktiv, wenn sie etwas tun können (Auswahl, Ort,
Zwischenablage, Papierkorbinhalt); „Einfügen“ wird zusätzlich geprüft, wenn das
Fenster aktiv wird (Wayland gibt die Zwischenablage erst mit der Tastatur).

### 1.2 Regel beim Ablegen (`ziehen::entscheiden`, Unit-Tests)

1. Auf den Papierkorb: in den Papierkorb (nicht aus dem Papierkorb).
2. In sich selbst oder in einen Unterordner: nichts.
3. Aus dem Papierkorb: nur Verschieben, gleich welche Taste (QD-113); die
   `.trashinfo` geht mit.
4. Strg+Umschalt verknüpft, Strg kopiert, Umschalt verschiebt.
5. Ohne Taste: gleiches Dateisystem (`id::filesystem`) verschiebt, sonst
   Kopieren; GVfs-Orte zählen als anderes Dateisystem (keine blockierende
   Abfrage).
6. In den Ordner, in dem die Dateien schon liegen: nichts (mit Strg: Kopie
   mit freiem Namen).
7. Bietet die Quelle nur eine Aktion an (Strg/Umschalt dort beim
   Ziehbeginn), gilt diese.

## 2. Gemessene Fallen (labwc 0.9.8 headless, GTK 4.22)

1. **Tasten beim Ziehen:** Nur die **Quelle** sieht Strg/Umschalt, und nur
   beim Ziehbeginn (`DragSource::prepare`, `current_event_state`). Das Ziel
   bekommt in `drag-motion`/`drop` nie Tastenzustände, auch im selben
   Fenster. Darum legt die Quelle die Aktion beim Ziehbeginn fest (Strg →
   nur COPY, Umschalt → nur MOVE, Strg+Umschalt → nur LINK) und das Ziel
   wendet die Regel auf dieses Angebot an. Tastenwechsel während des
   Ziehens wirken nicht.
2. **`gdk::Drop::actions()`** wird nach dem ersten Status zur einen vom
   Compositor gewählten Aktion (wlroots nimmt die bevorzugte des Ziels).
   Wer daraus das Angebot liest, bleibt bei der ersten Vermutung hängen.
   Gelöst: Angebot der eigenen Quelle gemerkt, bei fremden Quellen das
   Angebot beim Eintritt. Das Ziel beendet den Drop mit der wirklich
   ausgeführten Aktion.
3. **Fokus der Konfliktfrage:** `grab_focus` nach `present` kam zu früh; der
   Fokus lag auf dem Ausklapper „Einen neuen Namen …“, Eingabe klappte ihn
   auf, statt „Überspringen“ zu wählen. Gelöst mit
   `GtkWindowExt::set_focus` vor dem Zeigen und `grab_focus` im Leerlauf.
   Die Vorlage im Desktop 0.3.0 (`frage.rs`) hat dieselbe Stelle.
4. **Das Fortschrittsfenster nimmt beim Erscheinen die Tastatur** (wie im
   Desktop). Wer weitertippt, tippt dort. Für 0.1 so gelassen (GNOME 2
   zeigte es ebenso vorn); Vorschlag für den gemeinsamen Baustein:
   `focus-on-map` prüfen.
5. **`Outcome::arrived`** enthält Namen im Zielordner (nur Wiederherstellen
   volle Pfade), nicht volle Pfade, wie der Text in `auftraege.rs` sagt.
   `operationen::angekommen` löst beides auf.
6. Ohne GVfs ist der Papierkorb der Ordner `$XDG_DATA_HOME/Trash/files`;
   `ziehen::im_papierkorb` muss den Ordner **selbst** erkennen, nicht nur
   seine Einträge (sonst warf Entf im Papierkorb erneut in den Papierkorb).
7. GIO liest Test-Starter eines Benutzer-`applications`-Ordners nur über
   dessen `mimeinfo.cache`; ohne Cache erscheinen sie in keinem
   „Öffnen mit“.
8. **Prüfwerkzeug:** Eine qeingabe-Sitzung aus reinen Tasten („key Return“)
   erreichte kein Programm; eine, die mit einer Modifikatortaste begann,
   schon. `qeingabe.c` tippt deshalb zu Beginn einmal Umschalt. Ein Klick
   vor dem Einblenden des Fensters trifft den Hintergrund und öffnet das
   labwc-Wurzelmenü, das dann die Tastatur hält: erst auf das Programm
   warten, nie auf freie Fläche klicken.
9. Befund für `querencia-dateiarbeit` (kein Datenverlust): Zwei gleichzeitige
   Kopien derselben Quelle in ihren eigenen Ordner wählen beide beim Start
   „x (Kopie)“; die zweite fragt dann „Ordner … zusammenführen?“ statt
   „x (Kopie 2)“ zu nehmen.
10. **Gesten der Ansichten:** B2s Symbol- und Listenansicht nehmen jeden
    Druck mit eigenen Gesten (Gummiband, Klicks) auf der inneren Liste;
    eine `DragSource` auf dem äußeren Widget bekam in der Bubble-Phase nie
    einen Druck. Gelöst mit `PropagationPhase::Capture`: Sie beansprucht die
    Folge erst ab der Zieh-Schwelle und nur auf einem Eintrag
    (`eintrag_bei`), auf freier Fläche bleibt das Gummiband (Fall Z10).

## 3. Unit-Tests

`cargo test -p querencia-dateien`: 73 bestanden, 1 ignoriert (mit B1 und
B2); Workspace 620 bestanden, 0 fehlgeschlagen, 3 ignoriert. Davon B3 (27): Zwischenablage-Format
(6: Schreiben, Rücklesen, GNOME vor KDE, KDE-Markierung bei Dolphin,
Dateiliste vor URI-Liste, nichts einzufügen), Ablage-Regel (8, darunter
echte Dateien, Papierkorb, GVfs-Ort), Fortschrittstexte (4), Konflikttexte
(2), Zugriffsrechte (5, darunter Ordnerzählung im Faden), Operationen (2).

## 4. Isolierte GUI-Läufe

**Rahmen:** `werkzeug/dateien-pruefrahmen/huelle.sh` Stufe L
(`GIO_USE_VFS=local`) und Stufe G (gvfsd im Wegwerf-Bus, `trash:///`) mit
`--labwc` (headless labwc 0.9.8, 1280×720, für Fotos über `qausgabe`
1024×768 und 1920×1080; `env -i`, eigenes HOME, privater Bus, Netz nur
`lo`), Dateisystemgrenze `--tmpfs grenze:64M`,
Sprache `de_DE.UTF-8`, Kataloge aus `i18n.py mo`. Eingaben über
`qeingabe` (virtueller Zeiger und Tastatur); Aktionen ohne Taste über den
privaten Sitzungsbus der Hülle (`org.gtk.Actions.Activate` auf
`/org/querencia/Dateien/window/1`, derselbe Weg wie Menü und Kürzel).
Ansicht: B2s Listenansicht (Strg+2 beim Start, feste Zeilen), Ziehen
zusätzlich in der Symbolansicht (Z9, Z10).
Eigenschaften: mit der Integrationsprobe (Bau mit `b1-anhaengen.patch`),
denn ohne die Zeile von B1 öffnet `win.eigenschaften` noch B1s Dialog. Für die
Zwischenablage Querencia Files QF-0t (`f87447156d6c9638`) und Desktop 0.3.0
(Nachweis-Bau des Zweigs desktop-030), beide nur lesend eingeblendet.

**Aufruf:**

```bash
bash tools/dateien-operationen-pruefung/bauen.sh <bin-ordner>
QD_QEINGABE=<bin-ordner>/qeingabe QD_LOCALE=<kataloge> QD_DESKTOP=<desktop> QD_FILES=<files> \
  bash tools/dateien-operationen-pruefung/gesamt.sh target/debug/querencia-dateien <ausgabe> [dunkel]
```

ERGEBNISTABELLE

## 5. Fotos

Nicht im Repo: 
mit `SHA256SUMS` (Adwaita hell bzw. `GTK_THEME=Adwaita:dark`; die Module
setzen keine eigenen Farben), Auflösungen 1024×768 und 1920×1080, dazu der
Lauf 1280×720:
Fortschrittsfenster mit zwei Vorgängen, Konfliktfrage Datei und Ordner,
Eigenschaften (Grundlegend Ordner und Datei, Zugriffsrechte, Öffnen mit),
Frage „Papierkorb leeren“, Frage „endgültig löschen“, Papierkorbinhalt,
Zwischenablage mit dem Desktop. Ergebnis-CSVs und Logs:
.

## 6. Nicht gemessen, Grenzen

- **GVfs-Netzorte:** Kopieren auf SFTP/SMB (Stufe G mit `--sshd`) nicht
  gemessen; `trash:///` mit gvfsd ja (Szenario `papierkorb-g`).
- Ansichten: Listenansicht durchgehend, Symbolansicht nur für Ziehen und
  Gummiband (Z9, Z10).
- Ziehen zu fremden Programmen (Files, Desktop) und von dort: nur über die
  Zwischenablage gemessen; das Ziehformat ist dasselbe (`GdkFileList` und
  `text/uri-list`).
- Alt beim Ziehen (Nachfragemenü) und Ablage auf Reiter: B (Beta).
- „Mit anderer Anwendung öffnen …“: Beta, als eigene Liste
  (`GtkAppChooserDialog` ist seit GTK 4.10 veraltet, C §6).
- Eigenschaften mehrerer Dateien: je ein Fenster (höchstens acht), keine
  gemeinsame Ansicht (B).
- Rückgängig: erst nach der Beta (E8).
