#!/usr/bin/env python3
"""G2-M: Attrappe von NetworkManager auf einem eigenen D-Bus (nur Prüfung).

  python3 -I qnm.py <logdatei> [--ohne-wlan]

Verbindet sich mit dem Bus aus DBUS_SYSTEM_BUS_ADDRESS (in der Prüfung ein
eigener dbus-daemon im isolierten Laufzeitordner, nie der echte
Systembus), übernimmt dort den Namen org.freedesktop.NetworkManager und
bildet so viel der D-Bus-API 1.x nach, wie das Netzwerk-Applet benutzt:

- Objektmanager auf /org/freedesktop (GetManagedObjects,
  InterfacesAdded/Removed), Properties (Get/GetAll/Set, PropertiesChanged);
- Manager: Devices, ActiveConnections, PrimaryConnection, WirelessEnabled
  und WwanEnabled (schreibbar), ActivateConnection,
  AddAndActivateConnection, DeactivateConnection;
- ein Kabelgerät enp3s0 (verbunden, „Kabelgebundene Verbindung 1“) und ein
  WLAN-Gerät wlp2s0 mit drei Netzen: „Testnetz“ (WPA2, gespeichertes
  Profil ohne Schlüssel: der Schlüssel kommt vom Passwortagenten des
  Panels, richtig ist „geheim123“), „Cafe offen“ (offen), „Nachbar“ (WPA3,
  neu, Schlüssel „nachbar99“);
- Device.Disconnect, Wireless.RequestScan, Settings.ListConnections,
  Settings.Connection.GetSettings, AgentManager.Register; beim Verbinden
  ruft die Attrappe GetSecrets beim angemeldeten Agenten auf
  (falscher Schlüssel: noch einmal mit REQUEST_NEW).

Zustandswechsel per Testbefehl (Schnittstelle org.querencia.Attrappe,
Methode Befehl(s) → s, siehe qnmctl.py): kabel ab|an, signal <ssid> <0-100>,
netz-neu <ssid> <stärke> offen|wpa2|wpa3, netz-weg <ssid>,
wlan-schalter aus|an, zustand (JSON), beenden.

Jede Anfrage steht mit Uhrzeit in der Logdatei.
"""

import json
import os
import sys
import time
import warnings

import gi

gi.require_version("Gio", "2.0")
from gi.repository import Gio, GLib  # noqa: E402

# register_object with Python closures is all PyGObject offers here.
warnings.filterwarnings("ignore", category=DeprecationWarning)

NM = "org.freedesktop.NetworkManager"
NM_PATH = "/org/freedesktop/NetworkManager"
OM_PATH = "/org/freedesktop"
DEV = NM + ".Device"
WIRED = DEV + ".Wired"
WIRELESS = DEV + ".Wireless"
AP = NM + ".AccessPoint"
ACTIVE = NM + ".Connection.Active"
IP4 = NM + ".IP4Config"
IP6 = NM + ".IP6Config"
SETTINGS = NM + ".Settings"
SCONN = SETTINGS + ".Connection"
AGENTS = NM + ".AgentManager"
AGENT = NM + ".SecretAgent"
PROPS = "org.freedesktop.DBus.Properties"
OM = "org.freedesktop.DBus.ObjectManager"
CTL = "org.querencia.Attrappe"
CTL_PATH = "/org/querencia/Attrappe"

LOG = open(sys.argv[1], "a", buffering=1)
OHNE_WLAN = "--ohne-wlan" in sys.argv


def log(text):
    LOG.write(f"{time.strftime('%H:%M:%S')}.{int(time.time() * 1000) % 1000:03d} {text}\n")


V = GLib.Variant
NONE = "/"

METHODS = {
    NM: """
    <method name="ActivateConnection"><arg type="o" direction="in"/><arg type="o" direction="in"/><arg type="o" direction="in"/><arg type="o" direction="out"/></method>
    <method name="AddAndActivateConnection"><arg type="a{sa{sv}}" direction="in"/><arg type="o" direction="in"/><arg type="o" direction="in"/><arg type="o" direction="out"/><arg type="o" direction="out"/></method>
    <method name="DeactivateConnection"><arg type="o" direction="in"/></method>
    <method name="GetDevices"><arg type="ao" direction="out"/></method>
    <signal name="StateChanged"><arg type="u"/></signal>""",
    DEV: '<method name="Disconnect"/>',
    WIRELESS: """
    <method name="RequestScan"><arg type="a{sv}" direction="in"/></method>
    <method name="GetAllAccessPoints"><arg type="ao" direction="out"/></method>""",
    SETTINGS: '<method name="ListConnections"><arg type="ao" direction="out"/></method>',
    SCONN: '<method name="GetSettings"><arg type="a{sa{sv}}" direction="out"/></method>',
    AGENTS: """
    <method name="Register"><arg type="s" direction="in"/></method>
    <method name="Unregister"/>""",
    OM: """
    <method name="GetManagedObjects"><arg type="a{oa{sa{sv}}}" direction="out"/></method>
    <signal name="InterfacesAdded"><arg type="o"/><arg type="a{sa{sv}}"/></signal>
    <signal name="InterfacesRemoved"><arg type="o"/><arg type="as"/></signal>""",
    CTL: '<method name="Befehl"><arg type="s" direction="in"/><arg type="s" direction="out"/></method>',
}
WRITABLE = {(NM, "WirelessEnabled"), (NM, "WwanEnabled")}
# Interfaces that GetManagedObjects lists (not the control interface).
INTERNAL = {CTL, OM, AGENTS}


class Attrappe:
    def __init__(self, bus):
        self.bus = bus
        self.objects = {}        # path -> {iface: {prop: Variant}}
        self.registrations = {}  # path -> [ids]
        self.counter = 0
        self.agent = None
        self.secrets = {}        # profile path -> stored psk (system-owned)
        self.profile_settings = {}

    # ---- Objects ---------------------------------------------------------
    def new_path(self, kind):
        self.counter += 1
        return f"{NM_PATH}/{kind}/{self.counter}"

    def add(self, path, ifaces):
        self.objects[path] = ifaces
        ids = []
        for iface, props in ifaces.items():
            xml = f'<node><interface name="{iface}">{METHODS.get(iface, "")}'
            for name, value in props.items():
                access = "readwrite" if (iface, name) in WRITABLE else "read"
                xml += f'<property name="{name}" type="{value.get_type_string()}" access="{access}"/>'
            xml += "</interface></node>"
            info = Gio.DBusNodeInfo.new_for_xml(xml).interfaces[0]
            ids.append(self.bus.register_object(path, info, self.on_call, self.on_get, self.on_set))
        self.registrations[path] = ids
        public = {i: p for i, p in ifaces.items() if i not in INTERNAL}
        if public:
            self.bus.emit_signal(None, OM_PATH, OM, "InterfacesAdded", V("(oa{sa{sv}})", (path, public)))

    def remove(self, path):
        if path not in self.objects:
            return
        ifaces = [i for i in self.objects[path] if i not in INTERNAL]
        for rid in self.registrations.pop(path, []):
            self.bus.unregister_object(rid)
        del self.objects[path]
        self.bus.emit_signal(None, OM_PATH, OM, "InterfacesRemoved", V("(oas)", (path, ifaces)))

    def set(self, path, iface, **changes):
        props = self.objects[path][iface]
        changed = {}
        for name, value in changes.items():
            if props.get(name) is None or not props[name].equal(value):
                props[name] = value
                changed[name] = value
        if changed:
            self.bus.emit_signal(None, path, PROPS, "PropertiesChanged", V("(sa{sv}as)", (iface, changed, [])))
            if path == NM_PATH and "State" in changed:
                self.bus.emit_signal(None, path, NM, "StateChanged", V("(u)", (changed["State"].unpack(),)))

    def get(self, path, iface, name):
        return self.objects[path][iface][name].unpack()

    # ---- D-Bus callbacks --------------------------------------------------
    def on_get(self, bus, sender, path, iface, name):
        return self.objects[path][iface][name]

    def on_set(self, bus, sender, path, iface, name, value):
        log(f"Set {iface}.{name} = {value.unpack()} (von {sender})")
        if (iface, name) == (NM, "WirelessEnabled"):
            self.set_wireless(value.unpack())
        elif (iface, name) == (NM, "WwanEnabled"):
            self.set(NM_PATH, NM, WwanEnabled=V("b", value.unpack()))
        return True

    def on_call(self, bus, sender, path, iface, method, args, inv):
        try:
            result = self.dispatch(sender, path, iface, method, args.unpack(), inv)
        except Exception as e:  # noqa: BLE001 - every error goes back to the caller
            log(f"Fehler {method}: {e}")
            inv.return_dbus_error("org.freedesktop.NetworkManager.Failed", str(e))
            return
        if result is not None:
            inv.return_value(result)

    def dispatch(self, sender, path, iface, method, args, inv):
        if iface == OM and method == "GetManagedObjects":
            objects = {p: {i: dict(props) for i, props in ifs.items() if i not in INTERNAL}
                       for p, ifs in self.objects.items()}
            objects = {p: i for p, i in objects.items() if i}
            log(f"GetManagedObjects ({len(objects)} Objekte, von {sender})")
            return V("(a{oa{sa{sv}}})", (objects,))
        if iface == CTL:
            log(f"Befehl: {args[0]}")
            return V("(s)", (self.command(args[0]),))
        if iface == NM:
            if method == "GetDevices":
                return V("(ao)", (self.get(NM_PATH, NM, "Devices"),))
            if method == "ActivateConnection":
                profile, device, specific = args
                log(f"ActivateConnection {profile} {device} {specific} (von {sender})")
                if profile == NONE:
                    profile = self.wired_profile
                active = self.activate(profile, device, specific, sender)
                return V("(o)", (active,))
            if method == "AddAndActivateConnection":
                settings, device, specific = args
                ssid = bytes(settings.get("802-11-wireless", {}).get("ssid", b"")).decode(errors="replace")
                psk = settings.get("802-11-wireless-security", {}).get("psk")
                hidden = settings.get("802-11-wireless", {}).get("hidden", False)
                log(f"AddAndActivateConnection ssid={ssid} key-mgmt="
                    f"{settings.get('802-11-wireless-security', {}).get('key-mgmt', 'keine')} "
                    f"psk={'(' + str(len(psk)) + ' Zeichen)' if psk else 'keins'} hidden={hidden} (von {sender})")
                profile = self.add_profile(settings, psk)
                if specific == NONE:
                    specific = self.ap_by_ssid(ssid) or NONE
                if specific == NONE and hidden and self.wifi:
                    # a hidden network: its access point has no name
                    specific = self.new_ap("", 70, "wpa2")
                active = self.activate(profile, device, specific, sender)
                return V("(oo)", (profile, active))
            if method == "DeactivateConnection":
                log(f"DeactivateConnection {args[0]}")
                self.deactivate_active(args[0])
                return V("()", ())
        if iface == DEV and method == "Disconnect":
            log(f"Disconnect {path} (von {sender})")
            active = self.get(path, DEV, "ActiveConnection")
            if active == NONE:
                raise RuntimeError("Gerät ist nicht verbunden")
            self.deactivate_active(active)
            return V("()", ())
        if iface == WIRELESS:
            if method == "RequestScan":
                log(f"RequestScan {path}")
                self.set(path, WIRELESS, LastScan=V("x", int(time.monotonic() * 1000)))
                return V("()", ())
            if method == "GetAllAccessPoints":
                return V("(ao)", (self.get(path, WIRELESS, "AccessPoints"),))
        if iface == SETTINGS and method == "ListConnections":
            return V("(ao)", (list(self.profile_settings),))
        if iface == SCONN and method == "GetSettings":
            log(f"GetSettings {path}")
            return V("(a{sa{sv}})", (self.profile_settings[path],))
        if iface == AGENTS:
            if method == "Register":
                self.agent = sender
                log(f"Agent angemeldet: {args[0]} ({sender})")
                return V("()", ())
            if method == "Unregister":
                self.agent = None
                log("Agent abgemeldet")
                return V("()", ())
        raise RuntimeError(f"unbekannt: {iface}.{method}")

    # ---- Model ----------------------------------------------------------
    def build(self):
        self.add(SETTINGS_PATH, {SETTINGS: {"Connections": V("ao", [])}})
        self.add("/org/freedesktop/NetworkManager/AgentManager", {AGENTS: {}})
        self.wired_profile = self.add_profile({
            "connection": {"id": V("s", "Kabelgebundene Verbindung 1"), "type": V("s", "802-3-ethernet")},
        }, None, settings_variant=True)
        self.home_profile = self.add_profile({
            "connection": {"id": V("s", "Testnetz"), "type": V("s", "802-11-wireless")},
            "802-11-wireless": {"ssid": V("ay", b"Testnetz"), "mode": V("s", "infrastructure")},
            "802-11-wireless-security": {"key-mgmt": V("s", "wpa-psk"), "psk-flags": V("u", 1)},
        }, None, settings_variant=True)
        self.wired = NM_PATH + "/Devices/1"
        self.add(self.wired, {
            DEV: {"Interface": V("s", "enp3s0"), "DeviceType": V("u", 1), "State": V("u", 30),
                  "Driver": V("s", "r8169"), "HwAddress": V("s", "52:54:00:3A:7C:11"),
                  "ActiveConnection": V("o", NONE), "Ip4Config": V("o", NONE), "Ip6Config": V("o", NONE),
                  "Managed": V("b", True)},
            WIRED: {"HwAddress": V("s", "52:54:00:3A:7C:11"), "Carrier": V("b", True), "Speed": V("u", 1000)},
        })
        devices = [self.wired]
        self.aps = {}
        if not OHNE_WLAN:
            self.wifi = NM_PATH + "/Devices/2"
            self.add(self.wifi, {
                DEV: {"Interface": V("s", "wlp2s0"), "DeviceType": V("u", 2), "State": V("u", 30),
                      "Driver": V("s", "iwlwifi"), "HwAddress": V("s", "52:54:00:B4:21:9E"),
                      "ActiveConnection": V("o", NONE), "Ip4Config": V("o", NONE), "Ip6Config": V("o", NONE),
                      "Managed": V("b", True)},
                WIRELESS: {"HwAddress": V("s", "52:54:00:B4:21:9E"), "AccessPoints": V("ao", []),
                           "ActiveAccessPoint": V("o", NONE), "Bitrate": V("u", 0), "LastScan": V("x", 0)},
            })
            devices.append(self.wifi)
            for ssid, strength, sec in (("Testnetz", 82, "wpa2"), ("Cafe offen", 45, "offen"), ("Nachbar", 20, "wpa3")):
                self.new_ap(ssid, strength, sec)
        else:
            self.wifi = None
        self.add(NM_PATH, {NM: {
            "Devices": V("ao", devices), "AllDevices": V("ao", devices), "ActiveConnections": V("ao", []),
            "PrimaryConnection": V("o", NONE), "NetworkingEnabled": V("b", True),
            "WirelessEnabled": V("b", True), "WirelessHardwareEnabled": V("b", True),
            "WwanEnabled": V("b", True), "WwanHardwareEnabled": V("b", True),
            "State": V("u", 20), "Connectivity": V("u", 1), "Version": V("s", "1.54.0-attrappe")}})
        self.add(CTL_PATH, {CTL: {}})
        # the cable is plugged in and connected from the start
        self.activate(self.wired_profile, self.wired, NONE, None, quick=True)

    def new_ap(self, ssid, strength, sec):
        path = self.new_path("AccessPoint")
        flags, wpa, rsn = {"offen": (0, 0, 0), "wpa2": (1, 0, 0x188), "wpa3": (1, 0, 0x400)}[sec]
        self.add(path, {AP: {"Ssid": V("ay", ssid.encode()), "Strength": V("y", strength),
                             "Flags": V("u", flags), "WpaFlags": V("u", wpa), "RsnFlags": V("u", rsn),
                             "Frequency": V("u", 2437 if len(self.aps) % 2 == 0 else 5180),
                             "HwAddress": V("s", f"AA:BB:CC:00:00:{len(self.aps) + 1:02X}"),
                             "Mode": V("u", 2), "MaxBitrate": V("u", 270000)}})
        self.aps[path] = sec
        if self.get_safe(NM_PATH, NM, "WirelessEnabled", True) and self.wifi:
            aps = self.get(self.wifi, WIRELESS, "AccessPoints") + [path]
            self.set(self.wifi, WIRELESS, AccessPoints=V("ao", aps))
        return path

    def get_safe(self, path, iface, name, default):
        try:
            return self.get(path, iface, name)
        except KeyError:
            return default

    def ap_by_ssid(self, ssid):
        for path in self.aps:
            if bytes(self.get(path, AP, "Ssid")).decode() == ssid:
                return path
        return None

    def add_profile(self, settings, psk, settings_variant=False):
        if not settings_variant:
            converted = {}
            for group, values in settings.items():
                converted[group] = {}
                for k, v in values.items():
                    if k == "psk":
                        continue
                    if isinstance(v, (bytes, bytearray)) or (isinstance(v, list) and k == "ssid"):
                        converted[group][k] = V("ay", bytes(v))
                    elif isinstance(v, bool):
                        converted[group][k] = V("b", v)
                    elif isinstance(v, int):
                        converted[group][k] = V("u", v)
                    else:
                        converted[group][k] = V("s", str(v))
            settings = converted
        path = self.new_path("Settings")
        self.profile_settings[path] = settings
        if psk is not None:
            self.secrets[path] = psk
        self.add(path, {SCONN: {"Unsaved": V("b", False), "Flags": V("u", 0), "Filename": V("s", "")}})
        self.set(SETTINGS_PATH, SETTINGS, Connections=V("ao", list(self.profile_settings)))
        return path

    def profile_ssid(self, profile):
        ssid = self.profile_settings[profile].get("802-11-wireless", {}).get("ssid")
        return bytes(ssid.unpack()).decode() if ssid is not None else None

    def update_manager(self):
        actives = [p for p, i in self.objects.items() if ACTIVE in i]
        primary = NONE
        for path in actives:
            if self.get(path, ACTIVE, "State") == 2:
                devs = self.get(path, ACTIVE, "Devices")
                if devs and devs[0] == self.wired:
                    primary = path
                    break
                if primary == NONE:
                    primary = path
        connected = primary != NONE
        self.set(NM_PATH, NM, ActiveConnections=V("ao", actives), PrimaryConnection=V("o", primary),
                 State=V("u", 70 if connected else 20))

    def activate(self, profile, device, specific, sender, quick=False):
        if device == self.wired and not self.get(self.wired, WIRED, "Carrier"):
            raise RuntimeError("Kein Kabel angeschlossen")
        if self.wifi and device == self.wifi and not self.get(NM_PATH, NM, "WirelessEnabled"):
            raise RuntimeError("WLAN ist ausgeschaltet")
        old = self.get(device, DEV, "ActiveConnection")
        if old != NONE:
            self.deactivate_active(old, quiet=True)
        active = self.new_path("ActiveConnection")
        conn_id = self.profile_settings[profile]["connection"]["id"].unpack()
        ctype = self.profile_settings[profile]["connection"]["type"].unpack()
        self.add(active, {ACTIVE: {"Connection": V("o", profile), "SpecificObject": V("o", specific),
                                   "Id": V("s", conn_id), "Uuid": V("s", f"0000-{self.counter}"),
                                   "Type": V("s", ctype), "Devices": V("ao", [device]), "State": V("u", 1),
                                   "Default": V("b", False), "Vpn": V("b", False)}})
        self.set(device, DEV, ActiveConnection=V("o", active), State=V("u", 40))
        if device == self.wifi:
            self.set(self.wifi, WIRELESS, ActiveAccessPoint=V("o", specific))
        self.update_manager()
        if quick:
            self.finish(device, active)
            return active
        GLib.timeout_add(700, self.after_prepare, device, active, profile, 0)
        return active

    def after_prepare(self, device, active, profile, attempt):
        if self.get_safe(device, DEV, "ActiveConnection", NONE) != active:
            return False
        ssid = self.profile_ssid(profile)
        sec = self.profile_settings[profile].get("802-11-wireless-security", {})
        key_mgmt = sec.get("key-mgmt").unpack() if "key-mgmt" in sec else None
        if key_mgmt and profile not in self.secrets:
            self.set(device, DEV, State=V("u", 60))
            self.ask_agent(device, active, profile, attempt)
            return False
        if key_mgmt:
            right = {"Testnetz": "geheim123", "Nachbar": "nachbar99"}.get(ssid)
            if right is not None and self.secrets[profile] != right:
                log(f"Schlüssel für {ssid} falsch")
                del self.secrets[profile]
                if attempt >= 2:
                    self.fail(device, active)
                    return False
                self.set(device, DEV, State=V("u", 60))
                self.ask_agent(device, active, profile, attempt + 1, new=True)
                return False
        self.set(device, DEV, State=V("u", 70))
        GLib.timeout_add(700, self.finish, device, active)
        return False

    def ask_agent(self, device, active, profile, attempt, new=False):
        if not self.agent:
            log("kein Agent: Verbindung scheitert (keine Geheimnisse)")
            self.fail(device, active)
            return
        flags = 0x1 | (0x2 if new else 0)
        log(f"GetSecrets an {self.agent} für {self.profile_ssid(profile)} (Flags {flags})")
        args = V("(a{sa{sv}}osasu)", (self.profile_settings[profile], profile, "802-11-wireless-security", [], flags))

        def done(bus, result):
            try:
                reply = bus.call_finish(result).unpack()[0]
                psk = reply["802-11-wireless-security"]["psk"]
                log(f"GetSecrets beantwortet ({len(psk)} Zeichen)")
                self.secrets[profile] = psk
                self.after_prepare(device, active, profile, attempt)
            except GLib.Error as e:
                log(f"GetSecrets abgelehnt: {e.message}")
                self.fail(device, active)
        self.bus.call(self.agent, "/org/freedesktop/NetworkManager/SecretAgent", AGENT, "GetSecrets", args,
                      GLib.VariantType("(a{sa{sv}})"), Gio.DBusCallFlags.NONE, 120000, None, done)

    def finish(self, device, active):
        if self.get_safe(device, DEV, "ActiveConnection", NONE) != active:
            return False
        ip4 = self.new_path("IP4Config")
        wired = device == self.wired
        address = "192.0.2.24" if wired else "192.0.2.37"
        self.add(ip4, {IP4: {"AddressData": V("aa{sv}", [{"address": V("s", address), "prefix": V("u", 24)}]),
                             "Gateway": V("s", "192.0.2.1"),
                             "NameserverData": V("aa{sv}", [{"address": V("s", "192.0.2.1")}])}})
        ip6 = self.new_path("IP6Config")
        suffix = "5054:ff:fe3a:7c11" if wired else "5054:ff:feb4:219e"
        self.add(ip6, {IP6: {"AddressData": V("aa{sv}", [{"address": V("s", f"fd00::{suffix}"), "prefix": V("u", 64)},
                                                         {"address": V("s", f"fe80::{suffix}"), "prefix": V("u", 64)}]),
                             "Gateway": V("s", "fe80::1"), "Nameservers": V("aay", [])}})
        self.set(device, DEV, Ip4Config=V("o", ip4), Ip6Config=V("o", ip6), State=V("u", 100))
        if not wired:
            self.set(self.wifi, WIRELESS, Bitrate=V("u", 144400))
        self.set(active, ACTIVE, State=V("u", 2))
        self.update_manager()
        log(f"aktiviert: {self.get(active, ACTIVE, 'Id')} auf {self.get(device, DEV, 'Interface')}")
        return False

    def fail(self, device, active):
        log(f"fehlgeschlagen: {self.get_safe(active, ACTIVE, 'Id', '?')}")
        self.set(device, DEV, State=V("u", 120))
        GLib.timeout_add(500, lambda: (self.deactivate_active(active, quiet=True), False)[1])

    def deactivate_active(self, active, quiet=False):
        if active not in self.objects:
            return
        device = self.get(active, ACTIVE, "Devices")[0]
        for key in ("Ip4Config", "Ip6Config"):
            cfg = self.get(device, DEV, key)
            if cfg != NONE:
                self.remove(cfg)
        new_state = 30
        if device == self.wired and not self.get(self.wired, WIRED, "Carrier"):
            new_state = 20
        if self.wifi and device == self.wifi:
            self.set(self.wifi, WIRELESS, ActiveAccessPoint=V("o", NONE), Bitrate=V("u", 0))
            if not self.get(NM_PATH, NM, "WirelessEnabled"):
                new_state = 20
        self.set(device, DEV, ActiveConnection=V("o", NONE), Ip4Config=V("o", NONE), Ip6Config=V("o", NONE),
                 State=V("u", new_state))
        self.remove(active)
        self.update_manager()
        if not quiet:
            log(f"getrennt: {self.get(device, DEV, 'Interface')}")

    def set_wireless(self, on):
        if not self.wifi:
            self.set(NM_PATH, NM, WirelessEnabled=V("b", on))
            return
        self.set(NM_PATH, NM, WirelessEnabled=V("b", on))
        if on:
            self.set(self.wifi, DEV, State=V("u", 30))
            self.set(self.wifi, WIRELESS, AccessPoints=V("ao", list(self.aps)))
        else:
            active = self.get(self.wifi, DEV, "ActiveConnection")
            if active != NONE:
                self.deactivate_active(active)
            self.set(self.wifi, WIRELESS, AccessPoints=V("ao", []))
            self.set(self.wifi, DEV, State=V("u", 20))

    # ---- Test commands ---------------------------------------------------
    def command(self, text):
        words = text.split()
        if not words:
            return "leer"
        cmd = words[0]
        if cmd == "kabel" and words[1:] == ["ab"]:
            self.set(self.wired, WIRED, Carrier=V("b", False), Speed=V("u", 0))
            active = self.get(self.wired, DEV, "ActiveConnection")
            if active != NONE:
                self.deactivate_active(active)
            self.set(self.wired, DEV, State=V("u", 20))
            return "ok"
        if cmd == "kabel" and words[1:] == ["an"]:
            self.set(self.wired, WIRED, Carrier=V("b", True), Speed=V("u", 1000))
            self.set(self.wired, DEV, State=V("u", 30))
            self.activate(self.wired_profile, self.wired, NONE, None)
            return "ok"
        if cmd == "signal":
            ssid, strength = " ".join(words[1:-1]), int(words[-1])
            path = self.ap_by_ssid(ssid)
            if not path:
                return f"kein Netz {ssid}"
            self.set(path, AP, Strength=V("y", strength))
            return "ok"
        if cmd == "netz-neu":
            self.new_ap(" ".join(words[1:-2]), int(words[-2]), words[-1])
            return "ok"
        if cmd == "netz-weg":
            path = self.ap_by_ssid(" ".join(words[1:]))
            if not path:
                return "kein Netz"
            aps = [p for p in self.get(self.wifi, WIRELESS, "AccessPoints") if p != path]
            self.set(self.wifi, WIRELESS, AccessPoints=V("ao", aps))
            del self.aps[path]
            self.remove(path)
            return "ok"
        if cmd == "wlan-schalter":
            self.set(NM_PATH, NM, WirelessHardwareEnabled=V("b", words[1] == "an"))
            return "ok"
        if cmd == "zustand":
            return json.dumps(self.snapshot(), ensure_ascii=False, sort_keys=True)
        if cmd == "beenden":
            GLib.timeout_add(100, LOOP.quit)
            return "ok"
        return f"unbekannt: {text}"

    def snapshot(self):
        out = {"wlan": self.get(NM_PATH, NM, "WirelessEnabled"), "wwan": self.get(NM_PATH, NM, "WwanEnabled"),
               "agent": self.agent, "geraete": {}, "profile": []}
        for dev in self.get(NM_PATH, NM, "Devices"):
            active = self.get(dev, DEV, "ActiveConnection")
            out["geraete"][self.get(dev, DEV, "Interface")] = {
                "zustand": self.get(dev, DEV, "State"),
                "verbindung": self.get(active, ACTIVE, "Id") if active != NONE else None}
        for path, s in self.profile_settings.items():
            out["profile"].append(s["connection"]["id"].unpack())
        return out


SETTINGS_PATH = NM_PATH + "/Settings"
LOOP = GLib.MainLoop()


def main():
    address = os.environ.get("DBUS_SYSTEM_BUS_ADDRESS", "")
    rt = os.environ.get("XDG_RUNTIME_DIR", "")
    # Never the real system bus: only a bus in the isolated runtime directory.
    if "/nachweis/gui/rt/g2m-" not in rt or rt not in address:
        print("qnm: nur mit eigenem Bus im isolierten Laufzeitordner", file=sys.stderr)
        return 2
    bus = Gio.DBusConnection.new_for_address_sync(
        address, Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION,
        None, None)
    attrappe = Attrappe(bus)
    # the object manager first, then the objects, then the name
    attrappe.objects[OM_PATH] = {OM: {}}
    info = Gio.DBusNodeInfo.new_for_xml(f'<node><interface name="{OM}">{METHODS[OM]}</interface></node>').interfaces[0]
    bus.register_object(OM_PATH, info, attrappe.on_call, None, None)
    attrappe.build()

    def owner_changed(bus, sender, path, iface, signal, params):
        name, old, new = params.unpack()
        if name == attrappe.agent and not new:
            log(f"Agent {name} ist weg")
            attrappe.agent = None
    bus.signal_subscribe("org.freedesktop.DBus", "org.freedesktop.DBus", "NameOwnerChanged",
                         "/org/freedesktop/DBus", None, Gio.DBusSignalFlags.NONE, owner_changed)
    reply = bus.call_sync("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "RequestName",
                          V("(su)", (NM, 4)), None, Gio.DBusCallFlags.NONE, -1, None)
    log(f"Attrappe bereit als {NM} ({bus.get_unique_name()}, RequestName {reply.unpack()[0]}),"
        f" WLAN {'nein' if OHNE_WLAN else 'ja'}")
    LOOP.run()
    log("Attrappe beendet")
    return 0


if __name__ == "__main__":
    sys.exit(main())
