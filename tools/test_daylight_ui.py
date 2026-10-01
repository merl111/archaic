#!/usr/bin/env python3
"""Native Daylight interaction regression check (Linux GTK/X11).

Build with tools/dev.py first. Requires Xvfb, libXtst and ImageMagick:
  xvfb-run -a -s '-screen 0 1600x1050x24' python3 tools/test_daylight_ui.py
Uses a private loopback Matrix fixture and temporary in-memory SDK session.
No real homeserver, account or credential vault is accessed. Pixel coordinates
assume the default GTK font at 100%; protocol request assertions are the oracle.
Screenshots/runtime log go to target/daylight-ui (override with --output).
"""
import ctypes as c
import argparse
import json
import os
import subprocess
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import urlparse, unquote
requests = []
room = '!studio:local'
message_events = [{'type': 'm.room.message', 'sender': '@bob:local', 'event_id': '$bob', 'origin_server_ts': 1790884800000, 'content': {'msgtype': 'm.text', 'body': 'Hello from Bob'}}, {'type': 'm.room.message', 'sender': '@alice:local', 'event_id': '$alice', 'origin_server_ts': 1790884860000, 'content': {'msgtype': 'm.text', 'body': 'My original message'}}]
action_writes = []

class Handler(BaseHTTPRequestHandler):

    def log_message(self, *args):
        pass

    def reply(self, body, status=200):
        data = json.dumps(body).encode()
        self.send_response(status)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(data)))
        self.end_headers()
        try:
            self.wfile.write(data)
        except BrokenPipeError:
            pass

    def do_GET(self):
        path = unquote(urlparse(self.path).path)
        requests.append(('GET', path, None))
        if path.endswith('/devices'):
            return self.reply({'devices': [{'device_id': 'UI_TEST', 'display_name': 'This test device'}, {'device_id': 'OTHER', 'display_name': 'Second test device'}]})
        if path.endswith('/media/config'):
            return self.reply({'m.upload.size': 30000000})
        if '/relations/' in path:
            target = path.split('/relations/')[1].split('/')[0]
            kind = 'm.thread' if '/m.thread' in path else 'm.annotation' if '/m.annotation' in path else 'm.replace'
            return self.reply({'chunk': [e for e in message_events if e['content'].get('m.relates_to', {}).get('event_id') == target and e['content'].get('m.relates_to', {}).get('rel_type') == kind]})
        if '/download/local/file' in path:
            data = b'Native attachment bytes'
            self.send_response(200)
            self.send_header('Content-Type', 'application/octet-stream')
            self.send_header('Content-Length', str(len(data)))
            self.end_headers()
            self.wfile.write(data)
            return
        if '/directory/room/' in path:
            return self.reply({'room_id': '!new:local', 'servers': ['local']})
        if path.endswith('/versions'):
            return self.reply({'versions': ['v1.11']})
        if path.endswith('/sync'):
            time.sleep(0.15)
            state = [{'type': 'm.room.create', 'state_key': '', 'sender': '@alice:local', 'event_id': '$create', 'origin_server_ts': 1, 'content': {'creator': '@alice:local', 'room_version': '10'}}, {'type': 'm.room.member', 'state_key': '@alice:local', 'sender': '@alice:local', 'event_id': '$member', 'origin_server_ts': 2, 'content': {'membership': 'join', 'displayname': 'Alice'}}, {'type': 'm.room.name', 'state_key': '', 'sender': '@alice:local', 'event_id': '$name', 'origin_server_ts': 3, 'content': {'name': 'Native integration test'}}]
            rooms = {room: {'state': {'events': state}, 'timeline': {'events': [], 'limited': False, 'prev_batch': 'history'}, 'unread_notifications': {}, 'summary': {'m.joined_member_count': 2}}}
            return self.reply({'next_batch': 'batch-' + str(len(requests)), 'rooms': {'join': rooms}, 'account_data': {'events': [{'type': 'm.direct', 'content': {}}]}, 'device_one_time_keys_count': {'signed_curve25519': 50}})
        if path.endswith('/account_data/m.direct'):
            return self.reply({})
        if '/event/' in path:
            event = next((e for e in message_events if e['event_id'] == path.rsplit('/', 1)[1]), None)
            if event:
                return self.reply(dict(event, room_id=room))
        if path.endswith('/messages'):
            return self.reply({'start': 'history', 'chunk': list(reversed(message_events)), 'state': []})
        return self.reply({'errcode': 'M_NOT_FOUND', 'error': 'Not found'}, 404)

    def do_POST(self):
        path = unquote(urlparse(self.path).path)
        raw = self.rfile.read(int(self.headers.get('Content-Length', 0)))
        if path.endswith('/upload') and '/keys/' not in path:
            requests.append(('POST', path, raw.decode()))
            return self.reply({'content_uri': 'mxc://local/file'})
        body = json.loads(raw or b'{}')
        requests.append(('POST', path, body))
        if path.endswith('/search'):
            return self.reply({'search_categories': {'room_events': {'results': [{'result': dict(message_events[0], room_id=room)}]}}})
        if path.endswith('/login'):
            return self.reply({'user_id': '@alice:local', 'access_token': 'local-test-token', 'device_id': 'UI_TEST'})
        if path.endswith('/keys/upload'):
            return self.reply({'one_time_key_counts': {'signed_curve25519': 50}})
        if path.endswith('/keys/query'):
            return self.reply({'device_keys': {}})
        return self.reply({})

    def do_PUT(self):
        body = json.loads(self.rfile.read(int(self.headers.get('Content-Length', 0))) or b'{}')
        path = unquote(urlparse(self.path).path)
        requests.append(('PUT', path, body))
        action_writes.append((path, body))
        if '/account_data/' in path:
            return self.reply({})
        eid = '$sent' + str(len(action_writes))
        if '/redact/' in path:
            event = {'type': 'm.room.redaction', 'sender': '@alice:local', 'event_id': eid, 'origin_server_ts': 1790884920000 + len(action_writes), 'redacts': path.split('/redact/')[1].split('/')[0], 'content': {}}
        else:
            kind = path.split('/send/')[1].split('/')[0]
            event = {'type': kind, 'sender': '@alice:local', 'event_id': eid, 'origin_server_ts': 1790884920000 + len(action_writes), 'content': body}
        message_events.append(event)
        return self.reply({'event_id': eid})
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--binary', type=Path, default=Path(__file__).resolve().parents[1] / 'target/debug/archaic')
parser.add_argument('--output', type=Path, default=Path(__file__).resolve().parents[1] / 'target/daylight-ui')
args = parser.parse_args()
output = args.output.resolve()
output.mkdir(parents=True, exist_ok=True)
if not os.environ.get('DISPLAY'):
    parser.error('Run under xvfb-run with a 1600x1050x24 screen (see module docstring).')
env = dict(os.environ, GDK_BACKEND='x11', GDK_SCALE='1', GDK_DPI_SCALE='1', GSK_RENDERER='cairo', GTK_USE_PORTAL='0', DBUS_SESSION_BUS_ADDRESS='unix:path=/tmp/archaic-ui-test-no-bus')
server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
threading.Thread(target=server.serve_forever, daemon=True).start()
log = (output / 'runtime.log').open('w')
app = subprocess.Popen([str(args.binary.resolve()), '--temporary-session', '--theme', 'light'], env=env, stderr=log)
x = c.CDLL('libX11.so.6')
xt = c.CDLL('libXtst.so.6')
x.XOpenDisplay.argtypes = [c.c_char_p]
x.XOpenDisplay.restype = c.c_void_p
display = x.XOpenDisplay(os.environ['DISPLAY'].encode())
assert display
x.XStringToKeysym.argtypes = [c.c_char_p]
x.XStringToKeysym.restype = c.c_ulong
x.XKeysymToKeycode.argtypes = [c.c_void_p, c.c_ulong]
x.XKeysymToKeycode.restype = c.c_uint
x.XFlush.argtypes = [c.c_void_p]
xt.XTestFakeKeyEvent.argtypes = [c.c_void_p, c.c_uint, c.c_int, c.c_ulong]
xt.XTestFakeButtonEvent.argtypes = [c.c_void_p, c.c_uint, c.c_int, c.c_ulong]
xt.XTestFakeMotionEvent.argtypes = [c.c_void_p, c.c_int, c.c_int, c.c_int, c.c_ulong]

def key(name, down):
    code = x.XKeysymToKeycode(display, x.XStringToKeysym(name.encode()))
    assert code
    xt.XTestFakeKeyEvent(display, code, down, 0)
    x.XFlush(display)

def press(name):
    key(name, 1)
    key(name, 0)
    time.sleep(0.2)

def click(px, py):
    xt.XTestFakeMotionEvent(display, -1, px, py, 0)
    xt.XTestFakeButtonEvent(display, 1, 1, 0)
    xt.XTestFakeButtonEvent(display, 1, 0, 0)
    x.XFlush(display)
    time.sleep(0.4)
x.XDefaultRootWindow.argtypes = [c.c_void_p]
x.XDefaultRootWindow.restype = c.c_ulong
x.XQueryTree.argtypes = [c.c_void_p, c.c_ulong, c.POINTER(c.c_ulong), c.POINTER(c.c_ulong), c.POINTER(c.POINTER(c.c_ulong)), c.POINTER(c.c_uint)]
x.XGetGeometry.argtypes = [c.c_void_p, c.c_ulong, c.POINTER(c.c_ulong), c.POINTER(c.c_int), c.POINTER(c.c_int), c.POINTER(c.c_uint), c.POINTER(c.c_uint), c.POINTER(c.c_uint), c.POINTER(c.c_uint)]
x.XFree.argtypes = [c.c_void_p]

def geometry():
    root, parent, children, count = (c.c_ulong(), c.c_ulong(), c.POINTER(c.c_ulong)(), c.c_uint())
    x.XQueryTree(display, x.XDefaultRootWindow(display), c.byref(root), c.byref(parent), c.byref(children), c.byref(count))
    windows = []
    try:
        for i in range(count.value):
            px, py, w, h, border, depth = (c.c_int(), c.c_int(), c.c_uint(), c.c_uint(), c.c_uint(), c.c_uint())
            x.XGetGeometry(display, children[i], c.byref(root), c.byref(px), c.byref(py), c.byref(w), c.byref(h), c.byref(border), c.byref(depth))
            windows.append((w.value * h.value, children[i], w.value, h.value))
    finally:
        if children:
            x.XFree(children)
    _, window, w, h = max(windows)
    return (window, w, h)

def capture(name):
    _, width, height = geometry()
    subprocess.run(['import', '-window', 'root', '-crop', f'{width}x{height}+0+0', '+repage', str(output / (name + '.png'))], env=env, check=True, timeout=15)

def type_text(value):
    for char in value:
        shift = char.isupper() or char in ':@!#'
        name = {',': 'comma', ' ': 'space', ':': 'semicolon', '@': '2', '!': '1', '#': '3', '/': 'slash', '.': 'period', '-': 'minus'}.get(char, char.lower())
        if shift:
            key('Shift_L', 1)
        press_key = x.XKeysymToKeycode(display, x.XStringToKeysym(name.encode()))
        assert press_key
        xt.XTestFakeKeyEvent(display, press_key, 1, 0)
        xt.XTestFakeKeyEvent(display, press_key, 0, 0)
        if shift:
            key('Shift_L', 0)
    x.XFlush(display)
    time.sleep(0.2)

def shortcut(k, shift=False):
    key('Control_L', 1)
    if shift:
        key('Shift_L', 1)
    key(k, 1)
    key(k, 0)
    if shift:
        key('Shift_L', 0)
    key('Control_L', 0)
    time.sleep(0.4)

def wait_for(predicate):
    end = time.monotonic() + 10
    while time.monotonic() < end:
        assert app.poll() is None, 'app exited'
        if predicate():
            return
        time.sleep(0.1)
    capture('archaic-daylight-integration-failed')
    print([(m, p) for m, p, _ in requests])
    raise AssertionError('UI command did not reach test server')
try:
    time.sleep(2)
    capture('login')
    type_text(f'http://127.0.0.1:{server.server_port}')
    press('Tab')
    type_text('alice')
    press('Tab')
    type_text('local-test-password')
    press('Return')
    wait_for(lambda: any((p.endswith('/sync') for _, p, _ in requests)))
    time.sleep(0.5)
    click(120, 232)
    wait_for(lambda: any((p.endswith('/messages') for _, p, _ in requests)))
    time.sleep(0.5)
    capture('chat')
    bottom = geometry()[2] - 54
    click(550, bottom)
    type_text('Ordinary draft survives')
    xt.XTestFakeMotionEvent(display, -1, 440, 250, 0)
    x.XFlush(display)
    time.sleep(0.3)
    click(460, 205)
    capture('inline-reply')
    click(540, bottom)
    type_text('Native inline reply')
    press('Return')
    wait_for(lambda: any((b.get('m.relates_to', {}).get('m.in_reply_to', {}).get('event_id') == '$bob' for _, b in action_writes)))
    time.sleep(0.4)
    click(540, bottom)
    press('Return')
    wait_for(lambda: any((b.get('body') == 'Ordinary draft survives' for _, b in action_writes)))
    print('PASS login, inline reply, ordinary draft preservation and send', flush=True)
    time.sleep(0.4)
    xt.XTestFakeMotionEvent(display, -1, 440, 250, 0)
    x.XFlush(display)
    time.sleep(0.3)
    click(491, 205)
    time.sleep(0.8)
    capture('thread')
    click(1210, bottom)
    type_text('Native thread reply')
    press('Return')
    wait_for(lambda: any((b.get('m.relates_to', {}).get('rel_type') == 'm.thread' for _, b in action_writes)))
    print('PASS thread reply request', flush=True)
    time.sleep(0.5)
    click(1394, 88)
    time.sleep(0.4)
    click(1009, bottom)
    time.sleep(0.3)
    capture('emoji')
    def modal_surface(name):
        # Solid sheet padding: catches locale-invalid CSS even when all actions work.
        sample = subprocess.check_output(['convert', str(output / (name + '.png')), '-format', '%[pixel:p{490,240}]', 'info:'], text=True)
        return sample.strip()
    assert modal_surface('emoji') in ('srgb(252,254,255)', 'srgba(252,254,255,1)'), modal_surface('emoji')
    click(490, 240)  # Empty space inside the sheet must not dismiss it.
    capture('emoji-inside')
    assert modal_surface('emoji-inside') == modal_surface('emoji')
    click(40, 440)  # Dismiss via backdrop; this must not activate the chat behind it.
    capture('emoji-dismissed')
    assert modal_surface('emoji-dismissed') != modal_surface('emoji')
    click(1009, bottom)
    time.sleep(0.3)
    capture('emoji-reopened')
    assert modal_surface('emoji-reopened') == modal_surface('emoji')
    print('PASS opaque modal, inside hit testing, outside dismissal and reopen', flush=True)
    center = geometry()[2] // 2
    click(527, center - 119)
    time.sleep(0.3)
    press('Return')
    wait_for(lambda: any((b.get('body') == '😀' for _, b in action_writes)))
    print('PASS emoji composer send', flush=True)
    xt.XTestFakeMotionEvent(display, -1, 440, 250, 0)
    x.XFlush(display)
    time.sleep(0.4)
    click(429, 205)
    time.sleep(0.4)
    capture('reaction-picker')
    assert subprocess.check_output(['convert', str(output / 'reaction-picker.png'), '-format', '%[pixel:p{633,220}]', 'info:'], text=True).strip() not in ('srgb(0,0,0)', 'srgba(0,0,0,1)')
    click(561, 312)
    time.sleep(0.5)
    capture('reaction-expanded')
    assert subprocess.check_output(['convert', str(output / 'reaction-expanded.png'), '-format', '%[pixel:p{671,220}]', 'info:'], text=True).strip() not in ('srgb(0,0,0)', 'srgba(0,0,0,1)')
    # Search receives focus on expansion. Escape dismisses the native popover.
    type_text('rocket')
    capture('reaction-search')
    press('Escape')
    time.sleep(0.4)
    click(429, 205)
    time.sleep(0.4)
    capture('reaction-reopened')
    click(323, 312)
    wait_for(lambda: any((b.get('m.relates_to', {}).get('key') == '❤️' for _, b in action_writes)))
    print('PASS emoji reaction request', flush=True)
    time.sleep(0.5)
    shortcut('r')
    time.sleep(0.8)
    capture('reacted')
    xt.XTestFakeMotionEvent(display, -1, 440, 395, 0)
    x.XFlush(display)
    time.sleep(0.4)
    click(562, 355)
    capture('message-more')
    press('Escape')
    capture('menu-dismissed')
    xt.XTestFakeMotionEvent(display, -1, 440, 395, 0)
    x.XFlush(display)
    time.sleep(0.3)
    click(562, 355)
    for _ in range(5):
        press('Down')
    press('Return')
    capture('message-delete')
    click(583, 518)
    wait_for(lambda: any('/redact/$alice/' in p for p, _ in action_writes))
    print('PASS message action dialog and deletion confirmation', flush=True)
    time.sleep(0.5)
    reaction = next(e['event_id'] for e in message_events if e['content'].get('m.relates_to', {}).get('key') == '❤️')
    click(406, 328)
    wait_for(lambda: any('/redact/' + reaction + '/' in p for p, _ in action_writes))
    print('PASS existing reaction toggles off', flush=True)
    time.sleep(0.5)
    shortcut('f')
    type_text('Hello')
    press('Return')
    time.sleep(0.8)
    capture('search')
    press('Escape')
    shortcut('o', True)
    time.sleep(0.4)
    capture('organization')
    press('Escape')
    shortcut('s', True)
    time.sleep(0.6)
    capture('security')
    press('Escape')
    click(1380, 26)
    time.sleep(0.4)
    capture('account')
    press('Escape')
    assert app.poll() is None
    print('PASS search, organization, security and account dialog paths', flush=True)
finally:
    if app.poll() is None:
        app.terminate()
        app.wait(timeout=10)
    server.shutdown()
    server.server_close()
    x.XCloseDisplay.argtypes = [c.c_void_p]
    x.XCloseDisplay(display)
    log.close()
