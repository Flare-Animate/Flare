# Absorbing AnimationCraft

[AnimationCraft](https://github.com/KMSDax/AnimationCraft) (remote: `animationcraft`)
is a Rust rewrite of Adobe Animate's authoring model: symbols, layers, keyframes,
classic/shape/motion tweens, a Motion Editor, onion skin, and XFL read/write.
Flare is being rewritten into Rust toward the same target, so the question is
what AnimationCraft has that Flare does not, and what Flare already does better.

## License

AnimationCraft's workspace manifest declares **`MIT OR Apache-2.0`**
(`Cargo.toml`, `[workspace.package] license`). Both are permissive and compatible
with Flare's BSD-3-Clause. Vendored code must keep its notice; this document,
`doc/ANIMATE-GAP-ANALYSIS.md`, and any ported file are the places that notice
goes. Nothing in AnimationCraft is a signed or proprietary import: it is
clean-room work from Adobe's public documentation and the published XFL
structure, and carries no Adobe code, artwork, icons or sample files.

## Per-module verdict

Measured against Flare at commit `bc402dc34`. "Flare does better" means the
capability exists in the tree, not that it is equally polished.

| AnimationCraft module | Size | Verdict | Why |
|---|---|---|---|
| `ac-core/merge.rs` | 53 KB | **PORT — highest value** | The merge-drawing planar-map shape model. Flare/OpenToonz has no equivalent: no edge bending, no merge-shape editing, no planar boolean cut. This is Flash's defining drawing feature and the biggest single gap Flare has against Animate. |
| `ac-core/editor.rs` | 130 KB | Skip | Undo/redo, selection and editing commands. Flare has `TUndo`, `TPixelBlending` selection and the full command set in `flareqt`/`flarelib`. |
| `ac-core/commands.rs` | 65 KB | Skip | Same as above, at the command-dispatch layer. |
| `ac-core/model.rs` | 50 KB | Skip | Document model. Flare's `TXsheet`/`TXshLevel`/`TXshCell`/`TStageObject` graph is deeper and already serialised. |
| `ac-core/geom.rs` | 41 KB | Skip | `Path`/`Pt`/`Seg` geometry. Flare has `tline`, `tbezier`, `tcurves`, `tgeometry`, all in use by the renderer. Reimplementing it would create a second, incompatible geometry type — the classic way to make a port impossible later. |
| `ac-core/motion.rs` | 38 KB | Skip for now | Motion tweens. Flare has the deeper model already: `TFunction`, `TMotionTween`, `TMotionPath` with real curve editing and graph eases. Revisit only if a specific capability is missing, not as a wholesale port. |
| `ac-script` | 33 KB | Defer | JS/Python scripting console. Flare has `TExpression`; Animate's ActionScript is explicitly out of scope for both projects. No unique value yet. |
| `ac-render` | 29 KB | Skip | `tiny-skia` rasterisation. Flare has `tgl`/`TRaster`/OpenGL and the whole existing renderer. |
| `ac-core/deform.rs` | 16 KB | Skip | Mesh/bone deformation. Flare has `plasticskeleton`, `plasticskeletondeformation`, `tcurves` rigs — the full rig system, and further along. |
| `ac-core/rig.rs` | 11 KB | Skip | Same reason. |
| `ac-core/fx.rs` | 8 KB | Skip | Flare has `tffx`/`tfx` effect nodes and a far larger effect set. |
| `ac-core/frames.rs` | 13 KB | Skip | Frame/span bookkeeping. Flare has `tframe`, `tconstpool`, column-level cells. |
| `ac-core/text.ts` | 6 KB | Skip | Minimal text. Flare's text stack is much larger; Animate's dynamic text is the real gap and neither project has it. |
| `ac-core/xfl.rs` | 3 KB | Skip | Upstream marks it `EXPERIMENTAL — needs a corpus of real files`. Flare has a tested one: `common/flash/XFLShape.cpp` (`decodeEdges`, `decodeShapeXml`) and `common/flash/rust/xfl.rs`. Taking the experimental one would be a regression. |
| `ac-app`, `ac-cli` | — | Skip | `egui` UI and a CLI. Flare is Qt; the UI is not portable and not wanted. |

## The one that matters: merge.rs

`merge.rs` is the only AnimationCraft module that fills a hole Flare has rather
than duplicating something Flare already does well. It gives:

- `FillRegion` / `StrokeEdge` / `MergeShape` — the planar map with a separate
  fill region and stroke edges per shape, which is what lets a Flash artist cut
  a shape with another shape and keep the edges usable for stroke.
- `bool_polys(a, b, BoolOp)` — planar boolean ops via the `i_overlay` crate.
- `Bend` — edge bending along a bezier, the "reshape" tool.
- `morph(a, b, t)` — shape tweening between keyframes.
- `PaintMode`, `path_polygons`, `path_polylines` — the flattening step.

### Why it cannot be dropped in

It is not self-contained. Its dependency closure inside AnimationCraft is
`crate::geom` (`Path`, `Pt`, `Seg`, `Matrix`, ~41 KB) and `crate::model`
(`Color`, `Gradient`, `Stroke`), plus `i_overlay`, `serde`, and `serde_json`
(already dependencies of `flare_formats`). Porting the file alone would either
drag a second geometry type into the tree or need a translation layer between
Flare's `TLine`/`TBezier` and AnimationCraft's `Path`.

### Port order

1. **`geom` to `common/tcurves/rust/path.rs`** — `Pt`, `Seg`, `Path`, `Matrix`
   only. No dependency on `model`. This is the piece Flare can use on its own:
   Flare's `TLine`/`TBezier` are the C++ side of the same thing, so a Rust
   `Path` is the forward-compatible form for the renderer rewrite.
2. **`model`'s paint types to `common/tcolor/rust/paint.rs`** — `Color`,
   `Gradient`, `Stroke`. Flare has `TColorStyle`/`TGradient`/`TStrokeStyle` in
   C++; the Rust mirror keeps the two in step behind the same C ABI style used
   for the format modules.
3. **`merge.rs` to `common/tmerge/rust/merge.rs`**, registered in the
   `#[path]` list in `flare/sources/rust/flare_formats.rs`. Add `i_overlay` to
   `flare/sources/Cargo.toml`.
4. Only then wire it to the UI. A merge model nobody can edit is a library, not
   a feature.

Each step is independently testable, and steps 1–2 are useful on their own.
Steps 3–4 are the actual feature.

## Keeping the reference current

`doc/ANIMATE-GAP-ANALYSIS.md` is a vendored copy of AnimationCraft's gap
analysis: a feature-by-feature comparison of Animate against their project. It
is the most precise statement of the target this project works toward. Refresh
it with:

```
git fetch animationcraft
git show animationcraft/HEAD:docs/ANIMATE-GAP-ANALYSIS.md > doc/ANIMATE-GAP-ANALYSIS.md
```

(re-add the attribution header at the top when doing so).

## License texts

Both licenses are short and both permit redistribution inside Flare provided the
notice travels with the code:

```
git show animationcraft/HEAD:Cargo.toml          # declares MIT OR Apache-2.0
https://opensource.org/license/mit
https://www.apache.org/licenses/LICENSE-2.0
```

Any file ported from AnimationCraft keeps a header naming the upstream file and
the `MIT OR Apache-2.0` dual license; the top-level `LICENSE.txt` records that
Flare contains such files. Nothing from AnimationCraft may be signed into
Flare's BSD-3-Clause-only terms without that header.