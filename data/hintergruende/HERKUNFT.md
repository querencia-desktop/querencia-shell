# Herkunft der eingebauten Hintergründe

Prüfsummen: `SHA256SUMS`. Erzeugt mit `erzeugen.py` (Beschreibung im Kopf des
Skripts, Prüfung mit `python3 -I erzeugen.py --pruefen`).

## Lizenz

Wie in `COPYRIGHT`: `querencia-blau.png` ist ein eigenes, per Skript erzeugtes
Bild (GPL-3.0-or-later wie das Programm). Die vier Motive aus ChatGPT stehen unter
CC-BY-SA-4.0. Rein KI-erzeugte Bilder sind in manchen Rechtsordnungen womöglich
nicht urheberrechtlich geschützt; die Lizenzangabe ist dann ein Hinweis, kein
durchsetzbarer Schutz. Das Q-Logo in den Dateien `*-logo*` ist aus der eigenen
SVG-Vorlage aufgeprägt und fällt unter `MARKE.md`; die Bildlizenz erstreckt sich
nicht auf das Logo. Die Hintergründe ab 0.2 liegen im Repo
`querencia-hintergruende`.

## Querencia Blau

| Datei | Erzeuger | Datum | Bearbeitung |
|---|---|---|---|
| `querencia-blau.png` (2560×1440) | eigenes Programm `tools/hintergrund/erzeugen.py` (HMP-08, Python + Pillow, fester Zufallsstartwert 2026), kein fremdes Material, keine KI | 18.10.2026 | keine; seit HMP-08 unverändert (`c350f0da…`) |

## Vier Motive aus ChatGPT

| Angabe | Wert |
|---|---|
| Dienst | ChatGPT (OpenAI), eingebaute Bildgenerierung |
| Modell | laut Generierungsdaten „DALL-E generation metadata“; genaue Version (DALL·E 3 oder GPT-Image) nicht angezeigt |
| Datum | Nacht vom 19. auf den 20.10.2026 |
| Bediener | pottrauschen |
| Prompt-Blatt | Prompt-Blatt im Rohordner (nicht im Repo); Zusammenfassung unten |
| Rohordner | Rohordner des Projekts (nicht im Repo) mit Herkunftsliste, Prüfsummen und allen Originalen |
| Logo | nicht von der KI: aus `logo/querencia-logo.svg` (Kopie von `querencia-systemmonitor/data/logo/querencia-logo.svg`, SHA256 `a1746943…`) per Skript aufgeprägt. Die KI-Fassungen mit Logo dienten nur als Stilvorlage (Lage unten rechts, etwa 180 px, dezente Prägung) |

Prompts, zusammengefasst (Stil für alle: ruhig wie Ubuntu 10.04/GNOME 2,
weiche Verläufe, keine Objekte, kein Text, Akzent `#F07746`, Ränder links,
oben und unten ruhig, 2560×1440 und eigener 4:3-Ausschnitt 1600×1200):

| Motiv | Prompt |
|---|---|
| Abend | Verlauf von warmem Dunkelbraun unten zu gedämpftem Aubergine oben, zwei bis drei geschwungene Lichtbänder in Orange |
| Blau 2 | tiefes, gedämpftes Blau mit diagonalem Lichtverlauf und schmalem, dezentem orangem Lichtsaum rechts unten |
| Erde | warmes Terrakotta und Braun, matt, kaum sichtbare Textur wie gebrannter Ton, ein Hauch Orange rechts |
| Nacht | tiefes Aubergine, an den Rändern fast schwarz, ein weicher oranger Lichtbogen unten rechts wie ein ferner Horizont |

## Je Datei

Quelle relativ zum Rohordner, Prüfsumme der Quelle (16 Stellen).

| Datei | Quelle | Bearbeitung |
|---|---|---|
| `querencia-abend.png` | `chatgpt-original/Copper Ribbons in Plum Shadows.png` (1672×941, `a4bf6a11461add98`) | von `erzeugen.py` auf 2560×1440 hochgerechnet (GdkPixbuf HYPER). Die zuerst sortierte Datei `querencia-abend.png` des Rohordners (`6437418985bdce30`) war aus „Warm Amber Waves on Maroon Gradient.png“ hochgerechnet und trug ein KI-gezeichnetes Q. Eine am 20.10. 01:05 aus demselben Original neu hochgerechnete Fassung (`228cc5a83c8e077a`) liegt ebenfalls vor; maßgeblich ist die eigene, reproduzierbare Hochrechnung aus dem unveränderten Original, die beiden gelieferten Dateien werden nicht verwendet |
| `querencia-abend-4x3.png` | `querencia-abend-4x3.png` (`dc686546bb21d3c2`) | byte-gleiche Kopie |
| `querencia-blau2.png` | `querencia-blau2.png` (`fc2b8085b9a26260`) | byte-gleiche Kopie |
| `querencia-blau2-4x3.png` | `querencia-blau2-4x3.png` (`fd9e33e293495e76`) | byte-gleiche Kopie. Die Quelle ist kein ChatGPT-Original, sondern ein mittiger 4:3-Ausschnitt aus dem 16:9-Bild, beim Sortieren angelegt (für Blau 2 gab es keine eigene 4:3-Fassung) |
| `querencia-erde.png` | `querencia-erde.png` (`d1d00e2c49a92dd0`) | byte-gleiche Kopie |
| `querencia-erde-4x3.png` | `querencia-erde-4x3.png` (`728b0105326df748`) | byte-gleiche Kopie |
| `querencia-nacht.png` | `querencia-nacht.png` (`11f112f17d58f9f1`) | byte-gleiche Kopie |
| `querencia-nacht-4x3.png` | `querencia-nacht-4x3.png` (`b8c9e17be1e1b0cd`) | byte-gleiche Kopie |
| `querencia-<motiv>-logo.png` | die Datei ohne Logo desselben Motivs | Q-Logo aufgeprägt: 180 px breit, Lage (2177, 1165), 203 px vom rechten Rand, damit es auch auf 16:10 sichtbar bleibt |
| `querencia-<motiv>-logo-4x3.png` | die 4:3-Datei ohne Logo | Q-Logo aufgeprägt: 150 px breit, Lage (1338, 904), sichtbar auf 5:4, 4:3 und 3:2 |

Bilder mit Logo unterliegen der Markenregel (siehe Lizenz).
