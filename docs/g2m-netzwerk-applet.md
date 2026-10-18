# G2-M – Netzwerk-Applet

Stand 19.10.2026 · Zweig `netzwerk-applet` (Basis `lautstaerke-applet`, Tag `panel/lautstaerke-0.1`) · Tags `panel/netzwerk-0.1` (Applet), `panel/netzwerk-0.2` (mit Teil 5/6)

Das Netzwerk-Applet zeigt die Netzwerkverbindung im Panel, nach dem Vorbild des GNOME-2-NetworkManager-Applets. Läuft NetworkManager, kann es verbinden und trennen. Ohne NetworkManager zeigt es nur an, was Kernel und systemd-networkd melden.

## Bedienung

| Aktion | Wirkung |
|---|---|
| Symbol | Kabel verbunden / Kabel getrennt, WLAN in vier Signalstufen, WLAN aus, verbindet …, kein Netz, kein Netzwerkdienst (ausgegraut). Eigene Querencia-Symbole (`crates/querencia-netzwerk/data/symbole/`, erzeugt von `tools/netzwerk-pruefung/symbole-machen.py`) |
| Tooltip | Verbindung und Adresse je verbundenem Gerät, die Standardverbindung zuerst; im Rückfall zusätzlich „Verwaltet von systemd-networkd, nur Anzeige“ |
| Linksklick | Menü in der klassischen Menüoptik: „Kabelnetzwerk“ mit Verbindung und Adresse, „Trennen“/„Verbinden“; „WLAN-Netze“ mit Signal und Schloss, verbundenes Netz fett mit Adresse, höchstens acht Netze, dann „Weitere Netze …“; „Mit verborgenem Netz verbinden …“; „WLAN aktiviert“; „Flugmodus“; „Verbindungsinformationen …“; „Netzwerkeinstellungen …“ |
| Tastatur im Menü | Runter/Hoch/Pos1/Ende wählen, Eingabe oder Leertaste lösen aus, Escape schließt |
| Rechtsklick | „WLAN aktiviert“, „Flugmodus“, „Verbindungsinformationen …“, „Netzwerkeinstellungen …“, „Aus dem Panel entfernen“ |
| Neues gesichertes WLAN | eigener Passwortdialog (WPA/WPA2 und WPA3 Personal; „Verbinden“ erst ab 8 Zeichen), dann `AddAndActivateConnection` mit dem Schlüssel |
| Gespeichertes WLAN ohne Schlüssel, falscher Schlüssel | NetworkManager fragt den Passwortagenten des Panels (`GetSecrets`), derselbe Dialog, beim zweiten Mal mit „wurde nicht angenommen“ |
| Verbindungsinformationen | Fenster mit einem Reiter je Gerät: Schnittstelle, Zustand, Verbindung, Hardware-Adresse, Treiber, Geschwindigkeit, bei WLAN Netz, Sicherheit, Band, Signal; IPv4 (Adresse, Netzmaske, Gateway, DNS) und IPv6; „Schließen“ hat den Tastaturfokus |
| Netzwerkeinstellungen | startet die Desktop-Aktion `netzwerk` von `org.querencia.Einstellungen.desktop` (G2-O soll sie liefern: `Exec=… querencia-einstellungen --netzwerk`). Fehlt sie, ist der Eintrag grau mit „(kommt mit G2-O)“. Das Panel folgt Installationen über `GAppInfoMonitor` |

Nicht vorhanden: WPA-Enterprise (802.1X), WEP, VPN, Mobilfunk, Hotspot, Verbindungen bearbeiten. Solche Netze stehen grau mit „nicht unterstützt“ im Menü.

## panel.ini

Neuer Applet-Typ `network`, keine neuen Schlüssel, **Standardlayout unverändert**. Einmal je Layout, entfernbar, eigenes Rechtsklickmenü. Über „Zum Panel hinzufügen …“ kommt es in den rechten Bereich und dort rechts neben ein vorhandenes Lautstärke-Applet (`layout::RIGHT_OF`), sonst an den Anfang wie jedes Applet. Von Hand:

```ini
[Panel top]
End=volume-1;network-1;clock-1

[Applet network-1]
Type=network
```

## Technik

Crate `crates/querencia-netzwerk` (Bibliothek und Diagnose `querencia-netzwerk --zustand | --folgen`). Neue externe Abhängigkeiten gibt es keine: D-Bus läuft über GIO aus gtk4, dazu `libc` und `glib-unix`, beide schon in Cargo.lock. zbus 5.19 liegt im Cargo-Cache, wird aber nicht benutzt: Es bräuchte einen eigenen Executor neben der GTK-Hauptschleife.

| Modul | Inhalt |
|---|---|
| `model.rs` | Zustand als reine Daten; Regeln für Symbol, Tooltip, Adress-, Geschwindigkeits- und MAC-Format, Netzliste, Verbindungsinformationen, Flugmodus |
| `nm.rs` | NetworkManager: ein `GetManagedObjects` auf `/org/freedesktop` liest alles; jedes Signal von `org.freedesktop.NetworkManager` löst nach 150 ms einen neuen Lesevorgang aus. Profile über `GetSettings`. Aktionen: `ActivateConnection`, `AddAndActivateConnection`, `Device.Disconnect`, `WirelessEnabled`/`WwanEnabled` setzen, `RequestScan`, alles mit `ALLOW_INTERACTIVE_AUTHORIZATION` (der polkit-Agent der Sitzung fragt bei Bedarf) |
| `agent.rs` | Passwortagent (`org.freedesktop.NetworkManager.SecretAgent`, angemeldet als `org.querencia.Panel.Netzwerk`): beantwortet nur WLAN-Schlüssel (`802-11-wireless-security`, `wpa-psk`/`sae`), nur Anfragen vom Besitzer des NetworkManager-Namens, speichert nichts selbst |
| `kernel.rs` | Rückfall, nur lesend: `/sys/class/net` (operstate, carrier, address, speed, Treiber; nur Hardware und WLAN), IPv4 über `getifaddrs`, `/proc/net/if_inet6`, `/proc/net/route`, `/proc/net/ipv6_route`, `/proc/net/wireless`, `/etc/resolv.conf` |
| `fallback.rs` | Rückfall zur Laufzeit: Netlink-Socket (Link, Adressen, Routen) weckt nur, dann wird nach 200 ms neu gelesen; systemd-networkd (`AdministrativeState` je Link) und systemd-resolved (`DNS` je Link) über ihre D-Bus-Eigenschaften, mit deren `PropertiesChanged`. Kein Polling |
| `service.rs` | ein Dienst je Prozess; **Wahl des Backends** über `g_bus_watch_name` auf `org.freedesktop.NetworkManager` (NameOwnerChanged): mit Besitzer NetworkManager und Agent, ohne Besitzer oder bei unlesbarem NetworkManager der Rückfall; Wechsel ohne Panel-Neustart |
| `icons.rs` | eingebaute SVG-Symbole, im Menü weiße Varianten der WLAN-Symbole für die orange Hervorhebung |

Panel: `network_applet.rs` (Applet, Menüzeilen als reine Funktion `rows`, Tastatur), `network_dialogs.rs` (Verbindungsinformationen, Passwort, verborgenes Netz, Meldung), ein Registry-Block in `applets.rs`, das Symbol in `add_dialog.rs`, das CSS in `main.rs`, die Platzregel in `layout.rs`.

Flugmodus: NetworkManager hat keinen eigenen Schalter. Das Applet setzt WLAN und Mobilfunk aus (`WirelessEnabled`, `WwanEnabled`). „Flugmodus an“ heißt: beides aus. Bluetooth bleibt unberührt.

`QUERENCIA_NETZWERK_DEBUG=1` schreibt jedes gebaute Menü Zeile für Zeile ins Log. `QUERENCIA_NETZWERK_WURZEL=<ordner>` liest den Rückfall aus einem Testbaum statt aus `/sys` und `/proc` (siehe `kernel.rs`).

## Prüfung

- Unit-Tests: `cargo test --offline --workspace`
- Isolierte GUI-Prüfung: `tools/netzwerk-pruefung/laeufe.sh <panel> [1024x768 1920x1080]`, Auswertung je Lauf durch `auswerten.py`. Die Läufe:
  - `rueckfall`: echter Systembus, echte eth0, nur lesend;
  - `attrappe`: eigener Bus mit der NetworkManager-Attrappe `qnm.py`, Testbefehle über `qnmctl.py`;
  - `spaet`: die Attrappe startet erst während des Laufs.
- G2-J- und W2-Regression: `tools/netzwerk-pruefung/g2jreg-anlegen.sh` legt eine umgelenkte Kopie der G2-J-Werkzeuge an, dann `nachweis/gui/g2jreg/werkzeug/w2reg-anlegen.sh` und `…/endlaeufe.sh <panel> <lautstärkeregler>`.

## Grenzen

- Auf der Entwicklungs-VM läuft systemd-networkd und kein NetworkManager. Echt geprüft ist deshalb nur der Rückfall. Der NetworkManager-Weg ist gegen die Attrappe geprüft, nicht gegen echtes NetworkManager. Die Liste für echte Hardware steht im Bericht .
- WLAN im Rückfall: Netznamen in Reichweite meldet der Kernel ohne nl80211 nicht. Das Menü zeigt dann nur das Gerät, die Adresse und die Verbindungsqualität aus `/proc/net/wireless`.

## Teil 5 und 6 (unabhängig vom Netzwerk-Applet)

Beide Teile stehen in eigenen Commits. Sie gibt es auch eigenständig auf dem Zweig `applets-verschieben` (Basis G2-J 75a1eba, Tag `panel/verschieben-0.1`).

**Applets verschieben (Teil 5), in beiden Panels:**

- Jedes Applet-Menü endet mit „Verschieben“, „Nach links“, „Nach rechts“, „An den linken Rand“ und „An den rechten Rand“.
  - Ein Schritt tauscht mit dem Nachbarn. Am Rand eines Bereichs wechselt das Applet in den Nachbarbereich (links, Mitte, rechts), auch in eine leere Mitte.
  - Einträge ohne Wirkung sind grau.
  - Anwendungen, Orte, System, Fensterliste und Umschalter haben sonst kein Menü; sie bekommen eines nur mit diesen Einträgen.
- **Ziehen mit gedrückter mittlerer Taste** verschiebt live.
  - Der Bereich folgt dem Drittel unter dem Zeiger, die Stelle den Mitten der Nachbar-Applets.
  - Beim Loslassen wird gespeichert; bei einem Fehler geht das Applet zurück.
  - Die Fensterliste lässt sich auch auf ihrer freien Fläche greifen.
  - Ein mittlerer Klick ohne Ziehen bleibt beim Applet: Die Lautstärke schaltet beim Loslassen stumm.
- **Alt + Linksklick** ist eingebaut, kommt auf labwc aber nicht an. Das Panel hat `KeyboardMode::None` und bekommt nie den Tastaturfokus, und Wayland meldet den Zustand der Umschalttasten nur dem fokussierten Programm. GTK erfährt also nie, dass Alt gedrückt ist.
- **Ersatz für Alt + Linksklick:** „Verschieben“ wie in GNOME 2. Das Applet folgt dem Zeiger über seinem Panel, ein Linksklick setzt es, ein Rechtsklick bricht ab.
- **„Zum Panel hinzufügen …“** zeigt unten links „Bereich: Links/Mitte/Rechts“, vorbelegt mit dem angeklickten Drittel, und lässt den Bereich wählen.
- **Speicherung:** `panel.ini` wie bisher, mit den Listen `Start=`, `Center=` und `End=` je Panel. Nach einem Neustart steht alles wieder so.

**Symbolthema neu einlesen (Teil 6, Befund SW-6):** GTK sieht neue Symboldateien in bestehenden Themenordnern nicht. Das Panel überwacht deshalb `~/.local/share/icons` samt Themen-, Größen- und Kontextordnern sowie die installierten Anwendungen. 400 ms nach einer Änderung gibt es GTK denselben Suchpfad erneut. Ein nachträglich abgelegtes Programmsymbol erscheint so ohne Neustart.

Prüfung: `LAEUFE=verschieben bash tools/netzwerk-pruefung/laeufe.sh <panel>` (Prüffall V00–V14 samt Neustart, ohne Netzwerk-Applet). A/B für Teil 6: `szenario-machen.py <b> <h> symbol`.
