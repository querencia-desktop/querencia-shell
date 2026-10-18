# HMP-10b: Symbolbeschriftungen und „Papierkorb leeren…“

Baut auf HMP-10a auf (`hmp-10a-raster-und-mehrfachauswahl.md`). Raster, Einrasten, Mehrfachauswahl, Auswahlrahmen, gemeinsames Verschieben, Speicherung und Kontextmenüs sind unverändert.

## Beschriftungen

**Ursache des Umbruchs „Persönliche / r Ordner“:**

- Die Kachel eines Symbols war 94 px breit (`CELL.0 − 6`). Davon gingen 2 × 1 px Kachelrand und 2 × 2 px Beschriftungsrand ab (CSS), für den Text blieben **88 px**.
- „Persönlicher“ ist in der Sitzungsschrift „Adwaita Sans 11“ (auf dieser VM DejaVu Sans 11) **89 px** breit.
- Bei `WrapMode::WordChar` bricht Pango ein Wort, das nicht in die Zeile passt, zwischen Zeichen um. Es fehlte ein Pixel.

**Korrektur:**

- Die Kachel nutzt die ganze Zellbreite (100 px), es bleiben 94 px für den Text.
- Zellen überlappen nie; jedes Raster hat zwischen den Spalten mindestens eine Zellbreite (≥ 100 px). Kacheln und Auswahlhintergründe bleiben also in ihrer Zelle.
- GtkFixed teilt die Mindestgröße zu. Lange Namen ohne Leerzeichen machen die Kachel deshalb nicht breiter, sie brechen innerhalb der 94 px um und enden nach zwei Zeilen mit „…“.

**Ergebnis (Sitzungsschrift):**

| Name | Zeilen |
|---|---|
| Persönlicher Ordner | Persönlicher \| Ordner |
| Querencia Terminal | Querencia \| Terminal |
| Übersicht für Jörg.txt | Übersicht \| für Jörg.txt |
| Projektunterlagen | Projektunter \| lagen (Wort breiter als 94 px) |
| Donaudampfschifffahrtsgesellschaft.txt | Donaudamp \| fschiff… (gekürzt) |

- Symbol und Beschriftung sitzen jetzt in der Zellmitte (x + 50 statt x + 47), also 3 px weiter rechts. Das Umbenennen-Feld war schon auf x + 50 zentriert.
- Prüfhilfe: Mit `QUERENCIA_DESKTOP_DEBUG=1` schreibt der Desktop je Symbol die tatsächlichen Zeilen sowie die Lage von Beschriftung, Symbol und Kachel ins Protokoll. Eine gekürzte letzte Zeile ist als „[gekürzt]“ markiert und enthält den ganzen Rest. Die Prüfung „Kachel größer als die Zelle“ misst jetzt die Außenmaße (`compute_bounds`, CSS-Rand eingeschlossen).

## Papierkorb leeren…

**Kontextmenü des Desktop-Papierkorbs:**

- „Papierkorb öffnen“
- —
- „Papierkorb leeren…“, nur aktiv, wenn `trash:///` Objekte enthält. Leer oder nicht erreichbar: ausgegraut, keine Abfrage.
- —
- „Umbenennen …“, wie bisher

**Sicherheitsabfrage:**

- Text: „Möchten Sie alle Dateien im Papierkorb endgültig löschen?“, dazu „Wenn Sie den Papierkorb leeren, werden alle darin befindlichen Objekte unwiderruflich gelöscht.“
- Schaltflächen „Abbrechen“ und „Papierkorb leeren“. Standard ist „Abbrechen“ (Return), Esc bricht ab.
- Gelöscht wird nur nach „Papierkorb leeren“.

**Löschen über GIO/GVfs:**

- `trash:///` wird ohne Verfolgen von Links aufgezählt; jedes Objekt der obersten Ebene wird mit `g_file_delete` gelöscht (asynchron, eins nach dem anderen).
- GVfs nimmt das Objekt aus dem Papierkorb und entfernt es samt Unterordnern, ohne Links zu folgen.
- Der Desktop löscht selbst keinen Pfad. Es werden alle Papierkorb-Orte erfasst, die GVfs kennt (Heimordner und `.Trash-<uid>` eingehängter Datenträger).

**Danach:**

- Symbole neu laden: Das Papierkorbsymbol fragt `trash::item-count` und zeigt leer oder voll.
- Fehler je Objekt (keine Berechtigung, nicht mehr vorhanden, Datenträger nicht erreichbar, sonst GVfs-Meldung) werden gesammelt. Gab es welche: Meldung „Der Papierkorb konnte nicht vollständig geleert werden.“ mit „n von m Objekten …“ und den Gründen.
- Nie eine Erfolgsmeldung, wenn etwas fehlte. Bei Erfolg gibt es, wie in GNOME 2, keine Meldung.

**Grenze:** Die Abfrage ist wie alle Desktop-Meldungen ein `GtkAlertDialog` ohne Elternfenster. Er erscheint ohne labwc-Titelleiste und in der Fensterliste unter dem Programmnamen.

## Prüfung

| Modus | Inhalt |
|---|---|
| `HMP10_NUR=beschriftung` | 18 Symbole mit kurzen, langen, umlauthaltigen Namen, mit und ohne Leerzeichen. Prüft: Zeilen aus dem Protokoll, kein zerrissenes Wort, das in die Zeile passt (Pango-Messung), Zentrierung, Lage in der Kachel, Pixel jenseits des Zellrands, Raster, Mehrfachauswahl, Verschieben |
| `HMP10_NUR=papierkorb` | eigener Papierkorb im Testprofil (`XDG_DATA_HOME`). Schutz: Er muss zu Beginn leer sein, und vor jedem Leeren muss er genau die eigenen Testobjekte enthalten, sonst BLOCKED. Der echte Papierkorb wird vorher und nachher verglichen. Prüft: leer, eine Datei, mehrere Dateien, Unterordner, Links nach draußen, Esc, Return, Abbrechen, Leeren, Teilfehler (Ordner ohne Schreibrecht), Symbol, Menü mehrfach öffnen |
