# The web build

`apps/flare-web` runs the same `flare-animate` workspace as the desktop app, drawn by eframe's web
backend on a canvas. It exists to keep the split honest: whatever compiles for
`wasm32-unknown-unknown` is shared code, and whatever does not belongs to one platform.

## What is shared

Everything from the document up compiles for the browser and is used unchanged: the foundation
(`flare-core`, `flare-document`, `flare-eval`, `flare-input`, `flare-text`, `flare-audio`), editing (`flare-commands`,
`flare-history`, `flare-editor`, `flare-tools`), the XFL and SWF formats (`flare-import`,
`flare-export`, `flare-publish`), text and sound (`flare-text`, `flare-audio`), rendering, `flare-runtime`,
`flare-ui` and `flare-animate`. Check with `cargo check --target wasm32-unknown-unknown -p <crate>`.

## What the page does differently

`apps/flare-web` is laid out the way PaintCraft's web app is: the Rust crate in `src/flare-web/`, the
JavaScript that loads it in `src/lib/`, a static page in `public/`, and a build script.

- **Files.** There is no file system, so Open and Import read a `.fla` through the browser's file
  picker (`flare-import` can read one from memory), and Save, Export Image and Publish SWF offer a
  download (`flare-export` writes a `.fla` to memory; `flare-runtime` and `flare-publish` return
  bytes).
- **Preferences** live in local storage instead of `flare-platform`'s files.
- **`flare-platform`** is the desktop's operating system (directories, the sound card, native
  dialogs), so the web app does not use it.

## What is not there yet

- **The `.flare` container** is SQLite, which has no build for the browser here. `flare-storage`
  compiles there but `save` and `load` return `StorageError::Unsupported`. Pages keep a project as
  a `.fla` instead, which holds the same artwork but not everything Flare can (for example
  sounds, masks and gradients are reported as not carried over).
- **Threads.** The runtime stays in its inline mode, which runs jobs on the calling thread. Work
  that is slow on a desktop is slow on the page's one thread until a web worker is added.
- **Sound** has no output yet (events are dropped) and sounds cannot be imported.
- **Pen input.** `flare-input` defines the samples; the page needs to feed them from pointer
  events (pressure is available there).
- The page has not been run in a browser from here; it is checked by compiling for
  `wasm32-unknown-unknown`.
