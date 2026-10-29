# Roadmap

Status is evidence-based: a milestone item is done only when it has tests.

| Milestone | Scope | Status |
|---|---|---|
| M0 baseline | Pinned toolchain, centralized dependencies, locked builds, honest tests, renderer decision | Done |
| M1 source model and safe editing | Typed IDs, validated document, atomic edits, undo/redo, headless CLI | Done |
| M2 usable vertical slice | Draw, convert to symbol, instances, tween, scrub/play, save/reopen, PNG export, one modern workspace | Mostly done: see below |
| M3 FLA/XFL compatibility and automation | Raw source IR, modern XFL/FLA subset, unknown-data preservation, MCP over stdio | Not started |
| M4 Test Movie and advanced features | Hosted Ruffle playback, SWF publishing, richer text/audio, advanced tweens and masks | Not started: needs a tested Ruffle revision chosen and an SWF writer, and has no evidence base yet |
| M5 hardening | Long sessions, crash recovery, packaging, accessibility, corpus expansion | Partly: recovery on `feat/recovery`, fault-injection tests on `feat/hardening`; packaging, accessibility and long-session profiling not started |

## M2 remaining

- Look at and fix the desktop window and the native macOS menu; both have only been built and launch-tested, not exercised by hand.
- Docking and a saved workspace layout.
- Pen pressure, IME and accessibility checks on real hardware.
- Autosave and crash recovery; migration and fault-injection tests for storage.

## Branches on top of this one

Each is a small, independent PR off `rewrite-from-scratch` and passes the full checks on its
own. They touch some of the same files (`workspace.rs`, `actions.rs`), so expect small
merge conflicts when combining them.

| Branch | What it adds |
|---|---|
| `feat/recovery` | Autosave to a recovery store, recover on launch, discard on confirmed close |
| `feat/mcp` | MCP stdio server (`flare-cli mcp`) with permissions, revision checks, robustness tests |
| `feat/xfl-import` | XFL / modern FLA import subset (`flare-cli import`) with diagnostics, robustness tests |
| `feat/docking` | `egui_tiles` dock with a validated, remembered layout and Reset Workspace Layout |
| `feat/onion-skin` | Onion skinning with before/after ranges |
| `feat/hardening` | Fault-injection and random-edit invariant tests |

## Not feasible without more input

- Pen pressure, IME and accessibility need real hardware and a person; AccessKit is not on.
- Hosted Ruffle playback and SWF publishing need a Ruffle revision chosen and tested.
- Compatibility claims need real Animate files; the XFL importer has only seen authored fixtures.
- App icons must be trimmed of their black corners before they go in `/assets` (packaging).

## Next independent branches

Each of these can be its own branch and PR on top of the current work:

1. Docking prototype (`egui_tiles` against `egui_dock`) and persisted layout.
2. Drawing tools: pen/path, selection of multiple objects, transform handles.
3. Fills and strokes: gradients, stroke hit-testing, paint bucket and ink bottle behaviour.
4. Timeline UX: span selection and drag, copy/paste, onion skin, zoom.
5. Recovery: autosave, crash recovery, storage migration tests.
6. Compatibility evidence: a lawful fixture corpus, then XFL import for a named subset.
7. Input and accessibility: pen, IME and screen-reader results.
