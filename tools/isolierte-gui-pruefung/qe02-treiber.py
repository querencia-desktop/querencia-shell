#!/usr/bin/env python3
"""QE-02: Prüftreiber für das Papierkorb-Symbol voll/leer.

Läuft wie qe01-treiber.py in der isolierten labwc-Sitzung (pruefen-hmp10.sh
mit HMP10_TREIBER=…/qe02-treiber.py) und benutzt die Hilfsfunktionen von
hmp10a-treiber.py (Papierkorb des Testprofils über GVfs, Kontextmenü,
Sicherheitsabfrage) und hmp10-treiber.py (Eingaben, Lage, Bilder).

Nur der Papierkorb des Testprofils (XDG_DATA_HOME im Profil) wird benutzt.
Schutz wie HMP-10b: Er muss zu Beginn leer sein, es darf keinen weiteren
Papierkorb auf eingehängten Datenträgern geben, und vor jedem Leeren muss
er genau die eigenen Testobjekte enthalten; sonst BLOCKED. Der echte
Papierkorb des Besitzers wird vorher und nachher verglichen.

Geprüft wird über das Protokoll des Desktops (QUERENCIA_DESKTOP_DEBUG=1:
„Papierkorb-Symbol: …“, „Papierkorb gezählt: …“, „Papierkorb jetzt …“,
„n Symbole: …“ bei jedem Neuaufbau) und am Bildschirmfoto (Pixel der
Symbolfläche).

Ausgabe: logs/pruefung.txt, logs/bilder/*.png, logs/zeiten.txt.
"""

import importlib.util
import os
import shutil
import subprocess
import time
import traceback

HIER = os.path.dirname(os.path.abspath(__file__))
_spec = importlib.util.spec_from_file_location("a", os.path.join(HIER, "hmp10a-treiber.py"))
a = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(a)
t = a.t

P, LOGS = t.P, t.LOGS
d, pruefe, bild, lage, warte, nah = t.d, t.pruefe, t.bild, t.lage, t.warte, t.nah
MUELL = os.path.join(P, "testmuell")
ZEITEN = []
BILDER_ZUSTAND = {}


def q(*befehle):
    t.q(*befehle)


def zaehle(text):
    return d.text().count(text)


def neuaufbauten():
    """Zeilen „n Symbole: …“ = Neuaufbauten aller Symbole (set_items)."""
    return sum(1 for z in d.text().splitlines() if " Symbole: " in z and z.startswith("querencia-desktop: "))


def symbol():
    return a.papierkorb_symbol()


def wegwerfen(name, inhalt="x\n"):
    pfad = os.path.join(MUELL, name)
    with open(pfad, "w") as f:
        f.write(inhalt)
    a.wegwerfen(pfad)


def endgueltig_loeschen(name):
    """Wie „Endgültig löschen“ in einem Dateimanager: trash:///<name> über GVfs."""
    a.Gio().File.new_for_uri("trash:///" + name).delete(None)


def zeit_bis(bedingung, sekunden=5):
    """Sekunden, bis bedingung() wahr ist (Abfrage alle 20 ms), sonst None."""
    start = time.monotonic()
    while time.monotonic() - start < sekunden:
        if bedingung():
            return time.monotonic() - start
        time.sleep(0.02)
    return None


def symbolpixel():
    """Bild und Lage des Papierkorb-Symbols (48×48 über der Beschriftung)."""
    pfad = os.path.join(LOGS, "jetzt.png")
    subprocess.run(["grim", pfad], check=False)
    im = t.Image.open(pfad).convert("RGB")
    x, y = t.bildschirm(*lage()["Papierkorb"])
    return im.crop((x + 26, y + 2, x + 74, y + 50))


def unterschied(b1, b2):
    w, h = b1.size
    return sum(not nah(b1.getpixel((i, j)), b2.getpixel((i, j)), 30)
               for i in range(0, w, 2) for j in range(0, h, 2))


def nur_eigene(erwartet):
    jetzt = a.papierkorb()
    if jetzt != sorted(erwartet):
        pruefe("BLOCKED: unerwarteter Inhalt im Papierkorb – nichts geleert", False, jetzt)
        return False
    return True


# ------------------------------------------------------------------ Fälle

def test_v0_schutz():
    isoliert = os.environ.get("XDG_DATA_HOME", "")
    pruefe("V0 eigener Papierkorb im Testprofil (XDG_DATA_HOME)", isoliert.startswith(P + "/"), isoliert)
    start = a.papierkorb()
    pruefe("V0 isolierter Papierkorb ist zu Beginn leer", start == [], start[:5])
    fremd = [m for m in open("/proc/mounts").read().split() if m.startswith("/") and
             (os.path.exists(os.path.join(m, ".Trash-1001")) or os.path.exists(os.path.join(m, ".Trash")))]
    pruefe("V0 keine weiteren Papierkörbe auf Datenträgern", not fremd, fremd)
    if not (isoliert.startswith(P + "/") and start == [] and not fremd):
        raise RuntimeError("BLOCKED: Papierkorb nicht isoliert oder nicht leer")
    os.makedirs(MUELL, exist_ok=True)


def test_v1_start_leer():
    pruefe("V1 Start mit leerem Papierkorb: Symbol „leer“", symbol() == "leer", symbol())
    t.protokoll(f"V1 Neuaufbauten beim Start: {neuaufbauten()}")
    BILDER_ZUSTAND["leer"] = symbolpixel()
    bild("v1-papierkorb-leer")


def test_v2_kein_abfragen():
    """Ohne Änderung zählt der Desktop nichts nach (keine Abfrage im Takt)."""
    n0, r0 = zaehle("Papierkorb gezählt:"), neuaufbauten()
    time.sleep(12)
    n1, r1 = zaehle("Papierkorb gezählt:"), neuaufbauten()
    pruefe("V2 12 s Ruhe: keine Zählung des Papierkorbs (kein Polling)", n1 == n0, (n0, n1))
    pruefe("V2 12 s Ruhe: kein Neuaufbau", r1 == r0, (r0, r1))


def test_v3_von_aussen_voll():
    r0, j0 = neuaufbauten(), zaehle("Papierkorb jetzt voll")
    wegwerfen("eins.txt")
    dt = zeit_bis(lambda: symbol() == "voll")
    ZEITEN.append(("leer → voll (Datei von außen in den Papierkorb)", dt))
    pruefe("V3 Datei von außen weggeworfen: Symbol wird „voll“ (≤ 2 s)", dt is not None and dt <= 2.0, dt)
    pruefe("V3 Protokoll „Papierkorb jetzt voll“ genau einmal", zaehle("Papierkorb jetzt voll") == j0 + 1)
    pruefe("V3 nur das Papierkorb-Symbol gewechselt, kein Neuaufbau aller Symbole",
           neuaufbauten() == r0, (r0, neuaufbauten()))
    time.sleep(0.3)
    BILDER_ZUSTAND["voll"] = symbolpixel()
    bild("v3-papierkorb-voll")
    diff = unterschied(BILDER_ZUSTAND["leer"], BILDER_ZUSTAND["voll"])
    pruefe("V3 Symbol sichtbar anders als leer (Pixel)", diff > 20, diff)


def test_v4_zweite_datei():
    j0 = zaehle("Papierkorb jetzt")
    wegwerfen("zwei.txt")
    warte(lambda: a.papierkorb() == ["eins.txt", "zwei.txt"], 4, "zwei Dateien")
    time.sleep(0.8)
    pruefe("V4 zweite Datei: bleibt „voll“, kein weiterer Wechsel",
           symbol() == "voll" and zaehle("Papierkorb jetzt") == j0, (symbol(), zaehle("Papierkorb jetzt") - j0))


def test_v5_von_aussen_leer():
    if not nur_eigene(["eins.txt", "zwei.txt"]):
        return
    endgueltig_loeschen("eins.txt")
    time.sleep(0.8)
    pruefe("V5 eine von zwei endgültig gelöscht: bleibt „voll“", symbol() == "voll", symbol())
    r0 = neuaufbauten()
    endgueltig_loeschen("zwei.txt")
    dt = zeit_bis(lambda: symbol() == "leer")
    ZEITEN.append(("voll → leer (letztes Objekt von außen gelöscht)", dt))
    pruefe("V5 letztes Objekt von außen gelöscht: Symbol wird „leer“ (≤ 2 s)", dt is not None and dt <= 2.0, dt)
    pruefe("V5 kein Neuaufbau aller Symbole", neuaufbauten() == r0, (r0, neuaufbauten()))
    time.sleep(0.3)
    diff = unterschied(BILDER_ZUSTAND["leer"], symbolpixel())
    pruefe("V5 Symbol wieder wie am Anfang (Pixel)", diff <= 5, diff)


def test_v6_ordner_buendel():
    """Ein Ordner mit 30 Dateien: ein Wechsel, wenige Zählungen (Ereignisse gebündelt)."""
    ordner = os.path.join(MUELL, "Ordner mit 30 Dateien")
    os.makedirs(ordner, exist_ok=True)
    for i in range(30):
        with open(os.path.join(ordner, f"d{i:02}.txt"), "w") as f:
            f.write(f"{i}\n")
    n0, j0 = zaehle("Papierkorb gezählt:"), zaehle("Papierkorb jetzt")
    a.wegwerfen(ordner)
    dt = zeit_bis(lambda: symbol() == "voll")
    time.sleep(1.0)
    ZEITEN.append(("leer → voll (Ordner mit 30 Dateien)", dt))
    pruefe("V6 Ordner weggeworfen: „voll“ (≤ 2 s)", dt is not None and dt <= 2.0, dt)
    n = zaehle("Papierkorb gezählt:") - n0
    pruefe("V6 genau ein Wechsel, höchstens 3 Zählungen", zaehle("Papierkorb jetzt") == j0 + 1 and 1 <= n <= 3,
           (zaehle("Papierkorb jetzt") - j0, n))


def test_v7_leeren_im_menue():
    pruefe("V7 voll: „Papierkorb leeren…“ aktiv", (a.papierkorb_menue(None) or True) and a.menue_eintrag() == "an",
           a.menue_eintrag())
    bild("v7-menue-voll")
    q("key Escape", "wait 300")
    if not nur_eigene(["Ordner mit 30 Dateien"]):
        return
    r0 = neuaufbauten()
    a.leeren_bestaetigen("leeren")
    dt = zeit_bis(lambda: "Papierkorb geleert:" in d.text() and symbol() == "leer")
    ZEITEN.append(("voll → leer (Kontextmenü „Papierkorb leeren“, ab Bestätigung)", dt))
    pruefe("V7 nach „Papierkorb leeren“: Papierkorb leer (GVfs)", warte(lambda: a.papierkorb() == [], 6, "leer"))
    zeilen = d.text().splitlines()
    i_start = max(i for i, z in enumerate(zeilen) if "Papierkorb leeren: " in z)
    i_ende = max(i for i, z in enumerate(zeilen) if "Papierkorb geleert:" in z)
    i_leer = max(i for i, z in enumerate(zeilen) if "Papierkorb jetzt leer" in z)
    # sofort: der Wechsel steht zwischen Beginn des Leerens und den zwei
    # Zeilen nach seinem Ende (Zählung, Wechsel) – ohne Wartezeit dazwischen
    pruefe("V7 Symbol wechselt sofort mit dem Leeren (Protokollfolge)",
           i_start < i_leer <= i_ende + 2, (i_start, i_ende, i_leer))
    pruefe("V7 Symbol „leer“", symbol() == "leer", symbol())
    pruefe("V7 kein Neuaufbau aller Symbole", neuaufbauten() == r0, (r0, neuaufbauten()))
    time.sleep(0.5)
    bild("v7-geleert")


def test_v8_leer_ausgegraut():
    a.papierkorb_menue(None)
    pruefe("V8 leer: „Papierkorb leeren…“ ausgegraut", a.menue_eintrag() == "aus", a.menue_eintrag())
    bild("v8-menue-leer")
    q("key Escape", "wait 300")
    a.papierkorb_menue(a.LEEREN_DY)
    pruefe("V8 leer: Klick auf den Eintrag öffnet keine Abfrage", not t.dialog_offen())
    q("key Escape", "wait 300")


def test_v9_desktopdatei():
    """Eine Datei der Arbeitsfläche in den Papierkorb: Symbol der Datei weg, Papierkorb voll."""
    zebra = os.path.join(t.SCHREIBTISCH, "Zebra.txt")
    a.wegwerfen(zebra)
    ok = warte(lambda: symbol() == "voll" and "Zebra.txt" not in lage(), 4, "Zebra weg, voll")
    pruefe("V9 Datei der Arbeitsfläche weggeworfen: Symbol weg, Papierkorb „voll“", ok, (symbol(), list(lage())))
    bild("v9-zebra-im-papierkorb")
    if nur_eigene(["Zebra.txt"]):
        endgueltig_loeschen("Zebra.txt")
        pruefe("V9 danach endgültig gelöscht: „leer“", zeit_bis(lambda: symbol() == "leer") is not None, symbol())


def test_v10_neustart_voll():
    wegwerfen("drei.txt")
    warte(lambda: symbol() == "voll", 3, "voll")
    d.stop()
    d.start(lambda l: "Papierkorb" in l)
    pruefe("V10 Start mit vollem Papierkorb: Symbol „voll“", symbol() == "voll", symbol())
    bild("v10-neustart-voll")
    if nur_eigene(["drei.txt"]):
        a.leeren_bestaetigen("leeren")
        pruefe("V10 danach über das Menü geleert: „leer“",
               zeit_bis(lambda: symbol() == "leer" and a.papierkorb() == []) is not None, symbol())


def main():
    os.makedirs(t.BILDER, exist_ok=True)
    os.environ["QUERENCIA_DESKTOP_DEBUG"] = "1"
    t.protokoll(f"QE-02 Desktop {t.DESKTOP}")
    vorher_echt = a.echter_stand()
    panel = subprocess.Popen([t.PANEL], stdout=open(os.path.join(LOGS, "panel-1.log"), "w"),
                             stderr=subprocess.STDOUT)
    time.sleep(3)
    t.q = t.Eingabe()
    q("move 640 700")
    d.start(lambda l: len(l) >= 8)
    im = bild("start")
    t.oben[0] = t.miss_panel(im)
    tests = [test_v0_schutz, test_v1_start_leer, test_v2_kein_abfragen, test_v3_von_aussen_voll,
             test_v4_zweite_datei, test_v5_von_aussen_leer, test_v6_ordner_buendel, test_v7_leeren_im_menue,
             test_v8_leer_ausgegraut, test_v9_desktopdatei, test_v10_neustart_voll]
    for test in tests:
        name = getattr(test, "__name__", "test")
        t.protokoll(f"=== {name}")
        try:
            test()
        except Exception as err:  # noqa: BLE001
            pruefe(f"{name} ohne Ausnahme", False, f"{err!r}\n{traceback.format_exc()}")
            if "BLOCKED" in str(err):
                break
        q("key Escape", "wait 200")
    pruefe("V11 echter Papierkorb des Besitzers unverändert", a.echter_stand() == vorher_echt)
    d.stop()
    panel.terminate()
    with open(os.path.join(LOGS, "zeiten.txt"), "w") as f:
        for was, dt in ZEITEN:
            f.write(f"{was}: {'-' if dt is None else f'{dt * 1000:.0f} ms'}\n")
    ok = sum(1 for _, o, _ in t.ergebnisse if o)
    with open(os.path.join(LOGS, "pruefung.txt"), "w") as f:
        for n, o, det in t.ergebnisse:
            f.write(f"{'OK    ' if o else 'FEHLER'} {n}{'  – ' + det if det and not o else ''}\n")
        f.write(f"{ok}/{len(t.ergebnisse)} Prüfungen bestanden\n")
    t.protokoll(f"{ok}/{len(t.ergebnisse)} Prüfungen bestanden")


if __name__ == "__main__":
    main()
