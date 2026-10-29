# Roadmap

Status is evidence-based: a milestone item is done only when it has tests.

| Milestone | Scope | Status |
|---|---|---|
| M0 baseline | Pinned toolchain, centralized dependencies, locked builds, honest tests, renderer decision | Done |
| M1 source model and safe editing | Typed IDs, validated document, atomic edits, undo/redo, headless CLI | Done |
| M2 usable vertical slice | Draw, convert to symbol, instances, tween, scrub/play, save/reopen, PNG export, one modern workspace | Mostly done: see below |
| M3 FLA/XFL compatibility and automation | Raw source IR, modern XFL/FLA subset, unknown-data preservation, MCP over stdio | Not started |
| M4 Test Movie and advanced features | Hosted Ruffle playback, SWF publishing, richer text/audio, advanced tweens and masks | Not started |
| M5 hardening | Long sessions, crash recovery, packaging, accessibility, corpus expansion | Not started |

## M2 remaining

- Look at and fix the desktop window; it has only been launch-tested.
- Docking and a saved workspace layout.
- Pen pressure, IME and accessibility checks on real hardware.
- Autosave and crash recovery; migration and fault-injection tests for storage.

## Next independent branches

Each of these can be its own branch and PR on top of the current work:

1. Native macOS menu (muda) and the shared menu actions.
2. Docking prototype (`egui_tiles` against `egui_dock`) and persisted layout.
3. Drawing tools: pen/path, selection of multiple objects, transform handles.
4. Fills and strokes: gradients, stroke hit-testing, paint bucket and ink bottle behaviour.
5. Timeline UX: span selection and drag, copy/paste, onion skin, zoom.
6. Recovery: autosave, crash recovery, storage migration tests.
7. Compatibility evidence: a lawful fixture corpus, then XFL import for a named subset.
8. Input and accessibility: pen, IME and screen-reader results.
