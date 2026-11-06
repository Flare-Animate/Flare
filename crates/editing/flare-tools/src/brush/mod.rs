//! A brush stroke as a filled outline whose width follows the pressure of the pen.
//!
//! The centre line is simplified, smoothed with a Catmull-Rom spline, and offset to both sides by
//! half the width at each point; the ends are round. A mouse reports full pressure everywhere, so
//! it gives a stroke of one width.

use flare_core::DocumentPoint;
use flare_document::{MAX_SEGMENTS, PathSegment};
use flare_input::{InputSample, PressureCurve};
use glamx::DVec2;

mod nib;
mod shelf;
pub use nib::{Nib, NibShape};
pub use shelf::{
    BrushPreset, BrushProfileFile, BrushShelf, BuiltinBrushProfile, MAX_BRUSH_PROFILE_BYTES,
    MAX_BRUSHES, ShelfFull,
};

/// Points added between two kept points when smoothing.
const SUBDIVISIONS: usize = 4;
/// Points in each round end.
const CAP_STEPS: usize = 8;

/// A point of the centre line and how far the stroke reaches to each side of it.
#[derive(Clone, Copy)]
struct Spine {
    at: DVec2,
    half: f64,
}

/// Indices of the points that keep the polyline within `tolerance` of the original.
fn simplify(points: &[DVec2], tolerance: f64) -> Vec<usize> {
    if points.len() < 3 {
        return (0..points.len()).collect();
    }
    let mut keep = vec![false; points.len()];
    keep[0] = true;
    keep[points.len() - 1] = true;
    let mut stack = vec![(0, points.len() - 1)];
    while let Some((from, to)) = stack.pop() {
        let (a, b) = (points[from], points[to]);
        let ab = b - a;
        let length_sq = ab.length_squared();
        let far = (from + 1..to)
            .map(|i| {
                let t = if length_sq > f64::EPSILON {
                    ((points[i] - a).dot(ab) / length_sq).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                (i, (points[i] - (a + ab * t)).length())
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

/// Points along the Catmull-Rom spline through `spine`, with the widths blended between them.
fn smooth(spine: &[Spine], steps: usize) -> Vec<Spine> {
    let mut out = vec![spine[0]];
    for i in 0..spine.len() - 1 {
        let p0 = spine[i.saturating_sub(1)].at;
        let (p1, p2) = (spine[i].at, spine[i + 1].at);
        let p3 = spine[(i + 2).min(spine.len() - 1)].at;
        for s in 1..=steps {
            let t = s as f64 / steps as f64;
            let (t2, t3) = (t * t, t * t * t);
            let at = 0.5
                * ((2.0 * p1)
                    + (p2 - p0) * t
                    + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
                    + (3.0 * p1 - p0 - 3.0 * p2 + p3) * t3);
            let half = spine[i].half + (spine[i + 1].half - spine[i].half) * t;
            out.push(Spine { at, half });
        }
    }
    out
}

/// The closed outline of a round brush stroke through `samples` at `width`, or `None` for a click
/// (no movement), a non-positive width or an outline that would not be a valid path.
pub fn outline(
    samples: &[InputSample],
    width: f64,
    curve: PressureCurve,
    tolerance: f64,
) -> Option<Vec<PathSegment>> {
    outline_with(samples, width, curve, tolerance, Nib::ROUND)
}

/// Like [`outline`], painted with `nib`. The sides follow the farthest point of the nib in each
/// direction of travel and the ends are the nib's own half outline, so a flat nib draws thick
/// where the line crosses it and thin where it runs along it. A square nib turns in steps at its
/// corners, which the straight joins between points smooth over.
pub fn outline_with(
    samples: &[InputSample],
    width: f64,
    curve: PressureCurve,
    tolerance: f64,
    nib: Nib,
) -> Option<Vec<PathSegment>> {
    if !(width.is_finite() && width > 0.0) {
        return None;
    }
    let mut raw: Vec<(DVec2, f32)> = Vec::with_capacity(samples.len());
    for s in samples {
        let at = DVec2::new(s.x, s.y);
        if raw.last().is_none_or(|(last, _)| *last != at) {
            raw.push((at, s.pressure));
        }
    }
    if raw.len() < 2 {
        return None;
    }
    let positions: Vec<DVec2> = raw.iter().map(|(p, _)| *p).collect();
    let spine: Vec<Spine> = simplify(&positions, tolerance.max(0.0))
        .into_iter()
        .map(|i| Spine {
            at: raw[i].0,
            half: width / 2.0 * f64::from(curve.width_factor(raw[i].1)),
        })
        .collect();
    // Keep the outline well inside the segment limit however long the drag was.
    let steps = if spine.len() * SUBDIVISIONS * 2 + 2 * CAP_STEPS + 3 < MAX_SEGMENTS {
        SUBDIVISIONS
    } else {
        1
    };
    let line = smooth(&spine, steps);
    let normals = normals(&line);
    let left = |i: usize| line[i].at + nib.support(normals[i]) * line[i].half;
    let right = |i: usize| line[i].at - nib.support(normals[i]) * line[i].half;
    let end = line.len() - 1;
    let mut points: Vec<DVec2> = (0..=end).map(left).collect();
    points.extend(cap(line[end].at, normals[end], line[end].half, nib));
    points.extend((0..=end).rev().map(right));
    points.extend(cap(line[0].at, -normals[0], line[0].half, nib));
    let point = |p: DVec2| DocumentPoint::new(p.x, p.y).ok();
    let mut path = vec![PathSegment::MoveTo(point(points[0])?)];
    for p in &points[1..] {
        path.push(PathSegment::LineTo(point(*p)?));
    }
    path.push(PathSegment::Close);
    Some(path)
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
mod tests;
