// Loads the WebAssembly module built by scripts/build.sh. The module starts itself on the
// canvas with id "flare-canvas".
//
// First it loads what a page lacks for .flare files, SQLite and zstd as WebAssembly (see
// flare-file.js), and registers them as `globalThis.flareFile` for the Rust side. If that fails the
// editor still starts; opening and saving a .flare then say why they cannot.
import init, { flarePenReport } from "./pkg/flare_web.js";
import { createFlareRuffle } from "./flare-ruffle.js";
import { installPenPressure } from "./pen-pressure.js";

// Test Movie. Nothing of Ruffle is fetched until a movie is played.
globalThis.flareRuffle = createFlareRuffle();

// A phone or a tablet gets the touch interface: bigger targets everywhere, and a page that
// does not zoom under the fingers. The workspace is told through `globalThis.flareMobile`
// (read by canvas.rs). `?ui=desktop` says no and `?ui=mobile` says yes, for testing.
const forced = new URLSearchParams(location.search).get("ui");
const coarse = window.matchMedia("(pointer: coarse)").matches;
const small = Math.min(screen.width, screen.height) <= 900;
const touchable = "ontouchstart" in window || navigator.maxTouchPoints > 0;
globalThis.flareMobile =
  forced === "mobile"
    ? true
    : forced === "desktop"
      ? false
      : coarse && touchable && small;
if (globalThis.flareMobile) {
  const viewport = document.querySelector('meta[name="viewport"]');
  if (viewport) {
    viewport.content =
      "width=device-width, initial-scale=1, maximum-scale=1, user-scalable=no";
  }
}

async function loadFlareFile() {
  try {
    const [{ default: sqlite3InitModule }, zstd, { createFlareFile }] = await Promise.all([
      import("./vendor/sqlite/index.mjs"),
      import("./vendor/zstd/index.web.js"),
      import("./flare-file.js"),
    ]);
    const sqlite3 = await sqlite3InitModule();
    await zstd.init(new URL("./vendor/zstd/zstd.wasm", import.meta.url).href);
    globalThis.flareFile = createFlareFile({ sqlite3, zstd });
  } catch (error) {
    console.warn("Flare: .flare files are not available in this page:", error);
  }
}

// A blank canvas tells a person nothing, so any startup failure (a thrown error, a rejected
// promise, or a Rust panic, which only reaches the console) is shown as text over the page.
let shown = false;
function showError(reason) {
  if (shown) return;
  shown = true;
  const text = reason instanceof Error ? reason.message : String(reason);
  const panel = document.createElement("div");
  panel.id = "flare-error";
  panel.setAttribute("role", "alert");
  panel.style.cssText =
    "position:fixed;inset:0;z-index:10;display:flex;flex-direction:column;justify-content:center;" +
    "padding:32px;box-sizing:border-box;background:#1d2025;color:#e8eaed;font:15px/1.5 system-ui,sans-serif;";
  const title = document.createElement("h1");
  title.style.cssText = "font-size:20px;margin:0 0 12px";
  title.textContent = `Flare could not start: ${text.split("\n")[0].slice(0, 300)}`;
  const need = document.createElement("p");
  need.textContent =
    "Flare needs a current Chrome, Edge, Firefox or Safari with WebAssembly and WebGL 2 or WebGPU " +
    "turned on (check that hardware acceleration is enabled), and JavaScript allowed for this site.";
  panel.append(title, need);
  document.body.append(panel);
}
window.addEventListener("error", (e) => showError(e.error ?? e.message));
window.addEventListener("unhandledrejection", (e) => showError(e.reason));
const consoleError = console.error;
console.error = (...args) => {
  consoleError.apply(console, args);
  if (typeof args[0] === "string" && args[0].startsWith("panicked at")) showError(args[0]);
};

loadFlareFile()
  .then(() => init())
  .then(() => {
    // A pen's pressure reaches the Brush through the canvas's pointer events (pen-pressure.js).
    installPenPressure(document.getElementById("flare-canvas"), flarePenReport);
    document.documentElement.dataset.flare = "started";
  })
  .catch(showError);
