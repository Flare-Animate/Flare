//! The Delete Anchor Point tool: a click on a point takes it out, joining the outline across the gap.

use flare_commands::Command;
use flare_core::SceneId;
use flare_document::{Document, PathSegment};
use glamx::DVec2;

use crate::common::SpanRef;
use crate::common::path_edit::*;

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

#[cfg(test)]
mod tests;
