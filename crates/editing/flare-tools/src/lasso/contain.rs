//! What a loop encloses.

use emath::Pos2;
use flare_core::SceneId;
use flare_document::{Content, Document, PathSegment};

use crate::common::*;

/// Whether `point` is inside the closed polygon `polygon` (even-odd rule).
pub fn polygon_contains(polygon: &[Pos2], point: Pos2) -> bool {
    let mut inside = false;
    let mut previous = match polygon.last() {
        Some(p) => *p,
        None => return false,
    };
    for &current in polygon {
        if (current.y > point.y) != (previous.y > point.y)
            && point.x
                < (previous.x - current.x) * (point.y - current.y) / (previous.y - current.y)
                    + current.x
        {
            inside = !inside;
        }
        previous = current;
    }
    inside
}

/// Points along a shape's path (corners, and eight along each curve), so any part of a pencil,
/// brush or line stroke inside a loop is found, not only the middle of its bounds.
fn path_points(content: &Content) -> Vec<Pos2> {
    let Content::Shape(shape) = content else {
        return Vec::new();
    };
    let at = |p: &flare_core::DocumentPoint| Pos2::new(p.as_vec().x as f32, p.as_vec().y as f32);
    let mut out = Vec::new();
    let mut last = None;
    for segment in shape.path() {
        match segment {
            PathSegment::MoveTo(p) | PathSegment::LineTo(p) => {
                out.push(at(p));
                last = Some(at(p));
            }
            PathSegment::CubicTo(a, b, c) => {
                let (a, b, c) = (at(a), at(b), at(c));
                if let Some(from) = last {
                    for i in 1..8 {
                        let t = i as f32 / 8.0;
                        let u = 1.0 - t;
                        let mix = |f: fn(Pos2) -> f32| {
                            u * u * u * f(from)
                                + 3.0 * u * u * t * f(a)
                                + 3.0 * u * t * t * f(b)
                                + t * t * t * f(c)
                        };
                        out.push(Pos2::new(mix(|p| p.x), mix(|p| p.y)));
                    }
                }
                out.push(c);
                last = Some(c);
            }
            PathSegment::Close => {}
        }
    }
    out
}

/// The elements on unlocked layers at `frame` with any part inside `polygon` (a point of a shape's
/// path, or the center of its bounds): what a lasso drawn around them selects, topmost layer first.
pub fn spans_in_polygon(
    document: &Document,
    scene: SceneId,
    frame: u32,
    polygon: &[Pos2],
) -> Vec<SpanRef> {
    if polygon.len() < 3 {
        return Vec::new();
    }
    let Some(timeline) = scene_timeline(document, scene) else {
        return Vec::new();
    };
    timeline
        .layers()
        .iter()
        .filter(|layer| !layer.is_locked())
        .filter_map(|layer| Some((layer, layer.span_at(frame)?)))
        .flat_map(|(layer, span)| {
            span.elements()
                .enumerate()
                .filter_map(|(element, content)| {
                    let center = content_bounds(document, content)?.center();
                    // The middle of a stroke-only shape's bounds is often empty space.
                    let hollow = matches!(content, Content::Shape(s) if s.fill_is_invisible());
                    ((!hollow && polygon_contains(polygon, center))
                        || path_points(content)
                            .into_iter()
                            .any(|p| polygon_contains(polygon, p)))
                    .then_some(SpanRef {
                        layer: layer.id(),
                        start: span.start(),
                        element,
                    })
                })
                .collect::<Vec<_>>()
        })
        .collect()
}
