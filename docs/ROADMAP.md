# Roadmap

Status is evidence-based: a milestone item is done only when it has tests.

## Priorities

The high-priority list is the Roadmap in [README.md](../README.md) and is repeated here word for
word; the lower-priority list and what is not planned follow it. Priority is how much a person
animating with Flare misses the thing, not how hard it is. It was drawn up from a review of
`original/` (see [PARITY.md](PARITY.md)) and from Animate's own feature list, and it is the
maintainer's to reorder.

### Open GitHub issue triage (2026-11-11)

This checks the nine linked issues against the Rust workspace. Sizes are relative estimates;
priority is estimated against the roadmap above. Rows are ordered by priority, then size
(smallest first). Partial work does not close an issue.

| Issue | Priority | Size | Already built | Main gap |
|---|---|---:|---|---|
| [#11](https://github.com/Flare-Animate/Flare/issues/11) Custom toolbar | Medium | L | Built-in tools can be pinned, removed and saved in preferences | User-defined script actions, including the three JSFL workflows in the issue |
| [#25](https://github.com/Flare-Animate/Flare/issues/25) Editable vector brush | Medium | XL | Brush pressure profiles, nib settings and custom presets | Pencil strokes do not retain editable centerlines, pressure samples or width profiles |
| [#47](https://github.com/Flare-Animate/Flare/issues/47) FLA opening and workspace | Medium | XL | Modern FLA/XFL import, ZIP wrappers containing one `.fla`, and the shared open path; legacy binary FLA settings, library and layer metadata | Legacy artwork and timeline reconstruction; the reported BFDI sample is not verified |
| [#49](https://github.com/Flare-Animate/Flare/issues/49) Rigging | Medium | XL | Bones, keyed poses, forward kinematics and bounded inverse kinematics | Weighted mesh deformation, node constraints, master controllers and auto-patching |
| [#207](https://github.com/Flare-Animate/Flare/issues/207) ArtCraft UI modules | Low | M | Flare has its own tested egui/egui_tiles presentation stack | ArtCraft module reuse is unimplemented; the issue gives no reproducible UI defect |
| [#52](https://github.com/Flare-Animate/Flare/issues/52) JSFL and extensions | Low | XL | No JSFL or plugin integration | JSFL compatibility and safe ZXP/MXP plugin loading |
| [#53](https://github.com/Flare-Animate/Flare/issues/53) Studio pipeline | Low | XL | Dockable panels, the icon strip, named workspaces with a switcher, and four workspace profile definitions | Node compositing, shared asset linking and collaboration |
| [#123](https://github.com/Flare-Animate/Flare/issues/123) Interactive projects | Low | XL | Imported scripts are preserved as data; SWF playback is delegated to Ruffle | Authoring or running ActionScript 2/3 or JavaScript in Flare |
| [#230](https://github.com/Flare-Animate/Flare/issues/230) Flash 3D engines | Low | XL | SWF import identifies known engines and reports embedded binary blocks | Reconstructing 3D scenes or producing working source code from compiled SWFs |

The largest risks are model/format changes or untrusted code execution; those need separate
designs, fixtures and surface-specific tests. The issue links are cross-references, not a claim
that this documentation change resolves them.

### High priority

- Merge drawing: Flash's planar-map shapes, where overlapping shapes cut and merge each other (today every shape is a separate drawing object, though shapes drawn on a layer now share its keyframe)
- Drawing quality: brush dynamics (custom nib, drawn pressure curves, velocity and flow responses, a speed-aware stabilizer, quarter-sine tapers and inking-pen profiles are in; native Krita, Adobe and OpenToonz preset containers remain unsupported), and tracing a bitmap into vector shapes
- SWF that keeps more of the document: buttons with their states, shape tweens as morphs, masks of several shapes and sounds inside symbols. Flash 3D engine recognition on import is in (issue #230: the import names the engine a movie's scripts are written against, and counts its embedded binary blocks; running the movie stays Ruffle's job, and reading a script's code back out is not started)
- Motion Editor, remaining: per-property graphs (the segment editor with draggable handles, family, easing, handle kind, copy, paste and reset is in)
- Importing artwork from SVG and Photoshop files
- Bones and inverse kinematics (the Bone and Bind tools)
- Editable keyboard shortcuts: done in part (registry, four sets, editor, CLI and MCP; chords other than tool letters are still dispatched by fixed code, see SURFACES)

### Lower priority

Where Animate already has a feature (mask layers, the Width tool, Asset Warp rigging, lip sync, a camera layer) its version comes first and the improvements below build on it.

Drawing and tools
- The rest of Animate's tools: 3D Rotation and Translation, Deco, Width, Asset Warp and Gradient Transform
- Art and pattern brushes for the Paint Brush, and tablet tilt
- Layer depth (2.5D with the camera), and, for the camera, layers attached to it, following frames added to the movie and XFL import and export
- Rich text (mixed styles in one field) and text menu commands
- Bitmap compression settings; importing Illustrator (`.ai`) files
- Editable vector brushes: a stroke that keeps its centerline, pressure samples and a width profile, so its taper and curve can be changed after it is drawn, with reusable brush presets and a better width-profile editor
- Blender-style sculpt deformation for vector artwork
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

Formats
- Reading the artwork and keyframes of the old binary `.fla` (CS4 and before); today its stage, library and layer metadata are read
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

Android, P2P remote control, Adobe/Harman AIR and Flex SDK targets, and OpenToonz's
raster painting, FX schematic and xsheet. [PARITY.md](PARITY.md) says why for each.

| Milestone | Scope | Status |
|---|---|---|
| Game creation (`flare-game`; reverses the earlier Not planned entry): the entity-component world, its seven systems, game data in the project, runs under scripted input and export | The world, systems, commands, storage, `flare game`, MCP tools and the game export are built; the desktop and browser panel to edit and run a game is next. See [GAME.md](GAME.md) | In progress |
| M0 baseline | Pinned toolchain, centralized dependencies, locked builds, honest tests, renderer decision | Done |
| M1 source model and safe editing | Typed IDs, validated document, atomic edits, undo/redo, headless CLI | Done |
| M2 usable vertical slice | Draw, convert to symbol, instances, tween, scrub/play, save/reopen, PNG export, one modern workspace | Done in code; needs hands-on checking (see below) |
| M3 FLA/XFL compatibility and automation | Raw source IR, modern XFL/FLA subset, unknown-data preservation, MCP over stdio | MCP, XFL import, XFL export and element-level source preservation done (checked against fixtures and each other only, never against Animate) |
| M4 Test Movie and advanced features | Hosted Ruffle playback, SWF publishing, richer text/audio, advanced tweens and masks | SWF publishing (symbols kept as sprites, tweens baked) and Test Movie through an external Ruffle process done and tested without a real player; richer text/audio and masks not started |
| M5 hardening | Long sessions, crash recovery, packaging, accessibility, corpus expansion | Partly: crash recovery and fault-injection tests done; packaging, accessibility and long-session profiling not started |

## Required before a release (the maintainer's bar)

If drawing, exporting and playing do not all work well, Flare is not released. What is known to be missing against that bar, in the order it is meant to be tackled:

1. **MP4**: done, in pure Rust and desktop only (`flare-video`: `rusty_h264` for the picture, OxideAV's `oxideav-aac` for the sound, `muxide` for the container). Checked against the decoders of `rusty_h264` and `oxideav-aac` and, where one is installed, against ffprobe and ffmpeg. The sound encoder's one-frame delay is cancelled by holding the first picture that long (the container has no edit list), pinned by a test against the encoder itself. `rusty_h264` runs with its SIMD kernels (1.9 times faster than the scalar build on arm64, same bytes out; not measured on x86-64). Shipping H.264 and AAC encoders is accepted, as every media application does. Not measured: how the encoders hold up on long real animation and music.
2. **Symbols that behave like Animate's**: done in the model, the file, XFL, `flare-eval` and the Properties panel: a symbol is a Graphic, a Movie Clip (its own clock) or a Button (shows Up), a Graphic instance has Loop, Play Once, Single Frame, Loop Reverse and Play Once Reverse with a first frame (`symbol_frame`, property-tested), and `SetInstancePlayback` sets it as one undo step from every surface. Still to do: a Graphic's Last frame (Animate loops between First and Last), Button states beyond Up, a Movie Clip's instance name for scripts, filters, blend modes, cache-as-bitmap and 9-slice scaling grids.
3. **Merge drawing**, after **many elements in a keyframe** (done through stage 3 of [decisions/ELEMENTS.md](decisions/ELEMENTS.md): drawing no longer makes a layer per shape; stage 4, keeping a whole FLA's elements through save and open, and stage 5 remain): Flash's planar-map shapes, where overlapping shapes cut and merge each other and strokes and fills are edited separately. The largest gap in [PARITY.md](PARITY.md) and a change to the document model, not a feature on top of it.
4. **Camera**: done in code: a camera layer and tool whose keyframed position, zoom and rotation drive every export, the SWF and HTML5 pages, and a preview of the stage (real Ruffle draws the same picture as Flare). Still to do: someone using it by hand, XFL import and export of the camera, a camera that follows frames added to the movie, and attaching layers to it.
5. **Bones**: done in code: the Bone tool links symbol instances into an armature layer, the Selection tool poses it by inverse kinematics (joint limits, roots that move the whole armature), poses are keyframes that tween, and every export, SWF and HTML5 shows them; the command line and the MCP server do the same. Still to do: someone using it by hand, bones inside a shape and the Bind tool, XFL import and export of the armature itself (Animate's IK format), and pinning a joint while the rest is dragged.
6. **Timeline controls** good enough to animate with, and **Ruffle** playback proven against a real player.
7. **SWF that keeps its symbols**: symbols are published as sprites, each on the clock its playback says, and a real Ruffle plays them as Flare does (done). Still to do: buttons as buttons with their states (`DefineButton2`; today a Button is a sprite showing its Up state), shape tweens as morphs (`DefineMorphShape`; today each step is its own shape), masks of several shapes, and sounds inside symbols. The HTML5 page is still baked.
8. **Sharing the way Animate does it**: its Share menu logs in to YouTube and Twitter and uploads the exported video itself. That needs each service's API with credentials the project registers (YouTube is the practical first, now that MP4 exists); only Twitter's button is live (it fills in the post); the others are dimmed until then.

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
2. Fills and strokes: radial and multi-stop gradients, editing gradients, gradients and masks in XFL import and SWF, and the rest of the Paint Bucket: Lock Fill. It fills a shape, the inside of an outline whose ends meet or come within the chosen gap size (Animate's four, from none to 48 screen points), or any area lines close off, whatever drew them and however many shapes they are (a new shape under the lines), with the fill color or gradient; the Eyedropper takes a line's whole stroke or a fill's color or gradient, through empty insides and inside groups, and otherwise the color shown there. All three act on the press and are their own pointer over the stage.
3. Timeline UX: drag-moving selected frame ranges (one keyframe drags along its row or onto another layer, replacing what it lands on, and layers drag up and down the stack); sound: start/stop/loop options per cue, volume, playback that starts mid-sound (Animate's Stream sync), sounds in XFL and SWF. A loop range on the ruler, the waveform on a sound's bar, hearing a scrub, cells to the end of the longest sound, layer names and ruler that stay in view, and F5 on a layer with no frames are done.
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
- The Motion Editor edits a span's curve (`flare_tween::edit`: hit test, handle drag rules, view) through
  `SetKeyframeInterpolation`; per-property graphs over a whole `Curve` are the part left.
- Making the renderer use `flare-tween` for motion tweens in place of the Animate Ease in `flare-eval`,
  once the document stores keys (today a span holds one ease value).
- Done: classical raster inbetweening (`flare_tween::raster`, optical flow and warp, bounded and tested),
  wired as `flare tween bitmap-inbetweens` and the MCP tool `bitmap_inbetweens`, which write PNG frames
  between two bitmap keyframes. Next: a command that places those frames as keyframes, and the
  desktop control for it (SURFACES.md).
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
| Shortcut sets | `shortcuts/*.ini` | Four sets as data (`flare`, `opentoonz`, `animate`, `tahoma`) in `flare-menubar`'s `ShortcutMap`; edited, saved in Preferences, listed by `flare shortcuts` | P1 |
| Themes | `config/qss` (Clay, Synthwave, LiquidGlass, ...) | Eight palettes in `flare-ui` `theme.rs`, text at least 7:1 on every surface (tested) | Built |
| Preferences | Preferences | Built in part (General, Stage, Timeline, Recent) | Built |
| FX schematic and FX settings | Schematic, FxBrowser, FxSettings | Planned panels | P2 |
| Function editor | FunctionEditor | Built as the Motion Editor (one span's curve); per-property graphs are left | P2 |
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

Reads OpenToonz-lineage (Toonz Harlequin) files and converts scenes and vector levels to Flare
documents. Tested with synthetic fixtures built byte by byte in the tests; no real `.tnz`, `.pli`
or `.tlv` was available, so field names are taken from the OpenToonz source (BSD-3-Clause,
attribution in docs/ENGINEERING.md) and not yet checked against user files.

- [x] Phase 1, scene XML (`.tnz`): bounded XML tree, frame rate, camera count, camera resolution
  and size, level declarations, level, palette and sound columns with cell runs, limits.
- [x] Phase 2, palette XML (`.tpl`, `.plt`): styles in file order with real colours (solid colour
  styles, tag id 3; other style kinds keep their id and no colour), pages and indices.
- [x] Phase 3, sniffing: `.pli` (`PLIM`/`MILP`) and `.tlv` (`TLV10`..`TLV12`).
- [x] Phase 3b, vector `.pli` decoding (version 5.7 and later): thick quadratic chains, groups,
  colour tags, precision scale, palette style tags. Filled regions are counted, not decoded.
- [ ] Phase 4, raster `.tlv` decoding (LZO-compressed tiles and the icon and frame tables).
  Returns NotYetImplemented; a scene with raster levels converts with empty cells and a named note.
- [ ] Phase 5, fx schematics: fx nodes and their ports in the scene.
- [x] Phase 6, conversion: frame rate, camera resolution as the stage, a layer per level column
  (last column on top), a symbol per drawing with strokes grouped by style, drawn as cubic paths at
  the mean thickness. Sound columns and raster levels are named as unsupported. Level files are
  found beside the scene (`+drawings/X` in the scene's folder or its parent); paths that climb out
  are never read. One stage inch is taken as 53.33 level units.
- [ ] Phase 7, write-back and export to `.tnz`.
- [x] Surfaces: `flare import` and MCP `import_project` take `.tnz` and `.pli`; the desktop and
  browser open them.

## Moho projects (`flare-moho`)

Reads Moho (Lost Marble) `.moho`/`.anime` ZIP and `.mohoproj`/`.animeproj` JSON projects, and BVH
motion files, and converts both to Flare documents. The format notes come from
`original/flare/sources/common/moho` (read as reference, no code copied). Licences checked:
jpdurigan/godot_moho_importer is MIT, so its mesh layout (`mesh.points`/`mesh.curves`) and the
`anim_pos`/`anim_angle`/`anim_scale` track names are used with attribution here;
SimplSam/moho-bvh-bones is CC BY-NC-SA 4.0, so nothing was taken from it (behaviour only).

- Phase 1, parse (done): container detection; typed model of layers, bones, switch cells, keyframe
  tracks, mesh points and curves, image and audio references; every size, depth and count bounded.
- Phase 2, bones (done): BVH reader and writer, and `bvh::to_document` (a pose per frame per joint).
- Phase 3, switch layers (done): switch keys become one instance span per key.
- Phase 4, conversion (done, with gaps): `to_flare_document_with` builds one validated command batch:
  groups as folders, vector layers as shape symbols (polylines, with the first filled shape's colour)
  placed by translation, rotation and scale tracks, bones as folders with pose layers (Moho 13 keeps
  each skeleton in its bone layer; forward parents are resolved), switch layers as one symbol per
  alternative, and image layers as bitmaps from the project's folder (PNG only, see MOHO.md).
  Audio layers are named as unsupported. Moho units are assumed two per stage height.
- Phase 5, round trip: write a Moho project back, or export to BVH and XFL, and read it again unchanged.
- Gaps: bone-bound layers keep their own placement (the bone pose is not applied); actions and
  interpolation other than linear and hold; Bezier handles; audio; pre-11 `.anme` is refused by name;
  the audio key names are unverified. The verified and spec-only split is in [MOHO.md](MOHO.md).

## Reference backlog

What the absorbed and upstream projects have that `main` lacks, checked against `origin/main` on
2026-11-04. The evidence, licences and work packages are in [BACKLOG.md](BACKLOG.md); this is the
summary. Nothing here is scheduled. The maintainer orders it.

- Small: editable keyboard shortcuts and shortcut sets (WP-1, built except the chord dispatch).
- Medium: Motion Editor and tween curves (WP-2, clean-room), Trace Bitmap and Shift and Trace (WP-3),
  exports for sprite sheets, animated SVG, Lottie and XDTS (WP-5), deformers (WP-6), drawing assistants (WP-7), timeline and symbol extras (WP-10).
- Large: Photoshop and image import (WP-4; SVG import is done, it needs a bitmap layer in the model first), filters and effects (WP-8; the EffectCraft
  dependency needs a decision), raster brush engine (WP-11; each preset needs a licence check).
- Extra-large, decision first: bones and inverse kinematics (WP-9), scripting (WP-12).
- Licences decide what can be ported: AnimationCraft and ArtCraft are MIT or Apache-2.0; OpenToonz
  and Tahoma2D are BSD-3-Clause; nflare and the FFDec worker in Next2Flash are GPLv3 (behaviour
  only); minnamate has no licence file (behaviour only).
- Replicraft was not found. The maintainer gives the URL.

### ViewSpace Keys (experimental)

`flare_core::experimental::viewspace` (unstable, not in the document): multi-view 2D interpolation between sparse keyframes registered to a base mesh and semantic features, with confidence, expressions, clothing states and corrective keyframes. Phases 0 to 2 of the CPU reference path are built and tested; GPU, strokes, imports, exports, rig integration, auto depth and the tab are stubs. The spec, status and every stub id are in [experimental/VIEWSPACE.md](experimental/VIEWSPACE.md).

### Symbols and rigging

Done: symbol kinds and Graphic playback (above); `flare-rig` with bones, angle limits, keyed poses eased by `flare-tween`, FK, two-bone and CCD IK, bounded and property-tested; `flare symbol` / `flare rig` and the matching MCP tools on a rig JSON file.
Next, in order: store rigs in the document and its file (a `rigs` field with versioned defaults) with `CreateSymbolRig`, `AddBone`, `SetBoneAngleLimit` and `SetRigPose` as commands and undo steps; make bones drive their target instances in `flare-eval`; XFL and FLA symbol options already import, rig import does not; mesh deformation with skinning weights (after OpenToonz's Plastic deformer, BSD-3, attributed); weight-painting UI; IK solver UI with FABRIK; motion paths; skeletons from Moho and OpenToonz `.plastic`; a Rig panel in the desktop app and browser.
