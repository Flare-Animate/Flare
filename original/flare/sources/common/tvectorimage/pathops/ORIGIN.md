# vc-pathops in Flare

Imported from `github.com/storytold/vectorcraft` @ `a466625c8987550635cda951ebf36e08e1e80a56`,
`crates/pathops` (package `vectorcraft-pathops`). Licence `MIT OR Apache-2.0`; upstream's
`LICENSE-MIT` / `LICENSE-APACHE` (Copyright ArtCraft Team and the VectorCraft contributors) are kept
in this folder and listed in `flare/sources/THIRD-PARTY-NOTICES.md`.

Adaptations (everything else is verbatim):
- Package renamed `vc-pathops` (lib `vc_pathops`); lives beside the C++ it relates to
  (`common/tvectorimage`, regions / sweep boundary). Standard `src/` + `tests/` layout is kept
  because the sources are not 1:1 ports of C++ files.
- `vectorcraft-geom` dependency removed. The parts used are inlined as `src/geom/`: `path.rs`,
  `bez.rs`, `hit.rs` verbatim, `shapes.rs` trimmed to `rectangle`, `ellipse`, `line`. `use
  vectorcraft_geom::` became `use crate::geom::`.
- `vectorcraft-testkit` dev-dependency removed; the helpers the property tests use (hausdorff,
  polygon_area, grid, rel_close and the arb_* strategies) are copied into `tests/common/mod.rs`.
- Workspace deps replaced by explicit versions (kurbo 0.13, linesweeper 0.5, thiserror 2, serde 1;
  dev: proptest 1, serde_json 1). Upstream's workspace lints and `clippy.toml` are copied per crate.
- Edition stays 2024 (let-chains), unlike the rest of this workspace.
- Added `src/flare.rs` (feature `flare`, default): adapter union/intersect/subtract/xor/stroke_outline
  over `ac_core::geom::Path`. Not upstream code.
