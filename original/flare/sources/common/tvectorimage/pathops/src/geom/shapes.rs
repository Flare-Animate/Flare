//! The two primitive shapes the path-ops sources and tests need (inlined from
//! `vectorcraft-geom::shapes`; the rest of that module is not used here).

use kurbo::{Point, Rect, Vec2};

use super::path::{Anchor, AnchorKind, PathData, SubPath};

/// Circle-approximation handle ratio for a quarter arc.
pub const KAPPA: f64 = 0.552_284_749_830_793_6;

/// Rectangle with 4 corner anchors, clockwise from the top-left.
pub fn rectangle(r: Rect) -> PathData {
    let r = r.abs();
    PathData::single(SubPath::polyline(&[Point::new(r.x0, r.y0), Point::new(r.x1, r.y0), Point::new(r.x1, r.y1), Point::new(r.x0, r.y1)], true))
}


/// Ellipse inscribed in `r` with 4 smooth anchors (left, top, right, bottom).
pub fn ellipse(r: Rect) -> PathData {
    let r = r.abs();
    let c = r.center();
    let rx = r.width() / 2.0;
    let ry = r.height() / 2.0;
    let kx = rx * KAPPA;
    let ky = ry * KAPPA;
    let s = |p: Point, hin: Vec2| Anchor { p, h_in: p + hin, h_out: p - hin, kind: AnchorKind::Smooth };
    let anchors = vec![
        s(Point::new(c.x - rx, c.y), Vec2::new(0.0, ky)),
        s(Point::new(c.x, c.y - ry), Vec2::new(-kx, 0.0)),
        s(Point::new(c.x + rx, c.y), Vec2::new(0.0, -ky)),
        s(Point::new(c.x, c.y + ry), Vec2::new(kx, 0.0)),
    ];
    PathData::single(SubPath::new(anchors, true))
}

/// Open straight line.
pub fn line(a: Point, b: Point) -> PathData {
    PathData::single(SubPath::polyline(&[a, b], false))
}

