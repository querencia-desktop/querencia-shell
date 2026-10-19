# QE-04 – Symbole der Arbeitsfläche (QD-042, G2-X)

**Stand:** 19.10.2026 · Desktop QE-04 (Zweig `desktop-volumes` ab `main` 19357f2) liest `desktop.ini` `[Icons]` und `[Volumes]`.
Schreiber ist Querencia Einstellungen ab 0.0.4 (Bereich „Symbole der Arbeitsfläche“, Repo `~/projects/querencia-einstellungen`).

## Was QE-04 gegenüber QE-03 ändert

| Stelle | Änderung |
|---|---|
| `config.rs` | `IconSettings` + `parse_icons` (Standardwerte nur hier, gleich `Arbeitsflaeche::default()` der Einstellungen); `replace_group` |
| `volumes.rs` | `classify` → `VolumeType` (Network > Optical > Removable > Internal, §3.1 unten) statt `shown(facts)`; GVfs-Netzfreigaben ohne GIO-Volume werden aufgenommen; Bild `Picture::Network` (Thema `folder-remote`) |
| `desktop_view.rs` | `load_items` blendet Rechner/Persönlicher Ordner/Papierkorb nach `[Icons]` aus (ihre Plätze bleiben in `desktop-symbole`); Symbolgröße 32/48/72/96 mit Zelle `cell_for` (48 = HMP-10b-Zelle 100×88); `apply_settings` baut die Symbolschicht bei Änderung sofort neu; der HMP-09-Rückfall „Hintergrund ändern“ (Dateidialog) ersetzt nur noch `[Background]` |
| `main.rs` | `load` gibt `[Icons]/[Volumes]` an die Symbolschicht; `--faehigkeiten` zusätzlich `volumes-types icons-visible icon-size`; neu `--datentraeger-zeigen` (nur lesen: jeder Mount mit Typ und ob er ein Symbol bekäme) |

Unverändert: Notbremsen `QUERENCIA_DESKTOP_VOLUMES=0` und `QUERENCIA_DESKTOP_ICONS=0`; Plätze der Datenträger nur im Speicher; Menü Öffnen/Auswerfen/Aushängen (Netzfreigaben: „Aushängen“).

---

**Stand:** 19.10.2026 · **Version des Formats:** 1 · **Schreiber:** Querencia Einstellungen ab 0.0.4 (Bereich „Symbole der Arbeitsfläche“, direkt unter „Hintergrundbild“; 0.0.3 hieß er noch „Arbeitsfläche“) · **Leser:** Querencia Desktop ab QE-04 (Zweig `desktop-volumes`).

Diese Datei legt fest, wie Querencia Einstellungen und Querencia Desktop die Arbeitsflächen-Symbole in
`~/.config/querencia/desktop.ini` (genauer: `$XDG_CONFIG_HOME/querencia/desktop.ini`) austauschen.
Der Hintergrund (`[Background]`, QE-01) bleibt wie er ist. Er ist in `crates/querencia-desktop/src/config.rs`
(Runtime) und in `src/modell.rs` (Einstellungen) beschrieben.

## 1. Beispiel

```ini
[Background]
File=~/Bilder/berge.jpg
Mode=zoom
Color=#1c4e96

[Icons]
Computer=true
Home=true
Trash=true
Size=48

[Volumes]
Removable=true
Optical=true
Network=true
Internal=false
```

## 2. Gruppe `[Icons]` – feste Symbole und Symbolgröße

| Schlüssel | Werte | Standard | Bedeutung (Beschriftung in den Einstellungen) |
|---|---|---|---|
| `Computer` | Wahrheitswert | `true` | Symbol „Rechner“ |
| `Home` | Wahrheitswert | `true` | Symbol „Persönlicher Ordner“ |
| `Trash` | Wahrheitswert | `true` | Symbol „Papierkorb“ |
| `Size` | `32` \| `48` \| `72` \| `96` (Pixel) | `48` | Symbolgröße: Klein, Normal, Groß, Sehr groß. Das sind die Nautilus-Stufen 66/100/150/200 %. |

Die Symbole von Dateien und Ordnern in `~/Schreibtisch` sind keine festen Symbole. Sie lassen sich nicht abschalten
(wie bei GNOME 2), aber `Size` gilt auch für sie und für die Datenträger-Symbole.

## 3. Gruppe `[Volumes]` – eingehängte Laufwerke

| Schlüssel | Standard | Typ (Beschriftung in den Einstellungen) |
|---|---|---|
| `Removable` | `true` | Wechseldatenträger (USB-Sticks, Speicherkarten, externe Festplatten, Telefone/Kameras) |
| `Optical` | `true` | Optische Medien (CD, DVD, auch ein eingelegtes ISO wie `cidata`) |
| `Network` | `true` | Netzlaufwerke (eingehängte Freigaben: SMB, SFTP, FTP, WebDAV, NFS, AFP) |
| `Internal` | `false` | Eingebaute Laufwerke (interne Festplatten und Partitionen, die der Benutzer eingehängt hat) |

> **Standardwerte entschieden am 19.10.2026** (Status-Session, vom Besitzer übertragen), wie GNOME 2:
> Wechseldatenträger, optische Medien und Netzlaufwerke an, eingebaute Laufwerke aus; Rechner, Persönlicher
> Ordner und Papierkorb an; Größe 48 (Stufen 32/48/72/96).
> Sie stehen an genau einer Stelle je Programm: `Arbeitsflaeche::default()` in
> `src/arbeitsflaeche_modell.rs` (Einstellungen) und `IconSettings::default()` in `config.rs` (Desktop). Ändern sich
> die Standards, müssen beide Stellen und diese Tabelle gleichzeitig geändert werden.

### 3.0 Was das auf der VM entwicklungs-vm bedeutet (gemessen 19.10.2026, `gio mount -li`, `findmnt`)

| Mount | GIO-Fakten | Typ (§3.1) | QE-03 heute | mit QE-04 und Standardwerten |
|---|---|---|---|---|
| `cidata` → `/run/media/user/cidata` (`/dev/sr0`, iso9660) | Volume Klasse `device`, Laufwerk „QEMU DVD-ROM“ removable, auswerfbar, Symbol `media-optical-cd-rom` | Optical | sichtbar | **bleibt sichtbar** |
| `user on nas-test.local` → `smb://nas-test.local/user/` | GDaemonMount ohne Volume, aushängbar, Symbol `folder-remote` | Network | nicht sichtbar | **erscheint neu** |
| `/`, `/boot`, `/boot/efi` (`/dev/sda1`, `sda13`, `sda15`) | Systempfade; udisks meldet für `sda` keine Volumes | nie | nicht sichtbar | nicht sichtbar |
| selbst eingehängte eingebaute Partitionen | **gibt es auf der VM nicht** (keine weiteren sda-Partitionen eingehängt) | Internal | wären sichtbar (`can_unmount`) | würden **verschwinden** (Internal aus) – heute betrifft das nichts |
| `/run/user/1001/doc` (Portal), `/run/user/1001/gvfs` und GVfs-FUSE-Reste isolierter Läufe | Portal/FUSE-Hilfsmounts | nie | nicht sichtbar | nicht sichtbar |

### 3.1 Einordnung eines eingehängten Laufwerks (für den Desktop, verbindlich)

Grundlage sind die GIO-Fakten, die QE-03 schon sammelt (`volumes::Facts`). Die Regeln werden **in dieser
Reihenfolge** geprüft, die erste passende gilt:

1. **Nie gezeigt** (unabhängig von `[Volumes]`, wie QE-03): beschattete Mounts (`shadowed`), Systempfade
   (`/`, `/home`, `/root`, `/tmp`, `/boot`, `/usr`, `/var`, `/run` außer `/run/media` …, siehe `is_system_path`),
   das Dokumentportal (`…/doc`, `fuse.portal`), Archive (`archive://`) und virtuelle Orte (`trash:`, `recent:`,
   `burn:`, `computer:`, `network:`, `admin:`).
2. **Network**: Die Wurzel-URI hat das Schema `smb`, `sftp`, `ssh`, `ftp`, `ftps`, `dav`, `davs`, `nfs` oder `afp`.
   Dasselbe gilt, wenn die GIO-Volume-Klasse `network` ist. GVfs-Netz-Mounts haben meist **kein** GIO-Volume;
   das schließt sie hier nicht aus (anders als in QE-03).
3. **Optical**: Das Symbolbild ist `Optical` (Symbolname enthält `optical` oder `cdrom`).
4. **Removable**: Das Laufwerk ist `removable`, `media_removable` oder auswerfbar, oder das Symbolbild ist
   `Removable`. Darunter fallen auch `mtp://` und `gphoto2://` (Telefone, Kameras). Ein verschlüsseltes
   Laufwerk zählt nach seinem Gerät: auf einem USB-Stick als Removable, sonst als Internal.
5. **Internal**: alles Übrige mit lokalem `file://`-Pfad, das der Benutzer aushängen kann (`can_unmount`).
   Das sind Partitionen interner Platten unter `/run/media/<benutzer>` oder `/media`.
6. Sonst (weder aushängbar noch auswerfbar): nicht gezeigt, wie in QE-03.

Ein Laufwerk erscheint genau dann, wenn der Schlüssel seines Typs `true` ist. Wird ein Kästchen
abgeschaltet, verschwinden die Symbole dieses Typs sofort. Ihre Plätze waren ohnehin nur flüchtig (QE-03).

Die Notbremse `QUERENCIA_DESKTOP_VOLUMES=0` (Umgebung) schaltet weiter **alle** Datenträger-Symbole ab und
geht der Datei vor. Dasselbe gilt für `QUERENCIA_DESKTOP_ICONS=0`, das alle Symbole abschaltet.

## 4. Lesen

- Beide Programme lesen tolerant wie bei `[Background]`. Fehlt die Datei, die Gruppe oder ein Schlüssel,
  gilt der Standard. Unbekannte Schlüssel und ungültige Werte führen zum Standard des Schlüssels plus einer
  Protokollzeile („unbekannter Schlüssel …“, „ungültiger Wert …“). Nie bricht ein Programm ab.
- Wahrheitswerte: `true`/`false`, außerdem `1`/`0` und `yes`/`no`, Groß- und Kleinschreibung egal.
  Geschrieben wird immer `true`/`false` (wie GLib `GKeyFile`).
- `Size`: nur die vier Werte. Alles andere ergibt `48` mit Protokollzeile.
- Kommentare (`#`) und Leerzeilen sind erlaubt. Kommt eine Gruppe doppelt vor, zählt jede Zeile, die
  spätere gewinnt (wie `GKeyFile`).
- **Ältere Desktops** (HMP-10a bis QE-03) lesen nur `[Background]`, alle anderen Gruppen überspringen sie
  stumm (`config::parse`, Test `keys_of_other_groups_are_ignored`). Die neuen Gruppen sind für sie also
  wirkungslos und harmlos.

## 5. Schreiben – jede Stelle schreibt nur ihre eigenen Gruppen

1. Ein Schreiber ersetzt **nur seine Gruppen**. Der Hintergrund-Dialog ersetzt `[Background]`, der Bereich
   „Symbole der Arbeitsfläche“ ersetzt `[Icons]` und `[Volumes]`. Alles andere bleibt **byte-gleich**: andere Gruppen,
   Kommentare vor der ersten Gruppe, Reihenfolge.
2. Zum Block einer Gruppe gehören die Kopfzeile und alle Zeilen bis zur letzten Schlüsselzeile vor der
   nächsten Kopfzeile. Kommentar- und Leerzeilen direkt vor der nächsten Gruppe gehören zu dieser. Eine
   doppelte Gruppe wird beim Ersetzen zu einer zusammengefasst.
3. Fehlt die Gruppe, wird sie am Ende angehängt, mit einer Leerzeile Abstand. Eine leere oder fehlende
   Datei ergibt nur die eigenen Gruppen. Der Hintergrund-Dialog schreibt dann genau dieselben Bytes wie
   bisher (`[Background]\nFile=\nMode=zoom\nColor=#1c4e96\n`).
4. Die Einstellungen schreiben ihre Gruppen immer **vollständig**, mit allen Schlüsseln in der Reihenfolge
   der Tabellen oben.
5. Geschrieben wird atomar (`desktop.ini.neu`, dann `rename`). Die Einstellungen sichern eine vorhandene
   Datei vorher als `desktop.ini.vorher-<JJJJMMTT-hhmmss>`, wie QE-01.
6. „Nicht unbemerkt überschreiben“: Jeder Schreiber vergleicht **seine** Gruppen mit dem Stand beim Öffnen.
   Hat jemand anderes sie inzwischen geändert, wird nichts geschrieben und eine Meldung erscheint.
   Änderungen an fremden Gruppen bleiben dagegen einfach erhalten (Punkt 1).
7. **Desktop (ab QE-04):** Der Rückfallweg „Hintergrund … ändern“ ohne Einstellungen (Dateidialog, HMP-09)
   ersetzt ebenfalls nur `[Background]`. QE-03 und älter schreiben dort noch die ganze Datei. Mit ihnen
   gehen `[Icons]`/`[Volumes]` verloren, aber nur, wenn die Einstellungen fehlen.

## 6. Wirksamkeit und Fähigkeiten

- Der Desktop überwacht `desktop.ini` schon heute (`watch_config`). Ab QE-04 liest er bei jeder Änderung
  auch `[Icons]` und `[Volumes]` und baut die Symbolschicht neu. Es braucht keinen Neustart.
- `querencia-desktop --faehigkeiten` gibt ab QE-04 zusätzlich diese Wörter aus:
  `volumes-types` (die vier Typen aus §3), `icons-visible` (die drei Kästchen aus §2), `icon-size` (`Size`).
- Fehlt ein Wort, zeigen die Einstellungen im Bereich „Symbole der Arbeitsfläche“ einen Hinweis: „Der laufende Desktop
  wertet … noch nicht aus“. Übernehmen bleibt trotzdem möglich, weil die Gruppen alte Desktops nicht stören
  (§4). Die Werte gelten, sobald ein passender Desktop läuft.

## 7. Prüfpunkte (für beide Seiten)

| # | Fall | Erwartung |
|---|---|---|
| F1 | keine Datei | alle Standards; Desktop wie QE-03, außer Netz/eingebaut laut §3 |
| F2 | nur `[Background]` (QE-01-Datei) | Hintergrund unverändert; Symbole nach Standard |
| F3 | Einstellungen „Symbole der Arbeitsfläche“ übernimmt auf F2 | `[Background]` byte-gleich, `[Icons]`/`[Volumes]` angehängt |
| F4 | Hintergrund-Dialog übernimmt auf F3 | `[Icons]`/`[Volumes]` byte-gleich |
| F5 | `Size=50`, `Removable=vielleicht`, `Foo=1` | Standard je Schlüssel, je eine Protokollzeile, kein Abbruch |
| F6 | QE-03-Desktop mit F3-Datei | Hintergrund wie in F2, keine neue Protokollzeile, Verhalten wie QE-03 |
| F7 | Kästchen „Netzlaufwerke“ an/aus | SMB-Mount erscheint/verschwindet ohne Neustart (ab QE-04) |
