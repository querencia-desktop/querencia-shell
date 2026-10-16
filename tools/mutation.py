#!/usr/bin/env python3
"""Mutationsproben für einen Arbeitsbaum (HMP-06 ff.).

Aufruf: mutation.py <arbeitsbaum> <mutationen.json> <protokoll>

Jede Mutation: {"name", "datei" (relativ zum Baum), "alt", "neu",
"pruefung": "unit" | Shell-Befehl}. Die Datei wird mit Zeitstempel gesichert
(wie cp -p), mutiert, geprüft und wiederhergestellt; danach wird ihre mtime
erneuert, damit cargo neu baut. "unit" = cargo test; sonst ein Shell-Befehl
(baut selbst, Exitcode 0 = Mutation NICHT erkannt). Ein Muster muss genau
einmal vorkommen, sonst "MUSTER FEHLT".
"""
import json
import os
import shutil
import subprocess
import sys

ENV = dict(os.environ, PATH="/usr/lib/rust-1.93/bin:" + os.environ["PATH"], CARGO_BUILD_JOBS="2")


def run(cmd, cwd):
    p = subprocess.run(cmd, cwd=cwd, shell=True, env=ENV, capture_output=True, text=True)
    return p.returncode, (p.stdout + p.stderr)


def main():
    tree, spec, log = sys.argv[1:4]
    mutations = json.load(open(spec))
    out = open(log, "w")
    detected = 0
    for m in mutations:
        path = os.path.join(tree, m["datei"])
        text = open(path).read()
        if text.count(m["alt"]) != 1:
            line = f"{m['name']}: MUSTER FEHLT ({text.count(m['alt'])}x)"
            print(line); out.write(line + "\n"); out.flush()
            continue
        backup = path + ".mutation-orig"
        shutil.copy2(path, backup)
        try:
            open(path, "w").write(text.replace(m["alt"], m["neu"]))
            if m["pruefung"] == "unit":
                code, output = run("cargo test --locked --offline 2>&1", tree)
                summary = [l for l in output.splitlines() if l.startswith(("test result", "error"))][:2]
            else:
                code, output = run(m["pruefung"], tree)
                summary = output.strip().splitlines()[-3:]
        finally:
            shutil.move(backup, path)
            os.utime(path)
        verdict = "NICHT ERKANNT" if code == 0 else "erkannt"
        detected += code != 0
        line = f"{m['name']}: {verdict}"
        print(line); out.write(line + "\n")
        for s in summary:
            out.write("    " + s[:200] + "\n")
        out.flush()
    code, _ = run("cargo build --locked --offline 2>&1", tree)
    line = f"{detected}/{len(mutations)} erkannt; Wiederherstellung gebaut (exit {code})"
    print(line); out.write(line + "\n")
    return 0 if detected == len(mutations) else 1


if __name__ == "__main__":
    sys.exit(main())
