//! Adding, deleting and converting the points of a shape's outline: the Add, Delete and Convert
//! Anchor Point tools.

use flare_commands::Command;
use flare_core::{DocumentPoint, SceneId};
use flare_document::{Content, Document, FrameSpan, PathSegment, Shape};
use glamx::DVec2;

use super::*;

/// How many steps along a curve are tried when looking for the point nearest a click.
const CURVE_STEPS: usize = 64;

/// Two points this close are the same point (a path that returns to where it began).
const SAME: f64 = 1e-9;

/// How far along a segment a split must be to count as in its middle and not at an anchor.
const MIDDLE: f64 = 1e-3;

fn pos(p: DocumentPoint) -> DVec2 {
    p.as_vec()
}

fn point(p: DVec2) -> Option<DocumentPoint> {
    DocumentPoint::new(p.x, p.y).ok()
}

fn end_of(segment: PathSegment) -> Option<DVec2> {
    match segment {
        PathSegment::MoveTo(p) | PathSegment::LineTo(p) | PathSegment::CubicTo(_, _, p) => {
            Some(pos(p))
        }
        PathSegment::Close => None,
    }
}

/// The start of the subpath that the segment at `index` belongs to.
fn subpath_start(path: &[PathSegment], index: usize) -> Option<usize> {
    (0..=index.min(path.len().checked_sub(1)?))
        .rev()
        .take_while(|&i| i == index || !matches!(path[i], PathSegment::Close))
        .find(|&i| matches!(path[i], PathSegment::MoveTo(_)))
}

/// Where the segment at `index` starts: the end of the one before it, or the start of the subpath
/// the one before it closed.
fn start_of(path: &[PathSegment], index: usize) -> Option<DVec2> {
    let before = index.checked_sub(1)?;
    match path[before] {
        PathSegment::Close => end_of(path[subpath_start(path, before)?]),
        segment => end_of(segment),
    }
}

/// The segments of the subpath holding `index`: its first (the `MoveTo`), one past its last
/// drawn segment, and whether a `Close` ends it.
fn subpath(path: &[PathSegment], index: usize) -> Option<(usize, usize, bool)> {
    let first = subpath_start(path, index)?;
    let stop = (first + 1..path.len())
        .find(|&i| matches!(path[i], PathSegment::Close | PathSegment::MoveTo(_)))
        .unwrap_or(path.len());
    Some((
        first,
        stop,
        matches!(path.get(stop), Some(PathSegment::Close)),
    ))
}

/// The shape of a span and a copy of its path to edit.
fn shape_path(
    document: &Document,
    scene: SceneId,
    at: SpanRef,
) -> Option<(&FrameSpan, &Shape, Vec<PathSegment>)> {
    let span = find_span(document, scene, at)?;
    let Content::Shape(shape) = span.content() else {
        return None;
    };
    Some((span, shape, shape.path().to_vec()))
}

/// The commands that give the span at `at` the outline `path`, keeping its paint and motion.
fn replace(
    document: &Document,
    scene: SceneId,
    at: SpanRef,
    path: Vec<PathSegment>,
) -> Option<Vec<Command>> {
    let (span, shape, _) = shape_path(document, scene, at)?;
    let edited = Shape::new(shape.id(), path, shape.fill())
        .ok()?
        .painted_like(shape);
    let replacement =
        FrameSpan::new(span.start(), span.duration(), Content::Shape(edited)).with_motion_of(span);
    Some(vec![
        Command::RemoveSpan {
            scene,
            layer: at.layer,
            start: at.start,
        },
        Command::InsertSpan {
            scene,
            layer: at.layer,
            span: replacement,
        },
    ])
}

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

/// One piece standing in for the two that meet at an anchor: a line for two lines, otherwise a
/// curve that leaves along the first and arrives along the second.
fn merged(a: DVec2, first: PathSegment, second: PathSegment) -> Option<PathSegment> {
    let (p, b) = (end_of(first)?, end_of(second)?);
    if matches!(
        (first, second),
        (PathSegment::LineTo(_), PathSegment::LineTo(_))
    ) {
        return Some(PathSegment::LineTo(point(b)?));
    }
    let c1 = match first {
        PathSegment::CubicTo(c1, _, _) => pos(c1),
        _ => a + (p - a) / 3.0,
    };
    let c2 = match second {
        PathSegment::CubicTo(_, c2, _) => pos(c2),
        _ => b + (p - b) / 3.0,
    };
    Some(PathSegment::CubicTo(point(c1)?, point(c2)?, point(b)?))
}

/// Deletes the anchor ending `segment`, joining the outline across the gap (two curves become
/// one that leaves and arrives the way they did). `None` when the span is not a shape, the
/// segment is not an anchor, or it would leave fewer than two points (three in a closed shape).
pub fn delete_anchor(
    document: &Document,
    scene: SceneId,
    at: SpanRef,
    segment: usize,
) -> Option<Vec<Command>> {
    let (_, _, mut path) = shape_path(document, scene, at)?;
    if matches!(path.get(segment)?, PathSegment::Close) {
        return None;
    }
    let (first, stop, closed) = subpath(&path, segment)?;
    if stop - first <= if closed { 3 } else { 2 } {
        return None;
    }
    if segment == first {
        let next = path[first + 1];
        let new_start = end_of(next)?;
        let last = stop - 1;
        // A path that comes back to its start by a segment of its own: that segment now ends
        // where the next one did.
        let start = end_of(path[first])?;
        if closed && end_of(path[last]).is_some_and(|e| (e - start).length() <= SAME) {
            path[last] = merged(start_of(&path, last)?, path[last], next)?;
        }
        path[first] = PathSegment::MoveTo(point(new_start)?);
        path.remove(first + 1);
    } else if segment + 1 < stop {
        path[segment] = merged(start_of(&path, segment)?, path[segment], path[segment + 1])?;
        path.remove(segment + 1);
    } else {
        path.remove(segment);
    }
    replace(document, scene, at, path)
}

/// Converts the anchor ending `segment`. With no `drag` a smooth point becomes a corner: its
/// handles are pulled in to the point. With one (the pointer dragged from the point to `drag`) it
/// becomes smooth: the handle going on follows the drag and the one coming in is its mirror,
/// and a straight segment beside it becomes a curve. `None` when nothing would change, the span
/// is not a shape or the segment is not an anchor.
pub fn convert_anchor(
    document: &Document,
    scene: SceneId,
    at: SpanRef,
    segment: usize,
    drag: Option<DVec2>,
) -> Option<Vec<Command>> {
    let (_, _, mut path) = shape_path(document, scene, at)?;
    let anchor = end_of(*path.get(segment)?)?;
    let (first, stop, closed) = subpath(&path, segment)?;
    // The piece arriving at the point, and the piece leaving it.
    let incoming = if segment != first {
        Some(segment)
    } else if closed
        && stop - 1 > first
        && end_of(path[stop - 1]).is_some_and(|e| (e - anchor).length() <= SAME)
    {
        Some(stop - 1)
    } else {
        None
    };
    let outgoing = (segment + 1 < stop).then_some(segment + 1);
    let before = path.clone();
    match drag {
        None => {
            if let Some(i) = incoming
                && let PathSegment::CubicTo(c1, _, p) = path[i]
            {
                path[i] = PathSegment::CubicTo(c1, p, p);
            }
            if let Some(i) = outgoing
                && let PathSegment::CubicTo(_, c2, p) = path[i]
            {
                path[i] = PathSegment::CubicTo(point(anchor)?, c2, p);
            }
        }
        Some(to) => {
            let handle = to - anchor;
            if handle.length() <= SAME {
                return None;
            }
            if let Some(i) = incoming {
                let from = start_of(&path, i)?;
                let c1 = match path[i] {
                    PathSegment::CubicTo(c1, _, _) => pos(c1),
                    _ => from + (anchor - from) / 3.0,
                };
                path[i] = PathSegment::CubicTo(point(c1)?, point(anchor - handle)?, point(anchor)?);
            }
            if let Some(i) = outgoing {
                let end = end_of(path[i])?;
                let c2 = match path[i] {
                    PathSegment::CubicTo(_, c2, _) => pos(c2),
                    _ => end + (anchor - end) / 3.0,
                };
                path[i] = PathSegment::CubicTo(point(anchor + handle)?, point(c2)?, point(end)?);
            }
        }
    }
    if path == before {
        return None;
    }
    replace(document, scene, at, path)
}
