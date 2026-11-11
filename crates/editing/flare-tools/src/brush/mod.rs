//! A brush stroke as a filled outline whose width follows the pressure of the pen.
//!
//! The centre line is simplified, smoothed with a centripetal Catmull-Rom spline, and offset to both sides by
//! half the width at each point; the ends are round. A mouse reports full pressure everywhere, so
//! it gives a stroke of one width unless the brush tapers its ends or answers speed. The widths
//! come from [`BrushStroke::width_factors`], which the live preview uses too.

use flare_core::DocumentPoint;
use flare_document::{MAX_SEGMENTS, PathSegment};
use flare_input::{InputSample, PressureCurve};
use glamx::DVec2;

mod nib;
mod preset;
mod shelf;
mod stroke;
pub use nib::{Nib, NibShape};
pub use preset::{
    BrushPreset, BrushProfileFile, BuiltinBrushProfile, MAX_BRUSH_PROFILE_BYTES, MAX_BRUSHES,
};
pub use shelf::{BrushShelf, ShelfFull};
pub use stroke::{BrushStroke, MAX_TAPER, Taper};

/// Points added between two kept points when smoothing.
const SUBDIVISIONS: usize = 4;
/// Points in each round end.
const CAP_STEPS: usize = 8;
/// The step between two samples, in pixels, that counts as drawing "at speed". Samples arrive one
/// frame apart, so half of the step from the sample before to the one after a point is that
/// point's speed. Surfaces preview the stroke with the same reach, so it is public.
pub const SPEED_REACH: f64 = 24.0;

/// A point of the centre line and how far the stroke reaches to each side of it.
#[derive(Clone, Copy)]
struct Spine {
    at: DVec2,
    half: f64,
}

/// Indices of the points that keep both sides of the stroke within `tolerance` of the original:
/// a point is dropped only when it lies near the line between the points kept around it *and*
/// its width is near the width blended between theirs. Judged by position alone, a straight
/// stroke would keep only its two ends, and every change of width along it (a press, a taper)
/// would be lost.
fn simplify(points: &[Spine], tolerance: f64) -> Vec<usize> {
    if points.len() < 3 {
        return (0..points.len()).collect();
    }
    let mut keep = vec![false; points.len()];
    keep[0] = true;
    keep[points.len() - 1] = true;
    let mut stack = vec![(0, points.len() - 1)];
    while let Some((from, to)) = stack.pop() {
        let (a, b) = (points[from], points[to]);
        let ab = b.at - a.at;
        let length_sq = ab.length_squared();
        let far = (from + 1..to)
            .map(|i| {
                let p = points[i];
                let t = if length_sq > f64::EPSILON {
                    ((p.at - a.at).dot(ab) / length_sq).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                // An edge is the centre plus the half width, so it strays by both at once.
                let off_line = (p.at - (a.at + ab * t)).length();
                let off_width = (p.half - (a.half + (b.half - a.half) * t)).abs();
                (i, off_line + off_width)
            })
            .max_by(|x, y| x.1.total_cmp(&y.1));
        if let Some((index, away)) = far
            && away > tolerance
        {
            keep[index] = true;
            stack.push((from, index));
            stack.push((index, to));
        }
    }
    (0..points.len()).filter(|i| keep[*i]).collect()
}

/// Points along the centripetal Catmull-Rom spline through `spine`, with the widths blended
/// between them.
///
/// The centripetal form spaces its knots by the square root of the distance between points.
/// Hand-drawn points are rarely evenly spaced (a stabilizer, for one, starts a stroke with tiny
/// steps), and the uniform form overshoots where a short step meets a long one, looping back
/// past the start of a line; the centripetal form never makes a cusp or a loop inside a segment.
fn smooth(spine: &[Spine], steps: usize) -> Vec<Spine> {
    let last = spine.len() - 1;
    let mut out = vec![spine[0]];
    for i in 0..last {
        let (p1, p2) = (spine[i].at, spine[i + 1].at);
        // Past the ends, the line runs on as it was heading.
        let p0 = if i == 0 {
            p1 * 2.0 - p2
        } else {
            spine[i - 1].at
        };
        let p3 = if i + 1 == last {
            p2 * 2.0 - p1
        } else {
            spine[i + 2].at
        };
        for s in 1..=steps {
            let t = s as f64 / steps as f64;
            let at = centripetal(p0, p1, p2, p3, t);
            let half = spine[i].half + (spine[i + 1].half - spine[i].half) * t;
            out.push(Spine { at, half });
        }
    }
    out
}

/// The point `t` (0 to 1) of the way from `p1` to `p2` on the centripetal Catmull-Rom spline
/// through the four points (Barry and Goldman's pyramid).
fn centripetal(p0: DVec2, p1: DVec2, p2: DVec2, p3: DVec2, t: f64) -> DVec2 {
    let knot = |a: DVec2, b: DVec2| (b - a).length().sqrt().max(1e-9);
    let t1 = knot(p0, p1);
    let t2 = t1 + knot(p1, p2);
    let t3 = t2 + knot(p2, p3);
    let u = t1 + (t2 - t1) * t;
    let lerp = |a: DVec2, b: DVec2, from: f64, to: f64| {
        a * ((to - u) / (to - from)) + b * ((u - from) / (to - from))
    };
    let a1 = lerp(p0, p1, 0.0, t1);
    let a2 = lerp(p1, p2, t1, t2);
    let a3 = lerp(p2, p3, t2, t3);
    let b1 = lerp(a1, a2, 0.0, t2);
    let b2 = lerp(a2, a3, t1, t3);
    lerp(b1, b2, t1, t2)
}

/// The closed outline of a round brush stroke through `samples` at `width`, or `None` for a click
/// (no movement), a non-positive width or an outline that would not be a valid path.
pub fn outline(
    samples: &[InputSample],
    width: f64,
    curve: PressureCurve,
    tolerance: f64,
) -> Option<Vec<PathSegment>> {
    BrushStroke::new(width, curve).outline(samples, tolerance)
}

/// Like [`outline`], painted with `nib`, with the width answering speed as far as `velocity`
/// asks (see [`BrushStroke::velocity`]).
pub fn outline_with(
    samples: &[InputSample],
    width: f64,
    curve: PressureCurve,
    tolerance: f64,
    nib: Nib,
    velocity: f32,
) -> Option<Vec<PathSegment>> {
    BrushStroke {
        nib,
        velocity,
        ..BrushStroke::new(width, curve)
    }
    .outline(samples, tolerance)
}

/// The two sides and the two ends of a stroke's outline, before they are joined into a path:
/// `left[i]` and `right[i]` lie across the line from each other at `centre[i]`, and each cap runs
/// round its end from one side to the other (without the side points themselves). A live
/// preview draws these as a strip of triangles, so it shows the exact shape that is kept.
#[derive(Clone, Debug, PartialEq)]
pub struct StrokeSides {
    pub centre: Vec<DVec2>,
    pub left: Vec<DVec2>,
    pub right: Vec<DVec2>,
    /// From `left[last]` round to `right[last]`.
    pub end_cap: Vec<DVec2>,
    /// From `right[0]` round to `left[0]`.
    pub start_cap: Vec<DVec2>,
}

impl BrushStroke {
    /// The closed outline of the stroke through `samples`, its centre line simplified to within
    /// `tolerance`, or `None` for a click (no movement), a non-positive width or an outline that
    /// would not be a valid path. It is [`BrushStroke::sides`] joined up.
    pub fn outline(&self, samples: &[InputSample], tolerance: f64) -> Option<Vec<PathSegment>> {
        let sides = self.sides(samples, tolerance)?;
        let point = |p: &DVec2| DocumentPoint::new(p.x, p.y).ok();
        let mut points = sides.left.iter();
        let mut path = vec![PathSegment::MoveTo(point(points.next()?)?)];
        for p in points
            .chain(&sides.end_cap)
            .chain(sides.right.iter().rev())
            .chain(&sides.start_cap)
        {
            path.push(PathSegment::LineTo(point(p)?));
        }
        path.push(PathSegment::Close);
        Some(path)
    }

    /// The sides and ends of the stroke through `samples` (see [`BrushStroke::outline`]).
    ///
    /// The sides follow the farthest point of the nib in each direction of travel and the ends
    /// are the nib's own half outline, so a flat nib draws thick where the line crosses it and
    /// thin where it runs along it. A square nib turns in steps at its corners, which the
    /// straight joins between points smooth over.
    pub fn sides(&self, samples: &[InputSample], tolerance: f64) -> Option<StrokeSides> {
        let width = self.width;
        if !(width.is_finite() && width > 0.0) {
            return None;
        }
        let raw = Self::spine(samples);
        if raw.len() < 2 {
            return None;
        }
        let full: Vec<Spine> = raw
            .iter()
            .zip(self.width_factors(&raw))
            .map(|(s, factor)| Spine {
                at: DVec2::new(s.x, s.y),
                half: width / 2.0 * factor,
            })
            .collect();
        let spine: Vec<Spine> = simplify(&full, tolerance.max(0.0))
            .into_iter()
            .map(|i| full[i])
            .collect();
        // Keep the outline well inside the segment limit however long the drag was.
        let steps = if spine.len() * SUBDIVISIONS * 2 + 2 * CAP_STEPS + 3 < MAX_SEGMENTS {
            SUBDIVISIONS
        } else {
            1
        };
        let nib = self.nib;
        let line = smooth(&spine, steps);
        let normals = normals(&line);
        let end = line.len() - 1;
        Some(StrokeSides {
            centre: line.iter().map(|s| s.at).collect(),
            left: (0..=end)
                .map(|i| line[i].at + nib.support(normals[i]) * line[i].half)
                .collect(),
            right: (0..=end)
                .map(|i| line[i].at - nib.support(normals[i]) * line[i].half)
                .collect(),
            end_cap: cap(line[end].at, normals[end], line[end].half, nib),
            start_cap: cap(line[0].at, -normals[0], line[0].half, nib),
        })
    }
}

/// How far along the line, as a fraction of the half-width at a point, its direction is taken
/// from. Hand-drawn points are a pixel or so apart and never quite in line, so a direction taken
/// from the two nearest points swings back and forth; offset by half the width, those swings make
/// the two sides cross themselves in slivers, most of all when nothing has been smoothed away.
const DIRECTION_REACH: f64 = 0.75;

/// The unit normal at each point of `line`, from the direction of the line a short way before
/// and after it ([`DIRECTION_REACH`] times the half-width, or the neighbours if that is nothing).
fn normals(line: &[Spine]) -> Vec<DVec2> {
    let mut along = Vec::with_capacity(line.len());
    let mut total = 0.0;
    for (i, point) in line.iter().enumerate() {
        if i > 0 {
            total += (point.at - line[i - 1].at).length();
        }
        along.push(total);
    }
    let last = line.len() - 1;
    (0..line.len())
        .map(|i| {
            let reach = line[i].half * DIRECTION_REACH;
            let ahead = along.partition_point(|d| *d < along[i] + reach).min(last);
            let behind = along
                .partition_point(|d| *d <= along[i] - reach)
                .saturating_sub(1);
            let mut direction = (line[ahead].at - line[behind].at).normalize_or_zero();
            if direction == DVec2::ZERO {
                let before = line[i].at - line[i.saturating_sub(1)].at;
                let after = line[(i + 1).min(last)].at - line[i].at;
                direction =
                    (before.normalize_or_zero() + after.normalize_or_zero()).normalize_or_zero();
            }
            DVec2::new(-direction.y, direction.x)
        })
        .collect()
}

/// The half circle around `centre` from the `normal` side to the opposite side, turning the way
/// the stroke is heading (without the two end points, which the sides already have).
fn cap(centre: DVec2, normal: DVec2, radius: f64, nib: Nib) -> Vec<DVec2> {
    let heading = DVec2::new(normal.y, -normal.x);
    (1..CAP_STEPS)
        .map(|k| {
            let angle = std::f64::consts::PI * k as f64 / CAP_STEPS as f64;
            centre + nib.support(normal * angle.cos() + heading * angle.sin()) * radius
        })
        .collect()
}

#[cfg(test)]
mod shelf_tests;
#[cfg(test)]
mod stroke_tests;
#[cfg(test)]
mod tests;
