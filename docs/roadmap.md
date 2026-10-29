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
| `feat/xfl-import` | XFL / modern FLA import subset (`flare-cli import`) with diagnostics, robustness tests |
| `feat/xfl-export` | XFL / `.fla` export (`flare-cli export-xfl`), stacked on `feat/xfl-import` |
| `feat/source-preservation` | Keep unsupported XFL data as attachments through edit, save and export; stacked on `feat/xfl-export` |
| `feat/test-movie` | Test Movie via an external Ruffle process, stacked on `feat/swf-publish` |
| `feat/swf-publish` | SWF publishing (`flare-cli publish-swf`), stacked on `feat/xfl-export` |
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
