# flare-web

The Animate workspace in a browser tab. See [docs/web.md](../../docs/web.md) for what is shared
with the desktop app and what is not there yet.

Build the module and its JavaScript glue (needs the `wasm32-unknown-unknown` target and the
`wasm-bindgen` command-line tool):

```
cargo build -p flare-web --target wasm32-unknown-unknown --release
wasm-bindgen --target web --out-dir apps/flare-web/pkg \
  target/wasm32-unknown-unknown/release/flare_web.wasm
```

Then serve `apps/flare-web` with any static file server and open `index.html`.
