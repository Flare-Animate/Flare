# Dependency skew audit (UNIFICATION_PLAN step 1)

Scope: `flare/sources` Cargo workspace. Read-only audit of manifests and `Cargo.lock` (684 packages, 77 names with >1 version). No code changed.
Groups: **ac** = ac-* crates (ac-app `flare`, ac-cli, ac-render, ac-xfl, ac-audio, ac-core, ac-script, ac-package); **mm** = minnamate-core (`tnztools`), minnamate-editor (`tnzext`), minnamate-cli (`tcomposer`); **studio** = flare-studio (`toonzqt`).

## Manifest versions (direct deps only)

| Dep | ac | mm | studio | Lock versions | Status |
|---|---|---|---|---|---|
| egui / eframe / egui_extras | eframe 0.29 (`flare`) | eframe 0.36 (`tnzext`) | eframe =0.36.2, egui_extras =0.36.2 | 0.29.1, 0.36.2 | SPLIT (major) |
| egui_kittest (dev) | - | 0.36 | =0.36.2 | 0.36.2 | ok |
| tiny-skia | 0.11 (`flare`, `common/tvrender`) | 0.12 (`tnztools`) | 0.11 | 0.11.4, 0.12.0 | SPLIT |
| image | 0.25 (`flare`, `flare/common/flash`, tvrender) | - | - | 0.25.10 | one version, features differ |
| zip | 2 (`common/flash`, `common/tsystem`) | 9 (`tnztools`) | - | 2.4.2, 9.0.3 | SPLIT |
| flate2 | 1 (`common/flash`) | 1 (`tnztools`) | - | 1.1.10 | ok (not workspace-pinned) |
| serde / serde_json | workspace | - | 1 / 1 | 1.0.229 / 1.0.151 | ok but not all use `.workspace` |
| gif | 0.13 workspace (tvrender) | - | - | 0.13.3, 0.14.2 (via image) | SPLIT (transitive) |
| png | 0.17 workspace (`flarelib`, tvrender) | - | - | 0.17.16, 0.18.1 (via image) | SPLIT (transitive) |
| roxmltree | 0.20 (`common/flash`, `flare`) | 0.21 (`tnztools`) | - | 0.20.0, 0.21.1 | SPLIT |
| usvg / resvg | usvg 0.44, resvg 0.44 (`flare`) | - | - | usvg 0.44/0.45, resvg 0.44/0.45 | SPLIT (0.45 from egui_extras) |
| ttf-parser | 0.24 (`common/flash`) | - | - | 0.24.1, 0.25.1 | SPLIT (transitive) |
| rfd | 0.15 (`flare`) | 0.17 (`tnzext`) | 0.17 | 0.15.4, 0.17.2 | SPLIT |
| miniz_oxide | - | - | - | 0.8.9, 0.9.1 (flare_formats/flate2) | SPLIT (transitive) |
| wgpu | via eframe 0.29 | via eframe 0.36 | via eframe 0.36 | 22.1.0, 30.0.1 | SPLIT (follows egui) |
| thiserror / syn | - | - | - | 1.x+2.x; syn 1/2/3 | transitive only |

Note: `tnztools` and `tnzext` do not use `.workspace = true` for any dep, so the workspace table does not constrain them.

## Lock duplicates, grouped by cause

1. **egui generation split (0.29 vs 0.36):** egui, eframe, epaint, emath, ecolor, egui-wgpu, egui_glow, egui-winit, epaint_default_fonts, wgpu 22/30, naga, glow, winit paths. Caused by `flare` on 0.29 and `tnzext`/`toonzqt` on 0.36. Biggest cost in build time and binary size.
2. **tiny-skia split:** 0.11 (ac, studio, resvg 0.44) vs 0.12 (mm, pulls png 0.18 and tiny-skia-path 0.12).
3. **zip / roxmltree / rfd / png / gif** direct splits as in the table.
4. **Transitive, non-blocking:** miniz_oxide, ttf-parser, usvg/kurbo 0.11/0.13, getrandom 0.2/0.3/0.4, hashbrown 0.15/0.16/0.17, rustix 0.38/1.1, bitflags 1/2, thiserror 1/2, syn 1/2/3, windows-* 0.42-0.62, objc2 0.2/0.3 and similar platform crates. These follow the splits above or the platform crates; fixing the direct deps reduces most of them.

## Recommended `[workspace.dependencies]` unification table

Add to the root `Cargo.toml` (existing entries: serde, serde_json, tiny-skia 0.11, gif 0.13, png 0.17). Members switch to `dep.workspace = true`.

| Entry | Proposed pin | Action |
|---|---|---|
| `tiny-skia` | `0.11` | Keep 0.11 now. Move `tnztools` to 0.11 once step 3/12 needs it; until then record as known split. |
| `image` | `{ version = "0.25", default-features = false }` | Add; per-crate features stay as extra `features = [...]`. |
| `zip` | `"2"` | Add. `tnztools` on 9 is a blocker: check its API use before moving. |
| `flate2` | `"1"` | Add. Already single-version. |
| `roxmltree` | `"0.20"` | Add. Move `tnztools` to 0.20 when feasible. |
| `usvg` / `resvg` | `"0.44"` | Add. Lock 0.45 comes only from egui_extras 0.36. Resolves with egui unification. |
| `egui` / `eframe` / `egui_extras` | `"0.36.2"` (exact) | Add now as the target; `flare` (0.29) migrates in step 7/8. Do not force it in step 1. |
| `rfd` | `"0.17"` | Add. Move `flare` (0.15) when egui moves. |
| `ttf-parser`, `png`, `gif` | `0.24`, `0.17`, `0.13` | Leave transitive splits recorded; pin direct deps only. |

## Risks and follow-up

- egui 0.29 to 0.36 is a breaking API change. Unifying it touches all of `flare/ac_app/*`; it belongs to step 8 or a dedicated migration, not step 1.
- `toonzqt` pins `=0.36.2` while `tnzext` uses a caret. A single workspace `egui = "=0.36.2"` removes drift.
- Feature unification: `image` is enabled with different features in 3 crates; the lock shows one version, so codecs are shared, but each crate's features should be kept in the workspace table to avoid silent additions.
- Step 1 CI check: `cargo tree -d --workspace -e normal` (or a script over `Cargo.lock`) on the three direct groups above; fail on new splits in egui, tiny-skia, zip, image, roxmltree.
- Existing `[patch."https://github.com/KMSDax/AnimationCraft"]` in root `Cargo.toml` and git deps (effectcraft-*, photocraft-psd) are not version-checked here; they should be pinned to a rev in the table.

## Step 1 status: workspace table added (2026-10-16)

The `[workspace.dependencies]` unification table above is now in `flare/sources/Cargo.toml`
(egui/eframe/egui_extras = exact 0.36.2 target, plus image, zip, flate2, roxmltree, usvg,
resvg, rfd, dirs). Entries are declared but members are not forced onto them yet, so no
crate's resolved versions changed; the table is the single pin the migration steps move
members onto. `ac-app` (`flare`, 0.29) remains the last split and is step 14.

## Step 1 follow-up: the last split is closed (2026-10-16)

`ac-app` (`flare`) moved off eframe 0.29 (glow) onto eframe `=0.36.2` with the `wgpu`
backend, the same generation `minnamate-editor` and `flare-studio` already used (UNIFICATION
PLAN step 14's core). `Cargo.lock` now resolves **egui / eframe / epaint / egui-wgpu to a
single 0.36.2** — the 0.29 generation is gone — so the egui row of the table above is no
longer SPLIT. This audit's three "blocking" rows therefore reduce to: egui **closed**;
tiny-skia 0.11/0.12 and zip 2/9 remain, both `tnztools`-internal and unrelated to the UI
merge. `rfd` (ac 0.15 vs mm/studio 0.17) can now follow, since the audit said to move it
"when egui moves"; it is left as a follow-up rather than bundled here.

The migration is verified by compilation only (`cargo check --workspace`), not by running
the GUI: the panel API changed shape (see UNIFICATION PLAN step 14), so the layout should be
eyeballed once before release.

