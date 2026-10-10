import assert from "node:assert/strict";
import { parseHtml5, scriptSrcs } from "./html5-import.ts";
const js = `(function (cjs, an) {
var lib, images, createjs, ss, AdobeAn; lib = {};
(lib.hero = function() { this.initialize(img.hero); }).prototype = p = new cjs.Bitmap();
(lib.Walk = function(mode,start,loop,reversed) {
 this.timeline.addTween(cjs.Tween.get(this.a).to({x:5},3).wait(4));
 this.a = new lib.hero(); this.b = new lib.Inner();
}).prototype = p = new cjs.MovieClip();
(lib.Inner = function() { this.timeline.addTween(cjs.Tween.get({}).wait(1)); }).prototype = p;
(lib.main = function() { this.w = new lib.Walk(); this.timeline.addTween(cjs.Tween.get({}).wait(10)); }).prototype = p;
(lib.properties = {
 width: 640, height: 360, fps: 30, color: "#336699",
 manifest: [ {src:"images/hero.png?1", id:"hero"} ]
};
exportRoot = new lib.main();
})(createjs = createjs||{}, AdobeAn = AdobeAn||{});`;
const m = parseHtml5(js);
assert.equal(m.width, 640); assert.equal(m.fps, 30); assert.equal(m.bg, "#336699");
assert.deepEqual(m.assets, [{ src: "images/hero.png?1", id: "hero" }]);
const w = m.symbols.find(s => s.name === "Walk")!;
assert.equal(w.frames, 7); assert.deepEqual(w.children, ["hero", "Inner"]); assert.deepEqual(w.bitmaps, ["hero"]);
assert.equal(m.symbols.find(s => s.name === "main")!.frames, 10);
assert.equal(m.stage, "main");
assert.throws(() => parseHtml5("var x=1"));
assert.deepEqual(scriptSrcs('<script src="https://code.createjs.com/createjs.min.js"></script><script src="a.js"></script>'), ["a.js"]);
console.log("html5-import ok");
