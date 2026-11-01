//! The Convert Anchor Point tool: a click makes a smooth point a corner, a drag makes a corner smooth.

use flare_commands::Command;
use flare_core::SceneId;
use flare_document::{Document, PathSegment};
use glamx::DVec2;

use crate::common::SpanRef;
use crate::common::path_edit::*;

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

#[cfg(test)]
mod tests;
