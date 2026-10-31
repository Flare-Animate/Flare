// Loads the WebAssembly module built by scripts/build.sh. The module starts itself on the
// canvas with id "flare-canvas".
//
// First it loads what a page lacks for .flare files, SQLite and zstd as WebAssembly (see
// flare-file.js), and registers them as `globalThis.flareFile` for the Rust side. If that fails the
// editor still starts; opening and saving a .flare then say why they cannot.
import init from "./pkg/flare_web.js";

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

loadFlareFile()
  .then(() => init())
  .catch((error) => {
    document.body.textContent = `Flare could not start: ${error}`;
  });
