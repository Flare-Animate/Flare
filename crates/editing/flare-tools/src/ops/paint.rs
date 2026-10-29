//! Repainting shapes and tweening spans.

use flare_commands::Command;
use flare_core::{Rgba8, SceneId};
use flare_document::{Content, Document, FrameSpan, Stroke, Tween};

use super::*;

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
    let Content::Shape(shape) = span.content() else {
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
    let replacement = FrameSpan::new(span.start(), span.duration(), Content::Shape(painted))
        .with_tween(span.tween());
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

/// Sets or clears the motion tween on an instance span. `None` when the span is missing,
/// is not an instance (only transforms tween), or already has that setting.
pub fn set_tween(
    document: &Document,
    scene: SceneId,
    at: SpanRef,
    tween: Tween,
) -> Option<Vec<Command>> {
    let span = find_span(document, scene, at)?;
    if !matches!(span.content(), Content::Instance(_)) || span.tween() == tween {
        return None;
    }
    let replacement =
        FrameSpan::new(span.start(), span.duration(), span.content().clone()).with_tween(tween);
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
