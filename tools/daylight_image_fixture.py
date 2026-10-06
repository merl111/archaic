"""Deterministic, local-only image fixture for the native preview workflow."""
import math
import struct
import zlib

def png():
    width, height = 640, 360
    rows = bytearray()
    for y in range(height):
        rows.append(0)
        for x in range(width):
            color = (30 + y // 3, 65 + y // 4, 100 + y // 5)
            if (x - 455) ** 2 + (y - 100) ** 2 < 44 ** 2:
                color = (255, 213, 137)
            if y > 220 + 40 * math.sin(x / 110):
                color = (37, 91, 103)
            if y > 290 + 20 * math.sin(x / 70):
                color = (19, 52, 69)
            rows.extend(color)
    def chunk(kind, data):
        return struct.pack('!I', len(data)) + kind + data + struct.pack('!I', zlib.crc32(kind + data))
    return (b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('!2I5B', width, height, 8, 2, 0, 0, 0))
            + chunk(b'IDAT', zlib.compress(rows)) + chunk(b'IEND', b''))

IMAGE = png()

def setup(events):
    events[:] = [
        dict(type='m.room.message', sender='@bob:local', event_id='$caption', origin_server_ts=1790884800000,
             content={'msgtype': 'm.text', 'body': 'A native image preview. Click to open.',
                      'format':'org.matrix.custom.html',
                      'formatted_body':'A <strong>native image preview</strong>. Click to open.'}),
        dict(type='m.room.message', sender='@bob:local', event_id='$picture', origin_server_ts=1790884860000,
             content={'msgtype':'m.image', 'body':'Sunset.png', 'url':'mxc://local/landscape',
                      'info':{'w':640, 'h':360, 'mimetype':'image/png', 'size':len(IMAGE)}}),
    ]

def get(handler, path):
    if not path.endswith('/download/local/landscape'):
        return False
    assert handler.headers.get('Authorization') == 'Bearer local-test-token'
    handler.send_response(200)
    handler.send_header('Content-Type', 'image/png')
    handler.send_header('Content-Length', str(len(IMAGE)))
    handler.end_headers()
    handler.wfile.write(IMAGE)
    return True

def checks(g):
    requests, capture, click, wait_for = (g[k] for k in ('requests', 'capture', 'click', 'wait_for'))
    downloads = lambda: sum(p.endswith('/download/local/landscape') for _, p, _ in requests)
    wait_for(lambda: downloads() >= 1)
    g['time'].sleep(.8)
    capture('image-inline')
    before = downloads()
    click(500, 420)
    wait_for(lambda: downloads() > before)
    g['time'].sleep(.8)
    capture('image-viewer')
    g['press']('Escape')
    g['time'].sleep(.4)
    capture('image-dismissed')
    # Drag across differently styled spans, then paste the native clipboard into
    # the composer. This exercises real pointer events rather than canvas hooks.
    xt, x, display = (g[k] for k in ('xt', 'x', 'display'))
    xt.XTestFakeMotionEvent(display, -1, 388, 267, 0)
    xt.XTestFakeButtonEvent(display, 1, 1, 0)
    x.XFlush(display)
    g['time'].sleep(.1)
    for px in range(390, 700, 10):
        xt.XTestFakeMotionEvent(display, -1, px, 267, 0)
        x.XFlush(display)
        g['time'].sleep(.01)
    xt.XTestFakeButtonEvent(display, 1, 0, 0)
    x.XFlush(display)
    g['time'].sleep(.2)
    capture('text-selection')
    g['shortcut']('c')
    click(600, g['geometry']()[2] - 54)
    g['shortcut']('v')
    g['press']('Return')
    wait_for(lambda: any(b.get('body') == 'A native image preview. Click to open.' for _, b in g['action_writes']))
    print('PASS authenticated thumbnail, larger image, Escape, and native text selection/copy', flush=True)
