# The Flare Rust Rewrite

How Flare's C++/Qt codebase is being transformed into Rust, the rules that govern the migration, and the external projects whose code is being absorbed into the port. This document describes architecture, process, and policy rather than a delivery schedule.

## Why

Flare is a fork of OpenToonz. OpenToonz is a large, mature C++/Qt application containing decades of production-tested animation tooling. Throwing that work away and building an entirely new editor would be wasteful, so the rewrite preserves the upstream architecture while replacing implementations with Rust.

The objective is not merely to write new Rust modules that resemble the existing code. The objective is to produce a Rust implementation of the OpenToonz/Tahoma2D codebase while preserving the structure, layout, naming, responsibilities, and behaviour of the original project.

As a general rule, every C++ source file should eventually have a Rust equivalent occupying the same role within the application's architecture. Exceptions are allowed only where retaining a non-Rust implementation is demonstrably safer, more practical, or technically superior for its specific use case, such as selected scripting, tooling, or build-integration components.

The port is therefore **structural, incremental, and fidelity-driven**:

- Directory layouts remain aligned with OpenToonz/Tahoma2D.
- Module boundaries remain aligned with OpenToonz/Tahoma2D.
- File responsibilities remain aligned with OpenToonz/Tahoma2D.
- Public behaviour remains aligned with OpenToonz/Tahoma2D.
- Implementations are rewritten in Rust, replacing the original files, and match those files down to each line of code.
- Once changes are stable, the original script can be completely removed to the codebase, provided everything is ported over from it and it hasn't been subject to newer changes.

Where practical, a Rust port should mirror the source file it replaces as closely as possible, preserving intent, logic flow, naming, variables, line content, and behaviour so that the relationship between the original implementation and its replacement remains obvious.

The goal is a single application, not multiple competing products.

## The central rule: a ported module lives where the original was

There is no separate Rust codebase and **no `rust/` subfolder**. A ported file takes the original's name and directory with the `.rs` extension:

```text
flare/sources/<module>/<name>.cpp  ->  flare/sources/<module>/<name>.rs
```

Examples already in place: `common/flash/{swf,xfl,fcp,framecount,xflrender,symlib,cfbf}.rs`, `common/moho/moho.rs`, `common/tapptools/tenv.rs`, `common/tsystem/tfilepath.rs`, `tnzbase/permissionsmanager.rs`, `sound/tsioutils.rs`. The original `.cpp` is deleted once its Rust replacement is verified; headers remain only as thin C-ABI forwarders while C++ callers exist.

The temporary crate root is `flare/sources/flare_formats.rs` (scaffolding that disappears with the last C++ file). The imported AnimationCraft editor lives in the matching module directories (`flarelib/`, `toonzqt/`, ...; see ANIMATIONCRAFT_ABSORPTION.md). There is no `rust/` folder.

### Crate roots

A crate is the directory holding its `Cargo.toml`; the root `.rs` is named by `[lib] path`.

| Crate | Directory (`flare/sources/`) | Manifest / root |
|---|---|---|
| `flare_formats` (cdylib/staticlib, C ABI) | `.` | `Cargo.toml` / `flare_formats.rs` |
| `ac-core` | `flarelib/` | `Cargo.toml` / `ac_core.rs` (tests in `tests/`, `assets/mypaint/`, `ORIGIN.md`) |
| `minnamate-core` | `tnztools/` | `Cargo.toml` / `minnamate_core.rs` (tests in `tests/`) |
| `flare_remote` | `remote/` | `Cargo.toml` / `remote.rs` |
| `ac-render` | `common/tvrender/` | `Cargo.toml` / `ac_render.rs` (`ac_{fx,formats,html5,fonts,effectcraft}.rs`; `ac_formats.rs` also holds the MaxRects texture atlas, `html5_runtime.js`, `tests/`, `examples/`) |
| `ac-xfl` | `common/flash/` | `Cargo.toml` / `ac_xfl.rs` (`ac_xfl_{read,write,edges,brush,dat,tests}.rs`, `tests/fixtures/`; SWF: `ac_swf.rs` container + character ids, `ac_swf_{read,write,font,tests}.rs`, from the Next2Flash port, see `docs/NEXT2FLASH_INTEGRATION.md`) |
| `ac-audio` | `sound/` | `Cargo.toml` / `ac_audio.rs` (`ac_audio_{playback,rhubarb,tests}.rs`) |
| `ac-script` | `common/expressions/` | `Cargo.toml` / `ac_script.rs` (`ac_script_tests.rs`, `jsfl.js`) |
| `ac-package` | `common/tsystem/` | `Cargo.toml` / `ac_package.rs` |
| `ac-app` (bin `flare`, egui) | `flare/` | `Cargo.toml` / `ac_app_main.rs` (modules `ac_app/*.rs`, `build.rs`, `assets/`) |
| `ac-cli` (bin `flare-cli`) | `flareqt/` | `Cargo.toml` / `ac_cli_main.rs` (`examples/`) |
| `flare-studio` (bin `flare-studio`, egui/wgpu) | `toonzqt/` | `Cargo.toml` / `studio_lib.rs`, `studio_main.rs` (`locales/`, `assets/`, `snapshot.rs` example) |
| `minnamate-editor` (bin `minnamate`, egui 0.36, edition 2024) | `tnzext/` | `Cargo.toml` / `minnamate_editor_main.rs` (modules `mm_ed_*.rs` via `#[path]`, `assets/logo.png`, `ORIGIN.md`; no upstream licence) |
| `minnamate-cli` (bin `minnamate-cli`) | `tcomposer/` | `Cargo.toml` / `minnamate_cli_main.rs` (`ORIGIN.md`) |

CMake imports `flare_formats` via `cmake/FlareFormats.cmake` (always on).

Two rules follow directly from this structure:

### 1. Every Rust file must be registered

Every ported module must be registered in:

```rust
flare/sources/flare_formats.rs
```

using the appropriate `#[path]` declaration.

Unregistered modules compile successfully yet remain unused, making this the most common form of silent failure during migration.

### 2. Flare remains one workspace

The project manifest is:

```text
flare/sources/Cargo.toml
```

There are no independent Rust projects nested within the tree.

Tests are executed through:

```text
python tests/run_rust.py
```

---

## Language split

| Layer | Language | Rationale |
|---------|---------|---------|
| Core engine, backend, rendering, document model | **Rust** | Primary implementation language moving forward. |
| Tooling, web-facing utilities, importer/exporter workflows | **TypeScript** | Based largely on Next2Flash tooling patterns. |
| Small automation and orchestration tasks | **Python** | Existing tooling and CI infrastructure already depend on it. |

New engine-level work defaults to Rust unless a clear technical reason exists for another language.

## Code absorption and redistribution

The rewrite is not solely a language migration. It is also a consolidation effort.

Three projects serve as major sources of functionality, ideas, implementations, or workflows:

- AnimationCraft
- Next2Flash
- NFlare

Their code should not exist as isolated imported subsystems.

Instead, equivalent functionality should be mapped onto the Rust implementations of the OpenToonz/Tahoma2D architecture.

### AnimationCraft

AnimationCraft's codebase is treated as a source of Rust-native animation technology.

Rather than embedding AnimationCraft as a standalone component, its systems should be decomposed and redistributed throughout Flare's Rust architecture, with functionality mapped into the corresponding editor, rendering, animation, timeline, document, asset, and playback systems.

The objective is that no useful AnimationCraft functionality is discarded simply because its original project structure differs from OpenToonz.

### Next2Flash

Next2Flash serves as the primary reference for TypeScript-based Flash tooling.

Importer, exporter, workflow, asset-processing, and web-adjacent functionality should be integrated into the corresponding Flare subsystems rather than preserved as a separate product inside the repository.

### NFlare

NFlare remains a behavioural and architectural reference.

Its concepts, workflows, and implementation ideas can guide equivalent systems within Flare's Rust architecture.

Where licensing permits, functionality should be integrated into the corresponding subsystem rather than duplicated in parallel.

The overall objective is a unified codebase in which AnimationCraft, Next2Flash, NFlare, and OpenToonz contributions converge into a single coherent application instead of remaining as distinct projects.

## One application

The Qt application:

```text
flare/sources/flare/
```

is the product.

The Studio workspace:

```text
flare/sources/toonzqt/
```

began life as a separate executable but is being merged into the main application rather than being shipped as a second editor.

When Studio systems migrate, they should move into the subsystems they implement rather than remaining in a dedicated tree.

Studio also acts as the reference for many target UX features, including:

- Animate-style workflows
- Timeline interaction
- Layer management
- Docking systems
- Workspace management
- Theming and customization

The long-term direction is convergence toward a single integrated editor experience.

**UI consolidation (three egui front ends).** As imported, the tree has three egui
applications: `flare/` (ac-app, bin `flare`, eframe 0.29 - AnimationCraft's editor and
the interim product), `toonzqt/` (flare-studio, bin `flare-studio`, eframe 0.36), and
`tnzext/` (`minnamate-editor`, bin `minnamate`). `toonzqt` and `tnzext` are libraries to
be folded into `flare`; neither stays a shipped second editor. The first blocker is the
egui generation split (0.29 vs 0.36), so step 1 of `docs/UNIFICATION_PLAN.md` is executed
first: `flare/sources/Cargo.toml` now carries one `[workspace.dependencies]` pin per
shared direct dep (egui/eframe/egui_extras pinned to the 0.36 target, plus image, zip,
flate2, roxmltree, usvg, resvg, rfd) so the fronts cannot drift further. The ac-app
0.29 -> 0.36 migration that lets `flare` consume `toonzqt`'s chrome is step 14 and remains
open; the ordered sequence is in `docs/UNIFICATION_PLAN.md`.

## Upstream sync must accelerate the port

Flare continues tracking upstream OpenToonz and Tahoma2D.

The sync system:

```text
tools/sync/upstream_agent.py
```

must do more than replay upstream commits.

When an upstream change affects code that has already been ported to Rust, the sync process should automatically generate a draft pull request that attempts to map the upstream change onto the Rust implementation of the affected subsystem.

The purpose is not to fully automate porting.

The purpose is to provide maintainers with:

- Change detection.
- Candidate Rust-side modifications.
- Affected-file mapping.
- Draft implementation suggestions.
- Context for manual review.

This creates a head start whenever upstream functionality changes.

The ideal workflow is:

1. Upstream modifies a C++ file.
2. Sync detects the corresponding Rust replacement.
3. Sync analyses the change.
4. Sync proposes a Rust-side equivalent.
5. Sync opens a draft PR for developer review.
6. Developers finalize the port manually.

The upstream sync therefore becomes a migration assistant rather than merely a source mirror.

### Rebrand protection

Two rule sets remain critical:

- Shared rebrand rules rewrite includes and branding.
- CMake-specific rebrand rules rewrite build-system identifiers.

These rules are enforced by tests because broken rebranding can render every supported platform unbuildable.

`tools/sync/tests/test_upstream_agent.py` pins these transformations to prevent regressions.

## Licensing rules for absorbed work

Absorbing functionality is not the same as copying source verbatim.

Flare remains BSD-3-Clause.

### Permissive licences

MIT, BSD, and Apache licensed projects may be incorporated, provided attribution and licence requirements are respected.

AnimationCraft falls into this category and may be used as a source for redistributed subsystem implementations.

### Copyleft licences

GPL, LGPL, and AGPL code may not simply be copied into BSD-licensed Flare components.

Projects such as NFlare may be referenced for behaviour, architecture, workflows, and implementation guidance, but licence obligations must always be respected.

### Missing licences

Projects lacking an explicit licence grant should be treated as unavailable for direct inclusion until permission is clarified.

### Origin tracking

Every absorbed source must be recorded in:

```text
ORIGIN.md
```

including upstream repository and commit information.

## Keeping the documentation honest

`LOGIC-ATLAS.md` remains the authoritative architecture map.

`scripts/atlas_check.py` validates:

- Paths
- Symbols
- Constants
- Structural references

against the actual source tree.

Whenever a subsystem changes:

1. The code changes.
2. The atlas changes.
3. Validation passes.

Documentation should never silently drift away from implementation reality.

## What "done" means for a module

A module is considered fully ported when:

1. A Rust implementation exists under:

   ```text
   <module dir>/<name>.rs  (same dir as the .cpp it replaces)
   ```

2. The module is registered in:

   ```text
   flare_formats.rs
   ```

3. The production code path uses the Rust implementation.
4. The old C++ implementation is removed or retired.
5. Behaviour is covered by Rust tests.
6. Historical edge cases are covered.
7. `LOGIC-ATLAS.md` is updated.
8. `python scripts/atlas_check.py` succeeds.
9. Upstream-sync mapping exists for the subsystem where applicable.

A compiled-but-unused Rust implementation is not ported.

A Rust implementation running alongside an active C++ equivalent is not ported.

A module is only finished when the Rust version becomes the authoritative implementation.

## Reference

| Source | Role |
|----------|----------|
| AnimationCraft (`KMSDax`) | Primary Rust-native animation, document, rendering and timeline functionality source |
| Next2Flash (`SSF2-Mods-Official`) | TypeScript tooling and Flash workflow reference |
| NFlare | Architectural and behavioural reference |
| OpenToonz / Tahoma2D | Upstream structure, module layout, and subsystem architecture |

### Candidate absorption sources (requested 2026-10-16)

These are queued as sources of functionality, ideas or workflows, mapped onto the
matching Rust subsystem rather than vendored as separate products (see "Code absorption
and redistribution"). Licence must be confirmed before any code is copied; until then they
are reference-only.

| Source | Intended subsystem | Licence |
|----------|----------|----------|
| [ruffle-rs/rust-flash-lso](https://github.com/ruffle-rs/rust-flash-lso) | Flash Local Shared Object (.sol) reads/writes in `common/flash` (fills the SOL gap in FEATURE_STATUS #9) | MIT, **absorbed** (`common/flash/lso.rs`) |
| [ruffle-rs/ruffle](https://github.com/ruffle-rs/ruffle) | SWF/AVM behaviour already cross-referenced in `common/flash/*` | MIT/Apache-2.0 |
| [storytold/effectcraft](https://github.com/storytold/effectcraft) | Effects already consumed via `effectcraft-*` deps, rendered by `common/tvrender/ac_effectcraft.rs` | verify |
| [storytold/vectorcraft](https://github.com/storytold/vectorcraft) | Vector path/merge model (Flash planar shapes, `docs/ANIMATIONCRAFT_ABSORPTION.md`) | verify |
| [storytold/craft-libs](https://github.com/storytold/craft-libs) | Shared craft crate helpers (cross-check against effectcraft/vectorcraft) | verify |
| [mirvoxtm/minnamate](https://github.com/mirvoxtm/minnamate) | Imported as `minnamate-core` (tnztools); merge tracked in `docs/UNIFICATION_PLAN.md` | none upstream; `publish = false` until cleared |
| [anthony-hyo/animaty](https://github.com/anthony-hyo/animaty) | Animation curves/tweening ideas for `common/tparam`, `flarelib/motion_clip.rs` | verify |
| [Bauxitedev/bitmapflow](https://github.com/Bauxitedev/bitmapflow) | Bitmap morphing / inbetweening for the bitmap pass-through path | verify |
| [ruvnet/Rash](https://github.com/ruvnet/Rash) | Scripting/tooling reference only (Rash is a shell-agent framework, not an animation lib) | verify |

In short, Flare's end state is a unified Rust implementation of the OpenToonz/Tahoma2D architecture, augmented by functionality derived from AnimationCraft, Next2Flash, and NFlare, while preserving upstream compatibility, subsystem structure, and long-term maintainability.


### Ported so far (branch `Conversion-ft-animationCraft`)
- `tnzbase/permissionsmanager.rs` (cpp deleted; header forwards via C ABI). Tests: `cargo test --lib permissions`.
- `sound/tsioutils.rs` (cpp deleted; header is extern "C"). Tests: `cargo test --lib tsioutils`.
- Wave 3 (11 files, +62 tests; workspace total 683 -> 745): `common/tcore/{tmathutil,trandom}.rs`, `common/tparam/tcubicbezier.rs`, `common/tcolor/tcolorvalue.rs`, `common/trop/runsmap.rs`, `stdfx/igs_{color_rgb_hls,color_rgb_hsv,math_random,gauss_distribution,negate,density}.rs`. All `.cpp` deleted; headers are inline `extern "C"` forwarders (see PORTING.md). Tests: `cargo test --lib <module>`.
- Wave 4 (13 files, +104 tests; workspace total 802 -> 906): `stdfx/igs_{warp_hori,warp_vert,hls_adjust,hsv_adjust,hls_add,hsv_add,level_auto,level_auto_in_camera,levels,median_filter,median_filter_smooth,hsv_noise,hls_noise}.rs` (24 retired C++ files in total across waves 3 and 4 incl. batch 1's 11). All coarse-grained whole-buffer kernels; only `tnzstdfx` includes these headers and it already links `flare_formats`. Verified bit-identical to the old C++ with a differential harness (see PORTING.md, "Batch 2 notes"). Rejected in this wave: `igs_motion_wind_table` (per-line, takes `igs::math::random` objects), `igs_attenuation_distribution` (`#include`d by igs_fog, pointer-into-vector API), `igs_maxmin*` (thread pool + template class tree), `igs_radial_blur`/`igs_rotate_blur` (QPointF/QTransform/TPointD in body), `igs_perlin_noise` + `iwa_noise1234` (Noise1234 is called per pixel from `iwa_fractalnoisefx`/`iwa_pnperspectivefx`: FFI per call), `igs_{hls,hsv}_noise_in_camera` (public `noise_reference` class + `void*` templates), `igs_fog`/`igs_line_blur` (large, multi-class, deferred), `igs_resource_*` (OS threads / messages). (Of the deferred set, `igs_color_blend` has since been ported whole — see Batch 6.)

### Gaps closed (audit follow-up)
- HTML5 publish no longer discards filters: every `Filter` reaches the page. Blur / plain Drop Shadow use canvas `ctx.filter`; Glow, Bevel, inner / knockout / hide-object shadows and Adjust Color run as per-pixel stages in `html5_runtime.js` (same box-blur + shift + strength formulas as `ac_fx.rs`); EffectCraft effects are rendered at publish time by the stage rasteriser into a PNG sprite (deduplicated). `publish_report` returns a `Report { files, warnings }`; the warnings (Subtract / Invert blend approximations, unknown effect ids) appear in the Output panel.
- XFL import of a bitmap whose `bin/*.dat` is gone: obvious MISSING placeholder, `BitmapAsset.missing`, `Document.warnings`; both survive .acx and .fla saves (`flareMissing="true"`) and the Library panel flags the item.
- Publish targets are one enum (`PublishTarget`, ids listed in Publish Settings); the old `"unsupported"` branch is gone, unknown ids list the valid ones; new "Copy as .acx".
- JSFL `copyLayers` / `cutLayers` / `pasteLayers` / `duplicateLayers` map onto the editor's layer clipboard (`layer.copy|cut|paste|duplicate`).
- Review follow-ups: (1) HTML5 runtime pixel stages survive a tainted canvas (SecurityError skips the stage, one `console.warn`), links open only http/https/mailto with `noopener,noreferrer`, and a multi-file publish with pixel stages warns about file:// taint.
- (2) EffectCraft bake is capped (stage <= 4096 px, 2000 sprites, 256 MiB total) with warnings; sprites stay full-stage (cropping to group bounds needs a runtime offset, left for later).
- (3) `extern "C"` math/runs entry points never panic: n<0 returns status 2 (header throws `std::length_error` like `vector(n)`), null/empty input is a no-op or status 3; `get_hsv` clamps in release.
- (4) `layer.copy|cut` validate before mutating the editor; `layer` without `to_layer` keeps the frame selection.
- (5) Missing bitmaps keep their declared size (`declared_width/height`, also in .acp); the renderer scales the clamped placeholder via the item matrix.
- (6) `tnzext` and `image` link `flare_formats` (Apple links tnzcore by file, so it isn't transitive); `Cplx` documents libstdc++ semantics, MSVC numerics deliberately not emulated.
- Left as is on purpose: `flarelib/mm_xfl.rs` keeps the raw XML of tweens it can't read (round-trip fidelity).

### P2 group 2: symbols & motion (Oct 15 2026)
- **Copy / Paste Motion** (`flarelib/motion_clip.rs`): a classic or motion tween copies as per-frame transforms plus (motion tweens) `Keyframe.motion`, ease, ease curve and Orient to Path; Paste Motion makes a new motion tween of the selected object, relative to where it is (or absolute). Classic tweens are fitted to straight path segments and linear property keys by Douglas-Peucker. Copy Motion as JS emits `ac.timeline.set_tween` / `ac.motion.set_key` / `set_interp` / `set_ease` calls; tests replay the very same calls through `commands::execute`.
- **Motion presets** (`flarelib/motion_presets.rs`, `motion_presets.json`): 23 presets of our own, values relative to the object, stretched over an existing span; user presets live in `settings.json`.
- **Visible / Cache / Export as bitmap**: `Element.visible|cache_as_bitmap|export_as_bitmap` (serde defaults), `EvalCtx.show_hidden` (authoring view keeps hidden objects selectable), `FxGroup.cache` -> HTML5 `"cb"` ids + `drawCached` in the runtime; XFL `isVisible|cacheAsBitmap|exportAsBitmap`.
- **9-slice** (`flarelib/scale9.rs`): `Symbol.scale_grid`; `eval_element` evaluates the instance unscaled and remaps the items (vector paths split at the grid lines, bitmaps/patterns as nine clipped copies), so the stage, every export and the HTML5 publish share it. XFL `scaleGridLeft|Right|Top|Bottom`.
- **Layer effects**: `Layer.filters` wrap the layer's items in one outermost `FxGroup` in `evaluate`; XFL `<filters>` on `DOMLayer` (Flare extension).
- Tests: `flarelib/tests/{p2_motion,p2_symbols}.rs`, `common/tvrender/tests/p2_symbols.rs`, `common/flash/tests/p2_symbols.rs`.

### P2 group 3: drawing & rigging (Oct 15 2026)
- **Hairline + stroke scaling**: `StrokeStyle.scale` (`StrokeScale`: normal/horizontal/vertical/none), `hairline`, `pixel_hinting` (serde defaults, skipped when default). `StrokeScale::pen` gives the matrix the pen is scaled by; `ac_render` strokes in a space where the instance matrix is replaced by the pen matrix (hairline: stroked in device space at 1 px). HTML5 (`hl`, `sm` item keys; runtime does the same pen trick with `Path2D.addPath`), SVG (`vector-effect`), Lottie (hairline width only). XFL `scaleMode`, `solidStyle="hairline"`, `pixelHinting`. Hinting is stored, not applied.
- **Auto Keyframe**: `Editor.auto_keyframe` (default off, saved in settings); `target_keyframe` also keys non-keyframe frames of a plain hold when on. Tweens already set a pose either way.
- **3D global/local**: `Transform3D.local`; `deform::transform_3d` (rotations in the object's frame, depth along the rotated normal) feeds `project_3d_in`; `Editor.three_d_local` is the tool mode, `set_3d_local` converts the selection.
- **Asset Warp density**: `MeshWarp.density` (0 = 24, clamped 4..64), `Editor::set_mesh_density`.
- **Bone springiness/damping** (Flare extension, not in XFL): `Bone.spring` (rad/frame, 0 = off) and `damping` (ratio); `apply_springs` replays the last 120 frames from rest with an exact per-frame spring step, so a pose depends only on the frame index. Filters the child's own rotation about its joint.
- Commands: `flarelib/drawing.rs`. Tests: `flarelib/tests/p2_drawing.rs`, `common/tvrender/tests/p2_drawing.rs`, `common/flash/tests/p2_drawing.rs`.

### Batch 5: min-cut graph (Oct 16 2026)
- `stdfx/naru_maxflow.cpp` -> `stdfx/naru_maxflow.rs`: the Boykov-Kolmogorov max-flow / min-cut used by `naru_lazybrush.cpp`, ported whole with an opaque-handle C ABI (`flare_graph_*`). `stdfx/naru_graph.h` keeps the `Graph` class as an inline forwarder, so `naru_lazybrush.cpp` is unchanged; `naru_maxflow.cpp` is deleted and removed from the `tnzstdfx` source list. Deviations only where the C++ was UB (out-of-range node index, null parent during chain descent). Tests: `cargo test --lib naru_maxflow` (max-flow/min-cut values, capacity overflow, node-index bounds, FFI null handling).

### Batch 6: blend-mode kernels (Oct 16 2026)
- `stdfx/igs_color_blend.cpp` -> `stdfx/igs_color_blend.rs`: all 22 Photoshop / PDF blend modes (`over`, `darken`, `multiply`, `color_burn`, `linear_burn`, `darker_color`, `lighten`, `screen`, `color_dodge`, `linear_dodge`, `lighter_color`, `overlay`, `soft_light`, `hard_light`, `vivid_light`, `linear_light`, `pin_light`, `hard_mix`, `cross_dissolve`, `subtract`, `add`, `divide`) used by the `ino_blend_*` FX through `TBlendForeBackRasterFx::brendKernel`. The 13 anonymous-namespace helpers (`blend_transp_`, `up_add_dn_ch_`, …) are transcribed as private functions; the C ABI is `flare_igs_color_<name>` over a 4-double `[r,g,b,a]` in/out buffer. `stdfx/igs_color_blend.h` keeps `namespace igs::color` as inline forwarders, so the ~20 `ino_blend_*.cpp` files are unchanged; `igs_color_blend.cpp` is deleted and removed from the `tnzstdfx` source list.
- Verified bit-for-bit: the original C++ was compiled with MSVC into a differential harness (`tests/native/igs_color_blend_ref.cpp`, 22 functions x 20 cases = 440 rows, outputs as raw IEEE-754 bits), and both the Rust functions and the full C++->header-forwarder->C-ABI->Rust chain reproduce it exactly. The golden is `stdfx/igs_color_blend_cases.txt` (regenerated by `tests/native/gen_igs_color_blend_cases.py`, which rebuilds the reference from git history since the `.cpp` is gone). A 1e-12 perturbation is caught.
- Tests: `cargo test -p flare_formats igs_color_blend` (golden conformance; C ABI in-place update and null guard).

### SOL / LSO absorption (Oct 16 2026)
- `common/flash/lso.rs`: `.sol` (Flash Local Shared Object) reader/writer, absorbed from `ruffle-rs/rust-flash-lso` (`flash-lso` 0.7.0, MIT; recorded in `common/flash/ORIGIN.md`). Implements the SOL container header and the AMF0 body (number, bool, string / long string, object, ECMA array, strict array, date, null, undefined, reference, XML, typed object). Pure Rust with bounds-checked reads (no `nom`). Wired into format sniffing (`Format::Sol`, `flare_detect_format` now returns 5) and exposed through `flare_lso_is_lso` / `flare_lso_parse` / `flare_lso_free` (`include/flare_formats.h`). Tests: `cargo test -p flare_formats lso` (6 tests: hand-built layout, round-trip, every AMF0 type, JSON view, sniff/reject, truncation never panics).

### Tooling survey
- Automated C/C++→Rust tools exist (c2rust: rule-based, unsafe output; LLM+static-analysis research tools SACTOR, SmartC2Rust, RustMap, SafeTrans). None handle Qt/C++ class hierarchies; output would violate the safe-idiomatic rule. Port is manual, per file, test-verified.
- `ac-core` (≈24k LOC; now spread across the OpenToonz-matching directories, crate root `flare/sources/flarelib/ac_core.rs`) and `flare-studio` (`toonzqt/`, ≈14k LOC) are the AnimationCraft editor, a different architecture from OpenToonz; they are not a line-matching port. Counted against the 2.3k C++ files, the matching rewrite is a few modules deep.

## ac-core file locations (under `flare/sources`)

The crate `ac-core` is rooted at `flarelib/` (Cargo.toml, ac_core.rs with `#[path]` mods, tests/, assets/, ORIGIN.md, LICENSE-*). Sources live beside the C++ they correspond to:

| Directory | Files |
|---|---|
| `flarelib/` | bitmap, centerline, editor, frames, ik, model, motion_clip, motion_presets (+ `.json`), rig, scale9, sound, trace, xdts, flare_frame_clock_tests, flare_tween_boundary_tests |
| `tnztools/` | mypaint (was paint), sculpt; `assistants/assist` |
| `common/` | `expressions/expr`, `tgeometry/geom`, `tvectorimage/merge`, `tparam/{motion,tween_edit}`, `tvrender/text` |
| `stdfx/` | fx, particles |
| `tnzext/` `colorfx/` `image/` `flareqt/` `flare/` | deform; strokes; codecs; commands; demodocument (was demo) |
