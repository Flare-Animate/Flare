// Runs html5_runtime.js's per-pixel filter stages on a fake canvas and checks
// them against hand-computed values (argv[2] = path of the runtime).
const fs = require("fs");
const assert = require("assert");
let src = fs.readFileSync(process.argv[2], "utf8");
function mkCanvas(w, h) {
  const data = new Uint8ClampedArray(w * h * 4);
  const ctx = {
    getImageData: (x, y, ww, hh) => { const d = new Uint8ClampedArray(ww * hh * 4); for (let j = 0; j < hh; j++) for (let i = 0; i < ww; i++) for (let k = 0; k < 4; k++) d[(j * ww + i) * 4 + k] = data[((y + j) * w + x + i) * 4 + k]; return { data: d, width: ww, height: hh }; },
    putImageData: (img, x, y) => { for (let j = 0; j < img.height; j++) for (let i = 0; i < img.width; i++) for (let k = 0; k < 4; k++) data[((y + j) * w + x + i) * 4 + k] = img.data[(j * img.width + i) * 4 + k]; },
    setTransform() {}, clearRect() {}, drawImage() {}, save() {}, restore() {},
  };
  return { width: w, height: h, getContext: () => ctx, style: {}, data };
}
const cv = mkCanvas(40, 40);
global.window = { AC_MOVIE: { w: 40, h: 40, frames: [[]], images: [], paths: [], items: [], fx: [], clips: [], scenes: [{ start: 0 }], extra: [], sounds: [] }, devicePixelRatio: 1, addEventListener() {} };
global.document = { getElementById: () => cv, createElement: () => mkCanvas(40, 40), addEventListener() {} };
global.DOMMatrix = function () {}; global.Path2D = function () {}; global.Image = function () {}; global.requestAnimationFrame = () => {};
src = src.replace("  // per-frame: overrides by src", "  window.__t = { rampFilter: rampFilter, adjustColor: adjustColor, css: css };\n  // per-frame: overrides by src");
try { (0, eval)(src); } catch (e) { /* start-up needs more DOM than the stub; the helpers are defined before it */ }
const t = window.__t;
assert(t, "runtime helpers not reachable");
const px = (c, x, y) => Array.from(c.data.slice((y * 40 + x) * 4, (y * 40 + x) * 4 + 4));
const fill = (c, x0, x1, rgba) => { for (let y = x0; y < x1; y++) for (let x = x0; x < x1; x++) c.data.set(rgba, (y * 40 + x) * 4); };

// Adjust Color: brightness is additive (CSS brightness() would multiply)
let c = mkCanvas(40, 40); fill(c, 15, 25, [200, 100, 50, 255]);
t.adjustColor(c, [10, 0, 0, 0]);
assert.deepStrictEqual(px(c, 20, 20).map((v, i) => i < 3 ? Math.abs(v - [226, 126, 76][i]) <= 1 : v === 255), [true, true, true, true]);
assert.deepStrictEqual(px(c, 2, 2), [0, 0, 0, 0], "transparent pixels stay transparent");
// saturation -100 -> grey (luma .213r+.715g+.072b)
c = mkCanvas(40, 40); fill(c, 15, 25, [255, 0, 0, 255]);
t.adjustColor(c, [0, 0, -100, 0]);
let p = px(c, 20, 20); assert(p[0] === p[1] && p[1] === p[2] && Math.abs(p[0] - 54) <= 1, p);

// outer Glow: colour constant, alpha falls off outside the object, object untouched
c = mkCanvas(40, 40); fill(c, 15, 25, [0, 0, 255, 255]);
t.rampFilter(c, { k: "glow", bx: 4, by: 4, s: 1, q: 1, a: 0, d: 0, ko: false, h: false, nm: true, t: 1, r: [[0, 255, 0, 0, 0], [1, 255, 0, 0, 255]] }, 1);
assert.deepStrictEqual(px(c, 20, 20), [0, 0, 255, 255]);
p = px(c, 13, 20); assert(p[0] === 255 && p[3] > 20 && p[3] < 255, p);
assert.deepStrictEqual(px(c, 2, 2), [0, 0, 0, 0]);

// hide object: only the (shifted) shadow remains
c = mkCanvas(40, 40); fill(c, 15, 25, [0, 0, 255, 255]);
t.rampFilter(c, { k: "glow", bx: 4, by: 4, s: 1, q: 1, a: 0, d: 6, ko: false, h: true, nm: true, t: 1, r: [[0, 0, 0, 0, 0], [1, 0, 0, 0, 255]] }, 1);
assert.deepStrictEqual(px(c, 18, 20), [0, 0, 0, 0], "object hidden");
assert.strictEqual(px(c, 28, 20)[3], 255, "shadow shifted right by the distance");

// inner knockout: the object is replaced by its inner glow
c = mkCanvas(40, 40); fill(c, 10, 30, [0, 0, 255, 255]);
t.rampFilter(c, { k: "glow", bx: 4, by: 4, s: 1, q: 1, a: 0, d: 0, ko: true, h: false, nm: true, t: 0, r: [[0, 255, 0, 0, 0], [1, 255, 0, 0, 255]] }, 1);
assert.strictEqual(px(c, 20, 20)[3], 0, "centre empty"); assert(px(c, 10, 20)[3] > 100 && px(c, 10, 20)[2] === 0, px(c, 10, 20));
assert.strictEqual(px(c, 5, 5)[3], 0);

// solid bevel (inner): highlight top-left, shadow bottom-right, flat middle
c = mkCanvas(40, 40); fill(c, 10, 30, [128, 128, 128, 255]);
t.rampFilter(c, { k: "sbevel", bx: 4, by: 4, s: 1, q: 1, a: 45, d: 4, ko: false, t: 0, sh: [[0, 0, 0, 0, 0], [1, 0, 0, 0, 255]], hl: [[0, 255, 255, 255, 0], [1, 255, 255, 255, 255]] }, 1);
assert.deepStrictEqual(px(c, 10, 10), [255, 255, 255, 255]); assert.deepStrictEqual(px(c, 29, 29), [0, 0, 0, 255]); assert.deepStrictEqual(px(c, 20, 20), [128, 128, 128, 255]);

// ctx.filter lengths are canvas pixels: css() scales them with the pixel ratio (1 here)
assert.strictEqual(t.css("blur(2px) drop-shadow(1px -3.5px 2px red)"), "blur(2px) drop-shadow(1px -3.5px 2px red)");
console.log("runtime stages ok");
