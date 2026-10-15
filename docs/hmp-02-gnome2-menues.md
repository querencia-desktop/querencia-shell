# HMP-02 – Klassische GNOME-2-Menüs (Anwendungen | Orte | System)

**Stand:** 2026-10-15, Basis HMP-01 (`671297d` + `hmp-01-delta.patch`),
entwickelt in `~/projects/querencia-panel-werkstatt/hmp-02/arbeit`. Nicht
committet, nicht in die Runtime übernommen, keine GUI-Abnahme.

## Architektur

Die Menüs bleiben `MenuButton` + `PopoverMenu` („sliding“) mit `gio::Menu`,
Aktionen und `AppInfoMonitor` wie seit M1.1b-1c. Kein eigener
Menümechanismus (Entscheidungen M1.1b-1d und M1.1b-1e gelten weiter).

## Anwendungen

Gruppen wie im GNOME-2-Menü von Ubuntu 10.04 (`gnome-menus`,
deutsche Namen): Zubehör, Bildung, Spiele, Grafik, Internet, Büro,
Entwicklung, Unterhaltungsmedien, Systemwerkzeuge, Barrierefreiheit,
Sonstige. Jede Anwendung steht genau einmal; leere Gruppen erscheinen nicht.
„Zubehör“ nimmt `Utility` ohne `Accessibility` und `System` (wie GNOME 2).
Der Knopf zeigt das Logo (`start-here`) vor dem Text.

## Orte

Persönlicher Ordner, Arbeitsfläche, Dokumente, Musik, Bilder, Videos,
Downloads (nur vorhandene) – Trennlinie – Computer, Netzwerk, Papierkorb
(nur mit GVfs-Backend). Ohne Anwendung für `inode/directory` statt dessen nur
„Kein Dateimanager installiert“ (inaktiv). Neuaufbau bei geänderten
Anwendungen.

## System

Einstellungen ▸ / Systemverwaltung ▸ (nur mit Einträgen) – Informationen
über Querencia – Abmelden … (Rückfrage, dann `labwc --exit` mit dem
laufenden labwc aus `LABWC_PID`). Nicht angeboten, weil kein sicherer
Mechanismus vorhanden ist: Hilfe (kein Hilfebrowser), Bildschirm sperren
(kein Sperrprogramm), Herunterfahren (kein Querencia-Mechanismus).

## Optik

Keine Dropdown-Pfeile an den Panelknöpfen, Menü linksbündig ohne
Popover-Spitze, eckig, Zeilen 22 px, aktive Zeile in Auswahlfarben des
Themas. Das Panel nutzt GNOME Human als Symbolthema, wenn es installiert ist
und kein anderes Thema gewählt wurde (nur im Panel-Prozess).
