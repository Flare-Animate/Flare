# Roadmap

Status is evidence-based: a milestone item is done only when it has tests.

## Priorities

The high-priority list is the Roadmap in [README.md](../README.md) and is repeated here word for
word; the lower-priority list and what is not planned follow it. Priority is how much a person
animating with Flare misses the thing, not how hard it is. It was drawn up from a review of
`original/` (see [parity.md](parity.md)) and from Animate's own feature list, and it is the
maintainer's to reorder.

### High priority

- Merge drawing: Flash's planar-map shapes, where overlapping shapes cut and merge each other (today shapes are separate objects)
- Drawing quality: brush dynamics, and tracing a bitmap into vector shapes
- SWF that keeps more of the document: buttons with their states, shape tweens as morphs, masks of several shapes and sounds inside symbols
- A Motion Editor: per-property graphs and custom ease curves for tweens
- Importing artwork from SVG and Photoshop files
- Bones and inverse kinematics (the Bone and Bind tools)
- Editing text after it is placed
- Editable keyboard shortcuts

### Lower priority

Drawing and tools
- The rest of Animate's tools: 3D Rotation and Translation, Deco, Width, Asset Warp, Gradient Transform and Magic Wand
- Art and pattern brushes for the Paint Brush, and tablet tilt
- Layer depth (2.5D with the camera), and, for the camera, layers attached to it, following frames added to the movie and XFL import and export
- Rich text (mixed styles in one field) and text menu commands
- Bitmap compression settings; importing Illustrator (`.ai`) files

Timeline, symbols and library
- Shape hints and more than one scene
- Find and Replace across text, fonts, colors and symbols
- Shared and external libraries, and an Assets panel
- A visual Frame Picker for a Graphic's first frame
- History panel extras: replaying steps and saving them as a command

Scripting
- ActionScript and a script editor, Code Snippets, and scripting with the Commands menu (ActionScript in an imported file is kept as data and never run)

Interface
- A Flash 8 interface skin, and custom themes (a contrast check for them, as `original/` had, so a theme cannot make text unreadable)
- A fully rearrangeable toolbar (tools can already be pinned from the "…" list)
- Named workspaces and the Interface switcher at the top right

Formats
- Reading the old binary `.fla` (CS4 and before), which is recognised and refused today
- Opening a SWF as a document, `.sol` shared objects, importing video, and Moho projects

Sharing and releases
- Sharing from the Share menu to YouTube and other services
- A released web build (it is experimental today), with MP4 export and sound import
- An auto-update that checks a SHA-256 before it replaces anything
- Developer-ID signing and notarising on macOS, and a double-click that opens a file there
- Windows installer extras (a Chocolatey package)

Tooling for the project
- A pre-commit hook that runs the format check before a commit (the old project had one that kept
  its docs in step with the code)
- [codesight](https://www.npmjs.com/package/codesight) keeps `.codesight/`, an AI-readable map of
  the code that is not committed: `npx codesight --wiki --watch` (see [engineering.md](engineering.md))

### Not planned

Android, P2P remote control, a game engine, Adobe/Harman AIR and Flex SDK targets, and OpenToonz's
raster painting, FX schematic and xsheet. [parity.md](parity.md) says why for each.

| Milestone | Scope | Status |
|---|---|---|
| M0 baseline | Pinned toolchain, centralized dependencies, locked builds, honest tests, renderer decision | Done |
| M1 source model and safe editing | Typed IDs, validated document, atomic edits, undo/redo, headless CLI | Done |
| M2 usable vertical slice | Draw, convert to symbol, instances, tween, scrub/play, save/reopen, PNG export, one modern workspace | Done in code; needs hands-on checking (see below) |
| M3 FLA/XFL compatibility and automation | Raw source IR, modern XFL/FLA subset, unknown-data preservation, MCP over stdio | MCP, XFL import, XFL export and element-level source preservation done (checked against fixtures and each other only, never against Animate) |
| M4 Test Movie and advanced features | Hosted Ruffle playback, SWF publishing, richer text/audio, advanced tweens and masks | SWF publishing (symbols kept as sprites, tweens baked) and Test Movie through an external Ruffle process done and tested without a real player; richer text/audio and masks not started |
| M5 hardening | Long sessions, crash recovery, packaging, accessibility, corpus expansion | Partly: crash recovery and fault-injection tests done; packaging, accessibility and long-session profiling not started |

## Required before a release (the maintainer's bar)

If drawing, exporting and playing do not all work well, Flare is not released. What is known to be missing against that bar, in the order it is meant to be tackled:

1. **MP4**: done, in pure Rust and desktop only (`flare-video`: `rusty_h264` for the picture, OxideAV's `oxideav-aac` for the sound, `muxide` for the container). Checked against the decoders of `rusty_h264` and `oxideav-aac` and, where one is installed, against ffprobe and ffmpeg. The sound encoder's one-frame delay is cancelled by holding the first picture that long (the container has no edit list), pinned by a test against the encoder itself. `rusty_h264` runs with its SIMD kernels (1.9 times faster than the scalar build on arm64, same bytes out; not measured on x86-64). Shipping H.264 and AAC encoders is accepted, as every media application does. Not measured: how the encoders hold up on long real animation and music.
2. **Symbols that behave like Animate's.** Today every symbol is evaluated as a Graphic that loops with its parent (`flare-eval`), so a Button plays all of its frames instead of showing its Up state, a Graphic has no Loop, Play Once or Single Frame option with a first frame, and a Movie Clip has no clock of its own beyond that loop. Needs the instance's playback options in the document model, the file format and XFL, Properties controls, and the evaluator.
3. **Merge drawing**: Flash's planar-map shapes, where overlapping shapes cut and merge each other and strokes and fills are edited separately. The largest gap in [parity.md](parity.md) and a change to the document model, not a feature on top of it.
4. **Camera**: done in code: a camera layer and tool whose keyframed position, zoom and rotation drive every export, the SWF and HTML5 pages, and a preview of the stage (real Ruffle draws the same picture as Flare). Still to do: someone using it by hand, XFL import and export of the camera, a camera that follows frames added to the movie, and attaching layers to it.
5. **Timeline controls** good enough to animate with, and **Ruffle** playback proven against a real player.
6. **SWF that keeps its symbols**: symbols are published as sprites, each on the clock its playback says, and a real Ruffle plays them as Flare does (done). Still to do: buttons as buttons with their states (`DefineButton2`; today a Button is a sprite showing its Up state), shape tweens as morphs (`DefineMorphShape`; today each step is its own shape), masks of several shapes, and sounds inside symbols. The HTML5 page is still baked.
7. **Sharing the way Animate does it**: its Share menu logs in to YouTube and Twitter and uploads the exported video itself. That needs each service's API with credentials the project registers (YouTube is the practical first, now that MP4 exists); only Twitter's button is live (it fills in the post); the others are dimmed until then.

## M2 remaining

- Look at and fix the desktop window and the native macOS menu; both have only been built and launch-tested, not exercised by hand.
- Hands-on UX testing of the docking prototype; it was chosen on API fit only.
- Pen pressure, IME and accessibility checks on real hardware.
- Storage migration tests and broader fault injection (disk full, power loss).

## How this branch relates to the feature branches

`integration/all` merges every feature branch below and passes the full checks, so they are
known to combine. Each feature branch also stands alone as a small PR off
`rewrite-from-scratch`. Where they overlap (`workspace.rs`, `actions.rs`, the CLI), the
merge was resolved by hand and the merged result is tested as a whole.

| Branch | What it adds |
|---|---|
| `feat/recovery` | Autosave to a recovery store, recover on launch, discard on confirmed close |
| `feat/mcp` | MCP stdio server (`flare-mcp`, on `rmcp`) with permissions and revision checks |
| `feat/xfl-import` | XFL / modern FLA import subset (`flare import`) with diagnostics, robustness tests |
| `feat/xfl-export` | XFL / `.fla` export (`flare export-xfl`), stacked on `feat/xfl-import` |
| `feat/source-preservation` | Keep unsupported XFL data as attachments through edit, save and export; stacked on `feat/xfl-export` |
| `feat/test-movie` | Test Movie via an external Ruffle process, stacked on `feat/swf-publish` |
| `feat/swf-publish` | SWF publishing (`flare publish-swf`), stacked on `feat/xfl-export` |
| `feat/docking` | `egui_tiles` dock with a validated, remembered layout and Reset Workspace Layout |
| `feat/onion-skin` | Onion skinning with before/after ranges |
| `feat/hardening` | Fault-injection and random-edit invariant tests |

## Not feasible without more input

- Pen pressure, IME and accessibility need real hardware and a person; AccessKit is not on.
- Hosted Ruffle playback and SWF publishing need a Ruffle revision chosen and tested, and a
  real player to check output against. A written-but-unverifiable SWF writer would claim more
  than can be shown.
- Compatibility claims need real Animate files; the XFL importer and exporter have only seen
  authored fixtures and each other.
- App icons must be trimmed of their black corners before they go in `/assets` (packaging).

## Interface follow-ups

The interface is laid out in full and the planned parts are dimmed (see [ui.md](ui.md)). Each of
these replaces a mock with the real feature: the missing tools, the editable property fields, the
Align, Transform and History panels, library management, the color panel's editors, the timeline
extras, and the remaining menu commands.

## Next independent branches

Each of these can be its own branch and PR:

1. Text: editable text objects (the tool converts text to shapes), rich text, text on a path.
2. Fills and strokes: radial and multi-stop gradients, editing gradients, gradients and masks in XFL import and SWF, paint bucket and ink bottle behaviour.
3. Timeline UX: drag-moving frame ranges; sound: start/stop/loop options per cue, volume, mid-sound start when scrubbing, waveform display, sounds in XFL and SWF.
4. Compatibility evidence: a lawful corpus of real Animate files to test import and export against, and check what the importer keeps against files Animate wrote.
5. Input and accessibility: pen, IME and screen-reader results.
6. Packaging: app bundles and installers, once the icons are trimmed.
