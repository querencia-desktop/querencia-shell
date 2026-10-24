# Querencia Dateien – Ansichten: Bau und Prüfprotokoll (Welle B2)

**Stand:** 23.10.2026 · **Worker:** dateien-ansichten (B2) · **Zweig:** `dateien-ansichten`
(ab main `f57ce46`, `dateien/geruest-1` und Zweig `dateien` bis `dc9719e` gemergt) · **Tag:** `dateien/ansichten-1` ·
**Bau der Belege:** `target/release/querencia-dateien` sha256 `2e078d6bbc988a859802490fce3b869ce4f67313e80e35bcfee6bcf768193fc0`

Die Symbol- und die Listenansicht von Querencia Dateien 0.1 laufen gegen das Trait
`Ansicht` des Gerüsts (`docs/dateien.md` §3). Umbenennen im Platz und die Kontextmenüs
gehören dazu. Grundlage:  §1.5, §1.7, §3.7, §5.1,
 §1, §9, der Prototyp `dateien/prototyp-2`.

## 1. Was gebaut ist

| Datei | Inhalt |
|---|---|
| `symbolansicht.rs` | `GridView`: Zoom 50/66/100/150/200 % (24/32/48/72/96 px), Name unter dem Symbol auf höchstens drei Zeilen (das eine gewählte Objekt ganz), Trennung ohne Bindestriche, Zusatzzeilen ab 100 % und 150 % nach `dateien.ini` (`[Anzeige] Zusatz`, `ZusatzGross`), `max_columns` aus der Breite der Ansicht |
| `listenansicht.rs` | `ColumnView`: Name, Größe, Typ, Änderungsdatum; Zugriffsrechte, Eigentümer, Gruppe nach `[Listenspalten]`; Klick auf einen Spaltenkopf sortiert über die Fensteraktionen `win.anordnen`/`win.umgekehrt` (das Modell sortiert, die Spalten zeigen nur den Pfeil); Zoom als Symbolgröße und damit Zeilenhöhe; Spaltenköpfe ohne Umstellen |
| `ansichtskern.rs` | was beide teilen: Tastatur (Rücktaste, Eingabe öffnet die Auswahl, Menütaste, Umschalt+F10, „/“ und „~“), Tippsuche, Fokus-Verfolgung und Fokus nach dem Umordnen, Klick ins Leere, mittlere Taste, Umschalt+Doppelklick, langsamer zweiter Klick, Strg+Mausrad, eigenes Umschalt+Gummiband, Messzeilen; `Modul<D>` als `impl Ansicht`; `STIL` (nur benannte Themenfarben) |
| `ansichtsregeln.rs` | Regeln ohne Anzeige (Tests): Zoomstufen und Zellbreite, Spaltenzahl, Zusatzzeilen, Tippsuche, Spalten und Sortierklick, `ls`-Rechte, Größen- und Datumstext |
| `tippsuche.rs` | Kästchen unten rechts, 5 s, rot ohne Treffer |
| `umbenennen.rs` | Markierung ohne Endung (`namen::selected_chars`), Prüfung mit `namen::check_file_name` und „schon vorhanden“, Mietvertrag am Objekt (Feld wandert mit recycelten Zellen), Felder: mehrzeiliges `TextView` unter dem Symbol, `gtk::Text` in der Liste, Fehlermeldung nach GNOME 2 |
| `kontextmenue.rs` | Menüs nach B §1.7 Stufe 0.1 (Datei, Ordner, Mehrfachauswahl, Papierkorb-Objekt, Hintergrund), Texte der Menüleiste des Gerüsts, Anker in der Ansicht, Register-Haken |
| `po/de.ansichten.po` | deutscher Teilkatalog der neuen Texte (Regel `docs/dateien.md` §3.7) |
| `werkzeug/ansichten/` | isolierte Prüfung in der Hülle des Prüfrahmens (siehe §6) |

Registrierung in `main.rs` (`erweiterungen`): `symbolansicht::registrieren`,
`listenansicht::registrieren`, `kontextmenue::registrieren`.

## 2. Prüfumgebung

- `werkzeug/dateien-pruefrahmen/huelle.sh --labwc L` (QD-142): `env -i`, bwrap mit eigenem
  PID- und Netz-Namensraum, privater Session-Bus, Attrappen-System-Bus, echtes HOME und
  `/run` unsichtbar, headless labwc 0.9.8, `GIO_USE_VFS=local`. Restprüfung nach jedem
  Lauf: keine Reste in allen Läufen (`huelle.txt` je Lauf).
- Profil mit Kopie der Themen Querencia-Klassisch und Querencia, Portal-Ersatz (Werte aus
  den gsettings der Sitzung, nur gelesen), Testbaum aus dem Prototyp
  (`beispielordner.py`: 32 Einträge mit Umlauten, langen Namen, versteckten Dateien,
  Starter, Verknüpfungen; Ordner „10000 Einträge“ mit 10.000 Dateien und 20 Ordnern),
  Vorlage `~/Vorlagen/Brief.odt`, Prüfprogramm für `text/plain` über `mimeapps.list`.
- Eingaben über `qeingabe` (virtuelle Tastatur und Maus), Fotos über `grim`; beide
  verweigern alles außerhalb der Hülle (`/tmp/qd-*`, `QD_HUELLE`, `.huelle/konfig`).
- GTK 4.22.4, labwc 0.9.8. Dunkel: `gtk-application-prefer-dark-theme=1` in
  `settings.ini`, weil GTK in der Hülle das Farbschema des Portal-Ersatzes nicht übernimmt
  (gemessen: Einstellung 0 trotz `color-scheme 1`; im Prototyp-Rahmen ging es). Der Weg ist
  derselbe, den `farbschema::dunkel` ohne Portal nimmt.

## 3. Ergebnisse

| Lauf | Größe, Schema | Ergebnis | Ordner |
|---|---|---|---|
| `pruefung-1920-hell` | 1920×1080 hell | 80 PASS, 0 FAIL, Hülle ohne Reste | `nachweis/ansichten/laeufe/pruefung-1920-hell/` |
| `pruefung-1024-dunkel` | 1024×768 dunkel | 80 PASS, 0 FAIL, Hülle ohne Reste | `nachweis/ansichten/laeufe/pruefung-1024-dunkel/` |
| `messung-1920-hell` | 1920×1080 hell | 38 PASS, 0 FAIL, Hülle ohne Reste | `nachweis/ansichten/laeufe/messung-1920-hell/` |
| `dauer-1920-hell` | 1920×1080 hell, 5 min | 4 PASS, 0 FAIL, Hülle ohne Reste | `nachweis/ansichten/laeufe/dauer-1920-hell/` |
| `fotos-1920x1080-hell` | 1920×1080 hell | 3 PASS, 0 FAIL, Hülle ohne Reste | `nachweis/ansichten/laeufe/fotos-1920x1080-hell/` |
| `fotos-1920x1080-dunkel` | 1920×1080 dunkel | 3 PASS, 0 FAIL, Hülle ohne Reste | `nachweis/ansichten/laeufe/fotos-1920x1080-dunkel/` |
| `fotos-1024x768-hell` | 1024×768 hell | 3 PASS, 0 FAIL, Hülle ohne Reste | `nachweis/ansichten/laeufe/fotos-1024x768-hell/` |
| `fotos-1024x768-dunkel` | 1024×768 dunkel | 3 PASS, 0 FAIL, Hülle ohne Reste | `nachweis/ansichten/laeufe/fotos-1024x768-dunkel/` |

### 3.1 Fälle (Ablauf `pruefung`)

Bezug B §5.1: D10 (A07), D11 (A15), D12 (A08, A15, A17), D16 (A12), D20 (A09); D18 (A13) braucht
die Aktion `win.ordner-anlegen` von B3 und ist hier nicht geprüft. Dazu die Bedienung
der Ansichten aus dem Auftrag (Tastatur, Tippsuche, Gummiband, Maus, Umbenennen).

| Nr. | Fall | pruefung-1920-hell | pruefung-1024-dunkel |
|---|---|---|---|
| A01 | Start Symbolansicht 100 %: Ordner zuerst, „10000 Einträge“ vorn | PASS | PASS |
| A02 | Pfeil rechts zweimal: Bilder | PASS | PASS |
| A02 | Pfeil runter: Objekt der Zeile darunter | PASS | PASS |
| A02 | Ende: letztes Objekt | PASS | PASS |
| A02 | Pos1: erstes Objekt | PASS | PASS |
| A02 | Pfeiltasten wählen das Objekt mit | PASS | PASS |
| A03 | Tippen „re“ springt zu „Rechnungen 2026“ und wählt es | PASS | PASS |
| A03 | „red“ ohne Treffer: Fokus bleibt | PASS | PASS |
| A03 | Kästchen rot ohne Treffer | PASS | PASS |
| A03 | neu „da“ → datei1.txt | PASS | PASS |
| A03 | Pfeil runter in der Suche → datei2.txt | PASS | PASS |
| A03 | noch einmal → datei10.txt | PASS | PASS |
| A03 | Pfeil hoch → datei2.txt | PASS | PASS |
| A04 | Eingabe auf Ordner „Ablage“: geöffnet | PASS | PASS |
| A04 | Rücktaste: Elternordner, „Ablage“ gewählt | PASS | PASS |
| A05 | Klick wählt ein Objekt | PASS | PASS |
| A05 | Umschalt+Klick wählt den Bereich (4) | PASS | PASS |
| A05 | Strg+Klick nimmt eines dazu (5) | PASS | PASS |
| A05 | Strg+Klick noch einmal nimmt es weg (4) | PASS | PASS |
| A05 | Klick ins Leere hebt die Auswahl auf | PASS | PASS |
| A05 | Umschalt+Pfeil erweitert die Auswahl (2) | PASS | PASS |
| A05 | Eingabe mit zwei gewählten Textdateien öffnet beide (Prüfprogramm) | PASS | PASS |
| A06 | Gummiband wählt die überstrichenen Objekte | PASS | PASS |
| A06 | Strg+Gummiband ändert die Auswahl, ohne sie zu ersetzen | PASS | PASS |
| A06 | Umschalt+Gummiband erweitert die Auswahl | PASS | PASS |
| A06 | Umschalt+Gummiband: eigenes Band (GTK startet mit Umschalt keines) | PASS | PASS |
| A06 | Strg+A wählt alles (28 sichtbare Objekte) | PASS | PASS |
| A07 | Strg++: 150 % (72 px) | PASS | PASS |
| A07 | Strg++: 200 % (96 px) | PASS | PASS |
| A07 | Strg++ an der Grenze: bleibt 200 % | PASS | PASS |
| A07 | Strg+0: 100 % | PASS | PASS |
| A07 | Strg+- zweimal: 50 % (24 px) | PASS | PASS |
| A07 | Strg+Mausrad hoch: 66 % | PASS | PASS |
| A07 | Fokus bleibt beim Zoomen auf „bericht.final.odt“ | PASS | PASS |
| A08 | Nach Größe: Fokus bleibt auf „bericht.final.odt“ | PASS | PASS |
| A08 | Nach Größe: nach den acht Ordnern die größte Datei (film.mp4, 50 MB) | PASS | PASS |
| A08 | Umgekehrt: Ordner bleiben vorn, in umgekehrter Folge („Verknüpfung zu Dokumente“) | PASS | PASS |
| A08 | zurück nach Name: „10000 Einträge“ vorn | PASS | PASS |
| A09 | F2 auf bericht.final.odt: „bericht.final“ markiert (13 Zeichen) | PASS | PASS |
| A09 | Esc bricht ab | PASS | PASS |
| A09 | Datei unverändert nach Esc | PASS | PASS |
| A09 | Umschalt+F2: ganzer Name markiert (17 Zeichen) | PASS | PASS |
| A09 | Name mit „/“ abgelehnt mit Hinweis | PASS | PASS |
| A09 | leerer Name abgelehnt | PASS | PASS |
| A09 | Umbenennen „bericht.final.odt“ → „bericht.neu.odt“ | PASS | PASS |
| A09 | Fokus auf dem umbenannten Objekt | PASS | PASS |
| A09 | vorhandener Name abgelehnt | PASS | PASS |
| A10 | langsamer Klick auf den Namen des gewählten Objekts: Umbenennen | PASS | PASS |
| A10 | Doppelklick öffnet, ohne Umbenennen | PASS | PASS |
| A11 | mittlere Taste auf Ordner: neues Fenster | PASS | PASS |
| A11 | „/“ öffnet das Ortsfeld | PASS | PASS |
| A12 | Datei: Öffnen mit ▸, Ausschneiden, Kopieren, Umbenennen …, Papierkorb, Eigenschaften | PASS | PASS |
| A12 | Ordner: Öffnen, In neuem Fenster öffnen, In Ordner einfügen | PASS | PASS |
| A12 | Mehrfachauswahl: „Umbenennen …“ ausgegraut | PASS | PASS |
| A12 | Mehrfachauswahl: Auswahl bleibt (2) | PASS | PASS |
| A12 | Hintergrund: Ordner anlegen, Dokument anlegen ▸, Objekte anordnen ▸, Einfügen, Eigenschaften | PASS | PASS |
| A12 | Rechtsklick ins Leere hebt die Auswahl auf | PASS | PASS |
| A12 | Umschalt+F10 öffnet das Menü der Auswahl | PASS | PASS |
| A14 | Tippsuche mit Leerzeichen und Ziffern: „Foto 00003.txt“ | PASS | PASS |
| A14 | Feld kommt nach dem Scrollen zurück, Text bleibt | PASS | PASS |
| A15 | Strg+2: Listenansicht, Fokus bleibt auf „10000 Einträge“ | PASS | PASS |
| A15 | Klick auf „Größe“: nach Größe, aufsteigend | PASS | PASS |
| A15 | zweiter Klick: umgekehrt | PASS | PASS |
| A15 | Menü „Nach Name“: Pfeil folgt (kein Spaltenkopf-Ereignis) | PASS | PASS |
| A15 | Liste: Pfeil runter zweimal → Bilder | PASS | PASS |
| A15 | Liste: Tippsuche „bild.“ → bild.png | PASS | PASS |
| A15 | Liste: F2 auf bild.png markiert „bild“ | PASS | PASS |
| A15 | Liste: umbenannt in grafik.png | PASS | PASS |
| A15 | Liste: Strg++ 66 % (32 px) | PASS | PASS |
| A15 | Liste: Klick ins Leere hebt die Auswahl auf | PASS | – |
| A15 | Liste: Rechtsklick ins Leere zeigt das Hintergrundmenü | PASS | – |
| A15 | Strg+1 zurück zur Symbolansicht | PASS | PASS |
| A17 | Liste nach Sortierung in der Symbolansicht angelegt: Pfeil auf „Größe“ | PASS | PASS |
| A16 | Zoom 150 % aus dateien.ini | PASS | PASS |
| A16 | Liste mit Zugriffsrechten, Eigentümer, Gruppe | PASS | PASS |
| A15 | Liste: Strg+Klick nimmt die Zeile aus der Auswahl | – | PASS |
| A15 | Liste: Umschalt+F10 ohne Auswahl zeigt das Hintergrundmenü | – | PASS |

### 3.2 Messwerte (Ablauf `messung`, 10.020 Einträge, 1920×1080 hell)

Öffnen: „erstes Bild“ und „vollständig gezeigt“ vom Zustand `Laedt` des Ordnermodells bis
nach dem Zeichnen des Bildes (GDK `after-paint`). Umsortieren: von `items-changed` des
Auswahlmodells (vor dem Neubau der Zellen) bis die Zellen neu sind und bis nach dem
Zeichnen. Ausgelöst über `win.anordnen` (wie das Menü).

| Ansicht | Vorgang | n | Wert A (min–max) | Wert B (min–max), Median |
|---|---|---|---|---|
| liste | oeffnen | 3 | erstes Bild 97–101 ms | vollständig gezeigt 402–439 ms, Median 415 ms |
| liste | umsortieren | 9 | Zellen neu 10–15 ms | Bild 33–42 ms, Median 35 ms |
| symbole | oeffnen | 3 | erstes Bild 126–133 ms | vollständig gezeigt 680–731 ms, Median 710 ms |
| symbole | umsortieren | 9 | Zellen neu 37–54 ms | Bild 100–130 ms, Median 106 ms |

VmRSS nach dem Öffnen (symbole): 97112 KiB

VmRSS nach dem Öffnen (liste): 96468 KiB

Grenzen aus dem Auftrag: Öffnen < 1 s, Umsortieren < 0,5 s – erfüllt. Zum Vergleich der
Prototyp: Öffnen 0,88–0,92 s, Umsortieren 0,27 s (ohne Zeichnen).

### 3.3 Speicher (Ablauf `dauer`, 5 min Scrollen in „10000 Einträge“)

- D01 Start: VmRSS 97500 KiB
- D01 Prozess läuft nach 5 min Scrollen (69 Runden)
- D01 Ende: VmRSS 108648 KiB, Spitze (VmHWM) 108648 KiB, Zuwachs 11148 KiB
- D01 Zuwachs ab der ersten Minute: 348 KiB

- Messpunkte (s; VmRSS KiB): 0: 97500, 30: 108300, 60: 108296, 90: 108396, 120: 108380, 151: 108380, 184: 108460, 210: 108472, 240: 108472, 270: 108472, 300: 108648, 301: 108648

### 3.4 Fotos

Außerhalb des Repos unter  (mit `SHA256SUMS`),
mit labwc-Rahmen (Ablauf
`fotos`) in 1024×768 und 1920×1080, hell und dunkel: f01 Symbolansicht, f02 Kontextmenü
Datei, f03 Kontextmenü Hintergrund, f04 Umbenennen (Endung nicht markiert), f05
Tippsuche, f06 Gummiband, f07 Zoom 200 %, f08 Listenansicht nach Größe. Die Prüfläufe
haben weitere Fotos ohne Rahmen (a01…a16).

### 3.5 Unit-Tests

`cargo test -p querencia-dateien`: Zoomstufen und Zellbreiten, Spaltenzahl, Zusatzzeilen,
Tippsuche, Spaltenliste und Sortierklick, `ls`-Rechte, Größen-/Datumstext
(`ansichtsregeln`); Markierung ohne Endung, Ablehnung leer/„/“/„..“/vorhanden, eingefügte
Zeilenumbrüche (`umbenennen`); Menüs Datei, Ordner, Mehrfachauswahl, Hintergrund mit
Vorlagen und Sortierhaken, Papierkorb, Datei ohne weitere Programme (`kontextmenue`).
Workspace: `cargo fmt --all --check`, `cargo build`, `cargo test`, `cargo clippy
--all-targets -- -D warnings` grün (`nachweis/ansichten/workspace-pruefung.txt`).

## 4. Befunde und Grenzen

1. **Umschalt+Gummiband:** GTK 4.22 startet mit gedrückter Umschalttaste kein Gummiband
   (gemessen, Auswahl unverändert). Die Ansichten haben dafür ein eigenes Band auf freier
   Fläche; es nimmt die Objekte dazu, deren Zellen es berührt (nur sichtbare Zellen).
2. **Strg+Gummiband fügt hinzu** (GTK), Nautilus 2.30 kehrte die überstrichenen um.
   Gewählt wird erst beim Loslassen; die ganze Zelle ist Trefferfläche (Prototyp §5 B).
3. **Umbenenn-Feld unter dem Symbol** war im GridView 0 px hoch (`TextView` meldet seine
   Höhe erst nach der Validierung); es beginnt jetzt mit der Höhe der Beschriftung.
4. **Gerüst (behoben von B1):** In `geruest-1` hielt die App ihre Fenster nur schwach; das
   Ordnermodell fiel gleich nach dem Bau weg („Wird geladen …“), in der isolierten Prüfung
   gefunden. B1 korrigierte es in `e9398c4` (Zweig `dateien`, hier gemergt).
5. **Eigentümer und Gruppe** liest das Gerüst seit `dc9719e` mit (`owner::user`,
   `owner::group`); der Spaltenpfeil folgt `Ordnermodell::bei_sortierung`.
6. **Spaltenpfeil (behoben, Fall A17):** Eine Listenansicht, die erst nach einer Sortierung
   in der Symbolansicht entsteht, zeigte den Pfeil auf „Name“ (Befund aus den Fotos); sie
   übernimmt jetzt beim Anlegen die Ordnung des Modells. Außerdem behielt GTK die vorige
   Spalte als Nebensortierung und zeichnete zwei Pfeile; der Sortierer wird vor dem Setzen
   geleert (Foto a17).
7. **Aktionen von B3** (Ausschneiden, Kopieren, Einfügen, Papierkorb, Ordner anlegen …)
   fehlen im Zweig; die Menüs zeigen sie gesperrt. D18 (Ordner anlegen und gleich
   umbenennen) erst nach dem Zusammenbau prüfbar.
8. **Nicht gebaut:** Kompaktansicht (Beta), „Mit anderer Anwendung öffnen …“ (B21, Beta),
   Sortieren nach Rechte/Eigentümer/Gruppe, Aufklappen von Ordnern in der Liste (B07),
   Ziehen und Ablegen selbst (B3). Dafür liefern die Ansichten `Ansicht::eintrag_bei` (nur
   Symbol oder Name trifft, nicht der Rand der Zelle) und `ablage_markieren` (Rahmen in der
   Auswahlfarbe um die Ordnerzelle, bleibt beim Recyceln richtig); beide ruft erst B3 auf,
   im Lauf hier nicht geprüft. Verblasste Symbole ausgeschnittener Objekte
   (`operationen::ist_ausgeschnitten`) kommen beim Zusammenbau dazu.
9. **Nicht geprüft:** echte Maus und Bildschirm (pottrauschen), Englisch, Skalierung ≠ 1,
   Netz- und Wechseldatenträger-Ordner, Starter umbenennen (Code wie im Desktop, ohne Lauf).

## 5. Abweichungen vom Prototyp

Zoom wie Nautilus 2.30 (66 % statt 75 %); Zoom, Sortierung, Ordnerwechsel und
Statusleiste gehören dem Gerüst; Messung nach dem Zeichnen statt nach dem Tick;
Kontextmenüs, Umschalt+Gummiband, Listenansicht und gemeinsamer Kern neu.

## 6. Werkzeuge

```bash
cd ~/projects/querencia-runtime-wt-dateien-ansichten
```

```bash
bash crates/querencia-dateien/werkzeug/ansichten/bauen.sh
```

```bash
bash crates/querencia-dateien/werkzeug/ansichten/bauen-eingabe.sh
```

```bash
LAUF_GROESSE=1024x768 LAUF_DUNKEL=1 bash crates/querencia-dateien/werkzeug/ansichten/lauf.sh pruefung-x pruefung
```

Abläufe: `erkunden`, `pruefung`, `messung`, `fotos`, `dauer` (`LAUF_MINUTEN`). Ergebnisse
unter `crates/querencia-dateien/nachweis/ansichten/laeufe/<tag>/` (`ergebnis.txt`,
`dateien-N.log` mit `QUERENCIA_DATEIEN_DEBUG=1`, `huelle.txt`), Fotos unter
 (`LAUF_FOTOS`). `tabelle.py` macht aus `ergebnis.txt` die Falltabelle.
