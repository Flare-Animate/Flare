//! Text objects: swapping a shape for its next version, and taking text apart.

use flare_commands::Command;
use flare_core::SceneId;
use flare_document::{Content, Document, Shape};

use crate::common::*;

/// The commands that put `shape` in the place of the shape in the span at `at`, keeping the
/// span's length and motion. `None` when the span holds no shape or the shape is the same.
pub fn replace_shape(
    document: &Document,
    scene: SceneId,
    at: SpanRef,
    shape: Shape,
) -> Option<Vec<Command>> {
    let span = find_span(document, scene, at)?;
    let Content::Shape(old) = span.element(at.element)? else {
        return None;
    };
    if *old == shape {
        return None;
    }
    replace_element(document, scene, at, Content::Shape(shape))
}

/// Text taken apart: the letters stay as outlines and the words are forgotten, so the shape can
/// be edited point by point. `None` when the span holds no text.
pub fn break_text_apart(document: &Document, scene: SceneId, at: SpanRef) -> Option<Vec<Command>> {
    let Content::Shape(shape) = find_element(document, scene, at)? else {
        return None;
    };
    shape.text()?;
    replace_shape(document, scene, at, shape.clone().without_text())
}

#[cfg(test)]
mod tests;
