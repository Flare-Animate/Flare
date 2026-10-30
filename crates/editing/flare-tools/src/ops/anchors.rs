//! Editing the points of a shape's outline.

use emath::Pos2;
use flare_commands::Command;
use flare_core::{DocumentPoint, SceneId};
use flare_document::{Content, Document, FrameSpan, PathSegment, Shape};
use glamx::DVec2;

use super::*;

/// The outline points a person can grab: where each segment ends, with the index of that segment.
pub fn anchors(shape: &Shape) -> Vec<(usize, Pos2)> {
    shape
        .path()
        .iter()
        .enumerate()
        .filter_map(|(i, segment)| {
            let end = match segment {
                PathSegment::MoveTo(p) | PathSegment::LineTo(p) | PathSegment::CubicTo(_, _, p) => {
                    *p
                }
                PathSegment::Close => return None,
            };
            let v = end.as_vec();
            Some((i, Pos2::new(v.x as f32, v.y as f32)))
        })
        .collect()
}

/// Moves the anchor ending `segment` to `to`. A curve's handles move with it, so the curve keeps
/// its shape on either side. `None` when the span is not a shape, the segment is not an anchor,
/// or the point is not finite.
pub fn move_anchor(
    document: &Document,
    scene: SceneId,
    at: SpanRef,
    segment: usize,
    to: DVec2,
) -> Option<Vec<Command>> {
    let span = find_span(document, scene, at)?;
    let Content::Shape(shape) = span.content() else {
        return None;
    };
    let mut path = shape.path().to_vec();
    let from = match path.get(segment)? {
        PathSegment::MoveTo(p) | PathSegment::LineTo(p) | PathSegment::CubicTo(_, _, p) => {
            p.as_vec()
        }
        PathSegment::Close => return None,
    };
    let delta = to - from;
    if delta == DVec2::ZERO {
        return None;
    }
    let shift =
        |p: DocumentPoint| DocumentPoint::new(p.as_vec().x + delta.x, p.as_vec().y + delta.y).ok();
    path[segment] = match path[segment] {
        PathSegment::MoveTo(p) => PathSegment::MoveTo(shift(p)?),
        PathSegment::LineTo(p) => PathSegment::LineTo(shift(p)?),
        PathSegment::CubicTo(c1, c2, p) => PathSegment::CubicTo(c1, shift(c2)?, shift(p)?),
        PathSegment::Close => return None,
    };
    if let Some(PathSegment::CubicTo(c1, c2, p)) = path.get(segment + 1).copied() {
        path[segment + 1] = PathSegment::CubicTo(shift(c1)?, c2, p);
    }
    let moved = Shape::new(shape.id(), path, shape.fill())
        .ok()?
        .painted_like(shape);
    let replacement =
        FrameSpan::new(span.start(), span.duration(), Content::Shape(moved)).with_motion_of(span);
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
