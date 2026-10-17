# HMP-10: Desktop-Symbole verschieben und umbenennen

Baut auf HMP-09 (`hmp-09-arbeitsflaeche.md`) auf. Alles aus HMP-09 bleibt: Hintergrund, Symbole, Dateimonitor, Auswahl, Doppelklick, Kontextmenüs, „Ordner anlegen“, „Dokument anlegen“, „Nach Namen aufräumen“.

## Bedienung

| Aktion | Wirkung |
|---|---|
| Einzelklick | wählt aus |
| Doppelklick | öffnet (beim Loslassen des zweiten Klicks) |
| Linke Taste halten und ziehen | verschiebt das Symbol; ein durchscheinendes Abbild folgt dem Zeiger, das Original bleibt bis zum Loslassen stehen |
| Loslassen | übernimmt die Lage; ist sie belegt, rückt das Symbol bündig auf den nächsten freien Platz |
| F2 / „Umbenennen …“ im Symbolmenü | Beschriftung wird zum Eingabefeld (weiß, schwarzer Rahmen) |
| Enter / Klick daneben | übernimmt den neuen Namen |
| Escape | bricht ab |

- Ziehschwelle: GTKs `gtk-dnd-drag-threshold` (8 px). Darunter bleibt es ein Klick.
- Ein Drücken, das zum Ziehen wurde, zählt nie zu einem Doppelklick (Zähler `drags`; zusätzlich setzt GTK den Klickzähler bei Bewegung zurück).
- Während der Bearbeitung beendet ein Klick daneben nur die Bearbeitung: keine Auswahl, kein Ziehen, kein Öffnen.
- Verschoben wird nur das Symbol. Die Datei bleibt, wo sie ist (geprüft über Inode und Prüfsumme).
- Ziehen ist ein internes Ziehen in der Symbolebene, kein Wayland-Drag-and-drop: Ablegen auf Programme (z. B. Querencia Files) gibt es noch nicht, und damit auch kein versehentliches Verschieben einer Datei.

## Positionen (`layout.rs`)

- Jedes Symbol belegt eine Zelle von 100×88 px (Symbol 48 px und höchstens zwei Zeilen Beschriftung). Zellen überlappen nie.
- Erlaubt ist jede Lage in der Fläche zwischen den Panels mit 8 px Rand. Das Ablegen auf einem Panel oder jenseits des Bildschirmrands landet am Rand.
- Gespeichert wird je Achse **Kante und Abstand**:
  - Ein gezogenes Symbol gehört zur nächsten Kante. Das äußere Drittel gehört zur Kante, das mittlere zur Mitte.
  - Automatisch platzierte und aufgeräumte Symbole hängen an der linken oberen Ecke.
  - Folge: „Rechner rechts oben“ bleibt bei jeder Auflösung rechts oben; „Papierkorb rechts unten“ folgt dem unteren Panel.
- **Kontrollierter Reflow:** Ist die Fläche kleiner (Auflösung, Skalierung, Panelhöhe), wird nur die **Anzeige** angepasst:
  - erst Symbole, deren Platz passt;
  - dann die hereingeholten;
  - dann neue.
  - Ein belegter Platz führt zum nächsten freien.
  - Die gespeicherten Plätze ändern sich dabei nicht. Wird die Fläche wieder größer, kehrt jedes Symbol an seinen Platz zurück (Test 25: 1920×1080, 1024×768, 1280×720 mit Skalierung 2 und zurück).
- Neue Symbole (neue Datei, „Ordner anlegen“) kommen in die erste freie Zelle des Spaltenrasters (wie GNOME 2) und verdrängen niemanden.
- „Nach Namen aufräumen“: Spaltenraster in der GNOME-2-Reihenfolge (Rechner, Persönlicher Ordner, Papierkorb, Programme, dann nach Namen). Gespeichert wird nur die neue Lage, keine Datei wird berührt.
- Vorrang beim Reflow: zuletzt verschobenes Symbol zuerst (Reihenfolge der Zustandsdatei).

## Zustandsdatei (`store.rs`)

`$XDG_STATE_HOME/querencia/desktop-symbole` (Standard `~/.local/state/querencia/desktop-symbole`), Text, Tabulatoren:

```text
version	1
sonder	computer	rechts 8	oben 8
programm	org.example.Querencia.Terminal.desktop	mitte 0	mitte 0
ordner	Projektunterlagen	links 8	oben 96
datei	Brief.txt	links 108	oben 8
starter	Mein Browser.desktop	links 8	unten 8
name	programm	org.example.Querencia.Terminal.desktop	Mein Terminal
```

- **Arten:**
  - `sonder` (Rechner, Persönlicher Ordner, Papierkorb);
  - `programm` (eingebaute Querencia-Starter, nach Desktop-ID);
  - für Einträge des Arbeitsflächenordners, nach Dateiname: `starter` (`.desktop`), `datei`, `ordner`.
  - `starter`, `datei` und `ordner` teilen sich eine Identität. Eine Datei, die zum Ordner wurde, behält ihren Platz.
- `name`-Zeilen: eigene Namen für Symbole, deren Name keine eigene Datei ist.
- Atomar: temporäre Datei `…neu` im selben Ordner, `fsync`, `rename`.
- Tolerant:
  - Kaputte Zeilen werden übersprungen und protokolliert.
  - Fehlt ein Sonderobjekt oder Programm vorübergehend, bleibt sein Eintrag.
  - Gelöschte Dateien verlieren ihren Eintrag, aber nur, wenn der Ordner lesbar war.
- Eine Datei einer **neueren Version** wird gelesen, aber nie überschrieben.
- **Migration:** Beim ersten Start ohne Zustandsdatei wird die HMP-09-Reihenfolge (`desktop-order`) in Rasterplätze übernommen. `desktop-order` wird weiter geschrieben (nach Lage sortiert), damit ein Rückfall auf HMP-09 eine ähnliche Reihenfolge zeigt.
- Dateimonitor-Ereignisse:
  - Sie werden gebündelt (120 ms) und während Ziehen oder Umbenennen zurückgestellt.
  - Ein Refresh liest nur, welche Symbole es gibt. Die Plätze kommen immer aus der Zustandsdatei.
  - Mit `WATCH_MOVES` behält auch eine **außerhalb** umbenannte Datei (z. B. in Querencia Files) ihren Platz.

## Umbenennen (`rename.rs`)

| Objekt | Wirkung |
|---|---|
| Datei, Ordner | GIO `g_file_set_display_name` (verweigert vorhandene Namen); Platz und Eintrag gehen mit. Auswahl beim Start: Name ohne Endung (Dateien), alles (Ordner) |
| eigener Starter (reguläre, beschreibbare Datei in beschreibbarem Ordner) | nur die `Name`-Zeile, die der Desktop zeigt (`Name[de]` bei `LANGUAGE=de`, sonst `Name`); alle anderen Bytes bleiben. Schreiben über `g_file_replace_contents` mit Etag (kein Überschreiben fremder Änderungen), Rechte danach wiederhergestellt |
| fremder Starter (Link, nicht beschreibbar), eingebaute Querencia-Starter, Rechner, Persönlicher Ordner, Papierkorb | eigener Name in der Zustandsdatei; der ursprüngliche Name entfernt ihn wieder. Die Funktion hängt nie am Namen (GNOME 2 erlaubte das ebenso über gconf) |

**Fehler** (Meldungsfenster „„A“ konnte nicht in „B“ umbenannt werden.“ mit Erklärung):

- leerer Name;
- `/`;
- `.` und `..`;
- Name vorhanden;
- Datei inzwischen gelöscht;
- keine Berechtigung.

Ohne Schreibrecht im Ordner (`access::can-rename`) ist „Umbenennen …“ ausgegraut, F2 erklärt warum. Schreibgeschützte Dateien lassen sich umbenennen, wie unter POSIX und in Nautilus, und behalten ihre Rechte.

## Starter auf dem Arbeitsflächenordner

`.desktop`-Dateien im Arbeitsflächenordner sind jetzt Starter: Name und Symbol aus der Datei, Doppelklick startet die Anwendung. Vertrauenswürdig ist ein Starter mit Ausführungsrecht oder ein Link auf einen installierten Starter (`…/applications` der XDG-Datenordner). Wie in Nautilus 2.30 startet jeder andere Starter erst nach der Nachfrage „Nicht vertrauenswürdiger Anwendungsstarter“:

- „Abbrechen“;
- „Als vertrauenswürdig markieren“ (setzt `u+x`);
- „Trotzdem starten“.

## Prüfung

- Unit-Tests: `layout.rs`, `store.rs`, `rename.rs`, `icons.rs`.
- Isoliert: `tools/isolierte-gui-pruefung/pruefen-hmp10.sh <desktop> <panel> <profil>`
  - eigener labwc mit vier Arbeitsflächen, echtes Panel, echte Eingaben;
  - Treiber `hmp10-treiber.py`: Prüfungen 01–25, Bilder in `logs/bilder/`;
  - `HMP10_NUR=migration` prüft nur die Übernahme aus HMP-09.
- qeingabe kann jetzt `press`/`release` (Ziehen), `extent W H`, Umlaute und weitere Tasten; qausgabe setzt optional die Skalierung.
