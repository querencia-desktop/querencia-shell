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

## 7. Welle C2: Miniaturen und Suche (`src/miniaturen/`, `src/suche/`)

Zweig `dateien-miniaturen-suche`, Tag `dateien/miniaturen-1`. M9 aus
 §4 (gleicher Cache wie Files) und B §1
„Suche“ (nur Namen ab dem gezeigten Ordner, keine Kriterien, keine
Volltextsuche, kein Index). Registrierung in `main.rs` im Block
„Welle C2“, Übersetzungen im Teilkatalog `po/de.miniaturen.po`.

### 7.1 Schnittstelle „Bildquelle je Eintrag“

Die einzige Verbindung zwischen den Ansichten und den Miniaturen; das
Trait `Ansicht` bleibt unverändert.

```rust
use crate::miniaturen::bildquelle;
// im bind-Handler einer Zelle statt symbole::set(&bild, &d.symbol):
bildquelle::binden(&bild, eintrag, bildquelle::Groesse::symbolansicht(px)); // Liste: Groesse::liste(px)
// im unbind-Handler:
bildquelle::loesen(&bild);
```

`binden` zeigt sofort die Miniatur aus dem Speicher oder das Themensymbol
(weiter über `querencia_oberflaeche::symbole`, QD-106) und setzt die
Miniatur, sobald ein Arbeitsfaden sie hat; `Groesse { symbol, vorschau }`
gibt die Symbolgröße und die lange Kante der Miniatur samt Rahmen
(Symbolansicht ein Drittel größer als das Symbol wie Nautilus 2.30:
24→32, 32→43, 48→64, 72→96, 96→128 px; Liste und Suchergebnisse so groß
wie das Symbol). `loesen` nimmt den Auftrag zurück, wenn keine andere
Zelle auf ihn wartet. Ohne registriertes Modul verhält sich `binden` wie
`symbole::set`. Genutzt von `symbolansicht.rs`, `listenansicht.rs` (je
zwei Zeilen) und den Suchergebnissen; die Kompaktansicht (C3) bindet
ebenso.

### 7.2 Miniaturen

| Datei | Inhalt |
|---|---|
| `miniaturen/mod.rs` | Regeln, Zustand im Hauptfaden (Bindungen, wartende Zellen, LRU 48 MB der fertigen Texturen), `bildquelle`, `registrieren` |
| `miniaturen/cache.rs` | Thumbnail Managing Standard 0.9: `$XDG_CACHE_HOME/thumbnails/{normal,large}/<MD5 der URI>.png`, `fail/querencia-dateien/`, PNG-`tEXt` lesen und schreiben (CRC selbst), Lesen ohne Symlinks/FIFOs mit Größengrenze (Prüfung nach Files, MIT-Vermerk), Schreiben atomar 0600 in Ordnern 0700 |
| `miniaturen/erzeuger.rs` | Erzeugen: GdkPixbuf, GTK-Lader, `*.thumbnailer` (`TryExec`, `Exec` mit `%u %i %o %s`, Zeitlimit 20 s, je Auftrag ein eigener Ordner 0700) |
| `miniaturen/sandkasten.rs` | fremde Thumbnailer nur in bwrap (Bedingung der Status-Session, Recherche C §2): `--ro-bind / /`, `--dev`, `--proc`, `/tmp`, `/run` und das echte HOME als leeres tmpfs (`/run`: keine Bus-, Wayland- oder GVfs-Sockets – ein Netz-Namensraum verdeckt Dateisystem-Sockets nicht), dann nur die Eingabe lesend und der Auftragsordner schreibbar an ihren echten Pfaden, `--clearenv`, `--unshare-all` (kein Netz), `--die-with-parent`, `--new-session`, Zeitlimit 20 s. Fehlt bwrap oder startet es keinen Sandkasten, ist kein Thumbnailer verfügbar (nie ohne Sandkasten; eine Logzeile je Lauf). GdkPixbuf/glycin im Prozess unverändert (glycin hat eine eigene Sandbox) |
| `miniaturen/warteschlange.rs` | Arbeitsfäden (Kerne/2, 2–4: höchstens so viele Aufträge zugleich), Warteschlange in Ankunftsfolge, Zurücknehmen (wartend: entfernt; laufend: Abbruch-Merker, ein Thumbnailer wird beendet), Auslieferung über `glib::idle_add_once` |
| `miniaturen/bild.rs` | Pixel: Verkleinern über Flächenmittel, Rahmen, Textur |

Regeln (Nautilus 2.30, Einstellungen → Vorschau): „Vorschaubilder
anzeigen“ Immer / Nur lokale Dateien (Vorgabe; `gio::File::is_native`) /
Nie, „Nur für Dateien bis“ 1–100 MB (Vorgabe 10 MB; größere Dateien
bekommen keine Miniatur, auch keine vorhandene). Ordner, Starter und
Dateien im Miniaturen-Cache selbst bekommen keine. Eine Änderung wirkt
sofort auf alle offenen Fenster.

Ablauf je Auftrag (Arbeitsfaden): Cache des Eimers lesen (für `normal`
auch ein gültiges `large`, wie Files es schreibt), sonst ein
Fehleintrag → keine Miniatur, sonst erzeugen und in den Eimer schreiben
(Fehlschlag → `fail/querencia-dateien/`, neuer Versuch erst nach einer
Änderung der Datei). Eimer `normal` bis 128 px Anzeige, darüber `large`.
Erzeugen: **GdkPixbuf zuerst** (mit glycin dekodiert ein Laderprozess
und skaliert beim Dekodieren), PNG/JPEG/TIFF bei einem Fehler über GTKs
eigene Lader, alle anderen Typen über einen installierten Thumbnailer im
Sandkasten (`sandkasten.rs`); ein Thumbnailer prüft `try_wait` alle 5 ms,
weil er auch beim Lösen der Zelle (Merker aus dem Hauptfaden) enden muss
und std kein Warten mit Zeitlimit kennt.
Auf der VM gibt es nur die glycin-Thumbnailer für Bilder; PDF bekommen
eine Miniatur erst, wenn ein PDF-Thumbnailer (Poppler, Querencia
Dokumente) installiert ist. Gezeigt wird verkleinert auf die Anzeige;
undurchsichtige Bilder ab 48 px bekommen den Rahmen wie 2.30 (1 px Rand
35 % Schwarz, Schatten rechts unten), Bilder mit Transparenz und kleine
Symbole nicht. Ein Ordnerwechsel nimmt die offenen Aufträge zurück
(Zellen werden gelöst; ein laufender Thumbnailer wird beendet).

Protokoll mit `QUERENCIA_DATEIEN_DEBUG=1`: „Miniaturen: NurLokal, bis 10
MB, … Thumbnailer, … Fäden“, „Miniaturen fertig: N aus dem Cache, N
erzeugt, N fehlgeschlagen in N ms“, „Miniaturen nach Ordnerwechsel: N
zurückgenommen …“, „Miniaturen: Regel …“.

### 7.3 Suche

| Datei | Inhalt |
|---|---|
| `suche/mod.rs` | Suchleiste je Fenster (über der Ansicht im Ansichtsbereich), Aktion `win.suchen` (Zustand: Leiste offen; Strg+F, Knopf „Suchen“ der Werkzeugleiste, Gehe zu → „Suchen …“), Ablauf, Statusleiste, „Erweiterte Suche …“ |
| `suche/sucher.rs` | Suchfaden: `enumerate_children` in Breitensuche ab dem gezeigten Ordner, Vergleich nach 2.30 (NFD, klein, jedes Wort im Anzeigenamen), verborgene nach der Einstellung des Fensters (Strg+H), symbolische Verweise nie betreten (`NOFOLLOW_SYMLINKS`), `id::file` gegen doppelte Ordner (auch der Startordner), Grenzen 64 Ebenen und 50.000 Treffer (`Grenzen`, Statusleiste „(Grenze erreicht)“), Treffer in Blöcken (500 oder 150 ms) |
| `suche/ergebnisse.rs` | Ansicht „Suchergebnisse“: Spalten Name, Ort, Größe, Typ, Änderungsdatum (Klick sortiert), Doppelklick/Eingabe öffnet, Kontextmenü „Öffnen“, „Im Ordner anzeigen“ |

Eingabe startet die Suche; jede Änderung im Suchfeld und Esc halten eine
laufende Suche an (Treffer bleiben), ein zweites Esc, der Knopf, Strg+F oder jeder Ortswechsel
beendet sie und zeigt den Ordner wieder. Während der Suche heißt das
Fenster „Suchergebnisse“, die Statusleiste zählt die Treffer. Die Tasten,
die das Fenster für Dateien bindet (Entf, Strg+X/C/V, F2), wirken in den
Suchergebnissen nicht (sie gälten der Auswahl des verdeckten Ordners);
Strg+A wählt alle Treffer. „Erweiterte Suche …“ erscheint, wenn
`querencia-dateisuche` gefunden wird (PATH, `~/opt/querencia/bin`), und
startet es mit `--ordner` und `--name *Text*`. ↓ im Suchfeld führt in die
Treffer.

### 7.4 Prüfung und Messwerte

`crates/querencia-dateien/werkzeug/miniaturen/`: `material.sh`
(Testmaterial: 200 Fotos 1920×1080, Bildarten, kaputtes und zu großes
Bild, drei PDF, 10.092 Einträge für die Suche), `lauf.sh` (Prüfrahmen
Stufe L wie `../pruefung/lauf.sh`, dazu Material, PDF-Thumbnailer-
Stellvertreter `qd-pdf-miniatur` (in der Hülle ohne bwrap nicht
gestartet; `--versuche` für den Sandkasten-Test), Attrappe `querencia-dateisuche`,
`QUERENCIA_DATEIEN_DEBUG=1`), `cache-pruefen.py` (Cache gegen die
Spezifikation), Szenarien `pruefung` (26 Fälle M01–M11, S01–S15),
`messung`, `messung-ohne`, `plateau`, `fotos`. Belege unter
`crates/querencia-dateien/nachweis/miniaturen/`, Fotos (nie ins Repo)
unter .

Messung (Release, Prüfrahmen L, headless labwc mit Software-Rendering,
1920×1080, 16 Kerne; `nachweis/miniaturen/laeufe/messung-1920/messung.csv`,
Spannen aus fünf Läufen):

| Messung | Wert |
|---|---|
| 200 Miniaturen neu (JPEG 1920×1080, Symbolansicht 100 %, 4 Fäden) | 0,96–1,8 s (je nach Last der VM durch parallele Bauten) |
| 200 Miniaturen aus dem Cache (neuer Prozess) | 74–141 ms |
| dieselben im selben Prozess (Speicher) | sofort, kein Auftrag |
| Zoom 200 % (128 px aus `normal`) / Liste (24 px) | 52–123 ms / 82–105 ms |
| Ordnerwechsel 150 ms nach dem Lesen von 200 neuen Fotos | 320–400 Aufträge zurückgenommen, 0–66 erzeugt |
| Suche „rechnung“ über 10.092 Einträge (111 Ordner, 37 Treffer) | Faden 133–165 ms, Anzeige ≤ 185 ms |
| Suche „datei“ (9.942 Treffer) | Faden 169–198 ms, Anzeige ≤ 213 ms |
| VmRSS Start (Ordner ohne Bilder) | 230 MB (ohne Miniaturen auch 230/235 MB) |
| nach 200 neuen Miniaturen | VmRSS ±0, VmHWM +28 MB (vorübergehend) |
| nach 600 neuen Miniaturen (`plateau`) | +7–9 MB |
| GTK-Lader statt GdkPixbuf (Gegenversuch) | +54 MB nach 200, danach Plateau |
| nach der Suche mit 9.942 Treffern | +36 MB |

### 7.5 Grenzen (Stand dateien/miniaturen-1)

- Kein PDF-/Dokument-Thumbnailer auf der VM: geprüft ist der Weg mit dem
  Stellvertreter `qd-pdf-miniatur` im Sandkasten (Integrationstest); echte
  PDF-Miniaturen brauchen einen installierten Thumbnailer (QD-143).
- In der Prüfhülle erlaubt der Kern keinen verschachtelten
  Nutzer-Namensraum: glycin läuft dort ohne Sandbox („Glycin running
  without sandbox“), fremde Thumbnailer sind dort nicht verfügbar (M06,
  M11). Den Sandkasten belegt der Integrationstest
  `the_pdf_stand_in_runs_sandboxed_in_a_throwaway_home` außerhalb der Hülle
  mit `qd-pdf-miniatur --versuche` in einem Wegwerf-HOME (Schreiben nach
  `/var/tmp`, ins HOME und neben die Eingabe, Lesen einer Datei im HOME,
  Netz zu 127.0.0.1, Sitzungsvariablen, `/run/user`: alles scheitert;
  Gegenprobe ohne Sandkasten: alles gelingt) sowie der Test für Zeitlimit
  und Abbruch.
- Auch eine verdeckte Ansicht (Liste neben der Symbolansicht) bindet ihre
  Zellen und fragt Miniaturen an (Cache-Lesen, kein zweites Erzeugen).
- „Immer“ für entfernte Orte (sftp, smb) liest die Datei über GIO; im
  Prüfrahmen L nicht prüfbar (Stufe G mit `--sshd`: Folgepunkt).
- Suche ohne Kriterien und ohne Speichern (B §1: Rest über Querencia
  Dateisuche); keine Treffer-Aktualisierung bei Änderungen während der
  Anzeige; Kopieren/Ziehen aus den Suchergebnissen: Folgepunkt.
