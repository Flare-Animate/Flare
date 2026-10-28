// Tests for the web app bundle. Run by pages.yml before it builds, so a broken
// bundle fails the deploy instead of shipping.
//
//   node tools/flash/webapp/build.test.ts
import test from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

import { build, bundle, toPlainJs } from "./build.ts";
import { render } from "./app.ts";
import { parseHtml5, scriptSrcs } from "../ts/html5-import.ts";

/** Minimal CreateJS-shaped export: enough for parseHtml5's regexes to bite.
 *  parseHtml5's guard is `/createjs|cjs\./`, so the sample needs a lowercase
 *  "createjs" occurrence the way a real export has (createjs.Ticker and friends). */
const SAMPLE = `createjs.Ticker.framerate = 24;
lib.properties = { width: 1280, height: 720, fps: 24, color: "#101014",
  manifest: [ { id: "img1", src: "assets/img1.png" } ] };
(lib.hero = function(){ new lib.img1(); this.addTween().wait(3).to({x:10},7).wait(2); });
(lib.anim = function(){ this.addTween().wait(5).to({y:9},9); });
lib.root = new lib.hero();`;

test("parseHtml5 reads dimensions, fps and background", () => {
  const m = parseHtml5(SAMPLE);
  assert.equal(m.format, "animate-html5");
  assert.equal(m.width, 1280);
  assert.equal(m.height, 720);
  assert.equal(m.fps, 24);
  assert.equal(m.bg, "#101014");
});

test("parseHtml5 collects symbols, children and stage", () => {
  const m = parseHtml5(SAMPLE);
  assert.deepEqual(m.symbols.map((s) => s.name), ["hero", "anim"]);
  assert.deepEqual(m.stage, "hero");
  assert.deepEqual(m.symbols[0].children, ["img1"]);
  // img1 is in the manifest, so it is also reported as a bitmap.
  assert.deepEqual(m.symbols[0].bitmaps, ["img1"]);
  assert.deepEqual(m.assets, [{ src: "assets/img1.png", id: "img1" }]);
});

test("parseHtml5 sums wait+to into frame counts", () => {
  const m = parseHtml5(SAMPLE);
  assert.equal(m.symbols[0].frames, 3 + 7 + 2);
  assert.equal(m.symbols[1].frames, 5 + 9);
});

test("parseHtml5 rejects non-Animate input", () => {
  assert.throws(() => parseHtml5("console.log('hello')"), /not an Animate canvas export/);
});

test("scriptSrcs skips the CreateJS runtime", () => {
  const html = `<script src="https://code.createjs.com/1.0.0/createjs.min.js"></script>
                <script src="js/app.js"></script><script src="js/data.js"></script>`;
  assert.deepEqual(scriptSrcs(html), ["js/app.js", "js/data.js"]);
});

test("toPlainJs erases types and leaves no import/export", () => {
  const out = toPlainJs(
    `interface A { x: number }\nimport { b } from "./b.ts";\nexport const f = (n: A): string => b(n.x);`,
    "sample.ts",
  );
  assert.match(out, /const f/);
  assert.doesNotMatch(out, /interface/);
  assert.doesNotMatch(out, /^\s*(import|export)\s/m);
});

test("bundle parses as JavaScript and defines the entry points", () => {
  const js = bundle();
  // new Function is the cheapest parse check that does not need a temp file.
  new Function(js);
  assert.match(js, /function parseHtml5/);
  assert.match(js, /function render/);
});

test("build emits the three artifacts Pages deploys", () => {
  const dir = mkdtempSync(join(tmpdir(), "flare-webapp-"));
  try {
    const written = build(dir);
    assert.deepEqual(written.sort(), ["app.js", "index.html", "sw.js"]);
    const html = readFileSync(join(dir, "index.html"), "utf8");
    // app.ts wires #f (file input) and #out (result target) by id.
    assert.match(html, /id="f"/);
    assert.match(html, /id="out"/);
    assert.match(html, /src="\.\/app\.js"/);
    // app.ts registers ./sw.js, so the worker has to actually be published.
    assert.match(readFileSync(join(dir, "sw.js"), "utf8"), /addEventListener\("fetch"/);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("the real sources still bundle into runnable JavaScript", async () => {
  const js = bundle();
  // Re-import the bundle as a module to confirm the stripped output executes.
  // The bundle is flat, so re-export the pieces the tests need.
  const dir = mkdtempSync(join(tmpdir(), "flare-webapp-run-"));
  try {
    const file = join(dir, "bundle.mjs");
    writeFileSync(file, `${js}\nexport { parseHtml5, render };\n`, "utf8");
    // app.js guards its DOM wiring behind `typeof document !== "undefined"`,
    // so importing it under Node is safe and must not throw.
    const mod = await import(pathToFileURL(file).href);
    assert.equal(typeof mod.parseHtml5, "function");
    assert.equal(mod.parseHtml5(SAMPLE).symbols.length, 2);
    // render() builds the symbol table the page shows.
    const html = mod.render(SAMPLE);
    assert.match(html, /<table>/);
    assert.match(html, /hero/);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("render marks up the symbol table", () => {
  const html = render(SAMPLE);
  assert.match(html, /1280x720 @24fps, 2 symbols/);
  assert.match(html, /<th>Symbol<\/th>/);
  assert.match(html, /hero/);
});