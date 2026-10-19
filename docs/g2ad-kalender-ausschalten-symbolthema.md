# G2-AD – Kalender an der Uhr, „Ausschalten …“, Abmelden mit Countdown, Symbolthema aus den Einstellungen

Stand 20.10.2026, Zweig `kalender-ausschalten`, Tags `panel/kalender-0.1`
und `desktop/symbolthema-0.1`. Entscheidungen des Besitzers vom 19./20.10.:
QD-052 (Kalender), QD-053 („Ausschalten …“), QD-054 (Abmelden mit
Countdown) und die Vorgabe, das Symbolthema aus den Benutzereinstellungen zu
nehmen.

## Uhr mit Kalender (QD-052)

- `clock.rs`: Die Uhr bleibt ein `Label` (Text, Format und Lage
  unverändert, kein Tooltip wie bisher). Ein Linksklick öffnet ein Popover
  mit `gtk4::Calendar`; jedes Öffnen zeigt den laufenden Monat mit dem
  heutigen Tag ausgewählt. Monat und Jahr blättern über die Pfeile im Kopf
  oder Strg+Pfeil links/rechts (Monat); Pfeiltasten wandern durch die Tage,
  die Leertaste wählt einen aus. Bild auf/ab kennt der GTK-4-Kalender nicht.
- Schließen: Esc (GTK), Klick daneben und erneuter Klick auf die Uhr. Den
  Klick, der ein offenes Popover schließt, verwirft GTK (`autohide`); er
  öffnet den Kalender deshalb nicht gleich wieder.
- Tastatur: Das Panel bleibt auf `KeyboardMode::None`. labwc gibt dem
  greifenden Popover die Tastatur wie beim Lautstärkeregler; der Kalender
  bekommt beim Öffnen den Fokus. In der Prüfung erreichen Esc, Strg+Pfeil,
  Pfeiltasten und Leertaste den Kalender.
- Rechtsklick und Verschieben: unverändert über `panel_menu.rs` bzw.
  `applet_move.rs`; der Verschiebemodus fängt Klicks vor dem Kalender ab.
- Optik (`main.rs`, Klasse `querencia-calendar`): ausgewählter Tag im
  Orange der Menühervorhebung mit weißer Schrift, heute fett und, solange
  ein anderer Tag ausgewählt ist, orange umrandet.

### Deutsche Namen und Wochenbeginn

GTK nimmt Monats- und Tagesnamen und den ersten Wochentag aus `LC_TIME`. Die
Sitzung setzt `LANG=en_US.UTF-8` und `LANGUAGE=de`
(`session/querencia/labwc/environment`); damit wäre der Kalender englisch
und finge am Sonntag an. Das Panel legt den Kalender deshalb unter dem
Zeitformat der Oberflächensprache an (`time_locale`: erste Sprache aus
`LANGUAGE`, wie gettext; `de` → `de_DE.UTF-8`), und zwar nur für den
eigenen Thread (`newlocale`/`uselocale` aus glibc, vier Funktionen, kein
neues Crate). Prozess-Locale, andere Threads und alles andere im Panel
bleiben unberührt. Ist das Zeitformat nicht installiert, bleibt es beim
Kalender nach `LC_TIME` (Logzeile). Die saubere Lösung für alle Programme
wäre `LC_TIME=de_DE.UTF-8` in der Sitzungsumgebung; das ist eine Änderung
an `session/` und eine Entscheidung für den Besitzer.

## System-Menü: „Abmelden …“ und „Ausschalten …“ (QD-053, QD-054)

- `session.rs`: `querencia_program` sucht ein Querencia-Programm zuerst im
  `PATH`, dann unter `~/opt/querencia/bin` (wie der Lautstärkeregler, der
  jetzt dieselbe Funktion nutzt). `run_sitzung` startet
  `querencia-sitzung` über `gio::Subprocess`; GIO erntet den Prozess, das
  Ende steht im Panel-Log.
- `system_menu.rs`: Abschnitt Sitzung in GNOME-2-Reihenfolge
  „Abmelden …“ (`system-log-out`), „Ausschalten …“ (`system-shutdown`).
  - „Abmelden …“ in einer labwc-Sitzung: mit Querencia Sitzung
    `querencia-sitzung --abmelden` (Rückfrage mit 60-s-Countdown, danach
    meldet sie selbst ab); ohne sie wie bisher die eigene Rückfrage des
    Panels und `labwc --exit`.
  - „Ausschalten …“ nur mit Querencia Sitzung:
    `querencia-sitzung --herunterfahren` (Rückfrage mit Countdown,
    Herunterfahren über logind). Ohne Sitzungsprogramm fehlt der Eintrag.
  - Das Programm wird beim Füllen des Menüs und bei jeder Auswahl erneut
    gesucht; ein inzwischen entferntes Sitzungsprogramm führt beim Abmelden
    auf den eingebauten Weg.
- Die Sitzung selbst ist unverändert. Die rc.xml-Kürzel (Strg+Alt+Entf →
  `--dialog`, Strg+Alt+L → `--sperren --sofort`) sind nur geprüft, nicht
  geändert.

## Symbolthema aus den Benutzereinstellungen

- Ursprung: Die Sitzung setzt kein Symbolthema, GTK nähme Adwaita. Für den
  GNOME-2-Look ohne sitzungsweite Änderung setzten Panel (HMP-02) und
  Desktop (HMP-09) im eigenen Prozess GNOME Human, sobald GTK einen
  eingebauten Standard meldete („“, Adwaita, hicolor). Zwei Folgen: Wer
  Adwaita wählte, bekam trotzdem GNOME Human, und weil das Programm den Wert
  gesetzt hatte, übernahm GTK spätere Änderungen aus dem Settings-Portal
  nicht mehr.
- Jetzt (`crates/querencia-panel/src/icon_theme.rs`, im Desktop über
  `#[path]` eingebunden): Entscheidend ist der Benutzerwert von
  `org.gnome.desktop.interface icon-theme` (GSettings/dconf, dieselbe
  Quelle, aus der das GTK-Portal liest). Gesetzt → dieses Thema (wenn
  installiert, sonst GNOME Human); nicht gesetzt oder zurückgesetzt → GNOME
  Human. Ohne das Schema gilt die alte Regel.
- Der Schlüssel wird beobachtet: Ein neues Thema oder ein Zurücksetzen wirkt
  sofort in Panel und Desktop, ohne Neustart (geprüft: Desktop-Symbole und
  Menüsymbole; die Fensterliste zeigt Themen-Symbole über dieselben
  `gio::ThemedIcon`, nicht eigens geprüft).
- Gesetzt wird weiter nur `GtkSettings` des eigenen Prozesses; keine Datei,
  kein anderes Programm.

## Prüfung

- Unit-Tests: `clock::tests` (Zeitformat aus `LANGUAGE`/`LC_*`/`LANG`),
  `session::tests::querencia_programs_from_path_then_opt_querencia`,
  `system_menu::tests` (Reihenfolge, Symbole, Einträge je nach
  Verfügbarkeit), `icon_theme::tests` (Wahl, Fallback, ohne Schema),
  `tests::calendar_style_matches_the_clock_popover`.
- Isolierte GUI (headless labwc 0.9.8, eigener D-Bus, eigenes Profil und
  dconf), 1024×768 und 1920×1080: `tools/kalender-pruefung/laeufe-g2ad.sh`
  mit den Arten `kalender`, `system` (alles mit `QS_TROCKENLAUF=1`, die
  Countdowns laufen bis 0 nur im Trockenlauf), `thema` (gsettings zur
  Laufzeit) und `thema-start` (Adwaita vor dem Start).
- Regression über `tools/kalender-pruefung/regression.sh` (G2-M, G2-J,
  W2) und `laeufe.sh` (G2-Y), Desktop über `nachweis/regression.sh`
  (QE-04a).
- Ergebnisse: `nachweis/ergebnisse/g2ad/`.

## Grenzen

- Das Zeitformat-Umschalten betrifft nur den Kalender. Andere Datumstexte
  des Panels sind numerisch und hängen nicht davon ab.
- GTK markiert „heute“ beim Erzeugen der Tageszellen; bleibt der Kalender
  über Mitternacht offen, wandert die Markierung erst beim nächsten Öffnen.
- Ohne Querencia Sitzung gibt es kein „Ausschalten …“ (kein eigener
  Weg im Panel, wie bisher).
- Ein im Benutzerwert genanntes, aber nicht installiertes Thema ersetzt das
  Panel durch GNOME Human (Logzeile), statt GTK auf hicolor fallen zu
  lassen.
- Querencia Files (QF-0f) hat dieselbe alte Regel im eigenen Repository;
  die Einstellungen zeigen noch „gnome-human (im Programm festgelegt)“.
