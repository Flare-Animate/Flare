# Surfaces: what each front end can do

Flare reaches people four ways: the **desktop** app (`flare-desktop`), the **browser** build
(`flare-web`), the **command line** (`flare`) and the **MCP** server for agents (`flare-mcp`).
[AGENTS.md](../AGENTS.md) says a feature belongs on all four, as far as each can have it, and that
a gap is written down here with its reason. Every row below marked **no** is a debt.

`yes` works and is tested. `no` is not built on that surface. `n/a` the surface cannot have it.

First audit, from the code, 2026-10-31.

| Feature | Desktop | Browser | CLI | MCP | Why a gap, and what closes it |
|---|---|---|---|---|---|
| Open and save `.flare` | yes | yes | yes | yes | The browser brings SQLite and zstd as WebAssembly (`@sqlite.org/sqlite-wasm`, `@bokuweb/zstd-wasm`) and `flare-storage`'s own checks run in the page. It is a download and a file picker there, not a path. |
| Import XFL and `.fla` | yes | yes | yes | yes | The MCP tool reads only from its export directory and writes a new `.flare` beside it; the open project is not replaced. |
| Export XFL and `.fla` | yes (File > Export) | yes (as a download) | yes | yes | What was not carried over is listed by the CLI and the MCP tool and not yet by the two windows. |
| Export PNG | yes | yes | yes | yes | |
| Export GIF | yes | yes | yes | yes | |
| Export MP4 | yes | **no** | yes (`flare export --format mp4`) | yes (`export_mp4`) | `flare-video` is native only today; the browser needs the H.264 and AAC encoders on wasm (the thing to check first). |
| Publish SWF and HTML5 | yes | yes | yes | yes | |
| Test Movie in Ruffle | yes (in a window of the app) | yes (Ruffle in the page) | yes | **no** | The CLI and desktop start a Ruffle on the machine; the page runs Ruffle's own web build, fetched on first use. An MCP tool needs a Ruffle on the machine, as the CLI does. |
| Open the desktop app on projects | yes (it is the app) | n/a | yes (`flare open`) | n/a | The command line only finds `flare-desktop` (next to itself, on PATH, or `FLARE_DESKTOP`) and starts it on the files; a browser tab and an MCP server have no window to open. |
| Sound playback | yes | **no** | n/a | n/a | No browser sound output yet. |
| Frames, layers, folders, masks, guides | yes | yes | partly | partly | The CLI and MCP cover frames and layers; folders, masks and guides are not exposed. |
| Symbol kinds and a graphic's looping options | yes | yes | **no** | **no** | The CLI and MCP make Graphic symbols only; they need the kind and the playback settings as options. |
| Camera (a camera layer with keyframes and motion tweens) | yes (Camera tool, Properties, preview) | yes (the same workspace) | yes (`flare camera`) | yes (`set_camera`) | The XFL export leaves the camera out with a warning, and nothing reads it back. The editor part is tested by driving the workspace, not by a person. |
| Language and theme | yes | yes | n/a | n/a | |
| Share from the Publish popup | yes (Twitter only) | **no** | n/a | n/a | The browser's own `window.open` is wired, but the popup is a desktop dialog today. |
