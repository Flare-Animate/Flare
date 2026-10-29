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

## What is not

- **`flare-platform`** is the desktop's operating system: directories, the sound card, native
  dialogs. A page has none of these, so the web app does not use it.
- **The `.flare` container** is SQLite, which has no build for the browser here. `flare-storage`
  compiles there but `save` and `load` return `StorageError::Unsupported`, so a page edits an
  untitled document in memory. Opening and saving need a browser path: a file chooser and a
  download of the XFL export.
- **Threads.** The runtime stays in its inline mode, which runs jobs on the calling thread. Work
  that is slow on a desktop is slow on the page's one thread until a web worker is added.
- **Sound** has no output yet: the events a workspace produces are dropped.
- **Pen input.** `flare-input` defines the samples; the page needs to feed them from pointer
  events (pressure is available there).

The first two points are the real work. They are listed here so each gap is a decision, not a
surprise.
