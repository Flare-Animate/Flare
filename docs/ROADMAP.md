# Roadmap

Status is evidence-based: a milestone item is done only when it has tests.

## Priorities

The high-priority list is the Roadmap in [README.md](../README.md) and is repeated here word for
word; the lower-priority list and what is not planned follow it. Priority is how much a person
animating with Flare misses the thing, not how hard it is. It was drawn up from a review of
`original/` (see [PARITY.md](PARITY.md)) and from Animate's own feature list, and it is the
maintainer's to reorder.

### High priority

- Merge drawing: Flash's planar-map shapes, where overlapping shapes cut and merge each other (today every shape is a separate drawing object, though shapes drawn on a layer now share its keyframe)
- Drawing quality: brush dynamics, and tracing a bitmap into vector shapes
- SWF that keeps more of the document: buttons with their states, shape tweens as morphs, masks of several shapes and sounds inside symbols
- A Motion Editor: per-property graphs and editable curve handles for tweens (curve families, easings and in-betweens are in)
- Importing artwork from SVG and Photoshop files
- Bones and inverse kinematics (the Bone and Bind tools)
- Editable keyboard shortcuts

### Lower priority

Where Animate already has a feature (mask layers, the Width tool, Asset Warp rigging, lip sync, a camera layer) its version comes first and the improvements below build on it.

Drawing and tools
- The rest of Animate's tools: 3D Rotation and Translation, Deco, Width, Asset Warp, Gradient Transform and Magic Wand
- Art and pattern brushes for the Paint Brush, and tablet tilt
- Layer depth (2.5D with the camera), and, for the camera, layers attached to it, following frames added to the movie and XFL import and export
- Rich text (mixed styles in one field) and text menu commands
- Bitmap compression settings; importing Illustrator (`.ai`) files
- Editable vector brushes: a stroke that keeps its centerline, pressure samples and a width profile, so its taper and curve can be changed after it is drawn, with reusable brush presets and a better width-profile editor
- Live symmetry: a mirror or radial modifier whose copies follow the source until they are expanded into ordinary shapes (drawing with symmetry already works)
- Perspective guides: one-, two- and three-point sets with editable vanishing points and a horizon, snapping, and no part of the exported artwork
- Toon shading and lighting brushes: paint a chosen shadow or highlight level that stays at that level however often it is painted over, kept apart from the artwork so a recolor keeps the shading, with editable tone ramps
- A reference board dock (images, notes, color picking and pinned views) that is saved with the project and never exported
- A poseable mannequin for reference (joints, pose presets, adjustable proportions, a captured pose as a picture), not a 3D editor
- Selecting objects inside a bitmap, as Photoshop's Object Selection does

Timeline, symbols and library
- Shape hints and more than one scene; a scene manager that reorders scenes by dragging and shows thumbnails
- Several named cameras and a shot track that picks the camera for each stretch of time, with hard cuts first, so a close-up can be framed without keyframes or a copy of the scene
- Masking beyond mask layers: clipping groups, clip to base, and alpha and luminance masks, each with a defined order and no cycles
- An ordered, non-destructive effect stack for post-processing objects, groups and the scene's output
- Find and Replace across text, fonts, colors and symbols
- Shared and external libraries, and an Assets panel
- A visual Frame Picker for a Graphic's first frame
- History panel extras: replaying steps and saving them as a command

Rigging and character animation
- Rigs of Flare's own, as Spine and DragonBones have them: bones, attachment slots with their own draw order, skins, weighted mesh binding with weight painting, forward kinematics and IK, transform, path and physics constraints that run in a stated order, and a way to see why a bone moves; rig animations can be mixed and sit inside symbols next to the timeline
- Puppet deformation with pins or a cage that keeps the original artwork and derives the deformed picture from it, so quality is not lost with each pose
- Lip sync: a manual mouth-shape workflow first (a frame picker, a waveform and named mouth poses), then automatic cues from an optional analysis tool such as Rhubarb Lip Sync, with mappings for any number of mouth drawings and results that stay editable

Scripting
- Full ActionScript 3 support: a script editor, authoring AS3 in a document and publishing it, running it in Test Movie and the published SWF, Code Snippets, and scripting with the Commands menu (ActionScript in an imported file is kept as data and never run today)

Interface
- A Flash 8 interface skin, and custom themes (a contrast check for them, as `original/` had, so a theme cannot make text unreadable)
- A fully rearrangeable toolbar (tools can already be pinned from the "…" list)
- Named workspaces and the Interface switcher at the top right

Formats
- Reading the old binary `.fla` (CS4 and before), which is recognised and refused today
- `.sol` shared objects, importing video, and Moho projects
- Sprite export: sprite sheets and atlases of the stage or of one symbol or animation, with the pixels and the facts a game needs (frame durations, trim offsets, original sizes, pivots, animation names and loops), as PNG with JSON or a similar description
- Layered PSD and PSB export with a Unity profile that keeps names, groups, bounds and order and bakes what Unity ignores (blend modes, opacity, effects) into the pixels
- More video containers (MOV, MKV and AVI) where the encoder behind them supports the picture, sound and transparency asked for

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
  the code that is not committed: `npx codesight --wiki --watch` (see [ENGINEERING.md](ENGINEERING.md))

### References for the larger items

Spine, DragonBones, OpenToonz's Plastic tool, Unity's 2D Animation package and `openrr/k` are for
studying how a rig is authored and solved. Spine is proprietary and its runtimes are under a
licence that does not allow copying: its documentation, tutorials and behaviour are reference
only, and none of its source goes into Flare. Flare's rigging is its own, built on the same
document model, commands and renderer as the rest. When any of it leaves Flare for another
format it is preserved exactly, converted so it stays editable, baked with the person's approval,
or refused with a reason.

### Not planned

Android, P2P remote control, a game engine, Adobe/Harman AIR and Flex SDK targets, and OpenToonz's
raster painting, FX schematic and xsheet. [PARITY.md](PARITY.md) says why for each.

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
3. **Merge drawing**, after **many elements in a keyframe** (done through stage 3 of [decisions/ELEMENTS.md](decisions/ELEMENTS.md): drawing no longer makes a layer per shape; stage 4, keeping a whole FLA's elements through save and open, and stage 5 remain): Flash's planar-map shapes, where overlapping shapes cut and merge each other and strokes and fills are edited separately. The largest gap in [PARITY.md](PARITY.md) and a change to the document model, not a feature on top of it.
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

The interface is laid out in full and the planned parts are dimmed (see [UI.md](UI.md)). Each of
these replaces a mock with the real feature: the missing tools, the editable property fields, the
Align, Transform and History panels, library management, the color panel's editors, the timeline
extras, and the remaining menu commands.

## Next independent branches

Each of these can be its own branch and PR:

1. Text: rich text (mixed styles in one field), paragraph settings, text on a path, and the text menu's commands. Text objects that are edited again after they are placed are done.
2. Fills and strokes: radial and multi-stop gradients, editing gradients, gradients and masks in XFL import and SWF, paint bucket and ink bottle behaviour.
3. Timeline UX: drag-moving frame ranges; sound: start/stop/loop options per cue, volume, mid-sound start when scrubbing, waveform display, sounds in XFL and SWF.
4. Compatibility evidence: a lawful corpus of real Animate files to test import and export against, and check what the importer keeps against files Animate wrote.
5. Input and accessibility: pen, IME and screen-reader results.
6. Packaging: app bundles and installers, once the icons are trimmed.

## Tweening (`flare-tween`)

Built, tested in the library: interpolation families and easing with their parameters, Bezier
keyframe handles (Auto, AutoClamped, Vector, Aligned, Free) with solving, cubic-bezier evaluation by
bisection, the extrapolation modes, a `Curve` over sorted keys with validation, and grease-pencil
style in-betweens of stroke points (`interpolate_sequence`). Clean-room: see ARCHITECTURE.md.

Next, in order:
- A validated command (`SetKeyframeInterpolation`, which sets a key's segment shape, easing and
  handle kind, one undo step each) and a `CreateInbetweens` command over two keyframes' strokes.
- A `flare tween` command (`list` for the easings, `eval` for a curve) and a `flare-mcp` tool with
  the same two calls, so the surfaces in SURFACES.md get their rows to yes.
- The Motion Editor (the high-priority item above) draws each property's curve from `Curve` and edits
  handles through the command.
- Making the renderer use `flare-tween` for motion tweens in place of the Animate Ease in `flare-eval`,
  once the document stores keys (today a span holds one ease value).
## OpenToonz feature areas

Reference: the feature areas of OpenToonz (its rooms in `original/stuff/profiles/layouts/rooms`
and its panels). Phase: **Built** exists; **P1** next; **P2** after P1; **P3** later; **Not planned**
has no place in Flare. The panel rows are the planned panels of `flare-profile` (dimmed where they
appear, docs/UI.md); the profiles place them, but the interface does not read profiles yet.

| Area | OpenToonz panels | Flare status | Phase |
|---|---|---|---|
| Timeline | Timeline | Built (frames, layers, tweens) | Built |
| Xsheet | Xsheet | Planned panel; the timeline does the job today | P1 |
| Tool strip and options | ToolBar, ToolOptions | Built; More tools lists the unbuilt ones dimmed | Built |
| Workspace profiles | rooms, `layouts.txt` | Data model, loader and four built-in profiles in `flare-profile`; the interface does not read them yet | P1 |
| Shortcut sets | `shortcuts/*.ini` | Set ids named by profiles (`flare`, `opentoonz`, `animate`); key maps not built | P1 |
| Themes | `config/qss` (Clay, Synthwave, LiquidGlass, ...) | Eight palettes in `flare-ui` `theme.rs`, text at least 7:1 on every surface (tested) | Built |
| Preferences | Preferences | Built in part (General, Stage, Timeline, Recent) | Built |
| FX schematic and FX settings | Schematic, FxBrowser, FxSettings | Planned panels | P2 |
| Function editor | FunctionEditor | Planned panel | P2 |
| Level strip and palettes | LevelStrip, LevelPalette, StyleEditor | Planned panels | P2 |
| Scene cast and browser | SceneCast, Browser | Planned panels | P2 |
| Motion paths | MotionPath | Planned panel | P2 |
| Cleanup | Cleanup, CleanupSettings | Planned panels | P3 |
| Scan | Scan | Planned panel | P3 |
| Plastic (mesh deformation) | Plastic | Planned panel; stub in `flare-tools` `not_yet` | P3 |
| Tasks (batch render) | Tasks, BatchServers | Planned panel | P3 |
| Script console | ScriptConsole | Flare has no scripts; the Debug menu stays dimmed | Not planned |
## Stop motion

Stop motion is a capture session that a camera fills frame by frame, with an onion skin for
drawing against the previous frames. The domain is `flare-stopmotion` (`crates/editing`): the
frame rate, the hold each frame gets, the onion skin, the ghost frames while drawing, and the
export of the frames to a timeline as slots. It holds numbers, not pixels; the camera and the
captured images are the caller's. The session is not saved in a project yet.

Done: the session, frame list and holds with their bounds, onion skin with the ghost choice,
timeline export, the `CameraSource` trait with `NoCamera`, which answers capture and live view
with the named `NotYetImplemented` error, and the CLI and MCP commands that check settings and
name that error. Tested in the crate, with the failure paths.

Next, phased:
1. Capture backends: a `CameraSource` for a webcam and one for a DSLR over its SDK, each in its
   own adapter, with the encoded captures checked by `accept_capture`.
2. Live view: a preview of the camera in the stage, with the onion skin drawn over it.
3. Onion skin in the workspace: the ghost frames drawn in the editor, not only as numbers.
4. Audio cues: the sound a frame capture plays, and a cue for the next frame, both optional.
5. Export: the slots placed as keyframes on a layer, through the editing commands, and the
   captured images stored in the document.

Licence and reference: the old C++ code in `original/flare/sources/stopmotion` is the reference
for behaviour. The `brick-a-brack/eagle-animation` project was looked at as a possible reference,
but its licence could not be read in this session (the GitHub API refused the session and the raw
file was not found), so no code, art or structure is taken from it. If it is read later and its
licence allows, its use is recorded here with the source, licence and changes.

## Toonz import (`flare-toonz`, io layer)

Reads OpenToonz-lineage (Toonz Harlequin) files. Today: a typed reader and header sniffing, tested
with synthetic fixtures only. Not on any surface yet (see docs/SURFACES.md).

- [x] Phase 1, scene XML (`.tnz`): bounded XML tree, frame rate of the main output, camera count,
  level declarations, level and palette columns with cell runs (row, count, level, frame, step),
  sound columns, unknown columns kept by tag, reference and row-order checks, limits.
- [x] Phase 2, palette XML (`.tpl`, `.plt`): version, style count, style pages and their indices.
  Colours are not decoded.
- [x] Phase 3, sniffing: `.pli` (magic `PLIM`, or `MILP` from the big-endian writer) and `.tlv`
  (`TLV10`, `TLV11`, `TLV12`). Decoding returns NotYetImplemented.
- [ ] Phase 3b, vector `.pli` decoding: strokes, regions, styles.
- [ ] Phase 4, raster `.tlv` decoding: pixel data per format version.
- [ ] Phase 5, fx schematics: fx nodes and their ports in the scene.
- [ ] Phase 6, conversion into the Flare document through validated command batches. Not started;
  it returns NotYetImplemented until levels can be decoded.
- [ ] Phase 7, write-back and export to `.tnz`.
- [ ] Surfaces: `flare import-toonz` and an MCP tool that return the parse summary.

Gaps to close before the field names are trusted: the scene reader assumes level declarations carry
an `id`, a `name`, a `type` and a `path` child, and that cells name levels by that id. Camera
resolution is not read yet. Both were taken from the old Flare C++ (`original/flare/sources/flarelib`:
`txshlevelcolumn.cpp`, `levelset.cpp`, `txsheet.cpp`, `txshsoundcolumn.cpp`, `sceneproperties.cpp`;
`common/tvrender/tpalette.cpp`; `image/pli/pli_io.cpp`; `image/tzl/tiio_tzl.cpp`). The upstream
OpenToonz source was not consulted (its repository was not reachable from this session), so no
upstream code is ported and no upstream licence applies yet. A real `.tnz` from a user is the test
that these names are right.

## Moho projects (`flare-moho`)

Reads Moho (Lost Marble) `.moho`/`.anime` ZIP and `.mohoproj`/`.animeproj` JSON projects, and BVH
motion files. The format notes come from `original/flare/sources/common/moho` (read as reference,
no code copied). Upstream licences of SimplSam/moho-bvh-bones and jpdurigan/godot_moho_importer were
not checked (no GitHub access in that session), so nothing was borrowed from them.

- Phase 1, parse (done): container detection; typed model of layers, bones, switch cells, keyframe
  tracks, image and audio references; every size, depth and count bounded; golden tests on a
  synthetic fixture.
- Phase 2, bones (partly done): BVH reader and writer for the hierarchy and motion channels. Not
  yet: baking Moho's bone poses and keyframes into BVH frames.
- Phase 3, switch layers: resolve which child shows at each frame, and show it on every surface.
- Phase 4, art conversion: vector (mesh) and image layers into Flare shapes and bitmaps through
  validated command batches. Starts when `flare_moho::to_flare_document` can return a document.
- Phase 5, round trip: write a Moho project back, or export to BVH and XFL, and read it again unchanged.
- Gaps: pre-11 `.anme` is refused by name; audio reference keys are unverified against real projects;
  rendering is out of scope.

## Reference backlog

What the absorbed and upstream projects have that `main` lacks, checked against `origin/main` on
2026-11-04. The evidence, licences and work packages are in [BACKLOG.md](BACKLOG.md); this is the
summary. Nothing here is scheduled. The maintainer orders it.

- Small: editable keyboard shortcuts and shortcut sets (WP-1).
- Medium: Motion Editor and tween curves (WP-2, clean-room), Trace Bitmap and Shift and Trace (WP-3),
  exports for sprite sheets, animated SVG, Lottie and XDTS (WP-5), deformers and stroke styles
  (WP-6), drawing assistants (WP-7), timeline and symbol extras (WP-10).
- Large: SVG, Photoshop and image import (WP-4), filters and effects (WP-8; the EffectCraft
  dependency needs a decision), raster brush engine (WP-11; each preset needs a licence check).
- Extra-large, decision first: bones and inverse kinematics (WP-9), scripting (WP-12).
- Licences decide what can be ported: AnimationCraft and ArtCraft are MIT or Apache-2.0; OpenToonz
  and Tahoma2D are BSD-3-Clause; nflare and the FFDec worker in Next2Flash are GPLv3 (behaviour
  only); minnamate has no licence file (behaviour only).
- Replicraft was not found. The maintainer gives the URL.
