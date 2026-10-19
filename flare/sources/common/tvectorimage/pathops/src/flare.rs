//! Adapter between Flare's own vector path (`ac_core::geom::Path`, f32 `M/L/Q/C/Z` segments) and
//! this crate's native `PathData`/`BezPath` (f64 anchors with handles).
//!
//! Mapping:
//! * `Seg::M/L/C/Z` map one to one onto kurbo `MoveTo/LineTo/CurveTo/ClosePath`; `Seg::Q` becomes a
//!   `QuadTo` (the native model elevates quadratics to cubics, so results carry `C` segments only).
//! * Coordinates widen f32 to f64 exactly; results are rounded back to f32.
//! * Both operands are treated as filled shapes (open sub-paths are implicitly closed by the fill
//!   semantics of the sweep). The result is a compound path: holes are separate counter-wound
//!   sub-paths, filled with [`FillRule::NonZero`] unless the `_with_rule` variants say otherwise.
//! * Non-finite input (NaN/inf) yields an empty `Path`, as the native `boolean` does.
//!
//! Strokes (`ac_core::model::Stroke` is only colour + width) are not paths: call
//! [`stroke_outline`] to turn a path plus a width into a fillable outline first.

use ac_core::geom::{pt, Path, Pt, Seg};
use kurbo::{BezPath, PathEl, Point};

use crate::geom::{FillRule, PathData};
use crate::{boolean, outline_stroke, BoolOp, Cap, Join};

fn p64(p: Pt) -> Point {
    Point::new(f64::from(p.x), f64::from(p.y))
}

fn p32(p: Point) -> Pt {
    pt(p.x as f32, p.y as f32)
}

/// Flare path to kurbo path.
pub fn to_bezpath(path: &Path) -> BezPath {
    let mut bp = BezPath::new();
    for s in &path.segs {
        match *s {
            Seg::M(p) => bp.move_to(p64(p)),
            Seg::L(p) => bp.line_to(p64(p)),
            Seg::Q(q, p) => bp.quad_to(p64(q), p64(p)),
            Seg::C(a, b, p) => bp.curve_to(p64(a), p64(b), p64(p)),
            Seg::Z => bp.close_path(),
        }
    }
    bp
}

/// kurbo path to Flare path.
pub fn from_bezpath(bp: &BezPath) -> Path {
    let segs = bp
        .elements()
        .iter()
        .map(|el| match *el {
            PathEl::MoveTo(p) => Seg::M(p32(p)),
            PathEl::LineTo(p) => Seg::L(p32(p)),
            PathEl::QuadTo(q, p) => Seg::Q(p32(q), p32(p)),
            PathEl::CurveTo(a, b, p) => Seg::C(p32(a), p32(b), p32(p)),
            PathEl::ClosePath => Seg::Z,
        })
        .collect();
    Path { segs }
}

/// Flare path to the native anchor model.
pub fn to_path_data(path: &Path) -> PathData {
    PathData::from_bezpath(&to_bezpath(path))
}

/// Native anchor model back to a Flare path.
pub fn from_path_data(pd: &PathData) -> Path {
    from_bezpath(&pd.to_bezpath())
}

/// Boolean `op` of `a` and `b`, each filled with its own rule.
pub fn combine_with_rule(a: &Path, a_rule: FillRule, b: &Path, b_rule: FillRule, op: BoolOp) -> Path {
    from_path_data(&boolean(&to_path_data(a), a_rule, &to_path_data(b), b_rule, op))
}

/// Boolean `op` of `a` and `b`, both non-zero filled.
pub fn combine(a: &Path, b: &Path, op: BoolOp) -> Path {
    combine_with_rule(a, FillRule::NonZero, b, FillRule::NonZero, op)
}

/// `a` united with `b`.
pub fn union(a: &Path, b: &Path) -> Path {
    combine(a, b, BoolOp::Union)
}

/// The area covered by both `a` and `b`.
pub fn intersect(a: &Path, b: &Path) -> Path {
    combine(a, b, BoolOp::Intersect)
}

/// `a` with `b` cut out (`a - b`).
pub fn subtract(a: &Path, b: &Path) -> Path {
    combine(a, b, BoolOp::Difference)
}

/// The area covered by exactly one of `a`, `b`.
pub fn xor(a: &Path, b: &Path) -> Path {
    combine(a, b, BoolOp::Xor)
}

/// Fillable outline of `path` stroked `width` wide (butt caps, miter joins limit 4).
/// Returns an empty path if the native stroker rejects the input.
pub fn stroke_outline(path: &Path, width: f32) -> Path {
    from_path_data(&outline_stroke(&to_path_data(path), f64::from(width), Cap::Butt, Join::Miter, 4.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ac_core::geom::Path;

    fn area(p: &Path) -> f64 {
        use kurbo::Shape;
        to_bezpath(p).area().abs()
    }

    fn sq(x: f32, y: f32, s: f32) -> Path {
        Path::rect(x, y, s, s)
    }

    #[test]
    fn roundtrip_preserves_geometry() {
        let p = Path::ellipse(50.0, 50.0, 30.0, 20.0);
        let back = from_path_data(&to_path_data(&p));
        assert!((area(&p) - area(&back)).abs() < 1e-3);
    }

    #[test]
    fn rect_ops_areas() {
        let (a, b) = (sq(0.0, 0.0, 10.0), sq(5.0, 5.0, 10.0));
        assert!((area(&union(&a, &b)) - 175.0).abs() < 1e-3);
        assert!((area(&intersect(&a, &b)) - 25.0).abs() < 1e-3);
        assert!((area(&subtract(&a, &b)) - 75.0).abs() < 1e-3);
        assert!((area(&xor(&a, &b)) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn disjoint_intersection_is_empty() {
        let r = intersect(&sq(0.0, 0.0, 5.0), &sq(20.0, 20.0, 5.0));
        assert!(area(&r) < 1e-6);
    }

    #[test]
    fn curved_subtract_makes_a_hole() {
        let r = subtract(&Path::rect(0.0, 0.0, 100.0, 100.0), &Path::ellipse(50.0, 50.0, 20.0, 20.0));
        let sub = r.segs.iter().filter(|s| matches!(s, Seg::M(_))).count();
        assert_eq!(sub, 2, "outer ring plus hole");
        let circle = std::f64::consts::PI * 400.0;
        // signed areas of the two rings cancel under NonZero; compare the filled area instead
        let filled = to_path_data(&r).to_bezpath();
        use kurbo::Shape;
        let net: f64 = filled.area().abs();
        assert!((net - (10_000.0 - circle)).abs() < 5.0, "net {net}");
    }

    #[test]
    fn nan_input_gives_empty() {
        let bad = Path { segs: vec![Seg::M(pt(f32::NAN, 0.0)), Seg::L(pt(1.0, 1.0)), Seg::L(pt(0.0, 1.0)), Seg::Z] };
        assert!(union(&bad, &sq(0.0, 0.0, 1.0)).segs.is_empty());
    }

    #[test]
    fn stroke_outline_has_area() {
        let line = Path { segs: vec![Seg::M(pt(0.0, 0.0)), Seg::L(pt(100.0, 0.0))] };
        let o = stroke_outline(&line, 10.0);
        assert!((area(&o) - 1000.0).abs() < 1.0, "{}", area(&o));
    }
}
