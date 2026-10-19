#!/usr/bin/env python3
"""QE-03: Prüftreiber für Datenträger-Symbole auf der Arbeitsfläche.

Läuft wie qe02-treiber.py in der isolierten labwc-Sitzung (pruefen-hmp10.sh
mit HMP10_TREIBER=…/qe03-treiber.py, Zustand wie live über HMP10_ZUSTAND)
und benutzt die Hilfsfunktionen von hmp10-treiber.py.

Echte Datenträger: eigene Loop-Abbilder im Testprofil (Bezeichnung QD…),
über udisksctl eingerichtet, eingehängt und am Ende wieder entfernt. Fremde
Datenträger (cidata der VM, QF… anderer Sitzungen) werden nie angefasst,
höchstens ihr Menü geöffnet und mit Escape geschlossen.

Ausgabe: logs/pruefung.txt, logs/bilder/*.png, logs/zeiten.txt.
"""

import hashlib
import importlib.util
import os
import re
import subprocess
import time
import traceback

HIER = os.path.dirname(os.path.abspath(__file__))
_spec = importlib.util.spec_from_file_location("a", os.path.join(HIER, "hmp10a-treiber.py"))
a = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(a)
t = a.t

P, LOGS = t.P, t.LOGS
d, pruefe, bild, lage, warte = t.d, t.pruefe, t.bild, t.lage, t.warte
ABBILDER = os.path.join(P, "abbilder")
ECHTER_ZUSTAND = "/home/user/.local/state/querencia/desktop-symbole"
ZEITEN = []
ABBILD_LISTE = []
MENUE_AUSHAENGEN_DY = 46   # „Öffnen“ +15, Trennlinie, „Aushängen“ +46


def q(*befehle):
    t.q(*befehle)


def sha(pfad):
    try:
        return hashlib.sha256(open(pfad, "rb").read()).hexdigest()
    except OSError:
        return "fehlt"


def lauf(*befehl):
    r = subprocess.run(list(befehl), capture_output=True, text=True, timeout=60)
    t.protokoll(f"$ {' '.join(befehl)} → {r.returncode} {r.stdout.strip()} {r.stderr.strip()}")
    return r


def eingehaengt(mp):
    return any(z.split()[1] == mp for z in open("/proc/mounts") if len(z.split()) > 1)


class Abbild:
    """Ein eigenes Loop-Abbild (nur im Testprofil, Bezeichnung QD…)."""

    def __init__(self, name, luks=False):
        assert name.startswith("QD"), name
        self.name, self.luks = name, luks
        self.pfad = os.path.join(ABBILDER, name.lower() + ".img")
        assert os.path.realpath(self.pfad).startswith(os.path.realpath(P) + "/")
        self.dev = self.klar = self.mp = None
        ABBILD_LISTE.append(self)

    def anlegen(self):
        os.makedirs(ABBILDER, exist_ok=True)
        subprocess.run(["truncate", "-s", "64M", self.pfad], check=True)
        if not self.luks:
            subprocess.run(["mkfs.vfat", "-n", self.name, self.pfad], check=True,
                           stdout=subprocess.DEVNULL)
        r = lauf("udisksctl", "loop-setup", "-f", self.pfad, "--no-user-interaction")
        m = re.search(r"(/dev/loop\d+)", r.stdout)
        if not m:
            raise RuntimeError(f"BLOCKED: loop-setup fehlgeschlagen: {r.stderr}")
        self.dev = m.group(1)
        if self.luks:
            objekt = "/org/freedesktop/UDisks2/block_devices/" + os.path.basename(self.dev)
            lauf("gdbus", "call", "--system", "--dest", "org.freedesktop.UDisks2", "--object-path", objekt,
                 "--method", "org.freedesktop.UDisks2.Block.Format", "vfat",
                 f"{{'label': <'{self.name}'>, 'encrypt.passphrase': <'qd-test-geheim'>, "
                 "'auth.no_user_interaction': <true>}")
            r = lauf("lsblk", "-nro", "NAME,TYPE", self.dev)
            kinder = [z.split()[0] for z in r.stdout.splitlines() if z.endswith(" crypt")]
            if not kinder:
                raise RuntimeError("BLOCKED: kein entsperrtes LUKS-Kind")
            self.klar = "/dev/mapper/" + kinder[0]

    def block(self):
        return self.klar or self.dev

    def einhaengen(self):
        r = lauf("udisksctl", "mount", "-b", self.block(), "--no-user-interaction")
        m = re.search(r" at (/\S+)", r.stdout)
        self.mp = m.group(1) if m else None
        return self.mp

    def aushaengen(self):
        return lauf("udisksctl", "unmount", "-b", self.block(), "--no-user-interaction").returncode == 0

    def entfernen(self):
        """Aufräumen: nur die Loop-Geräte dieses Abbilds (losetup -j)."""
        if not os.path.exists(self.pfad):
            return
        geraete = subprocess.run(["losetup", "-j", self.pfad, "-n", "-O", "NAME"], capture_output=True,
                                 text=True).stdout.split()
        for dev in geraete:
            r = subprocess.run(["lsblk", "-nro", "NAME,TYPE,MOUNTPOINTS", dev], capture_output=True, text=True)
            for z in r.stdout.splitlines():
                teile = z.split()
                if len(teile) >= 3:
                    lauf("udisksctl", "unmount", "-b", "/dev/" + teile[0] if teile[1] != "crypt"
                         else "/dev/mapper/" + teile[0], "--no-user-interaction")
            if self.luks and any(z.split()[1:2] == ["crypt"] for z in r.stdout.splitlines()):
                lauf("udisksctl", "lock", "-b", dev, "--no-user-interaction")
            lauf("udisksctl", "loop-delete", "-b", dev, "--no-user-interaction")
        os.remove(self.pfad)


def protokollzeilen(text):
    return [z for z in d.text().splitlines() if text in z]


def neuaufbauten():
    return sum(1 for z in d.text().splitlines() if z.startswith("querencia-desktop: ") and " Symbole: " in z)


def zelle_darueber_belegt(l, name):
    x, y = l[name]
    return any(n != name and abs(p[0] - x) < t.CELL[0] and 0 < y - p[1] <= t.CELL[1] + 10
               for n, p in l.items())


def farbanteile(pos):
    """Pixel des 48er-Symbols: (#f07746 orange, #fff4ee hell, #c8c8c8 grau)."""
    pfad = os.path.join(LOGS, "symbol-probe.png")
    subprocess.run(["grim", pfad], check=False)
    im = t.Image.open(pfad).convert("RGB")
    x, y = t.bildschirm(*pos)
    feld = [im.getpixel((i, j)) for i in range(x + 26, x + 74) for j in range(y + 2, y + 50)]
    zaehle = lambda farbe, tol: sum(t.nah(p, farbe, tol) for p in feld)
    return zaehle((0xF0, 0x77, 0x46), 12), zaehle((0xFF, 0xF4, 0xEE), 8), zaehle((0xC8, 0xC8, 0xC8), 8)


def menue(name, eintrag_dy=None):
    """Rechtsklick auf das Symbol; das Menü beginnt am Zeiger und rückt am
    rechten Rand nach innen, also höchstens bis 1220 klicken."""
    m = t.mitte(lage()[name])
    t.klick(m, "rclick")
    q("wait 500")
    if eintrag_dy is not None:
        t.klick((min(m[0] + 60, 1220), m[1] + eintrag_dy))
        q("wait 700")


def fenster_offen():
    """Ein Fenster (Meldung) in der Bildschirmmitte: dort ist sonst nur der
    einfarbige Hintergrund."""
    pfad = os.path.join(LOGS, "fenster-probe.png")
    subprocess.run(["grim", pfad], check=False)
    im = t.Image.open(pfad).convert("RGB")
    pts = [im.getpixel((x, y)) for x in range(440, 840, 8) for y in range(250, 470, 8)]
    return sum(not t.nah(p, t.WALL, 20) for p in pts) / len(pts) > 0.5


def knopf_rechts():
    """Rechte Schaltfläche der Meldung („Trotzdem aushängen“)."""
    x0, y0, x1, y1 = a.dialog_rechteck()
    t.klick((x0 + 3 * (x1 - x0) // 4, y1 - 20))


def menue_zeile():
    z = protokollzeilen("Menü Datenträger: ")
    return z[-1].split("Menü Datenträger: ", 1)[1] if z else None


QDTEST = None
STAND = {}


# ------------------------------------------------------------------ Fälle

def test_d0_schutz():
    pruefe("D0 Zustand im Testprofil (XDG_STATE_HOME)", t.ZUSTAND.startswith(P + "/"), t.ZUSTAND)
    fremd = [z.split()[1] for z in open("/proc/mounts") if "/QD" in z.split()[1]]
    pruefe("D0 kein QD-Datenträger vorher eingehängt", not fremd, fremd)
    STAND["zustand"] = sha(t.ZUSTAND)
    STAND["echt"] = sha(ECHTER_ZUSTAND)
    if fremd:
        raise RuntimeError("BLOCKED: fremde QD-Einhängungen")


def test_d1_start():
    l = lage()
    pruefe("D1 Papierkorb rechts oben (wie live)", l.get("Papierkorb", (0, 0))[0] > 1000, l.get("Papierkorb"))
    da = protokollzeilen("Datenträger da: ")
    t.protokoll("D1 beim Start: " + " | ".join(z.split(": ", 2)[2] for z in da))
    STAND["start_namen"] = set(l)
    bild("desktop-qe03-start")


def test_d2_einhaengen():
    global QDTEST
    QDTEST = Abbild("QDTEST")
    QDTEST.anlegen()
    r0 = neuaufbauten()
    start = time.monotonic()
    mp = QDTEST.einhaengen()
    pruefe("D2 udisksctl hängt QDTEST ein", mp and eingehaengt(mp), mp)
    da = warte(lambda: "QDTEST" in lage(), 6, "Symbol QDTEST")
    ZEITEN.append(("Einhängen → Symbol da", time.monotonic() - start if da else None))
    pruefe("D2 Symbol erscheint nach dem Einhängen", da, list(lage()))
    l = lage()
    if not da:
        return
    pruefe("D2 Symbol in der Spalte des Papierkorbs", l["QDTEST"][0] == l["Papierkorb"][0],
           (l["QDTEST"], l["Papierkorb"]))
    pruefe("D2 darunter: Zelle darüber ist belegt (Papierkorb oder anderer Datenträger)",
           l["QDTEST"][1] > l["Papierkorb"][1] and zelle_darueber_belegt(l, "QDTEST"), l)
    pruefe("D2 kein anderes Symbol verschoben",
           all(l.get(n) == p for n, p in STAND["lage0"].items()), (STAND["lage0"], l))
    pruefe("D2 genau ein Neuaufbau", neuaufbauten() == r0 + 1, (r0, neuaufbauten()))
    z = protokollzeilen("Datenträger da: QDTEST")
    pruefe("D2 Protokoll: Wechseldatenträger, Aushängen",
           z and "(Wechseldatenträger, Aushängen, file:///run/media/" in z[-1], z[-1:] or "")
    time.sleep(0.5)
    orange, hell, grau = farbanteile(l["QDTEST"])
    pruefe("D3 eigenes Querencia-Symbol (Orange, Etikett, Grau des SVG)",
           orange > 300 and hell > 100 and grau > 60, (orange, hell, grau))
    bild("desktop-qe03-qdtest-eingehaengt")
    pruefe("D4 desktop-symbole unverändert nach dem Einhängen", sha(t.ZUSTAND) == STAND["zustand"])
    pruefe("D4 kein Eintrag „laufwerk“ oder QDTEST in desktop-symbole",
           "laufwerk" not in t.zustand() and "QDTEST" not in t.zustand())


def test_d5_zweiter():
    zwei = Abbild("QDZWEI")
    zwei.anlegen()
    zwei.einhaengen()
    da = warte(lambda: "QDZWEI" in lage(), 6, "Symbol QDZWEI")
    l = lage()
    pruefe("D5 zweiter Datenträger erscheint", da, list(l))
    if da:
        pruefe("D5 auch in der Spalte des Papierkorbs, unter einem belegten Platz",
               l["QDZWEI"][0] == l["Papierkorb"][0] and zelle_darueber_belegt(l, "QDZWEI"), l)
        pruefe("D5 QDTEST bleibt, wo es war", l.get("QDTEST") == STAND["qdtest"], (l.get("QDTEST"), STAND["qdtest"]))
        bild("desktop-qe03-zwei-datentraeger")
    r0 = neuaufbauten()
    time.sleep(5)
    pruefe("D5 5 s Ruhe: kein Neuaufbau (kein Abfragen im Takt)", neuaufbauten() == r0, (r0, neuaufbauten()))
    zwei.aushaengen()
    weg = warte(lambda: "QDZWEI" not in lage(), 6, "QDZWEI weg")
    pruefe("D5 von außen ausgehängt (udisksctl): Symbol verschwindet", weg, list(lage()))
    pruefe("D5 Protokoll „Datenträger weg: QDZWEI“", protokollzeilen("Datenträger weg: QDZWEI"))
    zwei.entfernen()


def test_d6_menue_oeffnen():
    menue("QDTEST")
    pruefe("D6 Menü des Datenträgers: Öffnen, Aushängen", menue_zeile() == "Öffnen, Aushängen", menue_zeile())
    bild("desktop-qe03-menue-qdtest")
    q("key Escape", "wait 400")
    vorher = len(t.marker())
    menue("QDTEST", 15)
    warte(lambda: len(t.marker()) > vorher, 5, "Öffnen")
    pruefe("D6 „Öffnen“ startet Files (Attrappe) mit dem Einhängeort",
           t.marker()[vorher:] == [f"files {QDTEST.mp}"], t.marker()[vorher:])


def test_d7_doppelklick():
    vorher = len(t.marker())
    t.klick(t.mitte(lage()["QDTEST"]), "dclick")
    warte(lambda: len(t.marker()) > vorher, 5, "Doppelklick")
    pruefe("D7 Doppelklick öffnet den Einhängeort in Files (Attrappe)",
           t.marker()[vorher:] == [f"files {QDTEST.mp}"], t.marker()[vorher:])


def test_d8_verschieben():
    l = lage()
    von = t.mitte(l["QDTEST"])
    nach = (330, t.oben[0] + 450)
    t.ziehen(von, nach)
    neu = t.neue_lage(lambda x: x.get("QDTEST") not in (None, l["QDTEST"]), 4, "verschoben")
    pruefe("D8 Datenträger-Symbol lässt sich in der Sitzung verschieben", neu.get("QDTEST") != l["QDTEST"],
           (l["QDTEST"], neu.get("QDTEST")))
    pruefe("D8 andere Symbole bleiben", all(neu.get(n) == p for n, p in l.items() if n != "QDTEST"))
    pruefe("D8 desktop-symbole unverändert nach dem Verschieben", sha(t.ZUSTAND) == STAND["zustand"])
    bild("desktop-qe03-verschoben")


def belegt_fragen(nr):
    """Menü → „Aushängen“ bei belegtem Datenträger: die eigene Frage."""
    n0 = len(protokollzeilen("Datenträger QDTEST wird noch benutzt"))
    menue("QDTEST", MENUE_AUSHAENGEN_DY)
    da = warte(lambda: len(protokollzeilen("Datenträger QDTEST wird noch benutzt")) > n0 and fenster_offen(),
               10, "Frage belegt")
    time.sleep(0.4)
    pruefe(f"{nr} belegter Datenträger: deutsche Frage sichtbar", da)
    z = protokollzeilen("Datenträger QDTEST wird noch benutzt")
    pruefe(f"{nr} das belegende Programm ist genannt (sleep)", z and "sleep (" in z[-1], z[-1:])
    return da


def test_d9_belegt_abbrechen():
    belegt = subprocess.Popen(["sleep", "300"], cwd=QDTEST.mp)
    try:
        time.sleep(0.5)
        n_fehl = len(protokollzeilen("Aushängen fehlgeschlagen"))
        if belegt_fragen("D9"):
            bild("desktop-qe03-belegt-frage")
            q("key Escape", "wait 1000")
            pruefe("D9 Escape: still abgebrochen, keine Fehlermeldung",
                   warte(lambda: protokollzeilen("Aushängen abgebrochen: QDTEST"), 5, "abgebrochen")
                   and len(protokollzeilen("Aushängen fehlgeschlagen")) == n_fehl)
            time.sleep(2.5)
            pruefe("D9 Frage zu und bleibt zu (keine neue Frage von GVfs)", not fenster_offen())
        pruefe("D9 Datenträger bleibt eingehängt, Symbol bleibt",
               eingehaengt(QDTEST.mp) and "QDTEST" in lage(), list(lage()))
    finally:
        belegt.kill()
        belegt.wait()


def test_d9b_trotzdem():
    belegt = subprocess.Popen(["sleep", "300"], cwd=QDTEST.mp)
    try:
        time.sleep(0.5)
        if belegt_fragen("D9b"):
            knopf_rechts()
            weg = warte(lambda: "QDTEST" not in lage(), 8, "QDTEST weg")
            pruefe("D9b „Trotzdem aushängen“: Symbol verschwindet", weg, list(lage()))
            pruefe("D9b ausgehängt trotz laufendem Programm", not eingehaengt(QDTEST.mp))
            pruefe("D9b Protokoll „trotzdem“ und „ausgehängt“",
                   protokollzeilen("Aushängen: trotzdem: QDTEST") and protokollzeilen("ausgehängt: QDTEST"))
            pruefe("D9b keine Meldung offen", not fenster_offen())
    finally:
        belegt.kill()
        belegt.wait()
    time.sleep(1)


def test_d9c_frei_geworden():
    QDTEST.einhaengen()
    warte(lambda: "QDTEST" in lage(), 6, "QDTEST wieder da")
    belegt = subprocess.Popen(["sleep", "300"], cwd=QDTEST.mp)
    try:
        time.sleep(0.5)
        if belegt_fragen("D9c"):
            belegt.kill()
            belegt.wait()
            weg = warte(lambda: "QDTEST" not in lage(), 10, "QDTEST weg")
            bild("desktop-qe03-frei-geworden")
            pruefe("D9c Programm beendet: GVfs hängt selbst aus, Symbol verschwindet", weg, list(lage()))
            pruefe("D9c die Frage schließt sich selbst",
                   warte(lambda: not fenster_offen(), 4, "Frage zu")
                   and protokollzeilen("Datenträger QDTEST ist nicht mehr belegt"))
            pruefe("D9c ausgehängt", not eingehaengt(QDTEST.mp))
    finally:
        if belegt.poll() is None:
            belegt.kill()
            belegt.wait()


def test_d10_aushaengen():
    QDTEST.einhaengen()
    warte(lambda: "QDTEST" in lage(), 6, "QDTEST wieder da")
    r0 = len(protokollzeilen("ausgehängt: QDTEST"))
    start = time.monotonic()
    menue("QDTEST", MENUE_AUSHAENGEN_DY)
    weg = warte(lambda: "QDTEST" not in lage(), 8, "QDTEST weg")
    ZEITEN.append(("„Aushängen“ im Menü → Symbol weg", time.monotonic() - start if weg else None))
    pruefe("D10 „Aushängen“ im Menü: Symbol verschwindet", weg, list(lage()))
    pruefe("D10 wirklich ausgehängt", not eingehaengt(QDTEST.mp), QDTEST.mp)
    pruefe("D10 Protokoll „ausgehängt: QDTEST“", len(protokollzeilen("ausgehängt: QDTEST")) == r0 + 1)
    pruefe("D10 keine Meldung", not fenster_offen())
    pruefe("D10 desktop-symbole unverändert", sha(t.ZUSTAND) == STAND["zustand"])
    bild("desktop-qe03-ausgehaengt")


def test_d11_wieder():
    QDTEST.einhaengen()
    da = warte(lambda: "QDTEST" in lage(), 6, "QDTEST wieder da")
    l = lage()
    pruefe("D11 wieder eingehängt: Symbol wieder unter dem Papierkorb (Platz vergessen)",
           da and l["QDTEST"] == STAND["qdtest"], (l.get("QDTEST"), STAND["qdtest"]))


def test_d12_neustart():
    d.stop()
    d.start(lambda l: "QDTEST" in l)
    l = lage()
    pruefe("D12 Neustart mit eingehängtem Datenträger: Symbol da, unter dem Papierkorb",
           "QDTEST" in l and l["QDTEST"][0] == l["Papierkorb"][0] and zelle_darueber_belegt(l, "QDTEST"), l)
    pruefe("D12 desktop-symbole unverändert", sha(t.ZUSTAND) == STAND["zustand"])
    QDTEST.aushaengen()
    pruefe("D12 danach von außen ausgehängt: Symbol weg", warte(lambda: "QDTEST" not in lage(), 6, "weg"))


def test_d13_verschluesselt():
    geheim = Abbild("QDGEHEIM", luks=True)
    geheim.anlegen()
    geheim.einhaengen()
    da = warte(lambda: "QDGEHEIM" in lage(), 6, "QDGEHEIM")
    pruefe("D13 entsperrter LUKS-Datenträger erscheint", da, list(lage()))
    z = protokollzeilen("Datenträger da: QDGEHEIM")
    pruefe("D13 Bild „verschlüsselt“ (Gerät /dev/dm-*)", z and "(verschlüsselt, Aushängen" in z[-1], z[-1:])
    if da:
        time.sleep(0.5)
        orange, hell, grau = farbanteile(lage()["QDGEHEIM"])
        pruefe("D13 Schloss-Symbol (Orange und Grau, ohne helles Etikett)", orange > 150 and grau > 100,
               (orange, hell, grau))
        bild("desktop-qe03-verschluesselt")
        menue("QDGEHEIM", MENUE_AUSHAENGEN_DY)
        pruefe("D13 „Aushängen“: Symbol verschwindet", warte(lambda: "QDGEHEIM" not in lage(), 8, "weg"))
        time.sleep(1)
        r = lauf("lsblk", "-nro", "NAME,TYPE", geheim.dev)
        t.protokoll(f"D13 nach dem Aushängen: {'noch entsperrt' if ' crypt' in r.stdout else 'gesperrt'}")
    geheim.entfernen()


def test_d14_fremd_optisch():
    """cidata der VM (falls eingehängt): optisches Symbol, „Auswerfen“; nur Menü auf, Escape."""
    if "cidata" not in lage():
        t.protokoll("D14 cidata nicht eingehängt – übersprungen")
        return
    z = protokollzeilen("Datenträger da: cidata")
    pruefe("D14 cidata: optisches Medium, Auswerfen", z and "(optisches Medium, Auswerfen" in z[-1], z[-1:])
    menue("cidata")
    pruefe("D14 Menü cidata: Öffnen, Auswerfen", menue_zeile() == "Öffnen, Auswerfen", menue_zeile())
    bild("desktop-qe03-menue-cidata")
    q("key Escape", "wait 600")
    pruefe("D14 Menü wieder zu, cidata unverändert eingehängt",
           eingehaengt("/run/media/user/cidata") and "cidata" in lage())


# ------------------------------------------------------------------ ohne Loop (G2-X)

def test_d12_ohne_loop():
    """Neustart mit dem eingehängten cidata der VM (kein eigenes Abbild)."""
    if "cidata" not in lage():
        t.protokoll("D12o cidata nicht eingehängt – übersprungen")
        return
    vorher = lage()
    d.stop()
    d.start(lambda l: "cidata" in l)
    l = lage()
    pruefe("D12o Neustart: cidata wieder da, in der Spalte des Papierkorbs",
           "cidata" in l and l["cidata"][0] == l["Papierkorb"][0], (l.get("cidata"), l.get("Papierkorb")))
    pruefe("D12o Neustart: feste Symbole an denselben Plätzen",
           all(l.get(n) == vorher.get(n) for n in ("Rechner", "Persönlicher Ordner", "Papierkorb")),
           {n: (vorher.get(n), l.get(n)) for n in ("Rechner", "Persönlicher Ordner", "Papierkorb")})
    pruefe("D12o desktop-symbole unverändert (cidata nie gespeichert)",
           sha(t.ZUSTAND) == STAND["zustand"] and "cidata" not in open(t.ZUSTAND, errors="replace").read())


# ------------------------------------------------------------------ QE-04 (G2-X): [Icons]/[Volumes]

INI = os.path.join(P, "config", "querencia", "desktop.ini")


def kann_qe04():
    r = subprocess.run([t.DESKTOP, "--faehigkeiten"], capture_output=True, text=True, timeout=10)
    return "volumes-types" in r.stdout.split()


def ini_setzen(gruppen):
    """desktop.ini = [Background] des Testprofils + gruppen (atomar wie die Einstellungen)."""
    text = STAND["ini0"] + ("\n" + gruppen if gruppen else "")
    neu = INI + ".neu"
    with open(neu, "w") as f:
        f.write(text)
    os.rename(neu, INI)


def geaendert_zahl():
    return d.text().count("Einstellungen geändert: ")


# Zelle je Symbolgröße wie desktop_view.rs cell_for
ZELLE = {32: (100, 72), 48: (100, 88), 72: (110, 112), 96: (130, 136)}


def ohne_ueberlappung(l, zelle):
    werte = list(l.items())
    return all(abs(pa[0] - pb[0]) >= zelle[0] or abs(pa[1] - pb[1]) >= zelle[1]
               for i, (_, pa) in enumerate(werte) for _, pb in werte[i + 1:])


def test_q04_filter():
    """Je Kästchen an/aus mit dem echten cidata der VM und den festen Symbolen."""
    STAND["ini0"] = open(INI).read()
    if not STAND["ini0"].endswith("\n"):
        STAND["ini0"] += "\n"
    fest = ("Rechner", "Persönlicher Ordner", "Papierkorb")
    lage0 = lage()
    cidata = "cidata" in lage0
    t.protokoll(f"Q04 Ausgang: {sorted(lage0)}; cidata {'da' if cidata else 'nicht eingehängt'}")
    alle = "Removable=true\nOptical=true\nNetwork=true\nInternal=true\n"
    faelle = [
        # (Nr, Text, Gruppen, erwartet da, erwartet weg)
        ("Q1", "Optical=false: cidata weg, feste Symbole bleiben", "[Volumes]\nOptical=false\n",
         fest, ("cidata",)),
        ("Q2", "Optical=true, alle anderen Laufwerkstypen aus: cidata da",
         "[Volumes]\nRemovable=false\nOptical=true\nNetwork=false\nInternal=false\n", fest + ("cidata",), ()),
        ("Q3", "Removable=false allein: cidata bleibt (optisch, nicht Wechsel)", "[Volumes]\nRemovable=false\n",
         fest + ("cidata",), ()),
        ("Q4", "Network=false allein: cidata bleibt", "[Volumes]\nNetwork=false\n", fest + ("cidata",), ()),
        ("Q5", "Internal=true allein: cidata bleibt", "[Volumes]\nInternal=true\n", fest + ("cidata",), ()),
        ("Q6", "alle vier aus: cidata weg", "[Volumes]\nRemovable=false\nOptical=false\nNetwork=false\nInternal=false\n",
         fest, ("cidata",)),
        ("Q7", "alle vier an: cidata da", "[Volumes]\n" + alle, fest + ("cidata",), ()),
        ("Q8", "Computer=false: Rechner weg", "[Icons]\nComputer=false\n", ("Persönlicher Ordner", "Papierkorb"), ("Rechner",)),
        ("Q9", "Home=false: Persönlicher Ordner weg", "[Icons]\nHome=false\n", ("Rechner", "Papierkorb"), ("Persönlicher Ordner",)),
        ("Q10", "Trash=false: Papierkorb weg", "[Icons]\nTrash=false\n", ("Rechner", "Persönlicher Ordner"), ("Papierkorb",)),
        ("Q11", "alle drei aus", "[Icons]\nComputer=false\nHome=false\nTrash=false\n", (), fest),
    ]
    for nr, text, gruppen, da, weg in faelle:
        if not cidata:
            da = tuple(n for n in da if n != "cidata")
            weg = tuple(n for n in weg if n != "cidata")
        n0 = geaendert_zahl()
        t0 = time.time()
        ini_setzen(gruppen)
        ok = warte(lambda: geaendert_zahl() > n0 and all(n in lage() for n in da)
                   and not any(n in lage() for n in weg), 6, nr)
        ZEITEN.append((f"{nr} bis zur neuen Lage", time.time() - t0 if ok else None))
        l = lage()
        pruefe(f"{nr} {text}", bool(ok), sorted(l))
        bleiben = [n for n in da if n in fest and lage0.get(n) != l.get(n)]
        pruefe(f"{nr} feste Symbole an ihren Plätzen", not bleiben, {n: (lage0.get(n), l.get(n)) for n in bleiben})
        if nr in ("Q1", "Q11"):
            bild(f"desktop-qe04-{nr.lower()}")
    # Größe: die Zelle wächst, die Plätze rasten neu ein, nichts überlappt
    for groesse in (32, 72, 96):
        n0 = geaendert_zahl()
        ini_setzen(f"[Icons]\nSize={groesse}\n")
        ok = warte(lambda: geaendert_zahl() > n0 and f"Größe {groesse}" in d.text().split("Einstellungen geändert: ")[-1], 6, f"Größe {groesse}")
        time.sleep(1.5)
        l = lage()
        pruefe(f"Q12 Size={groesse}: übernommen, alle Symbole da, keine Überlappung, keine zu große Kachel",
               bool(ok) and all(n in l for n in fest) and ohne_ueberlappung(l, ZELLE[groesse])
               and "Kachel größer als die Zelle" not in d.text(), sorted(l.items()))
        bild(f"desktop-qe04-groesse-{groesse}")
    # zurück zum Standard (Gruppen weg): alles wie am Anfang
    n0 = geaendert_zahl()
    ini_setzen("")
    ok = warte(lambda: geaendert_zahl() > n0 and all(n in lage() for n in lage0), 6, "Standard")
    time.sleep(1.5)
    l = lage()
    pruefe("Q13 ohne Gruppen wieder Standard: alle Symbole an den Plätzen vom Anfang",
           bool(ok) and all(l.get(n) == lage0.get(n) for n in lage0), {n: (lage0.get(n), l.get(n)) for n in lage0})
    pruefe("Q14 desktop-symbole nach allen Umschaltungen unverändert", sha(t.ZUSTAND) == STAND["zustand"])
    pruefe("Q15 Hintergrund unberührt: keine Problemzeile zu desktop.ini",
           "desktop.ini: " not in d.text().replace("desktop.ini: keine", ""),
           [z for z in d.text().splitlines() if "desktop.ini: " in z][:3])
    with open(INI, "w") as f:
        f.write(STAND["ini0"])


def main():
    os.makedirs(t.BILDER, exist_ok=True)
    os.environ["QUERENCIA_DESKTOP_DEBUG"] = "1"
    t.protokoll(f"QE-03 Desktop {t.DESKTOP}")
    panel = subprocess.Popen([t.PANEL], stdout=open(os.path.join(LOGS, "panel-1.log"), "w"),
                             stderr=subprocess.STDOUT)
    time.sleep(3)
    t.q = t.Eingabe()
    q("move 640 700")
    d.start(lambda l: len(l) >= 8)
    im = bild("start")
    t.oben[0] = t.miss_panel(im)
    STAND["lage0"] = dict(lage())
    tests = [test_d0_schutz, test_d1_start, test_d2_einhaengen]
    weitere = [test_d5_zweiter, test_d6_menue_oeffnen, test_d7_doppelklick, test_d8_verschieben,
               test_d9_belegt_abbrechen, test_d9b_trotzdem, test_d9c_frei_geworden, test_d10_aushaengen, test_d11_wieder, test_d12_neustart,
               test_d13_verschluesselt, test_d14_fremd_optisch]
    # G2-X: ohne eigene Loop-Abbilder (sie erscheinen systemweit, auch auf dem
    # Live-Desktop): nur cidata, Neustart, Zustandsdatei; mit QE-04 dazu der Filter
    # G2-X QE-04a: nur der LUKS-Fall (ein einziges Abbild, QDGEHEIM, ≈14 s)
    if os.environ.get("QE03_NUR_D13") == "1":
        tests = [test_d0_schutz, test_d1_start]
        weitere = [test_d13_verschluesselt]
        t.protokoll("nur D13 (QE03_NUR_D13=1)")
    if os.environ.get("QE03_OHNE_LUKS") == "1":
        weitere.remove(test_d13_verschluesselt)
        t.protokoll("ohne LUKS-Fall (QE03_OHNE_LUKS=1)")
    if os.environ.get("QE03_OHNE_LOOP") == "1":
        tests = [test_d0_schutz, test_d1_start, test_d14_fremd_optisch, test_d12_ohne_loop]
        weitere = [test_q04_filter] if kann_qe04() else []
        t.protokoll(f"ohne Loop-Abbilder; QE-04-Filter: {'ja' if weitere else 'nein (Desktop kann es nicht)'}")
    try:
        for test in tests + weitere:
            name = getattr(test, "__name__", "test")
            t.protokoll(f"=== {name}")
            try:
                test()
                if test is test_d2_einhaengen:
                    STAND["qdtest"] = lage().get("QDTEST")
                    if STAND["qdtest"] is None:
                        raise RuntimeError("BLOCKED: QDTEST ohne Symbol")
            except Exception as err:  # noqa: BLE001
                pruefe(f"{name} ohne Ausnahme", False, f"{err!r}\n{traceback.format_exc()}")
                if "BLOCKED" in str(err):
                    break
            q("key Escape", "wait 200")
    finally:
        for abbild in ABBILD_LISTE:
            try:
                abbild.entfernen()
            except Exception as err:  # noqa: BLE001
                t.protokoll(f"Aufräumen {abbild.name}: {err!r}")
    reste = subprocess.run(["losetup", "-a"], capture_output=True, text=True).stdout
    pruefe("D15 alle eigenen Loop-Geräte entfernt", ABBILDER not in reste, reste)
    alle = "".join(open(os.path.join(LOGS, f), errors="replace").read()
                   for f in os.listdir(LOGS) if f.startswith("desktop-"))
    pruefe("D15 keine GTK-Kritikmeldung, keine Panik", "CRITICAL" not in alle and "panicked" not in alle,
           [z for z in alle.splitlines() if "CRITICAL" in z or "panicked" in z][:5])
    pruefe("D15 desktop-symbole des Testprofils byte-gleich wie zu Beginn", sha(t.ZUSTAND) == STAND.get("zustand"))
    pruefe("D15 echte desktop-symbole des Besitzers unverändert", sha(ECHTER_ZUSTAND) == STAND.get("echt"))
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
