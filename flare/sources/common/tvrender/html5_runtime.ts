type Mat = number[];
type Fn = (...a: any[]) => any;
interface TextBox {
  s: string; k: number; f: string; c: string; ls: number; w: number; ml: number; in: number;
  al: string; lh: number; sz: number; pw?: boolean; mc?: number; bd?: boolean; bw: number; bh: number; url?: string;
}
interface NodeB { src: number; m: Mat; x: number; y: number; r: number; sx: number; sy: number; a: number; tx?: TextBox }
interface ButtonB { src: number; name: string; hit: [number, Mat][]; down: DList; over: DList }
interface Extra {
  n?: { [name: string]: NodeB }; b?: ButtonB[]; v?: DList[]; snd?: SoundEv[]; js?: string[];
}
interface SoundEv { s: number; y?: string; l?: boolean; r: number; e: number; fx?: string }
interface Gradient { m: Mat; x?: number; k: number; fx?: number; s: [number, string][] }
interface Item {
  src?: number; m: Mat; p: number; c?: number[]; a?: number; ol?: string; img?: number; pat?: number; pm?: Mat;
  g?: Gradient; f?: string; s?: string; cj?: string; w?: number; d?: number[]; hl?: boolean; sm?: string;
}
interface MotionBlur { n: number; f: number; m: Mat }
interface MeshWarp { s: number[]; d: number[]; c: number; r: number }
interface Group { c: DList; x: number; sp?: number; mb?: MotionBlur; cb?: number; mw?: MeshWarp }
type DList = (number | Group)[];
type Ramp = number[][];
interface Glow {
  k: string; bx: number; by: number; d: number; q: number; a: number; s: number; t: number;
  ko?: boolean; h?: boolean; nm?: boolean; r: Ramp; sh: Ramp; hl: Ramp;
}
interface Stage { c?: string; g?: Glow; a?: number[] }
interface FxEntry { s?: Stage[]; f?: string; o: GlobalCompositeOperation }
interface Scene { start: number; count: number }
interface Movie {
  w: number; h: number; fps: number; bg: string; loop: boolean;
  frames: DList[]; paths: string[]; images: [string, boolean][]; clips: [number, Mat][][];
  extra: Extra[]; scenes: Scene[]; labels: [string, number, number][]; items: Item[]; fx: FxEntry[];
  sounds: [string, string][];
}
interface Inst {
  name: string; _l: { [type: string]: Fn[] };
  x: number; y: number; rotation: number; scaleX: number; scaleY: number; alpha: number; visible: boolean;
  text?: string | null;
  addEventListener: (type: string, fn: Fn) => Fn; on: (type: string, fn: Fn) => Fn;
  removeEventListener: (type: string, fn: Fn) => void; off: (type: string, fn: Fn) => void;
  removeAllEventListeners: () => void;
}
interface Voice { s: number; src: AudioBufferSourceNode; alive: boolean; stream: boolean; from: number; end: number }
interface Override { hide?: boolean; hideText?: boolean; delta?: DOMMatrix; a?: number }
interface Timeline {
  stop: () => void; play: () => void; gotoAndPlay: (f: number | string) => void; gotoAndStop: (f: number | string) => void;
  nextFrame: () => void; prevFrame: () => void; getChildByName: (n: string) => Inst | Timeline | null;
  currentFrame: number; totalFrames: number; [name: string]: unknown;
}
interface Window {
  AC_MOVIE: Movie;
  webkitAudioContext?: typeof AudioContext;
  Flare: object;
  AnimationCraft: object;
}
/* Flare HTML5 Canvas runtime (MIT OR Apache-2.0).
 * Plays a movie published by Flare (AC_MOVIE): precomputed display
 * lists per frame, frame scripts with a CreateJS-style API, buttons and
 * Event/Start/Stop/Stream sounds.
 *
 * SOURCE OF TRUTH: html5_runtime.ts. This .js is its compiled output
 * (include_str! in ac_html5.rs reads it); regenerate with
 * scripts/build-html5-runtime.sh, never edit it by hand. */
(function () {
  "use strict";
  var M: Movie = window.AC_MOVIE;
  var cv = document.getElementById("ac-stage") as HTMLCanvasElement;
  var ctx = cv.getContext("2d") as CanvasRenderingContext2D;
  var dpr = Math.max(1, Math.min(3, window.devicePixelRatio || 1));
  cv.width = Math.round(M.w * dpr);
  cv.height = Math.round(M.h * dpr);
  cv.style.width = M.w + "px";
  cv.style.height = M.h + "px";
  var BASE = new DOMMatrix([dpr, 0, 0, dpr, 0, 0]);
  var MB: DOMMatrix | null = null; // motion-blur sample transform (stage space), while drawing a sample
  var N = M.frames.length;
  var dirty = true;

  var paths = M.paths.map(function (s) { return new Path2D(s); });
  var images = M.images.map(function (d) {
    var im = new Image();
    im.onload = function () { dirty = true; };
    im.src = d[0];
    return { img: im, smooth: d[1] };
  });
  var clipCache: Path2D[] = [];
  function clipPath(i: number): Path2D {
    if (!clipCache[i]) {
      var p = new Path2D();
      M.clips[i].forEach(function (s) { p.addPath(paths[s[0]], new DOMMatrix(s[1])); });
      clipCache[i] = p;
    }
    return clipCache[i];
  }

  // ---------------------------------------------------------------- matrices
  function decompose(m: Mat) {
    var a = m[0], b = m[1], c = m[2], d = m[3];
    var sx = Math.hypot(a, b), sy = Math.hypot(c, d);
    var det = a * d - b * c;
    var ky = Math.atan2(b, a), kx = Math.atan2(-c, d);
    if (det < 0) { sy = -sy; kx = Math.atan2(c, -d); }
    return { tx: m[4], ty: m[5], sx: sx, sy: sy, kx: kx, ky: ky };
  }
  function compose(o: { tx: number; ty: number; sx: number; sy: number; kx: number; ky: number }): Mat {
    return [o.sx * Math.cos(o.ky), o.sx * Math.sin(o.ky), -o.sy * Math.sin(o.kx), o.sy * Math.cos(o.kx), o.tx, o.ty];
  }

  // ---------------------------------------------------------------- state
  var frame = 0, playing = true;
  var clock = 0; // movie clips' clock: keeps running while a script holds the timeline
  var insts: { [name: string]: Inst } = {};       // name → script object (persists while playing)
  var changed: { [name: string]: boolean } = {};     // name → true once a script changed it
  var hover: number | null = null, press: number | null = null; // button src
  var scriptCache: { [code: string]: Fn } = {};

  function extra(f: number): Extra { return M.extra[f] || {}; }
  function sceneOf(f: number): Scene {
    for (var i = M.scenes.length - 1; i >= 0; i--) if (f >= M.scenes[i].start) return M.scenes[i];
    return M.scenes[0];
  }

  function instObj(name: string): Inst {
    var o = insts[name];
    if (!o) {
      o = insts[name] = { name: name, _l: {} } as Inst;
      o.addEventListener = o.on = function (type: string, fn: Fn) { (o._l[type] = o._l[type] || []).push(fn); return fn; };
      o.removeEventListener = o.off = function (type: string, fn: Fn) { o._l[type] = (o._l[type] || []).filter(function (g) { return g !== fn; }); };
      o.removeAllEventListeners = function () { o._l = {}; };
    }
    return o;
  }
  // refresh script objects from the frame (unless a script changed them)
  function bindInstances(f: number) {
    var n = extra(f).n || {};
    Object.keys(n).forEach(function (name) {
      var o = instObj(name), b = n[name];
      if (!changed[name]) { o.x = b.x; o.y = b.y; o.rotation = b.r; o.scaleX = b.sx; o.scaleY = b.sy; o.alpha = b.a; o.visible = true; }
      if (b.tx && !textSet[name]) o.text = b.tx.s;
      tl[name] = o;
    });
  }
  function snapshot(o: Inst): string { return [o.x, o.y, o.rotation, o.scaleX, o.scaleY, o.alpha, o.visible].join(","); }
  // text fields: a script (or the viewer) changed the text
  var textSet: { [name: string]: boolean } = {};

  // ---------------------------------------------------------------- drawing
  var pool: HTMLCanvasElement[] = [];
  var warpPool: HTMLCanvasElement[] = [];
  function offscreen(depth: number): HTMLCanvasElement {
    var c = pool[depth];
    if (!c) { c = pool[depth] = document.createElement("canvas"); }
    if (c.width !== cv.width || c.height !== cv.height) { c.width = cv.width; c.height = cv.height; }
    var x = c.getContext("2d")!;
    x.setTransform(1, 0, 0, 1, 0, 0);
    x.clearRect(0, 0, c.width, c.height);
    return c;
  }

  // ---- filter stages: CSS filters and per-pixel gradient glow / bevel
  var spare: HTMLCanvasElement[] = [], mbPool: HTMLCanvasElement[] = [];
  function spareCanvas(i: number): HTMLCanvasElement {
    var c = spare[i] || (spare[i] = document.createElement("canvas"));
    if (c.width !== cv.width || c.height !== cv.height) { c.width = cv.width; c.height = cv.height; }
    var x = c.getContext("2d", { willReadFrequently: true })!;
    x.setTransform(1, 0, 0, 1, 0, 0);
    x.clearRect(0, 0, c.width, c.height);
    return c;
  }
  // ctx.filter lengths are canvas pixels (the transform doesn't scale them),
  // while the movie's numbers are stage pixels: scale them by the pixel ratio.
  function css(f: string): string {
    return dpr === 1 ? f : f.replace(/(-?\d*\.?\d+)px/g, function (m: string, v: string) { return (+v * dpr) + "px"; });
  }
  function stages(src: HTMLCanvasElement, list: Stage[], depth: number): HTMLCanvasElement {
    var cur: HTMLCanvasElement = src, flip = 0;
    for (var i = 0; i < list.length; i++) {
      var st = list[i];
      if (st.c) {
        var t = spareCanvas(depth * 2 + (flip ^= 1));
        var x = t.getContext("2d")!;
        x.filter = css(st.c);
        x.drawImage(cur, 0, 0);
        x.filter = "none";
        cur = t;
      } else if (st.g) {
        if (cur === src) { var t2 = spareCanvas(depth * 2 + (flip ^= 1)); t2.getContext("2d")!.drawImage(cur, 0, 0); cur = t2; }
        rampFilter(cur, st.g, dpr);
      } else if (st.a) {
        if (cur === src) { var t3 = spareCanvas(depth * 2 + (flip ^= 1)); t3.getContext("2d")!.drawImage(cur, 0, 0); cur = t3; }
        adjustColor(cur, st.a);
      }
    }
    return cur;
  }
  function blurPlane(p: Float32Array, w: number, h: number, rx: number, ry: number, passes: number) {
    var tmp = new Float32Array(p.length), x: number, y: number, k: number, r: number, acc: number, n: number;
    for (var pass = 0; pass < Math.max(1, Math.min(3, passes)); pass++) {
      r = Math.round(rx / 2);
      if (r > 0) {
        n = 2 * r + 1;
        for (y = 0; y < h; y++) {
          var o = y * w; acc = 0;
          for (k = 0; k <= Math.min(r, w - 1); k++) acc += p[o + k];
          for (x = 0; x < w; x++) {
            tmp[o + x] = acc / n;
            if (x + r + 1 < w) acc += p[o + x + r + 1];
            if (x >= r) acc -= p[o + x - r];
          }
        }
        p.set(tmp);
      }
      r = Math.round(ry / 2);
      if (r > 0) {
        n = 2 * r + 1;
        for (x = 0; x < w; x++) {
          acc = 0;
          for (k = 0; k <= Math.min(r, h - 1); k++) acc += p[k * w + x];
          for (y = 0; y < h; y++) {
            tmp[y * w + x] = acc / n;
            if (y + r + 1 < h) acc += p[(y + r + 1) * w + x];
            if (y >= r) acc -= p[(y - r) * w + x];
          }
        }
        p.set(tmp);
      }
    }
  }
  function shadowPlane(alpha: Float32Array, w: number, h: number, bx: number, by: number, q: number, angle: number, dist: number, st: number, inner: boolean): Float32Array {
    var n = alpha.length, src = new Float32Array(n), out = new Float32Array(n), i: number;
    for (i = 0; i < n; i++) src[i] = inner ? 1 - alpha[i] : alpha[i];
    blurPlane(src, w, h, bx, by, q);
    var dx = Math.round(Math.cos(angle * Math.PI / 180) * dist), dy = Math.round(Math.sin(angle * Math.PI / 180) * dist);
    for (var y = 0; y < h; y++) {
      var sy = y - dy;
      if (sy < 0 || sy >= h) continue;
      for (var x = 0; x < w; x++) {
        var sx = x - dx;
        if (sx >= 0 && sx < w) out[y * w + x] = src[sy * w + sx];
      }
    }
    for (i = 0; i < n; i++) {
      var v = Math.max(0, Math.min(1, out[i] * st));
      out[i] = inner ? v * alpha[i] : v;
    }
    return out;
  }
  function rampAt(r: Ramp, t: number): number[] {
    if (t <= r[0][0]) return r[0];
    for (var i = 1; i < r.length; i++) {
      if (t <= r[i][0]) {
        var a = r[i - 1], b = r[i], u = (t - a[0]) / Math.max(1e-6, b[0] - a[0]);
        return [t, a[1] + (b[1] - a[1]) * u, a[2] + (b[2] - a[2]) * u, a[3] + (b[3] - a[3]) * u, a[4] + (b[4] - a[4]) * u];
      }
    }
    return r[r.length - 1];
  }
  // modes: 0 under the object, 1 over it, 2 the effect only (knockout)
  function combineRamp(px: Float32Array, values: Float32Array, mask: Float32Array, r: Ramp, mode: number) {
    for (var i = 0, j = 0; i < values.length; i++, j += 4) {
      var c = rampAt(r, Math.max(0, Math.min(1, values[i])));
      var a = c[4] / 255 * Math.max(0, Math.min(1, mask[i]));
      var f0 = c[1] / 255 * a, f1 = c[2] / 255 * a, f2 = c[3] / 255 * a, oa = px[j + 3];
      if (mode === 0) { px[j] += f0 * (1 - oa); px[j + 1] += f1 * (1 - oa); px[j + 2] += f2 * (1 - oa); px[j + 3] = oa + a * (1 - oa); }
      else if (mode === 1) { px[j] = f0 + px[j] * (1 - a); px[j + 1] = f1 + px[j + 1] * (1 - a); px[j + 2] = f2 + px[j + 2] * (1 - a); }
      else { px[j] = f0; px[j + 1] = f1; px[j + 2] = f2; px[j + 3] = a; }
    }
  }
  // getImageData throws SecurityError on a tainted canvas (file:// pages with
  // multi-file output): skip the pixel stage, warn once, keep animating.
  var warnedTaint = false;
  function readPixels(x: CanvasRenderingContext2D, a: number, b: number, w: number, h: number): ImageData | null {
    try { return x.getImageData(a, b, w, h); }
    catch (e) {
      if (!warnedTaint) { warnedTaint = true; console.warn("Flare: canvas is tainted, skipping pixel filters (publish single-file or serve over http): " + e); }
      return null;
    }
  }
  function rampFilter(canvas: HTMLCanvasElement, g: Glow, scale: number) {
    var x = canvas.getContext("2d", { willReadFrequently: true }) as CanvasRenderingContext2D;
    var W = canvas.width, H = canvas.height;
    var all = readPixels(x, 0, 0, W, H); if (!all) return;
    var d = all.data;
    // work on the object's bounds plus the reach of the effect
    var x0 = W, y0 = H, x1 = -1, y1 = -1, xx: number, yy: number;
    for (yy = 0; yy < H; yy++) for (xx = 0; xx < W; xx++) if (d[(yy * W + xx) * 4 + 3]) { if (xx < x0) x0 = xx; if (xx > x1) x1 = xx; if (yy < y0) y0 = yy; if (yy > y1) y1 = yy; }
    if (x1 < 0) return;
    var m = Math.ceil((Math.max(g.bx, g.by) * 1.5 + Math.abs(g.d) + 2) * scale);
    x0 = Math.max(0, x0 - m); y0 = Math.max(0, y0 - m); x1 = Math.min(W - 1, x1 + m); y1 = Math.min(H - 1, y1 + m);
    var w = x1 - x0 + 1, h = y1 - y0 + 1, n = w * h;
    var img = readPixels(x, x0, y0, w, h); if (!img) return;
    var p = img.data;
    var px = new Float32Array(n * 4), alpha = new Float32Array(n), i: number;
    for (i = 0; i < n; i++) {
      var a = p[i * 4 + 3] / 255;
      alpha[i] = a;
      px[i * 4] = p[i * 4] / 255 * a; px[i * 4 + 1] = p[i * 4 + 1] / 255 * a; px[i * 4 + 2] = p[i * 4 + 2] / 255 * a; px[i * 4 + 3] = a;
    }
    var bx = g.bx * scale, by = g.by * scale, dist = g.d * scale, k: number;
    if (g.ko) px.fill(0);
    var one: Float32Array | null = null;
    if (g.k === "glow") {
      // g.h: hide the object (outer shadow only); g.nm: solid colour, so the
      // inner effect isn't masked a second time (shadowPlane already is)
      if (g.t !== 0) {
        var o = shadowPlane(alpha, w, h, bx, by, g.q, g.a, dist, g.s, false), mk = new Float32Array(n);
        for (k = 0; k < n; k++) mk[k] = g.h ? 1 : (o[k] > 0 ? 1 : 0) * (g.ko ? 1 - alpha[k] : 1);
        combineRamp(px, o, mk, g.r, g.ko || g.h ? 2 : 0);
      }
      if (g.t !== 1) {
        if (g.nm) { one = new Float32Array(n); one.fill(1); }
        combineRamp(px, shadowPlane(alpha, w, h, bx, by, g.q, g.a, dist, g.s, true), g.nm ? one! : alpha, g.r, g.ko ? 2 : 1);
      }
    } else if (g.k === "sbevel") {
      // solid Bevel: the highlight and shadow planes are painted in turn
      one = new Float32Array(n); one.fill(1);
      if (g.t !== 1) {
        combineRamp(px, shadowPlane(alpha, w, h, bx, by, g.q, g.a + 180, dist, g.s, true), one, g.sh, 1);
        combineRamp(px, shadowPlane(alpha, w, h, bx, by, g.q, g.a, dist, g.s, true), one, g.hl, 1);
      }
      if (g.t !== 0) {
        combineRamp(px, shadowPlane(alpha, w, h, bx, by, g.q, g.a + 180, dist, g.s, false), one, g.hl, 0);
        combineRamp(px, shadowPlane(alpha, w, h, bx, by, g.q, g.a, dist, g.s, false), one, g.sh, 0);
      }
    } else {
      var side = function (inner: boolean) {
        var lit = shadowPlane(alpha, w, h, bx, by, g.q, g.a + 180, dist, g.s, inner), dark = shadowPlane(alpha, w, h, bx, by, g.q, g.a, dist, g.s, inner), v = new Float32Array(n);
        for (var q = 0; q < n; q++) v[q] = 0.5 + 0.5 * (inner ? lit[q] - dark[q] : dark[q] - lit[q]);
        return v;
      };
      if (g.t !== 1) combineRamp(px, side(true), alpha, g.r, g.ko ? 2 : 1);
      if (g.t !== 0) { var inv = new Float32Array(n); for (k = 0; k < n; k++) inv[k] = 1 - alpha[k]; combineRamp(px, side(false), inv, g.r, g.ko ? 2 : 0); }
    }
    for (i = 0; i < n; i++) {
      var A = px[i * 4 + 3];
      p[i * 4 + 3] = Math.round(Math.max(0, Math.min(1, A)) * 255);
      for (var c = 0; c < 3; c++) p[i * 4 + c] = A > 0 ? Math.round(Math.max(0, Math.min(1, px[i * 4 + c] / A)) * 255) : 0;
    }
    x.putImageData(img, x0, y0);
  }

  // Adjust Color, per pixel (same maths as ac-render's adjust_color):
  //   rgb' = hueRotate(rgb); l = .213r + .715g + .072b
  //   v = l + (v - l) * (1 + saturation/100); v = (v - .5) * (1 + contrast/100) + .5 + brightness/100
  // hueRotate is the luminance-preserving matrix (as CSS hue-rotate, which
  // can't be used alone: CSS brightness() multiplies where Animate adds).
  function adjustColor(canvas: HTMLCanvasElement, p: number[]) {
    var x = canvas.getContext("2d", { willReadFrequently: true }) as CanvasRenderingContext2D;
    var img = readPixels(x, 0, 0, canvas.width, canvas.height); if (!img) return;
    var d = img.data;
    var cl = function (v: number) { return Math.max(-100, Math.min(100, v)); };
    var b = cl(p[0]) / 100, c = 1 + cl(p[1]) / 100, s = 1 + cl(p[2]) / 100;
    var sin = Math.sin(p[3] * Math.PI / 180), cos = Math.cos(p[3] * Math.PI / 180);
    var lr = 0.213, lg = 0.715, lb = 0.072;
    var m = [
      lr + cos * (1 - lr) + sin * -lr, lg + cos * -lg + sin * -lg, lb + cos * -lb + sin * (1 - lb),
      lr + cos * -lr + sin * 0.143, lg + cos * (1 - lg) + sin * 0.140, lb + cos * -lb + sin * -0.283,
      lr + cos * -lr + sin * -(1 - lr), lg + cos * -lg + sin * lg, lb + cos * (1 - lb) + sin * lb
    ];
    for (var i = 0; i < d.length; i += 4) {
      if (!d[i + 3]) continue;
      var r = d[i] / 255, g = d[i + 1] / 255, bl = d[i + 2] / 255;
      var v = [m[0] * r + m[1] * g + m[2] * bl, m[3] * r + m[4] * g + m[5] * bl, m[6] * r + m[7] * g + m[8] * bl];
      var l = lr * v[0] + lg * v[1] + lb * v[2];
      for (var k = 0; k < 3; k++) {
        var t = l + (v[k] - l) * s;
        t = (t - 0.5) * c + 0.5 + b;
        d[i + k] = Math.round(Math.max(0, Math.min(1, t)) * 255);
      }
    }
    x.putImageData(img, 0, 0);
  }

  // per-frame: overrides by src, button substitutions
  var ov: { [src: number]: Override } = {}, alt: { [src: number]: DList | null } = {}, altDone: { [src: number]: boolean } = {}, live: { name: string; b: NodeB }[] = [];
  function prepare(f: number) {
    ov = {}; alt = {}; altDone = {}; live = [];
    var n = extra(f).n || {};
    Object.keys(n).forEach(function (name) {
      var tb = n[name].tx, to: Inst | undefined = insts[name];
      if (tb && to) {
        if (to.text !== undefined && to.text !== tb.s) textSet[name] = true;
        // dynamic text changed by a script, and every input field, are drawn live
        if (tb.k === 2 || (tb.k === 1 && textSet[name])) live.push({ name: name, b: n[name] });
      }
      if (!changed[name]) return;
      var o = insts[name], b = n[name];
      if (!o.visible) { ov[b.src] = { hide: true }; return; }
      var d = decompose(b.m);
      var rot = o.rotation * Math.PI / 180 - d.ky;
      d.kx += rot; d.ky += rot; d.sx = +o.scaleX; d.sy = +o.scaleY; d.tx = +o.x; d.ty = +o.y;
      var want = new DOMMatrix(compose(d));
      ov[b.src] = { delta: want.multiply(new DOMMatrix(b.m).inverse()), a: b.a > 1e-4 ? o.alpha / b.a : 1 };
    });
    live.forEach(function (l) { ov[l.b.src] = Object.assign(ov[l.b.src] || {}, { hideText: true }); });
    (extra(f).b || []).forEach(function (bt) {
      if (press === bt.src && hover === bt.src) alt[bt.src] = bt.down;
      else if (hover === bt.src && !press) alt[bt.src] = bt.over;
    });
  }

  function drawList(c: CanvasRenderingContext2D, list: DList, depth: number) {
    for (var i = 0; i < list.length; i++) {
      var e = list[i];
      if (typeof e === "number") { drawItem(c, M.items[e], depth); continue; }
      var off = offscreen(depth + 1);
      if (e.sp !== undefined) {
        // EffectCraft effects were rendered at publish time (full-stage sprite)
        var sp = images[e.sp];
        if (sp.img.complete && sp.img.naturalWidth) {
          var sx = off.getContext("2d")!;
          sx.imageSmoothingEnabled = sp.smooth;
          sx.drawImage(sp.img, 0, 0, off.width, off.height);
        }
      } else if (e.mb) drawMotionBlur(off, e, depth);
      else if (e.cb !== undefined) drawCached(off, e, depth);
      else drawList(off.getContext("2d")!, e.c, depth + 1);
      var fx = M.fx[e.x];
      var src = off;
      if (e.mw) src = warpMesh(off, e.mw, depth + 1);
      if (fx.s) src = stages(off, fx.s, depth + 1);
      c.save();
      c.setTransform(1, 0, 0, 1, 0, 0);
      if (fx.f) c.filter = css(fx.f);
      c.globalCompositeOperation = fx.o;
      c.drawImage(src, 0, 0);
      c.restore();
    }
  }

  // Cache as bitmap: the group's picture is made once per distinct display
  // list and size, then reused (a static cached object costs one drawImage).
  // Nothing is cached while a bitmap is still loading.
  var bitmapCache: { [key: string]: HTMLCanvasElement } = {}, bitmapOrder: string[] = [];
  function drawCached(off: HTMLCanvasElement, e: Group, depth: number) {
    var key = e.cb + ":" + off.width + "x" + off.height, c = bitmapCache[key];
    if (!c) {
      var ready = images.every(function (im) { return im.img.complete; });
      if (!ready) { drawList(off.getContext("2d")!, e.c, depth + 1); return; }
      c = document.createElement("canvas");
      c.width = off.width; c.height = off.height;
      drawList(c.getContext("2d")!, e.c, depth + 1);
      bitmapCache[key] = c;
      bitmapOrder.push(key);
      if (bitmapOrder.length > 48) delete bitmapCache[bitmapOrder.shift() as string];
    }
    var x = off.getContext("2d")!;
    x.setTransform(1, 0, 0, 1, 0, 0);
    x.drawImage(c, 0, 0);
  }

  // Asset Warp: each grid triangle of the group's picture drawn through the
  // affine map from where it was to where it is (clip + transform), as in
  // ac-render's warp_mesh; triangles grown half a pixel to hide seams.
  function warpMesh(off: HTMLCanvasElement, mw: MeshWarp, depth: number): HTMLCanvasElement {
    var out = warpPool[depth] || (warpPool[depth] = document.createElement("canvas"));
    if (out.width !== off.width || out.height !== off.height) { out.width = off.width; out.height = off.height; }
    var x = out.getContext("2d")!, s = mw.s, d = mw.d, cols = mw.c, rows = mw.r;
    x.setTransform(1, 0, 0, 1, 0, 0);
    x.clearRect(0, 0, out.width, out.height);
    function P(v: number[], i: number): number[] { return [v[i * 2] * dpr, v[i * 2 + 1] * dpr]; }
    function tri(a: number, b: number, c: number) {
      var s0 = P(s, a), s1 = P(s, b), s2 = P(s, c), d0 = P(d, a), d1 = P(d, b), d2 = P(d, c);
      // source → destination affine: solve [d] = M [s]
      var ux = s1[0] - s0[0], uy = s1[1] - s0[1], vx = s2[0] - s0[0], vy = s2[1] - s0[1];
      var det = ux * vy - vx * uy;
      if (Math.abs(det) < 1e-9) return;
      var px = d1[0] - d0[0], py = d1[1] - d0[1], qx = d2[0] - d0[0], qy = d2[1] - d0[1];
      var A = (px * vy - qx * uy) / det, C = (qx * ux - px * vx) / det;
      var B = (py * vy - qy * uy) / det, D = (qy * ux - py * vx) / det;
      var E = d0[0] - A * s0[0] - C * s0[1], F = d0[1] - B * s0[0] - D * s0[1];
      var cx = (d0[0] + d1[0] + d2[0]) / 3, cy = (d0[1] + d1[1] + d2[1]) / 3;
      function g(p: number[]): number[] { var dx = p[0] - cx, dy = p[1] - cy, l = Math.hypot(dx, dy) || 1; return [p[0] + dx / l * 0.5, p[1] + dy / l * 0.5]; }
      var g0 = g(d0), g1 = g(d1), g2 = g(d2);
      x.save();
      x.setTransform(1, 0, 0, 1, 0, 0);
      x.beginPath(); x.moveTo(g0[0], g0[1]); x.lineTo(g1[0], g1[1]); x.lineTo(g2[0], g2[1]); x.closePath();
      x.clip();
      x.setTransform(A, B, C, D, E, F);
      x.drawImage(off, 0, 0);
      x.restore();
    }
    for (var r = 0; r < rows; r++) for (var c = 0; c < cols; c++) {
      var a = r * (cols + 1) + c, b = a + 1, dd = a + cols + 1, ee = dd + 1;
      tri(a, b, ee); tri(a, ee, dd);
    }
    return out;
  }

  // Motion Blur: the group drawn at e.mb.n instants over the shutter, averaged
  // (each sample added at 1/n with "lighter"), as in ac-render's draw_group.
  function drawMotionBlur(off: HTMLCanvasElement, e: Group, depth: number) {
    var mb = e.mb as MotionBlur, n = mb.n, oc = off.getContext("2d")!;
    var tmp = mbPool[depth] || (mbPool[depth] = document.createElement("canvas"));
    if (tmp.width !== off.width || tmp.height !== off.height) { tmp.width = off.width; tmp.height = off.height; }
    var tc = tmp.getContext("2d")!;
    var saved = MB;
    for (var k = 0; k < n; k++) {
      var u = (k + 0.5) / n * mb.f, m = mb.m;
      MB = new DOMMatrix([1 + (m[0] - 1) * u, m[1] * u, m[2] * u, 1 + (m[3] - 1) * u, m[4] * u, m[5] * u]);
      if (saved) MB = saved.multiply(MB);
      tc.setTransform(1, 0, 0, 1, 0, 0);
      tc.clearRect(0, 0, tmp.width, tmp.height);
      drawList(tc, e.c, depth + 1);
      oc.save();
      oc.setTransform(1, 0, 0, 1, 0, 0);
      oc.globalCompositeOperation = "lighter";
      oc.globalAlpha = 1 / n;
      oc.drawImage(tmp, 0, 0);
      oc.restore();
    }
    MB = saved;
  }

  function drawItem(c: CanvasRenderingContext2D, it: Item, depth: number) {
    var o: Override | null = it.src !== undefined ? ov[it.src] : null;
    if (it.src !== undefined && alt[it.src]) {
      if (!altDone[it.src]) { altDone[it.src] = true; var l = alt[it.src] as DList; alt[it.src] = null; drawList(c, l, depth); alt[it.src] = l; }
      return;
    }
    if (o && o.hide) return;
    if (o && o.hideText && !it.s) return; // its outlines: the live text replaces them (borders stay)
    var m = new DOMMatrix(it.m);
    if (o) m = (o.delta as DOMMatrix).multiply(m);
    var fa = o ? (o.a as number) : 1;
    var p = paths[it.p];
    c.save();
    if (it.c) {
      c.setTransform(BASE);
      for (var k = 0; k < it.c.length; k++) c.clip(clipPath(it.c[k]), "nonzero");
    }
    var T = (MB ? BASE.multiply(MB) : BASE).multiply(m);
    c.setTransform(T);
    c.globalAlpha = fa * (it.a === undefined ? 1 : it.a);
    if (it.ol) {
      c.lineWidth = 1 / Math.max(1e-3, Math.sqrt(Math.abs(T.a * T.d - T.b * T.c)));
      c.strokeStyle = it.ol;
      c.stroke(p);
      c.restore();
      return;
    }
    if (it.img !== undefined) {
      var im = images[it.img];
      c.imageSmoothingEnabled = im.smooth;
      if (im.img.complete && im.img.naturalWidth) c.drawImage(im.img, 0, 0);
    } else if (it.pat !== undefined) {
      var pi = images[it.pat];
      if (pi.img.complete && pi.img.naturalWidth) {
        var pat = c.createPattern(pi.img, "repeat") as CanvasPattern;
        if (pat.setTransform) pat.setTransform(new DOMMatrix(it.pm));
        c.imageSmoothingEnabled = pi.smooth;
        c.fillStyle = pat;
        c.fill(p, "evenodd");
      }
    } else if (it.g) {
      var gm = new DOMMatrix(it.g.m);
      var p2 = new Path2D();
      p2.addPath(p, gm.inverse());
      c.setTransform(T.multiply(gm));
      var ex = it.g.x || 1;
      var gr = it.g.k === 0 ? c.createLinearGradient(-ex, 0, ex, 0) : c.createRadialGradient(it.g.fx || 0, 0, 0, 0, 0, ex);
      it.g.s.forEach(function (s) { gr.addColorStop(Math.min(1, Math.max(0, s[0])), s[1]); });
      c.fillStyle = gr;
      c.fill(p2, "evenodd");
      c.setTransform(T);
    } else if (it.f) {
      c.fillStyle = it.f;
      c.fill(p, "evenodd");
    }
    if (it.s) {
      var cj = (it.cj || "round round").split(" ");
      c.lineCap = cj[0] as CanvasLineCap;
      c.lineJoin = cj[1] as CanvasLineJoin;
      c.lineWidth = it.w as number;
      c.setLineDash(it.d || []);
      c.strokeStyle = it.s;
      if (it.hl) {
        c.lineWidth = 1 / Math.max(1e-3, Math.sqrt(Math.abs(T.a * T.d - T.b * T.c)));
        c.stroke(p);
      } else if (it.sm) {
        // pen follows only part of the instance scale (h = horizontal, v = vertical, n = none)
        var L = (MB ? MB : new DOMMatrix()).multiply(m);
        var sx = Math.hypot(L.a, L.b), sy = Math.hypot(L.c, L.d);
        if (sx > 1e-6 && sy > 1e-6) {
          var R = it.sm === "h" ? new DOMMatrix([1, 0, 0, sy, 0, 0]) : it.sm === "v" ? new DOMMatrix([sx, 0, 0, 1, 0, 0]) : new DOMMatrix();
          var p3 = new Path2D();
          p3.addPath(p, R.inverse().multiply(L));
          c.setTransform(BASE.multiply(R));
          c.stroke(p3);
        } else c.stroke(p);
      } else c.stroke(p);
    }
    c.restore();
  }

  function frameList() {
    var v = extra(frame).v;
    return v ? v[clock % v.length] : M.frames[frame];
  }
  function draw() {
    prepare(frame);
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.globalAlpha = 1;
    ctx.fillStyle = M.bg;
    ctx.fillRect(0, 0, cv.width, cv.height);
    drawList(ctx, frameList(), 0);
    drawLive();
    dirty = false;
  }

  // ---------------------------------------------------------------- text fields
  var inputs: { [name: string]: HTMLInputElement | HTMLTextAreaElement } = {};
  function wrapLines(c: CanvasRenderingContext2D, text: string, tb: TextBox): string[] {
    var out: string[] = [];
    text.split("\n").forEach(function (para) {
      if (!tb.w) { out.push(para); return; }
      var words = para.split(" "), line = "";
      words.forEach(function (wd) {
        var tryLine = line ? line + " " + wd : wd;
        if (line && c.measureText(tryLine).width > tb.w - tb.ml) { out.push(line); line = wd; } else line = tryLine;
      });
      out.push(line);
    });
    return out;
  }
  function liveMatrix(l: { name: string; b: NodeB }): DOMMatrix | null {
    var its = itemsOf(frameList(), l.b.src, []);
    if (!its.length) return null;
    var o = ov[l.b.src], m = new DOMMatrix(its[its.length - 1].m);
    return o && o.delta ? o.delta.multiply(m) : m;
  }
  function drawLive() {
    var seen: { [name: string]: boolean } = {};
    live.forEach(function (l) {
      var tb = l.b.tx as TextBox, o = insts[l.name], m = liveMatrix(l);
      if (!m || (o && !o.visible)) return;
      seen[l.name] = true;
      if (tb.k === 2) { placeInput(l.name, tb, m, o); return; }
      ctx.save();
      ctx.setTransform(BASE.multiply(m));
      ctx.font = tb.f;
      ctx.fillStyle = tb.c;
      ctx.textBaseline = "top";
      if (ctx.letterSpacing !== undefined) ctx.letterSpacing = tb.ls + "px";
      var text = String(o.text === undefined || o.text === null ? "" : o.text);
      var lines = wrapLines(ctx, text, tb), width = tb.w || Math.max.apply(null, lines.map(function (s) { return ctx.measureText(s).width; }).concat([0]));
      lines.forEach(function (s, i) {
        var lw = ctx.measureText(s).width, x = tb.ml + (i === 0 ? tb.in : 0);
        if (tb.al === "center") x += (width - lw) / 2; else if (tb.al === "right") x += width - lw;
        ctx.fillText(s, x, i * tb.lh + tb.sz * 0.1);
      });
      ctx.restore();
    });
    Object.keys(inputs).forEach(function (n) { if (!seen[n]) inputs[n].style.display = "none"; });
  }
  function placeInput(name: string, tb: TextBox, m: DOMMatrix, o: Inst) {
    var el: HTMLInputElement | HTMLTextAreaElement = inputs[name];
    if (!el) {
      el = inputs[name] = document.createElement(tb.w ? "textarea" : "input");
      if (!tb.w) (el as HTMLInputElement).type = tb.pw ? "password" : "text";
      if (tb.mc) el.maxLength = tb.mc;
      el.value = o.text || "";
      el.style.cssText = "position:fixed;margin:0;padding:0;outline:none;resize:none;overflow:hidden;box-sizing:border-box;";
      el.style.background = tb.bd ? "#fff" : "transparent";
      el.style.border = tb.bd ? "1px solid #000" : "none";
      el.addEventListener("input", function () { o.text = el.value; textSet[name] = true; dirty = true; });
      document.body.appendChild(el);
    }
    if (document.activeElement !== el && el.value !== String(o.text || "")) el.value = o.text || "";
    var r = cv.getBoundingClientRect(), k = r.width / M.w;
    var sx = Math.hypot(m.a, m.b) * k, sy = Math.hypot(m.c, m.d) * k;
    el.style.display = "block";
    el.style.left = (r.left + m.e * k) + "px";
    el.style.top = (r.top + m.f * k) + "px";
    el.style.width = Math.max(20, tb.bw * sx + 4) + "px";
    el.style.height = Math.max(tb.sz * 1.3, tb.bh) * sy + 4 + "px";
    el.style.font = tb.f.replace(/[0-9.]+px/, (tb.sz * sy) + "px");
    el.style.color = tb.c;
    el.style.textAlign = tb.al === "justify" ? "left" : tb.al;
    el.style.letterSpacing = (tb.ls * sx) + "px";
  }

  // ---------------------------------------------------------------- sound
  var actx: AudioContext | null = null, buffers: AudioBuffer[] = [], voices: Voice[] = [];
  try {
    var ac = actx = new (window.AudioContext || (window.webkitAudioContext as typeof AudioContext))();
    M.sounds.forEach(function (s, i) {
      var bin = atob(s[1].split(",")[1]);
      var u8 = new Uint8Array(bin.length);
      for (var k = 0; k < bin.length; k++) u8[k] = bin.charCodeAt(k);
      ac.decodeAudioData(u8.buffer, function (b) { buffers[i] = b; }, function () { console.warn("Flare: can't decode sound " + s[0]); });
    });
  } catch (e) { actx = null; }
  function unlock() { if (actx && actx.state === "suspended") actx.resume(); }
  window.addEventListener("pointerdown", unlock, true);
  window.addEventListener("keydown", unlock, true);

  function startSound(e: SoundEv, f: number) {
    if (!actx) return;
    var alive = voices.filter(function (v) { return v.alive; });
    if (e.y === "stop") { alive.forEach(function (v) { if (v.s === e.s) stopVoice(v); }); return; }
    if (e.y === "start" && alive.some(function (v) { return v.s === e.s; })) return;
    var b = buffers[e.s];
    if (!b) return;
    var src = actx.createBufferSource();
    src.buffer = b;
    src.loop = e.l || e.r > 1;
    var t0 = actx.currentTime;
    var total = e.l ? Infinity : b.duration * e.r;
    if (e.y === "stream") total = Math.min(total, Math.max(0, (e.e - f) / M.fps));
    var out: AudioNode = src;
    if (e.fx) {
      // Properties > Sound > Effect: pan and fades over one play of the sound
      var gain = actx.createGain(), pan = actx.createStereoPanner ? actx.createStereoPanner() : null;
      var span = isFinite(total) ? total : b.duration;
      if (e.fx === "LeftChannel" && pan) pan.pan.value = -1;
      if (e.fx === "RightChannel" && pan) pan.pan.value = 1;
      if (e.fx === "FadeLeftToRight" && pan) { pan.pan.setValueAtTime(-1, t0); pan.pan.linearRampToValueAtTime(1, t0 + span); }
      if (e.fx === "FadeRightToLeft" && pan) { pan.pan.setValueAtTime(1, t0); pan.pan.linearRampToValueAtTime(-1, t0 + span); }
      if (e.fx === "FadeIn") { gain.gain.setValueAtTime(0, t0); gain.gain.linearRampToValueAtTime(1, t0 + b.duration); }
      if (e.fx === "FadeOut" && isFinite(total)) { var d = Math.min(b.duration, total); gain.gain.setValueAtTime(1, t0 + total - d); gain.gain.linearRampToValueAtTime(0, t0 + total); }
      src.connect(gain);
      out = gain;
      if (pan) { gain.connect(pan); out = pan; }
    }
    out.connect(actx.destination);
    src.start(t0);
    if (!e.l && e.r > 1) src.stop(t0 + b.duration * e.r);
    var v = { s: e.s, src: src, alive: true, stream: e.y === "stream", from: f, end: e.e };
    src.onended = function () { v.alive = false; };
    voices.push(v);
    if (voices.length > 64) voices = voices.filter(function (x) { return x.alive; });
  }
  function stopVoice(v: Voice) { if (v.alive) { try { v.src.stop(); } catch (e) { /* already stopped */ } v.alive = false; } }
  function cutStreams() {
    voices.forEach(function (v) { if (v.stream && v.alive && (!playing || frame >= v.end || frame < v.from)) stopVoice(v); });
  }

  // ---------------------------------------------------------------- scripts
  var jump: { f: number; play: boolean } | null = null;
  function target(t: number | string): number {
    var sc = sceneOf(frame);
    if (typeof t === "number" || /^\s*-?\d+(\.\d+)?\s*$/.test(String(t))) return Math.max(0, Math.min(N - 1, sc.start + Math.floor(+t)));
    var si = M.scenes.indexOf(sc), hit: number | null = null;
    M.labels.forEach(function (l) { if (l[0] === t && (hit === null || l[1] === si)) hit = M.scenes[l[1]].start + l[2]; });
    if (hit === null) throw new Error('no frame label "' + t + '"');
    return Math.max(0, Math.min(N - 1, hit));
  }
  var tl: Timeline = {
    stop: function () { playing = false; },
    play: function () { playing = true; },
    gotoAndPlay: function (f) { jump = { f: target(f), play: true }; },
    gotoAndStop: function (f) { jump = { f: target(f), play: false }; },
    nextFrame: function () { jump = { f: Math.min(N - 1, frame + 1), play: false }; },
    prevFrame: function () { jump = { f: Math.max(0, frame - 1), play: false }; },
    getChildByName: function (n) { return (tl[n] as Inst | Timeline | undefined) || null; },
  } as Timeline;
  Object.defineProperty(tl, "currentFrame", { get: function () { return frame - sceneOf(frame).start; } });
  Object.defineProperty(tl, "totalFrames", { get: function () { return sceneOf(frame).count; } });

  function runScript(code: string) {
    var fn = scriptCache[code];
    if (!fn) {
      try {
        fn = scriptCache[code] = new Function("stop", "play", "gotoAndPlay", "gotoAndStop", "print", code) as Fn;
      } catch (e) { console.error("Flare frame script:", e); scriptCache[code] = function () {}; return; }
    }
    var names = Object.keys(insts), before: { [name: string]: string } = {};
    names.forEach(function (n) { before[n] = snapshot(insts[n]); });
    try {
      fn.call(tl, tl.stop, tl.play, tl.gotoAndPlay, tl.gotoAndStop, function () { console.log.apply(console, arguments as any); });
    } catch (e) {
      console.error("Flare frame " + (frame + 1) + " script:", e);
      playing = false;
    }
    Object.keys(insts).forEach(function (n) { if (before[n] !== undefined && before[n] !== snapshot(insts[n])) changed[n] = true; });
  }

  // entering a frame: sounds, then scripts (following up to 8 jumps)
  function enter() {
    for (var hop = 0; hop < 8; hop++) {
      var x = extra(frame);
      bindInstances(frame);
      cutStreams();
      (x.snd || []).forEach(function (e) { startSound(e, frame); });
      jump = null;
      (x.js || []).forEach(runScript);
      var jp = jump as { f: number; play: boolean } | null;
      if (!jp) break;
      var same = jp.f === frame;
      frame = jp.f;
      playing = jp.play;
      if (same) break;
    }
    cutStreams();
    dirty = true;
  }

  function advance() {
    if (frame + 1 >= N) {
      if (M.loop) frame = 0;
      else { playing = false; cutStreams(); return; }
    } else frame++;
    enter();
  }

  // ---------------------------------------------------------------- input
  function stagePoint(ev: MouseEvent) {
    var r = cv.getBoundingClientRect();
    return { x: (ev.clientX - r.left) * (M.w / r.width), y: (ev.clientY - r.top) * (M.h / r.height) };
  }
  function itemsOf(list: DList, src: number, out: Item[]): Item[] {
    list.forEach(function (e) {
      if (typeof e === "number") { if (M.items[e].src === src) out.push(M.items[e]); }
      else itemsOf(e.c, src, out);
    });
    return out;
  }
  function hitTest(pt: { x: number; y: number }): { src: number; name: string; button: boolean } | null {
    var x = extra(frame), px = pt.x * dpr, py = pt.y * dpr;
    var bs = x.b || [];
    for (var i = bs.length - 1; i >= 0; i--) {
      var b = bs[i];
      var shapes = b.hit.length ? b.hit.map(function (h) { return { p: h[0] as unknown as number, m: h[1] }; }) : itemsOf(frameList(), b.src, []);
      for (var k = 0; k < shapes.length; k++) {
        ctx.setTransform(BASE.multiply(new DOMMatrix(shapes[k].m)));
        if (ctx.isPointInPath(paths[shapes[k].p], px, py)) return { src: b.src, name: b.name, button: true };
      }
    }
    var n = x.n || {};
    var names = Object.keys(n);
    for (var j = names.length - 1; j >= 0; j--) {
      var o = insts[names[j]];
      var linked = n[names[j]].tx && n[names[j]].tx!.url;
      if (!o || !(linked || Object.keys(o._l).some(function (t) { return o._l[t].length; }))) continue;
      var its = itemsOf(frameList(), n[names[j]].src, []);
      for (var q = 0; q < its.length; q++) {
        ctx.setTransform(BASE.multiply(new DOMMatrix(its[q].m)));
        if (ctx.isPointInPath(paths[its[q].p], px, py)) return { src: n[names[j]].src, name: names[j], button: false };
      }
    }
    return null;
  }
  function fire(name: string | null, type: string, pt: { x: number; y: number }) {
    var o = name && insts[name];
    if (!o || !o._l[type]) return;
    var ev = { type: type, target: o, currentTarget: o, stageX: pt.x, stageY: pt.y };
    o._l[type].slice().forEach(function (fn) {
      try { fn.call(tl, ev); } catch (e) { console.error("Flare " + type + " handler:", e); }
    });
    if (jump) { frame = jump.f; playing = jump.play; jump = null; enter(); }
    dirty = true;
  }
  var overName: string | null = null;
  cv.addEventListener("pointermove", function (ev) {
    var pt = stagePoint(ev), h = hitTest(pt);
    var src = h && h.button ? h.src : null;
    if (src !== hover) { hover = src; dirty = true; }
    var name = h ? h.name : null;
    if (name !== overName) { fire(overName, "mouseout", pt); fire(name, "mouseover", pt); overName = name; }
    cv.style.cursor = h ? "pointer" : "default";
  });
  cv.addEventListener("pointerleave", function () { hover = null; press = null; dirty = true; });
  cv.addEventListener("pointerdown", function (ev) {
    var h = hitTest(stagePoint(ev));
    press = h && h.button ? h.src : null;
    pressName = h ? h.name : null;
    dirty = true;
  });
  var pressName: string | null = null;
  function openUrl(u: string) {
    var ok = false;
    try { ok = /^(https?|mailto):$/.test(new URL(String(u), window.location && window.location.href).protocol); } catch (e) {}
    if (ok) window.open(String(u), "_blank", "noopener,noreferrer");
  }
  cv.addEventListener("pointerup", function (ev) {
    var pt = stagePoint(ev), h = hitTest(pt);
    if (h && h.name === pressName) {
      var nn = extra(frame).n || {};
      if (nn[h.name] && nn[h.name].tx && nn[h.name].tx!.url) openUrl(nn[h.name].tx!.url!);
      fire(h.name, "click", pt);
    }
    press = null; pressName = null;
    dirty = true;
  });

  // ---------------------------------------------------------------- loop
  var acc = 0, last: number | null = null;
  function tick(t: number) {
    if (last === null) last = t;
    var dt = Math.min(0.25, (t - last) / 1000);
    last = t;
    acc += dt;
    var spf = 1 / M.fps;
    while (acc >= spf) {
      acc -= spf;
      clock++;
      if (extra(frame).v) dirty = true;
      if (playing) advance();
    }
    if (!playing) cutStreams();
    if (dirty || Object.keys(changed).length) draw();
    requestAnimationFrame(tick);
  }
  enter();
  draw();
  requestAnimationFrame(tick);
  // for tests and embedding pages
  window.Flare = window.AnimationCraft = {
    get frame() { return frame; },
    get playing() { return playing; },
    gotoAndStop: function (f: number) { frame = Math.max(0, Math.min(N - 1, f)); playing = false; enter(); draw(); },
    play: function () { playing = true; },
    stop: function () { playing = false; },
    timeline: tl,
  };
})();
