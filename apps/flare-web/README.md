# flare-web

Flare in a browser tab: the same `flare-animate` workspace as the desktop app, on an eframe canvas.
See [docs/WEB.md](../../docs/WEB.md) for what is shared with the desktop and what is missing.

```
apps/flare-web/
  src/flare-web/   the Rust crate (lib.rs, canvas.rs, app.rs, files.rs, preferences.rs)
  src/lib/main.js  loads the WebAssembly module and starts it
  public/          index.html
  scripts/build.sh builds everything into dist/
```

Build (needs the `wasm32-unknown-unknown` target and the `wasm-bindgen` command-line tool), then
serve `apps/flare-web/dist` with any static file server:

```
apps/flare-web/scripts/build.sh
```

Open reads a `.flare` or a `.fla` with the browser's file picker, and Import a `.fla`. Save downloads
the project as a `.flare`, the file the desktop app writes: the page loads SQLite and zstd as
WebAssembly (from npm, see `package.json` and `scripts/vendor.mjs`) and the checks on what a file
contains are `flare-storage`'s own. File > Export > Flash Document downloads a `.fla`. Export Image
and Publish download their files. Preferences are kept in local storage.

A pen's pressure comes from the page's pointer events (`src/lib/pen-pressure.js`, which `main.js`
installs after the module starts). `node scripts/test_pen_pressure.mjs` checks the shim with a stand-in canvas.

## Smoke test

`node tests/web_smoke.mjs <url> [shot.png]` loads a built site in headless Chromium (Playwright) and fails on a page error, a Rust panic, a failed request, the "Flare could not start" panel, or a blank stage. `.github/workflows/pages.yml` runs it against `dist` before anything deploys.
