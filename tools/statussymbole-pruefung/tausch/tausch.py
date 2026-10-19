#!/usr/bin/env python3
"""Kontrollierter Austausch des laufenden Querencia-Panels (Nacht 2026-10-16).

Aufruf: tausch.py <neues-binary> <erwarteter-sha16-neu> <logname> [alt-sha16]

- findet das laufende Panel der Sitzung (XDG_RUNTIME_DIR=/run/user/1001),
  prüft optional dessen Binary-Prüfsumme;
- übernimmt seine Umgebung (aus /proc/<pid>/environ) und sein
  Arbeitsverzeichnis, setzt QUERENCIA_MENU_DEBUG=1;
- beendet nur dieses Panel (SIGTERM), startet das neue mit eigener
  Prozessgruppe, Log ~/.local/state/querencia/<logname>;
- gesund = Prozess lebt nach dem Aufbau ("Panel(s)" im Log) noch 4 s und
  das Log zeigt keinen Absturz (panicked/CRITICAL);
- sonst automatisch zurück auf das alte Binary (gleiche Umgebung).
- G2-Y (19.10.2026): Nach SIGTERM gilt das alte Panel erst als beendet,
  wenn der Sitzungsbus seinen Namen org.example.Querencia.Panel freigegeben
  hat (NameHasOwner, höchstens 5 s). /proc meldet den Hauptthread schon
  nach ~1 ms als Zombie, die übrigen Threads und damit die Busverbindung
  leben noch 40–70 ms; ein sofort gestartetes Panel wurde dann zur
  Zweitinstanz und endete mit „Failed to register: … NoReply“. Gleiches vor
  dem Rückfall. Log-Abschnitte werden nach Bytes geschnitten (Umlaute).
Schreibt ~/.local/state/querencia/panel-rueckfall.txt mit altem und neuem
Binary, damit der Rückfall jederzeit von Hand geht.
"""
import hashlib
import os
import signal
import subprocess
import sys
import time

def in_own_scope(name, argv):
    """argv in an own transient user scope (systemd-run --scope execs the
    program, the PID stays the same), so a restart of the workshop service,
    whose cgroup ends all its processes, never takes the component along."""
    unit = f"{name}-{time.strftime('%Y%m%d-%H%M%S')}"
    return ["systemd-run", "--user", "--scope", "--quiet", f"--unit={unit}",
            f"--description=Querencia {name} (live)", *argv]


STATE = os.path.expanduser("~/.local/state/querencia")
SESSION_RUNTIME = "/run/user/1001"
APP_NAME = "org.example.Querencia.Panel"
NAME_WAIT = 5.0


def sha16(path):
    return hashlib.sha256(open(path, "rb").read()).hexdigest()[:16]


def environ(pid):
    env = {}
    for item in open(f"/proc/{pid}/environ", "rb").read().split(b"\0"):
        if b"=" in item:
            k, v = item.split(b"=", 1)
            env[k.decode()] = v.decode()
    return env


def running_panel():
    found = []
    for pid in os.listdir("/proc"):
        if not pid.isdigit():
            continue
        try:
            exe = os.readlink(f"/proc/{pid}/exe")
            if not exe.endswith("/querencia-panel") or "(deleted)" in exe:
                continue
            if environ(pid).get("XDG_RUNTIME_DIR") == SESSION_RUNTIME:
                found.append((int(pid), exe))
        except (FileNotFoundError, PermissionError, ProcessLookupError):
            pass
    return found


def gone(pid):
    try:
        return open(f"/proc/{pid}/stat").read().split()[2] == "Z"
    except FileNotFoundError:
        return True


def name_owned(env):
    """Ob der Sitzungsbus aus env den Namen des Panels noch vergeben hat;
    None, wenn das nicht zu klären ist (kein Bus, gdbus fehlt)."""
    address = env.get("DBUS_SESSION_BUS_ADDRESS")
    if not address:
        return None
    try:
        out = subprocess.run(
            ["gdbus", "call", "--address", address, "--dest", "org.freedesktop.DBus",
             "--object-path", "/org/freedesktop/DBus",
             "--method", "org.freedesktop.DBus.NameHasOwner", APP_NAME],
            capture_output=True, text=True, timeout=2)
    except (OSError, subprocess.TimeoutExpired):
        return None
    if out.returncode != 0:
        return None
    return out.stdout.strip() == "(true,)"


def wait_name_released(env, seconds=NAME_WAIT):
    """Wartet, bis der Bus den Panel-Namen freigegeben hat. Gibt die
    Wartezeit zurück, None bei Zeitlimit oder wenn es nicht zu prüfen ist."""
    t = time.monotonic()
    while True:
        owned = name_owned(env)
        if owned is None:
            return None
        if not owned:
            return time.monotonic() - t
        if time.monotonic() - t > seconds:
            return None
        time.sleep(0.02)


def log_since(log, mark):
    """Logtext ab Byte mark (getsize zählt Bytes, das Log enthält Umlaute)."""
    with open(log, "rb") as f:
        f.seek(mark)
        return f.read().decode(errors="replace")


def start(binary, env, cwd, log):
    out = open(log, "a")
    out.write(f"\n=== {time.strftime('%F %T')} Start {binary} ({sha16(binary)}) ===\n")
    out.flush()
    return subprocess.Popen(in_own_scope("querencia-panel", [binary]), env=env, cwd=cwd, stdin=subprocess.DEVNULL,
                            stdout=out, stderr=subprocess.STDOUT, start_new_session=True)


def healthy(proc, log, mark, seconds=10):
    deadline = time.time() + seconds
    while time.time() < deadline:
        if proc.poll() is not None:
            return False, "Prozess beendet"
        text = log_since(log, mark)
        if "Panel(s)" in text:
            time.sleep(4)
            text = log_since(log, mark)
            if proc.poll() is not None:
                return False, "nach dem Aufbau beendet"
            if "panicked" in text or "CRITICAL" in text:
                return False, "Absturzmeldung im Log"
            return True, "läuft"
        time.sleep(0.2)
    return False, "kein Aufbau im Log"


def main():
    new_bin, new_sha, logname = sys.argv[1:4]
    old_sha = sys.argv[4] if len(sys.argv) > 4 else None
    assert sha16(new_bin) == new_sha, f"neues Binary {sha16(new_bin)} != {new_sha}"
    panels = running_panel()
    assert len(panels) == 1, f"nicht genau ein Sitzungs-Panel: {panels}"
    old_pid, old_bin = panels[0]
    if old_sha:
        assert sha16(f"/proc/{old_pid}/exe") == old_sha, "altes Binary unerwartet"
    env = environ(old_pid)
    env["QUERENCIA_MENU_DEBUG"] = "1"
    cwd = os.readlink(f"/proc/{old_pid}/cwd")
    log = os.path.join(STATE, logname)
    with open(os.path.join(STATE, "panel-rueckfall.txt"), "a") as f:
        f.write(f"{time.strftime('%F %T')} alt PID {old_pid} {old_bin} ({sha16(old_bin)}) -> neu {new_bin} ({new_sha})\n")
    print(f"alt: PID {old_pid} {old_bin}")
    os.kill(old_pid, signal.SIGTERM)
    for _ in range(50):
        if gone(old_pid):
            break
        time.sleep(0.1)
    else:
        sys.exit("altes Panel endet nicht – nichts gestartet")
    waited = wait_name_released(env)
    if waited is None:
        print(f"Warnung: Busname {APP_NAME} nach {NAME_WAIT:.0f} s noch vergeben oder nicht prüfbar – starte trotzdem")
    else:
        print(f"Busname frei nach {waited * 1000:.0f} ms")
    mark = os.path.getsize(log) if os.path.exists(log) else 0
    new = start(new_bin, env, cwd, log)
    ok, why = healthy(new, log, mark)
    if ok:
        print(f"NEU LÄUFT: PID {new.pid} {new_bin} ({why})")
        return 0
    print(f"STARTFEHLER ({why}) – Rückfall auf {old_bin}")
    if new.poll() is None:
        new.terminate()
        new.wait(5)
    if wait_name_released(env) is None:
        print(f"Warnung: Busname {APP_NAME} vor dem Rückfall noch vergeben oder nicht prüfbar")
    back_log = os.path.join(STATE, "panel-rueckfall.log")
    mark = os.path.getsize(back_log) if os.path.exists(back_log) else 0
    back = start(old_bin, env, cwd, back_log)
    ok, why = healthy(back, back_log, mark)
    print(f"Rückfall: {why}, PID {back.pid}")
    return 1


if __name__ == "__main__":
    sys.exit(main())
