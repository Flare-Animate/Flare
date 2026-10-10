"""Smoke test: export scene, run runtime in node with DOM stubs (no jsdom needed)."""
import os, subprocess, sys, tempfile
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..', 'tools', 'flash'))
import game_export as ge

assert 'let x' in ge.as3_to_js('var x:int = 1;') and 'private' not in ge.as3_to_js('private var a:Number')
assert 'function f(a, b) {' in ge.as3_to_js('function f(a:int, b:String):void {')
assert 'for (const q of arr)' in ge.as3_to_js('for each (var q:Foo in arr)')

scene = {"width": 100, "height": 100, "fps": 24, "sounds": {"ding": "d.mp3"},
 "scenes": [
  {"name": "a", "sprites": [
   {"name": "hero", "x": 0, "y": 0, "w": 10, "h": 10,
    "onEnterFrame": "if (Key.isDown(Key.RIGHT)) this.x += 5;",
    "onHit": "if (other.name=='coin'){ other.visible=false; Sound.play('ding'); gotoScene('b'); }"},
   {"name": "coin", "x": 20, "y": 0, "w": 10, "h": 10},
   {"name": "clip", "x": 50, "y": 50, "w": 5, "h": 5, "loop": False,
    "frames": [{"color": "#f00"}, {"color": "#0f0", "script": "trace('f2'); this.stop();"}, {"color": "#00f"}],
    "onClick": "this.gotoAndStop(3);"}]},
  {"name": "b", "sprites": [{"name": "end", "x": 0, "y": 0, "w": 1, "h": 1}]}]}
js = ge.export(scene).split('<script>')[1].rsplit('</script>', 1)[0]
harness = r"""
const ctx=new Proxy({},{get:()=>()=>{},set:()=>true});
const listeners={};
global.document={getElementById:()=>({getContext:()=>ctx,addEventListener:(t,f)=>listeners[t]=f})};
global.window=global;global.Image=function(){};
global.Audio=function(){this.play=()=>{global.__played=1}};
global.setInterval=()=>0;
%s
const as=(c,m)=>{if(!c)throw new Error(m)};
const FL=__flare;as(FL.root().hero,'hero');
onkeydown({keyCode:39});__flareTick();__flareTick();
onkeyup({keyCode:39});as(FL.root().hero.x==10,'move '+FL.root().hero.x);
const c=FL.root().clip;as(c.totalFrames==3,'frames');
""" % js + r"""
const c2=FL.root().clip;c2.gotoAndStop(1);c2.play();__flareTick();
as(c2.currentFrame==2&&!c2.playing,'frame script stop '+c2.currentFrame);
FL.root().hero.x=18;__flareTick();
as(FL.root().end,'scene switch');as(global.__played,'sound');
console.log('OK');
"""
with tempfile.NamedTemporaryFile('w', suffix='.js', delete=False) as t:
    t.write(harness)
r = subprocess.run(['node', t.name], capture_output=True, text=True)
os.unlink(t.name)
print(r.stdout.strip(), r.stderr.strip()[:600])
sys.exit(0 if 'OK' in r.stdout else 1)
