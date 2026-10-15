# Third-party notices

Flare (Rust) — formerly AnimationCraft — is licensed MIT OR Apache-2.0 (see
`LICENSE-MIT`, `LICENSE-APACHE`); parts taken from Flare and OpenToonz keep their
BSD-3-Clause notices below.
It contains or links the following work by others. Each port records its source
at the top of the file and in `log/devlog.md`, as CONTRIBUTING.md requires.

## Code ported into this repository

### Flare (github.com/Flare-Animate/flare)
This repository is the Rust conversion of Flare. Flare's native Studio runs on a
copy of this project's `ac-core` (MIT OR Apache-2.0); its changes to that core
were merged back so both share one engine:

- `crates/ac-core/src/tween_edit.rs` (retained-curve pose corrections)
- `crates/ac-core/src/flare_frame_clock_tests.rs`, `flare_tween_boundary_tests.rs`
- the `TweenBoundary` / playback-clock changes in `model.rs`, `frames.rs`, `editor.rs`

Source: `flare/sources/flare/rust/core`, commit `a27136a` (October 2026).

Artwork from Flare (BSD-3-Clause, © Flare contributors; Flare's tool icons
descend from OpenToonz's, © DWANGO Co., Ltd. and OpenToonz contributors):

- `assets/icon/flare.png`, `flare.ico`: Flare's application icon
  (`flare/sources/flare/Flare.ico`; the .ico here adds resampled small sizes).
- `assets/flare-icons/tools/*.svg`: 28 tool icons from
  `flare/sources/flare/icons/dark/tools/20x20/` and `icons/dark/actions/`,
  unchanged (tinted at runtime). Our own icons for the other tools are in
  `assets/icons/tools` (original, this project's licence).

Neither was taken from Adobe software.
Not taken from Flare: its Qt theme materials (derived from studying Adobe
binaries) and its Flash/SWF readers (written with GPL/AGPL references). See
`docs/FLARE-REVIEW-PHASE5.md`.

### OpenToonz (github.com/opentoonz/opentoonz)
Features modelled on OpenToonz subsystems are listed in
`docs/OPENTOONZ-REVIEW.md`. Files that follow OpenToonz code or formats:

| This repository | OpenToonz |
|---|---|
| `ac-core/src/strokes.rs` (Herringbone … Curl) | `toonz/sources/colorfx/strokestyles.cpp` |
| `ac-core/src/sculpt.rs` | `toonz/sources/tnztools/{magnet,pinch,pump,bender,iron}tool.cpp`, `tnzext/Stroke*Deformation.cpp` |
| `ac-core/src/assist.rs` | `toonz/sources/tnztools/assistants/*` (incl. perspective, fisheye, affine, jitter), `assistant.cpp`; jitter noise ported from `tnztools/modifiers/modifierjitter.cpp` |
| `ac-core/src/ik.rs` | `toonz/sources/toonzlib/ikjacobian.cpp`, `ikengine.cpp` (damped least squares, step limit, pins) |
| `ac-core/src/deform.rs` (`MeshWarp`, `MeshMap`) | `toonz/sources/tnzext/plasticdeformer.cpp` (approach: deform a triangle grid, draw each triangle by its affine map) |
| `ac-core/src/expr.rs` | `toonz/sources/common/expressions/tgrammar.cpp` |
| `ac-core/src/motion.rs` (`Interp`) | `toonz/sources/common/tparam/tdoubleparam.cpp` |
| `ac-core/src/centerline.rs` | `toonz/sources/toonzlib/tcenterline*.cpp` (approach) |
| `ac-core/src/trace.rs` | `toonz/sources/toonzlib/toutlinevectorizer.cpp` (approach) |
| `ac-core/src/xdts.rs` | `toonz/sources/toonz/xdtsio.cpp` (format, cell rules for import), `xdtsimportpopup.cpp` (level lookup, approach) |

Where code is ported (not just the approach), the file says so and carries
this notice:

> Copyright (c) 2016, DWANGO Co., Ltd. and OpenToonz contributors.
> All rights reserved. Redistribution and use in source and binary forms, with
> or without modification, are permitted under the BSD 3-Clause License
> (full text: https://github.com/opentoonz/opentoonz/blob/master/LICENSE.txt).

## Libraries used as dependencies (not copied)

### EffectCraft (github.com/storytold/effectcraft)
`effectcraft-effects`, `effectcraft-raster`, `effectcraft-keyframe` and
`effectcraft-project` are git dependencies of `ac-render` (object effects from
EffectCraft's registry). Copyright (c) 2026 ArtCraft Team and the EffectCraft
contributors, MIT OR Apache-2.0. Update with `cargo update -p effectcraft-effects`.

### PhotoCraft (github.com/storytold/photocraft)
`photocraft-psd` is a git dependency of `ac-app`: its `.aco` (Photoshop swatches)
and `.ase` (Adobe Swatch Exchange) readers and writers, and its `.grd`
(Photoshop gradients) reader and writer, back the Swatches panel's import and save. Copyright (c) 2026 ArtCraft Team and the PhotoCraft
contributors, MIT OR Apache-2.0. Update with `cargo update -p photocraft-psd`.

### hokusai (github.com/reearth/hokusai)
Pure-Rust port of libmypaint, the brush engine behind the Raster Brush
(`ac-core/src/paint.rs`). crates.io dependency, MIT OR Apache-2.0.

### image (github.com/image-rs/image)
Decoders for bitmap import (PNG, JPEG, GIF, BMP, TGA, TIFF, WebP, OpenEXR) in
`ac-core/src/codecs.rs`. crates.io dependency, MIT OR Apache-2.0.

### Rust crates
All other dependencies come from crates.io under their own licences (mostly
MIT and/or Apache-2.0). `cargo tree` lists them; `cargo about` or
`cargo-license` can produce the full licence report for a release.

## Data

### MyPaint brushes (github.com/mypaint/mypaint-brushes)
`assets/mypaint/*.myb` (25 brushes from `brushes/classic` and
`brushes/deevad`, commit 08da4a48): CC0-1.0 (public domain dedication), by
Martin Renold and the MyPaint Development Team and by David Revoy. Embedded in
the binary.

## Artwork
The app icon (`assets/icon/`) is original artwork for this project, under the
same licence as the code.
