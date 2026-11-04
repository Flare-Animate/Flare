# Surfaces: what each front end can do

Flare reaches people five ways: the **desktop** app (`flare-desktop`), the **browser** build
(`flare-web`), the **command line** (`flare`) and the **MCP** server for agents (`flare-mcp`),
and as an **Android** app (`flare-android`).
[AGENTS.md](../AGENTS.md) says a feature belongs on all of them, as far as each can have it, and
that a gap is written down here with its reason. Every row below marked **no** is a debt.

`yes` works and is tested. `no` is not built on that surface. `n/a` the surface cannot have it.

First audit, from the code, 2026-10-31.

| Feature | Desktop | Browser | CLI | MCP | Why a gap, and what closes it |
|---|---|---|---|---|---|
| Open and save `.flare` | yes | yes | yes | yes | The browser brings SQLite and zstd as WebAssembly (`@sqlite.org/sqlite-wasm`, `@bokuweb/zstd-wasm`) and `flare-storage`'s own checks run in the page. It is a download and a file picker there, not a path. |
| Text objects: add, edit, restyle, break apart, list fonts | yes (typed on the stage; Properties has Character) | yes | yes (`add-text`, `set-text`, `break-text`, `fonts`) | yes (`add_text`, `set_text`, `break_text`, `list_fonts`) | A browser has no installed fonts, so there text is set in the one font built into Flare (Source Sans 3), which every surface has. |
| Open XFL, `.fla` (modern and legacy binary) and `.swf` (File > Open; `flare import` and `import_project` make a project from them) | yes | yes | yes | yes | The MCP tool reads only from its export directory and writes a new `.flare` beside it; the open project is not replaced. A legacy binary `.fla` imports as a skeleton (docs/ARCHITECTURE.md), the same on every surface because they share `import_path`/`import_bytes`. |
| Save and Save As to `.fla`, `.xfl` and `.flare`; Save writes back to the file a `.fla`/`.xfl`/`.flare` was opened from; the report of what was rebuilt or lost | yes (File > Save, Save As, the close prompt; Output panel) | yes, as a download with a name prompt (`.fla` default, `.flare` a choice; `.xfl` is saved as `.fla`) | yes (every project command takes a `.fla`/`.xfl`/`.flare` and edits it in place; `flare convert` is Save As; `new x.fla`) | yes (`open` takes the same; `save` writes back; `save_as` writes a new file in the export directory) | The browser has no file system, so Save is always a download and the page cannot write back to a `.fla` it opened; the prompt offers the last name again. An XFL folder cannot be downloaded. The CLI and MCP refuse in-place edits of a file Flare cannot write whole (a movie, a pre-CS5 `.fla`, one over the size cap) and say to convert it. The browser's name prompt is a `window.prompt`. |
| Export XFL and `.fla` | yes (File > Export) | yes (as a download) | yes | yes | What was not carried over is listed by the CLI and the MCP tool; the desktop app shows it in the Output panel after a Save, not yet after File > Export. |
| Open and save an imported file unchanged (the source is kept; unchanged exports are the original; an edit keeps every untouched file) | yes (it is the same library call) | yes (the same, from the bytes dropped or picked) | yes (`flare import`, then `export-xfl` or `publish-swf`; both print `original_kept`, `lossless` and `original`) | yes (`import_project`, `export_xfl`, `publish_swf` report the same) | The desktop app and the browser tell the person after a Save whether the file was the original or rebuilt (File > Export Flash Document and Publish still do not; `Published::original` is shown only by the CLI and the MCP tool). A browser tab holds the source in memory and in the project it downloads, so a very large `.fla` is bounded by the page's memory as well as the 256 MiB cap. Closing the rest means showing the report after an export too. |
| Export PNG | yes | yes | yes | yes | |
| Export GIF | yes | yes | yes | yes | |
| Export MP4 | yes | **no** | yes (`flare export --format mp4`) | yes (`export_mp4`) | `flare-video` is native only today; the browser needs the H.264 and AAC encoders on wasm (the thing to check first). |
| Publish SWF and HTML5 | yes | yes | yes | yes | |
| Test Movie in Ruffle | yes (in a window of the app) | yes (Ruffle in the page) | yes | **no** | The CLI and desktop start a Ruffle on the machine; the page runs Ruffle's own web build, fetched on first use. An MCP tool needs a Ruffle on the machine, as the CLI does. |
| Open the desktop app on projects | yes (it is the app) | n/a | yes (`flare open`) | n/a | The command line only finds `flare-desktop` (next to itself, on PATH, or `FLARE_DESKTOP`) and starts it on the files; a browser tab and an MCP server have no window to open. |
| Rename the scene | yes (double-click its name above the stage) | yes (the same workspace) | yes (`flare rename-scene`) | yes (`rename_scene`) | The name is trimmed and 1 to 256 characters; the command line and MCP rename the first scene, which is the only one the editor shows. |
| Add, Delete and Convert Anchor Point | yes (the Pen's cell) | yes (the same workspace) | **no** | **no** | The command line and MCP server have no path editing at all yet (they draw rectangles); `flare_tools::ops::{add_anchor, delete_anchor, convert_anchor}` are the library the commands would call. |
| Cut, Select All and Reverse Frames, and the right-click menu on frames | yes (Edit > Timeline, Modify > Timeline, a right click on a frame) | yes (the same workspace) | partly (`remove-frames` and `reverse-frames`, with `--at` and `--to`) | partly (`remove_frames` and `reverse_frames`) | Cut and Select All Frames are about a clipboard and a selection, which the command line and MCP server do not have; removing the range and reversing it are what they offer. Reverse needs whole keyframes, and moves a tween with the pair it joined. |
| Sound playback | yes | **no** | n/a | n/a | No browser sound output yet. |
| Drawing into the active layer: a shape joins the layer's keyframe as one more element, picked and edited one at a time | yes (every drawing tool; Select, Lasso, Select All) | yes (the same workspace) | partly (`flare draw-rect --layer` joins the keyframe at `--start`; `flare info` lists each layer's `elements`) | partly (`draw_rect` with `layer`, `start`; `get_info` lists `elements`) | The command line and MCP server draw rectangles only (they have no pen, brush or text drawing), and have no selection, so moving, deleting, grouping or reordering one element is a desktop and browser feature; `flare_tools::ops::{translate_many, delete_many, group, arrange_elements}` are the library the commands would call. Merge Drawing is not built on any surface; the desktop shows its switch dimmed. |
| Frames, layers, folders, masks, guides | yes | yes | partly | partly | The CLI and MCP cover frames and layers; folders, masks and guides are not exposed. |
| Symbol kinds and a graphic's looping options | yes | yes | **no** | **no** | The CLI and MCP make Graphic symbols only; they need the kind and the playback settings as options. |
| Camera (a camera layer with keyframes and motion tweens) | yes (Camera tool, Properties, preview) | yes (the same workspace) | yes (`flare camera`) | yes (`set_camera`) | The XFL export leaves the camera out with a warning, and nothing reads it back. The editor part is tested by driving the workspace, not by a person. |
| Language and theme | yes | yes | n/a | n/a | |
| Pen pressure on the Brush | partly | **no** | n/a | n/a | The desktop works on Linux and Windows and not on macOS. It reads it from winit (Windows) or from the X server (Linux, `flare-tablet`). winit drops a tablet's pressure on macOS, so a pen paints like a mouse there until `flare-tablet` has an AppKit module. The browser build does not read `PointerEvent.pressure` yet; a command line and an MCP server have no pen. Native Wayland needs its tablet protocol. |
| Share from the Publish popup | yes (Twitter only) | **no** | n/a | n/a | The browser's own `window.open` is wired, but the popup is a desktop dialog today. |

## Android

The first build (2026-11-04) is [`flare-android`](../apps/flare-android/README.md): one
`NativeActivity` around the desktop's own shell (`flare-desktop`'s library, through
`run_on_activity`), so the workspace there is exactly the one the desktop's tests exercise —

- **What works:** the whole editor workspace (drawing, tools, layers, frames, symbols, camera,
    scene naming, themes and languages), the dock layout, and the saved layout, preferences and
    recovery files, under the app's own data directory (see `flare-platform`'s `set_app_root`).
- **What does not, and why:** a phone's storage is reachable only through the system's picker,
    which is not wired (see `flare-platform`'s `dialogs.rs` phone backend, where a picker
    answers as cancelled) — so every row above that opens, saves or exports a file to or from a
    place is **no** here; sound is not demonstrated on a device yet; Test Movie is off (the
    desktop feature that runs Ruffle inside the process); a pen's touch pressure is not wired to
    the Brush. The library is built by the Android workflow (the `.so` alone proves the tree
    compiles for the target); an APK comes from `apps/flare-android`'s manifest through
    `cargo apk`.

Each of those gaps is a debt: the next step for the phone is the Storage Access Framework
picker, which turns most of the table's **no** into what the rest of the table already has.

| Tween curves, easing and in-betweens of stroke points (`flare-tween`) | **no** | **no** | **no** | **no** | The library is built and tested (`crates/foundation/flare-tween`). Not yet reached from any surface: no command sets a key's interpolation or handles, no Motion Editor draws a curve, and the command line and MCP server do not evaluate a curve or list the easings. Each would call `flare_tween::{ease, Curve, interpolate_sequence}`. |
