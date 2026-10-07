#!/usr/bin/env python3
"""Notification delivery and default-action routing on a private D-Bus (no real account)."""
import os
from pathlib import Path
import subprocess
import sys
import time

if '--private-bus' not in sys.argv:
    raise SystemExit(subprocess.call(['dbus-run-session','--','/usr/bin/python3',__file__,'--private-bus']))
from gi.repository import Gio, GLib
ROOT=Path(__file__).resolve().parents[1]
xml='''<node><interface name="org.freedesktop.Notifications">
<method name="GetCapabilities"><arg type="as" direction="out"/></method>
<method name="GetServerInformation"><arg type="s" direction="out"/><arg type="s" direction="out"/><arg type="s" direction="out"/><arg type="s" direction="out"/></method>
<method name="Notify"><arg type="s" direction="in"/><arg type="u" direction="in"/><arg type="s" direction="in"/><arg type="s" direction="in"/><arg type="s" direction="in"/><arg type="as" direction="in"/><arg type="a{sv}" direction="in"/><arg type="i" direction="in"/><arg type="u" direction="out"/></method>
<method name="CloseNotification"><arg type="u" direction="in"/></method>
<signal name="ActionInvoked"><arg type="u"/><arg type="s"/></signal>
<signal name="NotificationClosed"><arg type="u"/><arg type="u"/></signal>
</interface></node>'''
loop=GLib.MainLoop(); bus=Gio.bus_get_sync(Gio.BusType.SESSION,None); errors=[]; delivered=[]
def action():
    bus.emit_signal(None,'/org/freedesktop/Notifications','org.freedesktop.Notifications','ActionInvoked',GLib.Variant('(us)',(42,'default')))
    return False
def method(connection,sender,path,interface,name,args,invocation):
    try:
        if name=='GetCapabilities': invocation.return_value(GLib.Variant('(as)',(['actions','body'],)))
        elif name=='GetServerInformation': invocation.return_value(GLib.Variant('(ssss)',('Archaic fixture','Archaic','1','1.2')))
        elif name=='Notify':
            values=args.unpack()
            assert values[3]=='Archaic'
            assert values[4]=='Room &lt;unsafe&gt; &amp; label',values[4]
            assert values[5]==['default','Archaic'],values[5]
            delivered.append(42); invocation.return_value(GLib.Variant('(u)',(42,)))
            GLib.timeout_add(500,action)
        else: invocation.return_value(None)
    except Exception as error:
        errors.append(error); invocation.return_dbus_error('org.archaic.TestError',str(error)); loop.quit()
bus.register_object('/org/freedesktop/Notifications',Gio.DBusNodeInfo.new_for_xml(xml).interfaces[0],method,None,None)
owner=Gio.bus_own_name_on_connection(bus,'org.freedesktop.Notifications',Gio.BusNameOwnerFlags.NONE,None,None)
# Build separately, then execute the test binary: cargo diagnostics must not delay the bus fixture.
command=['cargo','test','--locked','-p','archaic','native_notification_activation','--','--ignored','--nocapture']
config=os.environ.get('ARCHAIC_TEST_CARGO_CONFIG')
if config: command[2:2]=['--config',config]
app=subprocess.Popen(command,cwd=ROOT,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
started=time.monotonic()
def watch():
    if app.poll() is not None:
        loop.quit(); return False
    if time.monotonic()-started>180:
        errors.append(TimeoutError('notification integration test deadline')); loop.quit(); return False
    return True
GLib.timeout_add(100,watch)
try: loop.run()
finally:
    if app.poll() is None: app.terminate()
    output=app.communicate(timeout=10)[0].decode(); Gio.bus_unown_name(owner)
print(output)
if errors: raise errors[0]
assert app.returncode==0,app.returncode
assert delivered==[42],delivered
print('Native notification text, default action, and account/event activation passed.')
