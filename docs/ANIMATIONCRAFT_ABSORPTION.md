Absorbing AnimationCraft

AnimationCraft (remote: animationcraft) is a Rust implementation of the Adobe Animate authoring model: symbols, layers, keyframes, classic/shape/motion tweens, a Motion Editor, onion skinning, and XFL import/export.

Flare is simultaneously undergoing its own Rust rewrite. However, the goal is not to import AnimationCraft wholesale or adopt its architecture. Flare intentionally preserves the existing OpenToonz-derived project structure, system boundaries, and data model, which already integrate deeply with the rest of the codebase. As a result, every useful capability from AnimationCraft must be decomposed, analysed, and mapped into Flare's corresponding Rust subsystems rather than copied directly.

The merger therefore consists of extracting features and algorithms from AnimationCraft, identifying where those concepts belong within Flare's architecture, and reimplementing them inside Flare's Rust modules while maintaining the same directory layout, ownership boundaries, and file organisation that Flare currently inherits from OpenToonz.

AnimationCraft should be treated as a feature source and reference implementation, not as a framework to embed.

Architectural Rule

All AnimationCraft functionality must be redistributed across Flare's existing subsystem layout during the Rust rewrite.

This means:

No AnimationCraft crate structure should be imported intact.
No parallel document model should exist alongside Flare's document model.
No duplicate geometry system should coexist with Flare geometry.
No AnimationCraft-specific editor architecture should survive the merge.
Features are absorbed into their equivalent OpenToonz/Flare subsystems.
The Rust rewrite must preserve Flare's current file structure so that Rust modules map directly onto existing OpenToonz concepts and C++ counterparts.

For example:

AnimationCraft	Destination in Flareac-core/geom.rs	common/tcurves/*
ac-core/model.rs	Rust mirrors of existing TXsheet, TXshLevel, TStageObject, styles and paint systems
ac-core/merge.rs	common/tmerge/*
ac-core/motion.rs	Existing tweening and function systems
ac-core/xfl.rs	Existing XFL importer/exporter modules
ac-core/deform.rs	Existing plastic and skeleton systems
ac-core/fx.rs	Existing effects architecture

AnimationCraft's implementation details may disappear entirely during absorption; the only requirement is that the resulting functionality exists inside the correct Flare subsystem.

What "Absorption" Means

AnimationCraft is not being merged at the repository level.

Instead, absorption involves:

Identifying capabilities missing from Flare.
Locating the equivalent subsystem inside Flare's OpenToonz-derived architecture.
Rewriting or adapting the relevant AnimationCraft implementation to fit that subsystem.
Preserving Flare's existing ownership and file structure.
Ensuring every new Rust module remains consistent with the ongoing Rust replacement of existing C++ systems.

The success criterion is not whether AnimationCraft's source tree survives. The success criterion is whether Flare gains the capability while retaining a coherent OpenToonz-compatible architecture.

Per-module Verdict

Measured against Flare at commit bc402dc34.

The question is not "should this crate be ported?" but rather "does this contain functionality that needs to be redistributed into Flare's Rust rewrite?"

AnimationCraft module	Verdict	Integration strategyac-core/merge.rs	Absorb	Break apart and remap into common/tmerge, tcurves, paint systems and editor tooling.
ac-core/editor.rs	Reference only	Features already exist in Flare's command architecture. Extract ideas only if gaps are identified.
ac-core/commands.rs	Reference only	Merge concepts into existing command systems where useful.
ac-core/model.rs	Do not import	Functionality must map onto Flare's existing scene and document model.
ac-core/geom.rs	Absorb selectively	Concepts belong in common/tcurves; avoid introducing competing geometry types.
ac-core/motion.rs	Compare against existing implementation	Integrate missing behaviour into current tween systems.
ac-script	Defer	Potential future scripting integration.
ac-render	Do not import	Rendering belongs to Flare's renderer architecture.
ac-core/deform.rs	Reference only	Existing rigging systems already occupy this role.
ac-core/rig.rs	Reference only	Same as above.
ac-core/fx.rs	Reference only	Effects belong in the existing effects stack.
ac-core/frames.rs	Reference only	Existing timeline infrastructure already exists.
ac-core/text.ts	Reference only	Evaluate for missing Animate-specific functionality.
ac-core/xfl.rs	Compare, do not replace	Existing XFL implementation remains authoritative.
ac-app, ac-cli	Ignore	UI architecture is unrelated to Flare's Qt-based frontend.
The Important Distinction

AnimationCraft and Flare are converging on similar goals, but they are not converging on the same architecture.

AnimationCraft is organised around its own Rust workspace, crates, and internal data structures.

Flare is retaining the OpenToonz architecture while replacing implementation layers with Rust over time.

Because of that, every absorbed feature must be translated into Flare's architecture. The merge is therefore not a repository merge, crate merge, or file-for-file port. It is a systematic decomposition of AnimationCraft's functionality and redistribution of that functionality into Flare's existing subsystem hierarchy.

The resulting codebase should still look like Flare, still mirror OpenToonz's structure, and still present the same high-level organisation after the Rust rewrite is complete. AnimationCraft's functionality may be present throughout the tree, but AnimationCraft's architecture should not be.

## ac-core file locations (under `flare/sources`)

The crate `ac-core` is rooted at `flarelib/` (Cargo.toml, ac_core.rs with `#[path]` mods, tests/, assets/, ORIGIN.md, LICENSE-*). Sources live beside the C++ they correspond to:

| Directory | Files |
|---|---|
| `flarelib/` | bitmap, centerline, editor, frames, ik, model, rig, sound, trace, xdts, flare_frame_clock_tests, flare_tween_boundary_tests |
| `tnztools/` | mypaint (was paint), sculpt; `assistants/assist` |
| `common/` | `expressions/expr`, `tgeometry/geom`, `tvectorimage/merge`, `tparam/{motion,tween_edit}`, `tvrender/text` |
| `stdfx/` | fx, particles |
| `tnzext/` `colorfx/` `image/` `flareqt/` `flare/` | deform; strokes; codecs; commands; demodocument (was demo) |

## Wave 2: remaining RustConversion-AnimateCraft crates (f636b05)

Each crate keeps its package name; manifest + root `.rs` + `ORIGIN.md` sit in the OpenToonz-matching directory, files are `ac_`-prefixed and wired with `#[path]` (no `rust/`, `crates/`, `src/`). Workspace-level `[workspace.package]`, `[workspace.dependencies]` and `[profile.*]` were added to `flare/sources/Cargo.toml`; `LICENSE-MIT`, `LICENSE-APACHE`, `THIRD-PARTY-NOTICES.md` are at `flare/sources/`.

| Crate | Location (under `flare/sources`) |
|---|---|
| `ac-render` | `common/tvrender/` |
| `ac-xfl` | `common/flash/` |
| `ac-audio` | `sound/` |
| `ac-script` | `common/expressions/` |
| `ac-package` | `common/tsystem/` |
| `ac-app` (bin `flare`) | `flare/` (`ac_app_main.rs`, `ac_app/`, `build.rs`, `assets/{icon,flare-icons,icons}`) |
| `ac-cli` (bin `flare-cli`) | `flareqt/` |

The git dependencies on `storytold/effectcraft` and `storytold/photocraft` are fetched through the proxy; Cargo.lock pins the upstream commits.

## minnamate import

Crate `minnamate-core` (clean-room, no upstream licence file; see `flare/sources/tnztools/ORIGIN.md`) is rooted at `tnztools/` (Cargo.toml, minnamate_core.rs with `#[path]` mods, tests/ incl. fixtures). All files are prefixed `mm_` pending merge with overlapping AnimationCraft modules.

| Directory (under `flare/sources`) | Files |
|---|---|
| `flarelib/` | mm_arrange, mm_display, mm_doc, mm_layer_motion, mm_layer_tree, mm_symbols, mm_xfl, mm_xfl_write |
| `tnztools/` | mm_draw, mm_erase, mm_hit, mm_paint, mm_pen, mm_transform |
| `common/tvrender/` | mm_fonts, mm_render, mm_shape_tween, mm_swf_export, mm_text |
| `common/tparam/` | mm_motion |
| `flareqt/` `stdfx/` `flare/` | mm_filters; mm_gradient; mm_edit |

Overlap pairs to merge: mm_motion <-> common/tparam/motion.rs; mm_layer_tree <-> flarelib/model.rs (layers); mm_filters <-> stdfx/fx.rs; mm_erase (vs AnimationCraft erase tooling); mm_text <-> common/tvrender/text.rs; mm_doc <-> flarelib/model.rs; mm_edit <-> flarelib/editor.rs.

## Studio location

The egui/wgpu Studio workspace (`flare-studio`, formerly `flare/rust/studio`) now lives in `flare/sources/toonzqt/` (OpenToonz's Qt widget library: widgets, theme, chrome). Root files are `studio_lib.rs` and `studio_main.rs`; `locales/` and `assets/` sit beside them. It depends on `ac-core` (`flarelib`, feature `retain-tween-curves`) and `ac-render` (`common/tvrender`) by path; the private AnimationCraft git pin and `[patch]` are gone. PhotoCraft/ArtCraft attribution is unchanged (`toonzqt/assets/ATTRIBUTION.md`, `THIRD-PARTY-NOTICES.md`).
