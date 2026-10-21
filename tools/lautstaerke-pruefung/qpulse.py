#!/usr/bin/env python3
"""G2-J: kleiner Testserver für das PulseAudio-Protokoll (nur isolierte Prüfung).

Auf der VM ist kein Audiodienst installiert (kein PipeWire, kein
pipewire-pulse, kein PulseAudio, kein pactl). Damit libpulse – und mit ihr
das Lautstärke-Applet und der Lautstärkeregler – trotzdem echt gegen einen
Server laufen, spricht dieses Programm den nativen Teil des Protokolls, den
die beiden brauchen:

  AUTH, SET_CLIENT_NAME, GET_SERVER_INFO, GET_{SINK,SOURCE}_INFO_LIST,
  GET_{SINK_INPUT,SOURCE_OUTPUT}_INFO_LIST, SUBSCRIBE (+ SUBSCRIBE_EVENT),
  SET_{SINK,SOURCE}_VOLUME/_MUTE, SET_DEFAULT_{SINK,SOURCE},
  SET_{SINK_INPUT,SOURCE_OUTPUT}_VOLUME/_MUTE.

Kein Ton, kein SHM/memfd (die Antwort auf AUTH bietet beides nicht an),
Protokollversion 35 wie PulseAudio 17. Die Feldfolge der Antworten ist die,
die libpulse 17 in introspect.c liest (Ports ab v16/v24/v34, Formate ab
v21/v22). Unbekannte Befehle bekommen PA_ERR_NOTSUPPORTED.

Steuerung für die Prüfung über $XDG_RUNTIME_DIR/qpulse.ctl (eine Zeile JSON
je Anfrage, Antwort eine Zeile JSON); qpactl.py ist die pactl-artige
Oberfläche dazu. Jede Anfrage eines Clients und jede Änderung steht im Log
(Zeilen „BEFEHL …“, „ZUSTAND …“, „EREIGNIS …“).

Läuft nur in der isolierten Umgebung (XDG_RUNTIME_DIR unter
nachweis/gui/rt/g2j-), nie in der Sitzung.

  qpulse.py <logdatei> [--leer]   --leer: ohne Geräte starten
"""

import json
import os
import selectors
import socket
import struct
import sys
import time

RT_MARK = "/querencia-runtime-wt-lautstaerke/nachweis/gui/rt/g2j-"
PROTOCOL_VERSION = 35
NORM = 0x10000
INVALID = 0xFFFFFFFF

# Befehle (pulsecore/native-common.h)
ERROR, REPLY, AUTH, SET_CLIENT_NAME = 0, 2, 8, 9
GET_SERVER_INFO, GET_SINK_INFO, GET_SINK_INFO_LIST = 20, 21, 22
GET_SOURCE_INFO, GET_SOURCE_INFO_LIST = 23, 24
GET_SINK_INPUT_INFO_LIST, GET_SOURCE_OUTPUT_INFO_LIST = 30, 32
SUBSCRIBE, SET_SINK_VOLUME, SET_SINK_INPUT_VOLUME, SET_SOURCE_VOLUME = 35, 36, 37, 38
SET_SINK_MUTE, SET_SOURCE_MUTE = 39, 40
SET_DEFAULT_SINK, SET_DEFAULT_SOURCE = 44, 45
SUBSCRIBE_EVENT, SET_SINK_INPUT_MUTE = 66, 69
SET_SOURCE_OUTPUT_VOLUME, SET_SOURCE_OUTPUT_MUTE = 98, 99
NAMES = {v: k for k, v in {
    "ERROR": ERROR, "REPLY": REPLY, "AUTH": AUTH, "SET_CLIENT_NAME": SET_CLIENT_NAME,
    "GET_SERVER_INFO": GET_SERVER_INFO, "GET_SINK_INFO": GET_SINK_INFO,
    "GET_SINK_INFO_LIST": GET_SINK_INFO_LIST, "GET_SOURCE_INFO": GET_SOURCE_INFO,
    "GET_SOURCE_INFO_LIST": GET_SOURCE_INFO_LIST, "GET_SINK_INPUT_INFO_LIST": GET_SINK_INPUT_INFO_LIST,
    "GET_SOURCE_OUTPUT_INFO_LIST": GET_SOURCE_OUTPUT_INFO_LIST, "SUBSCRIBE": SUBSCRIBE,
    "SET_SINK_VOLUME": SET_SINK_VOLUME, "SET_SINK_INPUT_VOLUME": SET_SINK_INPUT_VOLUME,
    "SET_SOURCE_VOLUME": SET_SOURCE_VOLUME, "SET_SINK_MUTE": SET_SINK_MUTE,
    "SET_SOURCE_MUTE": SET_SOURCE_MUTE, "SET_DEFAULT_SINK": SET_DEFAULT_SINK,
    "SET_DEFAULT_SOURCE": SET_DEFAULT_SOURCE, "SUBSCRIBE_EVENT": SUBSCRIBE_EVENT,
    "SET_SINK_INPUT_MUTE": SET_SINK_INPUT_MUTE, "SET_SOURCE_OUTPUT_VOLUME": SET_SOURCE_OUTPUT_VOLUME,
    "SET_SOURCE_OUTPUT_MUTE": SET_SOURCE_OUTPUT_MUTE}.items()}

# Fehler (pulse/def.h)
ERR_PROTOCOL, ERR_NOENTITY, ERR_INVALID, ERR_NOTSUPPORTED = 7, 5, 3, 19

# Ereignisse (pulse/def.h): Bereich | Art
FAC = {"sink": 0, "source": 1, "sink-input": 2, "source-output": 3, "module": 4, "server": 7}
NEW, CHANGE, REMOVE = 0x00, 0x10, 0x20


class Tags:
    """Tagstruct schreiben (pulsecore/tagstruct.c)."""

    def __init__(self):
        self.b = bytearray()

    def u32(self, v):
        self.b += b"L" + struct.pack(">I", v & 0xFFFFFFFF)
        return self

    def u8(self, v):
        self.b += b"B" + bytes([v])
        return self

    def s(self, v):
        self.b += b"N" if v is None else b"t" + v.encode() + b"\0"
        return self

    def boolean(self, v):
        self.b += b"1" if v else b"0"
        return self

    def usec(self, v):
        self.b += b"U" + struct.pack(">II", v >> 32, v & 0xFFFFFFFF)
        return self

    def volume(self, v):
        self.b += b"V" + struct.pack(">I", v)
        return self

    def sample_spec(self, channels=2, rate=48000, fmt=3):  # PA_SAMPLE_S16LE
        self.b += b"a" + bytes([fmt, channels]) + struct.pack(">I", rate)
        return self

    def channel_map(self, channels=2):
        positions = [1, 2] if channels == 2 else [0] * channels  # FRONT_LEFT/RIGHT bzw. MONO
        self.b += b"m" + bytes([channels] + positions)
        return self

    def cvolume(self, values):
        self.b += b"v" + bytes([len(values)]) + b"".join(struct.pack(">I", v) for v in values)
        return self

    def proplist(self, props):
        self.b += b"P"
        for key, value in props.items():
            data = value.encode() + b"\0"
            self.s(key)
            self.u32(len(data))
            self.b += b"x" + struct.pack(">I", len(data)) + data
        self.b += b"N"
        return self

    def format_info(self):
        self.b += b"f"
        self.u8(1)  # PA_ENCODING_PCM
        self.proplist({})
        return self


class Reader:
    """Tagstruct lesen."""

    def __init__(self, data):
        self.d, self.i = data, 0

    def tag(self, want):
        t = self.d[self.i:self.i + 1]
        if t not in want:
            raise ValueError(f"Tag {t!r} statt {want!r} an {self.i}")
        self.i += 1
        return t

    def u32(self):
        self.tag(b"L")
        v, = struct.unpack_from(">I", self.d, self.i)
        self.i += 4
        return v

    def s(self):
        if self.tag((b"t", b"N")) == b"N":
            return None
        end = self.d.index(b"\0", self.i)
        v = self.d[self.i:end].decode(errors="replace")
        self.i = end + 1
        return v

    def boolean(self):
        return self.tag((b"1", b"0")) == b"1"

    def arbitrary(self):
        self.tag(b"x")
        n, = struct.unpack_from(">I", self.d, self.i)
        self.i += 4 + n
        return self.d[self.i - n:self.i]

    def cvolume(self):
        self.tag(b"v")
        n = self.d[self.i]
        self.i += 1
        vals = list(struct.unpack_from(f">{n}I", self.d, self.i))
        self.i += 4 * n
        return vals

    def proplist(self):
        self.tag(b"P")
        props = {}
        while True:
            key = self.s()
            if key is None:
                return props
            self.u32()
            props[key] = self.arbitrary().rstrip(b"\0").decode(errors="replace")

    def end(self):
        return self.i >= len(self.d)


def pct(values):
    return round(max(values or [0]) * 100 / NORM)


class Server:
    def __init__(self, log_path, empty):
        self.log = open(log_path, "a", buffering=1)
        self.next_index = {"sink": 0, "source": 0, "sink-input": 0, "source-output": 0, "module": 0, "client": 0}
        self.sinks, self.sources, self.inputs, self.outputs, self.modules = [], [], [], [], []
        self.default_sink = self.default_source = None
        self.clients = {}  # Socket -> Client
        if not empty:
            self.add_sink("alsa_output.test-analog-stereo", "Eingebautes Tonsystem Analog Stereo", 40)
            self.add_source("alsa_input.test-analog-stereo", "Eingebautes Tonsystem Mikrofon", 70)
            # Wie PipeWire 1.6 ohne eigenes Standardmikrofon (isolierte Probe
            # mit echtem PipeWire, G2-J): Standardquelle ist der Monitor.
            self.default_source = "alsa_output.test-analog-stereo.monitor"
        self.say("ZUSTAND Start", self.summary())

    # ---------------------------------------------------------------- Log
    def say(self, *parts):
        self.log.write(f"{time.strftime('%H:%M:%S')}.{int(time.time() * 1000) % 1000:03d} " +
                       " ".join(str(p) for p in parts) + "\n")

    def summary(self):
        return json.dumps({
            "default_sink": self.default_sink, "default_source": self.default_source,
            "sinks": [{"index": d["index"], "name": d["name"], "percent": pct(d["volume"]), "muted": d["muted"]}
                      for d in self.sinks],
            "sources": [{"index": d["index"], "name": d["name"], "percent": pct(d["volume"]),
                         "muted": d["muted"], "monitor": d["monitor_of"] != INVALID} for d in self.sources],
            "sink_inputs": [{"index": s["index"], "app": s["props"].get("application.name"),
                             "percent": pct(s["volume"]), "muted": s["muted"]} for s in self.inputs],
            "source_outputs": [{"index": s["index"], "app": s["props"].get("application.name"),
                                "percent": pct(s["volume"]), "muted": s["muted"]} for s in self.outputs],
        }, ensure_ascii=False)

    # ---------------------------------------------------------- Objekte
    def new_index(self, kind):
        i = self.next_index[kind]
        self.next_index[kind] += 1
        return i

    def add_sink(self, name, description, percent=100, module=INVALID):
        sink = {"index": self.new_index("sink"), "name": name, "description": description,
                "volume": [round(percent * NORM / 100)] * 2, "muted": False, "module": module}
        monitor = self.add_source(name + ".monitor", "Monitor of " + description, 100,
                                  monitor_of=sink, module=module, quiet=True)
        sink["monitor"] = monitor
        self.sinks.append(sink)
        if self.default_sink is None:
            self.default_sink = name
            self.event("server", CHANGE, 0)
        self.event("sink", NEW, sink["index"])
        return sink

    def add_source(self, name, description, percent=100, monitor_of=None, module=INVALID, quiet=False):
        source = {"index": self.new_index("source"), "name": name, "description": description,
                  "volume": [round(percent * NORM / 100)] * 2, "muted": False, "module": module,
                  "monitor_of": monitor_of["index"] if monitor_of else INVALID,
                  "monitor_of_name": monitor_of["name"] if monitor_of else None}
        self.sources.append(source)
        if self.default_source is None and monitor_of is None:
            self.default_source = name
            self.event("server", CHANGE, 0)
        self.event("source", NEW, source["index"])
        return source

    def remove_sink(self, sink):
        self.sinks.remove(sink)
        self.sources.remove(sink["monitor"])
        self.event("source", REMOVE, sink["monitor"]["index"])
        self.event("sink", REMOVE, sink["index"])
        for s in [s for s in self.inputs if s["device"] == sink["index"]]:
            if self.sinks:
                s["device"] = self.sinks[0]["index"]
                self.event("sink-input", CHANGE, s["index"])
        if self.default_sink == sink["name"]:
            self.default_sink = self.sinks[0]["name"] if self.sinks else None
            self.event("server", CHANGE, 0)

    def find(self, items, index, name):
        for item in items:
            if (index != INVALID and item["index"] == index) or (name is not None and item["name"] == name):
                return item
        return None

    # --------------------------------------------------------- Ereignisse
    def event(self, facility, kind, index):
        self.say("EREIGNIS", facility, {NEW: "neu", CHANGE: "geändert", REMOVE: "entfernt"}[kind], index)
        packet = Tags().u32(SUBSCRIBE_EVENT).u32(INVALID).u32(FAC[facility] | kind).u32(index).b
        mask_bit = {"sink": 0x1, "source": 0x2, "sink-input": 0x4, "source-output": 0x8,
                    "module": 0x10, "server": 0x80}[facility]
        for client in list(self.clients.values()):
            if client.mask & mask_bit:
                client.send(packet)

    # ------------------------------------------------------- Serialisieren
    def put_device(self, t, d, sink):
        t.u32(d["index"]).s(d["name"]).s(d["description"]).sample_spec().channel_map()
        t.u32(d["module"]).cvolume(d["volume"]).boolean(d["muted"])
        if sink:
            t.u32(d["monitor"]["index"]).s(d["monitor"]["name"])
        else:
            t.u32(d["monitor_of"]).s(d["monitor_of_name"])
        t.usec(0).s("qpulse.py").u32(0x0001 | 0x0004)  # HW_VOLUME_CTRL | HARDWARE
        t.proplist({"device.description": d["description"], "device.class": "monitor" if
                    (not sink and d["monitor_of"] != INVALID) else "sound"})
        t.usec(0)                                     # v13: configured latency
        t.volume(NORM).u32(0).u32(NORM + 1).u32(INVALID)  # v15: base volume, state RUNNING, steps, card
        t.u32(0).s(None)                              # v16: keine Ports, aktiver Port NULL
        t.u8(1).format_info()                         # v21/v22: ein Format (PCM)

    def put_stream(self, t, s, sink_input):
        t.u32(s["index"]).s(s["props"].get("media.name", "Wiedergabe")).u32(INVALID).u32(INVALID)
        t.u32(s["device"]).sample_spec().channel_map()
        if sink_input:
            t.cvolume(s["volume"]).usec(0).usec(0).s("trivial").s("qpulse.py")
            t.boolean(s["muted"]).proplist(s["props"]).boolean(False).boolean(True).boolean(True)
            t.format_info()
        else:
            t.usec(0).usec(0).s("trivial").s("qpulse.py").proplist(s["props"]).boolean(False)
            t.cvolume(s["volume"]).boolean(s["muted"]).boolean(True).boolean(True).format_info()

    # ------------------------------------------------------------ Befehle
    def handle(self, client, payload):
        r = Reader(payload)
        command, tag = r.u32(), r.u32()
        name = NAMES.get(command, str(command))
        try:
            reply = self.command(client, command, tag, r)
        except (ValueError, IndexError, struct.error) as err:
            self.say("FEHLER", name, err)
            reply = Tags().u32(ERROR).u32(tag).u32(ERR_PROTOCOL).b
        if reply is None:
            self.say("BEFEHL", client.name, name, "nicht unterstützt")
            reply = Tags().u32(ERROR).u32(tag).u32(ERR_NOTSUPPORTED).b
        client.send(reply)

    def command(self, client, command, tag, r):
        ok = Tags().u32(REPLY).u32(tag)
        if command == AUTH:
            version = r.u32() & 0xFFFF
            client.version = min(version, PROTOCOL_VERSION)
            self.say("BEFEHL", client.name, "AUTH Version", version, "→", client.version)
            return ok.u32(client.version).b  # ohne SHM- und memfd-Bit
        if command == SET_CLIENT_NAME:
            props = r.proplist() if client.version >= 13 else {"application.name": r.s()}
            client.name = props.get("application.name", client.name)
            self.say("BEFEHL", client.name, "SET_CLIENT_NAME")
            return ok.u32(client.index).b if client.version >= 13 else ok.b
        if command == GET_SERVER_INFO:
            self.say("BEFEHL", client.name, "GET_SERVER_INFO")
            return (ok.s("qpulse (Testserver G2-J)").s("17.0").s("user").s("testrechner")
                    .sample_spec().s(self.default_sink).s(self.default_source).u32(0x51)
                    .channel_map().b)
        if command in (GET_SINK_INFO_LIST, GET_SOURCE_INFO_LIST):
            sink = command == GET_SINK_INFO_LIST
            self.say("BEFEHL", client.name, NAMES[command])
            for d in (self.sinks if sink else self.sources):
                self.put_device(ok, d, sink)
            return ok.b
        if command in (GET_SINK_INFO, GET_SOURCE_INFO):
            sink = command == GET_SINK_INFO
            index, name = r.u32(), r.s()
            d = self.find(self.sinks if sink else self.sources, index, name)
            if d is None:
                return Tags().u32(ERROR).u32(tag).u32(ERR_NOENTITY).b
            self.put_device(ok, d, sink)
            return ok.b
        if command in (GET_SINK_INPUT_INFO_LIST, GET_SOURCE_OUTPUT_INFO_LIST):
            sink_input = command == GET_SINK_INPUT_INFO_LIST
            self.say("BEFEHL", client.name, NAMES[command])
            for s in (self.inputs if sink_input else self.outputs):
                self.put_stream(ok, s, sink_input)
            return ok.b
        if command == SUBSCRIBE:
            client.mask = r.u32()
            self.say("BEFEHL", client.name, f"SUBSCRIBE Maske 0x{client.mask:x}")
            return ok.b
        if command in (SET_SINK_VOLUME, SET_SOURCE_VOLUME, SET_SINK_MUTE, SET_SOURCE_MUTE):
            sink = command in (SET_SINK_VOLUME, SET_SINK_MUTE)
            index, name = r.u32(), r.s()
            d = self.find(self.sinks if sink else self.sources, index, name)
            if d is None:
                return Tags().u32(ERROR).u32(tag).u32(ERR_NOENTITY).b
            if command in (SET_SINK_VOLUME, SET_SOURCE_VOLUME):
                d["volume"] = r.cvolume()
                self.say("BEFEHL", client.name, NAMES[command], d["name"], pct(d["volume"]), "%", d["volume"])
            else:
                d["muted"] = r.boolean()
                self.say("BEFEHL", client.name, NAMES[command], d["name"], "stumm" if d["muted"] else "laut")
            self.event("sink" if sink else "source", CHANGE, d["index"])
            self.say("ZUSTAND", self.summary())
            return ok.b
        if command in (SET_DEFAULT_SINK, SET_DEFAULT_SOURCE):
            name = r.s()
            sink = command == SET_DEFAULT_SINK
            if self.find(self.sinks if sink else self.sources, INVALID, name) is None:
                return Tags().u32(ERROR).u32(tag).u32(ERR_NOENTITY).b
            if sink:
                self.default_sink = name
            else:
                self.default_source = name
            self.say("BEFEHL", client.name, NAMES[command], name)
            self.event("server", CHANGE, INVALID)
            self.say("ZUSTAND", self.summary())
            return ok.b
        if command in (SET_SINK_INPUT_VOLUME, SET_SINK_INPUT_MUTE, SET_SOURCE_OUTPUT_VOLUME, SET_SOURCE_OUTPUT_MUTE):
            sink_input = command in (SET_SINK_INPUT_VOLUME, SET_SINK_INPUT_MUTE)
            index = r.u32()
            s = self.find(self.inputs if sink_input else self.outputs, index, None)
            if s is None:
                return Tags().u32(ERROR).u32(tag).u32(ERR_NOENTITY).b
            if command in (SET_SINK_INPUT_VOLUME, SET_SOURCE_OUTPUT_VOLUME):
                s["volume"] = r.cvolume()
                self.say("BEFEHL", client.name, NAMES[command], index, pct(s["volume"]), "%")
            else:
                s["muted"] = r.boolean()
                self.say("BEFEHL", client.name, NAMES[command], index, "stumm" if s["muted"] else "laut")
            self.event("sink-input" if sink_input else "source-output", CHANGE, index)
            self.say("ZUSTAND", self.summary())
            return ok.b
        return None

    # ------------------------------------------------------------ Steuerung
    def control(self, request):
        op = request.get("op")
        if op == "info":
            return {"default_sink": self.default_sink, "default_source": self.default_source,
                    "server": "qpulse 17.0", "clients": [c.name for c in self.clients.values()]}
        if op == "state":
            return json.loads(self.summary())
        if op == "load-null-sink":
            module = self.new_index("module")
            name = request.get("name", "null")
            sink = self.add_sink(name, request.get("description", "Null Output"), request.get("percent", 100), module)
            self.modules.append({"index": module, "sink": sink})
            self.event("module", NEW, module)
            self.say("STEUERUNG load-module module-null-sink", name, "→ Modul", module)
            self.say("ZUSTAND", self.summary())
            return {"module": module, "sink": sink["index"]}
        if op == "unload-module":
            module = next((m for m in self.modules if m["index"] == request["module"]), None)
            if module is None:
                return {"error": "kein solches Modul"}
            self.modules.remove(module)
            self.remove_sink(module["sink"])
            self.event("module", REMOVE, module["index"])
            self.say("STEUERUNG unload-module", module["index"])
            self.say("ZUSTAND", self.summary())
            return {"ok": True}
        if op in ("set-sink-volume", "set-sink-mute", "set-default-sink"):
            sink = self.find(self.sinks, INVALID, request["name"])
            if sink is None:
                return {"error": "kein solches Gerät"}
            if op == "set-sink-volume":
                sink["volume"] = [round(request["percent"] * NORM / 100)] * len(sink["volume"])
                self.event("sink", CHANGE, sink["index"])
            elif op == "set-sink-mute":
                sink["muted"] = bool(request["muted"])
                self.event("sink", CHANGE, sink["index"])
            else:
                self.default_sink = sink["name"]
                self.event("server", CHANGE, INVALID)
            self.say("STEUERUNG", op, json.dumps(request, ensure_ascii=False))
            self.say("ZUSTAND", self.summary())
            return {"ok": True}
        if op in ("play", "record"):
            play = op == "play"
            kind = "sink-input" if play else "source-output"
            devices = self.sinks if play else [s for s in self.sources if s["monitor_of"] == INVALID]
            if not devices:
                return {"error": "kein Gerät"}
            stream = {"index": self.new_index(kind), "device": devices[0]["index"],
                      "volume": [round(request.get("percent", 100) * NORM / 100)] * 2, "muted": False,
                      "props": {"application.name": request.get("app", "Testanwendung"),
                                "media.name": request.get("title", ""),
                                "application.icon_name": request.get("icon", "audio-x-generic")}}
            (self.inputs if play else self.outputs).append(stream)
            self.event(kind, NEW, stream["index"])
            self.say("STEUERUNG", op, json.dumps(request, ensure_ascii=False), "→", stream["index"])
            self.say("ZUSTAND", self.summary())
            return {"index": stream["index"]}
        if op == "stop":
            for items, kind in ((self.inputs, "sink-input"), (self.outputs, "source-output")):
                for s in [s for s in items if s["index"] == request["index"] and kind == request.get("kind", kind)]:
                    items.remove(s)
                    self.event(kind, REMOVE, s["index"])
            return {"ok": True}
        if op == "quit":
            self.say("STEUERUNG quit")
            raise SystemExit(0)
        return {"error": f"unbekannt: {op}"}


class Client:
    def __init__(self, server, sock, index):
        self.server, self.sock, self.index = server, sock, index
        self.name, self.version, self.mask, self.buf, self.out = f"Client#{index}", 8, 0, b"", b""

    def send(self, payload):
        self.out += struct.pack(">IIIII", len(payload), INVALID, 0, 0, 0) + payload
        self.flush()

    def flush(self):
        while self.out:
            try:
                n = self.sock.send(self.out)
            except BlockingIOError:
                return
            except OSError:
                self.out = b""
                return
            self.out = self.out[n:]

    def feed(self, data):
        self.buf += data
        while len(self.buf) >= 20:
            length, channel, _, _, _ = struct.unpack_from(">IIIII", self.buf)
            if len(self.buf) < 20 + length:
                return
            payload, self.buf = self.buf[20:20 + length], self.buf[20 + length:]
            if channel == INVALID:
                self.server.handle(self, payload)


def main():
    rt = os.environ.get("XDG_RUNTIME_DIR", "")
    if rt.startswith("/run/user/") or RT_MARK not in rt:
        print("qpulse: nur in der isolierten Prüfumgebung (XDG_RUNTIME_DIR)", file=sys.stderr)
        return 2
    server = Server(sys.argv[1], "--leer" in sys.argv[2:])
    os.makedirs(os.path.join(rt, "pulse"), mode=0o700, exist_ok=True)
    path = os.path.join(rt, "pulse", "native")
    ctl_path = os.path.join(rt, "qpulse.ctl")
    for p in (path, ctl_path):
        if os.path.exists(p):
            os.unlink(p)
    listener = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    listener.bind(path)
    listener.listen(16)
    control = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    control.bind(ctl_path)
    control.listen(4)
    sel = selectors.DefaultSelector()
    sel.register(listener, selectors.EVENT_READ, "pulse")
    sel.register(control, selectors.EVENT_READ, "control")
    server.say("ZUSTAND bereit", path)
    try:
        while True:
            for key, _ in sel.select():
                if key.data == "pulse":
                    sock, _ = listener.accept()
                    sock.setblocking(False)
                    client = Client(server, sock, server.new_index("client"))
                    server.clients[sock] = client
                    sel.register(sock, selectors.EVENT_READ, "client")
                    server.say("VERBINDUNG", client.name)
                elif key.data == "control":
                    sock, _ = control.accept()
                    with sock:
                        line = sock.makefile("r").readline()
                        try:
                            answer = server.control(json.loads(line))
                        except SystemExit:
                            sock.sendall(b'{"ok": true}\n')
                            raise
                        except (KeyError, ValueError) as err:
                            answer = {"error": str(err)}
                        sock.sendall(json.dumps(answer, ensure_ascii=False).encode() + b"\n")
                else:
                    sock = key.fileobj
                    client = server.clients[sock]
                    try:
                        data = sock.recv(65536)
                    except OSError:
                        data = b""
                    if not data:
                        sel.unregister(sock)
                        del server.clients[sock]
                        sock.close()
                        server.say("TRENNUNG", client.name)
                    else:
                        client.feed(data)
    except SystemExit:
        pass
    finally:
        for p in (path, ctl_path):
            if os.path.exists(p):
                os.unlink(p)
    return 0


if __name__ == "__main__":
    sys.exit(main())
