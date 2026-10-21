# Änderungen – querencia-desktop

## 0.2.1 – 2026-10-21

Standardbild „tiefblau“ (Hintergründe 0.2, QD-093, Entscheidung des
Besitzers 21.10.2026 19:50, Zweig `standard-tiefblau`).

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
