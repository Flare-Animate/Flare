//! What the Add, Delete and Convert Anchor Point tools share: walking a path's pieces and putting
//! an edited path back in the span it came from.

use flare_commands::Command;
use flare_core::{DocumentPoint, SceneId};
use flare_document::{Content, Document, FrameSpan, PathSegment, Shape};
use glamx::DVec2;

use super::span::{SpanRef, find_span};

/// Two points this close are the same point (a path that returns to where it began).
pub(crate) const SAME: f64 = 1e-9;

/// How far along a segment a split must be to count as in its middle and not at an anchor.
pub(crate) const MIDDLE: f64 = 1e-3;

pub(crate) fn pos(p: DocumentPoint) -> DVec2 {
    p.as_vec()
}

pub(crate) fn point(p: DVec2) -> Option<DocumentPoint> {
    DocumentPoint::new(p.x, p.y).ok()
}

pub(crate) fn end_of(segment: PathSegment) -> Option<DVec2> {
    match segment {
        PathSegment::MoveTo(p) | PathSegment::LineTo(p) | PathSegment::CubicTo(_, _, p) => {
            Some(pos(p))
        }
        PathSegment::Close => None,
    }
}

/// The start of the subpath that the segment at `index` belongs to.
pub(crate) fn subpath_start(path: &[PathSegment], index: usize) -> Option<usize> {
    (0..=index.min(path.len().checked_sub(1)?))
        .rev()
        .take_while(|&i| i == index || !matches!(path[i], PathSegment::Close))
        .find(|&i| matches!(path[i], PathSegment::MoveTo(_)))
}

/// Where the segment at `index` starts: the end of the one before it, or the start of the subpath
/// the one before it closed.
pub(crate) fn start_of(path: &[PathSegment], index: usize) -> Option<DVec2> {
    let before = index.checked_sub(1)?;
    match path[before] {
        PathSegment::Close => end_of(path[subpath_start(path, before)?]),
        segment => end_of(segment),
    }
}

/// The segments of the subpath holding `index`: its first (the `MoveTo`), one past its last
/// drawn segment, and whether a `Close` ends it.
pub(crate) fn subpath(path: &[PathSegment], index: usize) -> Option<(usize, usize, bool)> {
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
pub(crate) fn shape_path(
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
pub(crate) fn replace(
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
