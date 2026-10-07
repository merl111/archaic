#!/usr/bin/env python3
"""Linux tray protocol/activation test on a private D-Bus and Xvfb; no Matrix account."""
import os
from pathlib import Path
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
if "--private-bus" not in sys.argv:
    raise SystemExit(subprocess.call(["dbus-run-session", "--", "/usr/bin/python3", __file__, "--private-bus"]))
from gi.repository import Gio, GLib
xml = """<node><interface name="org.kde.StatusNotifierWatcher">
<method name="RegisterStatusNotifierItem"><arg type="s" direction="in"/></method>
<property name="RegisteredStatusNotifierItems" type="as" access="read"/>
<property name="IsStatusNotifierHostRegistered" type="b" access="read"/>
<property name="ProtocolVersion" type="i" access="read"/>
<signal name="StatusNotifierItemRegistered"><arg type="s"/></signal>
</interface></node>"""
loop = GLib.MainLoop()
bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
registered = []
errors = []
started = time.monotonic()
checked = False
app = None


def get_property(connection, sender, path, interface, name):
    return {"RegisteredStatusNotifierItems": GLib.Variant("as", registered),
            "IsStatusNotifierHostRegistered": GLib.Variant("b", True),
            "ProtocolVersion": GLib.Variant("i", 0)}[name]


def method(connection, sender, path, interface, name, args, invocation):
    service = args.unpack()[0]
    registered.append((sender + service) if service.startswith("/") else service + "/StatusNotifierItem")
    invocation.return_value(None)
    GLib.timeout_add(600, verify, sender, "/StatusNotifierItem")


def call(destination, path, interface, method, args):
    return bus.call_sync(destination, path, interface, method, args, None, Gio.DBusCallFlags.NONE, 5000, None).unpack()


def verify(sender, path):
    global checked
    try:
        properties = call(sender, path, "org.freedesktop.DBus.Properties", "GetAll", GLib.Variant("(s)", ("org.kde.StatusNotifierItem",)))[0]
        assert properties["Status"] == "Active"
        assert properties["IconPixmap"][0][:2] == (32, 32)
        menu = properties["Menu"]
        layout = call(sender, menu, "com.canonical.dbusmenu", "GetLayout", GLib.Variant("(iias)", (0, -1, [])))[1]
        children = layout[2]
        labels = {node[1].get("label"): node[0] for node in children}
        assert "Show Archaic" in labels and "Quit Archaic" in labels, labels
        assert "No unread activity" in labels, labels
        call(sender, menu, "com.canonical.dbusmenu", "Event", GLib.Variant("(isvu)", (labels["Show Archaic"], "clicked", GLib.Variant("i", 0), 0)))
        call(sender, menu, "com.canonical.dbusmenu", "Event", GLib.Variant("(isvu)", (labels["Quit Archaic"], "clicked", GLib.Variant("i", 0), 0)))
        checked = True
    except Exception as error:
        errors.append(error)
        loop.quit()
    return False


def watch():
    if app.poll() is not None:
        if not checked or app.returncode != 0:
            errors.append(RuntimeError(f"unexpected exit: {app.returncode}; {app.stderr.read().decode()}"))
        loop.quit()
        return False
    if time.monotonic() - started > 25:
        errors.append(TimeoutError("Tray did not register/quit before the deadline"))
        loop.quit()
        return False
    return True


bus.register_object("/StatusNotifierWatcher", Gio.DBusNodeInfo.new_for_xml(xml).interfaces[0], method, get_property, None)
owner = Gio.bus_own_name_on_connection(bus, "org.kde.StatusNotifierWatcher", Gio.BusNameOwnerFlags.NONE, None, None)
xserver = subprocess.Popen(["Xvfb", "-displayfd", "1", "-screen", "0", "1280x900x24", "-nolisten", "tcp"], stdout=subprocess.PIPE, stderr=subprocess.PIPE)
try:
    display = ":" + xserver.stdout.readline().decode().strip()
    app = subprocess.Popen([str(ROOT / "target/debug/archaic"), "--temporary-session"], cwd=ROOT,
        env=dict(os.environ, DISPLAY=display, GDK_BACKEND="x11", GSK_RENDERER="cairo", GTK_USE_PORTAL="0"), stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    GLib.timeout_add(100, watch)
    loop.run()
finally:
    if app is not None and app.poll() is None:
        app.terminate()
        app.wait(timeout=5)
    xserver.terminate()
    xserver.wait(timeout=5)
    Gio.bus_unown_name(owner)
if errors:
    raise errors[0]
assert checked
print("Native tray registration, RGBA icon, menu labels, Show action, and orderly Quit passed.")
