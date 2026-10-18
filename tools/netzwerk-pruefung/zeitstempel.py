#!/usr/bin/env python3
"""G2-M: setzt vor jede Zeile von stdin die Uhrzeit (HH:MM:SS.mmm)."""
import sys
import time

for line in sys.stdin:
    now = time.time()
    sys.stdout.write(time.strftime("%H:%M:%S.", time.localtime(now)) + f"{int(now * 1000) % 1000:03d} " + line)
    sys.stdout.flush()
