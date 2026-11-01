//! Text objects: swapping a shape for its next version, and taking text apart.

use flare_commands::Command;
use flare_core::SceneId;
use flare_document::{Content, Document, FrameSpan, Shape};

use super::*;

/// The commands that put `shape` in the place of the shape in the span at `at`, keeping the
/// span's length and motion. `None` when the span holds no shape or the shape is the same.
pub fn replace_shape(
    document: &Document,
    scene: SceneId,
    at: SpanRef,
    shape: Shape,
) -> Option<Vec<Command>> {
    let span = find_span(document, scene, at)?;
    let Content::Shape(old) = span.content() else {
        return None;
    };
    if *old == shape {
        return None;
    }
    let replacement =
        FrameSpan::new(span.start(), span.duration(), Content::Shape(shape)).with_motion_of(span);
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

/// Text taken apart: the letters stay as outlines and the words are forgotten, so the shape can
/// be edited point by point. `None` when the span holds no text.
pub fn break_text_apart(document: &Document, scene: SceneId, at: SpanRef) -> Option<Vec<Command>> {
    let span = find_span(document, scene, at)?;
    let Content::Shape(shape) = span.content() else {
        return None;
    };
    shape.text()?;
    replace_shape(document, scene, at, shape.clone().without_text())
}
