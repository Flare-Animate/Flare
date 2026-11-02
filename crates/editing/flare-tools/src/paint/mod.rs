//! The Paint Bucket and Ink Bottle tools, and the fill and stroke controls: repainting a shape.

use flare_commands::Command;
use flare_core::{Rgba8, SceneId};
use flare_document::{Content, Document, Stroke};

use crate::common::*;

/// The span's shape repainted: `fill` replaces the solid fill (dropping a gradient) and `stroke`
/// replaces the outline (`Some(None)` removes it). `None` when the span holds no shape or the
/// paint would not change.
pub fn restyle(
    document: &Document,
    scene: SceneId,
    at: SpanRef,
    fill: Option<Rgba8>,
    stroke: Option<Option<Stroke>>,
) -> Option<Vec<Command>> {
    let span = find_span(document, scene, at)?;
    let Some(Content::Shape(shape)) = span.element(at.element) else {
        return None;
    };
    let mut painted = shape.clone();
    if let Some(fill) = fill {
        painted = painted.with_fill(fill);
    }
    if let Some(stroke) = stroke {
        painted = painted.restroked(stroke);
    }
    if painted == *shape {
        return None;
    }
    replace_element(document, scene, at, Content::Shape(painted))
}
