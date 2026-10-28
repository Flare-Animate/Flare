//! Minimal geometry kernel inlined from `vectorcraft-geom` (same upstream commit as the crate).
//! Only what path-ops needs: the anchor/sub-path/path model (`path`), Bezier helpers (`bez`),
//! point-in-fill and outline distance (`hit`) and the rectangle/ellipse primitives (`shapes`).
//! Source text is verbatim apart from `crate::` path fixes and the trimmed `shapes`.

pub mod bez;
pub mod hit;
pub mod path;
pub mod shapes;

pub use kurbo;
pub use kurbo::{Affine, BezPath, CubicBez, Line, ParamCurve, PathEl, PathSeg, Point, Rect, Shape, Size, Vec2};
pub use path::{Anchor, AnchorKind, FillRule, PathData, SubPath};

/// Geometric tolerance used by the path model.
pub const EPS: f64 = 1e-9;
