# ViewSpace Keys (VS-Keys)

Status: **experimental and unstable.** Code: `flare_core::experimental::viewspace`
(`crates/foundation/flare-core/src/experimental/viewspace/`). Nothing here is wired into the
document model, the command batches, the project file or any front end. APIs may change or vanish.

This document is the full specification of the feature (so no outside paper is needed to finish it),
the status of every part, and the list of stub ids. Every stub in the code returns
`Err(ViewSpaceError::NotImplemented("..."))` and carries a `// TODO(viewspace:<id>)` comment holding
the algorithm to implement; `grep -rn "TODO(viewspace:" crates/` finds them. A test in
`viewspace/mod_tests.rs` checks that the status table below, the `STATUS` constant in `mod.rs` and
the TODO comments agree, so change all three together.

## 1. Goal

Automated tweening that extends, or works independently of, 2D rigging. The artist loads a custom
tab, customises a built-in base head mesh (and other body parts, combined into a full body), takes
keyframes of a character from different angles and overlays them on the mesh (the mesh is rotated
and moved to match). With a magic brush or select tool the artist selects parts of the keyframe and
maps them to semantic features with presets (eyes, eyebrows, nose, chin, cheek dents and lines,
pupils, hair and bangs, folds and creases, zippers...) and tags what each colour is (skin, clothes,
hair, eyes). From shadows, user data, angles and the base mesh a depth system like a heightmap
(Smack Studio style) with extra parameters makes a 3D-styled 2D rig. The user moves the character to
different angles and the result is interpolated from the keyframes. More keyframes can be added; the
result is editable in the engine (like Blender Grease Pencil edit mode); wrong results are fixed and
the rig updated. It also applies to expressions (the "angle" can be a kind or angle of expression)
and to clothing (more user setup). It mixes vector angle changing with Blender shape keys.

Primary goals: continuous view interpolation between sparse keyframes; artist control at every stage
(no black-box ML); an extension of bone rigs, not a replacement (it can feed shape-key-like deltas
into a bone system or run standalone); expressions and clothing as orthogonal parameter spaces
sharing the body correspondence; real-time preview (GPU-friendly) plus a higher quality offline
pass; graceful degradation with confidence visualisation.

Non-goals of v1: full 3D reconstruction; replacing frame timing tools; unsupervised learning that
replaces semantic mapping; real-time at 4K without level of detail.

## 2. Concept

Not pure bone deformation, not image morphing, not 3D reconstruction. It combines Smack Studio style
depth-augmented sprite projection, Adobe Character Animator style multi-view Head Turner, Live2D
style parameter-driven mesh deformation, Blender shape keys driven by view angle, and Grease Pencil
freehand correction. The output stays a 2D/2.5D billboard.

Key insight: register every keyframe into a common mesh plus semantic feature space and interpolate
in that parameter space, which encodes the correspondence, not in raw image space. That handles large
angles better than optical flow and stays editable by the artist.

## 3. The six layers

### 3.1 Base mesh / proxy (`mesh.rs`, `head.rs`)

A library of parametric base meshes (head, head and torso, full body as modular parts that snap
together; stylised variants), customised in a "Mesh Setup" tab: scale regions, move control points,
jaw width, eye spacing, forehead height. Low poly (hundreds to low thousands of vertices). It carries:
shared topology; a default depth/height field (an initial guess, like Smack Studio's auto depth, from
a simple extrusion or a template); semantic region labels (forehead, L/R cheek, nose bridge, upper and
lower lip, chin, eye sockets, hair root zones, clothing attachment zones) as integer ids plus a
human-readable dictionary; an optional bone hierarchy (it can drive or be driven); feature anchor
points (eye centres, mouth corners, ear tips). It stores the bind pose and the control points (cage);
cage deformation or linear blend skinning is enough for v1. A "snap to silhouette" helper projects the
mesh outline onto the keyframe silhouette after rough alignment. It is modular: start head-only, attach
a body later without losing the mappings.

Built: `BaseMesh` (the contract in section 7), validation, normals, bounds, serde, the parametric head
with its regions and anchors. Stubs: `mesh-snap`, `mesh-cage`, `mesh-modular`.

### 3.2 Keyframe capture and alignment (`keyframe.rs`)

Import PNG/PSD layers, Grease Pencil objects, sprite sheets, or draw in the tool. Per keyframe: rotate,
translate and scale the base mesh to match the pose; optional automatic rough alignment by feature
detection (eyes, nose, mouth, silhouette) if a CV helper or landmark model exists. Once aligned, the
keyframe is projected onto the mesh and stored as textures or stroke layers registered to the mesh UV and
feature space (not a free-floating image). It stores the exact view parameters (azimuth, elevation,
optional perspective strength, scale) so the keyframe is an anchored sample in view-space. More angles of
the same feature give better coverage.

Built: `ViewParams`, `MeshAlignment`, `Bitmap`, `KeyframeSample`, `register` (per-vertex registration with
a visibility weight, which is also the occlusion test for convex-ish parts). Stubs: `align-auto`,
`align-import`; stroke-layer sources are `interp-strokes`.

### 3.3 Semantic mapping / Magic Brush (`features.rs`, `magic.rs`)

Selection tools: lasso (freehand and polygonal), brush with hardness and size, magic wand (colour and
edge tolerance, contiguous or not), region grow/shrink/border/smooth, stroke-aware selection (snaps along
line art), quick-mask overlay. Assign selections to pre-built feature presets or custom named features:
facial (L/R eye, pupil, iris, eyebrow, nose, mouth upper/lower lip, teeth, tongue optional, chin, cheek
fold/dent, forehead, ear), hair (bangs, side locks, main volume, ponytail/accessory), clothing and body
(collar, sleeve, fold/crease, zipper, seam, belt, pocket, scar, accessory attachment), and custom user
features stored in the project. A second pass tags colour and material: skin, hair, eyes, clothing layers,
accessories; materials survive interpolation and drive shading, outline and deformation (hair gets softer
depth blending than hard clothing edges). The result is a feature correspondence field between keyframe
pixels or strokes and the mesh semantic regions.

Magic brush details: pressure-sensitive size and hardness; colour tolerance plus edge bias; stroke-aware
mode; semantic-aware grow (stay inside the same material/feature once seeded); a multi-keyframe consistency
check that warns when the same feature is mapped very differently across keyframes; quick-assign hotkeys
(number keys, radial menu); overlay colour coding with solo/hide; after confirming, the correspondence field
updates and may re-estimate local depth.

Built: presets with number-key hotkeys, custom features, materials, `RegionMask`, `assign_feature`, wand
(contiguous or not, edge bias), grow, shrink, border, smooth, polygon lasso, brush stamp with hardness and
pressure, combine. Stubs: `magic-stroke-aware`, `magic-semantic-grow`, `magic-consistency`.

### 3.4 Depth and parameter fields, "Heightmap++" (`fields.rs`)

Auto depth is seeded from the aligned mesh, the silhouette and the user's shadows and highlights, then the
artist paints and refines it. Editable fields in mesh parameter space: depth/height (lighter is closer);
thickness/extrusion (prevents paper-thin rotation); occlusion priority / layering order (hair, clothing,
limbs); softness/feather (edge hardness); material-specific deformation weights (per-region rigidity
multipliers); optional normal/curvature hints. Fields interpolate with the view like textures.

Built: per-vertex fields named `thickness`, `occlusion`, `softness`, `rigidity` plus depth, linear and gamma
blending, a depth/field brush. `thickness` and `occlusion` are used by `evaluate`; `softness` and
`rigidity` are stored and blended but consumed only by the stubs `style` and `material-blend`. Stub:
`depth-auto`.

### 3.5 Interpolation engine (`neighbours.rs`, `interp.rs`, `project.rs`, `confidence.rs`)

View-space parameterisation: azimuth, elevation, optionally distance/perspective, as continuous
parameters (like Live2D angle parameters or Blender shape-key drivers on a camera bone). For a novel
angle: 1) find the nearest keyframes / weighted neighbours in view-space; 2) interpolate registered
textures and stroke data in the common mesh parameter space; 3) apply the interpolated depth and parameter
fields; 4) reconstruct 2D by projecting the deformed mesh and textures to the canvas (compute-shader
friendly). Extra blend modes: feature-wise (eyes weighted toward one keyframe, hair toward another; the
artist can pin features to particular keyframes), expression-weighted (deltas after or jointly with view
interpolation), clothing/state layers swappable and blendable independently. Sparse-data fallback:
mesh-driven deformation plus simple parallax or morph, with visible confidence/coverage indicators.

Built: all of the CPU reference path: `ViewSpaceInterpolator::evaluate(view, expr, clothing)`, region pins,
thickness, occlusion bias, depth-buffered rasteriser, grey zero-confidence fallback for unseen vertices.
Stubs: `interp-strokes`, `interp-gpu`, `lod`, `style`, `material-blend`.

### 3.6 In-engine correction, Grease Pencil style (`edit.rs`)

Edit Mode on a generated frame or on the parameter space: draw/move strokes, repaint depth, reassign
features, adjust mesh control points. Changes are either local to that view (they become a corrective
keyframe, denser sampling) or propagated back into the shared feature maps / base mesh. The loop is
generate, inspect, correct, regenerate.

Built: `corrective_keyframe`, `commit_corrective`. Stub: `edit-push`.

## 4. Expressions (`expression.rs`)

An orthogonal (or loosely coupled) parameter space to view angle. The user supplies expression keyframes
(neutral, smile, angry, surprise...) at one or more angles; the system builds expression shape keys / delta
maps in the same view-space. At runtime: view interpolation first, then expression deltas (or a joint
multi-dimensional interpolation when there is enough data). Expressions can be driven by bone/shape-key
controls or by magic-brush features (mouth corner positions as drivers). Example: 3/4 angry is interpolated
from front-angry plus 3/4-neutral, and the user can override at any time. Deltas are stored per vertex or
per feature as offsets relative to neutral at the same view angle when possible; with one angle only, the
delta is applied after view interpolation (approximate).

Built: `ExpressionDelta::from_meshes`, `apply_expression` (weights 0 to 2; unknown names are errors).
Stubs: `expr-from-keyframes`, `expr-view`.

## 5. Clothing / variant states (`clothing.rs`)

Extra semantic layers or alternate mesh skins; they may share the body mesh and correspondence when the
silhouette is similar, or be fully independent (cape, armour). Clothing features are mapped with the same
magic brush; a "duplicate body mapping" action means only clothing regions are re-mapped; clothing-aware
depth/occlusion helper suggestions come from semantic tags (hair is usually in front of the collar) and are
overridable; the UI shows shared versus unique per state.

Built: state selection with fallback to the shared samples (`usable_samples`). Stubs: `cloth-dup`,
`cloth-occlusion`.

## 6. UI (stub `ui-tab`; not wired to the desktop app)

A custom tab. Left: asset browser (base meshes, keyframes, expression sets, clothing variants, saved view
configurations). Centre: viewport (mesh and overlaid keyframe, rotation gizmos, onion skin of neighbouring
views, confidence heatmap overlay, depth preview). Right tabs: feature mapping / magic brush and presets,
depth and parameter paint, colour/material tagging, interpolation settings and confidence, expression and
clothing managers. Bottom: view-sphere navigator / timeline for browsing keyframes and generated
intermediates, with Generate & Bake. Mapping and correction actions take one or two inputs. This is a debt
row in `docs/SURFACES.md`.

## 7. Data contracts

Implemented as Rust types with typed ids (`KeyframeId`, `FeatureId`, `FeaturePreset`, `Material`):

- `BaseMesh { vertices: Vec3[] (bind pose), faces: [[u32;3]], uvs, region_ids (u16 per vertex), region_dict:
  Map<u16,String>, default_depth (f32 per vertex), control_points (cage), bones: Option<BoneHierarchy>,
  feature_anchors: Map<String,Vec3> }`
- `KeyframeSample { id, source (raster; strokes are a stub), view: ViewParams { azimuth, elevation: Option,
  distance: Option, perspective: Option }, alignment (scale, offset, roll), feature_map: Map<FeatureId,
  RegionMask>, material_ids (per pixel), depth_override: Option<f32[]>, other_param_fields: Map<String,f32[]>,
  expression_tag, clothing_state, is_corrective }`
- Feature correspondence field: which samples contribute to a point or region and with what weight under a
  view. Computed on the fly by `neighbours::select` plus the visibility from `register`; the
  `Renderable2D::contributions` list reports it. A pre-baked form is not built.
- `ViewSpaceInterpolator`: holds samples, the base mesh, pins, expression deltas and settings;
  `evaluate(view, expr, clothing) -> Renderable2D` does 1) select and weight neighbours, 2) interpolate
  colour and fields in mesh space, 3) apply expression deltas, 4) project with depth and occlusion,
  5) return the textured picture with depth, confidence and the heatmap.

Conventions: right-handed; +x is to the viewer's right in the front view, +y up, +z towards the camera at
azimuth 0; angles in radians; azimuth turns positive towards the character's left (+x); triangles
counter-clockwise from outside; "L" regions are the character's left (+x).

## 8. Algorithm

8.1 Neighbour selection: each keyframe is a point `(azimuth, elevation)` on a sphere (or a cylinder when
elevation is limited); angular distance (great circle or simple difference) to all samples; pick the k nearest
(2 to 4 is usually enough) or all within a threshold; blend weights are inverse-distance, spherical linear,
or a small RBF, normalised to sum 1; optional per-feature weight overrides (the artist pins features to
keyframes). Built as `neighbours.rs` (great circle, k = 1..8, `InverseDistance`, `Slerp`, `Gaussian`, exact
hit gives weight 1) and `ViewSpaceInterpolator::pin_region`.

8.2 Mesh-space interpolation: per vertex (or per texel in a mesh-registered atlas) look up the value from each
contributing keyframe via the correspondence / UV registration; blend with the weights (colour in a suitable
colour space, premultiplied; depth linear or a chosen non-linear mix; binary masks softly blended then
thresholded, or kept soft); apply expression delta maps (additive or multiplicative) after the view blend.
Built per vertex: premultiplied colour, weighted by weight times visibility; depth linear or gamma;
expression deltas additive. Per-texel atlas and multiplicative deltas are not built.

8.3 Projection / compositing: transform the base mesh by the current view (plus bone-driven secondary
animation), project vertices to the canvas, use the interpolated depth and occlusion priority to resolve
visibility (painter's algorithm, depth buffer or layered), rasterise strokes with interpolated thickness,
softness and material, optional style rules (outline thickness scaling with foreshortening, cel-shade bands
from normal/curvature hints). Built: depth buffer with occlusion bias, Gouraud colour. Stub: `style`.

8.4 Confidence / coverage: per pixel or region, for example the inverse of the angular distance to the nearest
sample, or coverage including feature completeness; heatmap or outline. Built per vertex as
`angular * coverage` (`confidence.rs`), rendered as a heatmap.

## 9. Limits already in the code

All input is untrusted and everything that grows has a bound: 50 000 vertices, 512 bones, 64 keyframe
samples, 64 expressions, 256 features per keyframe, images at most 8192 per side and 32 million pixels,
128 rings and segments in the head, 256 morphology iterations, 100 000 lasso points, 4096 frames per
sequence, 1 to 8 neighbours. Flood fills are iterative. Errors are `ViewSpaceError`; no function panics
on bad data.

## 10. Edge cases and rules

- Correspondence quality is the hardest problem (optical flow fails on large rotations, style changes, and
  parts appearing, such as an ear becoming visible), so always work in mesh/feature space.
- Sparse data must degrade gracefully and never invent unjustified detail (unseen vertices are flat grey with
  zero confidence). Offer "suggest next keyframe angle" maximising information gain (built as
  `suggest_next_view`, a max-min angular gap).
- Style (outline thickness, cel rules, banding) is parameter-driven, never baked into textures.
- Performance: GPU compute is feasible at 512 to 2048 px; provide an LOD / low-resolution preview and an
  offline denser pass (`interp-gpu`, `lod`).
- Silhouette and topology changes need extra keyframes, local corrective meshes, or silhouette-override
  strokes active only in certain view ranges.
- Occlusion and layering (hair, clothing, limbs) use the occlusion priority map plus semantic layering rules,
  with artist override.
- Known limit of the CPU path: occlusion inside a keyframe is decided by facing only, which is right for a
  convex head and wrong where one part hides another (a hand over a face). Needs a depth test per sample in
  `register`.

## 11. Rig integration

Standalone (no bones); upstream provider (outputs deformed mesh / shape-key-like deltas into a conventional 2D
bone rig, which adds secondary motion and IK); downstream consumer (a bone rig drives the base mesh first, the
view system applies view-dependent appearance on top). Start unidirectional; bidirectional later. Flare
already has `flare-rig` (bones, symbols) and `flare-tween` (interpolation; raster in-betweening in
`flare_tween::raster`): integrate by reading their types, do not duplicate. `flare-core` is below them, so
the integration lives in a crate above (stubs `rig-upstream`, `rig-downstream`).

## 12. Export targets

Sprite sheets / image sequences at chosen angles and expressions (the frames come from `render_sequence`;
`flare_runtime::sprite_sheet` packs them, in a crate above this one); a Live2D-style parameter model; a
Blender Grease Pencil object with shape keys (one key per major view/expression sample plus drivers); a custom
runtime format preserving the base mesh, keyframe samples and correspondence for hosts that implement
`evaluate()`; a layered PSD with clear layer names.

## 13. Phases and status

MVP success: an artist takes 3 to 5 head keyframes, maps the major features in under 30 minutes, scrubs
continuous left-right turns with acceptable quality, and fixes the worst frames in Edit Mode.

- Phase 0, foundations: base mesh with a simple parametric head, a viewport with the mesh and one overlaid image
  with transform controls, manual depth paint on the mesh, simple projection of painted depth and image onto the
  mesh for a novel view.
- Phase 1: magic-brush selection and assignment to a small hard-coded feature set (eyes, nose, mouth, hair),
  several keyframe samples with view parameters, neighbour selection and mesh-space colour/depth blending for
  continuous azimuth scrubbing, simple distance-based confidence visualisation.
- Phase 2: Edit Mode (draw/correct on a generated view, save as a corrective keyframe or push back to shared
  maps), expression deltas (neutral and at least one other), material tagging and material-aware blend rules.
- Phase 3: clothing state and the duplicate-mapping workflow, more presets, better occlusion tools, a
  performance pass, export to a sprite sheet and at least one of Grease Pencil / Live2D / the custom runtime,
  a full-body modular mesh.

### Status table

`done` rows are built and tested; `stub` rows return `ViewSpaceError::NotImplemented` and have a
`// TODO(viewspace:<id>)` in the named file with the algorithm.

| Id | Status | Phase | What | File |
|---|---|---|---|---|
| `mesh-head` | done | 0 | parametric head template, regions, anchors | `head.rs` |
| `mesh-validate` | done | 0 | BaseMesh validation, normals, bounds, serde | `mesh.rs` |
| `view-params` | done | 0 | ViewParams, MeshAlignment, Bitmap, KeyframeSample, registration | `keyframe.rs` |
| `depth-paint` | done | 0 | manual depth/field paint on the mesh | `fields.rs` |
| `project` | done | 0 | camera, projection, depth-buffered rasteriser | `project.rs` |
| `mesh-snap` | stub | 0 | snap mesh outline to a silhouette | `mesh.rs` |
| `mesh-cage` | stub | 0 | cage deformation / skinning of the mesh | `mesh.rs` |
| `mesh-modular` | stub | 3 | attach body parts without losing mappings | `mesh.rs` |
| `align-auto` | stub | 1 | automatic alignment from landmarks | `keyframe.rs` |
| `align-import` | stub | 1 | PSD / sprite sheet / Grease Pencil import | `keyframe.rs` |
| `feature-presets` | done | 1 | presets, custom features, hotkeys, materials, masks, assignment | `features.rs` |
| `magic-wand` | done | 1 | magic wand (contiguous or not, edge bias) | `magic.rs` |
| `magic-morph` | done | 1 | grow, shrink, border, smooth, combine | `magic.rs` |
| `magic-lasso` | done | 1 | lasso polygon and brush stamp | `magic.rs` |
| `neighbours` | done | 1 | view-sphere distance, k-nearest, IDW/slerp/Gaussian weights | `neighbours.rs` |
| `depth-blend` | done | 1 | field blending, linear and gamma | `fields.rs` |
| `interp-evaluate` | done | 1 | `evaluate`, pins, fallback | `interp.rs` |
| `confidence` | done | 1 | per-vertex confidence and heatmap | `confidence.rs` |
| `confidence-suggest` | done | 1 | suggest next keyframe view | `confidence.rs` |
| `magic-stroke-aware` | stub | 1 | selection snapping to line art | `magic.rs` |
| `magic-semantic-grow` | stub | 1 | grow limited to a material/feature | `magic.rs` |
| `magic-consistency` | stub | 1 | multi-keyframe feature consistency warning | `features.rs` |
| `depth-auto` | stub | 1 | auto depth from silhouette and shading | `fields.rs` |
| `edit-corrective` | done | 2 | corrective keyframes from a repainted frame | `edit.rs` |
| `expr-apply` | done | 2 | expression deltas from meshes and their application | `expression.rs` |
| `material-sharpen` | done | 2 | material-aware weight sharpening function | `quality.rs` |
| `edit-push` | stub | 2 | push a correction back into shared maps | `edit.rs` |
| `expr-from-keyframes` | stub | 2 | expression deltas from expression keyframes | `expression.rs` |
| `expr-view` | stub | 2 | view-dependent expression deltas | `expression.rs` |
| `material-blend` | stub | 2 | material rules wired into `evaluate` | `quality.rs` |
| `cloth-select` | done | 3 | clothing state sample selection with fallback | `clothing.rs` |
| `export-sequence` | done | 3 | render a sequence of views (sprite sheet input) | `export.rs` |
| `cloth-dup` | stub | 3 | duplicate body mapping for a clothing state | `clothing.rs` |
| `cloth-occlusion` | stub | 3 | clothing-aware occlusion suggestions | `clothing.rs` |
| `export-live2d` | stub | 3 | Live2D-style parameter model | `export.rs` |
| `export-gp` | stub | 3 | Grease Pencil shape-key export | `export.rs` |
| `export-runtime` | stub | 3 | custom runtime format | `export.rs` |
| `export-psd` | stub | 3 | layered PSD export | `export.rs` |
| `style` | stub | 3 | outline and cel-shade style rules | `quality.rs` |
| `interp-strokes` | stub | 3 | stroke-layer keyframes | `quality.rs` |
| `interp-gpu` | stub | 3 | wgpu compute evaluation | `quality.rs` |
| `lod` | stub | 3 | LOD presets | `quality.rs` |
| `rig-upstream` | stub | 3 | deltas into a bone rig | `quality.rs` |
| `rig-downstream` | stub | 3 | bone rig drives the mesh | `quality.rs` |
| `ui-tab` | stub | 3 | the custom tab (UI); described in section 6, no code | `mod.rs` (doc) |

## 14. Future

Automatic next-keyframe suggestion beyond the angular gap; cross-character learning; lighting and shadow
transfer; physics-aware secondary motion; batch from video reference; full-body turnaround; an optional small
ML model for feature proposals (the artist always has the final say).

## 15. Open questions

- Target host app or plugin (inside Flare only, or also a plugin for other tools)?
- Renderer / compute path (Vulkan, Metal, GL; Flare uses wgpu, which is the working assumption).
- Is tablet / pressure input mandatory on day one? (Brush pressure is in the algorithms; input is not.)
- How important is a Grease Pencil / Live2D round trip?
- Maximum resolution and the number of simultaneously loaded keyframes (the code caps 8192 px and 64 samples).
- Non-humanoid topologies out of the box?
- Constraints on shipping ML models (size, licensing, offline use)?

## Glossary

Base Mesh / Proxy; Feature Correspondence Field; View-Space (camera azimuth and elevation plus optional
distance and perspective); KeyframeSample; Corrective Keyframe (made in Edit Mode); Heightmap / Depth Field
(per vertex or texture); Magic Brush; Expression Delta (the offset turning neutral into the target); Clothing
State.

References (ideas only, no code copied): Smack Studio depth maps, Adobe Character Animator Head Turner
(discrete views), Live2D Cubism (angle parameters), Blender Grease Pencil and shape keys.
