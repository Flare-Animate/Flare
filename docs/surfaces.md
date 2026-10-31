# Surfaces: what each front end can do

Flare reaches people four ways: the **desktop** app (`flare-desktop`), the **browser** build
(`flare-web`), the **command line** (`flare-cli`) and the **MCP** server for agents (`flare-mcp`).
[AGENTS.md](../AGENTS.md) says a feature belongs on all four, as far as each can have it, and that
a gap is written down here with its reason. Every row below marked **no** is a debt.

`yes` works and is tested. `no` is not built on that surface. `n/a` the surface cannot have it.

First audit, from the code, 2026-10-31.

| Feature | Desktop | Browser | CLI | MCP | Why a gap, and what closes it |
|---|---|---|---|---|---|
| Open and save `.flare` | yes | **no** | yes | yes | The browser has no SQLite or zstd build. Closed by a JavaScript SQLite and zstd behind the same records, or by a pure-Rust container. |
| Import XFL and `.fla` | yes | yes | yes | **no** | An MCP tool over `flare-import`. |
| Export XFL and `.fla` | yes | yes (as a download) | yes | **no** | An MCP tool over `flare-export`. |
| Export PNG | yes | yes | yes | **no** | `evaluate_frame` returns the frame's contents, not a picture; a PNG tool over `flare-runtime`. |
| Export GIF | yes | yes | **no** | **no** | A `flare-cli` subcommand and an MCP tool over `flare_runtime::gif_bytes`. |
| Export MP4 | yes | **no** | **no** | **no** | `flare-video` is native only today; the browser needs the encoders on wasm, the CLI and MCP only need to call it. |
| Publish SWF and HTML5 | yes | yes | yes | **no** | An MCP tool over `flare-publish`. |
| Test Movie in Ruffle | yes | **no** | yes | **no** | Both need Ruffle on the surface; the browser can run Ruffle's own web build. |
| Sound playback | yes | **no** | n/a | n/a | No browser sound output yet. |
| Frames, layers, folders, masks, guides | yes | yes | partly | partly | The CLI and MCP cover frames and layers; folders, masks and guides are not exposed. |
| Symbol kinds and a graphic's looping options | yes | yes | **no** | **no** | The CLI and MCP make Graphic symbols only; they need the kind and the playback settings as options. |
| Language and theme | yes | yes | n/a | n/a | |
| Share from the Publish popup | yes (Twitter only) | **no** | n/a | n/a | The browser's own `window.open` is wired, but the popup is a desktop dialog today. |
