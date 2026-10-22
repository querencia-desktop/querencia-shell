# Panel 0.3.0 – Englisch per gettext

Stand 22.10.2026. Zweig `panel-024`, Tag `panel/0.3.0`, Integrationsstand
`panel/0.3.0-int`. Ausgang ist Panel 0.2.5 (live seit 21:46). Muster:
`docs/i18n-muster.md` im Editor-Repo, im selben Repo umgesetzt von
Querencia Desktop 0.2.5. Änderungen: `crates/querencia-panel/CHANGELOG.md`.

## 1. Aufbau

- **Textdomänen:**
  - `querencia-panel` für alle Texte des Panels.
  - `querencia-lautstaerke` und `querencia-netzwerk` für die Texte aus den
    beiden Bibliotheken, die das Panel zeigt: Tooltips, Netzliste,
    Verbindungsinformationen, Fehlermeldungen.
  - Die Bibliotheken übersetzen mit `dgettext` über ihre eigene Domäne.
    Das Panel bindet alle drei an dasselbe Verzeichnis
    (`crates/querencia-panel/src/i18n.rs`, `querencia_*::i18n::bind`).
- **msgid englisch, Deutsch aus `po/de.po`:**
  - Der deutsche Wortlaut ist der von 0.2.5.
  - `werkzeug-030/deutsch-gleich.py` im Arbeitsordner liest alle alten
    Literale aus `panel/0.2.5` und findet alle 91 (Panel), 3 (Lautstärke)
    und 19 (Netzwerk) sichtbaren deutschen Texte als Übersetzung wieder.
  - Was dort ausgenommen ist, steht mit Grund in `ausnahmen-*.py`:
    Protokoll, D-Bus-Antworten, Kennungen, technische Fehlerdetails, die
    Diagnose `--zustand` und die Kommandozeile `--panel-ini`.
- **Kataloge:**
  - Das Programm sucht `<lang>/LC_MESSAGES/<domäne>.mo` zuerst neben der
    Binary (`share/locale`), dann im Präfix (`/usr/bin/../share/locale`),
    zuletzt in `/usr/share/locale`.
  - `QUERENCIA_PANEL_LOCALEDIR` setzt das Verzeichnis für Tests.
- **Sprachwahl:**
  - gettext wie üblich, `LANGUAGE` vor `LC_ALL`/`LC_MESSAGES`/`LANG`.
  - Die Sitzung läuft mit `LANG=en_US.UTF-8` und `LANGUAGE=de`, also
    deutsch.
  - `LANGUAGE=en` oder eine Sprache ohne Katalog ergibt Englisch.
- **Unübersetzt bleiben:**
  - Protokollzeilen (deutsch, wie überall im Projekt).
  - Kennungen.
  - Programmnamen aus den Desktop-Dateien; die übersetzt GIO selbst über
    `Name[de]`.
  - Der Release-Name aus `/usr/share/querencia/release`.
- **Desktop-Datei:** Das Panel hat keine, es wird vom Sitzungsstarter
  gestartet.

## 2. Wortwahl

Die englischen Texte folgen dem GNOME-2-Panel:

- Hauptmenüs: Applications, Places, System.
- Kategorien: Accessories, Graphics, Internet, Office, Programming,
  Sound & Video, System Tools, Universal Access, Other.
- System-Untermenüs: Preferences, Administration.
- Orte: Home Folder, Desktop, Documents, Music, Pictures, Videos,
  Downloads, Computer, Network, Trash.
- Panel-Kontextmenü: Add to Panel…, Remove From Panel, Move.
- Netzwerk-Applet nach nm-applet: Wireless Networks, Connect to Hidden
  Network…, Connection Information.
- Die Uhr englisch wie GNOME 2: `%a %b %e, %l:%M %p`.

Zugriffstasten sind je Dialog eindeutig. Wo sie im Englischen kollidierten,
weicht die Taste ab: „For_ward“ neben „_Find …“, „C_onnect“ neben
„_Cancel“, „Sho_w password“ neben „_Password:“. Das Deutsche behält seine
bisherigen Tasten.

## 3. Pflege

Je Crate (`crates/querencia-{panel,lautstaerke,netzwerk}`):

```bash
python3 werkzeug/i18n.py pot       # po/<domäne>.pot aus src/**/*.rs
python3 werkzeug/i18n.py update    # po/de.po nachziehen
python3 werkzeug/i18n.py pruefen   # PASS: aktuell, vollständig, Platzhalter, Zugriffstasten
python3 werkzeug/i18n.py mo <ziel> # <ziel>/de/LC_MESSAGES/<domäne>.mo
msgfmt -c -o /dev/null po/de.po    # Gegenprobe GNU gettext 0.23.2: ohne Meldung
```

Die `.mo` aus `i18n.py` sind laut `msgunfmt` inhaltsgleich mit denen aus
`msgfmt`; sie unterscheiden sich nur im leeren Kopffeld
`POT-Creation-Date`, wie schon beim Editor.

Tests:

- `german_catalog_is_complete` in allen drei Crates;
- `german_wording_is_the_old_one` (Stichproben, auch mit Kontext);
- in anderen Tests `crate::i18n::german(msgid)` für den deutschen Wortlaut.

Tests ohne Katalog erwarten die englische msgid.

## 4. Grenzen

- Der Lautstärkeregler (`querencia-lautstaerke`, eigenes Programm) bleibt
  deutsch. Er nutzt die Bibliothekskonstante `NO_SERVICE`, nicht die
  übersetzten Panel-Texte.
- Die Diagnose `querencia-netzwerk --zustand` setzt keine Locale. Nach
  ihrem nächsten Bau gibt sie Tooltip und Verbindungsinformationen englisch
  aus. Vor einem Release der Diagnose muss sie `bind`/`init` und eine
  Locale bekommen (Folgepunkt).
- Technische Fehlerdetails aus dem Speichern von `panel.ini` bleiben
  deutsch. Sie erscheinen nur hinter dem übersetzten Satz „The item could
  not be added: …“.
- `classic_menu.rs` ist mit dem Desktop geteilt. Die Tastennamen laufen in
  beiden Crates über `pgettext`; der Desktop zeigt keine Kürzel.

## 5. Prüfung

Siehe Bericht .

- fmt, clippy und Tests grün.
- `pruefen` in allen drei Crates PASS, `msgfmt -c` ohne Meldung.
- Deutsch-Gleichheit gegen 0.2.5 PASS.
- Isolierte Fotos je `LANGUAGE=de` und `en`, je 1024×768 und 1920×1080,
  hell und dunkel. In der de-Sitzung zeigt `/proc/<pid>/maps` den Katalog
  aus der Ablage, in der en-Sitzung keinen.
