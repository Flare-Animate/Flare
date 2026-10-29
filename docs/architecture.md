# Architecture

## Crates

Dependencies point down this list; a crate never depends on one above it.

| Crate | Responsibility |
|---|---|
| `flare-core` | Typed IDs (UUIDv7), `FrameRate`, `DocumentPoint`, `Transform2D`, `Rgba8`. No I/O. |
| `flare-document` | The document model and its validation. No GUI, GPU, database or async code. |
| `flare-edit` | The only way to change a document: commands, atomic batches, undo/redo, frame operations, in-place symbol editing support. |
| `flare-eval` | Pure frame evaluation: document + scene + frame to placed instances and drawn shapes. |
| `flare-render` | CPU rasterization (vello_cpu), PNG output, hit testing. |
| `flare-storage` | The `.flare` SQLite container: save, load, revalidation. |
| `flare-ui` | The egui workspace: stage, timeline, tools, library, menu actions. No window or GPU code. |
| `flare-mcp` | An MCP server over stdio for one open project: agent tools built on the same edit commands, with permissions and revision checks. |
| `flare-import` | XFL import: XFL directories and zipped modern `.fla` files into a Flare document, with a diagnostic for everything it cannot carry over. |
| `flare-export` | XFL export: a Flare document as an XFL folder or a zipped `.fla`, with a diagnostic for everything the format cannot hold exactly. |
| `flare-publish` | SWF publishing: a document as a baked SWF, encoded with Ruffle's `swf` crate. |
| `flare-flash` | Container signature inspection only (SWF/ZIP/CFB prefixes). Not a parser. |
| `apps/flare-cli` | Headless project editing and export. |
| `apps/flare-desktop` | The winit/egui-wgpu shell: window, GPU surface, native dialogs and menu. |

The CLI and the desktop app edit documents through the same `flare-edit` commands and
render through the same `flare-eval` and `flare-render`. That is deliberate: there is one
set of semantics, and the UI is not allowed to have private ways to mutate a document.

## Document model

A `Document` has a frame rate, a `Stage` (size 1 to 16384 per side, background), ordered
scenes and a library of symbols. A scene or symbol owns a `Timeline`: layers ordered top
to bottom, each with a name, visible and locked flags, and non-overlapping `FrameSpan`s.

A span is a held exposure: start frame, positive duration, `Content` (`Empty`, `Shape` or
`Instance`) and an optional linear `Tween`. An empty keyframe is a span with `Empty`
content, which is not the same as having no span. A held exposure is one span, never many
cloned keyframes.

A `Shape` is a validated path (move/line/cubic/close, segment count capped), a fill, an
optional two-color linear `Gradient` (painted instead of the solid fill, in the shape's own
coordinates, so it follows transforms) and an optional round-cap `Stroke`. An `Instance` references a symbol and carries a full
finite 2D affine matrix (shear and reflection are preserved, not decomposed away).
Symbols are Graphic symbols: parent-clocked, looping over their own length.

Invariants, enforced by constructors and by `Document::new`, which reports every
violation as a `Diagnostic`:

- IDs are unique per kind, and instances only reference existing symbols.
- Symbols cannot contain themselves, directly or indirectly.
- Spans on a layer never overlap; durations are non-zero; frame arithmetic cannot overflow.
- Coordinates and matrices are finite; stage and stroke sizes are bounded.

Nothing in the model derives `Deserialize`. Storage rebuilds documents through the same
validating constructors, so a file cannot smuggle in an invalid document.

## Editing

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
functions in `flare_edit::frames` from a layer and frame to a command batch. Copies get
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
side and 100 million pixels. Hit testing counts a fill (same non-zero rule) and a stroke (where it is drawn: round caps
and joins, width scaled by the transform), so a line or an outline-only shape can be picked.
It returns candidates topmost first so callers can skip locked layers.

Onion skinning composites translucent copies of neighbouring frames (up to 8 before and
after, fainter with distance) beneath the current one using straight-alpha source-over
blending in `flare-render`. It is a display aid only: it is off while playing and never
affects exports.

## Storage

A `.flare` file is a SQLite database (`application_id` "FLAR", `user_version` 1) with a
`project` row and an `entity` table of scene and symbol payloads (JSON, versioned). The
format is a draft and not stable: do not ship user documents before migration, recovery
and fault-injection tests exist.

- Save writes a complete staged database next to the target and renames it into place, so a
  failed save leaves the previous file intact. The directory is not fsynced after the rename.
- Load treats the file as untrusted: defensive SQLite settings, schema trust off, entity and
  payload size limits, a version check that refuses newer files, and full revalidation.
- Older files load with defaults for fields added later (stage, layer flags, strokes).

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

`flare-cli mcp <project>` serves the project to an AI agent as newline-delimited JSON-RPC
over stdio; stdout carries only protocol messages. Tools use the same commands, frame
operations and renderer as the UI, so validation, atomic batches and undo apply to agents
too. The server is read-only unless started with `--allow-write`, and nothing reaches disk
until the `save` tool. `export_png` is off unless `--export-dir` is given and only writes a
sanitized file name inside it. Mutating tools accept `expected_revision` (from `get_info`)
and fail on a mismatch, so an agent cannot overwrite changes it has not seen. Request lines
are bounded (1 MiB) and batches are not supported. Tool failures are results with
`isError` set, so the agent can read and react; protocol misuse uses JSON-RPC errors.
## Import (XFL)

`flare-cli import <source> <out.flare>` reads an XFL directory, an `.xfl` file (its folder is
used) or a zipped modern `.fla`, and writes a new project. It never overwrites, and the
result is an ordinary document. Input is untrusted: XML depth, element count and file sizes
are capped, DTDs are refused, file names cannot escape the document, and symlinks out of an
XFL folder are not followed. Legacy binary `.fla` files are recognised and refused.

Imported: stage size, frame rate (NTSC rates stay exact) and background; scenes; normal
layers with name, visibility and lock; keyframes and durations; symbol instances with their
matrix; solid fills and solid strokes of drawn shapes, including quadratic curves and
holes (edges are decoded and chained into contours per fill); the library as Graphic
symbols; classic motion tweens between instances. A keyframe with several elements puts
the extras on additional layers above, preserving stacking.

Reported and not imported: gradients, bitmaps, text, video, sound, frame scripts, filters,
color effects, blend modes, masks, guides, folders, shape tweens and easing, primitives, and
symbol runtime behaviour (movie clips and buttons become Graphic symbols with a warning).
Foreign data is not preserved, so anything not imported is lost on save. This has been
tested only against fixtures written for these tests, not against files from Animate; real
corpus evidence is the next step before calling any feature supported.

## Export (XFL)

`flare-cli export-xfl <project> <out>` writes an XFL folder (with the `.xfl` stub), or a
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
Animate opens them is untested. Source preservation of unknown data is not implemented.

## Publishing (SWF)

`flare-cli publish-swf <project> <out.swf>` writes a SWF (zlib-compressed unless
`--no-compress`). It never overwrites, and the file appears only when complete. A SWF has
one timeline of shapes and placements, so publishing *bakes* each frame with the same
evaluator the editor uses: tweens become per-frame matrices, symbol instances are flattened
into their shapes, hidden layers are left out, and scenes play one after another (reported).
The result shows the same pictures but has no symbols or runtime behaviour, which is a
different job from Test Movie playback.

Each distinct shape becomes one `DefineShape4`, defined the first time it appears. SWF
shapes hold only lines and quadratic curves, so cubics are approximated within 0.05 px. Which
side of a contour is filled is decided from the geometry (probing both sides with a winding
test), so holes work whichever way the contours are wound; closed contours get an explicit
closing edge so strokes have all their sides. Limits are reported or refused: at most 65,535
frames, stage at most 8,191 px, frame rate capped at 255, background transparency dropped.

### Test Movie

Test Movie plays the published SWF in an *external* Ruffle process (Timeline > Test Movie,
or `flare-cli test-movie <project>`). Running a foreign player is a trust boundary, so it is
kept at arm's length: the SWF is written to a private temporary directory, the player is
started directly with exactly one argument and never through a shell, its standard streams
are closed, and stopping or dropping the movie kills the player and removes the files. Ruffle
is found through `FLARE_RUFFLE`, then `ruffle` on `PATH`; if it is missing the app says how to
install it. Nothing here makes a hostile SWF safe: that is the player's job.

This is tested with a stand-in player (a script that records its arguments and the file it
received), including a path full of shell syntax and a mutation that routes the launch through
a shell. It has not been run against a real Ruffle.

Encoding uses the `swf` crate from the Ruffle project (MIT/Apache-2.0). Tests parse the output
back with that crate's reader, play the display list, rebuild shapes from the decoded records
and compare renders with the document, and check fill sides against SWF's own definition (a
style belongs to one side of each edge). That is real evidence the file is well-formed and
says what we meant. It has not been played in Flash Player or Ruffle.
## Source preservation

Data Flare cannot edit is kept, not dropped. An `Attachment` is opaque source text (for XFL,
one XML element) tied to a stable entity ID (the document, a scene, a symbol or a layer),
with a placement saying where it belongs and a BLAKE3 digest of its content. Attachments
travel with the document: edits carry them through, undo and redo restore them, projects
save them, and XFL export writes them back where they came from.

What is kept on XFL import: unsupported elements inside a keyframe (text, bitmaps, video,
primitives), keyframe extras (frame scripts, sounds), whole layers that were skipped (masks,
guides, folders, cameras), document-level data (media, fonts, publish settings) and
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
  binary assets such as bitmaps are not carried, so kept references to them dangle; and kept
  data has only been checked by round-tripping through our own importer, not by Adobe Animate.

## Frontends

`flare-cli` commands: `new`, `info`, `set-stage`, `add-symbol`, `draw-rect`, `add-layer`,
`layer`, `place`, `frame`, `eval`, `export`, `export-frames`, `inspect`, `rate`.

`flare-desktop` is a dock of panels: the stage (rendered by the same pipeline), a timeline with scrubbing
and playback at the document frame rate, a library, and tools: Select/Move, Rectangle,
Ellipse, Line and Pen (click for corners, drag for smooth handles, click the first point or
press Enter to finish, Backspace to remove a point, Esc to cancel), with fill and outline
settings. The stage zooms (pinch, Ctrl/Cmd+scroll or the View menu, about the cursor)
and pans (scroll or middle-drag). Shift-click builds a multi-selection that moves and deletes together (other
commands act on the first selected span). Duplicate (Cmd/Ctrl+D) copies the selection onto
new layers nudged aside; Copy and Paste (Cmd/Ctrl+C, V) remember the selection and paste it in
place at the current frame on new layers. An instance whose symbol is missing from the document is refused. The Modify menu flips, rotates by 90 degrees and scales the selection about its
center. Eight handles on the
selection's bounds scale it by dragging (Shift keeps proportions). It supports undo/redo, Delete, Convert to
Symbol, editing symbols in place, motion tweens, layer management and the frame shortcuts
F5/F6/F7 (with Shift for the inverse). Dragging a span in the timeline moves it, dragging its right edge resizes it; neither may overlap another span. The timeline zooms (buttons, pinch or
Ctrl/Cmd+scroll) and scrolls sideways. On macOS the menu bar is native (muda); everywhere
else it is drawn in the window. See [decisions/ui-stack.md](decisions/ui-stack.md).

## Known gaps

- No radial or multi-stop gradients, masks, text or bitmap fills. The desktop app creates
  horizontal or vertical gradients on new shapes only; they cannot yet be edited afterwards.
  XFL export and SWF publishing paint the solid fill, ignoring gradients.
- XFL import covers a defined subset (see Import) and keeps the rest as source data (see Source preservation); XFL export and SWF publishing are verified only against our own importer and Ruffle's parser, never against Animate or a player.
- No rotate or skew handles, no timeline zoom, no detachable panels.
- Undo history is not persisted, and recovery covers one document at a time.
- Pen pressure, IME and accessibility are untested. The desktop window and the native macOS
  menu have been built and launched but not exercised by hand; the menu's enabled-state and
  labelling logic is covered by tests, the AppKit integration is not.
