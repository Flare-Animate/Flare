//! The geometry of reshaping one edge: finding the edge nearest a point, and turning it into a
//! curve that passes through where it was pulled to.

use flare_document::{MAX_SEGMENTS, PathSegment, Shape};
use glamx::DVec2;

use crate::common::path_edit::{point, pos};

/// Points sampled along a curve to find how near it comes.
const SAMPLES: usize = 32;
/// How near either end of an edge a grab may be taken, as a share of its length: the corner
/// itself moves with the Subselection tool, and a grab pinned at an end would not bend anything.
const END: f64 = 0.05;
/// A pull that moves nothing is not an edit.
const TINY: f64 = 1e-9;

fn cubic_at(a: DVec2, c1: DVec2, c2: DVec2, b: DVec2, t: f64) -> DVec2 {
    let u = 1.0 - t;
    a * (u * u * u) + c1 * (3.0 * u * u * t) + c2 * (3.0 * u * t * t) + b * (t * t * t)
}

/// One edge: where its segment is in the path and the cubic it draws.
struct Edge {
    segment: usize,
    /// Start, first and second control, end. A straight edge has controls a third along.
    curve: [DVec2; 4],
    straight: bool,
    /// A closing edge gets a segment of its own when bent, ending at the subpath's start.
    closing: bool,
}

fn edges(path: &[PathSegment]) -> Vec<Edge> {
    let (mut at, mut start) = (DVec2::ZERO, DVec2::ZERO);
    let mut out = Vec::new();
    for (segment, piece) in path.iter().enumerate() {
        match *piece {
            PathSegment::MoveTo(p) => (at, start) = (pos(p), pos(p)),
            PathSegment::LineTo(p) => {
                let end = pos(p);
                out.push(line_edge(segment, at, end, false));
                at = end;
            }
            PathSegment::CubicTo(c1, c2, p) => {
                let end = pos(p);
                out.push(Edge {
                    segment,
                    curve: [at, pos(c1), pos(c2), end],
                    straight: false,
                    closing: false,
                });
                at = end;
            }
            PathSegment::Close => {
                out.push(line_edge(segment, at, start, true));
                at = start;
            }
        }
    }
    out
}

fn line_edge(segment: usize, a: DVec2, b: DVec2, closing: bool) -> Edge {
    Edge {
        segment,
        curve: [a, a + (b - a) / 3.0, b - (b - a) / 3.0, b],
        straight: true,
        closing,
    }
}

/// How near `p` comes to the edge, and where: the distance, the edge's parameter and the point.
fn nearest_on(edge: &Edge, p: DVec2) -> (f64, f64, DVec2) {
    let [a, c1, c2, b] = edge.curve;
    let mut best = (f64::INFINITY, 0.0, a);
    for i in 0..=SAMPLES {
        let t = i as f64 / SAMPLES as f64;
        let at = cubic_at(a, c1, c2, b, t);
        let d = (at - p).length();
        if d < best.0 {
            best = (d, t, at);
        }
    }
    if edge.straight {
        // A straight edge is measured exactly, not by samples.
        let ab = b - a;
        let len_sq = ab.length_squared();
        if len_sq > f64::EPSILON {
            let t = ((p - a).dot(ab) / len_sq).clamp(0.0, 1.0);
            let at = a + ab * t;
            return ((at - p).length(), t, at);
        }
    }
    best
}

/// The edge of `path` nearest `p`: its distance, the segment, the parameter and the point on it.
/// Edges too short to bend are passed over.
pub(super) fn nearest_edge(path: &[PathSegment], p: DVec2) -> Option<(f64, usize, f64, DVec2)> {
    edges(path)
        .iter()
        .filter(|e| (e.curve[3] - e.curve[0]).length() > TINY || !e.straight)
        .map(|e| {
            let (d, t, at) = nearest_on(e, p);
            (d, e.segment, t.clamp(END, 1.0 - END), at)
        })
        .min_by(|x, y| x.0.total_cmp(&y.0))
}

/// `shape` with the edge at `segment`, grabbed `t` of the way along it, moved by `offset`: the
/// edge becomes one cubic from the same start to the same end, shifted so that the grabbed
/// point lands `offset` away, and the rest of the outline is untouched. The shape keeps its
/// identity and paint. `None` when the segment is not an edge, the pull moves nothing, or the
/// result would not be a valid shape.
pub fn bend_shape(shape: &Shape, segment: usize, t: f64, offset: DVec2) -> Option<Shape> {
    if !(offset.is_finite() && t.is_finite()) || offset.length() <= TINY {
        return None;
    }
    let edge = edges(shape.path())
        .into_iter()
        .find(|e| e.segment == segment)?;
    let t = t.clamp(END, 1.0 - END);
    let [_, c1, c2, b] = edge.curve;
    // The cubic's two controls share the pull in the weight that puts the curve's point at `t`
    // exactly `offset` away (the weights of the two controls at `t` sum, with this share, to 1).
    let share = offset / (3.0 * (1.0 - t) * t);
    let (n1, n2) = (c1 + share, c2 + share);
    let cubic = PathSegment::CubicTo(point(n1)?, point(n2)?, point(b)?);
    let mut path = shape.path().to_vec();
    if edge.closing {
        // The closing edge had no segment of its own; the curve goes in front of the `Close`.
        if path.len() >= MAX_SEGMENTS {
            return None;
        }
        path.insert(segment, cubic);
    } else {
        path[segment] = cubic;
    }
    Shape::new(shape.id(), path, shape.fill())
        .ok()
        .map(|s| s.painted_like(shape))
}
