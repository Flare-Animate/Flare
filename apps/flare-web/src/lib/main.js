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
