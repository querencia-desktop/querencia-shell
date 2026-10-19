# Herkunft der eingebauten Hintergründe

Stand 20.10.2026, G2-AF (QD-058). Prüfsummen: `SHA256SUMS`. Erzeugt mit
`erzeugen.py` (Beschreibung im Kopf des Skripts, Prüfung mit
`python3 -I erzeugen.py --pruefen`). Einbau und Auswahl:
`docs/g2af-hintergruende.md`.

## Lizenz

Die Lizenz der Hintergründe ist **noch nicht entschieden**
(`querencia-github/ENTSCHEIDUNGEN.md` Nr. 15, Vorschlag der Status-Session,
offen beim Besitzer). Vorschlag:

- Bilder **ohne Logo**: CC-BY-SA-4.0, wie die Dokumentation.
- Bilder **mit Q-Logo**: zusätzlich die Markenregel (Nr. 5, `MARKE.md`).
  Das Logo ist ein Markenzeichen von Querencia Desktop und nicht frei;
  die Bildlizenz erstreckt sich nicht auf das Logo.
- **KI-erzeugte Bilder** nur, wenn die Nutzungsbedingungen des Erzeugers
  die Weitergabe unter CC-BY-SA erlauben. Für die vier Motive unten hat
  ChatGPT im Erzeugungs-Chat Weitergabe, Veränderung und Nutzung in einem
  Open-Source-Projekt, auch kommerziell, bejaht. Zu prüfen bleiben die
  OpenAI-Nutzungsbedingungen zum Zeitpunkt der Erzeugung.

Bis zur Entscheidung gelten die Bilder als Teil des privaten Repos und
werden nicht veröffentlicht.

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
| Prompt-Blatt | `WALLY-PROMPTS.md` im Rohordner `~/Bilder/querencia-hintergruende-roh/` (Prompts 1–4, Zusatz „4:3-Variante“); Zusammenfassung unten |
| Rohordner | `~/Bilder/querencia-hintergruende-roh/` mit `herkunft.txt`, `SHA256SUMS` und allen Originalen unter `chatgpt-original/` |
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
| `querencia-abend.png` | `chatgpt-original/Copper Ribbons in Plum Shadows.png` (1672×941, `a4bf6a11461add98`) | auf 2560×1440 hochskaliert (GdkPixbuf HYPER). Die sortierte Datei `querencia-abend.png` des Rohordners (`6437418985bdce30`) enthält ein KI-gezeichnetes Q und wird nicht verwendet |
| `querencia-abend-4x3.png` | `querencia-abend-4x3.png` (`dc686546bb21d3c2`) | byte-gleiche Kopie |
| `querencia-blau2.png` | `querencia-blau2.png` (`fc2b8085b9a26260`) | byte-gleiche Kopie |
| `querencia-blau2-4x3.png` | `querencia-blau2-4x3.png` (`fd9e33e293495e76`) | byte-gleiche Kopie |
| `querencia-erde.png` | `querencia-erde.png` (`d1d00e2c49a92dd0`) | byte-gleiche Kopie |
| `querencia-erde-4x3.png` | `querencia-erde-4x3.png` (`728b0105326df748`) | byte-gleiche Kopie |
| `querencia-nacht.png` | `querencia-nacht.png` (`11f112f17d58f9f1`) | byte-gleiche Kopie |
| `querencia-nacht-4x3.png` | `querencia-nacht-4x3.png` (`b8c9e17be1e1b0cd`) | byte-gleiche Kopie |
| `querencia-<motiv>-logo.png` | die Datei ohne Logo desselben Motivs | Q-Logo aufgeprägt: 180 px breit, Lage (2177, 1165), 203 px vom rechten Rand, damit es auch auf 16:10 sichtbar bleibt |
| `querencia-<motiv>-logo-4x3.png` | die 4:3-Datei ohne Logo | Q-Logo aufgeprägt: 150 px breit, Lage (1338, 904), sichtbar auf 5:4, 4:3 und 3:2 |

Bilder mit Logo unterliegen der Markenregel (siehe Lizenz).
