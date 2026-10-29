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

A `Shape` is a validated path (move/line/cubic/close, segment count capped), a fill and
an optional round-cap `Stroke`. An `Instance` references a symbol and carries a full
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
side and 100 million pixels. Hit testing uses the fill with the same non-zero rule and
returns candidates topmost first so callers can skip locked layers. Strokes are not
hit-tested yet.

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

## Frontends

`flare-cli` commands: `new`, `info`, `set-stage`, `add-symbol`, `draw-rect`, `add-layer`,
`layer`, `place`, `frame`, `eval`, `export`, `export-frames`, `inspect`, `rate`.

`flare-desktop` shows the stage (rendered by the same pipeline), a timeline with scrubbing
and playback at the document frame rate, a library, and tools: Select/Move, Rectangle,
Ellipse and Line, with fill and outline settings. Duplicate (Cmd/Ctrl+D) copies the selection onto a new layer. It supports undo/redo, Delete, Convert to
Symbol, editing symbols in place, motion tweens, layer management and the frame shortcuts
F5/F6/F7 (with Shift for the inverse). On macOS the menu bar is native (muda); everywhere
else it is drawn in the window. See [decisions/ui-stack.md](decisions/ui-stack.md).

## Known gaps

- No pen or path tool, gradients, masks, text or bitmap fills.
- No FLA/XFL import or export, and no SWF parsing beyond signature inspection.
- No stage zoom or pan, no docking, no multi-select, no drag-editing of spans.
- No autosave or crash recovery; undo history is not persisted.
- Pen pressure, IME and accessibility are untested. The desktop window and the native macOS
  menu have been built and launched but not exercised by hand; the menu's enabled-state and
  labelling logic is covered by tests, the AppKit integration is not.
