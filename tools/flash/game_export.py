#!/usr/bin/env python3
"""Flash-style game export: scene JSON -> standalone HTML5 game.

Patterns (entity/update loop, AABB hitTest, key state) follow Citrus-Engine
(MIT), Adobe-Flash--Simple-Game-Engine and swf2js (MIT) timeline playback;
scripting is an AS3-like subset translated to JS by `as3_to_js`.

Scene JSON (all keys but width/height/sprites-or-scenes optional):
{"width":550,"height":400,"fps":24,"bg":"#fff","sounds":{"ding":"d.mp3"},
 "scenes":[{"name":"a","sprites":[ ... ]}],   # or top-level "sprites"
 "sprites":[{"name":"hero","x":10,"y":10,"w":32,"h":32,"color":"#f00",
   "image":"optional.png","frames":[{"color":"#f00","image":"x.png","script":"stop();"}],
   "loop":true,"stop":false,
   "onEnterFrame":"if (Key.isDown(Key.RIGHT)) this.x += 4;",
   "onKey":"if (code==Key.SPACE) Sound.play('ding');",
   "onClick":"this.nextFrame();",
   "onHit":"if (other.name=='coin') other.visible=false;"}]}
Runtime API: this.play/stop/gotoAndPlay/gotoAndStop/nextFrame/prevFrame,
currentFrame, totalFrames, hitTestObject, hitTestPoint; Key, Mouse, Sound.play,
gotoScene(name), root.<name>, other.
"""
import json, re, sys

_AS3 = [(re.compile(r'\bfor\s+each\s*\(\s*var\s+(\w+)(?:\s*:\s*[\w.*]+)?\s+in\s+'), r'for (const \1 of '),
        (re.compile(r'\bvar\s+(\w+)\s*:\s*[\w.<>*]+'), r'let \1'),
        (re.compile(r'\b(?:public|private|protected|internal|static|override|final)\s+'), ''),
        (re.compile(r'(\w+)\s*:\s*[\w.*]+(?=\s*[,)])'), r'\1'),
        (re.compile(r'\)\s*:\s*[\w.*<>]+\s*\{'), ') {'),
        (re.compile(r'\s+as\s+[A-Z]\w*'), ''),
        (re.compile(r'\b(?:int|uint)\(([^()]*)\)'), r'Math.trunc(\1)'),
        (re.compile(r'\bVector\.<[\w.]+>'), 'Array'),
        (re.compile(r'\btrace\('), 'console.log(')]


def as3_to_js(src):
    src = src or ''
    for rx, rep in _AS3:
        src = rx.sub(rep, src)
    return src


RUNTIME = r"""
const S=__SCENE__,cv=document.getElementById('c'),g=cv.getContext('2d');
cv.width=S.width;cv.height=S.height;
const Key={LEFT:37,UP:38,RIGHT:39,DOWN:40,SPACE:32,ENTER:13,_d:{},isDown(k){return!!this._d[k]}};
const Mouse={x:0,y:0,down:false};
const Sound={_a:{},play(n){const u=(S.sounds||{})[n];if(!u||typeof Audio=='undefined')return;
 const a=this._a[n]||(this._a[n]=new Audio(u));try{a.currentTime=0;const p=a.play();p&&p.catch&&p.catch(()=>{})}catch(e){}}};
const scenes=S.scenes||[{name:'main',sprites:S.sprites||[]}];
let L=[],root={};
function hitTest(a,b){return a.visible&&b.visible&&a.x<b.x+b.w&&b.x<a.x+a.w&&a.y<b.y+b.h&&b.y<a.y+a.h}
const F=b=>b?new Function('Key','Mouse','Sound','root','gotoScene','other',b):null;
const call=(s,f,o)=>f&&f.call(s,Key,Mouse,Sound,root,gotoScene,o||null);
function setup(s){s.visible=s.visible!==false;s.frames=s.frames||[{}];s.currentFrame=1;
 s.playing=s.frames.length>1&&!s.stop;s.totalFrames=s.frames.length;
 s.hitTestObject=o=>hitTest(s,o);
 s.hitTestPoint=(x,y)=>s.visible&&x>=s.x&&x<s.x+s.w&&y>=s.y&&y<s.y+s.h;
 s.play=()=>{s.playing=true};s.stop=()=>{s.playing=false};
 const go=n=>{s._lf=0;s.currentFrame=Math.max(1,Math.min(s.totalFrames,n))};
 s.gotoAndStop=n=>{go(n);s.playing=false};s.gotoAndPlay=n=>{go(n);s.playing=true};
 s.nextFrame=()=>go(s.currentFrame+1);s.prevFrame=()=>go(s.currentFrame-1);
 root[s.name]=s;s._e=F(s.onEnterFrame);s._h=F(s.onHit);s._c=F(s.onClick);
 s.frames.forEach(f=>{f._s=F(f.script);if(f.image){f._img=new Image();f._img.src=f.image}});
 const k=F(s.onKey&&'const code=other;'+s.onKey);s._k=k&&(c=>call(s,k,c));
 if(s.image){s._img=new Image();s._img.src=s.image}}
function gotoScene(n){const sc=scenes.find(x=>x.name==n);if(!sc)return;root={};L=sc.sprites;L.forEach(setup)}
onkeydown=e=>{Key._d[e.keyCode]=1;L.slice().forEach(s=>s._k&&s._k(e.keyCode))};
onkeyup=e=>{Key._d[e.keyCode]=0};
if(cv.addEventListener){
 cv.addEventListener('mousemove',e=>{Mouse.x=e.offsetX;Mouse.y=e.offsetY});
 cv.addEventListener('mousedown',e=>{Mouse.down=true;Mouse.x=e.offsetX;Mouse.y=e.offsetY;
  L.slice().forEach(s=>s._c&&s.hitTestPoint(Mouse.x,Mouse.y)&&call(s,s._c))});
 cv.addEventListener('mouseup',()=>{Mouse.down=false})}
function tick(){
 L.slice().forEach(s=>{call(s,s._e);
  if(s.playing){const n=s.currentFrame+1;
   if(n>s.totalFrames){if(s.loop!==false)s.currentFrame=1;else s.playing=false}else s.currentFrame=n}
  const f=s.frames[s.currentFrame-1];if(f!==s._lf){s._lf=f;call(s,f._s)}});
 for(const a of L)for(const b of L)if(a!==b&&a._h&&hitTest(a,b))call(a,a._h,b);
 g.fillStyle=S.bg||'#fff';g.fillRect(0,0,cv.width,cv.height);
 L.forEach(s=>{if(!s.visible)return;const f=s.frames[s.currentFrame-1],im=f._img||s._img;
  if(im&&im.complete&&im.width)g.drawImage(im,s.x,s.y,s.w,s.h);
  else{g.fillStyle=f.color||s.color||'#000';g.fillRect(s.x,s.y,s.w,s.h)}});}
gotoScene(scenes[0].name);
window.__flareTick=tick;window.__flare={get sprites(){return L},root:()=>root,gotoScene};
setInterval(tick,1000/(S.fps||24));
"""


def export(scene):
    groups = [scene.get('sprites', [])] + [c.get('sprites', []) for c in scene.get('scenes', [])]
    for sp in groups:
        for s in sp:
            for k in ('onEnterFrame', 'onHit', 'onKey', 'onClick'):
                if k in s:
                    s[k] = as3_to_js(s[k])
            for f in s.get('frames', []):
                if 'script' in f:
                    f['script'] = as3_to_js(f['script'])
    js = RUNTIME.replace('__SCENE__', json.dumps(scene).replace('</', '<\\/'))
    return ('<!doctype html><meta charset="utf-8"><title>Flare Game</title>'
            '<body style="margin:0;background:#222"><canvas id="c"></canvas>'
            '<script>%s</script>' % js)


if __name__ == '__main__':
    if len(sys.argv) != 3:
        sys.exit('usage: game_export.py scene.json out.html')
    with open(sys.argv[1]) as f:
        html = export(json.load(f))
    with open(sys.argv[2], 'w') as f:
        f.write(html)
