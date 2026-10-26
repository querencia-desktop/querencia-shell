# Panel 0.4.0 – Infobereich (StatusNotifierItem)

Stand 26.10.2026. Zweig `infobereich` ab `main` `6935c48`. Punkt QD-164
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
`querencia-oberflaeche` 0.1.2, Attribute `x-querencia-toggle`/`-toggled`),
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

(wird nach den Läufen ergänzt)

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
