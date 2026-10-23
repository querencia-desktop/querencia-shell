# Prüfrahmen Querencia Dateien – Bedienung und Regeln

**Stand:** 23.10.2026, `pruefrahmen/0.1.0` (QD-140 Welle A Schritt 3, QD-142).
**Nutzer:** Crate `querencia-dateiarbeit`, Desktop 0.3.0 (QD-144), später der Dateimanager Querencia Dateien.
**Grundlage:** Recherche C §10.2, W4-Übergabe Files, 13 §5 (Fallen, Nachtrag 21.10. 20:38 zu `env -i`).

## 1. Inhalt des Ordners

| Datei | Zweck |
|---|---|
| `huelle.sh` | startet einen Befehl in Stufe L oder G, prüft danach auf Reste |
| `attrappen.sh` | Laufwerks-Attrappen ohne sudo, je Attrappe `pruefen` |
| `attrappen/fstab` | fstab-Zeile der Attrappe „nicht eingehängt“ |
| `gio-sicht.py` | zeigt in der Hülle, was GIO an Volumes, Mounts und Dateisystemen sieht |
| `reste.py` | Restprüfung über `/proc` und `mountinfo` (auch allein nutzbar) |
| `beispiel-cargo-test.sh` | Beispiel: `cargo test -p …` bauen draußen, testen in der Hülle |
| `selbsttest.sh` | 29 Fälle, Protokoll in `nachweis/` |
| `nachweis/` | Ergebnisprotokolle des Selbsttests |

## 2. Stufen

| Stufe | GIO | D-Bus | wofür |
|---|---|---|---|
| **L** lokal | `GIO_USE_VFS=local`, `GIO_USE_VOLUME_MONITOR=unix` | Session-Bus ohne aktivierbare Dienste | Ordner, Operationen, Papierkorb über `$XDG_DATA_HOME/Trash`, Eigenschaften, Suche, Umbenennen, Attrappen |
| **G** GVfs | GVfs-Client, `GIO_USE_VOLUME_MONITOR=unix` | Session-Bus mit nur `gvfsd --no-fuse` und `gvfsd-metadata` | `trash:///`, `sftp://` gegen den eigenen sshd (`--sshd`), `network:///` (bleibt leer), Attrappen |
| H Hardware | – | – | echte udisks/USB/SMB nur auf der Probe-VM oder mit pottrauschens Zustimmung; diese Hülle deckt H nicht ab |

**Beide Stufen:**
- Start mit `env -i`. Gesetzt werden nur `HOME`, `USER`, `LOGNAME`, `LANG`, `LC_ALL`, `TERM`, `PATH`, `TMPDIR`, die vier `XDG_*_HOME`, `XDG_RUNTIME_DIR`, `XDG_DATA_DIRS`, `XDG_CONFIG_DIRS`, beide Busadressen, `GSETTINGS_BACKEND=keyfile`, `GIO_USE_VOLUME_MONITOR`, `GVFS_REMOTE_VOLUME_MONITOR_IGNORE`, `GTK_USE_PORTAL=0`, `NO_AT_BRIDGE`, `GTK_A11Y`, `QD_HUELLE`, `QD_PROFIL`, dazu `--setenv`. Keine `QUERENCIA_*`-Variable aus der Live-Umgebung.
- `bwrap` mit eigenem Nutzer-, PID-, Netz-, IPC- und UTS-Namensraum. Das Netz besteht nur aus `lo`, der Rechnername ist `qd-huelle`.
- **Unsichtbar:**
  - das echte HOME (tmpfs darüber; sichtbar bleiben nur Profil, Werkzeugordner und `--lesen`/`--schreiben`); damit sieht auch ssh kein `~/.ssh`.
  - `/run`: Live-Bus, `wayland-0`, Portale, gvfs-fuse, NAS-Mount, Avahi- und System-Bus-Socket.
  - `/tmp`, `/media` und `/mnt` als eigene tmpfs.
- **System-Bus:** Attrappe unter `/run/dbus/system_bus_socket` ohne Dienste (QD-142). Avahi, UDisks2, NetworkManager und logind gibt es dort nicht.
- **gsettings** aus `$XDG_CONFIG_HOME/glib-2.0/settings/keyfile`: wsdd `display-mode='disabled'` (QD-041), Schrift `'DejaVu Sans 12'` (PT-1).
- `XDG_RUNTIME_DIR=/tmp/qd-xxxxxx` (kurz wegen der 108-Byte-Grenze für Sockets), Rechte 0700.
- Das Profil liegt auf ext4 unter `~/.cache/qd-pruefrahmen/p-xxxxxx`. Mit `--profil DIR` liegt es dort und bleibt erhalten.
- **Ende:** Endet der Befehl, endet der PID-Namensraum. Der Kern beendet dann jeden übrigen Prozess, auch abgelöste (`setsid`, `nohup`) und AppArmor-begrenzte. Ein Signal des Kerns prüft AppArmor nicht.

## 3. Aufruf

```
huelle.sh [Optionen] L|G -- BEFEHL [ARG …]
```

| Option | Wirkung |
|---|---|
| `--profil DIR` | Profil dort anlegen und behalten (leer oder mit `.qd-profil`) |
| `--lesen PFAD` / `--schreiben PFAD` | Pfad nur lesend / schreibbar sichtbar machen (wiederholbar; `/`, `/usr`, `/etc` und HOME sind nicht schreibbar freigebbar) |
| `--ordner PFAD` | Arbeitsordner (Vorgabe: HOME im Profil); muss sichtbar sein |
| `--setenv NAME=WERT` | zusätzliche Variable |
| `--sprache LOCALE` | `LANG`/`LC_ALL` (Vorgabe `C.UTF-8`; für deutsche GVfs-Texte `de_DE.UTF-8`) |
| `--cargo` | `~/.cargo/bin` im PATH, `CARGO_HOME`/`RUSTUP_HOME` nur lesend, `RUSTC_WRAPPER` leer, `CARGO_NET_OFFLINE=true` |
| `--sshd` | nur G: Test-sshd auf `127.0.0.1:2222` im eigenen Netz-Namensraum, nur SFTP, Wegwerfschlüssel im Profil, ssh-Hülle vorn im PATH; `QD_SSH_URI` zeigt auf `sftp-freigabe/` im Profil |
| `--labwc` | headless labwc 0.9.8 (pixman) in der Hülle, `WAYLAND_DISPLAY=wayland-0` im eigenen `XDG_RUNTIME_DIR` |
| `--einhaengen Q:NAME` | ganzes Dateisystem `Q` nur lesend als `/media/NAME` |
| `--ramfs NAME` | schreibbares ramfs als `/media/NAME` |
| `--tmpfs PFAD[:GRÖSSE]` | tmpfs (Größe wie `1M`) relativ zum Profil oder unter `/media`, `/mnt` |
| `--fstab DATEI` / `--verzeichnis PFAD` | `LIBMOUNT_FSTAB` / leerer Einhängepunkt |
| `--zeitlimit S` | Abbruch nach S Sekunden (Vorgabe 600) |
| `--protokoll DATEI` | Hüllenprotokoll mit Restbericht (JSON) |
| `--wsdd-probe` | wsdd **nicht** abschalten (nur Selbsttest) |

**Exit:**
- Exit des Befehls.
- 90 Aufruffehler, 91 Aufbau gescheitert, 92 Isolation verletzt (Vorprüfung vor dem Befehl: mehr als `lo`, Live-Bus, Wayland, `~/.ssh` oder echter Papierkorb sichtbar), 93 Zeitlimit.
- 95: Reste gefunden. Laufzeit und Profil bleiben dann zur Untersuchung stehen.

**Restprüfung nach jedem Lauf** (`reste.py`, nur lesend). Als Rest gilt:
- ein Prozess mit dem eigenen `XDG_RUNTIME_DIR` in `/proc/<pid>/environ`;
- ein Prozess, dessen argv Laufzeit oder Profil nennt. So wird wsdd erkannt, dessen environ AppArmor sperrt: Sein `--listen` liegt im eigenen `XDG_RUNTIME_DIR`. wsdd ist ein Python-Skript, `comm` lautet „python3“, deshalb nach argv suchen.
- eine Einhängung unter Laufzeit oder Profil;
- eine neue `fuse.portal`-Einhängung.

Neue wsdd-Prozesse ohne Bezug zum Lauf meldet die Prüfung nur als Warnung (anderer Worker oder Live-Sitzung).

### Beispiele

```
H=~/projects/querencia-runtime-wt-pruefrahmen/werkzeug/dateien-pruefrahmen
# Tests eines Crates: bauen draußen (Bau-Platz), jedes Testbinary einzeln in der Hülle
bash $H/beispiel-cargo-test.sh ~/projects/querencia-runtime-wt-dateiarbeit L
bash $H/beispiel-cargo-test.sh ~/projects/querencia-runtime-wt-dateiarbeit G querencia-dateiarbeit -- --test-threads=1
# ganz in der Hülle (langsamer, Worktree schreibbar):
bash $H/huelle.sh --cargo --schreiben ~/projects/WT --ordner ~/projects/WT L -- cargo test -p querencia-dateiarbeit --offline
# Attrappen
bash $H/attrappen.sh alle pruefen G
bash $H/attrappen.sh grenze starten L -- /pfad/zum/testbinary
bash $H/huelle.sh $(bash $H/attrappen.sh alle optionen) --lesen ~/projects/WT L -- ~/projects/WT/target/debug/deps/…
# SFTP
bash $H/huelle.sh --sshd G -- sh -c 'gio mount "$QD_SSH_URI" </dev/null && gio list "$QD_SSH_URI"'
```

**Tests im Crate** können „nur in der Hülle“ erzwingen: `QD_HUELLE` muss gesetzt sein, sonst überspringen oder scheitern. Feste Pfade unter `~` sind in der Hülle unsichtbar. Dateisystemgrenzen kommen über `--tmpfs grenze[:GRÖSSE]` als `$QD_PROFIL/grenze`.

## 4. Laufwerks-Attrappen (`attrappen.sh`)

| Attrappe | Hüllenoption | GIO sieht | Grenze |
|---|---|---|---|
| `eingehaengt` | `--einhaengen <squashfs unter /snap>:QD-Stick` (sonst `/boot`) | GMount „QD-Stick“, `can_unmount=True`, nur lesend | Aushängen über GIO scheitert (umount braucht root). Mit `/boot` erscheint `/boot/efi` als zweites GMount „efi“: Im Nutzer-Namensraum sind geerbte Untereinhängungen gesperrt, und GIO liest auch verdeckte Einträge mit. `QD_ATTRAPPE_QUELLE` wählt die Quelle. |
| `schreibbar` | `--ramfs QD-Schreibbar` | GMount „QD-Schreibbar“, schreibbar, eigenes Dateisystem | Quelle heißt `/dev/qd-NAME`. Mit der Quelle „none“ blendet GIO den Mount aus. Keine Größengrenze und keine vfat-/ext4-Eigenheiten. Aushängen scheitert. |
| `nicht-eingehaengt` | `--fstab attrappen/fstab --verzeichnis /media/QD-Nicht` | GVolume „QD-Nicht“, `can_mount=True`, nicht eingehängt | Einhängen scheitert (setuid-`mount` unter `no_new_privs`); geprüft wird der Fehlerweg |
| `grenze` | `--tmpfs grenze:1M` | kein Mount (tmpfs gilt als systemintern), anderes `id::filesystem` | „Platte voll“ ab 1 MiB; Verschieben HOME → grenze kopiert und löscht |

`attrappen.sh ART pruefen [L|G]` belegt die Sicht mit `gio-sicht.py` und `gio mount -l` und prüft die Erwartungen der Tabelle. `ART` ist eine der vier Attrappen oder `alle`.

### Was ohne sudo nicht geht (für pottrauschen)

1. **Schreibbare Attrappe mit echtem Dateisystem** (vfat/ext4-Semantik, Größe, Aushängen): `fuse2fs` (Paket `fuse2fs`) bzw. FUSE im Namensraum. In der Hülle gibt es heute weder `/dev/fuse` noch setuid-`fusermount3`. ramfs ersetzt das nur für „schreibbares GMount“.
2. **SMB-Erfolgsfall:** `samba` als Testserver, am besten auf der Probe-VM. In der Hülle wäre er auf `127.0.0.1` erreichbar.
3. **udisks-/Geräte-Attrappen** (GDrive, Auswerfen, „kann entfernt werden“): `umockdev` (`umockdev-run` fehlt). Ohne ihn gibt es keine GDrive-Attrappe.
4. **Echtes USB** und Loop-Abbilder über `udisksctl`: systemweit sichtbar (G2-N-Falle 1), deshalb nur auf der Probe-VM mit USB-Durchreichung (Stufe H).
5. **Einhängen und Aushängen über GIO** (`mount`/`umount` setuid) schlagen in jeder unprivilegierten Hülle fehl. Den Erfolgsweg zeigt nur Stufe H.
6. **Thumbnailer für PDF und Video** (z. B. `ffmpegthumbnailer`, Papers-Thumbnailer): fehlen auf entwicklungs-vm, betrifft R5.
7. **Alte wsdd-Reste** außerhalb der Hülle (QD-041): `sudo apparmor_parser -R /etc/apparmor.d/usr.bin.wsdd` und `pkill`, oder Neustart. Die Hülle selbst erzeugt keine (siehe §5).

## 5. Fallen, die die Hülle abfängt

| Falle | Quelle | Abwehr |
|---|---|---|
| Live-Variablen in isolierten Läufen (`QUERENCIA_DESKTOP_RECORD`) | 13 §5 Nachtrag 21.10. 20:38 | `env -i`, Selbsttest `umgebung` |
| GTK erreicht `wayland-0` der Live-Sitzung | W4-Falle 1, 13 §5 20.10. 15:20 | eigenes `XDG_RUNTIME_DIR`, `/run` unsichtbar |
| wsdd-Reste unter AppArmor | QD-041 | `display-mode='disabled'`, nur `lo`, PID-Namensraum. Gemessen: wsdd stürzt in der Hülle sofort ab (Exit 139), gvfsd-wsdd meldet „exited unexpectedly“ |
| gvfsd-dnssd sieht das Heimnetz über Avahi | QD-142, n2-Vorfall | Attrappen-System-Bus, echter Socket unsichtbar |
| Portal-Dokumentlaufwerke „doc“ | QD-037 | kein Portal aktivierbar, `/run/user` leer, Restprüfung `fuse.portal` |
| ssh liest `~/.ssh` aus passwd, ControlPath > 108 Byte, `NoHostAuthenticationForLocalhost` | QF-0m | echtes HOME unsichtbar, ssh-Hülle (`QD_SSH_HOSTKEY_FRAGE=1`, `QD_SSH_OHNE_SCHLUESSEL=1`) |
| Test-sshd kollidiert mit anderen Workern auf 2222 | QF-0m | eigener Netz-Namensraum |
| schreibgeschützte Profildateien | W4-Falle 10 | `chmod -R u+w` vor dem Löschen |
| `/tmp` anderer Läufe sichtbar | – | eigenes `/tmp` |

**Nicht abgefangen:**
- `/sys` ist das des Hosts (nur lesbar).
- Ohne FUSE bekommen Programme ohne URI-Unterstützung für GVfs-Orte keinen Pfad (C §2, §8).
- Schrift und Thema unter `--labwc` sind nur über das keyfile fest; es gibt keinen Portal-Ersatz.

## 6. Regressionsregeln (aus den Files-Übergaben)

1. **A/B nur gegen einen frischen Lauf des Vorgängers vom selben Tag**, nie gegen alte Läufe (W4-Falle 5: Klickpunkte drifteten).
2. **Startwartezeit ≥ 15 s unter Last:** Ein frischer Build braucht beim ersten Start bis zu 12 s (W4-Falle 3). Danach auf ein Ereignis warten, nicht nur auf die Zeit.
3. **Auf Ereignisse warten statt `sleep`:**
   - Markerdateien, Logzeilen, `gdbus --timeout 10`, Dateisystem-Ergebnis (QF-0s);
   - Wartezeiten nur als Obergrenze (`timeout`, `read -t`).
4. **`grep -a`** auf Protokollen (Steuerzeichen, W4-Falle 8).
5. **Keine Warteschleifen mit `pgrep -f` und kein `pkill -f`:** Sie treffen die eigene Shell (13 §5, 20.10. 23:42). Prozesse nur nach PID (`wait`, `kill -0`) oder über `/proc/<pid>/exe` bzw. `/proc/<pid>/cmdline`.
6. **Papierkorb- und Löschfälle nur mit eigenem HOME** (QD-108). In der Hülle ist der echte Papierkorb unsichtbar; die Vorprüfung bricht mit 92 ab, falls er sichtbar wäre.
7. **Nie „Trotzdem anmelden“ mit echtem HOME;** Host-Key-Fälle nur mit `--sshd` und `QD_SSH_HOSTKEY_FRAGE=1`.
8. **Nie gegen NAS oder Heimnetzgeräte.** Netzfälle nur gegen `127.0.0.1` im eigenen Namensraum; SMB erst mit Testserver auf der Probe-VM.
9. **Keine Querencia-Programme in der normalen Shell starten** (kein `--help`, 13 §5 20.10. 22:08); GUI nur mit `--labwc`.
10. **Kein Snap-Firefox in Tests** (W4-Falle 13): eigene `mimeapps.list` im Profil, Testprogramme mit `%u`.
11. **Bauen unter dem Bau-Platz, Tests nicht** (13 §5 20.10. 13:40): `beispiel-cargo-test.sh` trennt beides.
12. **Nach jedem Lauf Exit prüfen:** 95 heißt Reste. Nichts liegen lassen, zuerst den Rest melden.
13. **Parallele Läufe** sind möglich (eigene Namensräume, eigener Port 2222 je Lauf), verlangsamen sich aber gegenseitig (W4-Falle 15).

## 7. Selbsttest

```
bash selbsttest.sh nachweis/selbsttest-$(date +%F).txt
```

Der Selbsttest hat 29 Fälle, Laufzeit etwa 40 s, und endet mit Exit 0 nur bei 29/29 und sauberem Aufräumen. Am Ende vergleicht er:
- die Zahl der wsdd-Prozesse und der `fuse.portal`-Einhängungen mit dem Ausgangswert,
- übrig gebliebene `/tmp/qd-*`,
- übrig gebliebene Wegwerfprofile.

Der Fall `heimnetz` liest das Standard-Gateway des Hosts. Er reicht es nur als Variable in die Hülle weiter und gibt es nie aus. Die Hülle prüft vorher, dass nur `lo` da ist, sonst versucht sie keine Verbindung.
