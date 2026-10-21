# Änderungen – querencia-desktop

## 0.2.2 – 2026-10-21

Leere Dateien öffnen sich wieder (Befund des Besitzers 21.10.2026 20:33: Doppelklick
auf ein leeres `leer.txt` öffnete den Editor nicht).

- GIO nennt eine leere Datei `application/x-zerosize`; dafür gibt es kein
  Programm. Bisher stand dann nur „nicht geöffnet: Es wurde keine Anwendung
  gefunden …“ im Protokoll.
- Hat der Inhaltstyp einer Datei kein Programm, entscheidet jetzt wie in
  Nautilus der Dateiname (`.txt` → `text/plain` → Standardanwendung, hier
  Querencia Editor). Das Protokoll nennt den Weg
  (`… application/x-zerosize ohne Programm, öffne als text/plain mit …`).
- Sagt auch der Name nichts (leere Datei ohne Endung), wird nichts gestartet
  und der Fehler bleibt im Protokoll, wie bisher.
- Alles andere unverändert: Dateien mit eigenem Programm, Ordner, Starter,
  Laufwerke.

## 0.2.1 – 2026-10-21

Zwei Punkte für Alpha 2 (Zweig `desktop-0.2.1`; Hintergründe 0.2, QD-093,
Entscheidung des Besitzers 21.10.2026 19:50: alle 16 neuen Hintergründe,
Standard „tiefblau“).

**Beschriftung der Symbole lesbar auf hellen Hintergründen.**

- Die Beschriftung liegt auf einer abgerundeten dunklen Hinterlegung
  (`alpha(#000000, 0.55)`, Radius 4 px, Innenabstand 1 px oben/unten und
  3 px seitlich). Der Schatten der weißen Schrift ist kräftiger
  (`1px 1px 2px #000000`, vorher 85 % Deckkraft).
- Weiß bleibt damit auch über reinem Weiß bei 4,7:1. Gemessen auf allen
  33 Hintergründen (17 alte, 16 neue) bei mindestens 5,1:1 gegen das
  hellste Pixel unter der Schrift (`huegel`). Vorher lagen die sechs
  hellen Landschaften bei 1,1–1,6:1.
- Bei Auswahl ersetzt wie bisher die orange Hinterlegung die dunkle.
  Beim Umbenennen bleibt das Feld unverändert.
- Hell und dunkel sehen gleich aus: Der dunkle Block hat keine Regel für
  die Beschriftung, und die Beschriftungszonen sind pixelgleich.
- Keine Kachel wird größer als ihre Zelle (zweizeilige Namen gemessen).
- Ein Test prüft die Deckkraft gegen 4,5:1 über Weiß.

**Standardbild „tiefblau“** (übernommen aus `standard-tiefblau` `3fafafd`).

- Ohne Bild in `desktop.ini` (`File=` leer oder fehlt) zeigt die
  Arbeitsfläche `querencia-tiefblau.png` aus `querencia/hintergruende/` der
  XDG-Datenordner (Repo und Paket `querencia-hintergruende`). Fehlt es, bleibt
  es beim bisherigen `querencia-blau.png`.
- Das Protokoll nennt das gefundene eingebaute Bild
  (`querencia-desktop: eingebautes Bild <Pfad>`).
- Farbe (`Color=`, Standard `#1c4e96`) und alles andere unverändert.

## 0.2.0 – 2026-10-21

CSS-Welle dunkel (QD-081, Alpha 2, Zweig `welle-dunkel`): Die Arbeitsfläche
folgt der dunklen Fassung von Querencia-Klassisch (Thema 0.2).

- `DUNKEL_CSS`: Das Kontextmenü übernimmt Zeile für Zeile die dunklen
  Menüregeln des Panels (ein Test vergleicht sie). Das Feld „Umbenennen“
  wird dunkel (`#2b2a27`, Schrift `#f2f1f0`). Die Beschriftungen der Symbole
  bleiben weiß mit Schatten auf dem Bild.
- Schaltet über `farbschema.rs` des Panels (per `#[path]`): dunkel nur bei
  dunklem Farbschema und einem Thema mit `gtk-dark.css`. Unter Thema 0.1
  bleibt alles hell.
- Startfehler GTK 4.22: einmal nach dem Start
  `notify("gtk-interface-color-scheme")`.
- Hell unverändert, isoliert gemessen pixelgleich zum Live-Stand G2-AI.

## 0.1.0 – bis 2026-10-20

Stufen ohne eigene Versionsnummer (HMP-08 … QE-04a, G2-AI), live zuletzt
G2-AI `4b45feac`; Einzelheiten in `docs/` und den Übergaben.
