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
    let replacement =
        FrameSpan::new(span.start(), span.duration(), Content::Shape(painted)).with_motion_of(span);
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

/// Sets or clears the tween on a span: a motion tween on an instance, a shape tween on a shape.
/// `None` when the span is missing, the tween does not fit what the span holds, or it already
/// has that setting. The easing is kept.
pub fn set_tween(
    document: &Document,
    scene: SceneId,
    at: SpanRef,
    tween: Tween,
) -> Option<Vec<Command>> {
    let span = find_span(document, scene, at)?;
    let fits = matches!(
        (span.content(), tween),
        (_, Tween::None)
            | (Content::Instance(_), Tween::Linear)
            | (Content::Shape(_), Tween::Shape)
    );
    if !fits || span.tween() == tween {
        return None;
    }
    let replacement = FrameSpan::new(span.start(), span.duration(), span.content().clone())
        .with_tween(tween)
        .with_ease(span.ease());
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

/// Sets the easing of the span's tween, from `-MAX_EASE` to `MAX_EASE` (clamped). `None` when the
/// span is missing or has that easing already.
pub fn set_ease(
    document: &Document,
    scene: SceneId,
    at: SpanRef,
    ease: i8,
) -> Option<Vec<Command>> {
    let span = find_span(document, scene, at)?;
    let replacement = FrameSpan::new(span.start(), span.duration(), span.content().clone())
        .with_tween(span.tween())
        .with_ease(ease);
    if replacement.ease() == span.ease() {
        return None;
    }
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
