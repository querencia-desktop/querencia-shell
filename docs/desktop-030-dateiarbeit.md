# Desktop 0.3.0: Dateiarbeit mit Fortschritt, Abbruch, Konfliktfrage und Einhängen

**Stand:** 23.10.2026, Worker desktop-030 (Welle A von „Querencia Dateien“,
QD-140, Schritt 5). Code: Tag `desktop/0.3.0`, Integration:
`desktop/0.3.0-int`, Arbeitsordner `~/projects/querencia-desktop-030`.
Grundlage: Machbarkeitsvorlage , Inventur A
§3, Übergaben  §6 und
 §6.

## 1. Ausgangslage

| Was | Stand |
|---|---|
| Live (gemessen über `/proc/<pid>/exe`) | Desktop 0.2.6-2 `a9e36001` läuft (PID 2556); der Link zeigt seit der Aktivierung von 0.2.7-2 auf `e193b23d` |
| Rückfall- und Ausgangsstand von 0.3.0 | Desktop 0.2.7-2 `e193b23d` (Tag `desktop/0.2.7-2`) |
| Code-Basis | main `f57ce46` (Merge oberflaeche und dateiarbeit 0.1.1), in `desktop-030` gemergt |
| Dateioperationen bis 0.2.7 | `fileops.rs` blockierend im GIO-Thread-Pool, ohne Fortschritt und Abbruch; Kopie auf belegten Namen still mit freiem Namen, Verschieben abgelehnt („nie überschreiben“) |
| Laufwerke bis 0.2.7 | nur eingehängte Mounts; Einhängen gab es nicht |

## 2. Entscheidungen, die gelten

- **E2:** Konfliktregel „fragen“ mit Vorgabeknopf **Überspringen**.
- **E3:** Einhängen und Automount gehören in den Desktop.
- **E8:** Rückgängig erst nach der Beta.
- Fortschritt nach Variante A der Inventur (Bibliothek im Prozess, eigenes
  Fenster je Programm); ein gemeinsamer Dienst folgt in Welle C.
- Status-Session 23.10.: `IconLoading::WithoutPreload` ist eine gewollte
  Verhaltensänderung, belegt mit QD-106-Belastung und Fotos.

## 3. Was sich geändert hat

| Bereich | 0.3.0 |
|---|---|
| Aufträge | `transfer.rs` gibt Kopieren, Verschieben, Papierkorb, Löschen, Papierkorb leeren, Neuer Ordner, Neues Dokument und Umbenennen an `auftraege::Queue` (ein Faden je Auftrag, nie GLibs Thread-Pool); `job_event` setzt Fortschritt, Fragen und Abschluss um; nach dem Abschluss wie bisher Platzieren an der Ablagestelle, Neuaufbau, Fehlerdialog (nicht nach Abbruch) |
| Fortschrittsfenster | `desktop_view/fortschritt.rs`: Fenster „Dateioperationen“ nach 1 s Laufzeit, je Auftrag Titel („Kopieren von 3 Dateien nach „Ordner A““), Balken, „x von y – noch n Sekunden (Rate/s)“, aktuelle Datei, Abbrechen; schließt sich ohne Aufträge; QD-076: zerstören statt verstecken; wartet, solange eine Frage des Auftrags offen ist |
| Konfliktfrage | `desktop_view/frage.rs`: „Datei „x“ ersetzen?“ / „Ordner „x“ ersetzen?“ / „Ordner „x“ zusammenführen?“, Größe und Datum beider Seiten, neuer Name (Vorschlag aus dem Crate), „Diese Aktion auf alle Dateien anwenden“, Abbrechen / **Überspringen** (Vorgabe) / Umbenennen / Ersetzen (/ Zusammenführen); Escape = Abbrechen; ohne Papierkorb: „sofort löschen?“ mit Vorgabe Abbrechen |
| Einhängen (E3) | nicht eingehängte Wechsel- und optische Medien als Symbol (`volumes::listed`, eingebaute Partitionen nie); Doppelklick hängt ein und öffnet, Menü „Einhängen“; Automount bei `volume-added` mit `should_automount`; Platz bleibt beim Einhängen (`Volume::slot`) |
| Kennwortfrage | `desktop_view/kennwort.rs`: eigene, nicht modale Fenster für `ask-password`/`ask-question` auf einer `gio::MountOperation` (siehe §6 Befund) |
| Papierkorb | `papierkorb::Papierkorb` (Monitor, Zählung außerhalb des Hauptfadens, Signal); Leeren als Auftrag |
| Überwachung | `ueberwachung::FolderWatch` mit Warten auf einen fehlenden Ordner; eigene Entprellung und Neuanmeldung entfallen |
| Öffnen | `oeffnen::plan_open`, `is_trusted`, `mark_trusted`, `file_manager()` (Dateien vor Files); „Öffnen mit“ zeigt wie bisher Standard- und empfohlene Programme |
| Symbole (QD-106) | Arbeitsfläche, Eigenschaften und Desktop-Menü ohne GTKs Vorladen (`symbole`, `IconLoading::WithoutPreload`) |
| Entfallen | `fileops.rs`, `rename.rs`, Teile von `volumes.rs`/`icons.rs`, `data/icons/` (jetzt im Crate) |
| Texte | 118 msgids in `querencia-desktop` (de vollständig); Fehlergründe und Kopienamen aus `querencia-dateiarbeit.mo`, die die Ablage mitbringt |

## 4. Prüfung

### 4.1 Unit-Tests

Desktop **70** Tests (0.2.7: 93). Umgezogen ins Crate: fileops 8, volumes 9,
icons 4, rename 7, Files-Kennung 1 (= 29); neu: Fortschrittstexte 3,
Konfliktfragen 2, Medien ohne Einhängung 1. Workspace fmt, clippy
(`-D warnings`) und Tests grün (Log `nachweis/pruefung/`).

### 4.2 Isolierte GUI-Läufe

Werkzeug `~/projects/querencia-desktop-030/werkzeug/` (aus 0.2.6-2), Abläufe
über `lauf.sh <tag> <desktop> <ablauf>`:

| Ablauf | Inhalt |
|---|---|
| `gesamt` | Regression der Dateioperationen aus 0.2.3 (QD-144), 48 Fälle |
| `dateiarbeit` | N1 Fortschritt (40 000 Dateien), N2 Abbruch mitten im Baum, N3–N11 jede Konfliktantwort, „für alle“ mit Überspringen/Ersetzen/Umbenennen, Escape, Verschieben mit Ersetzen, Zusammenführen, N12–N14 Einhängen per Doppelklick, mit Kennwort, Automount, N15 Papierkorb leeren als Auftrag |
| `dialoge` | Fotos von Fortschrittsfenster, Datei- und Ordnerfrage, Kennwortfrage, Medienmenü |
| `belastung` | QD-106: Dateien anlegen/entfernen, Symbolthema wechseln, Menüs mit Programmsymbolen, Eigenschaften; Zähler `qvorlade.so` |
| `medien` | Einhängen mit Lebenszeichen nach jedem Schritt (Befund §6) |
| `files`, `tausch`, `texte`, `lage`, `ordner`, `oeffnen`, `fotos` | wie 0.2.6-2 |

Ergebnisse: §7 (nach dem Lauf eingetragen).

### 4.3 Isolation

`isoliert.py` setzt `GVFS_MOUNTABLE_DIR` auf eine Kopie der
gvfs-Mountbeschreibungen ohne `network`, `wsdd`, `smb-browse`, `dns-sd` und
`afp-browse`: Dateidialog und Files starten im Wegwerf-Bus keine Netzsuche.
Laufwerke kommen nur aus der Attrappe `qattrappe.c` (GIO-Modul, seit 0.3.0
auch nicht eingehängte Medien mit Einhängen, Kennwort und Automount); die
Volume-Monitore von GVfs sind in diesen Läufen gesperrt.

## 5. Integration

Integrationsordner nach dem Muster 0.2.7-2 (`desktop-030-integrieren.sh`,
`DESKTOP_030_FREIGABE=ja`): `pruefen`, `aktivieren` (Ablage mit Programm und
drei Katalogen, Link), `desktop-neustart` (laufend 0.2.7-2 oder 0.2.6-2),
`zurueck` (Link auf 0.2.7-2 `e193b23d`), `desktop-neustart-zurueck`.
Repo-Kopie `integration/desktop-030/` ohne Binaries und Kataloge.

## 6. Befunde

1. **GTKs Kennwortdialog stürzt im Desktop ab.** Nach erfolgreichem
   Einhängen mit Kennwort über `gtk::MountOperation` endete der Desktop beim
   Freigeben des (modalen) Dialogs mit SIGSEGV in
   `wl_proxy_marshal_flags` (gtk4-layer-shell), mit und ohne Elternfenster;
   Abbrechen stürzte nicht ab. 0.3.0 stellt Kennwort- und Auswahlfragen
   selbst in nicht modalen Fenstern. Wichtig: GIOs eigener Handler von
   `ask-password` antwortet sonst sofort „nicht behandelt“; die Signalausgabe
   wird deshalb gestoppt (wie bei `show-processes`).
2. **Fokus:** Das Fortschrittsfenster, das nach 1 s erschien, nahm einer
   offenen Konfliktfrage die Tastatur. Es wartet jetzt auf die Antwort.
3. **Teildatei beim Abbruch:** Das Crate baut ein Objekt unter einem Baunamen
   und benennt es erst fertig um; ein Abbruch mitten im Baum hinterlässt im
   Ziel weder Teildatei noch halben Ordner (N2).
4. **Laufwerksbilder ohne Vorladen:** Die eingebauten Laufwerksbilder sind
   SVG-Bytes (`gio::BytesIcon`); `symbole` kann sie nicht als Schlüssel
   führen und schlug sie bei jedem Neuaufbau neu nach. GTK lud sie dann im
   Hauptfaden über glycin (eigener Prozess): normal ~50 ms je Symbol, nach
   einem abgebrochenen großen Auftrag (Platte mit Löschen beschäftigt)
   2,5–4,5 s – der Desktop reagierte bis zu 7 s nicht (strace:
   `ppoll` auf den glycin-Kanal). Der Desktop behält diese Bilder jetzt je
   Bild, Größe und Skalierung (`volume_image`); Antwortzeit nach dem Abbruch
   wieder 1 s einschließlich Eingabe (Prüffall N2). Für den Dateimanager
   gilt dasselbe: eigene SVG-Symbole einmal laden.
5. **Fenster „Dateioperationen“ wanderte:** Die Zeile mit Menge, Restzeit
   und Rate war nicht gekürzt; das Fenster wuchs und schrumpfte mit jeder
   Aktualisierung, „Abbrechen“ am Ende der Leiste wanderte mit, und Klicks
   gingen daneben (im Prüflauf 4 von 4 Mal). Jetzt feste Breite 500 px,
   lange Texte gekürzt.

## 7. Ergebnisse

Release `c9fd30fb…` aus Commit `364b895` (Tag `desktop/0.3.0`), Neubau
byte-gleich; Kataloge `querencia-desktop.mo` `e5d3ee8f…`,
`querencia-oberflaeche.mo` `5fcc46d8…` (gleich 0.2.7-2),
`querencia-dateiarbeit.mo` `4d9209d3…`. Belege im Arbeitsordner
`nachweis/` (Repo-Kopie `integration/desktop-030/nachweis/`).

| Prüfung | Ergebnis |
|---|---|
| Workspace fmt, clippy `-D warnings`, build | sauber |
| Workspace-Tests | 524 bestanden, 0 fehlgeschlagen, 2 ignoriert; Desktop 70 |
| Deutsch unverändert gegen 0.2.7 (`deutsch-gleich-030.py`) | 116 von 120 Übersetzungen gleich (46 davon jetzt im Katalog des Crates), 0 geändert; 4 entfallen mit der alten Leeren-Schleife |
| `gesamt` (QD-144) | 48/0 (zweimal); 0.2.6-2 zur Eichung zweimal 48/0 |
| `dateiarbeit` | 52/0 (N1 Fortschritt 40 000 Dateien, N2 Abbruch bei ~28 000 Objekten ohne Reste, N3–N11 alle Antworten, N12–N14 Einhängen, Kennwort, Automount, N15 Papierkorb leeren) |
| `medien`, `abbruch` | 13/0, 5/0 |
| `files` live/neu/alt/beide | je 11/0 |
| `tausch` von 0.2.7-2 und von 0.2.6-2 | je 15/0 |
| `texte` de/en, `lage`, `ordner`, `oeffnen` | 20/0, 20/0, 13/0, 13/0, 12/0 |
| `fotos` hell/dunkel × 1920/1024 | je 10/0 |
| `dialoge` hell/dunkel × 1920/1024, en | je 9/0 |
| `belastung` 300 s (QD-106) | 0.3.0: 0 Vorladeaufträge, Desktop läuft, kein `object_already_finalized`; 0.2.7-2 zum Vergleich: 435 |
| Probe Integrationsskript | 54/54 |
| `pruefen` am echten Stand | D=alt (Link 0.2.7-2, laufend 0.2.6-2) |

Ein erster Lauf `gesamt` der Schlussreihe hatte 6 Folgefehler, weil
gleichzeitig die Probe lief und alte Diagnose-Läufe beendet wurden (der
Papierkorb-Auftrag brauchte länger als die feste Wartezeit des Werkzeugs);
ohne Parallellast zweimal 48/0. Bilder: .

## 8. Grenzen

- GVfs-Ziele (SMB/SFTP), echte Wechseldatenträger über udisks und echte
  `MountOperation`-Fragen von GVfs sind nicht gemessen (Stufe G/H des
  Prüfrahmens); die Attrappe bildet `ask-password` nach.
- „Kein Papierkorb am Ort“ (`Conflict::NoTrash`) ist im GUI-Lauf nicht
  nachgestellt (der Wegwerf-Bus hat keinen Ort ohne Papierkorb); im Crate
  getestet.
- Die Frage „Ausführen oder anzeigen?“ für ausführbare Textdateien (Nautilus
  2.30) fehlt noch; solche Dateien öffnen wie bisher im Standardprogramm.
- Ein gemeinsames Fortschrittsfenster für Desktop und Dateien (Variante B)
  kommt mit Welle C.
