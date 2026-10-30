// Runs a published page's player against a stand-in canvas and prints what it drew, as JSON.
// Usage: node html5_player_harness.js page.html
"use strict";
const fs = require("fs");

const html = fs.readFileSync(process.argv[2], "utf8");
const dataText = html.match(/<script id="flare-data" type="application\/json">([\s\S]*?)<\/script>/)[1];
const playerText = html.match(/<\/script>\s*<script>([\s\S]*?)<\/script>/)[1];

const calls = [];
const ctx = new Proxy({}, {
  get(target, name) {
    if (name in target) return target[name];
    return (...args) => {
      if (name === "createLinearGradient") {
        const gradient = { stops: [], addColorStop(at, c) { this.stops.push([at, c]); } };
        calls.push(["gradient", ...args]);
        return gradient;
      }
      calls.push([name, ...args.map((a) => (a && a.d !== undefined ? { path: a.d } : a))]);
    };
  },
  set(target, name, value) {
    target[name] = value;
    if (name === "fillStyle" || name === "strokeStyle" || name === "lineWidth") {
      calls.push(["set " + name, typeof value === "object" ? "gradient" : value]);
    }
    return true;
  },
});
const canvas = { style: {}, width: 0, height: 0, getContext: () => ctx };
const callbacks = [];
const listeners = {};
global.window = {
  innerWidth: 640,
  innerHeight: 360,
  devicePixelRatio: 1,
  addEventListener: (name, fn) => { listeners[name] = fn; },
  requestAnimationFrame: (fn) => { callbacks.push(fn); },
};
global.document = {
  getElementById: (id) => (id === "flare-data" ? { textContent: dataText } : canvas),
};
global.Path2D = class { constructor(d) { this.d = d; } addPath() {} };
global.DOMMatrix = class { constructor(values) { this.values = values; } };

// Frames drawn: a frame starts with the background fill.
function framesAt(times) {
  const out = [];
  for (const time of times) {
    const next = callbacks.splice(0);
    const before = calls.length;
    next.forEach((fn) => fn(time));
    out.push(calls.slice(before));
  }
  return out;
}

new Function(playerText)();
const first = calls.splice(0);
const later = framesAt([0, 1000, 2000]);
const data = JSON.parse(dataText);
// Resizing keeps what is on show.
const sizeBefore = [canvas.width, canvas.height];
window.innerWidth = 800;
listeners.resize && listeners.resize();
const resized = [canvas.width, canvas.height];
console.log(JSON.stringify({
  first,
  later,
  rafLeft: callbacks.length,
  size: sizeBefore,
  resized,
  style: canvas.style,
  fps: data.fps,
  frames: data.seq.length,
}));
