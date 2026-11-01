//! Tweens and their easing: what a span does between its keyframe and the next.

use flare_commands::Command;
use flare_core::SceneId;
use flare_document::{Content, Document, FrameSpan, Tween};

use crate::common::*;

/// Sets or clears the tween on a span: a motion tween on an instance or a camera keyframe, a shape
/// tween on a shape.
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
            | (Content::Instance(_) | Content::Camera(_), Tween::Linear)
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

#[cfg(test)]
mod tests;
