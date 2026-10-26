# Panel 0.4.0 – Infobereich (StatusNotifierItem)

Stand 27.10.2026. Zweig `infobereich` ab `main` `6935c48`, main `6d95ff9` (it/es/fr, Panel 0.3.3) eingemergt. Punkt QD-164
(08): Das Panel hatte keinen Infobereich; Programme wie Webex, Nextcloud,
Discord oder ZapZap zeigten kein Symbol. Entscheidung pottrauschen 23.10. und
26.10.: Infobereich-Applet bauen.

## 1. Was der Infobereich ist

Applet-Typ `notification-area` („Infobereich“, englisch „Notification
Area“), ein Status-Applet wie Lautstärke, Netzwerk und Glocke. Im
Standardlayout oben rechts vor der Glocke (`notification-area-1`), beim
Hinzufügen rechts neben Netzwerk bzw. Lautstärke, also links der Glocke.
Höchstens einer je Layout (ein Watcher und ein Host je Sitzung).

Jedes Programm, das sich mit dem StatusNotifierItem-Protokoll anmeldet,
bekommt einen Statusknopf in Panelgröße (wie die übrigen Statussymbole,
2 px Abstand, Hover-Rahmen):

| Eingabe | Wirkung |
|---|---|
| Linksklick | `Activate(0, 0)`; vorher `ProvideXdgActivationToken` mit einem xdg-activation-Token des Klicks (Wayland-Programme dürfen ihr Fenster nach vorn holen). Ein Item mit `ItemIsMenu` zeigt stattdessen sein Menü; scheitert `Activate` und hat das Item ein Menü, öffnet sich das Menü. |
| Mittlere Taste | `SecondaryActivate(0, 0)` beim Loslassen; mittleres Ziehen verschiebt das Applet wie bei allen Applets. |
| Mausrad | `Scroll(±120, "vertical"/"horizontal")` je Raste, hoch positiv (wie KDE). |
| Rechtsklick | Menü des Programms (com.canonical.dbusmenu) als klassisches Querencia-Menü; ohne `Menu`-Eigenschaft `ContextMenu(0, 0)`. |
| Rechtsklick auf den 2-px-Rand | Appletmenü: „Aus dem Panel entfernen“, „Verschieben“ und die Verschiebe-Einträge. |

Anzeige:

- Symbol: `IconName` aus dem Symbolthema; mit `IconThemePath` zuerst aus
  diesem Ordner (eigenes `GtkIconTheme` mit Ordner + Suchpfad des Themas,
  ohne Vorladen); ein absoluter Pfad in `IconName` als Datei; sonst
  `IconPixmap` (ARGB32, das kleinste Bild ≥ Symbolgröße, mit gdk-pixbuf
  „Hyper“ skaliert); sonst `application-x-executable`.
- `OverlayIconName`/`…Pixmap` in halber Größe unten rechts.
- Status `Passive`: ausgeblendet. `NeedsAttention`: `AttentionIconName`
  bzw. `…Pixmap` (ohne eines das normale Symbol) auf hellorange
  hinterlegtem Knopf mit orangem Rand (dunkel: dunkles Orange).
- Tooltip: Titel und Text von `ToolTip` (Markup entfernt, `<br>` als
  Zeilenumbruch), sonst `Title`, sonst `Id`.
- Ohne sichtbares Item ist das Applet ausgeblendet (wie ein leerer GNOME-2-
  Infobereich).
- Reihenfolge: wie beim Watcher angemeldet, links nach rechts.
- Hell/dunkel: Programmsymbole bleiben, wie sie sind (symbolische
  Themensymbole färbt GTK selbst); die Hervorhebung hat einen Dunkelblock
  (QD-081: dunkel nur bei `dunkel_aktiv`, Regeln wie die übrigen
  Statussymbole).

## 2. Aufbau (`crates/querencia-panel/src/infobereich/`)

| Datei | Inhalt |
|---|---|
| `mod.rs` | Applet (Zeile der Knöpfe), Knopf je Item, Klicks, Zeichnen der Symbole, Menüanker |
| `watcher.rs` | StatusNotifierWatcher (reines `Register` mit Tests, D-Bus-Dienst) |
| `host.rs` | Host: Watcher folgen, Items und ihre Eigenschaften, Aufrufe an Items |
| `eintrag.rs` | Eigenschaften eines Items (rein, mit Tests): Status, Pixmaps, Symbolquellen, Tooltip, Item-Adresse |
| `dbusmenu.rs` | dbusmenu-Client: Layout lesen, als `gio::Menu` für `classic_menu`, `Event` |

D-Bus: `gio::DBusConnection` wie Glocke, Netzwerk und Neuladen; keine neue
Abhängigkeit (kein zbus). Der Host ist ein Prozess-Einzelstück (zwei
Applets beim Neuladen von `panel.ini` teilen ihn); er endet mit dem letzten
Applet, mit ihm Watcher und Busnamen.

### Watcher

- Busname `org.kde.StatusNotifierWatcher` mit Warteschlange: Hat ein anderes
  Programm den Namen (anderer Desktop, `snixembed`, …), bietet das Panel
  keinen zweiten an, ist nur Host und übernimmt den Namen, wenn der andere
  endet. Erst mit dem KDE-Namen nimmt es auch
  `org.freedesktop.StatusNotifierWatcher` (dasselbe Objekt unter beiden
  Schnittstellennamen); sonst stünden Items an einem Alias, dem der Host
  nicht folgt.
- `RegisterStatusNotifierItem(s)`: Busname oder Objektpfad des Aufrufers
  (libappindicator); ID `<busname><pfad>` wie KDE. Ungültige Argumente:
  `InvalidArgs`; Busname ohne Besitzer: `ServiceUnknown`. Ein Item geht,
  wenn sein Busname den Besitzer verliert (`NameOwnerChanged`).
- `RegisterStatusNotifierHost(s)`: der Aufrufer gilt als Host bis zu seinem
  Ende. Eigenschaften `RegisteredStatusNotifierItems`,
  `IsStatusNotifierHostRegistered`, `ProtocolVersion` (0); Signale
  `StatusNotifierItemRegistered/-Unregistered`,
  `StatusNotifierHostRegistered/-Unregistered` (unter beiden
  Schnittstellennamen).

### Host

- Besitzt `org.kde.StatusNotifierHost-<pid>` und folgt dem Besitzer von
  `org.kde.StatusNotifierWatcher` (`bus_watch_name`): bei jedem neuen
  Besitzer Anmeldung als Host, Signale des Watchers, Liste
  `RegisteredStatusNotifierItems`. Geht der Watcher, gehen alle Items; die
  Programme melden sich beim nächsten neu an.
- Je Item `Properties.GetAll` (Schnittstelle `org.kde.StatusNotifierItem`,
  bei Fehler einmal `org.freedesktop.StatusNotifierItem`), danach nach jedem
  `New*`-Signal bzw. `PropertiesChanged` neu, entprellt (40 ms; animierte
  Symbole schicken viele `NewIcon`).
- Zeitgrenze jedes Aufrufs 3 s, Menüaufrufe 2 s, immer `NO_AUTO_START`.

### Menüs

Beim Öffnen: `AboutToShow(0)`, `GetLayout(0, -1, [])`, für alle Untermenüs
`AboutToShowGroup`; verlangt das ein Update, wird das Layout noch einmal
gelesen. Fehler von `AboutToShow*` stoppen das Menü nicht. Daraus wird ein
`gio::Menu`: Trennlinien → Abschnitte, Untermenüs, `toggle-type`
`checkmark`/`radio` mit `toggle-state` → Häkchen/Punkt (neu in
`querencia-oberflaeche` 0.1.3, Attribute `x-querencia-toggle`/`-toggled`),
`enabled=false` → grau (keine Aktion), `visible=false` → fehlt, `icon-name`
bzw. `icon-data` (PNG) → Symbol, `shortcut` → Kürzelspalte,
Zugriffstasten (`_`) entfernt. Gezeigt wird es mit
`classic_menu::attach_popup` an einem unsichtbaren Anker über dem Symbol
(wie das Kontextmenü der Arbeitsfläche): Tastatur, Untermenüs mit
Zielkegel, Ambiance-Optik wie die Panelmenüs. Ein Eintrag schickt
`Event(id, "clicked", 0, zeit)`; Öffnen und Schließen `Event(0,
"opened"/"closed")`. `LayoutUpdated` während das Menü offen ist ändert es
nicht (die klassischen Menüs ändern ihr Modell nur geschlossen); das
nächste Öffnen liest neu. Ein leeres oder unlesbares Menü führt zu
`ContextMenu`.

Vor dem Öffnen nimmt der Anker den Fokus im Panelfenster, wie ein Klick auf
einen Menüknopf; ohne Fokus-Widget meldete GTK 4.22 beim Öffnen
`gtk_widget_is_ancestor: assertion 'GTK_IS_WIDGET (widget)' failed`
(isoliert gemessen, Lauf t1).

## 3. panel.ini und Integration

- Standardlayout: `End=notification-area-1;benachrichtigungen-1;clock-1`.
- Bestehende `panel.ini` bekommt den Infobereich nur über
  `querencia-panel --panel-ini infobereich-ergaenzen <datei>` (vor der
  Glocke, sonst vor der Uhr, sonst am Ende des oberen Panels; schon
  vorhanden: unverändert). `infobereich-zuruecknehmen` nimmt nur ihn heraus
  (Glocke und „Arbeitsfläche anzeigen“ bleiben). Das Panel ändert die
  Benutzerdatei nie von selbst.
- Integration: siehe `integration/panel-040/` (Skript
  `panel-040-integrieren.sh`).

## 4. Prüfung

Alles isoliert (eigenes HOME, `XDG_RUNTIME_DIR`, Sitzungsbus über
`dbus-run-session`, headless labwc, Portal-Ersatz; nie der Live-Bus).
Prüfrahmen: `integration/panel-040/werkzeug/` (Infobereich) und
`integration/panel-040/regression-werkzeug/` (Kopie der Panel-Regression aus
0.3.1). Belege: `integration/panel-040/nachweis/`.

- **Unit-Tests:** `cargo test --workspace --locked` grün, Panel 309 + 1
  ignoriert (Menü-GUI-Test, eigener Lauf); neu u. a. Watcher-Register,
  Item-Adressen, Eigenschaften, Pixmaps, Tooltip-Markup, dbusmenu-Layout →
  Menü, Häkchen/Radio in `classic_menu`, Platz im Layout,
  `infobereich-ergaenzen/-zuruecknehmen`. Clippy `-D warnings` und
  `fmt --check` ohne Befund.
- **Szenario `infobereich`** (`sni-attrappe.py`, Python/Gio): Watcher und
  Alias, Host angemeldet, `ProtocolVersion`, ungültige/besitzerlose
  Anmeldung abgelehnt, leerer Infobereich unsichtbar, Item erscheint
  (Erkennungsfarbe im Bild), Tooltip, Linksklick → `Activate` mit Token,
  mittlere Taste, Mausrad ±120, Menü per Zeiger und Tastatur (`AboutToShow`,
  `GetLayout`, `AboutToShowGroup`, `opened`/`clicked`/`closed`), Häkchen,
  Radio im Untermenü, grauer Eintrag übersprungen, versteckter fehlt, Item
  verschwindet, NeedsAttention/Passive/Active, Symbolwechsel, zweites Item
  (per Objektpfad angemeldet, ohne Menü → `ContextMenu`), Appletmenü am Rand,
  ItemIsMenu, fehlendes `Activate` → Menü, `IconThemePath`, unbekannter
  `IconName` → Pixmap, Watcher-Neustart (Panel neu, Items melden sich neu
  an), kein CRITICAL: de/en × 1024/1920 × hell/dunkel je **63/63**.
- **Szenario `fremd`** (`fremd-watcher.py`): fremder Watcher zuerst, Panel
  nur Host, kein Alias ohne KDE-Namen, Items des fremden Watchers sichtbar,
  Übernahme nach dessen Ende: **19/19**.
- **Szenario `echt`** mit libayatana-appindicator3 + libdbusmenu-gtk3
  (`appindicator-probe.py`, echte Bibliothek über ctypes, in bwrap mit
  eigenem PID-/Netz-Namensraum, eigenem `/tmp` und `/dev/shm`): Item,
  Themensymbol `mail-unread`, Menü, Eintrag, Häkchen, Untermenü kommen beim
  Programm an, Linksklick (das Programm hat kein `Activate` → Menü): hell und
  dunkel je **13/13**.
- **Webex** im selben Rahmen: startet, zeigt im frischen Profil zuerst die
  Lizenzzustimmung und eine Schlüsselbund-Abfrage, meldet vor Zustimmung und
  Anmeldung kein Item an. Zustimmen und Anmelden gehören pottrauschen; die
  Webex-Abnahme steht in der Übergabe.
- **Szenario `alt`**: laufendes Panel 0.3.2-2 (Live-Ablage, isoliert
  gestartet) bekommt `infobereich-ergaenzen`: liest neu, überspringt den
  unbekannten Typ, baut nichts um, läuft weiter; `-zuruecknehmen` gibt die
  Datei byte-gleich zurück: **11/11**.
- **Panel-Regression:** Menü-GUI-Test (`menu_gui_test`) und Symbol-GUI-Test
  der Bibliothek bestanden; Fotolauf de/en × 1024/1920 × hell/dunkel mit
  0.4.0 (panel.ini wie nach `aktivieren`) und 0.3.2-2 je 14/1 – der eine
  FAIL („Files-Symbol“) ist bei beiden gleich, weil seit der Ausmusterung
  von Files (24.10.) kein Files-Starter mehr da ist (Umgebung, kein
  Panelfehler). Pixelvergleich: in allen acht Paaren gleich bis auf die
  Versionszeile im Info-Dialog („Panel 0.3.2“ → „Panel 0.4.0“) und in zwei
  Paaren die Reihenfolge der zwei Testfenster in der Fensterliste (wie bei
  0.3.1 dokumentiert); oberes Panel und Menübereich gleich. Live-Tausch
  0.3.2-2 ⇄ 0.4.0 mit `tausch-panel-3.py` (Prüfkopie, Überwachung,
  Absturz, Rückfall): **31/31**.
- **Integrationsprobe** `probe.sh` (umgelenkte Pfade): **85/85**.

Nach dem Merge von main `6d95ff9` (it/es/fr) mit dem Release-Bau
`dfc6bef1…` (zweimal gebaut, byte-gleich) wiederholt: `infobereich` de 1024
hell und en 1920 dunkel je 63/63, `fremd` 19/19, `alt` 11/11, `echt`
(AppIndicator) 13/13, Fotos de 1024 hell und en 1920 dunkel je 14/1 (wie
oben), Pixelvergleich gleich bis auf Versionszeile und Testfenster, Tausch
31/31, Menü-GUI-Test bestanden, Probe 85/85 (`nachweis/kette-r3.txt`,
`kette-reg3.txt`, `bildvergleich-r3.txt`).

## 5. Grenzen

- XEmbed-Tray (alte X11-Programme ohne StatusNotifierItem) gibt es nicht;
  solche Programme bleiben unsichtbar (z. B. über `snixembed` nachrüstbar,
  nicht Teil von 0.4.0).
- `AttentionMovieName` (Animation) wird nicht abgespielt; es gilt das
  Aufmerksamkeitssymbol.
- Koordinaten in `Activate`/`ContextMenu` sind immer (0, 0): unter Wayland
  kennt das Panel keine globalen Bildschirmkoordinaten. Programme, die ihr
  eigenes Menü per `ContextMenu` an diese Stelle setzen, zeigen es dort, wo
  der Compositor es hinlegt.
- Ein Menü, das sich ändert, während es offen ist, zeigt die Änderung erst
  beim nächsten Öffnen.
- Passive Items sind immer verborgen; ein Aufklappknopf für versteckte
  Symbole fehlt.
