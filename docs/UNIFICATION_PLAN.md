# Unification plan: AnimationCraft + minnamate -> one Flare app

Status: plan only, no code changed. Paths are under `flare/sources/`. Sources: RUST_REWRITE.md,
ANIMATIONCRAFT_ABSORPTION.md (the file is spelled ANIMATION..., not ANIMATE...), PORTING.md, `Cargo.toml`, ORIGIN.md x3.
Line counts are `wc -l` as of this branch. "More complete" is a judgement from size, tests and call sites.

Policy tension to resolve first: ABSORPTION.md says "no parallel document model" and calls `model.rs` "do not import",
yet `flarelib/model.rs` (ac) and `flarelib/mm_doc.rs` (mm) both exist today. RUST_REWRITE.md "One application" names the
Qt `flare/` app the product. This plan treats the Rust egui `flare` binary as the interim product and ac `model.rs` as the
single Rust document until the TXsheet-shaped model lands. Maintainers should confirm (step 0).

## 0. Decision record (step 0)

Decided (recorded in this branch, pending maintainer sign-off in RUST_REWRITE.md): the Rust egui `flare` binary
(`ac-app`, bin `flare`) is the interim single app, and ac `flarelib/model.rs` is the single document model, as this plan
states. `mm_doc` stays only as the XFL-shaped side of the adapter `flarelib/mm_bridge.rs` (owned by `minnamate-core`,
which depends on `ac-core`; step 3 is implemented, lossy fields are listed in its module docs and returned as a
`BridgeReport`). Revisit when the TXsheet-shaped model lands.

## 1. Overlap table

| Area | AnimationCraft (ac) | minnamate (mm) | More complete |
|---|---|---|---|
| Document model | `flarelib/model.rs` 2383, `editor.rs` 4965, `frames.rs`, `ac_core.rs` (Keyframe, TweenBoundary, Swatch, ArtParams) | `flarelib/mm_doc.rs` 474 (XFL-shaped: Document/Timeline/Layer/Frame/Shape/Edge/Instance), `mm_layer_tree.rs`, `mm_arrange.rs`, `mm_symbols.rs`, `mm_scenes.rs` | ac (authoring model). mm_doc is a thin XFL mirror. |
| XFL read | `common/flash/ac_xfl_read.rs` 1170 (+edges, brush, dat, fuzz tests) | `flarelib/mm_xfl.rs` 606 | ac |
| XFL write | `common/flash/ac_xfl_write.rs` 963 | `flarelib/mm_xfl_write.rs` 707 | ac (check mm for .fla/CFBF quirks) |
| SWF read/write | `common/flash/ac_swf_read.rs` 756, `ac_swf_write.rs` 800, `ac_swf_font.rs` 704 | `common/tvrender/mm_swf_export.rs` 470, `sound/mm_sound_swf.rs` 374 (write only) | ac. mm adds only SWF sound blocks. |
| HTML5 export | `common/tvrender/ac_html5.rs` 983 + `html5_runtime.js` 857 | `mm_html5.rs` 379 + `mm_html5_runtime.js` 307 (runtimes differ ~1149 diff lines) | ac. mm runtime is the older/smaller. |
| Render | `ac_render.rs` 838, `ac_fx.rs` 503, `ac_effectcraft.rs` | `mm_render.rs` 378, `mm_shape_tween.rs` 636, `mm_bitmap.rs` 771, `mm_text.rs` 600, `mm_fonts.rs` | ac for render and fx. mm has unique shape tween, text and bitmap code. |
| Publish, video, encode | `flare/ac_app/publish.rs` 604, `ac_formats.rs` 819 | `mm_publish.rs` 525, `mm_encode.rs` 436, `mm_video.rs` 68, `mm_export.rs` 244 | Tie. mm publish-settings model is richer; ac has more formats. |
| Trace | `flarelib/trace.rs` 717, `centerline.rs` 466 | `tvrender/mm_trace.rs` 437 | ac |
| Audio | `sound/ac_audio.rs` 395 (+playback, rhubarb lip sync) | `sound/mm_sound.rs` 650 (mixer, volume envelopes, `FrameSound`) | mm for mixing and envelopes; ac for playback and lip sync. Merge. |
| Scripting | `common/expressions/ac_script.rs` 868 + `jsfl.js` 319 | `mm_script.rs` 1348 (+ `tnztools/tests/scripting.rs`) | mm is bigger (ActionScript dialect); ac has JSFL. Compare before merging. |
| Tweens | `flarelib/motion_clip.rs`, `ik.rs`, `rig.rs`, `scale9.rs` | `tparam/mm_motion.rs`, `flarelib/mm_layer_motion.rs` | ac |
| Draw/paint tools | `flare/ac_app/toolbar.rs`, `swatches.rs` | `tnztools/mm_pen/draw/paint/erase/hit/transform.rs` (~2.8k, pure logic, egui-free) | mm tools are the cleaner, reusable core. |
| Filters | `ac_fx.rs` | `flareqt/mm_filters.rs` 986 | mm for Flash-style filters. |
| Editor binary | `flare/ac_app_main.rs` 1622 + `ac_app/*` 18k, bin `flare` | `tnzext/minnamate_editor_main.rs` + `mm_ed_*` 28k, bin `minnamate` (egui 0.36 wgpu) | mm UI is larger and has Home, Library, Actions, Text, Free-transform. ac is the integrated one (docks, motion UI, scripting, rulers). |
| Studio chrome | `toonzqt/` 14k, bin `flare-studio` (theme, workspace docking, timeline) | `mm_ed_theme/widgets` | toonzqt is the target UX reference per RUST_REWRITE.md. |
| CLI | `flareqt/ac_cli_main.rs` + `commands.rs` 2313, bin `flare-cli` | `tcomposer/minnamate_cli_main.rs` 110, bin `minnamate-cli` | ac. mm CLI is a strict subset (render, swf, html5, image, gif, png-sequence, video, publish). |
| Dependencies | tiny-skia 0.11 | `minnamate-core` re-exports tiny_skia; eframe 0.36 | Check version skew (step 1). |

Structural fact: `tnztools/minnamate_core.rs` is a facade of `#[path]` modules scattered across 9 dirs. mm_* is therefore
compiled as ONE crate (`minnamate-core`) from files that sit inside other crates' directories. `ac-render` etc. also
compile those same dirs, so every `mm_*.rs` must stay out of the ac crates' `mod` lists.

## 2. Target design

App crate: `flare` (`ac-app`, bin `flare`) becomes the only editor binary. `toonzqt` (flare-studio) stays a library of
chrome/theme/docking that `flare` depends on (RUST_REWRITE.md says Studio merges in; do not ship it separately).
CLI: `flareqt` (`flare-cli`) is the only CLI. `minnamate` and `minnamate-cli` binaries are removed at the end.

Document: ac `model.rs` is the single model. `mm_doc` is not merged by hand; it becomes an adapter,
`flarelib/mm_bridge.rs` with `impl From<&mm::doc::Document> for ac::Document` and the reverse where lossless.
Rationale: the two models meet at XFL, and both already read/write it, so the XFL file is the lossless bridge. Smallest
safe sequence (bridge reports every lossy field, incl. renamed duplicate symbols, shape extras, dangling parents; see mm_bridge.rs header): (a) round-trip test mm_xfl -> bridge -> ac_xfl_write -> ac_xfl_read -> compare, (b) migrate mm-only
features one at a time onto ac types, (c) delete the mm reader/writer/doc when no mm consumer remains.

Panel registry: add to `flare/ac_app/dock.rs` (today a closed `enum Module {Tools, Properties, Library, Timeline}`):
```rust
pub trait Panel { fn id(&self) -> &'static str; fn title(&self) -> &str;
                  fn ui(&mut self, ui: &mut egui::Ui, cx: &mut PanelCx); }
pub struct Registry { panels: Vec<Box<dyn Panel>> }  // layout stores ids, not enum variants
```
`PanelCx` exposes `&mut ac::Document`, selection, history and the active tool; it never exposes mm types. Each
`mm_ed_*` panel (Library, Actions, Scene, Sound, Layer tree, Tweens, Publish, Filters, Text tool) is ported behind this
trait one at a time, reading ac types through a small mm-facing view (`ViewDoc`) until its logic is moved. `dock::Layout`
gains `#[serde(default)] panels: Vec<(String, DockSlot)>` so saved layouts stay valid.
Naming follows OpenToonz: panels go in `flare/ac_app/` now and move to their tool dirs as ports complete.

Shared code moves, mm file -> destination (all keep the `mm_` prefix until the merge step, then drop it):
`tnztools/mm_*.rs` stay in `tnztools/` and become tool impls on ac types; filters -> `stdfx/`; text/shape tween ->
`common/tvrender/`; mixer -> `sound/`; scripting dialect -> `common/expressions/`.

## 3. Ordered steps (each builds and passes tests alone; tier = who implements)

| # | Step | Tier | Files touched | Test | Risk |
|---|---|---|---|---|---|
| 0 | Maintainer decision: confirm ac `model.rs` is the single model; record in RUST_REWRITE.md | sonnet | `docs/RUST_REWRITE.md` | none | low |
| 1 | Add `cargo tree -d` and a workspace CI check that egui/eframe/tiny-skia resolve to one version; record skew | haiku | `Cargo.toml` | `cargo build --workspace` | low |
| 2 | Delete `mm_html5_runtime.js`; point `mm_html5.rs` at `html5_runtime.js` after diffing features; port any mm-only runtime features into ac runtime first | sonnet | `tvrender/mm_html5*.{rs,js}`, `html5_runtime.js` | `tvrender/tests/html5_filters.rs`, `runtime_stages.js` | med (runtime behaviour differs) |
| 3 | Add `flarelib/mm_bridge.rs` ac<->mm and the XFL round-trip test | sonnet | `flarelib/mm_bridge.rs`, `tests/` | new `mm_bridge` test plus `tnztools/tests/xfl_write.rs` | med (lossy fields) |
| 4 | Make `minnamate-cli` render/swf/html5/publish call ac readers/writers via bridge; keep its CLI flags | haiku | `tcomposer/minnamate_cli_main.rs` | cli smoke on demo doc | low |
| 5 | Fold mm CLI subcommands (`publish`, `video`, `gif`, `png-sequence`) into `flare-cli`; make `minnamate-cli` a 5-line alias then drop | sonnet | `flareqt/commands.rs`, `ac_cli_main.rs`, `Cargo.toml` | `cargo test -p ac-cli` | low |
| 6 | Merge audio: move `FrameSound` mixer and envelopes into `ac_audio.rs`, keep mm API as re-export | sonnet | `sound/ac_audio.rs`, `mm_sound.rs`, `mm_sound_swf.rs` | `sound/ac_audio_tests.rs`, `tnztools/tests/sound.rs` | med |
| 7 | Add `Panel` trait and `Registry` in `dock.rs`; port existing 4 modules to it without UI change | sonnet | `flare/ac_app/dock.rs`, `windows.rs`, `panels.rs` | `cargo test -p ac-app`, manual layout reset | med (layout serde) |
| 8 | Add `minnamate-editor` as a dependency of `flare` behind feature `mm-panels`; register first mm panel (Library) via bridge view | sonnet | `flare/Cargo.toml`, `tnzext/mm_ed_library.rs`, `ac_app/panels.rs` | launch `flare`, open Library | high (egui versions, state ownership) |
| 9 | Port remaining mm panels one per PR: Sound, Actions, Scene, Layer tree, Tweens, Filters, Publish, Text | haiku for thin ones (Sound, Scene), sonnet for rest | `tnzext/mm_ed_*.rs` | per-panel smoke plus existing unit tests | med |
| 10 | Unify scripting: diff `mm_script.rs` vs `ac_script.rs`; keep one dialect enum, move AS code under `ac_script` | sonnet | `common/expressions/*` | `ac_script_tests.rs`, `tnztools/tests/scripting.rs` | med |
| 11 | Move mm-only shape tween, text, bitmap, fonts into ac-render types (drop mm_bitmap/mm_text duplicates in `text.rs`, `bitmap.rs`) | sonnet | `tvrender/*`, `flarelib/bitmap.rs` | `tvrender/tests/*`, `tests/bitmaps.rs` | med |
| 12 | Move mm draw/erase/paint/hit/transform to operate on ac `Shape`; delete `mm_doc` consumers | sonnet | `tnztools/mm_*.rs` | `tnztools/tests/*` | high |
| 13 | Retire `minnamate` binary, `mm_doc`, `mm_xfl*`, `mm_swf_export`, `minnamate_core.rs` facade; rename remaining `mm_` files; update ORIGIN.md and PORTING.md | haiku | many deletions, `Cargo.toml`, docs | `cargo test --workspace` | med |
| 14 | Merge `toonzqt` chrome into `flare` (theme, workspace) per RUST_REWRITE.md | sonnet | `toonzqt/*`, `flare/ac_app/*` | `cargo test -p flare-studio` | high |

Step rule: never delete a duplicate until its replacement has a test that failed before the switch. Steps 2, 4, 5, 6 are
independent; 7 must precede 8; 3 must precede 4, 8 and 12.

## 4. This sprint

1. Step 1 (version skew audit): this decides whether step 8 is feasible and costs almost nothing.
2. Step 2 (single `html5_runtime.js`): the cheapest real de-duplication, small blast radius.
3. Step 3 (bridge plus XFL round-trip test): unblocks 4, 8 and 12, and measures how lossy the models really are.
4. Step 0 as a written decision before anyone starts step 7.
Do not start 8 to 14 this sprint. Licence: mm has no upstream LICENSE; get one before any mm code is relicensed or
published (ORIGIN.md), and keep `publish = false` until then.

## 5. Step 2 outcome: single `html5_runtime.js` NOT done (contracts incompatible)

Diffed `html5_runtime.ts` (ac, 918 lines) against `mm_html5_runtime.js` (307 lines). mm's runtime was left in place; nothing was ported. Exact incompatibilities:

1. Global and canvas: ac reads `window.AC_MOVIE` and `<canvas id="ac-stage">`; mm reads global `movie` and `<canvas id="movie">`, sets `window.exportRoot`.
2. Timeline model: ac plays one flat, precomputed `M.frames[]` of display lists (items refer to `M.items`, `M.paths` as SVG path strings, `M.clips`, `M.extra[f]`). mm has `movie.timelines[symbol].frames[]` with nested live MovieClip instances (`item.clip`, `item.k` keys, `item.b` button flag), which are advanced, synced and drawn recursively with per-clip colour transforms. Adapting means flattening nested symbols to per-frame lists on the Rust side (a new export pass, not a small adapter).
3. Scripts: mm `timelines[s].scripts[frame]` are JS functions called with `this` = the clip; ac `extra[f].js` are strings compiled with a CreateJS-style `this` (`scriptCache`). Different `this` APIs (mm: `scenes`, `frameOf`, `gotoAndPlay(frame, scene)`, `nextScene/prevScene`, `getChildByName` on live clips).
4. Shapes: mm `movie.shapes[i] = {f:[{d,c,g,b}], s:[{d,w,cap,join,c,g,b}]}` (fills and strokes inline, gradient `{t,m,f,s}`, bitmap fill `{i,m,r}`); ac uses `Item` records (`f`/`s`/`g`/`pat`/`pm`) over `M.paths`.
5. Masks: mm nests `{mask:[...], items:[...]}` in display lists; ac uses `Group.cb` / `M.clips` clip paths.
6. Images: mm `movie.images=[{src,w,h}]`; ac `M.images=[[src, smooth]]`.
7. Features: mm has no real button, sound, text, filter or motion-blur handling (button clips only flagged and never respond), so there is nothing unique to port from mm to ac except scenes with named-scene goto and nested symbol timelines, which ac lacks by design (it precomputes them).

Recommended next step: a Rust-side lowering pass (mm timelines -> ac `AC_MOVIE`), tied to step 3's bridge, then drop `mm_html5_runtime.js`.
