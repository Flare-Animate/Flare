# flare-web

Flare in a browser tab: the same `flare-animate` workspace as the desktop app, on an eframe canvas.
See [docs/web.md](../../docs/web.md) for what is shared with the desktop and what is missing.

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

Open and Import read a `.fla` with the browser's file picker. Save downloads the project as a `.fla`
(XFL), because the `.flare` container is SQLite, which a page does not have. Export Image and
Publish download their files. Preferences are kept in local storage.
