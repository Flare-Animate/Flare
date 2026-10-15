# AnimationCraft core in Flare

Copied from `KMSDax/AnimationCraft`, commit
`e3f25db0140ea24590184b117e928e35f5bc6302` (main, 2026-10-14), `crates/ac-core` (sources, integration tests).
Licence: `MIT OR Apache-2.0` (`LICENSE-MIT`, `LICENSE-APACHE` in this folder).
Original source comments and attribution are preserved.

## History

The first copy (October 2026) was taken at AnimationCraft commit `e3aadad`. Flare
then added, in this folder: per-layer row height; `TweenBoundary` retained tween
curves (partial clipboard boundaries keep the original curve over a source-time
interval, split/rejoin, reversal, guide traversal, retiming, clipboard walks);
keyframe graphic playback clocks (`playback_offset` / `playback_reversed`);
widened graphic first-frame arithmetic; frame-by-frame conversion sampling the
source before splits; bitmap-fill usage/deletion; the frame-clock and
tween-boundary regression tests.

AnimationCraft merged all of those back upstream (commit `b777ecd`; see its
`log/devlog.md` and `docs/FLARE-REVIEW-PHASE5.md`), so this copy contains them
unchanged: `tween_edit.rs`, the `TweenBoundary` / playback-clock changes in
`model.rs`, `frames.rs`, `editor.rs`, and the tests as
`src/flare_frame_clock_tests.rs`, `src/flare_tween_boundary_tests.rs`. Upstream
keeps Animate's behaviour as the default: retained curves on split are switched
on per thread with `frames::set_retain_tween_curves(true)` (Studio should call it
where it relied on the old always-on behaviour).

## What this update brings (upstream since `e3aadad`)

- `.fla`/XFL reading and writing moved to its own crate (`ac-xfl`); the edge
  parser formerly at `ac_core::xfl` is no longer in the core.
- Sounds, scenes, layer parenting, motion tweens + Motion Editor, custom eases,
  shape hints, buttons, Merge Drawing engine, filters/blend modes, masks,
  guides, camera (incl. VCam), text with system fonts.
- OpenToonz ports (BSD-3-Clause, notices in upstream `THIRD-PARTY-NOTICES.md`):
  stroke styles (`strokes.rs`), deformation tools (`sculpt.rs`), drawing
  assistants (`assist.rs`), expressions + keyframe types (`expr.rs`,
  `motion.rs`), Trace Bitmap outline/centerline (`trace.rs`, `centerline.rs`),
  XDTS export/import (`xdts.rs`), damped-least-squares IK with joint limits and
  pins (`ik.rs`), Asset Warp meshes on bitmaps/symbols (`deform.rs`).
- MyPaint raster painting (`paint.rs`, through the `hokusai` crate) with 25 CC0
  brushes from mypaint-brushes in `assets/mypaint/` (see its README).
- Particle emitters (`particles.rs`), image decoders (`codecs.rs`, feature
  `codecs`, on by default; `default-features = false` drops the `image` crate).

## Flare adaptations in this copy

- Standalone manifest (workspace inheritance replaced by the same versions).
- Feature `retain-tween-curves` (used by Studio): retained tween curves are on by
  default, as Flare's timeline expects; without it the default is Animate's
  (off), so upstream's tests run unchanged. Studio's manifest patches the
  AnimationCraft git dependency to this folder, so Studio and the pinned
  `ac-render` share this copy.
- The MyPaint brushes live in this folder (`assets/mypaint/`), so `paint.rs`
  includes them from `../assets/mypaint/` instead of the upstream repo root.

Further adaptations must be listed here and covered by tests. To update, copy
`crates/ac-core/{src,tests}` and `assets/mypaint` from a newer upstream commit,
re-apply the adaptations above, and move Studio's `ac-core` / `ac-render` git
pins to the same commit.
