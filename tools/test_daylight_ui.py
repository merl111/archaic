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
import daylight_navigation_fixture as navigation
import daylight_image_fixture as images
requests = []
room = '!studio:local'
message_events = [{'type': 'm.room.message', 'sender': '@bob:local', 'event_id': '$bob', 'origin_server_ts': 1790884800000, 'content': {'msgtype': 'm.text', 'body': 'Hello from Bob'}}, {'type': 'm.room.message', 'sender': '@alice:local', 'event_id': '$alice', 'origin_server_ts': 1790884860000, 'content': {'msgtype': 'm.text', 'body': 'My original message'}}]
action_writes = []
history_cursors = []
receipts_moved = False
delivery_fail = True
delivery_read = False
thread_lookup_blocked = False

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
        if args.images and images.get(self, path):
            return
        if path.endswith('/profile/@alice:local/displayname'):
            return self.reply({'displayname':'Alice Example'})
        if path.endswith('/profile/@alice:local/avatar_url'):
            return self.reply({'avatar_url':'mxc://local/user'})
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
        if (args.navigation or args.polish or args.settings or args.history_cache or args.receipts) and navigation.get(self, path):
            return
        if path.endswith('/versions'):
            return self.reply({'versions': ['v1.11']})
        if path.endswith('/sync'):
            time.sleep(0.15)
            if args.navigation or args.settings or args.history_cache:
                return self.reply(navigation.sync(len(requests)))
            state = [{'type': 'm.room.create', 'state_key': '', 'sender': '@alice:local', 'event_id': '$create', 'origin_server_ts': 1, 'content': {'creator': '@alice:local', 'room_version': '10'}}, {'type': 'm.room.member', 'state_key': '@alice:local', 'sender': '@alice:local', 'event_id': '$member', 'origin_server_ts': 2, 'content': {'membership': 'join', 'displayname': 'Alice'}}, {'type': 'm.room.name', 'state_key': '', 'sender': '@alice:local', 'event_id': '$name', 'origin_server_ts': 3, 'content': {'name': 'Native integration test'}}]
            if args.polish:
                state.append(navigation.state_event('m.room.member','@bob:local', {'membership':'join','displayname':'Bob','avatar_url':'mxc://local/user'},'bob-member'))
            rooms = {room: {'state': {'events': state}, 'timeline': {'events': list(message_events) if (args.threads or args.slow_thread or args.delivery) else [], 'limited': False, 'prev_batch': 'history'}, 'unread_notifications': {}, 'summary': {'m.joined_member_count': 2}}}
            if args.delivery and delivery_read:
                rooms[room]['ephemeral']={'events':[{'type':'m.receipt','content':{message_events[-1]['event_id']:{'m.read':{'@bob:local':{'ts':200,'thread_id':'main'}}}}}]}
            if args.receipts:
                for user in ['bob','carol','dana','erin']:
                    state.append(navigation.state_event('m.room.member',f'@{user}:local',{'membership':'join','displayname':user.title(),'avatar_url':'mxc://local/user'},user+'-member'))
                content={('$alice' if receipts_moved else '$bob'):{'m.read':{f'@{u}:local':{'ts':200 if receipts_moved else 100,'thread_id':'main'} for u in ['bob','carol','dana','erin']}}}
                rooms[room]['ephemeral']={'events':[{'type':'m.receipt','content':content}]}
            return self.reply({'next_batch': 'batch-' + str(len(requests)), 'rooms': {'join': rooms}, 'account_data': {'events': [{'type': 'm.direct', 'content': {}}]}, 'device_one_time_keys_count': {'signed_curve25519': 50}})
        if path.endswith('/account_data/m.direct'):
            return self.reply({})
        if args.replies and '/context/' in path:
            original=dict(type='m.room.message',sender='@bob:local',event_id='$outside',origin_server_ts=1790884700000,content={'msgtype':'m.text','body':'Original outside loaded history'})
            return self.reply({'event':dict(original,room_id=room),'events_before':[],'events_after':[dict(e,room_id=room) for e in message_events[:2]],'state':[]})
        if args.slow_thread and thread_lookup_blocked and ('/event/' in path or '/relations/' in path):
            time.sleep(20)
        if '/event/' in path:
            event = next((e for e in message_events if e['event_id'] == path.rsplit('/', 1)[1]), None)
            if event:
                return self.reply(dict(event, room_id=room))
        if path.endswith('/messages') and args.composer:
            from urllib.parse import parse_qs
            cursor = parse_qs(urlparse(self.path).query).get('from', [''])[0]
            history_cursors.append(cursor)
            if cursor == 'older-one':
                return self.reply({'start': cursor, 'chunk': [{'type':'m.room.message','sender':'@bob:local','event_id':'$older','origin_server_ts':1790884700000,'content':{'msgtype':'m.text','body':'Older message loaded by scrolling'}}], 'state':[]})
            return self.reply({'start':'history','end':'older-one','chunk':list(reversed(message_events)),'state':[]})
        if path.endswith('/messages'):
            if args.history_cache and sum(p == path for _,p,_ in requests) > 1:
                time.sleep(4) # Returning to a visited room must not wait for this response.
            return self.reply({'start': 'history', 'chunk': list(reversed(message_events)), 'state': []})
        return self.reply({'errcode': 'M_NOT_FOUND', 'error': 'Not found'}, 404)

    def do_DELETE(self):
        path = unquote(urlparse(self.path).path)
        requests.append(('DELETE', path, None))
        if args.navigation and navigation.delete(self, path):
            return
        self.reply({'errcode': 'M_NOT_FOUND', 'error': 'Not found'}, 404)

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
        if args.navigation and navigation.put(self, path, body):
            return
        if '/account_data/' in path:
            return self.reply({})
        if args.slow_thread and body.get('m.relates_to',{}).get('rel_type')=='m.thread':
            time.sleep(6)
        if args.delivery and '/send/' in path:
            time.sleep(3)
            if delivery_fail and body.get('body') == 'Failed delivery test':
                return self.reply({'errcode':'M_FORBIDDEN','error':'Synthetic failure'},403)
        eid = '$sent' + str(len(action_writes))
        if '/redact/' in path:
            event = {'type': 'm.room.redaction', 'sender': '@alice:local', 'event_id': eid, 'origin_server_ts': 1790884920000 + len(action_writes), 'redacts': path.split('/redact/')[1].split('/')[0], 'content': {}}
        else:
            kind = path.split('/send/')[1].split('/')[0]
            event = {'type': kind, 'sender': '@alice:local', 'event_id': eid, 'origin_server_ts': 1790884920000 + len(action_writes), 'content': body}
        message_events.append(event)
        return self.reply({'event_id': eid})
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--images', action='store_true')
parser.add_argument('--navigation', action='store_true')
parser.add_argument('--polish', action='store_true')
parser.add_argument('--composer', action='store_true')
parser.add_argument('--settings', action='store_true')
parser.add_argument('--threads', action='store_true')
parser.add_argument('--slow-thread', action='store_true')
parser.add_argument('--receipts', action='store_true')
parser.add_argument('--replies', action='store_true')
parser.add_argument('--delivery', action='store_true')
parser.add_argument('--history-cache', action='store_true')
parser.add_argument('--theme', choices=['light','dark'], default='light')
parser.add_argument('--binary', type=Path, default=Path(__file__).resolve().parents[1] / 'target/debug/archaic')
parser.add_argument('--output', type=Path, default=Path(__file__).resolve().parents[1] / 'target/daylight-ui')
args = parser.parse_args()
if args.images: images.setup(message_events)
output = args.output.resolve()
output.mkdir(parents=True, exist_ok=True)
if args.history_cache:
    message_events[:] = [dict(type='m.room.message',sender='@bob:local',event_id=f'$history{i}',origin_server_ts=1790884800000+i*60000,content={'msgtype':'m.text','body':f'Cached message {i:02} stays in this conversation'}) for i in range(50)]
if args.replies:
    message_events.extend(dict(type='m.room.message',sender='@bob:local',event_id=f'$filler{i}',origin_server_ts=1790884860000+i*60000,content={'msgtype':'m.text','body':f'Intermediate message {i:02}'}) for i in range(24))
    for target in ['$bob', '$outside']:
        message_events.append(dict(type='m.room.message',sender='@alice:local',event_id='$reply'+target[1:],origin_server_ts=1790887000000,content={'msgtype':'m.text','body':'> <@bob:local> Original quote\n\nReply body only '+target,'m.relates_to':{'m.in_reply_to':{'event_id':target}}}))
if args.polish:
    message_events.append(dict(type='m.room.message',sender='@bob:local',event_id='$link',origin_server_ts=1790884900000,content={'msgtype':'m.text','body':'https://example.org/guide'}))
    launcher = output / 'bin'
    launcher.mkdir(exist_ok=True)
    browser = launcher / 'xdg-open'
    browser.write_text('#!/bin/sh\nprintf "%s\\n" "$1" > "$ARCHAIC_TEST_LINK"\n')
    browser.chmod(0o700)
if args.composer:
    message_events[0]['content']['body'] = 'Hello @alice:local'
    for i,user in enumerate(['@alice:local','@bob:local']):
        message_events.append({'type':'m.reaction','sender':user,'event_id':'$reaction'+str(i),'origin_server_ts':1790884800100+i,'content':{'m.relates_to':{'rel_type':'m.annotation','event_id':'$bob','key':'👍'}}})
if not os.environ.get('DISPLAY'):
    parser.error('Run under xvfb-run with a 1600x1050x24 screen (see module docstring).')
env = dict(os.environ, XDG_CONFIG_HOME=str(output/'config'), XDG_DATA_HOME=str(output/'data'), XDG_CACHE_HOME=str(output/'cache'), GDK_BACKEND='x11', GDK_SCALE='1', GDK_DPI_SCALE='1', GSK_RENDERER='cairo', GTK_USE_PORTAL='0', DBUS_SESSION_BUS_ADDRESS='unix:path=/tmp/archaic-ui-test-no-bus')
if args.polish:
    env['PATH'] = str(output/'bin') + os.pathsep + env.get('PATH','')
    env['ARCHAIC_TEST_LINK'] = str(output/'opened-link.txt')
server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
threading.Thread(target=server.serve_forever, daemon=True).start()
log = (output / 'runtime.log').open('w')
app = subprocess.Popen([str(args.binary.resolve()), '--temporary-session', '--theme', args.theme], env=env, stderr=log)
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
    window, _, _ = geometry()
    properties = subprocess.check_output(['xprop', '-id', str(window), 'WM_CLASS', '_NET_WM_ICON'], text=True)
    assert 'org.archaic.desktop' in properties, 'Wrong Linux application identity'
    assert '_NET_WM_ICON(CARDINAL)' in properties, 'Missing native window icon'
    capture('login')
    type_text(f'http://127.0.0.1:{server.server_port}')
    press('Tab')
    type_text('alice')
    press('Tab')
    type_text('local-test-password')
    press('Return')
    wait_for(lambda: any((p.endswith('/sync') for _, p, _ in requests)))
    time.sleep(0.5)
    if args.history_cache:
        def choose(value):
            click(130,134); shortcut('a'); press('BackSpace'); type_text(value)
            click(120,232)
        choose('Native integration')
        wait_for(lambda:any('/rooms/!studio:local/messages' in p for _,p,_ in requests))
        time.sleep(.7)
        xt.XTestFakeMotionEvent(display,-1,650,350,0)
        for _ in range(6):
            xt.XTestFakeButtonEvent(display,4,1,0); xt.XTestFakeButtonEvent(display,4,0,0)
        x.XFlush(display);time.sleep(.4)
        xt.XTestFakeMotionEvent(display,-1,120,232,0);x.XFlush(display)
        capture('cached-before-switch')
        choose('Room 39')
        wait_for(lambda:any('/rooms/!room39:local/messages' in p for _,p,_ in requests))
        time.sleep(.7)
        choose('Native integration')
        capture('cached-return-during-network-delay')
        for name in ['cached-before-switch','cached-return-during-network-delay']:
            subprocess.check_call(['convert',str(output/(name+'.png')),'-crop','590x480+425+150',str(output/(name+'-crop.png'))])
        compared=subprocess.run(['compare','-metric','AE',str(output/'cached-before-switch-crop.png'),str(output/'cached-return-during-network-delay-crop.png'),'null:'],capture_output=True,text=True)
        assert compared.returncode == 0, 'Cached messages or scroll position changed: '+compared.stderr
        print('PASS room history and scroll position restored before delayed network response',flush=True)
        raise SystemExit(0)
    if args.settings:
        click(130,134); type_text('Native integration'); time.sleep(.5); click(140,234)
        wait_for(lambda:any(p.endswith('/messages') for _,p,_ in requests));time.sleep(1)
        capture('names-space-avatar')
        click(45,90);time.sleep(.5);capture('avatar-rail')
        click(45,90);time.sleep(.5);capture('list-expanded')
        click(1375,28);time.sleep(.5);capture('settings-account')
        click(420,433);time.sleep(.8);capture('settings-security')
        click(740,410);time.sleep(.3);capture('security-dropdown');press('Home');press('Down');press('Return');time.sleep(.4);capture('settings-recovery')
        press('space');press('Home');press('Down');press('Down');press('Return');time.sleep(.4);capture('settings-keys')
        print('SETTINGS screenshots captured',flush=True)
        raise SystemExit(0)
    if args.navigation:
        navigation.checks(globals())
        raise SystemExit(0)
    click(120, 232)
    wait_for(lambda: any((p.endswith('/messages') for _, p, _ in requests)))
    time.sleep(0.5)
    capture('chat')
    if args.images:
        images.checks(globals())
        raise SystemExit(0)
    if args.delivery:
        click(600,geometry()[2]-54);type_text('Delayed delivery test');press('Return')
        wait_for(lambda:any(b.get('body')=='Delayed delivery test' for _,b in action_writes))
        time.sleep(.4);capture('delivery-sending')
        time.sleep(3.2);capture('delivery-delivered')
        delivery_read=True;time.sleep(.8);capture('delivery-read')
        delivery_read=False
        click(600,geometry()[2]-54);type_text('Failed delivery test');press('Return')
        wait_for(lambda:any(b.get('body')=='Failed delivery test' for _,b in action_writes))
        time.sleep(3.5);capture('delivery-failed')
        delivery_fail=False
        click(1016,592);time.sleep(.4);capture('delivery-retry-menu')
        press('Home');press('Return')
        wait_for(lambda:sum(b.get('body')=='Failed delivery test' for _,b in action_writes)>=2)
        time.sleep(3.5);capture('delivery-retried')
        attempts=[p for p,b in action_writes if b.get('body')=='Failed delivery test']
        assert len(set(attempts))==1, 'Retry must keep the original SDK transaction'
        message_events.append({'type':'m.reaction','sender':'@bob:local','event_id':'$existing-reaction','origin_server_ts':1790884999999,'content':{'m.relates_to':{'rel_type':'m.annotation','event_id':'$bob','key':'👍'}}})
        time.sleep(.8);capture('delivery-reaction-before')
        click(495,300)
        wait_for(lambda:any(b.get('m.relates_to',{}).get('key')=='👍' for _,b in action_writes))
        time.sleep(.3);capture('delivery-reaction-sending')
        time.sleep(3.2);capture('delivery-reaction-delivered')
        print('PASS inline sending, failure, delivered, read, retry and reaction without an outbox bar',flush=True)
        raise SystemExit(0)
    if args.replies:
        time.sleep(.8);capture('replies-before')
        click(550,646);time.sleep(.6);capture('reply-loaded-original')
        assert not any('/context/' in p for _,p,_ in requests), 'Loaded original should not be fetched again'
        xt.XTestFakeMotionEvent(display,-1,700,600,0)
        for _ in range(35):
            xt.XTestFakeButtonEvent(display,5,1,0);xt.XTestFakeButtonEvent(display,5,0,0)
        x.XFlush(display);time.sleep(.6)
        click(550,720)
        wait_for(lambda:any('/context/$outside' in p for _,p,_ in requests))
        time.sleep(.7);capture('reply-older-original')
        print('PASS reply quote clicks reuse loaded history and fetch missing original context',flush=True)
        raise SystemExit(0)
    if args.receipts:
        time.sleep(1.5)
        capture('readers-initial')
        xt.XTestFakeMotionEvent(display,-1,950,300,0);x.XFlush(display);time.sleep(1)
        capture('reader-tooltip')
        click(950,300);time.sleep(.7);capture('reader-profile')
        click(1200,85)
        xt.XTestFakeMotionEvent(display,-1,150,150,0);x.XFlush(display)
        receipts_moved=True
        time.sleep(1.5)
        capture('readers-moved')
        click(1380,26);capture('receipt-settings');click(410,336);capture('receipt-appearance');click(800,491);press('Home');press('Down');press('Return');capture('receipt-compact-setting');press('Escape')
        time.sleep(.4);capture('readers-compact')
        print('RECEIPTS screenshots captured',flush=True)
        raise SystemExit(0)
    if args.threads or args.slow_thread:
        xt.XTestFakeMotionEvent(display, -1, 520, 267, 0)
        xt.XTestFakeButtonEvent(display, 3, 1, 0)
        xt.XTestFakeButtonEvent(display, 3, 0, 0)
        x.XFlush(display)
        time.sleep(.4)
        capture('thread-menu')
        press('Down'); press('Down'); press('Return')
        time.sleep(.5)
        capture('thread-open')
        click(1210, geometry()[2]-54)
        type_text('Native thread reply')
        thread_lookup_blocked=True
        before_lookup=sum('/event/' in p or '/relations/' in p for _,p,_ in requests)
        press('Return')
        wait_for(lambda: any(b.get('m.relates_to', {}).get('rel_type') == 'm.thread' for _, b in action_writes))
        if args.slow_thread:
            time.sleep(.3);capture('thread-send-pending')
            click(1210,geometry()[2]-54);type_text('Next reply can be typed')
            capture('thread-send-editable')
            click(1394,88);time.sleep(.3);capture('thread-send-closed')
            click(600,geometry()[2]-54);type_text('Main chat stays usable')
            capture('thread-send-main-usable')
            assert sum('/event/' in p or '/relations/' in p for _,p,_ in requests)==before_lookup, 'Sending must reuse loaded thread metadata'
            click(1380,26);time.sleep(.3);capture('thread-send-settings');press('Escape')
            time.sleep(6);capture('thread-send-complete')
            print('PASS slow thread send: no refetch, editable composer, closable pane and usable main chat/settings',flush=True)
            raise SystemExit(0)
        time.sleep(3)
        capture('thread-reply')
        click(1394, 88)
        time.sleep(.4)
        capture('main-after-thread')
        print('PASS native thread reply relation; captured room and thread projections', flush=True)
        raise SystemExit(0)
    if args.composer:
        xt.XTestFakeMotionEvent(display,-1,454,311,0); x.XFlush(display)
        time.sleep(1.2); capture('reaction-tooltip')
        click(900, geometry()[2]-54)
        capture('composer-menu')
        press('Down'); press('Down'); press('Return')
        time.sleep(.3)
        capture('poll-form')
        click(540,337); type_text('Lunch')
        click(490,435); type_text('Pizza'); press('Return'); type_text('Sushi')
        click(720,648)
        wait_for(lambda:any('/org.matrix.msc3381.poll.start/' in p for p,_ in action_writes))
        time.sleep(.5)
        capture('poll-sent')
        click(540,geometry()[2]-54); type_text('@bob:local @room'); press('Return')
        wait_for(lambda:any(b.get('m.mentions',{}).get('room') and '@bob:local' in b.get('m.mentions',{}).get('user_ids',[]) for _,b in action_writes))
        time.sleep(.5)
        xt.XTestFakeMotionEvent(display,-1,650,250,0)
        xt.XTestFakeButtonEvent(display,4,1,0); xt.XTestFakeButtonEvent(display,4,0,0); x.XFlush(display)
        wait_for(lambda:'older-one' in history_cursors)
        capture('older-loaded')
        click(900, geometry()[2]-54)
        press('Down'); press('Return')
        time.sleep(.3); capture('voice-form')
        click(50,440) # Outside dismisses; never open a real microphone in tests.
        time.sleep(.3); capture('voice-dismissed')
        click(410,geometry()[2]-54)
        time.sleep(.5)
        press('Escape'); time.sleep(.5); capture('attachment-cancelled')
        variant=subprocess.check_output(['xprop','-id',str(geometry()[0]),'_GTK_THEME_VARIANT'],env=env,text=True)
        assert args.theme in variant,variant
        print('PASS native poll send, mention metadata, scroll pagination, voice form, file-picker cancel and native title theme',flush=True)
        raise SystemExit(0)
    if args.polish:
        click(460,267)
        capture('body-click')
        click(510,416)
        time.sleep(.3)
        capture('clicked-link')
        assert (output/'opened-link.txt').read_text().strip() == 'https://example.org/guide'

        click(405,275)
        time.sleep(0.8)
        capture('profile')
        click(1200,85)
        xt.XTestFakeMotionEvent(display, -1, 480,267,0)
        xt.XTestFakeButtonEvent(display,3,1,0)
        xt.XTestFakeButtonEvent(display,3,0,0)
        x.XFlush(display)
        time.sleep(.4)
        capture('pointer-menu')
        press('Escape')
        click(1380,26)
        time.sleep(0.4)
        capture('settings')
        click(410,336)
        capture('settings-appearance')
        click(800,491)
        press('End'); press('Return')
        capture('settings-compact')
        press('Escape')
        capture('compact-chat')
        click(1380,26)
        click(800,491)
        press('Home'); press('Return')
        click(800,407)
        if args.theme == 'dark':
            press('Home'); press('Down')
        else:
            press('End')
        press('Return')
        capture('theme-switched-settings')
        press('Escape')
        capture('theme-switched-chat')
        click(854, geometry()[2]-54)
        capture('expanded-emoji')
        click(914,654)
        capture('emoji-page-two')
        click(630,251); type_text('salute')
        capture('emoji-search-extra')
        # Filtered grid collapses vertically: use keyboard focus order from search.
        press('Tab'); press('Return')
        press('Return')
        wait_for(lambda:any(b.get('body')=='🫡' for _,b in action_writes))
        print('PASS profile, pointer menu, compact messages, theme switching and extended emoji search',flush=True)
        raise SystemExit(0)
    bottom = geometry()[2] - 54
    click(550, bottom)
    type_text('Ordinary draft survives')
    xt.XTestFakeMotionEvent(display, -1, 440, 250, 0)
    x.XFlush(display)
    time.sleep(0.3)
    click(505, 194)
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
    click(535, 194)
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
    click(854, bottom)
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
    def backdrop_pixel(name):
        return subprocess.check_output(['convert', str(output / (name + '.png')), '-format', '%[pixel:p{40,440}]', 'info:'], text=True).strip()
    assert backdrop_pixel('emoji-dismissed') != backdrop_pixel('emoji')
    click(854, bottom)
    time.sleep(0.3)
    capture('emoji-reopened')
    assert modal_surface('emoji-reopened') == modal_surface('emoji')
    print('PASS opaque modal, inside hit testing, outside dismissal and reopen', flush=True)
    center = geometry()[2] // 2
    click(527, center - 143)
    time.sleep(0.3)
    press('Return')
    wait_for(lambda: any((b.get('body') == '😀' for _, b in action_writes)))
    print('PASS emoji composer send', flush=True)
    xt.XTestFakeMotionEvent(display, -1, 440, 250, 0)
    x.XFlush(display)
    time.sleep(0.4)
    click(475, 194)
    time.sleep(0.4)
    capture('reaction-picker')
    assert subprocess.check_output(['convert', str(output / 'reaction-picker.png'), '-format', '%[pixel:p{633,220}]', 'info:'], text=True).strip() not in ('srgb(0,0,0)', 'srgba(0,0,0,1)')
    click(603, 300)
    time.sleep(0.5)
    capture('reaction-expanded')
    assert subprocess.check_output(['convert', str(output / 'reaction-expanded.png'), '-format', '%[pixel:p{671,220}]', 'info:'], text=True).strip() not in ('srgb(0,0,0)', 'srgba(0,0,0,1)')
    # Search receives focus on expansion. Escape dismisses the native popover.
    type_text('rocket')
    capture('reaction-search')
    press('Escape')
    time.sleep(0.4)
    click(475, 194)
    time.sleep(0.4)
    capture('reaction-reopened')
    click(368, 300)
    wait_for(lambda: any((b.get('m.relates_to', {}).get('key') == '❤️' for _, b in action_writes)))
    print('PASS emoji reaction request', flush=True)
    time.sleep(0.5)
    shortcut('r')
    time.sleep(0.8)
    capture('reacted')
    xt.XTestFakeMotionEvent(display, -1, 440, 417, 0)
    x.XFlush(display)
    time.sleep(0.4)
    click(607, 344)
    capture('message-more')
    press('Escape')
    capture('menu-dismissed')
    xt.XTestFakeMotionEvent(display, -1, 440, 417, 0)
    x.XFlush(display)
    time.sleep(0.3)
    click(607, 344)
    for _ in range(8):
        press('Down')
    press('Return')
    capture('message-delete')
    click(583, 518)
    wait_for(lambda: any('/redact/$alice/' in p for p, _ in action_writes))
    print('PASS message action dialog and deletion confirmation', flush=True)
    time.sleep(0.5)
    reaction = next(e['event_id'] for e in message_events if e['content'].get('m.relates_to', {}).get('key') == '❤️')
    click(450, 350)
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
