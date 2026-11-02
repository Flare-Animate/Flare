# What Flare takes from OpenToonz, and what AnimationCraft can use

> **Note (2026-10-14):** written when this code base was still called AnimationCraft. It has since
> become the base of Flare's Rust conversion and is named Flare; "AnimationCraft" below means this
> repository, "Flare" the C++ Flare app.

Review of `Flare-Animate/Flare` at commit `0368b5b` (Oct 2026). Flare is a fork
of OpenToonz (BSD-3-Clause, © DWANGO and contributors) with an Animate-style
workspace, Flash-family importers and a native Rust "Studio" built on
AnimationCraft's `ac-core` (copied at our commit `e3aadad`).

**Bottom line:** most of Flare's value comes from OpenToonz, and OpenToonz's
licence lets us port it (keeping its copyright notice). About a dozen
OpenToonz subsystems fill gaps on our own gap list (Trace Bitmap, Magic Wand,
art/pattern brushes, raster painting, PSD import, better lip-sync and IK, a
fuller curve editor, drawing guides, particles). The exposure-sheet/level
model, the render farm, scanning and Qt UI code are not worth taking.

## Status (October 2026)

Ported or rebuilt in AnimationCraft, each with tests and a devlog entry:
#1 Trace Bitmap (outline and centerline), #2 Magic Wand, #3 stroke styles
(13 OpenToonz styles), #4 raster painting (MyPaint brushes through the
pure-Rust `hokusai` port), #5 Rhubarb lip sync, #6 curve keyframe types and
expressions, #7 damped-least-squares IK (after `ikjacobian.cpp`) with joint
limits and pinned joints, #8 mesh deformation of bitmaps, symbols and groups
(Asset Warp pins drive a deformed triangle grid; our pin warp, not Plastic's
skeleton-driven ARAP), #9 drawing assistants (line, vanishing point, ellipse,
perspective, fisheye) and replicators (mirror, star, grid, affine, jitter),
#10 vector deformation tools, #11 PSD import, #12 particles (our own emitter,
evaluated per frame so every export draws it; not a port of `iwa_particles`),
#14 Shift & Trace, #15 XDTS export and import, #16 TGA/TIFF/WebP/OpenEXR
import (the `image` crate).

Still open: more effects (#13, partly covered by EffectCraft; wait for a GPU
renderer), PDF timing sheets (#15), Plastic-style skeleton-driven meshes
(#8, beyond Animate's Asset Warp), and the brush input modifiers other than
jitter (smooth, simplify) from `tnztools/modifiers`.

## 1. What Flare is made of

C++/C lines, excluding `thirdparty/` (≈670k lines of bundled libraries).

| Part | Lines | What it is | Origin |
|---|---|---|---|
| `flare/` (app) | 209k | Qt application: rooms, panels, xsheet/timeline viewers, popups, commands, Flash/Moho import | OpenToonz app + Flare additions |
| `include/` | 110k | Public headers of every library | OpenToonz |
| `flarelib/` (= toonzlib) | 89k | Scene, xsheet, levels, stage objects, palettes, cleanup, vectorizers, IK, onion skin, script bindings | OpenToonz |
| `flareqt/` (= toonzqt) | 81k | Reusable Qt widgets: function editor, schematic, palette viewer | OpenToonz |
| `stdfx/` | 81k | ~150 effects (blur, glow, bokeh, particles, warps, gradients, blend modes, noise…) | OpenToonz (igs/ino/iwa sets) |
| `tnztools/` | 79k | Drawing and editing tools, assistants, brush modifiers | OpenToonz |
| `common/` | 140k | Core libraries (below) | OpenToonz + Flare `flash/`, `moho/`, `tgame/` |
| `image/` | 44k | Image/movie formats: TLV/PLI (Toonz), PSD, EXR, TIFF, TGA, SGI, FFmpeg, sprite sheets, SVG | OpenToonz |
| `flarefarm/`, `tcomposer/`, `tcleanupper/`… | 26k | Render farm, command-line renderer, batch cleanup | OpenToonz |
| `colorfx/` | 12k | Special stroke and fill styles (rope, chain, fur, braid, sketch, flow lines, textures) | OpenToonz |
| `tnzext/` | 11k | Plastic mesh deformation, stroke deformation (bend/pinch/pump tools) | OpenToonz |
| `stopmotion/` | 10k | Camera capture for stop motion | OpenToonz |
| `rust/` + `*/rust/` | 33k Rust | Studio app (14k) on AnimationCraft core, format sniffers, small ports | Flare + AnimationCraft |

Inside `common/`: `trop` raster ops (26k), `tvectorimage` stroke/region
vector model (19k), `flash` (17k, Flare + the old Macromedia SDK writer),
`tvrender` (14k), `tfx` effect graph (8k), `tiio` image I/O, `tsound`, `tparam`
animatable parameters + curves, `expressions` (parser for curve expressions),
`psdlib`, `tgeometry`, `tcache`.

## 2. Worth porting, ordered by value to AnimationCraft

"Gap" refers to `docs/ANIMATE-GAP-ANALYSIS.md`.

| # | Feature | OpenToonz source | Fills which gap | Port approach | Size |
|---|---|---|---|---|---|
| 1 | **Trace Bitmap** (raster → vector) | `flarelib/toutlinevectorizer.cpp` (outline, 1.7k), `tcenterline*.cpp` (centerline: skeletonizer, polygonizer, colours, ~3k) | Media: Trace Bitmap | Port the outline vectorizer first (colour regions → our merge shapes); centerline later for line art | M |
| 2 | **Magic Wand** | `stdfx/magicwandfx.cpp` (flood fill with tolerance, Fishkin) + `flarelib/fill.cpp` | Tools: Magic Wand | Small: tolerance flood fill on a bitmap → selection mask → polygon | S |
| 3 | **Art / pattern brushes, ragged / hatched strokes** | `colorfx/strokestyles`, `regionstyles.cpp` (4k), `flowlinestrokestyle.cpp` | Drawing: Art/Pattern brushes, ragged/hatched strokes | Port the stroke styles as generators that output our paths (rope, chain, braid, sketch, dotted, flow lines); texture fills later | M |
| 4 | **Raster painting** (MyPaint brushes) | `tnztools/toonzrasterbrushtool.cpp`, `fullcolorbrushtool.cpp`, `mypainttoonzbrush.cpp`; `thirdparty/libmypaint` (ISC); brushes in `stuff/library/mypaint brushes` (CC0) | Animate's raster brushes; frame-by-frame painting | Use libmypaint through a Rust binding (or the pure-Rust port) and ship the CC0 brush set; paint into a new bitmap-layer element | L |
| 5 | **Lip-sync via Rhubarb** | `flare/autolipsyncpopup.cpp` runs the Rhubarb Lip Sync program (MIT) and maps its mouth shapes to frames | Our heuristic lip-sync | Optional external tool like ffmpeg: if `rhubarb` is installed, use its timings (A–H, X shapes → our visemes); keep our analysis as the fallback | S |
| 6 | **Curve editor depth** | `common/tparam` (keyframe types: Constant, Linear, Speed In/Out, Ease In/Out, %, Exponential, Expression, File, Similar Shape), `common/expressions` (parser, 4k), `flareqt` function editor | Motion Editor: copy/paste curves, Bezier handles, expressions | Port keyframe types and the expression grammar (`frame`, maths, references to other curves); reuse in Motion Editor tracks | M |
| 7 | **IK solver** | `flarelib/ikjacobian.cpp`, `ikskeleton.cpp`, `ikengine.cpp` (Jacobian / damped least squares, 1.6k) | Animation: bone springiness / joint constraints | Replace or complement our CCD with damped least squares; add joint angle limits and pinned joints | M |
| 8 | **Mesh deformation of bitmaps and symbols** | `tnzext/plasticdeformer.cpp`, `plasticskeleton*.cpp`, `meshbuilder.cpp`, `meshtexturizer.cpp` (as-rigid-as-possible with skeleton, ~4k) | Drawing: Asset Warp on symbols/bitmaps; armature layers | Mesh from the bitmap's alpha, ARAP deformation driven by our bones/pins, rendered as textured triangles | L |
| 9 | **Drawing assistants and symmetry** | `tnztools/assistants/*` (perspective, vanishing point, line, ellipse, fisheye guides; mirror, star, grid, affine, jitter replicators), `tnztools/modifiers/*` (smooth, simplify, jitter, line) | Not in Animate; a differentiator | Snap the pencil/brush input to assistants; replicate strokes for symmetry | M |
| 10 | **Vector deformation tools** | `tnztools/bendertool.cpp`, `pinchtool.cpp`, `pumptool.cpp`, `magnettool.cpp`, `irontool.cpp`, `tnzext/Stroke*Deformation.cpp` | Beyond Animate (helps merge-shape editing) | Apply to our merge-shape edges and drawing-object paths | M |
| 11 | **PSD import** (layers) | `common/psdlib/psd.cpp` (1.3k) | Media: AI/PSD import | Read layers as bitmaps on separate layers; consider the `psd` crate (MIT/Apache) first | S |
| 12 | **Particles** | `stdfx/iwa_particles*.cpp` (~3k) | Not in Animate (needs FX) | Later: needs our FX layer (13) | M |
| 13 | **Effect library (subset)** | `stdfx/` igs/ino/iwa sets: motion blur, radial/spin blur, glow, bloom, bokeh, noise, warp, gradients, tone curve | Filters beyond Animate's five | Port only after a GPU path (wgpu) exists; pick ~10 popular effects | L |
| 14 | **Onion skin extras: Shift & Trace** | `tnztools/shifttracetool.cpp`, `flarelib/onionskinmask.cpp` | Timeline polish | Shift & Trace (move/rotate the onion ghost to trace) on our onion markers | S |
| 15 | **Exposure-sheet exchange** | `flare/xdtsimportpopup.cpp` (XDTS, used by CSP/TVPaint), `exportxsheetpdf.cpp` | Interchange with other studios | Import/export XDTS timing; PDF timing sheets | S |
| 16 | **Image/movie formats** | `image/` (EXR, TGA, TIFF, SGI, sprite) | Import/export breadth | Prefer Rust crates (`image`, `exr`, `tiff`); no port needed | — |

Size: S = days, M = 1–3 weeks, L = 1–2 months (one developer with AI help).

## 3. Ideas to learn from, not code to port

- **Levels and the exposure sheet (xsheet).** OpenToonz stores drawings as
  numbered levels exposed in cells, which suits traditional animation and
  re-timing. Animate users expect keyframes on layers, which we have. A
  read-only xsheet view of our timeline (plus XDTS export) gives studios what
  they need without changing the model.
- **The FX schematic** (node graph of effects). Powerful but heavy; our
  filters + blend modes cover Animate's model. Revisit with the GPU renderer.
- **Palettes and styles by index.** Toonz drawings reference palette styles,
  so recolouring a palette recolours every drawing. Animate's swatches don't
  do this; a "linked swatch" option on fills is a cheap, popular idea.
- **Scene file / project folders** (`.tnz`, `tproject`). Our `.acx` package
  already covers this better for single documents.
- **Script bindings** (`scriptbinding*.cpp`, QtScript). We already have a
  command registry with several languages; nothing to take.

## 4. Not worth taking

Scanning and cleanup (`tcleanupper`, `twain`, `scanpopup`): paper-pipeline
features for studios scanning drawings. Render farm (`flarefarm`). Stop motion
capture (`stopmotion`, `libgphoto2`, Canon SDK): niche, platform-heavy. The
Toonz raster/vector file formats (TLV, PLI, TZP): only useful for OpenToonz
interchange. Qt widgets and popups: we use egui. Bundled libraries
(`boost`, `glew`, `glut`, `openblas`, `superlu`, `quicktime`): Rust has
equivalents or doesn't need them.

## 5. Flare's own additions

Flare adds Animate-style rooms and themes, Flash-family import (`.fla`, `.xfl`,
`.swf`, `.swc`, legacy binary `.fla`), Moho import, a game-engine scaffold, a
Next2Flash/AS3 decompiler sidecar, and the Rust Studio on our core.

- **Legacy binary `.fla` and SWF readers** would complete our interchange
  story. Their code comments cite JPEXS (GPL-3.0) and an AGPL project as
  references ("ideas only"). Under our CONTRIBUTING.md we don't copy that
  code: we write our own readers from public format descriptions, and for SWF
  use Ruffle's `swf` crate (MIT/Apache).
- **Moho import** (`common/moho`, 850 lines + Rust 194): small; useful for
  migrating Moho users. Port with attribution after checking its provenance.
- **Their Rust Studio patches to our core** (row heights, partial tween
  copy, graphic playback direction, `TweenBoundary`) are listed in their
  `ORIGIN.md`. If we cooperate, take them back into `ac-core` so there is one
  engine.

## 6. How to port OpenToonz code safely

1. **Port from upstream `opentoonz/opentoonz`**, not from Flare's copy, unless
   Flare's version contains a fix we need. That keeps the provenance simple
   (DWANGO and OpenToonz contributors, BSD-3-Clause).
2. **Keep the notice.** BSD-3 requires the copyright notice and licence text
   in source and binary distributions. Add a `THIRD-PARTY-NOTICES.md` (and a
   line in the About box) listing OpenToonz, and put
   `// Ported from OpenToonz <path> (BSD-3-Clause, © DWANGO and contributors)`
   at the top of each ported Rust file.
3. **Check each file's header** before porting. Most stdfx/tools files are
   plain OpenToonz BSD. Look closer at: `stdfx/naru_maxflow`/`naru_graph`
   (max-flow for the LazyBrush fill; the well-known Boykov–Kolmogorov library
   is research/GPL-licensed, so confirm this is an independent version), the
   old Macromedia/Middlesoft SWF SDK in `common/flash` (1999), and anything
   in `thirdparty/`, which keeps its own licence.
4. **Bundled data**: the MyPaint brushes are CC0 (fine). Rhubarb Lip Sync is
   MIT, called as a separate program.
5. **Record the source** in the devlog for each port, as CONTRIBUTING.md asks.
6. Flare's `AI_POLICY.md` refuses AI-generated pull requests. That governs
   contributions *to Flare*, not our use of BSD code. If we later send changes
   to Flare, that policy has to be resolved first.

## 7. Suggested order

1. **Quick wins (1–2 weeks):** Magic Wand, Rhubarb lip-sync, PSD import,
   Shift & Trace, XDTS export, `THIRD-PARTY-NOTICES.md`.
2. **Drawing (4–6 weeks):** Trace Bitmap (outline), stroke styles (art /
   pattern / ragged brushes), vector deformation tools, drawing assistants and
   symmetry.
3. **Animation (4–6 weeks):** curve editor keyframe types + expressions,
   Jacobian IK with joint limits, mesh deformation of bitmaps/symbols.
4. **Painting and effects (2–3 months, after a GPU renderer):** MyPaint raster
   brushes, centerline vectorizer, particles, a curated effect set.
