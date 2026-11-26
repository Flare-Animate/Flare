//! One keyframe's pictures traced into shapes, as one batch of commands.

use flare_commands::Command;
use flare_core::{BitmapId, LayerId, SceneId};
use flare_document::{Content, Document, MAX_ELEMENTS, Tween};

use super::{TraceError, trace_bitmap};
use crate::common::{SpanRef, find_span, replace_span};
use crate::magic_wand::uses;

/// The commands that replace every picture in the keyframe of `layer` starting at `start` with the
/// shapes that trace it, in its place in the stacking order, and take a picture out of the library
/// once nothing else shows it. Shapes, instances and anything else are kept as they are. Nothing
/// to do (no picture there) is no commands; a keyframe left with nothing is a blank keyframe.
pub fn trace_keyframe(
    document: &Document,
    scene: SceneId,
    layer: LayerId,
    start: u32,
) -> Result<Vec<Command>, TraceError> {
    let at = SpanRef {
        layer,
        start,
        element: 0,
    };
    let span = find_span(document, scene, at).ok_or(TraceError::Missing)?;
    if !span.elements().any(|c| matches!(c, Content::Bitmap(_))) {
        return Ok(Vec::new());
    }
    if span.tween() != Tween::None {
        return Err(TraceError::Tweened);
    }
    let mut kept = Vec::with_capacity(span.element_count());
    let mut traced: Vec<BitmapId> = Vec::new();
    for content in span.elements() {
        let Content::Bitmap(placed) = content else {
            kept.push(content.clone());
            continue;
        };
        let Some(bitmap) = document.bitmap(placed.bitmap()) else {
            // A picture missing from the library draws nothing; it is left for the file's own
            // checks to report.
            kept.push(content.clone());
            continue;
        };
        kept.extend(
            trace_bitmap(bitmap, placed)?
                .into_iter()
                .map(Content::Shape),
        );
        if kept.len() > MAX_ELEMENTS {
            return Err(TraceError::TooManyShapes);
        }
        traced.push(placed.bitmap());
    }
    if kept.is_empty() {
        kept.push(Content::Empty);
    }
    let new_span = span
        .with_elements(kept)
        .map_err(|e| TraceError::Shape(e.to_string()))?
        .ok_or(TraceError::Missing)?;
    let mut commands = replace_span(scene, at, new_span);
    let mut dropped: Vec<BitmapId> = Vec::new();
    for id in &traced {
        let here = traced.iter().filter(|t| *t == id).count();
        if !dropped.contains(id) && uses(document, *id) == here {
            dropped.push(*id);
        }
    }
    commands.extend(dropped.into_iter().map(Command::RemoveBitmap));
    Ok(commands)
}
