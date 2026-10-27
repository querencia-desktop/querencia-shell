# Änderungen – querencia-dateiarbeit

## 0.1.3 – 2026-10-27

Für Rückgängig in Querencia Dateien 0.3.0:

- `Outcome::undo` nennt je gewähltem, erledigtem Objekt Quelle und Ankunftsnamen, wenn es sich einfach zurücknehmen lässt: nichts am Ziel ersetzt oder zusammengeführt, beim Verschieben kein neuer Name und nicht in den eigenen Ordner, beim Papierkorb wirklich im Papierkorb (nicht nach Rückfrage gelöscht). Test `undo_lists_only_objects_that_can_simply_go_back`.

## 0.1.2 – 2026-10-26

Italienisch, Spanisch, Französisch:

- Kataloge `po/it.po`, `po/es.po`, `po/fr.po` (je 71 Texte): Fehlermeldungen der Dateioperationen und Laufwerke; Wortwahl nach der gemeinsamen Querencia-Begriffstabelle.

## 0.1.1 – 2026-10-23

Prüfstand und Freigabe (Status-Session):

- Tests ohne Klarnamen und Heimnetz-Namen (Laufwerks-Testdaten neutral).
- Der Namensraum-Test läuft auch in der Prüfrahmen-Hülle: dort ohne
  eigenes `unshare` auf deren `--tmpfs`-Ordnern (`grenze`, `voll`,
  `ohne-papierkorb`), sonst meldet er „NICHT GEMESSEN“. Die Markierung
  „kein Papierkorb“ legt der Test selbst an (`.Trash-<uid>`).
- main mit dem Prüfrahmen (`werkzeug/dateien-pruefrahmen/`) zusammengeführt.

## 0.1.0 – 2026-10-23

Erste Fassung (Welle A von „Querencia Dateien“, QD-140; noch von keinem
Programm genutzt, Desktop 0.3.0 stellt um):

- Aus dem Desktop übernommen, ohne GTK: URI-Listen und
  Zwischenablage-Formate, Kopien- und Neu-Namen, Namensprüfung und
  Starter-Namen, Typ nach dem Namen, Symbole, Laufwerks-Einordnung,
  Fehlertexte, Programme auf einem Datenträger, Vertrauensregel für
  Starter, Warten auf einen fehlenden Ordner (QD-112).
- Neu: Aufträge mit Warteschlange, Fortschritt (Vorlauf und
  `g_file_copy`-Rückruf), Abbruch je Auftrag und Konfliktfrage
  (Ersetzen, Überspringen, Umbenennen, Zusammenführen, für alle);
  Teilobjekte nur unter Baunamen, Ersetzen über `RENAME_EXCHANGE`,
  Verschieben über Dateisystemgrenzen erst nach vollständiger Kopie.
- Neu: Einhängen nicht eingehängter Volumes, Liste aller Laufwerke,
  Papierkorb mit Zähler-Signal, Wiederherstellen und endgültigem Löschen
  einzelner Einträge, Ordnerüberwachung mit Bündelung, Öffnen-Plan mit
  `org.querencia.Dateien` an erster Stelle.
- Aus Querencia Files (MIT): Wiederherstellen aus dem Papierkorb,
  Einhängetabelle, Zusammenführen doppelter Netzwerkgeräte,
  Host-Key-Abruf und known_hosts-Abgleich.
- Textdomäne `querencia-dateiarbeit` mit `po/de.po` (71 Texte).
