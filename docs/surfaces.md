# Surfaces: what each front end can do

Flare reaches people four ways: the **desktop** app (`flare-desktop`), the **browser** build
(`flare-web`), the **command line** (`flare-cli`) and the **MCP** server for agents (`flare-mcp`).
[AGENTS.md](../AGENTS.md) says a feature belongs on all four, as far as each can have it, and that
a gap is written down here with its reason. Every row below marked **no** is a debt.

`yes` works and is tested. `no` is not built on that surface. `n/a` the surface cannot have it.

First audit, from the code, 2026-10-31.

| Feature | Desktop | Browser | CLI | MCP | Why a gap, and what closes it |
|---|---|---|---|---|---|
| Open and save `.flare` | yes | yes | yes | yes | The browser brings SQLite and zstd as WebAssembly (`@sqlite.org/sqlite-wasm`, `@bokuweb/zstd-wasm`) and `flare-storage`'s own checks run in the page. It is a download and a file picker there, not a path. |
| Import XFL and `.fla` | yes | yes | yes | **no** | An MCP tool over `flare-import`. |
| Export XFL and `.fla` | yes (File > Export) | yes (as a download) | yes | **no** | An MCP tool over `flare-export`. What was not carried over is listed by the CLI and not yet by the two windows. |
| Export PNG | yes | yes | yes | **no** | `evaluate_frame` returns the frame's contents, not a picture; a PNG tool over `flare-runtime`. |
| Export GIF | yes | yes | **no** | **no** | A `flare-cli` subcommand and an MCP tool over `flare_runtime::gif_bytes`. |
| Export MP4 | yes | **no** | **no** | **no** | `flare-video` is native only today; the browser needs the encoders on wasm, the CLI and MCP only need to call it. |
| Publish SWF and HTML5 | yes | yes | yes | **no** | An MCP tool over `flare-publish`. |
| Test Movie in Ruffle | yes (in a window of the app) | yes (Ruffle in the page) | yes | **no** | The CLI and desktop start a Ruffle on the machine; the page runs Ruffle's own web build, fetched on first use. An MCP tool needs a Ruffle on the machine, as the CLI does. |
| Sound playback | yes | **no** | n/a | n/a | No browser sound output yet. |
| Frames, layers, folders, masks, guides | yes | yes | partly | partly | The CLI and MCP cover frames and layers; folders, masks and guides are not exposed. |
| Symbol kinds and a graphic's looping options | yes | yes | **no** | **no** | The CLI and MCP make Graphic symbols only; they need the kind and the playback settings as options. |
| Language and theme | yes | yes | n/a | n/a | |
| Share from the Publish popup | yes (Twitter only) | **no** | n/a | n/a | The browser's own `window.open` is wired, but the popup is a desktop dialog today. |
