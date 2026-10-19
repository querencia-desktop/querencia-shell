#!/usr/bin/env python3
"""G2-AD: qeingabe-Szenarien für Kalender, System-Menü und Symbolthema.

  szenario-g2ad.py <breite> <höhe> kalender|system|thema|thema-start > szenario.txt

Läuft mit den umgelenkten G2-M-Werkzeugen (nachweis/gui/gm-werkzeug/pruefen.sh,
G2M_NETZ=echt, G2M_PANELINI=panel-mit-netzwerk.ini, G2M_VORHER=vorher-g2ad.sh,
QS_TROCKENLAUF=1). SHOT ersetzt szenario-g2m.sh.

- kalender:     Uhr anklicken, Monat und Jahr blättern (Pfeile im Kopf),
                Esc, erneuter Klick schließt, Tastatur (Strg+Pfeil blättert
                den Monat, Pfeil und Leertaste wählen einen Tag; GTK 4 kennt
                im Kalender kein Bild auf/ab),
                Klick daneben, Rechtsklick öffnet keinen Kalender.
- system:       System-Menü; „Abmelden …“ und „Ausschalten …“ öffnen den
                Dialog von querencia-sitzung, Esc bricht ab; danach je einmal
                den Countdown bis 0 laufen lassen (nur Trockenlauf).
- thema:        Panel und Desktop; Symbolthema zur Laufzeit über gsettings:
                Adwaita, Humanity, ein nicht installiertes, zurücksetzen.
- thema-start:  Panel und Desktop starten mit gesetztem Adwaita
                (G2AD_THEMA_START, vorher-g2ad.sh).

Koordinaten vermessen bei 1024x768 (nachweis/gui/ad-erkunden*): Uhr-Mitte
W - 76, Panelhöhe 32 (Mitte 16); der Kalender hängt am rechten Rand, Kopf
y 69: Monat zurück W - 258, vor W - 142, Jahr zurück W - 109, vor W - 26.
System-Knopf x 269; im Menü „Abmelden …“ y 78, „Ausschalten …“ y 102.
Orte-Knopf x 195.
"""
import sys

W, H, ART = int(sys.argv[1]), int(sys.argv[2]), sys.argv[3]
NY = 16
CX, CY = W // 2, H // 2
AWAY = (CX, CY + 200)
UHR = W - 76
KOPF_Y = 69
MONAT_VOR, JAHR_VOR = W - 142, W - 26
SYSTEM, ORTE = 269, 195
MENU_X, ABMELDEN, AUSSCHALTEN = 300, 78, 102
out = []


def z(*lines):
    out.extend(lines)


def mark(key, text):
    z(f"mark {key} {text}")


def click(x, y, wait=900, button="click"):
    z(f"move {x} {y}", "wait 250", button, f"wait {wait}")


def shot(name):
    z(f"shot SHOT/{name}.png")


def away(wait=600):
    z(f"move {AWAY[0]} {AWAY[1]}", f"wait {wait}")


def key(name, wait=500):
    z(f"key {name}", f"wait {wait}")


def chord(arrow, wait=700):
    """Strg + Pfeil links/rechts."""
    z("keydown Control_L", "wait 80", f"keydown {arrow}", "wait 80", f"keyup {arrow}",
      "wait 80", "keyup Control_L", f"wait {wait}")


def icon_theme(action):
    """gsettings im dconf des Profils (set <Thema> oder reset)."""
    z(f"run gsettings {action.split()[0]} org.gnome.desktop.interface icon-theme "
      + " ".join(action.split()[1:]), "wait 3000")


def orte_menue(name):
    click(ORTE, NY, 1200)
    shot(name)
    key("Escape", 700)


z("wait 1500")
if ART == "kalender":
    mark("K00", "Ausgang")
    away(800)
    shot("k00-ausgang")
    mark("K01", "Uhr anklicken: Kalender des laufenden Monats")
    click(UHR, NY, 1200)
    shot("k01-offen")
    mark("K02", "Pfeil Monat vor")
    click(MONAT_VOR, KOPF_Y, 900)
    shot("k02-monat-vor")
    mark("K03", "Pfeil Jahr vor")
    click(JAHR_VOR, KOPF_Y, 900)
    shot("k03-jahr-vor")
    mark("K04", "Esc schließt")
    key("Escape", 900)
    away(600)
    shot("k04-esc")
    mark("K05", "wieder öffnen: wieder der laufende Monat")
    click(UHR, NY, 1200)
    shot("k05-wieder")
    mark("K06", "erneuter Klick auf die Uhr schließt (und öffnet nicht wieder)")
    click(UHR, NY, 1500)
    away(600)
    shot("k06-zweiter-klick")
    mark("K07", "Tastatur: Strg+Pfeil rechts/links blättert, Pfeil ab und Leertaste wählen")
    click(UHR, NY, 1200)
    chord("Right")
    shot("k07-strg-rechts")
    chord("Left")
    shot("k07-zurueck")
    key("Down", 400)
    key("space", 700)
    shot("k07-auswahl")
    key("Escape", 900)
    mark("K08", "Klick daneben schließt")
    click(UHR, NY, 1200)
    click(AWAY[0], AWAY[1], 1000)
    shot("k08-daneben")
    mark("K09", "Rechtsklick: Applet-Menü, kein Kalender")
    click(UHR, NY, 1200, "rclick")
    shot("k09-rechtsklick")
    key("Escape", 800)
    away(600)
    shot("k10-ende")
elif ART == "system":
    mark("S00", "System-Menü")
    click(SYSTEM, NY, 1200)
    shot("s00-menue")
    mark("S01", "Abmelden … öffnet den Dialog mit Countdown, Esc bricht ab")
    click(MENU_X, ABMELDEN, 2500)
    shot("s01-abmelden")
    z("wait 3000")
    shot("s01-abmelden-3s")
    key("Escape", 1500)
    away(600)
    shot("s01-abgebrochen")
    mark("S02", "Ausschalten … öffnet den Dialog mit Countdown, Esc bricht ab")
    click(SYSTEM, NY, 1200)
    click(MENU_X, AUSSCHALTEN, 2500)
    shot("s02-ausschalten")
    key("Escape", 1500)
    away(600)
    shot("s02-abgebrochen")
    mark("S03", "Abmelden …: Countdown bis 0 (Trockenlauf)")
    click(SYSTEM, NY, 1200)
    click(MENU_X, ABMELDEN, 2500)
    away(500)
    shot("s03-start")
    z("wait 61000")
    shot("s03-nach-countdown")
    mark("S04", "Ausschalten …: Countdown bis 0 (Trockenlauf)")
    click(SYSTEM, NY, 1200)
    click(MENU_X, AUSSCHALTEN, 2500)
    away(500)
    z("wait 61000")
    shot("s04-nach-countdown")
elif ART == "thema":
    mark("T00", "Ausgang: keine Benutzereinstellung")
    away(800)
    shot("t00-ausgang")
    orte_menue("t00-orte")
    mark("T01", "gsettings set Adwaita")
    icon_theme("set Adwaita")
    away(500)
    shot("t01-adwaita")
    orte_menue("t01-orte")
    mark("T02", "gsettings set Humanity")
    icon_theme("set Humanity")
    away(500)
    shot("t02-humanity")
    mark("T03", "gsettings set ein nicht installiertes Thema")
    icon_theme("set Gibtsnicht")
    away(500)
    shot("t03-gibtsnicht")
    mark("T04", "gsettings reset: zurück auf GNOME Human")
    icon_theme("reset")
    away(500)
    shot("t04-reset")
    orte_menue("t04-orte")
elif ART == "thema-start":
    mark("T10", "Start mit gesetztem Adwaita")
    away(800)
    shot("t10-start")
    orte_menue("t10-orte")
else:
    sys.exit(f"unbekannte Art {ART}")
mark("ENDE", "")
print("\n".join(out))
