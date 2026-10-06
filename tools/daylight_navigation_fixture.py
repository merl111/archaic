"""Matrix fixtures and native checks for Daylight navigation and room metadata."""
import copy
import json
import struct
import time
import zlib

favorite = False
ROOM = '!studio:local'

def state_event(kind, key, content, suffix):
    return dict(type=kind, state_key=key, sender='@alice:local', event_id='$'+suffix,
                origin_server_ts=1790884900000, content=content)

def sync(sequence):
    members = [state_event('m.room.member', user, {'membership':'join','displayname':user[1:].split(':')[0].title(), 'avatar_url':'mxc://local/user'}, user) for user in ['@alice:local','@bob:local','@carol:local']]
    rooms = {}
    for i, (rid, name) in enumerate([(ROOM,'Native integration test'), ('!direct:local','Bob'), ('!space:local','AxLabs')] + [(f'!room{i}:local',f'Room {i:02}') for i in range(40)]):
        content = {'creator':'@alice:local','room_version':'10'}
        if rid == '!space:local': content['type']='m.space'
        state = [state_event('m.room.create','',content,rid+'create'),state_event('m.room.name','',{'name':name},rid+'name')] + copy.deepcopy(members[:2] if rid == "!direct:local" else members)
        if rid in (ROOM, '!space:local'): state.append(state_event('m.room.avatar','',{'url':'mxc://local/room'},rid+'avatar'))
        if rid == '!space:local':
            state += [state_event('m.space.child',f'!room{j}:local',{'via':['local']},f'child{j}') for j in range(3)]
        tags = {'m.favourite': {'order':0.5}} if favorite and rid == ROOM else {}
        # Increasing timestamps make Room 39 the newest ordinary conversation.
        timeline = [] if rid in [ROOM,'!space:local'] else [dict(type='m.room.message',sender='@bob:local',event_id=f'$recent{i}',origin_server_ts=1790884860000+i*1000,content={'msgtype':'m.text','body':name})]
        rooms[rid] = {'state':{'events':state},'timeline':{'events':timeline,'limited':False,'prev_batch':'history'},'account_data':{'events':[{'type':'m.tag','content':{'tags':tags}}]},'summary': {} if rid == ROOM else {'m.joined_member_count':2 if rid == '!direct:local' else 3},'unread_notifications':{}}
    return {'next_batch':f'batch-{sequence}', 'rooms':{'join':rooms}, 'account_data':{'events':[{'type':'m.direct','content':{'@bob:local':['!direct:local']}}]},'device_one_time_keys_count':{'signed_curve25519':50}}

def png(red):
    def chunk(kind, data): return struct.pack('!I',len(data))+kind+data+struct.pack('!I',zlib.crc32(kind+data))
    color = bytes([220,45,80] if red else [30,180,120])
    return b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('!2I5B',8,8,8,2,0,0,0))+chunk(b'IDAT',zlib.compress((b'\0'+color*8)*8))+chunk(b'IEND',b'')

def get(handler, path):
    if '/thumbnail/' in path:
        assert handler.headers.get('Authorization') == 'Bearer local-test-token'
        data = png(path.endswith('/room'))
        handler.send_response(200);handler.send_header('Content-Type','image/png');handler.send_header('Content-Length',str(len(data)));handler.end_headers();handler.wfile.write(data)
        return True
    if path.endswith('/members'):
        rid = path.split('/rooms/')[1].split('/')[0]
        events = sync(0)['rooms']['join'].get(rid, sync(0)['rooms']['join'][ROOM])['state']['events']
        handler.reply({'chunk':[dict(e,room_id=rid) for e in events if e['type']=='m.room.member']})
        return True
    return False

def put(handler, path, body):
    global favorite
    if '/tags/m.favourite' in path:
        favorite = True; handler.reply({}); return True
    return False

def delete(handler, path):
    global favorite
    if '/tags/m.favourite' in path:
        favorite = False
        handler.reply({})
        return True
    return False

def checks(ui):
    click, capture, press, shortcut, type_text = [ui[k] for k in ('click','capture','press','shortcut','type_text')]
    x, xt, display = [ui[k] for k in ('x','xt','display')]
    def move(xp,yp): xt.XTestFakeMotionEvent(display,-1,xp,yp,0);x.XFlush(display);time.sleep(.3)
    def button(n): xt.XTestFakeButtonEvent(display,n,1,0);xt.XTestFakeButtonEvent(display,n,0,0);x.XFlush(display);time.sleep(.2)
    def search(value): click(130,134);shortcut('a');press('BackSpace');type_text(value);time.sleep(.5)
    def pixel(name,xp,yp):
        return ui['subprocess'].check_output(['convert',str(ui['output']/(name+'.png')),'-format',f'%[pixel:p{{{xp},{yp}}}]','info:'],text=True).strip()
    search('Native integration');click(120,234)
    ui['wait_for'](lambda: any(p.endswith('/members') for _,p,_ in ui['requests']))
    time.sleep(1);capture('navigation-avatars')
    assert pixel('navigation-avatars',1230,165) in ('srgb(220,45,80)','srgba(220,45,80,1)'), 'room avatar did not render'
    # Context menu is triggered from the body, not the floating toolbar.
    move(480,268);button(3);capture('navigation-context');press('Return')
    capture('navigation-context-react');press('Escape')
    move(480,268);button(3);press('Down');press('Return')
    capture('navigation-context-reply')
    click(550,ui['geometry']()[2]-54);type_text('Context reply');press('Return')
    ui['wait_for'](lambda: any(b.get('m.relates_to',{}).get('m.in_reply_to',{}).get('event_id')=='$bob' for _,b in ui['action_writes']))
    print('PASS authenticated avatars, member lookup and right-click Reply',flush=True)
    # Make favourites observable through a real Matrix tag request.
    click(1258,28)
    ui['wait_for'](lambda:any('/tags/m.favourite' in p for _,p,_ in ui['requests']))
    search('');capture('navigation-list-top')
    old = len(ui['requests']);click(140,418)
    ui['wait_for'](lambda:any('/rooms/!room39:local/messages' in p for _,p,_ in ui['requests'][old:]))
    move(140,650)
    for _ in range(12):button(5)
    capture('navigation-list-scrolled')
    assert pixel('navigation-list-top',52,232) != pixel('navigation-list-scrolled',52,232), 'room list did not scroll'
    old = len(ui['requests']);click(140,600)
    ui['wait_for'](lambda:any('/rooms/!room' in p and p.endswith('/messages') for _,p,_ in ui['requests'][old:]))
    print('PASS room-list wheel scrolling and selecting a scrolled room, Matrix favorites',flush=True)
    click(335,28);capture('navigation-direct')
    old = len(ui['requests']);click(140,234)
    ui['wait_for'](lambda:any('/rooms/!direct:local/messages' in p for _,p,_ in ui['requests'][old:]))
    capture('navigation-dm-avatar')
    assert pixel('navigation-dm-avatar',45,230) in ('srgb(30,180,120)','srgba(30,180,120,1)'), 'direct room must use peer avatar without room art'
    click(180,28);capture('navigation-home')
    click(430,28);capture('navigation-space')
    old = len(ui['requests']);click(140,234)
    ui['wait_for'](lambda:any('/rooms/!room2:local/messages' in p for _,p,_ in ui['requests'][old:]))
    click(180,28)
    click(45,88);capture('navigation-avatar-rail');click(45,80)
    move(364,460);xt.XTestFakeButtonEvent(display,1,1,0);move(280,460);xt.XTestFakeButtonEvent(display,1,0,0);x.XFlush(display);time.sleep(.5)
    capture('navigation-resized')
    click(968,90);capture('navigation-info-hidden')
    assert pixel('navigation-info-hidden',1200,160) != pixel('navigation-resized',1200,160), 'inspector did not hide'
    click(1386,90)
    click(1375,28);capture('navigation-settings')
    click(500,310);capture('navigation-settings-appearance')
    press('Escape')
    search('Native integration');click(140,234);click(1258,28)
    ui['wait_for'](lambda:any(method == 'DELETE' and '/tags/m.favourite' in path for method,path,_ in ui['requests']))
    print('PASS favorite removal', flush=True)
    print('PASS Home/Direct/space navigation, sidebar controls, settings entry (screenshots captured)',flush=True)
