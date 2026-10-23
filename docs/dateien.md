# Querencia Dateien (`crates/querencia-dateien`)

**Stand:** 23.10.2026, Querencia Dateien 0.1.0 (Welle B, QD-140), Zweig
`dateien`: Gerüst `dateien/geruest-1` (B1), Ansichten `dateien/ansichten-1`
(B2), Operationen `dateien/operationen-1` (B3), zusammengebaut mit main
c8d5e66. Eigener Dateimanager nach Nautilus 2.30 (Browser-Modus), Rust/GTK 4
ohne libadwaita. 0.1 läuft neben Querencia Files (kein Ordnerprogramm).
Ansichten: `docs/dateien-ansichten-pruefung.md`; Operationen:
`docs/dateien-operationen-pruefung.md`.
Grundlage:  (Soll, Menüs, Abnahme 0.1),
, `docs/querencia-oberflaeche.md`,
`docs/dateiarbeit.md`.

## 1. Kennungen

| | |
|---|---|
| Name | de „Querencia Dateien“, en „Files“ (Entscheidung S2) |
| App-ID, Desktop-ID | `org.querencia.Dateien` (läuft in 0.1 neben Querencia Files, ohne `MimeType=inode/directory`) |
| Binary | `querencia-dateien`, GPL-3.0-or-later |
| Textdomäne | `querencia-dateien` (msgid englisch, `po/de.po`); dazu die Kataloge `querencia-oberflaeche` und `querencia-dateiarbeit` im selben Ordner |
| Einstellungen | `~/.config/querencia/dateien.ini` |
| Lesezeichen | `~/.config/gtk-3.0/bookmarks` (gemeinsam mit GTK-Dateiauswahl und Files) |

## 2. Aufbau und Zuständigkeit in Welle B

Drei Worker arbeiten parallel im selben Crate, jeder in eigenen Dateien.

| Datei | Inhalt | Worker |
|---|---|---|
| `main.rs` | Start, Stil, App-Aktionen, **Registrierung der Module** (`erweiterungen`) | B1 (B2/B3 nur `mod`-Zeilen und je ein Aufruf, §3.5) |
| `app.rs` | `App` (gemeinsamer Zustand), `Register` (Haken) | B1 |
| `eintrag.rs` | GObject `Eintrag` (§3.1) | B1 |
| `modell.rs` | `Ordnermodell` (§3.2) | B1 |
| `ansicht.rs` | Trait `Ansicht`, `Rueckruf`, Umschaltung `Ansichten`, `Ersatzansicht` (§3.3) | B1 |
| `fenster.rs` | Fenster nach B §3.2, Aktionen, Verlauf (§3.4) | B1 |
| `menue.rs` | Menüleiste über `classic_menu`, Kürzel, Untermenüs „Dokument anlegen“ und „Öffnen mit“ | B1 |
| `werkzeugleiste.rs`, `adressleiste.rs`, `seitenleiste.rs`, `statusleiste.rs` | Leisten | B1 |
| `oeffnen.rs` | Öffnen über `dateiarbeit::oeffnen::plan_open`, Rückfrage ausführbarer Texte | B1 |
| `konfig.rs`, `einstellungen.rs` | `dateien.ini`, Migration M6, Einstellungen mit fünf Reitern | B1 |
| `lesezeichen.rs`, `orte.rs`, `regeln.rs`, `info.rs`, `i18n.rs` | Hilfen, Info/Hilfe | B1 |
| `symbolansicht.rs`, `listenansicht.rs`, `ansichtskern.rs`, `ansichtsregeln.rs`, `tippsuche.rs`, `umbenennen.rs`, `kontextmenue.rs` | Ansichten gegen das Trait (gemeinsamer Kern), Tippsuche, Umbenennen im Platz, Kontextmenüs | B2 „dateien-ansichten“ |
| `operationen.rs`, `zwischenablage.rs`, `ziehen.rs`, `fortschritt.rs`, `konflikt.rs`, `eigenschaften_dialog.rs` | Dateioperationen über `dateiarbeit::auftraege`, Eigenschaften (Grundlegend, Zugriffsrechte, Öffnen mit; an `win.eigenschaften`) | B3 „dateien-operationen“ |

Die Symbolansicht ist ausdrücklich **austauschbar**: Jede Implementierung
von `Ansicht`, die für `Ansichtsart::Symbole` registriert ist, ersetzt
die andere (Prototyp `dateien/prototyp-2` wird noch am Bildschirm
beurteilt). Ohne registrierte Ansicht zeigt das Fenster die
`Ersatzansicht` (einfache Liste mit Symbol und Name).

## 3. Schnittstelle der Welle B

Änderungen an diesem Abschnitt nach `dateien/geruest-1` nur mit Nachricht
an die Status-Session (sie gibt sie an B2/B3 weiter).

### 3.1 `Eintrag` (`eintrag.rs`)

Ein Objekt je Datei, **unveränderlich**: Ändert sich die Datei, setzt das
Modell ein neues `Eintrag` an dieselbe Stelle (recycelte Zellen werden
über `items-changed` neu gebunden, nie veraltete Daten).

- `Eintrag::aus_info(&ordner, &info)`; `daten() -> &Daten` mit `datei`,
  `name` (Dateiname = Identität im Ordner), `anzeige` (Anzeigename, bei
  Startern deren Name), `symbol: gio::Icon`, `info: gio::FileInfo`,
  `ist_ordner`, `verborgen`, `verweis`, `starter`, `typ` (schneller
  Inhaltstyp, Ordner `inode/directory`), `suchname` (Tippsuche), `sort`.
- Abkürzungen: `datei()`, `name()`, `ist_ordner()`, `ziel()` (bei
  `computer:///` und Papierkorb `standard::target-uri`, sonst die Datei),
  `gewaehlt()` (für die Statusleiste), `beschreibung()` (Typbeschreibung).
- Abgefragte Attribute: `eintrag::ATTRIBUTE` (lokal, mit `unix::mode`,
  `owner::user`, `owner::group` für die Listenspalten, ohne
  `standard::icon` und `standard::content-type`, C §1), bei GVfs-Orten
  zusätzlich `ATTRIBUTE_ORTE` (`standard::icon`, `standard::target-uri`,
  `trash::orig-path`, `trash::deletion-date`); `attribute_fuer(&ordner)`.
- Symbole immer über `querencia_oberflaeche::symbole::set(&bild,
  &eintrag.daten().symbol)` (QD-106).

### 3.2 `Ordnermodell` (`modell.rs`)

`gio::ListStore<Eintrag>` → `FilterListModel` (verborgene) →
`SortListModel` (Nautilus-Ordnung, `regeln::vergleiche` mit
`FilenameCollationKey`, Ordner zuerst) → `MultiSelection`. Ein Modell je
Fenster; **alle Ansichten zeigen `modell.auswahl()`**, Auswahl und
Reihenfolge gehören dem Modell.

| Methode | Zweck |
|---|---|
| `laden(&ordner)`, `neu_laden()`, `stoppen()`, `ordner()`, `laedt()` | asynchron in Blöcken (200, dann 1000) lesen; `neu_laden` behält die Auswahl |
| `auswahl() -> &MultiSelection` | Modell jeder Ansicht |
| `eintrag(pos)`, `anzahl()`, `eintrag_von(&datei)`, `position_von(&datei)` | Zugriff, Positionen in `sortiert`/`auswahl` |
| `gewaehlte()`, `waehlen(&[datei])`, `alle_waehlen()`, `auswahl_umkehren()` | Auswahl über die Identität |
| `sortierung()`, `sortierung_setzen(Sortierung)`, `verborgene_zeigen()`, `verborgene_setzen(bool)` | Ordnung und Filter |
| `bei_zustand(Fn(&Zustand))` | `Laedt(ordner)`, `ErsterBlock`, `Geladen(n)`, `Gestoppt`, `Fehler(text)`, `OrdnerWeg` |
| `bei_erscheinen(&datei, FnOnce(&Eintrag))` | sobald eine Datei im Ordner ist (sofort, wenn schon da) |
| `bei_sortierung(Fn(Sortierung))` | nach jeder neuen Sortierung (Menü, Einstellungen, Spaltenkopf) |

Änderungen kommen über `dateiarbeit::ueberwachung::FolderWatch`
(gebündelt): neue und geänderte Dateien werden neu abgefragt und
ersetzt, Umbenennen behält die Auswahl unter dem neuen Namen, `NeuLaden`
liest neu, `OrdnerWeg` geht im Fenster zum nächsten vorhandenen
Elternordner. Die Spaltenköpfe der Listenansicht ändern die Sortierung
über `sortierung_setzen` (nicht über einen eigenen Sorter der
`ColumnView`), damit beide Ansichten dieselbe Ordnung zeigen.

### 3.3 Trait `Ansicht` und `Rueckruf` (`ansicht.rs`)

```rust
pub trait Ansicht {
    fn art(&self) -> Ansichtsart;
    fn widget(&self) -> gtk4::Widget;
    fn set_modell(&self, modell: &Ordnermodell);
    fn modell(&self) -> Option<Ordnermodell>;
    fn verbinden(&self, rueckruf: Rc<dyn Rueckruf>);
    fn zoom(&self) -> Zoom;
    fn zoom_setzen(&self, zoom: Zoom);
    fn zoomstufen(&self) -> &'static [u32] { &ZOOMSTUFEN }        // 33…400 %
    fn fokus(&self);
    fn fokus_auf(&self, datei: &gio::File, waehlen: bool);
    fn umbenennen_anstossen(&self, datei: &gio::File, ganzer_name: bool);
    fn umbenennen_laeuft(&self) -> bool { false }
    fn eintrag_bei(&self, x: f64, y: f64) -> Option<Eintrag>;      // nur Symbol/Beschriftung
    fn ablage_markieren(&self, _: Option<&Eintrag>) {}
    fn einstellungen_anwenden(&self, _: &Werte) {}                  // Klickart, Spalten …
}

pub trait Rueckruf {                                    // implementiert das Fenster
    fn aktivieren(&self, eintraege: &[Eintrag], neues_fenster: bool);
    fn kontextmenue(&self, ziel: Kontextziel, widget: &gtk4::Widget, x: f64, y: f64);
    fn adresse_tippen(&self, anfang: &str);             // „/“ oder „~“
    fn zoom_geaendert(&self, zoom: Zoom);               // Strg+Mausrad in der Ansicht
    fn nach_oben(&self);                                // Rücktaste
    fn umbenennen(&self, datei: &gio::File, neuer_name: &str);
    fn melden(&self, text: &str);                       // Statusleiste
}
```

Regeln für jede Ansicht:

1. Sie zeigt `modell.auswahl()` und hält **keine eigene Auswahl oder
   Ordnung**; `fokus_auf` mit `waehlen` geht über `modell.waehlen`.
2. Sie behandelt Navigationstasten, Tippsuche (`regeln::treffer`,
   `Eintrag::daten().suchname`), Eingabe (→ `aktivieren`), Rücktaste (→
   `nach_oben`), „/“ und „~“ (→ `adresse_tippen`), Strg+Mausrad (→
   `zoom_setzen` und `zoom_geaendert`), Menütaste und Rechtsklick (→
   `kontextmenue`; Rechtsklick auf einen nicht gewählten Eintrag wählt ihn
   vorher, auf freie Fläche hebt die Auswahl auf, Ziel `Hintergrund`).
3. Doppelklick bzw. bei `Werte::einfacher_klick` ein Klick ruft
   `aktivieren` mit der Auswahl; Umschalt-Doppelklick und Mitteltaste auf
   einem Ordner mit `neues_fenster`.
4. Tasten, die Dateien betreffen (Entf, Umschalt+Entf, Strg+X/C/V/A, F2,
   Umschalt+F2, Esc), bindet das **Fenster** am Ansichtsbereich
   (`menue::KUERZEL_IM_BEREICH`, Bubble-Phase): Ein Umbenenn-Feld in der
   Zelle, das die Taste selbst verarbeitet, bekommt sie zuerst.
5. Umbenennen: `umbenennen_anstossen` öffnet das Feld (Name ohne Endung
   markiert, `ganzer_name`: alles, `querencia_dateiarbeit::namen::
   selected_chars`); Eingabe prüft mit `namen::check_file_name` und ruft
   `Rueckruf::umbenennen` – das Fenster startet `Op::Rename` über die
   Warteschlange und wählt den neuen Namen, sobald er erscheint. Esc oder
   Fokusverlust ohne Änderung bricht ab.
6. Symbole nur über `querencia_oberflaeche::symbole` (QD-106); recycelte
   Zellen setzen ihr Bild beim Binden neu.

`Ansichten` (die Umschaltung) baut je Art beim ersten Gebrauch eine
Instanz aus der registrierten Fabrik (sonst `Ersatzansicht`), ruft
`set_modell`, `verbinden`, `einstellungen_anwenden`, `zoom_setzen` mit
der Vorgabe aus `dateien.ini` und danach die Haken `neue_ansicht` (§3.5).

### 3.4 `Fenster` (`fenster.rs`)

Für B2/B3: `window()`, `app()`, `ort()`, `modell()`, `ansicht()` (die
gezeigte), `ansichten()`,
`gehe_zu(&ort)`, `zurueck()`, `vor()`, `nach_oben()`,
`bei_ortswechsel(Fn(&gio::File))`, `bei_auswahl(Fn())`,
`melden(text)`, `nach_erscheinen_umbenennen(&datei)` (neuer Ordner,
neues Dokument: wählen, zeigen, Umbenennen starten),
`nach_erscheinen_waehlen(&[datei])` (Ergebnis von Einfügen, Kopieren).
`App` (`app::app_wenn()` oder `fenster.app()`): `konfig()`, `queue()`, `auftrag_starten(job,
Some(fenster))`, `fenster_fuer_auftrag(id)`, `auftrag_vergessen(id)`,
`papierkorb()`, `lesezeichen()`, `laufwerke()`, `alle_fenster()`,
`fenster_von(&window)`, `neues_fenster(Some(&ort))`.

### 3.5 Registrierung (`app::Register`, `main.rs`)

```rust
fn erweiterungen(register: &mut app::Register) {
    let _ = register;
    // Welle B2: Aufrufe unter dieser Zeile.
    //   z. B. symbolansicht::registrieren(register);
    // Welle B3: Aufrufe unter dieser Zeile.
    //   z. B. operationen::registrieren(register);
}
```

| Haken | Wofür | Wer |
|---|---|---|
| `register.ansicht(art, \|\| -> Rc<dyn Ansicht>)` | Fabrik je `Ansichtsart` (neue Instanz je Fenster) | B2 |
| `register.kontextmenue(\|fenster, ziel, widget, x, y\|)` | Kontextmenü der Ansicht (ein Modul) | B2 |
| `register.neue_ansicht(\|fenster, ansicht\|)` | je Ansicht und Fenster (Ziehen/Ablegen auf `ansicht.widget()` mit `eintrag_bei`) | B3 (B2 bei Bedarf) |
| `register.fenster(\|fenster\|)` | je neuem Fenster: `win.`-Aktionen hinzufügen | B3 |
| `register.ablage(\|fenster, widget, zielfinder\|)` | macht Pfadknöpfe und Orte der Seitenleiste zu Ablagezielen; `zielfinder(x, y)` liefert den Ordner | B3 |
| `register.anwendung(\|app\|)` | einmal beim Start (z. B. `app.queue().subscribe` für Fortschritt, Konflikte, Fehler) | B3 |

In `main.rs` stehen die Einfügestellen für B2 und B3 je unter einer
eigenen Kommentarzeile mit einer Leerzeile dazwischen (`mod`-Zeilen und
Aufrufe); so bleiben die Merges konfliktfrei.

### 3.6 Aktionen (`win.`/`app.`) und Kürzel

Die Menüs werden vor jedem Öffnen neu gebaut: Ein Eintrag ist nur wählbar,
wenn seine Aktion **existiert und aktiviert** ist; wer eine Aktion
anlegt, setzt `set_enabled` passend (Auswahl, Ort, Zwischenablage).

| Aktion | Zweck | Wer |
|---|---|---|
| `app.neues-fenster` (Strg+N), `app.alle-schliessen` (Umschalt+Strg+W), `win.schliessen` (Strg+W) | Fenster | B1 |
| `win.oeffnen` (Strg+O), `win.oeffnen-neues-fenster`, `win.oeffnen-mit` (s: Desktop-ID), `win.eigenschaften` (Alt+Eingabe) | Öffnen | B1 |
| `win.alles-auswaehlen` (Strg+A), `win.auswahl-umkehren`, `win.umbenennen` (F2), `win.umbenennen-ganz` (Umschalt+F2), `win.einstellungen` | Bearbeiten | B1 |
| `win.stopp` (Esc), `win.aktualisieren` (Strg+R, F5), `win.werkzeugleiste`, `win.seitenleiste` (F9), `win.adressleiste`, `win.statusleiste`, `win.verborgene` (Strg+H), `win.umgekehrt` (b), `win.anordnen` (s: `name`/`groesse`/`typ`/`datum`), `win.ansicht` (s: `symbole`/`liste`; Strg+1/2), `win.vergroessern` (Strg++), `win.verkleinern` (Strg+-), `win.normale-groesse` (Strg+0) | Ansicht | B1 |
| `win.nach-oben` (Alt+↑), `win.zurueck` (Alt+←), `win.vor` (Alt+→), `win.persoenlich` (Alt+Pos1), `win.rechner`, `win.papierkorb-zeigen`, `win.ort` (Strg+L), `win.gehe-zu` (s: URI) | Gehe zu | B1 |
| `win.lesezeichen-hinzufuegen` (Strg+D), `win.hilfe` (F1), `win.fehler-melden`, `win.info` | Lesezeichen, Hilfe | B1 |
| `win.ausschneiden` (Strg+X), `win.kopieren` (Strg+C), `win.einfuegen` (Strg+V), `win.in-ordner-einfuegen` | Zwischenablage | B3 |
| `win.papierkorb` (Entf), `win.loeschen` (Umschalt+Entf, Nachfrage), `win.wiederherstellen`, `win.papierkorb-leeren` (Nachfrage) | Papierkorb | B3 |
| `win.ordner-anlegen` (Umschalt+Strg+N), `win.dokument-anlegen` (s: URI der Vorlage, `""` = leere Datei) | Anlegen | B3 |

Für Kontextmenüs (B2): `menue::dokument_anlegen_menue(&fenster)` und
`menue::oeffnen_mit_menue(&auswahl)` liefern die Untermenüs,
`menue::eintrag(text, aktion, ziel, kuerzel, icon)` einen Eintrag (der
Unterstrich der Zugriffstaste wird entfernt). Kontextmenü wie im
Desktop: ein unsichtbarer Anker-`MenuButton` mit `classic_menu::attach`,
Modell neu füllen, `set_pointing_to`, `popup()` (Muster
`seitenleiste.rs`). „Eigenschaften“ ist `win.eigenschaften`, „Öffnen“
`win.oeffnen`, „Umbenennen …“ `win.umbenennen`.

### 3.7 Übersetzungen (während der Welle; seit dem Zusammenbau gibt es nur `po/de.po`)

msgid englisch, Wortlaut der Menüs nach B §3.3. **B2 und B3 tragen ihr
Deutsch in eigene Teilkataloge** `po/de.ansichten.po` bzw.
`po/de.operationen.po` ein (nur `msgid`/`msgstr`-Blöcke) und ändern
`po/de.po` und `po/querencia-dateien.pot` nicht; `werkzeug/i18n.py`
(`mo`, `pruefen`) liest alle zusammen, bei Teilkatalogen ist eine veraltete
`.pot` nur ein Hinweis. Bei der Integration übernimmt `i18n.py update`
die Teilkataloge in `po/de.po`, danach entfallen sie. Tabellentexte
markiert `N_("…")` (Schlüsselwort des Werkzeugs).

## 4. Einstellungen (`konfig.rs`, `einstellungen.rs`)

`dateien.ini` mit den Abschnitten `[Ansichten]`, `[Verhalten]`,
`[Anzeige]`, `[Listenspalten]`, `[Vorschau]`, `[Fenster]`,
`[Migration]`; Vorgaben nach B §3.7 (Doppelklick, Symbolansicht 100 %,
Liste 50 %, Name aufsteigend, Ordner zuerst, verborgene aus, Nachfrage
beim Leeren und Löschen, „Löschen“ ausgeblendet). Jede Änderung wird
sofort atomar geschrieben und erreicht alle Fenster
(`Konfig::bei_aenderung`). Der erste Start ohne `dateien.ini` übernimmt
einmal sechs Werte aus `~/.config/querencia-files/settings.toml` (M6:
`show_hidden`, `browser_mode` mit `columns` → Liste, `sort_key`,
`folders_first`, beide `*_folder_clicks` = 1 → Einfacher Klick,
`sidebar_expanded`), Marke `AusFiles=1`; die Files-Datei wird nur
gelesen. Der Dialog hat fünf Reiter (Ansichten, Verhalten, Anzeige,
Listenspalten, Vorschau); Anzeige, Listenspalten und Vorschau sind
gespeicherte Beta-Werte, die Ansichten über `einstellungen_anwenden`
nutzen können.

## 5. Prüfung

- `cargo test -p querencia-dateien`: Regeln (Sortierung, Zoom, Status,
  Knopfpfad, Tippsuche), Einstellungen und Migration, Lesezeichen,
  Verlauf, Ortseingabe und Tab-Vervollständigung, Menütabellen und
  Kürzel, Info/Versionsdatei, Katalog vollständig.
- GTK-Tests (`#[ignore]`, brauchen eine Anzeige) nur isoliert:
  `bash crates/querencia-dateien/werkzeug/gui-tests.sh` (Prüfrahmen
  Stufe L mit headless labwc; die Tests verweigern
  `XDG_RUNTIME_DIR=/run/user/*`). Darin der Ablauf des Ordnermodells
  (Lesen, Sortieren, Filtern, Auswahl über Umbenennen hinweg, Erscheinen,
  Ändern, Löschen, Neu laden, Ordner weg, Lesefehler).
- Übersetzungen: im Crate-Ordner `python3 -I werkzeug/i18n.py pruefen`.
- **Isolierte GUI-Läufe** (`crates/querencia-dateien/werkzeug/pruefung/`):
  `bauen-eingabe.sh` baut `qeingabe`/`qausgabe` (verweigern jede Sitzung
  außerhalb des Prüfrahmens: `XDG_RUNTIME_DIR=/tmp/qd-*` und `QD_HUELLE`);
  `lauf.sh NAME szenarien/X.sh` legt ein Profil mit Testbaum
  (`testbaum.py`: XDG-Ordner, Umlaute, lange Namen, versteckte Dateien,
  Verweis, ausführbare Textdatei, 200 Dateien, 50 MB, 12 Ebenen, Vorlagen,
  Attrappen-Programme mit `mimeapps.list`), Thema Querencia-Klassisch,
  Symbolthema und den drei Katalogen an und startet das Szenario in
  `werkzeug/dateien-pruefrahmen/huelle.sh` (Stufe L, headless labwc,
  Fenster per labwc-Regel oben links). Umgebung: `LAUF_GROESSE`,
  `LAUF_DUNKEL`, `LAUF_SPRACHE`, `LAUF_FILES_TOML` (M6),
  `LAUF_ATTRAPPEN` (Laufwerks-Attrappen). Hilfen im Szenario
  (`innen.sh`): `starte`, `q`, `taste`, `menue`, `warte_log`, `foto`,
  `fall`. Ergebnisse unter `~/projects/dateien-arbeit/laeufe/NAME/`
  (`ergebnis.txt`, `programm.log`, `fotos/`). Szenarien: `menues`,
  `navigation`, `dialoge`, `oeffnen`, `einstellungen`, `laufwerke`, `start`.

## 6. Grenzen (Stand 0.1.0)

- Die klassischen Menüs zeigen keine Zugriffstasten in Einträgen und keine
  Haken; Haken und Auswahlpunkte kommen als Symbol
  (`object-select-symbolic`, `media-record-symbolic`), Alt+Buchstabe
  öffnet nur die Menüs der Leiste (GTK-Mnemonik der Knöpfe).
- Seitenleiste nur „Orte“ (Baum: Beta), Reiter, Suche, Netzwerk,
  Kompaktansicht, „Mit anderer Anwendung öffnen …“, Menühinweise in der
  Statusleiste: Beta.
- Ohne GVfs (Prüfrahmen Stufe L) gibt es `trash:///` und `computer:///`
  nicht; der Papierkorb ist dann `$XDG_DATA_HOME/Trash/files` (Menü
  „Wiederherstellen“/„Papierkorb leeren …“ erkennen ihn, die Leiste nicht).
- Aushängen und die Frage „noch belegt“ brauchen udisks/GVfs mit echten
  Geräten (Stufe H); im Prüfrahmen sind nur Öffnen und die Fehlermeldungen
  gemessen.
- Das Fortschrittsfenster nimmt beim Erscheinen die Tastatur (wie im
  Desktop 0.3.0).
