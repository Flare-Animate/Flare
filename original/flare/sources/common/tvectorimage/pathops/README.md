# vectorcraft-pathops

Path operations for VectorCraft: booleans, the Pathfinder panel, Shape Builder regions, Live Paint planar maps, offset path, outline stroke, simplify and the other Object → Path commands. Everything takes and returns `vectorcraft_geom::PathData`.

## Booleans (curve-preserving)

```rust
pub enum BoolOp { Union, Intersect, Difference, Xor }
pub fn boolean(a: &PathData, a_rule: FillRule, b: &PathData, b_rule: FillRule, op: BoolOp) -> PathData;
pub fn try_boolean(.., precision: f64) -> Result<PathData, PathOpsError>; // errors on NaN/inf
pub fn boolean_n(paths: &[(&PathData, FillRule)], pred: impl Fn(&[bool]) -> bool) -> PathData;
pub fn unite_all(paths: &[(&PathData, FillRule)]) -> PathData;   // fast many-shape union
pub fn normalize(path: &PathData, rule: FillRule) -> PathData;   // resolve self-overlaps
pub fn area(path: &PathData, rule: FillRule) -> f64;             // exact filled area
```

The engine is `linesweeper` (a robust sweep line that works directly on cubic Béziers, so there's no polygon flattening). The sweep splits curves at intersections and y-extrema. Afterwards:

- pieces that lie on one input cubic are rebuilt exactly as a single sub-curve of that cubic;
- other smooth joints (for example, in stroker output) are refitted by least squares within `DEFAULT_PRECISION` (0.01 pt);
- collinear line pieces are merged;
- the inputs' own anchors are always kept;
- slivers narrower than the precision are dropped.

Output contours are simple and consistently oriented: outer contours and holes wind in opposite directions, so the result fills the same under either fill rule. Open subpaths are closed implicitly because they're treated as filled.

## Pathfinder / Shape Builder

```rust
pub struct Shape { pub path: PathData, pub rule: FillRule, pub key: u64 } // key = paint identity
pub enum PathfinderOp { Unite, MinusFront, Intersect, Exclude, Divide, Trim, Merge, Crop, Outline, MinusBack }
pub fn pathfinder(op: PathfinderOp, shapes: &[Shape] /* back → front */) -> Vec<Shape>;

pub struct Region { pub path: PathData, pub sources: Vec<usize> }
pub fn regions(shapes: &[Shape]) -> Vec<Region>;            // all faces of the arrangement
pub fn region_at(shapes: &[Shape], p: Point) -> Option<Region>;
pub fn merge_regions(regions: &[&Region]) -> PathData;      // Shape Builder merge = union
pub fn shape_builder(shapes: &[Shape]) -> (Vec<Region>, Vec<Shape>); // (regions, open-path pieces)
```

Each result takes its key as follows:

| Operation | Result |
|---|---|
| Unite, Intersect, Exclude, Minus Back | Takes the front-most key |
| Minus Front | Takes the back-most key |
| Divide | One shape per face, keyed by the front-most shape that covers the face |
| Trim | One shape for each object that is still visible |
| Merge | Joins touching visible parts that share a key; each connected component becomes one shape |
| Crop | Keeps the visible parts of the lower objects inside the front-most object |
| Outline | Returns open edge paths, split at every junction; coincident edges are de-duplicated, and each keeps the front-most key |

## Live Paint

```rust
pub fn live_paint(shapes: &[Shape] /* back → front */) -> (Vec<Region>, Vec<Shape>); // (faces, edges)
pub fn interior_point(path: &PathData) -> Option<Point>;
```

Unlike `regions`, which splits the filled areas of closed shapes, `live_paint` treats every path as an edge, open paths included. Three crossing lines enclose a triangle, a line across a rectangle splits it in two, and an area enclosed by paths becomes a face even when nothing fills it. Each face lists the inputs whose fill covers it; the list is empty for an area that is only enclosed by paths. Each edge is a piece of a path, split wherever another path meets it, and keeps the key of the front-most input it lies on.

`shape_builder` gives the Shape Builder's regions. With closed paths only, they are the `regions`. Once an open path is among them, they are the `live_paint` faces, so a stroke-only line across a shape splits it and lines can enclose a region of their own. The second list holds the pieces of each path that has no closed subpath, keyed by its index, so the Shape Builder can delete them one by one. The caller closes any open path whose fill should count. Inputs with more than `SHAPE_BUILDER_MAX_SEGMENTS` segments fall back to the filled areas.

## Offset / stroke

```rust
pub use kurbo::{Cap, Join};
pub fn offset_path(path: &PathData, delta: f64, join: Join, miter_limit: f64) -> PathData; // delta < 0 insets
pub fn outline_stroke(path: &PathData, width: f64, cap: Cap, join: Join, miter_limit: f64) -> PathData;
```

`outline_stroke` passes the path to `kurbo::stroke` and then normalizes the result, which removes the overlaps at joins. `offset_path` computes `fill ∪ stroke(2|d|)` for a positive offset and `fill − stroke(2|d|)` for a negative one.

## Path editing

```rust
pub fn simplify(path: &PathData, tolerance: f64) -> PathData;
pub fn simplify_with(path: &PathData, opts: &SimplifyOptions) -> PathData; // tolerance, corner_angle_deg, straight_lines
pub fn smooth(path: &PathData, amount: f64) -> PathData;
pub fn remove_redundant_points(path: &PathData, tolerance: f64) -> PathData;
pub fn add_anchor_points(path: &PathData) -> PathData;
pub fn average(path: &PathData, selection: &[(usize, usize)], axis: AverageAxis) -> PathData;
pub fn join(paths: &[PathData], tolerance: f64) -> PathData;
pub fn split_into_grid(rect: Rect, rows: usize, cols: usize, gutter: f64) -> Vec<PathData>;
```

`simplify` fits least-squares cubics with fixed end tangents, then reparameterizes with Newton steps and splits at the point of worst error. Any anchor that turns more than the corner threshold (30° by default) stays a corner.

## Caveats

- linesweeper is still in beta. When two identical curves overlap, the sweep can leave a tiny lens between them; the sliver filter removes it.
- The Outline op finds split points by snapping to arrangement junctions with a small tolerance.
- The quality of offsets depends on kurbo's parallel-curve stroker. Its output is refitted, so offset results have more anchors than boolean results.
