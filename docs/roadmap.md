# Roadmap

Status is evidence-based: a milestone item is done only when it has tests.

| Milestone | Scope | Status |
|---|---|---|
| M0 baseline | Pinned toolchain, centralized dependencies, locked builds, honest tests, renderer decision | Done |
| M1 source model and safe editing | Typed IDs, validated document, atomic edits, undo/redo, headless CLI | Done |
| M2 usable vertical slice | Draw, convert to symbol, instances, tween, scrub/play, save/reopen, PNG export, one modern workspace | Done in code; needs hands-on checking (see below) |
| M3 FLA/XFL compatibility and automation | Raw source IR, modern XFL/FLA subset, unknown-data preservation, MCP over stdio | MCP, XFL import, XFL export and element-level source preservation done (checked against fixtures and each other only, never against Animate) |
| M4 Test Movie and advanced features | Hosted Ruffle playback, SWF publishing, richer text/audio, advanced tweens and masks | SWF publishing (baked frames) and Test Movie through an external Ruffle process done and tested without a real player; richer text/audio and masks not started |
| M5 hardening | Long sessions, crash recovery, packaging, accessibility, corpus expansion | Partly: crash recovery and fault-injection tests done; packaging, accessibility and long-session profiling not started |

## Required before a release (the maintainer's bar)

If drawing, exporting and playing do not all work well, Flare is not released. What is known to be missing against that bar, in the order it is meant to be tackled:

1. **MP4**: done, in pure Rust and desktop only (`flare-video`: `rusty_h264` for the picture, OxideAV's `oxideav-aac` for the sound, `muxide` for the container). Checked against the decoders of `rusty_h264` and `oxideav-aac` and, where one is installed, against ffprobe and ffmpeg. The sound encoder's one-frame delay is cancelled by holding the first picture that long (the container has no edit list), pinned by a test against the encoder itself. `rusty_h264` runs with its SIMD kernels (1.9 times faster than the scalar build on arm64, same bytes out; not measured on x86-64). Shipping H.264 and AAC encoders is accepted, as every media application does. Not measured: how the encoders hold up on long real animation and music.
2. **Symbols that behave like Animate's.** Today every symbol is evaluated as a Graphic that loops with its parent (`flare-eval`), so a Button plays all of its frames instead of showing its Up state, a Graphic has no Loop, Play Once or Single Frame option with a first frame, and a Movie Clip has no clock of its own beyond that loop. Needs the instance's playback options in the document model, the file format and XFL, Properties controls, and the evaluator.
3. **Merge drawing**: Flash's planar-map shapes, where overlapping shapes cut and merge each other and strokes and fills are edited separately. The largest gap in [parity.md](parity.md) and a change to the document model, not a feature on top of it.
4. **Camera**: a camera layer and tool whose keyframed position, zoom and rotation drive the stage view and every export.
5. **Timeline controls** good enough to animate with, and **Ruffle** playback proven against a real player.
6. **SWF that keeps its symbols**: publishing today bakes every frame (tweens and symbols are flattened). A published movie should keep symbols as sprites, buttons as buttons and tweens as tweens (`DefineSprite`, `DefineButton2`, `DefineMorphShape`), so a SWF made by Flare plays and edits like one made by Animate.
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
