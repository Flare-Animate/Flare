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

- **Files.** There is no file system, so Open reads a `.flare` or a `.fla` through the browser's
  file picker (`flare-import` can read a `.fla` from memory), and Save, Export Image, Export
  Flash Document and Publish offer a download (`flare-export` writes a `.fla` to memory;
  `flare-runtime` and `flare-publish` return bytes). A `.flare` is the same file the desktop app
  writes: the page brings SQLite and zstd as WebAssembly (`@sqlite.org/sqlite-wasm` and
  `@bokuweb/zstd-wasm` from npm, copied into the served folder by `scripts/vendor.mjs`, no
  bundler) and registers them as `globalThis.flareFile` (`src/lib/flare-file.js`); the Rust side
  (`flare_file.rs`) turns them into rows for `flare-storage`'s `to_parts` and `from_parts`, which
  do every check the desktop does. A project opened that way has no path, so Save downloads.
- **Test Movie** publishes the movie as a SWF and plays it in Ruffle, Ruffle's own WebAssembly build
  (`@ruffle-rs/ruffle` from npm, exact version, vendored with its licence files by
  `scripts/vendor.mjs`). `src/lib/flare-ruffle.js` shows it in an overlay (Close or Escape to leave)
  and fetches Ruffle only on the first play; `ruffle.rs` hands it the bytes and turns a failure into
  a message. A headless Chrome test loads a movie we published and reads back what Ruffle reports
  it loaded; sound and how it looks on screen have not been checked by a person.
- **Sound** plays through Web Audio, and File > Import Sound uses the browser's file picker. A
  browser may want a click on the page before it makes a sound. Dragging the playhead plays the
  stretch of sound under it, as on the desktop; each sound is copied into the browser once and
  kept for the next play, up to 16 million samples in all.
- **Pen pressure** comes from pointer events (`src/lib/pen-pressure.js`) and reaches the workspace
  through `flare-input`'s `PenFeed`. A node test checks it; a real pen has not.
- **Preferences** live in local storage instead of `flare-platform`'s files.
- **`flare-platform`** is the desktop's operating system (directories, the sound card, native
  dialogs), so the web app does not use it.

## What is not there yet

- **Saving to a path.** A page has no path: a project is opened from the file picker and saved by
  download, and autosave and crash recovery (which need a folder) are not there.
- **Threads.** The runtime stays in its inline mode, which runs jobs on the calling thread. Work
  that is slow on a desktop is slow on the page's one thread until a web worker is added.
- The page is checked by compiling for `wasm32-unknown-unknown`, by tests of its JavaScript under
  node and by the headless Chrome test of Test Movie above. The rest of the interface has not
  been exercised in a browser from here.
