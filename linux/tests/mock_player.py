#!/usr/bin/env python3
"""A fake MPRIS player for tests: plays 'Test Song' by 'Test Artist', changes song after 4 s, and logs the commands it gets."""
import sys
from gi.repository import Gio, GLib

XML = """
<node>
 <interface name="org.mpris.MediaPlayer2"><property name="Identity" type="s" access="read"/></interface>
 <interface name="org.mpris.MediaPlayer2.Player">
  <method name="PlayPause"/><method name="Next"/><method name="Previous"/>
  <method name="Seek"><arg type="x" direction="in"/></method>
  <method name="SetPosition"><arg type="o" direction="in"/><arg type="x" direction="in"/></method>
  <property name="PlaybackStatus" type="s" access="read"/>
  <property name="Metadata" type="a{sv}" access="read"/>
  <property name="Position" type="x" access="read"/>
  <property name="CanSeek" type="b" access="read"/>
 </interface>
</node>"""
state = {"title": "Test Song", "playing": True, "pos": 30_000_000}

def meta():
    return {
        "mpris:trackid": GLib.Variant("o", "/mock/track1"),
        "mpris:length": GLib.Variant("x", 200_000_000),
        "xesam:title": GLib.Variant("s", state["title"]),
        "xesam:artist": GLib.Variant("as", ["Test Artist"]),
        "xesam:album": GLib.Variant("s", "Test Album"),
    }

def get_prop(conn, sender, path, iface, name):
    if iface.endswith("Player"):
        return {"PlaybackStatus": GLib.Variant("s", "Playing" if state["playing"] else "Paused"),
                "Metadata": GLib.Variant("a{sv}", meta()),
                "Position": GLib.Variant("x", state["pos"]),
                "CanSeek": GLib.Variant("b", True)}.get(name)
    return {"Identity": GLib.Variant("s", "Mock Player")}.get(name)

def call(conn, sender, path, iface, method, params, inv):
    print("CALL", method, params.unpack(), flush=True)
    if method == "PlayPause": state["playing"] = not state["playing"]
    if method == "SetPosition": state["pos"] = params.unpack()[1]
    inv.return_value(None)

def on_bus(conn, name):
    info = Gio.DBusNodeInfo.new_for_xml(XML)
    for i in info.interfaces:
        conn.register_object("/org/mpris/MediaPlayer2", i, call, get_prop, None)
    def change():
        state["title"] = "Second Song"
        conn.emit_signal(None, "/org/mpris/MediaPlayer2", "org.freedesktop.DBus.Properties", "PropertiesChanged",
            GLib.Variant("(sa{sv}as)", ("org.mpris.MediaPlayer2.Player", {"Metadata": GLib.Variant("a{sv}", meta())}, [])))
        return False
    GLib.timeout_add_seconds(4, change)

Gio.bus_own_name(Gio.BusType.SESSION, "org.mpris.MediaPlayer2.mock", Gio.BusNameOwnerFlags.NONE, on_bus, None, None)
GLib.MainLoop().run()
