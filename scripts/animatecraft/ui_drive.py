import sys, time

from Xlib import X, XK, display
from Xlib.ext import xtest
import subprocess
d = display.Display(':99')
w = d.screen().root.query_pointer().child
def mv(x,y): xtest.fake_input(d, X.MotionNotify, x=x, y=y); d.sync(); time.sleep(0.06)
def click(x,y,b=1):
    mv(x,y); xtest.fake_input(d, X.ButtonPress, b); d.sync(); time.sleep(0.06); xtest.fake_input(d, X.ButtonRelease, b); d.sync(); time.sleep(0.3)
def key(name):
    kc = d.keysym_to_keycode(XK.string_to_keysym(name))
    xtest.fake_input(d, X.KeyPress, kc); d.sync(); time.sleep(0.05); xtest.fake_input(d, X.KeyRelease, kc); d.sync(); time.sleep(0.3)
def drag(x1,y1,x2,y2,steps=14):
    mv(x1,y1); xtest.fake_input(d, X.ButtonPress, 1); d.sync(); time.sleep(0.08)
    for i in range(1,steps+1):
        mv(int(x1+(x2-x1)*i/steps), int(y1+(y2-y1)*i/steps))
    time.sleep(0.1); xtest.fake_input(d, X.ButtonRelease, 1); d.sync(); time.sleep(0.35)
def combo(spec):
    mods = {'ctrl': XK.XK_Control_L, 'shift': XK.XK_Shift_L, 'alt': XK.XK_Alt_L}
    parts = spec.split('+'); keys=[mods[m] for m in parts[:-1]]
    for k in keys: xtest.fake_input(d, X.KeyPress, d.keysym_to_keycode(k))
    key(parts[-1])
    for k in reversed(keys): xtest.fake_input(d, X.KeyRelease, d.keysym_to_keycode(k))
    d.sync(); time.sleep(0.2)
def dclick(x,y):
    mv(x,y)
    for _ in range(2):
        xtest.fake_input(d, X.ButtonPress, 1); d.sync(); time.sleep(0.04); xtest.fake_input(d, X.ButtonRelease, 1); d.sync(); time.sleep(0.06)
    time.sleep(0.3)
def shot(n): subprocess.run(['import','-window','root',f'out/{n}.png'], env={'DISPLAY':':99'})
mv(700,300)
w = d.screen().root.query_pointer().child
d.set_input_focus(w, X.RevertToParent, X.CurrentTime); d.sync()
for a in sys.argv[1:]:
    op, *args = a.split(':')
    if op=='click': click(*map(int,args))
    elif op=='rclick': click(*map(int,args), b=3)
    elif op=='key': key(args[0])
    elif op=='type':
        names = {'/':'slash','.':'period','-':'minus','_':'underscore',' ':'space',':':'colon','\\':'backslash'}
        for ch in ':'.join(args):
            shift = ch.isupper() or ch in '_:'
            kc = d.keysym_to_keycode(XK.string_to_keysym(names.get(ch, ch)))
            if shift: xtest.fake_input(d, X.KeyPress, d.keysym_to_keycode(XK.XK_Shift_L))
            xtest.fake_input(d, X.KeyPress, kc); xtest.fake_input(d, X.KeyRelease, kc)
            if shift: xtest.fake_input(d, X.KeyRelease, d.keysym_to_keycode(XK.XK_Shift_L))
            d.sync(); time.sleep(0.02)
        time.sleep(0.3)
    elif op=='shot': shot(args[0])
    elif op=='drag': drag(*map(int,args))
    elif op=='path':
        v=list(map(int,args)); pts=list(zip(v[0::2], v[1::2]))
        mv(*pts[0]); xtest.fake_input(d, X.ButtonPress, 1); d.sync(); time.sleep(0.08)
        for (a,b),(c,e) in zip(pts, pts[1:]):
            for i in range(1,9): mv(int(a+(c-a)*i/8), int(b+(e-b)*i/8))
        time.sleep(0.1); xtest.fake_input(d, X.ButtonRelease, 1); d.sync(); time.sleep(0.35)
    elif op=='combo': combo(args[0])
    elif op=='dclick': dclick(*map(int,args))
    elif op=='shiftclick':
        xtest.fake_input(d, X.KeyPress, d.keysym_to_keycode(XK.XK_Shift_L)); d.sync()
        click(*map(int,args))
        xtest.fake_input(d, X.KeyRelease, d.keysym_to_keycode(XK.XK_Shift_L)); d.sync()
    elif op in ('hold', 'release'):
        k = {'ctrl': XK.XK_Control_L, 'shift': XK.XK_Shift_L, 'alt': XK.XK_Alt_L}[args[0]]
        xtest.fake_input(d, X.KeyPress if op == 'hold' else X.KeyRelease, d.keysym_to_keycode(k)); d.sync(); time.sleep(0.15)
    elif op=='sleep': time.sleep(float(args[0]))
    elif op=='mv': mv(*map(int,args)); time.sleep(0.2)
    elif op=='down': xtest.fake_input(d, X.ButtonPress, 1); d.sync(); time.sleep(0.3)
    elif op=='up': xtest.fake_input(d, X.ButtonRelease, 1); d.sync(); time.sleep(0.3)
    elif op=='scroll':
        x, y, n = map(int, args); mv(x, y)
        b = 5 if n > 0 else 4
        for _ in range(abs(n)):
            xtest.fake_input(d, X.ButtonPress, b); d.sync(); time.sleep(0.03); xtest.fake_input(d, X.ButtonRelease, b); d.sync(); time.sleep(0.03)
        time.sleep(0.3)
    else: raise SystemExit(f'unknown op {op}')
