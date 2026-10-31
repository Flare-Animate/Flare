// flare-ruffle.js with a fake page: a document that records elements, and a window whose Ruffle
// script "loads" when it is appended. Checks the overlay's life (open, replace, close by button
// and by Escape), that Ruffle is fetched once and only on the first play, and the failure paths.
//
//   node scripts/test_flare_ruffle.mjs
import assert from "node:assert/strict";
import { createFlareRuffle } from "../src/lib/flare-ruffle.js";

function element(tag) {
  const e = {
    tag,
    children: [],
    style: {},
    listeners: {},
    attrs: {},
    parent: null,
    textContent: "",
    setAttribute(k, v) { this.attrs[k] = v; },
    addEventListener(type, fn) { (this.listeners[type] ||= []).push(fn); },
    append(...nodes) { for (const n of nodes) this.appendChild(n); },
    appendChild(n) { n.parent = this; this.children.push(n); return n; },
    remove() { if (this.parent) this.parent.children = this.parent.children.filter((c) => c !== this); this.parent = null; },
  };
  return e;
}

function page({ ruffleWorks = true, loadFails = false } = {}) {
  const document_ = {
    listeners: {},
    head: element("head"),
    body: element("body"),
    createElement: (tag) => element(tag),
    addEventListener(type, fn) { (this.listeners[type] ||= []).push(fn); },
    removeEventListener(type, fn) { this.listeners[type] = (this.listeners[type] || []).filter((f) => f !== fn); },
    press(key) { for (const f of [...(this.listeners.keydown || [])]) f({ key }); },
  };
  const loads = [];
  const created = [];
  const win = {};
  // Appending the script makes Ruffle "load": it registers its player, as the real one does.
  const appendChild = document_.head.appendChild.bind(document_.head);
  document_.head.appendChild = (script) => {
    appendChild(script);
    loads.push({ src: script.src, config: { ...win.RufflePlayer.config } });
    queueMicrotask(() => {
      if (loadFails) return script.onerror();
      if (ruffleWorks) {
        win.RufflePlayer.newest = () => ({
          createPlayer: () => {
            const player = element("ruffle-player");
            player.load = async (options) => { player.loaded = options; };
            created.push(player);
            return player;
          },
        });
      }
      script.onload();
    });
    return script;
  };
  return { document: document_, win, loads, created };
}

const base = "http://localhost/app/";
const bytes = new Uint8Array([0x46, 0x57, 0x53, 10]);
const overlays = (p) => p.document.body.children;

// Nothing is fetched until a movie plays.
{
  const p = page();
  createFlareRuffle({ doc: p.document, win: p.win, base });
  assert.equal(p.loads.length, 0);
}

// A play opens an overlay holding Ruffle's player, loaded with the bytes.
{
  const p = page();
  const ruffle = createFlareRuffle({ doc: p.document, win: p.win, base });
  await ruffle.play(bytes);
  assert.equal(p.loads.length, 1);
  assert.equal(p.loads[0].src, `${base}vendor/ruffle/ruffle.js`);
  assert.equal(p.loads[0].config.publicPath, `${base}vendor/ruffle/`);
  assert.equal(p.loads[0].config.autoplay, "on");
  assert.equal(overlays(p).length, 1);
  assert.equal(overlays(p)[0].attrs.role, "dialog");
  assert.equal(p.created.length, 1);
  assert.deepEqual(p.created[0].loaded, { data: bytes });
  assert.ok(ruffle.isOpen());

  // A second play replaces the movie and does not fetch Ruffle again.
  await ruffle.play(bytes);
  assert.equal(p.loads.length, 1);
  assert.equal(overlays(p).length, 1);
  assert.equal(p.created.length, 2);

  // Escape closes it, and takes its key listener with it.
  p.document.press("a");
  assert.ok(ruffle.isOpen());
  p.document.press("Escape");
  assert.ok(!ruffle.isOpen());
  assert.equal(overlays(p).length, 0);
  assert.equal((p.document.listeners.keydown || []).length, 0);

  // The Close button closes too.
  await ruffle.play(bytes);
  const button = overlays(p)[0].children.find((c) => c.tag === "button");
  button.listeners.click[0]();
  assert.equal(overlays(p).length, 0);
}

// Closing while Ruffle is still loading leaves nothing behind.
{
  const p = page();
  const ruffle = createFlareRuffle({ doc: p.document, win: p.win, base });
  const playing = ruffle.play(bytes);
  ruffle.close();
  await playing;
  assert.equal(overlays(p).length, 0);
  assert.equal(p.created.length, 0);
}

// A script that cannot be fetched is an error, closes the overlay, and a later play tries again.
{
  const p = page({ loadFails: true });
  const ruffle = createFlareRuffle({ doc: p.document, win: p.win, base });
  await assert.rejects(ruffle.play(bytes), /could not be loaded/);
  assert.equal(overlays(p).length, 0);
  assert.equal(p.win.RufflePlayer, undefined);
  await assert.rejects(ruffle.play(bytes), /could not be loaded/);
  assert.equal(p.loads.length, 2);
}

// A script that loads but registers nothing is reported.
{
  const p = page({ ruffleWorks: false });
  const ruffle = createFlareRuffle({ doc: p.document, win: p.win, base });
  await assert.rejects(ruffle.play(bytes), /did not register/);
  assert.equal(overlays(p).length, 0);
}

console.log("ok: flare-ruffle.js");
