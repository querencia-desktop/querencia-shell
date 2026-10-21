# Änderungen – querencia-panel

## 0.2.1 – 2026-10-21

Statussymbole auf dem dunklen Panel (Entscheidung vom 21.10.2026:
Variante A).

- Neues Modul `symbol_farbe.rs`: Solange `farbschema::dunkel` gilt, zeichnet
  das Panel seine eigenen Symbole für Lautstärke, Netzwerk (Applet und
  Netzwerkmenü) und Glocke mit umgetauschten Grautönen (`UMFAERBUNG`).
  - Fläche `#5a5a5a` → `#bdb7b1`, Umriss `#3c3c3c` → `#23221f`.
  - Blasse „leere“ Balken bleiben blass (`#5f5d57`), ausgegraute Symbole
    bleiben ausgegraut.
  - Orange, Weiß und die orangen Zeilenvarianten bleiben.
- Beim Wechsel des Farbschemas oder Themas werden die Symbole sofort neu
  gezeichnet.
- Die SVG-Quellen und die Crates `querencia-lautstaerke` und
  `querencia-netzwerk` bleiben unverändert. Das Panel nutzt deren
  öffentliches `svg()`. Hell zeichnen die Crates wie bisher (pixelgleich).
- Tests: Jede Graufarbe der Applet-Symbole hat ein dunkles Gegenstück, und
  Orange bleibt an derselben Stelle.

## 0.2.0 – 2026-10-21

CSS-Welle dunkel (Alpha 2, Zweig `welle-dunkel`): Das Panel folgt
der dunklen Fassung von Querencia-Klassisch (Thema 0.2).

- Dunkle Gegenstücke zu den eigenen Regeln in `PANEL_CSS` als ein Block
  `@media (prefers-color-scheme: dark)` (`PANEL_CSS_DUNKEL`):
  Anwendungen/Orte/System und alle Schriften des Panels in `#dfdbd2`,
  Arbeitsflächenumschalter, Fensterliste (normal, überfahren, aktiv,
  minimiert), Knöpfe (Starter, Statussymbole, „Arbeitsfläche anzeigen“),
  Netzwerkmenü, klassische Menüs (`#353430`, Rand `#2b2a27`, Pfeile und
  Kürzel `#a5a09a`). Werte aus der Zuordnung des Themas. Orange-Hervorhebung
  mit weißer Schrift bleibt. Hell ist unverändert, isoliert gemessen
  pixelgleich zum vorigen Stand.
- Neues Modul `farbschema.rs` (auch vom Desktop benutzt): Die eigene CSS
  wird dunkel, genau wenn das Farbschema dunkel ist **und** das GTK-Thema
  eine dunkle Fassung hat (`gtk-4.0/gtk-dark.css`, wie Einstellungen 0.1.2).
  Grundlage ist die isolierte Messung: Ohne Bindung greift der Dunkelblock
  nie. Mit reiner Bindung an `gtk-interface-color-scheme` wird die CSS
  unter Thema 0.1 dunkel, obwohl das Thema hell bleibt. Folgt
  Farbschema, `gtk-application-prefer-dark-theme` (Betrieb ohne Portal)
  und Themenname im Lauf. Logzeile „Farbschema hell|dunkel (Start/Wechsel
  …)“.
- Startfehler GTK 4.22: einmal nach dem Start
  `notify("gtk-interface-color-scheme")`, damit das Thema unter „dunkel“
  nicht hell bleibt.
- Tests: Dunkelblock nur aus Selektoren der hellen Regeln; jede dunkle
  Schrift `#3c3c3c` hat ein helles Gegenstück; Hover gewinnt nie über
  aktive Zelle oder minimierten Knopf; eingebautes Thema hat Dunkel.

## 0.1.0 – bis 2026-10-20

Stufen ohne eigene Versionsnummer (M1 … HMP-11), zuletzt `57a01524`
(Arbeitsfläche anzeigen, Glocke der Mitteilungszentrale); Einzelheiten in
`docs/`.
