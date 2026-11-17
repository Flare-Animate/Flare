//! A pull on one outline: the stretch of it near the pull gets points enough to bend smoothly,
//! then every point moves by the pull, by less the farther it is from where the pull started.

use flare_document::{MAX_SEGMENTS, PathSegment, Shape};
use glamx::DVec2;

use super::Pull;
use crate::common::path_edit::{point, pos};

/// The most pieces one segment is split into.
const MAX_PIECES: usize = 64;
/// Points along a curve when measuring how near it comes, and how long it is.
const CURVE_SAMPLES: usize = 16;

/// How much of a pull a point `distance` from its start takes: all of it at the start, none at
/// the edge of the circle and beyond, easing in and out between so no corner shows at the edge.
pub fn falloff(distance: f64, radius: f64) -> f64 {
    if !(distance.is_finite() && radius.is_finite() && radius > 0.0) || distance >= radius {
        return 0.0;
    }
    let t = distance / radius;
    let k = 1.0 - t * t;
    k * k
}

fn segment_distance(p: DVec2, a: DVec2, b: DVec2) -> f64 {
    let ab = b - a;
    let length_sq = ab.length_squared();
    let t = if length_sq > f64::EPSILON {
        ((p - a).dot(ab) / length_sq).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p - (a + ab * t)).length()
}

fn cubic_at(p0: DVec2, c1: DVec2, c2: DVec2, p3: DVec2, t: f64) -> DVec2 {
    let u = 1.0 - t;
    p0 * (u * u * u) + c1 * (3.0 * u * u * t) + c2 * (3.0 * u * t * t) + p3 * (t * t * t)
}

/// Points along a cubic, both ends included.
fn cubic_samples(p0: DVec2, c1: DVec2, c2: DVec2, p3: DVec2) -> Vec<DVec2> {
    (0..=CURVE_SAMPLES)
        .map(|i| cubic_at(p0, c1, c2, p3, i as f64 / CURVE_SAMPLES as f64))
        .collect()
}

/// How near a run of straight pieces comes to `p`, and how long it is.
fn polyline_reach(p: DVec2, points: &[DVec2]) -> (f64, f64) {
    points
        .windows(2)
        .fold((f64::INFINITY, 0.0), |(near, long), w| {
            (
                near.min(segment_distance(p, w[0], w[1])),
                long + (w[1] - w[0]).length(),
            )
        })
}

/// Into how many pieces a segment `length` long that comes `near` the pull is split, so that the
/// pull has a point every quarter of its radius to bend.
fn pieces(near: f64, length: f64, pull: &Pull) -> usize {
    if near >= pull.radius {
        return 1;
    }
    let step = pull.radius / 4.0;
    ((length / step).ceil() as usize).clamp(1, MAX_PIECES)
}

/// The cubic from `p0` split at `t0..t1` (de Casteljau), as its two control points and its end.
fn cubic_piece(p: [DVec2; 4], t0: f64, t1: f64) -> [DVec2; 3] {
    let [p0, c1, c2, p3] = p;
    let derivative = |t: f64| {
        let u = 1.0 - t;
        (c1 - p0) * (3.0 * u * u) + (c2 - c1) * (6.0 * u * t) + (p3 - c2) * (3.0 * t * t)
    };
    let (a, b) = (cubic_at(p0, c1, c2, p3, t0), cubic_at(p0, c1, c2, p3, t1));
    let span = (t1 - t0) / 3.0;
    [a + derivative(t0) * span, b - derivative(t1) * span, b]
}

/// `Some` when `more` segments still fit in `out` under the segment limit. Checked before a
/// segment's pieces are added, so the path being built never grows past the limit.
pub(super) fn room(out: &[PathSegment], more: usize) -> Option<()> {
    (out.len().saturating_add(more) <= MAX_SEGMENTS).then_some(())
}

/// `path` with the segments near the pull split into pieces, or `None` when that would pass
/// the segment limit (the pull then bends only the points there are).
pub(super) fn densified(path: &[PathSegment], pull: &Pull) -> Option<Vec<PathSegment>> {
    let mut out: Vec<PathSegment> = Vec::with_capacity(path.len());
    let (mut at, mut start) = (DVec2::ZERO, DVec2::ZERO);
    let line = |out: &mut Vec<PathSegment>, from: DVec2, to: DVec2| -> Option<()> {
        let (near, long) = polyline_reach(pull.from, &[from, to]);
        let n = pieces(near, long, pull);
        room(out, n)?;
        for i in 1..=n {
            out.push(PathSegment::LineTo(point(
                from + (to - from) * (i as f64 / n as f64),
            )?));
            if out.len() > MAX_SEGMENTS {
                return None;
            }
        }
        Some(())
    };
    for segment in path {
        match *segment {
            PathSegment::MoveTo(p) => {
                room(&out, 1)?;
                (at, start) = (pos(p), pos(p));
                out.push(*segment);
            }
            PathSegment::LineTo(p) => {
                line(&mut out, at, pos(p))?;
                at = pos(p);
            }
            PathSegment::CubicTo(c1, c2, p) => {
                let curve = [at, pos(c1), pos(c2), pos(p)];
                let (near, long) =
                    polyline_reach(pull.from, &cubic_samples(at, curve[1], curve[2], curve[3]));
                let n = pieces(near, long, pull);
                room(&out, n)?;
                for i in 0..n {
                    let (t0, t1) = (i as f64 / n as f64, (i + 1) as f64 / n as f64);
                    let [a, b, end] = cubic_piece(curve, t0, t1);
                    out.push(PathSegment::CubicTo(point(a)?, point(b)?, point(end)?));
                }
                at = pos(p);
            }
            PathSegment::Close => {
                // The closing edge is drawn back to the start: split it too, short of the start.
                let (near, long) = polyline_reach(pull.from, &[at, start]);
                let n = pieces(near, long, pull);
                // `n - 1` lines and the close itself.
                room(&out, n)?;
                for i in 1..n {
                    out.push(PathSegment::LineTo(point(
                        at + (start - at) * (i as f64 / n as f64),
                    )?));
                }
                out.push(PathSegment::Close);
                at = start;
            }
        }
    }
    Some(out)
}

/// `shape` pulled, keeping its paint; `None` when no point of it is near enough to move.
pub fn pull_shape(shape: &Shape, pull: &Pull) -> Option<Shape> {
    let path = densified(shape.path(), pull).unwrap_or_else(|| shape.path().to_vec());
    let offset = pull.to - pull.from;
    let mut moved = false;
    let mut shift = |p: flare_core::DocumentPoint| {
        let v = pos(p);
        let share = falloff((v - pull.from).length(), pull.radius);
        if share > 0.0 {
            moved = true;
        }
        point(v + offset * share)
    };
    let mut out = Vec::with_capacity(path.len());
    for segment in path {
        out.push(match segment {
            PathSegment::MoveTo(p) => PathSegment::MoveTo(shift(p)?),
            PathSegment::LineTo(p) => PathSegment::LineTo(shift(p)?),
            PathSegment::CubicTo(c1, c2, p) => {
                PathSegment::CubicTo(shift(c1)?, shift(c2)?, shift(p)?)
            }
            PathSegment::Close => PathSegment::Close,
        });
    }
    if !moved {
        return None;
    }
    Shape::new(shape.id(), out, shape.fill())
        .ok()
        .map(|s| s.painted_like(shape))
}

/// Whether any of `shape`'s outline comes within the pull's circle.
pub fn reaches(shape: &Shape, pull: &Pull) -> bool {
    let (mut at, mut start) = (DVec2::ZERO, DVec2::ZERO);
    for segment in shape.path() {
        let near = match *segment {
            PathSegment::MoveTo(p) => {
                (at, start) = (pos(p), pos(p));
                (at - pull.from).length()
            }
            PathSegment::LineTo(p) => {
                let near = segment_distance(pull.from, at, pos(p));
                at = pos(p);
                near
            }
            PathSegment::CubicTo(c1, c2, p) => {
                let samples = cubic_samples(at, pos(c1), pos(c2), pos(p));
                at = pos(p);
                polyline_reach(pull.from, &samples).0
            }
            PathSegment::Close => {
                let near = segment_distance(pull.from, at, start);
                at = start;
                near
            }
        };
        if near < pull.radius {
            return true;
        }
    }
    false
}
