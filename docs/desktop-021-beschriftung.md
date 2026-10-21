# Desktop 0.2.1: Beschriftung der Symbole auf hellen Hintergründen

Stand 21.10.2026, Zweig `desktop-0.2.1`.

## Anlass

Hintergründe 0.2 bringt 16 neue Bilder, darunter sechs helle Landschaften:
`huegel`, `kueste`, `duenen`, `see`, `wueste` und `berge` (Mitte L 174–234).
Desktop 0.2.0 zeichnet die Beschriftung weiß mit einem Schatten
(`1px 1px 2px`, 85 % Schwarz) direkt auf das Bild. Auf diesen sechs Bildern
ist sie kaum lesbar (Bericht Hintergründe 0.2, §7).

Der Besitzer hat am 21.10.2026 um 19:50 entschieden: Alle 16 Bilder kommen
in Alpha 2. Die Beschriftung muss deshalb auf jedem Bild lesbar sein.

## Lösung

`DESKTOP_CSS` in `crates/querencia-desktop/src/main.rs`:

```css
box.desktop-icon > label { color: #ffffff; text-shadow: 1px 1px 2px #000000;
  background-color: alpha(#000000, 0.55); padding: 1px 3px; border-radius: 4px; }
```

- **Hinterlegung:** eine abgerundete, halbtransparente dunkle Fläche hinter
  jeder Beschriftung, bei zweizeiligen Namen eine Fläche für beide Zeilen.
  Das Bild scheint gedämpft durch, wie bei den Beschriftungen von GNOME 2.
- **Schatten:** voll deckend statt 85 %.
- **Auswahl:** Die orange Hinterlegung ersetzt die dunkle (Regel `.selected`,
  unverändert, jetzt mit Radius 4 statt 3 px).
- **Umbenennen:** Das weiße bzw. dunkle Eingabefeld liegt wie bisher über der
  ausgeblendeten Beschriftung.
- **Hell und dunkel** sehen gleich aus. `DUNKEL_CSS` enthält keine Regel für
  die Beschriftung; ein Test prüft das.

**Warum 55 %:** Weiße Schrift braucht 4,5:1 gegen den hellsten Grund. Über
reinem Weiß ergibt eine schwarze Fläche mit Deckkraft a den Grauwert
255 · (1 − a).

| Deckkraft | Grauwert über Weiß | Kontrast Weiß |
|---|---|---|
| 0,45 | 140 | 3,4:1 |
| 0,50 | 128 | 4,0:1 |
| 0,55 | 115 | 4,7:1 |

0,55 ist damit die kleinste Stufe in 5-%-Schritten, die auf **jedem** Bild
hält, auch auf eigenen Bildern der Benutzer. Der Test
`the_label_plate_keeps_white_text_readable_on_any_picture` rechnet das aus
der CSS nach.

**Innenabstand:** 1 px oben und unten, 3 px seitlich. Mehr Höhe ließe
zweizeilige Kacheln (100 × 86) über die Zelle (100 × 88) wachsen. Die
Probe meldet keine Kachel, die größer als ihre Zelle ist.

## Verworfene Wege

| Weg | Warum nicht |
|---|---|
| Nur den Schatten verstärken (dicker, mehrfach, Halo) | Die Glyphen bleiben weiß auf fast weißem Grund. Ein Schatten von 1–2 px dunkelt nur den Rand ab, nicht die Fläche zwischen und in den Buchstaben. Auf `huegel` bleibt der Kontrast gegen das Hellste unter dem Text bei etwa 1:1. Ein dichter Halo aus vielen Schatten wirkt verschmiert und hält auf unruhigen Bildern (`see`, `wueste`) keinen Mindestwert. |
| Hinterlegung nur beim Überfahren | Gelesen wird die Beschriftung in Ruhe, nicht nur mit dem Zeiger darüber; im Ruhezustand bliebe sie unlesbar. Der Desktop hat bisher keinen Überfahren-Zustand (wie GNOME 2 ohne „prelight“ der Schrift). Zudem gibt es mit Tastatur und Touch keinen Zeiger. |
| Dunkle Schrift je Bild | ausgeschlossen (Auftrag). Sie bräuchte eine Helligkeitsmessung je Bild und Bildschirm und wechselte die Optik zwischen den Bildern. |

## Messung

`~/projects/querencia-desktop-021/` (Arbeitsordner): `werkzeug/lauf.sh`,
`szenario.sh` und `messen.py`. Je Hintergrund (17 alte, 16 neue) ein
Bildschirmfoto 1920 × 1080 mit vier Symbolen links. Die Läufe sind
isoliert: eigener labwc, eigenes HOME, eigener Bus.

Gemessen wird der Kontrast weißer Schrift gegen das hellste Pixel der Fläche
unter der Schrift, ohne die Schriftpixel. Die Fläche ist:
- nachher die Hinterlegung;
- vorher das Rechteck der Schrift mit demselben Innenabstand.

Die Formel ist `kontrast.py` des Thema-Repos.

**Ergebnis:** 33/33 erreichen hell und dunkel mindestens 4,5:1, und die
Beschriftungszonen sind in hell und dunkel pixelgleich. Volle Tabelle:
`integration/desktop-021/nachweis/kontrast.md`.

| Hintergrund | vorher | nachher |
|---|---|---|
| huegel | 1,10 | 5,11 |
| see | 1,22 | 5,50 |
| wueste | 1,28 | 5,70 |
| kueste | 1,47 | 6,25 |
| duenen | 1,47 | 6,28 |
| berge | 1,56 | 6,58 |
| strand | 8,88 | 16,02 |
| alle 17 alten | 10,04–20,21 | 16,83–20,70 |

Das hellste Pixel unter der Hinterlegung hat höchstens den Kanalwert 114
(`huegel`). Das liegt unter dem rechnerischen Grenzwert 115 über reinem Weiß.

**Zustände:**
- Überfahren ändert nichts (pixelgleich zum Ruhezustand).
- Auswahl, Mehrfachauswahl und Umbenennen sind wie bisher.
- Keine Kachel ist größer als ihre Zelle.
