# Architecture

## Crates

Crates sit in layers under `crates/`, and dependencies point down this list: a crate depends
only on layers above it in the table (and on earlier rows of its own layer), never below.
Executables are in `apps/`. The folders mean:

- **foundation**: the model and the pure functions over it, including text shaping and sound decoding.
- **editing**: how a document is changed and how the tools turn gestures into changes.
- **io**: file formats, in and out (the project container, XFL, SWF).
- **adapters**: integrations with the Flash ecosystem: recognising its files (`flare-flash`) and running its player (`flare-ruffle`).
- **services**: rendering, background work and the operating system.
- **presentation**: egui widgets and the workspaces built from them.

| Layer | Crate | Responsibility |
|---|---|---|
| foundation | `flare-core` | Typed IDs (UUIDv7), `FrameRate`, `DocumentPoint`, `Transform2D`, `Rgba8`. No I/O. |
| foundation | `flare-document` | The document model and its validation. No GUI, GPU, database or async code. |
| foundation | `flare-eval` | Pure frame evaluation: document + scene + frame to placed instances and drawn shapes. |
| foundation | `flare-text` | Text to vector outlines: shaping with `parley`, glyph outlines with `skrifa`. Holds the font database, so it is created off the UI thread. |
| foundation | `flare-audio` | Sound: decoding with `symphonia` (WAV, MP3, Ogg Vorbis, FLAC, with size limits), a pure stereo mixer, and the events a front end sends to the sound card. |
| foundation | `flare-i18n` | The words of the interface: Fluent files per language and section under `localizations/`, compiled in, with English as the fallback; a build script finds the files, and `has_key` lets a `tr!` check its key while the program is built. |
| foundation | `flare-input` | What a pointing device reports: samples with position, pressure and device kind, and the pressure-to-width curve. A mouse is just full pressure. `PenFeed` carries a native source's pressure to the stage. |
| editing | `flare-commands` | The vocabulary of edits (`Command`) and applying a batch as one validated step. |
| editing | `flare-history` | `EditorSession`: undo and redo over validated edits, snapshots, the saved mark. |
| editing | `flare-editor` | Operations built on commands: frame and keyframe operations, in-place symbol editing support. |
| editing | `flare-tools` | What the tools compute with no window, a folder for each tool with its code and tests: the Pen's paths and the shape a finished path makes, the Pencil's smoothing, the Brush's pressure-sensitive outlines, the Free Transform handles, selection and lasso picking, the anchor point tools, the Ruler and the Mirror, and the operations of the Paint Bucket and Ink Bottle, the Eraser, Text, grouping, the clipboard, alignment, tweens and Graphic playback. |
| io | `flare-storage` | The `.flare` SQLite container with compressed CBOR records: save, load, revalidation, crash recovery files. |
| io | `flare-import` | Import: XFL directories, zipped modern `.fla` files and Flash movies (`.swf`) into a Flare document, with a diagnostic for everything it cannot carry over. |
| io | `flare-export` | XFL export: a Flare document as an XFL folder or a zipped `.fla`, with a diagnostic for everything the format cannot hold exactly. |
| io | `flare-publish` | SWF publishing: a document as a SWF with its symbols kept as sprites and its tweens baked, encoded with Ruffle's `swf` crate; a file written whole or not at all. |
| adapters | `flare-flash` | Container signature inspection only (SWF/ZIP/CFB prefixes). Not a parser. |
| adapters | `flare-ruffle` | Ruffle, the Flash player: Test Movie publishes a SWF and plays it in an external Ruffle process, kept at arm's length (private directory, no shell, one argument). |
| services | `flare-render` | CPU rasterization (vello_cpu), PNG output, hit testing. |
| services | `flare-runtime` | Work that must stay off the UI thread: workers for drawing, files, sound and text, with latest-wins and an inline mode for tests; image export. No UI types. |
| services | `flare-video` | The movie as an MP4 with its sound, made in pure Rust: H.264 from `rusty_h264`, AAC-LC from OxideAV's `oxideav-aac`, the container from `muxide`. Frames are drawn, encoded and written one at a time, the cued sounds mixed and encoded beside them, and the file appears only when whole and never over another. Desktop only: the web build does not carry it. |
| services | `flare-recent` | What a list of recent projects knows: each file's time and size read off the UI thread, its kind, the search over names and folders, and the sorting. No UI types. |
| services | `flare-tablet` | A tablet pen's pressure from the operating system where the window system does not pass it on: one gated module per system, each writing into `flare-input`'s `PenFeed`. Today the X server's XInput2 raw events (Unix), with the check for a plugged-in pen that picks XWayland; macOS and Windows modules will need `unsafe` and so cannot live in `flare-input`. |
| services | `flare-platform` | What a desktop needs from the operating system: per-user directories (the `directories` crate), the sound card, native dialogs, recovery prompts, and the threads for publishing, exporting and test movies. No UI types. |
| presentation | `flare-ui` | Shared egui building blocks: palette and type, the icon set, small widgets, the stage's pan and zoom math. |
| presentation | `flare-dock` | The dockable panel layout: which panels exist (`Pane`), how they are arranged by default, and reading and writing a saved arrangement as untrusted input (size-capped, every panel exactly once, else the default). |
| presentation | `flare-menubar` | The menu bar with no window in it: every command (`MenuAction`), the menus and their order, shortcuts, icons and what each item is called for the state of what it toggles (`LabelState`). Shared by the native macOS menu and the in-window one. |
| presentation | `flare-animate` | The Animate-style workspace built from them: stage, timeline, panels, and, in `workspace/tools/`, what the workspace does for each tool (wiring pointer events and settings to what `flare-tools` computes). No window or GPU code. |
| apps | `flare-cli` | Headless project editing and export. |
| apps | `flare-desktop` | The desktop window (winit, egui-wgpu, the native menu), all in `ui/`; everything else it needs is `flare-platform`. |
| apps | `flare-web` | The same workspace in a browser tab (eframe on a canvas); compiles for `wasm32-unknown-unknown` only, see [WEB.md](WEB.md). |
| apps | `flare-mcp` | An MCP server over stdio for one open project, built on `rmcp`: agent tools over the same edit commands, with permissions and revision checks. |

The CLI and the desktop app edit documents through the same `flare-commands` commands and
render through the same `flare-eval` and `flare-render`. That is deliberate: there is one
set of semantics, and the UI is not allowed to have private ways to mutate a document.

## Document model

A `Document` has a frame rate, a `Stage` (size 1 to 16384 per side, background), ordered
scenes and a library of symbols. A scene or symbol owns a `Timeline`: layers ordered top
to bottom, each with a name, visible and locked flags, an optional folder it sits in, and non-overlapping `FrameSpan`s. A folder is a layer with no frames of its own that sits above the layers it holds (so a parent is always an earlier layer and folders cannot loop); hiding or locking it hides or locks them. A motion guide is a layer that is never drawn: while a layer below it tweens an instance, the instance rides along the guide's first shape from its start to its end (by length) instead of going straight to the next position. A scene may have one camera layer, also never drawn, whose spans hold `Camera` keyframes (the point of the scene at the middle of the picture, a zoom from 0.01 to 100, and a rotation); a motion tween between two keyframes moves the camera with the same Ease as an instance's. `flare_eval::evaluate_frame` applies the camera's view to the whole scene, so every render, export and preview shows the camera's picture, and `evaluate_frame_without_camera` gives the scene as it is edited. A symbol cannot hold a camera. Hidden, absent at a frame or not yet keyed, the camera is the scene as drawn.

A span is a held exposure: start frame, positive duration, `Content` (`Empty`, `Shape` or
`Instance`) and an optional linear `Tween`. An empty keyframe is a span with `Empty`
content, which is not the same as having no span. A held exposure is one span, never many
cloned keyframes.

A `Shape` is a validated path (move/line/cubic/close, segment count capped), a fill, an
optional two-color linear `Gradient` (painted instead of the solid fill, in the shape's own
coordinates, so it follows transforms) and an optional `Stroke` (width, color, and how its ends and corners look; round by default). An `Instance` references a symbol and carries a full
finite 2D affine matrix (shear and reflection are preserved, not decomposed away).
A symbol has a kind: Graphic, Movie Clip or Button. The kind is kept, edited in the Library, saved in the project and exchanged through XFL, and played as follows: a Graphic and a Movie Clip are parent-clocked, looping over their own length from the frame their instance starts on (a movie clip has no clock beyond that, and no scripts); a Button shows its first frame, the Up state, and its Over, Down and Hit states are not used. A Graphic instance also carries Animate's frame setting: Loop, Play Once (then holds the last frame) or Single Frame, and the first frame it starts on; they are saved in the project and exchanged through XFL (`loop`, `firstFrame`).

Invariants, enforced by constructors and by `Document::new`, which reports every
violation as a `Diagnostic`:

- IDs are unique per kind, and instances only reference existing symbols.
- Symbols cannot contain themselves, directly or indirectly.
- Spans on a layer never overlap; durations are non-zero; frame arithmetic cannot overflow.
- A span's `content` is its first element, and `extras` hold more (shapes and instances only, at most `MAX_ELEMENTS`, never with a tween), so a keyframe can hold what Animate's does; the project file writes `extras` only when there are some. `FrameSpan::with_elements`, `with_element`, `without_element`, `with_appended`, `with_element_moved`, `retimed` and `held` rebuild a span without ever losing an element. Everything reads every element: evaluation draws them bottom to top inside the layer (a tween applies to a single-element span only), `evaluate_level` lists them with their index, the SWF publisher gives each its own depth (a layer reserves one for its mask and one for each element of its fullest span, so depths do not move from frame to frame), the XFL writer puts them all in one `<DOMFrame>`, and the editor's frame operations (insert, remove, copy, paste, reverse, convert to keyframes) carry them all. [decisions/ELEMENTS.md](decisions/ELEMENTS.md) lists the stages.
- Coordinates and matrices are finite; stage and stroke sizes are bounded.

Nothing in the model derives `Deserialize`. Storage rebuilds documents through the same
validating constructors, so a file cannot smuggle in an invalid document.

## Editing

A selection names one element: `SpanRef` is the layer, the start frame of the span and the element's index in `FrameSpan::elements()` (0 for a span of one, so single-element behaviour is unchanged). A hit on the stage, Select All and a lasso each give one `SpanRef` per element, and the operations of `flare-tools` (move, transform, restyle, delete, copy, duplicate, convert to symbol, group, break apart, path edits, arrange) act on the elements named. An edit rebuilds the span the element is in with `Command::RemoveSpan` and `Command::InsertSpan`, so it is one undo step with no new command; for several elements of one span `each_element`, `translate_many`, `transform_many` and `delete_many` edit the span once or one after another on a working copy, never from the same stale span. `drawing::draw_shapes` decides where a new shape goes (the active layer's keyframe at the frame, a new keyframe, or a new layer in a document with none) and refuses a locked, hidden, folder, mask, guide or camera layer; the desktop app, `flare draw-rect` and the MCP `draw_rect` all call it.

`EditorSession` owns the current document. `apply` takes a batch of `Command`s and runs
them against a working copy; the session only changes if the whole batch succeeds, and a
batch is one undo step. History keeps shared whole-document snapshots (bounded, 1000 by
default). Dirty state compares the current snapshot with the saved one, so undoing back to
the saved content is clean again. `revision()` is a cache key, not a dirty flag.

Every edit rebuilds the document, so anything a command does not touch must be carried
through explicitly (stage, layer flags, tweens). Tests exist for each of these because
dropping them silently was a real hazard.

Gestures (drawing, moving) are tool state until released and commit as one undo step.
Escape, a tool switch or losing the gesture cancels without touching the document.

Frame operations (insert/remove frame, insert/clear keyframe, blank keyframe) are pure
functions in `flare_editor::frames` from a layer and frame to a command batch. Copies get
fresh shape and instance IDs.

### In-place symbol editing

Entering a symbol builds a scratch document whose single scene is that symbol's timeline,
with its own `EditorSession`, so every scene tool works unchanged inside a symbol. Leaving
sends one `MergeSymbolEdit` to the parent: it replaces the symbol's timeline and adds any
symbols created inside, as a single validated step. If the parent rejects the result, for
example because it would create a symbol cycle, you stay in the symbol and the reason is
shown. Saving applies pending symbol edits first.

## Evaluation and rendering

`evaluate_frame` is pure: no I/O, rendering or UI state. Instances are identified by their
instance path, so two uses of one symbol keep separate clocks and transforms. Hidden layers
are skipped. Linear tweens interpolate scale, rotation (shortest arc) and translation, and
return an error for shear or reflection rather than approximating. Depth (64) and node
count (100,000) are bounded.

`render` maps document units 1:1 to pixels from the top-left, draws fills then strokes in
paint order, and returns straight (non-premultiplied) RGBA8. Size is limited to 16384 per
side and 100 million pixels. Hit testing counts a fill (same non-zero rule) and a stroke (where it is drawn: with its
caps and joins, width scaled by the transform), so a line or an outline-only shape can be picked.
It returns candidates topmost first so callers can skip locked layers.

Onion skinning composites translucent copies of neighbouring frames (up to 8 before and
after, fainter with distance) beneath the current one using straight-alpha source-over
blending in `flare-render`. It is a display aid only: it is off while playing and never
affects exports.

## Storage

A `.flare` file is a SQLite database (`application_id` "FLAR", `user_version` 1) with a
`project` row (frame rate, stage) and an `entity` table with one row for each scene, symbol,
sound, source attachment and, for an imported file, its source (below). A row's payload is the record's fields as CBOR, compressed with zstd
(level 3, 256 KiB window, content size and checksum) when that is smaller, beside the record's
`codec`, its uncompressed `size` and the BLAKE3 `digest` of the stored bytes. A sound is one
record too: the length of its fields, the fields as CBOR, then the audio file as its own bytes,
not as text.

The database is arranged to cost little: 1 KiB pages and an `entity` table without a second
index (`WITHOUT ROWID`), so an empty project is about 2 KiB and a large one about 1% over its
compressed records. Measured on a generated animation of 6,000 shapes, this is 1.0 MB where the
JSON in text columns was 7.3 MB; with a megabyte of sound, 0.96 MB against 3.8 MB. A raw binary
container of our own with the same records came out within 1% to 4% of that, which did not
justify giving up SQLite's atomic writes and the ability to open a project in any SQLite tool.

The records are made and checked in code that touches neither SQLite nor a compression library
(`records.rs`: a document to rows and rows to a document, with the checks; `codec.rs`: CBOR, the
digest, and the zstd frame header read by code of ours). zstd and the database are supplied from
outside: the desktop app brings the `zstd` library and SQLite, and a browser build brings a
WebAssembly zstd and a WebAssembly SQLite and gets exactly the same checks. The frame header is
read here (declared size equal to the record's, window no bigger than the writer's, a checksum,
no dictionary) before any decompressor is asked, so a record cannot be a decompression bomb
whichever zstd is behind it. A test (`scripts/tests/test_web_wasm.py`) compiles the browser
build's bridge to WebAssembly, runs it under node with the two npm libraries, has it read a
project `flare-cli` made and write one back, and has the desktop's code read that.

- Save writes a complete staged database next to the target and renames it into place, so a
  failed save leaves the previous file intact. The directory is not fsynced after the rename.
- Load treats the file as untrusted: defensive SQLite settings, schema trust off, a version check
  that refuses other versions, the entity count limited, each record's digest checked before it is
  decoded, its stated size limited before anything is allocated (256 MiB for one record: far
  above what drawing produces, and a bound all the same) and the
  decompressor held to that
  size and to the window the writer uses (so a record cannot be a decompression bomb), CBOR
  nesting limited, and full revalidation through the document's constructors.
- An imported project can carry one `origin` record: the file it was imported from (a `.fla` or `.swf` as its bytes, an
  XFL folder as its files), with the fingerprint of the document the import made. It is laid out like a sound (the
  length of its fields, the fields as CBOR, then the bytes), capped at 256 MiB plus its fields and checked like every
  record (its size against the limit before anything is allocated, the stored digest, then the source's own digest
  after it is unframed, every size it states against the bytes that are there, names that stay inside the folder).
  Projects with no source have no such row and are no bigger than before; files from before the record still open;
  a build from before it refuses a file that has one. See [decisions/LOSSLESS.md](decisions/LOSSLESS.md).
- There is one version of the format. Files made before the records were compressed (JSON in
  text columns) are not read.

## Crash recovery

Autosave writes the main document to a separate recovery store (per-user app data,
`Flare/recovery`), never over the project file, so discarding changes really discards them.
A recovery file is a normal container named by document ID, so each document has one. It is
written only when there are unsaved changes and the interval (30 s by default) has passed,
and removed when the work is saved, undone back to clean, or deliberately discarded. On a
plain launch the newest recovery file is offered: recover, discard, or decide next time.
Recovered work counts as unsaved. An in-progress symbol edit is not included until it is
applied, and only the newest recovery is offered per launch.

## Automation (MCP)

`flare-mcp <project>` (an app, built on the `rmcp` SDK) serves the project to an AI agent over
stdio; stdout carries only protocol messages, which `rmcp` frames and validates. Tools are typed
(their argument schemas come from the parameter structs) and use the same commands, frame
operations and renderer as the UI, so validation, atomic batches and undo apply to agents
too. The server is read-only unless started with `--allow-write`, and nothing reaches disk
until the `save` tool. The exports (`export_png`, `export_gif`, `export_mp4`, `publish_swf`, `publish_html5`, `export_xfl`) and `import_project` are off
unless `--export-dir` is given; they write only a sanitized file name inside it, never overwrite, and read
only from it. Mutating tools accept `expected_revision` (from `get_info`)
and fail on a mismatch, so an agent cannot overwrite changes it has not seen. Tool failures
are results with `isError` set, so the agent can read and react; arguments of the wrong shape
are protocol errors from `rmcp`.

## The tools (`flare-tools`)

A tool is a folder: `pen/`, `pencil/`, `brush/`, `free_transform/`, `selection/`, `subselection/`,
`add_anchor/`, `delete_anchor/`, `convert_anchor/`, `ruler/`, `mirror/`, `paint/`, `eraser/`,
`text/`, `tween/`, `grouping/`, `clipboard/`, `arrange/` and `playback/`, each with its code and
its tests in files of its own. A tool depends on the shared `common` module (finding a span, bounds,
copies of what a span holds, path editing) and on nothing in another tool, so one can be read,
changed or removed alone; their tests may call other tools through `ops`. `ops` gathers the
operations a UI calls (`ops::translate`, `ops::restyle`, ...) under one name and holds none of
their code. What a tool computes belongs here; the workspace in `flare-animate` only turns pointer
events and settings into calls (the pen's `path_shape` builds the shape, `pen_tool.rs` wires the
mouse).

## Text

Text is a shape that carries its source (`flare_document::TextSource`): see
[decisions/TEXT.md](decisions/TEXT.md) for why and for the rules. `flare-text` sets a source
into outlines with `parley` and `skrifa` (`TextEngine::shape`) and lists the font families
(`TextEngine::families`); `flare-runtime` runs both on the text worker (`TextJob::Set` and
`TextJob::Families`) and exposes them to the command line and the MCP server
(`text_shape`, `font_families`). `flare-tools` has `replace_shape` and `break_text_apart`. The
file keeps the source in an optional `text` record on the shape, so files from before text could
be edited open as plain outlines. `flare add-text`, `set-text`, `break-text` and `fonts`, and the
MCP tools `add_text`, `set_text`, `break_text` and `list_fonts`, are the same operations; both
take font files the person brings (`--font-file`) next to the system's.

## Import (SWF)

`flare import <movie.swf> <out.flare>` (and File > Open, the browser's picker, and the MCP tool `import_project`) reads a Flash movie, found by what the file starts with (`FWS`, `CWS` or `ZWS`) rather than its name, using Ruffle's `swf` crate. The size a movie claims for itself once decompressed is checked before anything is allocated for it, and the counts of symbols, frames and keyframes are capped. Every shape character becomes a Graphic symbol and every sprite a Movie Clip symbol with a timeline of its own; the main timeline becomes one scene with a layer for each depth, the highest on top, and names given by Export Assets name the symbols. A SWF holds the position of each object on each frame, not tweens, so each frame an object changes on is a keyframe. The edges of a shape are turned into shapes by the same code as XFL's (`shapes_from`), so a stroke along a fill's boundary is its outline in both. Imported: stage size, frame rate and background; solid fills and strokes with their caps and joins; placements with their matrices. Reported and left out: gradients (a gradient takes the color of its first stop), picture fills, text, buttons, pictures, video, sound, scripts, color effects, filters, blend modes, masks and frame labels. The movie's bytes are kept with the document: publishing it as a SWF while unchanged writes the original file back byte for byte (`Published::original`), and an edited one is rebuilt. Verification is the movie `flare-publish` writes, imported back and drawn (`apps/flare-cli/tests/project/formats.rs`), and movies built tag by tag in `crates/io/flare-import/tests/swf.rs`, including every truncation and random damage.

## Import (legacy binary FLA)

A `.fla` from Flash MX 2004 to CS4 is not a zip but a Compound File Binary (OLE2) container, found by its
eight-byte signature. `flare-import`'s `legacy` module reads it; `import_path` and `import_bytes` (so the desktop
app, the browser build, `flare import` and `import_project`) use it. There is no published specification: what is
read was worked out from the community notes at github.com/Kreijstal/fla-decoder and then checked against the 159
genuine legacy files in the vendored Ruffle test suite (format versions 0x129, 0x138, 0x13f and 0x147). They are
small test movies, not real projects, and none is known to come from CS3 or CS4.

The container reader is our own, bounded and safe: file size, directory entries and stream sizes are capped, a
sector chain that visits a sector twice is an error, and a stream that claims more than the file holds is refused
before anything is allocated. Read: the stream names (numbered `Page N`/`Symbol N`/`Media N`, or stamped `P N t`/
`S N t`/`M N t`) and so the era and the number of scenes, symbols and media; the stage size, frame rate (a 16.16
fixed-point number) and background color from a fixed record in `Contents`, found by a marker that follows it in
every file checked; and the library table, the name and type (graphic, movie clip, button) of each symbol and the
name and class (bitmap, sound, video) of each media item. The import is a skeleton: the stage, one empty scene and
the symbols as empty Graphic, Movie Clip and Button symbols, with a warning that the rest is not read.

Not read: drawings, timelines and layers, text, tweens, the content of sounds and bitmaps, scripts, fonts, library
folders and scene names. Checked only on the vendored files: stage sizes 550x400 and 400x400, frame rates 5, 12 and
24, backgrounds white and black, so the color channel order and fractional frame rates are not confirmed. A
document whose settings record is not where those files have it is refused with what was found, not guessed.
`crates/io/flare-import/tests/legacy_corpus.rs` compares every file with a sibling `.swf` compiled from it;
`legacy_robust.rs` cuts a file at every length, damages it at random and builds cyclic and oversized containers.

## Import (XFL)

`flare import <source> <out.flare>` reads an XFL directory, an `.xfl` file or a folder's `DOMDocument.xml` (the folder is
used) or a zipped modern `.fla`, and writes a new project. It never overwrites, and the
result is an ordinary document. Input is untrusted: XML depth, element count and file sizes
are capped, DTDs are refused, file names cannot escape the document, and symlinks out of an
XFL folder are not followed. Legacy binary `.fla` files are read as a skeleton (next section).

Imported: stage size, frame rate (NTSC rates stay exact) and background; scenes; normal
layers with name, visibility and lock; keyframes and durations; symbol instances with their
matrix; solid fills and solid strokes of drawn shapes, including quadratic curves and
holes (edges are decoded and chained into contours per fill); the library, each symbol with its
Graphic, Movie Clip or Button type; layer folders (a layer in a folder points at it by index, and a hidden or locked
folder hides or locks what it holds); motion tweens between instances and shape tweens between shapes (with the ease, `acceleration` in XFL). A keyframe with several elements stays one span: the elements are kept together in the layer, in the order the file lists them (a keyframe that also tweens is held, because a span of several elements cannot tween).

XFL linear gradients of two colors are imported as gradients, laid on the shape by the file's matrix (the matrix is in pixels, unlike the edges, and maps a square 32768 units wide; the numbers of a real file were checked against that) only if that puts a square about the shape's size on it; a radial gradient, one with more stops (the first and last are kept), or one that does not fit its shape is imported as the average of its first and last stop, with a warning. SWF import still takes a gradient's first stop. Reported and not imported: bitmap fills, bitmaps, text, video, sound, frame scripts, filters,
color effects, blend modes, masks, guides, primitives, and
symbol runtime behaviour (a movie clip's own clock and a button's states keep their type but are not carried out, with a warning).
What is not imported is kept as source data (see Source preservation) and the file itself is kept
with the document, so that exporting an unchanged import writes the original back (see Source preservation).
Importing never touches the source: a file is only opened for reading, a folder is read into memory once and
imported from there, and nothing is created, renamed or deleted beside or inside it. The feature set has been
tested against fixtures written for these tests and, for the file-level guarantees, against every zipped `.fla` in
the vendored Ruffle suite; not against files from Animate.

## Export (XFL)

`flare export-xfl <project> <out>` writes an XFL folder (with the `.xfl` stub), or a
zipped `.fla` when `out` ends in `.fla`. It never overwrites, and the folder or archive
appears only once complete. Written: stage, frame rate, background, scenes, layers with
visibility and lock, frames and durations, instances with their matrix and motion tweens,
the library as Graphic symbols, and fills and strokes of shapes.

XFL edges hold only lines and quadratic curves, so cubic curves are approximated: each is
split into as many quadratic pieces as a published error bound requires to stay within
0.05 px, and the report says so. Also reported: gaps (written as empty frames), empty layers
(written with one empty frame), duplicate library names (renamed, since XFL needs unique
ones), a translucent background (written opaque) and invisible shapes (skipped). Names are
escaped and file names are made safe and unique.

Verification is round-trip only: exported files re-import with `flare-import` to the same
structure, identical pixels for straight edges and close pixels for curves. Whether Adobe
Animate opens them is untested. Unknown data is preserved as described in Source preservation.

A project imported from a `.fla` or an XFL folder exports as the original while it is unchanged, and keeps every
original file it did not touch once it is edited. The output says which (`Output::lossless`, printed by
`flare export-xfl` and the MCP tool as `lossless: {exact, reason}`).

## Publishing (SWF)

`flare publish-swf <project> <out.swf>` writes a SWF (zlib-compressed unless
`--no-compress`). It never overwrites, and the file appears only when complete. A SWF has
one timeline of characters and placements, so publishing keeps what a SWF can hold and bakes
the rest, with the editor's own evaluator (`flare_eval::evaluate_level`, one timeline a level at
a time). Every symbol becomes a `DefineSprite`, defined once however many instances there are,
and every instance a placement of it; each layer has a depth of its own and only what changed
is written from one frame to the next. A sprite plays by its own clock, so each symbol is
published the way its playback says: a Movie Clip and a looping Graphic loop (starting at the
instance's first frame), a Graphic that plays once ends on a `stop()`, a Graphic that holds a
frame is a one-frame sprite, and a Button is a one-frame sprite showing its Up state
(reported). A new span of an instance is a new object, so the symbol starts again. A camera is applied to the scene's own placements (its view multiplies each one's matrix),
so the movie shows the camera's picture and the sprites stay as they are. Tweens are
baked into a position or a shape for each frame, hidden layers are left out, and scenes play
one after another (reported). Real Ruffle plays the result on the clocks Flare predicts
(`flare-ruffle`'s render test), and a small SWF player in `flare-publish`'s tests checks the
same for every kind of symbol. Buttons that react to the pointer (`DefineButton2`) and shape
tweens kept as morphs (`DefineMorphShape`) are not written yet.

Each distinct shape becomes one `DefineShape4`, defined the first time it appears. SWF
shapes hold only lines and quadratic curves, so cubics are approximated within 0.05 px. Which
side of a contour is filled is decided from the geometry (probing both sides with a winding
test), so holes work whichever way the contours are wound; closed contours get an explicit
closing edge so strokes have all their sides. Limits are reported or refused: at most 65,535
frames, stage at most 8,191 px, frame rate capped at 255, background transparency dropped.

### Test Movie

Test Movie plays the published SWF in an *external* Ruffle process (Control > Test Movie,
or `flare test-movie <project>`). Running a foreign player is a trust boundary, so it is
kept at arm's length: the SWF is written to a private temporary directory, the player is
started directly with exactly one argument and never through a shell, its standard streams
are closed, and stopping or dropping the movie kills the player and removes the files. Ruffle
is found through `FLARE_RUFFLE`, then `ruffle` on `PATH`; if it is missing the app says how to
install it. Nothing here makes a hostile SWF safe: that is the player's job.

This is tested with a stand-in player (a script that records its arguments and the file it
received), including a path full of shell syntax and a mutation that routes the launch through
a shell. It has also been run against a real Ruffle: `scripts/build_ruffle.sh` builds Ruffle's desktop
player from the pinned checkout (about 3 minutes, 1.4 GB, release mode), and an ignored test
(`cargo test -p flare-ruffle --test real_player -- --ignored`, with `FLARE_RUFFLE` set) has
Test Movie open a published movie in it and checks the player is still running four seconds
later and is stopped and cleaned up on request. That shows the movie loads and plays; what it
sounds and looks like on screen is still for a person to judge. Shipping a player with the app
is a packaging decision not made yet: today it is found through `FLARE_RUFFLE` or `PATH`.

What Ruffle *draws* for our files is checked separately. Ruffle is pinned as a git submodule at
`third_party/ruffle` (a shallow checkout; `cargo` loads the whole of Ruffle's workspace, so it is not sparse), and with the
`render-check` feature `flare-ruffle` builds Ruffle's own `exporter` and its tests have Ruffle
render a published SWF on the GPU and compare the picture with Flare's own render of the same
frame: solid shapes, a gradient and a mask. It is off by default, because it adds Ruffle's
dependency tree (about 1.5 minutes and 2 GB of build output on top of a warm build) and needs
Java (Ruffle compiles its ActionScript library with Adobe's `asc.jar`) and a GPU adapter, so CI
does not run it: `cargo test -p flare-ruffle --features render-check`. Ruffle's source is used
unmodified under its MIT/Apache-2.0 licence, not copied or stripped.

Ruffle also runs *inside* the app, with `flare-ruffle`'s `embedded` feature: `EmbeddedMovie` builds
Ruffle's own core and wgpu renderer over a movie held in memory, on a `wgpu` device it is given
(the app's own, so the picture stays on the GPU as a texture), advances it by the time that
passed, and plays its sound through Ruffle's CPAL backend (or any backend a test injects). A
panic inside the foreign engine is caught and marks the movie failed instead of ending the
editor. Tests play published movies for real: a two-frame movie shows its second frame only after
a frame's time has passed and holds still when paused, a non-SWF is refused, and the sound a
published movie carries comes out of Ruffle's mixer (a movie with no cue is exactly silent).
Ruffle's first tick only loads the movie; the first frame follows once a frame's time has passed.

The desktop app uses it by default (its `ruffle` feature, on unless built with
`--no-default-features`, which falls back to an external Ruffle): Test Movie publishes the SWF on
a thread (`jobs::publish_movie`), and when the bytes come back `MovieViewport` opens an
operating-system window of its own, titled "Test Movie", opened at the picture's own size when the screen has room (most of the screen at most), as a second viewport of the editor's egui
context and wgpu painter. `MovieWindow` starts an `EmbeddedMovie` on the editor's own wgpu
device, registers its texture with that painter's renderer, and draws it: the picture fills the
window (shrunk to fit, never enlarged), and under it is a bar with a clock and the frame at the
left, one box in the middle (Restart, Pause/Play, Step one frame while paused, a playback speed
drop-down of 0.25x, 0.5x, 1x and 2x, a loop switch, and a sound switch) and Close at the right. Sound plays at
normal speed only, so it is off at the other speeds. The clock is real time, capped at 100 ms a
frame so a stall does not make the movie race; the texture is freed after the window
closes; sound goes to the default output device. A test shows Ruffle drawing on a device made
with wgpu's default limits, as the editor's is. Another paints the window with egui's own renderer into an off-screen texture and counts the movie's pixels in it: every one of a 120 x 90 picture. It failed at first, when the window showed nothing at all: Ruffle replaces its render texture when the player sets its viewport, and the handle taken before that was a picture nothing drew into. The window shares the editor's egui context as a second viewport, which egui forgets at the end of every editor frame unless that frame declares it (`keep_viewport`) and the context says it has real windows (`set_embed_viewports(false)`): without both, no control could be clicked. A GPU test clicks the controls with editor frames in between, and shows that it fails without the declaration. The window's contents have been looked at only as that
off-screen picture, and opening, closing and the editor drawing on afterwards are exercised by the scripted run (`scripts/gui_tests.sh`), which looks at no pixels; the movie gets no mouse or keyboard input yet (published movies have no scripting).
Tests that need a GPU adapter are ignored in the ordinary run and are run by `scripts/gpu_tests.sh`; `scripts/gui_tests.sh` runs the real app through opening and closing the window.

Encoding uses the `swf` crate from the Ruffle project (MIT/Apache-2.0). Tests parse the output
back with that crate's reader, play the display list, rebuild shapes from the decoded records
and compare renders with the document, and check fill sides against SWF's own definition (a
style belongs to one side of each edge). That is real evidence the file is well-formed and
says what we meant. Ruffle has drawn the frames of the render-check movies (above); sound,
playback over time and Flash Player have not been checked.
## Source preservation

Data Flare cannot edit is kept, not dropped, at two levels: the whole source file (below), and element by element.

### The source file

An import keeps what it read as the document's `Origin` (`flare-document`): the bytes of a `.fla` or `.swf`, or every
file and empty folder of an XFL folder (at most 256 MiB and 50,000 files; beyond that the import succeeds without a
source and says so in a diagnostic, and an export of it says it is rebuilt). With it goes a fingerprint of the
document the import made (`flare_storage::fingerprint`: a BLAKE3 over the records the project file stores, split into
"everything but the symbols" and one part for each symbol). The source rides along with every edit, undo step, save
and reload. It is what exports use:

- Unchanged (the fingerprint is the baseline again; undoing an edit counts): `.fla` to `.fla` is the original file
  byte for byte; a folder to a folder is every file and empty folder byte for byte; a `.fla` to a folder, or a folder to a
  `.fla`, has every entry identical but is a new container; a SWF import published as SWF is the original file.
- Edited: `DOMDocument.xml` is rebuilt unless only symbols changed, each library item is rebuilt unless it was not
  touched, and every other file in the source (bitmaps, sounds, `bin/`, publish settings, metadata, anything unknown)
  is written byte for byte, in the original order. Library files of deleted or renamed symbols are not written, and
  entries that cannot be read back out of the archive are left out, and both are reported.
- A movie imported from a SWF has no XFL to write back; an XFL export of it is rebuilt and says so. An edited SWF
  import is published by `flare-publish` as for any document.

Not kept: modification times and permissions of a folder's files; a folder containing a symbolic link (nothing is
followed out of it) or more than the limits; a legacy binary `.fla` (read as a skeleton).

### Elements

An `Attachment` is opaque source text (for XFL,
one XML element) tied to a stable entity ID (the document, a scene, a symbol or a layer),
with a placement saying where it belongs and a BLAKE3 digest of its content. Attachments
travel with the document: edits carry them through, undo and redo restore them, projects
save them, and XFL export writes them back where they came from.

What is kept on XFL import: unsupported elements inside a keyframe (text, bitmaps, video,
primitives), keyframe extras (frame scripts, sounds), whole layers that were skipped (masks,
guides, cameras), document-level data (media, fonts, publish settings) and
symbol-level data. Kept layers return to their original index among the layers.

The policy is deliberately honest rather than clever:

- Deleting the layer, scene or symbol an attachment hangs off deletes the attachment with
  it. Frame-level data remembers the keyframe it came from; if edits remove that keyframe,
  export reports it could not be placed and does not write it, instead of guessing.
- Attachment text is untrusted. It is size-capped (1 MiB each, 10,000 per document), checked
  against its digest on load, and on export must be exactly one well-formed element with no
  DTD or processing instruction, otherwise it is reported and skipped.
- Limits to know about: preservation is element-level, so an unsupported *part* of a supported
  element (a gradient in a shape, a filter on an instance) is still lost; kept elements are
  written after Flare's own in their keyframe, so their stacking relative to them can change;
  masks and their relationships to masked layers are kept as layers but not as semantics;
  a bitmap file is carried only through the source file above (so a project with no source, or an export to
  another format, has references to bitmaps that dangle); and kept
  data has only been checked by round-tripping through our own importer, not by Adobe Animate.

## Frontends

`flare` commands: `new`, `info`, `set-stage`, `add-symbol`, `draw-rect`, `add-layer`, `camera`,
`layer`, `place`, `frame`, `eval`, `export`, `export-frames`, `inspect`, `rate`.

Inside `flare-animate`, `Workspace` is the one object the window talks to, and its topics are modules
under `workspace/` (layers, selection, timeline, files, tabs, and so on; one module per panel under
`panels/`). Its settings and working state are grouped by concern in `workspace/groups.rs` (the
Text tool, paint, what the stage displays, open windows, autosave and recovery, sound) instead of
being separate fields.

`flare-desktop` is a dock of panels: the stage (rendered by the same pipeline), a timeline with scrubbing
and playback at the document frame rate, a library, and tools: Select/Move, Rectangle,
Ellipse, Line, Pen and Text (click for corners, drag for smooth handles, click the first point or
press Enter to finish, Backspace to remove a point, Esc to cancel), with fill and outline
settings. The stage zooms (pinch, Ctrl/Cmd+scroll or the View menu, about the cursor)
and pans (scroll or middle-drag). A layer can be made a mask for the layer directly below it (Modify > Timeline): the mask is never drawn, and the layer below shows only where the mask has shapes (the union of its shapes at that frame; overlapping shapes of opposite winding cut holes). Masked artwork can only be picked where it shows. XFL export writes real mask layers; XFL import still keeps masks as preserved source data, and SWF publishing reports that masks are not written. Sounds: File > Import Sound reads and decodes a file on a worker, keeps the original file in the project (the document shares it, so snapshots stay cheap), and puts a cue at the playhead on a new layer. A cue plays its sound from its frame while the timeline plays; sounds start with playback, again when the loop wraps or the playhead jumps back, and stop on pause. Cues move with their frames when frames are inserted or removed, and deleting a layer removes sounds nothing else plays. XFL export and SWF publishing report that sounds are not written. The Text tool opens a small editor where you click; Place (Ctrl/Cmd+Enter) turns the text into a filled shape in the active layer's keyframe, in the chosen font family and size, with the font work done on a worker. Shift-click on timeline cells selects frames of a layer; Copy Frames and Paste Frames (Ctrl/Cmd+Alt+C, V) copy them and paste them from the playhead, replacing what is there. Shift-click builds a multi-selection that moves and deletes together (other
commands act on the first selected span). Duplicate (Cmd/Ctrl+D) copies the selection into the same keyframes nudged aside (onto a new layer only where the keyframe tweens); Copy and Paste (Cmd/Ctrl+C, V) remember the selection and paste it
centered on the stage at the current frame on new layers; Paste in Place (Edit menu) puts it back where it was. An instance whose symbol is missing from the document is refused. The Modify menu flips, rotates by 90 degrees and scales the selection about its
center. Eight handles on the
selection's bounds scale it by dragging (Shift keeps proportions); dragging just outside a
corner rotates it about its center (Shift snaps to 15 degrees); Alt-dragging an edge handle skews
it. A motion-tweened instance cannot be skewed or mirrored, because a tween only interpolates
scale, rotation and translation. It supports undo/redo, Delete, Convert to
Symbol, editing symbols in place, motion tweens, layer management and the frame shortcuts
F5/F6/F7 (with Shift for the inverse). Dragging a span in the timeline moves it, dragging its right edge resizes it; neither may overlap another span. The timeline zooms (buttons, pinch or
Ctrl/Cmd+scroll) and scrolls sideways. On macOS the menu bar is native (muda); everywhere
else it is drawn in the window. See [decisions/UI-STACK.md](decisions/UI-STACK.md).

## Known gaps

- No radial or multi-stop gradients or bitmap fills. Text is converted to shapes when it is placed, so it cannot be edited as text afterwards, and there is no rich text, text on a path or per-character styling. The desktop app creates
  horizontal or vertical gradients on new shapes only; they cannot yet be edited afterwards.
  XFL export and SWF publishing paint the solid fill, ignoring gradients.
- XFL import covers a defined subset (see Import) and keeps the rest as source data (see Source preservation); XFL export and SWF publishing are verified only against our own importer and Ruffle's parser, never against Animate or a player.
- No detachable panels.
- Undo history is not persisted, and recovery covers one document at a time.
- Pen pressure, IME and accessibility are untested. The desktop window and the native macOS
  menu have been built and launched but not exercised by hand; the menu's enabled-state and
  labelling logic is covered by tests, the AppKit integration is not.
