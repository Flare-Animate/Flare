#!/usr/bin/env python3
"""Rough Flash-style game export: scene JSON -> standalone HTML5 game.

Patterns (entity/update loop, AABB hitTest, key state) follow Citrus-Engine
(MIT) and Adobe-Flash--Simple-Game-Engine; scripting is a tiny AS3-like subset
(`onEnterFrame`, `onKey`, `onHit` bodies, JS-compatible expressions).

Scene JSON:
{"width":550,"height":400,"fps":24,"bg":"#fff",
 "sprites":[{"name":"hero","x":10,"y":10,"w":32,"h":32,"color":"#f00",
   "image":"optional.png","solid":true,
   "onEnterFrame":"if (Key.isDown(Key.RIGHT)) this.x += 4;",
   "onHit":"if (other.name=='coin') other.visible=false;"}]}
"""
import json, re, sys

_AS3 = [(re.compile(r'\bvar\s+(\w+)\s*:\s*\w+'), r'let \1'),
        (re.compile(r'\)\s*:\s*\w+\s*\{'), ') {'),
        (re.compile(r'\btrace\('), 'console.log(')]

def as3_to_js(src):
    for rx, rep in _AS3:
        src = rx.sub(rep, src or '')
    return src

RUNTIME = r"""
const S=__SCENE__,cv=document.getElementById('c'),g=cv.getContext('2d');
cv.width=S.width;cv.height=S.height;
const Key={LEFT:37,UP:38,RIGHT:39,DOWN:40,SPACE:32,_d:{},isDown(k){return!!this._d[k]}};
onkeydown=e=>{Key._d[e.keyCode]=1;S.sprites.forEach(s=>s._k&&s._k.call(s,e.keyCode))};
onkeyup=e=>{Key._d[e.keyCode]=0};
const root={};
function hitTest(a,b){return a.visible&&b.visible&&a.x<b.x+b.w&&b.x<a.x+a.w&&a.y<b.y+b.h&&b.y<a.y+a.h}
S.sprites.forEach(s=>{s.visible=s.visible!==false;s.hitTestObject=o=>hitTest(s,o);root[s.name]=s;
 const f=b=>b?new Function('Key','root','other',b):null;
 s._e=f(s.onEnterFrame);s._h=f(s.onHit);const k=f(s.onKey&&'const code=other;'+s.onKey);
 s._k=k&&(c=>k.call(s,Key,root,c));
 if(s.image){s._img=new Image();s._img.src=s.image}});
function tick(){const L=S.sprites;
 L.forEach(s=>s._e&&s._e.call(s,Key,root,null));
 for(const a of L)for(const b of L)if(a!==b&&a._h&&hitTest(a,b))a._h.call(a,Key,root,b);
 g.fillStyle=S.bg||'#fff';g.fillRect(0,0,cv.width,cv.height);
 L.forEach(s=>{if(!s.visible)return;if(s._img&&s._img.complete)g.drawImage(s._img,s.x,s.y,s.w,s.h);
  else{g.fillStyle=s.color||'#000';g.fillRect(s.x,s.y,s.w,s.h)}});}
window.__flareTick=tick;setInterval(tick,1000/(S.fps||24));
"""

def export(scene):
    for s in scene.get('sprites', []):
        for k in ('onEnterFrame', 'onHit', 'onKey'):
            if k in s:
                s[k] = as3_to_js(s[k])
    js = RUNTIME.replace('__SCENE__', json.dumps(scene))
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
