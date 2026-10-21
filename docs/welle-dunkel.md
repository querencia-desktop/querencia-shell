# CSS-Welle dunkel – Panel 0.2.1 und Desktop 0.2.0 (QD-081)

Stand 21.10.2026, Zweig `welle-dunkel`. Auftrag „CSS-Welle dunkel“ für
Alpha 2: Jedes Programm bekommt eine dunkeltaugliche Fassung, erst danach
aktiviert die Status-Session Thema 0.2 (dunkle Fassung von
Querencia-Klassisch) zusammen mit allen Programmen.

## Was sich ändert

- **Panel** (`crates/querencia-panel/src/main.rs`): `PANEL_CSS` bleibt
  unverändert. Dazu kommt `PANEL_CSS_DUNKEL`, ein Block
  `@media (prefers-color-scheme: dark)` mit dunklen Gegenstücken für
  - Anwendungen/Orte/System,
  - Arbeitsflächenumschalter und Fensterliste,
  - Starter, Statusknöpfe und „Arbeitsfläche anzeigen“,
  - Zahl der Glocke,
  - Netzwerkmenü und die klassischen Menüs.

  Jede dunkle Regel hat denselben Selektor wie eine helle, daher bleibt die
  Kaskade dieselbe. Ausnahme ist eine überfahrene aktive Zelle des
  Umschalters (`:hover:not(.active)`).
- **Desktop** (`crates/querencia-desktop/src/main.rs`): `DUNKEL_CSS` mit den
  dunklen Menüregeln des Panels, Zeile für Zeile, und dem Feld
  „Umbenennen“. Die Beschriftungen der Symbole bleiben weiß auf dem Bild.
- **`farbschema.rs`** (Panel, im Desktop per `#[path]`):
  - `folgen(provider, programm)` setzt `prefers-color-scheme` des eigenen
    `GtkCssProvider`, und zwar vor dem Laden der CSS sowie nach jedem
    Wechsel von Farbschema, `gtk-application-prefer-dark-theme` oder
    Themenname.
  - `startfehler_beheben()` ist der GTK-4.22-Einzeiler
    `notify("gtk-interface-color-scheme")`.

**Regel für Panel und Desktop:** Eigene Farben werden nur dunkel, wenn
`farbschema::dunkel` gilt, also bei dunklem Farbschema und einem Thema mit
dunkler Fassung. Nie wird allein an `gtk-interface-color-scheme` gebunden.

Dunkel gilt nur, wenn das Farbschema dunkel ist (Portal: color-scheme 1;
ohne Portal: `gtk-application-prefer-dark-theme` aus `portal.rs`) **und**
das GTK-Thema eine dunkle Fassung hat (`gtk-4.0/gtk-dark.css`). Das ist
dasselbe Kriterium wie `dunkel_aktiv()` in Einstellungen 0.1.2.

## Warum nicht einfach binden

Gemessen isoliert (`~/projects/querencia-runtime-welle-dunkel/nachweis/bindung/`):

| Probe | Thema 0.2 dunkel (Start und Wechsel) | Thema 0.1 dunkel |
|---|---|---|
| ohne Bindung | Programm-CSS bleibt hell | hell (Thema hell) |
| `bind_property` an `gtk-interface-color-scheme` | dunkel | **Programm-CSS dunkel, Thema hell** (Mischzustand) |
| Kriterium `farbschema.rs` | dunkel | hell |

Unter Thema 0.1 (heute live) und „Dunkle Darstellung bevorzugen“ (seit
Einstellungen 0.1.2 einstellbar) würde reines Binden dunkle Knöpfe und
helle Schrift auf das helle Panel legen. Mit dem Kriterium bleibt das
Panel pixelgleich zum Live-Stand, bis Thema 0.2 aktiv ist. Ein Rückfall
des Themas auf 0.1 braucht keinen Rückfall von Panel oder Desktop.

## Farben

Die Werte stammen aus der Zuordnung des Themas
(`querencia-thema-wt-02/quelle/dunkel-zuordnung.txt`):

| Teil | Werte |
|---|---|
| Panelgrund | `#3c3b37` (vom Thema) |
| Schrift | `#dfdbd2` |
| Erhabene Knöpfe | wie Themenknöpfe (`#57554f`→`#4a4843`, überfahren `#5f5d57`→`#524f4a`, gedrückt `#33322e`→`#3c3b37`, Rand `#23221f`) |
| Umschalterzellen | wie Kopfleiste (`#524f4a`→`#3c3b37`) |
| Menü | `#353430`, Rand `#2b2a27` |
| Graue Nebenschrift | `#a5a09a`/`#b3aea7` |

Die Fensterknopf-Fläche der Sichtungstabelle (`≈#4d4528`) war ein
Zuordnungsfehler (Farbe der Infoleiste) und ist durch `#4f4d48` ersetzt.
Kontrast AA: `werkzeug/kontrast-panel.py` im Arbeitsordner nutzt
`kontrast.py` des Thema-Repos.

## Grenzen

- Die Symbole von Lautstärke, Netzwerk und Glocke sind feste graue SVGs.
  Ab Panel 0.2.1 zeichnet das Panel sie bei `farbschema::dunkel` umgefärbt
  (`symbol_farbe.rs`):
  - Fläche `#bdb7b1`, Umriss `#23221f`;
  - leere Balken `#5f5d57`;
  - ausgegraut `#858079`.

  Die Quellen bleiben unverändert, hell ist pixelgleich.
- Die Titelleisten (labwc) haben keine dunkle Fassung (Thema-Übergabe §6).
- Das Applet-Kontextmenü und das Glockenmenü sind `PopoverMenu` und folgen
  dem Thema selbst.
