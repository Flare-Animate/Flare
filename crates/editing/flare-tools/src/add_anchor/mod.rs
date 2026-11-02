//! The Add Anchor Point tool: a click on an outline puts a point there.

use flare_commands::Command;
use flare_core::SceneId;
use flare_document::{Document, PathSegment};
use glamx::DVec2;

use crate::common::SpanRef;
use crate::common::path_edit::*;

/// How many steps along a curve are tried when looking for the point nearest a click.
const CURVE_STEPS: usize = 64;

/// A straight or cubic piece of an outline, for finding the point nearest a click.
#[derive(Clone, Copy)]
enum Piece {
    Line(DVec2, DVec2),
    Cubic(DVec2, DVec2, DVec2, DVec2),
}

impl Piece {
    fn at(self, t: f64) -> DVec2 {
        match self {
            Self::Line(a, b) => a.lerp(b, t),
            Self::Cubic(a, c1, c2, b) => {
                let (p01, p12, p23) = (a.lerp(c1, t), c1.lerp(c2, t), c2.lerp(b, t));
                p01.lerp(p12, t).lerp(p12.lerp(p23, t), t)
            }
        }
    }

    /// The parameter of the point nearest `near`, and how far that point is from it.
    fn nearest(self, near: DVec2) -> (f64, f64) {
        match self {
            Self::Line(a, b) => {
                let along = b - a;
                let length = along.length_squared();
                let t = if length <= SAME {
                    0.0
                } else {
                    ((near - a).dot(along) / length).clamp(0.0, 1.0)
                };
                (t, (a.lerp(b, t) - near).length())
            }
            Self::Cubic(..) => {
                let step = 1.0 / CURVE_STEPS as f64;
                let distance = |t: f64| (self.at(t) - near).length();
                let mut best = (0..=CURVE_STEPS)
                    .map(|i| i as f64 * step)
                    .min_by(|a, b| distance(*a).total_cmp(&distance(*b)))
                    .unwrap_or(0.0);
                // A few halvings around the best sample.
                let mut width = step;
                for _ in 0..12 {
                    let (lo, hi) = ((best - width).max(0.0), (best + width).min(1.0));
                    best = [lo, best, hi]
                        .into_iter()
                        .min_by(|a, b| distance(*a).total_cmp(&distance(*b)))
                        .unwrap_or(best);
                    width /= 2.0;
                }
                (best, distance(best))
            }
        }
    }
}

/// Adds an anchor on the outline at the point nearest `near`, if one is within `tolerance`:
/// the segment there splits in two and a curve keeps exactly its shape. `None` when the span is
/// not a shape, nothing is that close, or the nearest point is an anchor already.
pub fn add_anchor(
    document: &Document,
    scene: SceneId,
    at: SpanRef,
    near: DVec2,
    tolerance: f64,
) -> Option<Vec<Command>> {
    let (_, _, mut path) = shape_path(document, scene, at)?;
    let mut best: Option<(f64, usize, f64, Piece)> = None;
    for (i, segment) in path.iter().enumerate() {
        let piece = match *segment {
            PathSegment::MoveTo(_) => continue,
            PathSegment::LineTo(b) => Piece::Line(start_of(&path, i)?, pos(b)),
            PathSegment::CubicTo(c1, c2, b) => {
                Piece::Cubic(start_of(&path, i)?, pos(c1), pos(c2), pos(b))
            }
            // The edge a Close draws back to where the subpath began.
            PathSegment::Close => {
                let first = end_of(path[subpath_start(&path, i)?])?;
                Piece::Line(start_of(&path, i)?, first)
            }
        };
        let (t, distance) = piece.nearest(near);
        if distance <= tolerance && best.is_none_or(|b| distance < b.0) {
            best = Some((distance, i, t, piece));
        }
    }
    let (_, index, t, piece) = best?;
    if !(MIDDLE..=1.0 - MIDDLE).contains(&t) {
        return None;
    }
    let at_t = point(piece.at(t))?;
    match (piece, path[index]) {
        (Piece::Line(..), PathSegment::Close) => path.insert(index, PathSegment::LineTo(at_t)),
        (Piece::Line(_, b), _) => {
            path[index] = PathSegment::LineTo(at_t);
            path.insert(index + 1, PathSegment::LineTo(point(b)?));
        }
        (Piece::Cubic(a, c1, c2, b), _) => {
            // De Casteljau: the two halves are the same curve.
            let (p01, p12, p23) = (a.lerp(c1, t), c1.lerp(c2, t), c2.lerp(b, t));
            let (p012, p123) = (p01.lerp(p12, t), p12.lerp(p23, t));
            path[index] = PathSegment::CubicTo(point(p01)?, point(p012)?, at_t);
            path.insert(
                index + 1,
                PathSegment::CubicTo(point(p123)?, point(p23)?, point(b)?),
            );
        }
    }
    replace(document, scene, at, path)
}

#[cfg(test)]
mod tests;
