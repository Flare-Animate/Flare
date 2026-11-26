//! Converting a layer between vector and raster, keeping what it shows: every keyframe's shapes,
//! instances and pictures drawn into one painting (to raster), or every picture traced into
//! filled shapes (to vector), and the layer's kind switched, as one batch of commands. Every
//! surface calls [`convert_layer`]; the window runs it on the pixels worker, since drawing and
//! tracing a stage-sized painting per keyframe takes too long for a click.

use flare_commands::{Command, apply_all};
use flare_core::{LayerId, SceneId};
use flare_document::{Content, Document, Tween};
use flare_tools::blend::PAINT_DENSITY;
use flare_tools::ops::SpanRef;
use flare_tools::to_bitmap::ToBitmapError;
use flare_tools::trace::{TraceError, trace_keyframe};
use thiserror::Error;

use crate::to_bitmap::convert_to_bitmap;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LayerConvertError {
    #[error("there is no such layer in the scene")]
    NoSuchLayer,
    #[error("the layer is locked")]
    Locked,
    #[error("{0}")]
    Refused(#[from] flare_commands::layers::LayerRefusal),
    #[error("the keyframe at frame {frame} could not be drawn into pixels: {error}")]
    ToRaster { frame: u32, error: ToBitmapError },
    #[error("the keyframe at frame {frame} could not be traced: {error}")]
    ToVector { frame: u32, error: TraceError },
    #[error("the converted layer is not valid: {0}")]
    Invalid(String),
}

/// The commands that make `layer` a raster layer (`raster`) or a vector one, converting what each
/// keyframe holds so it looks the same: one undo step. Nothing when the layer is that kind
/// already. A painting is named `name` in the library.
pub fn convert_layer(
    document: &Document,
    scene: SceneId,
    layer: LayerId,
    raster: bool,
    name: &str,
) -> Result<Vec<Command>, LayerConvertError> {
    let found = document
        .scenes()
        .iter()
        .find(|s| s.id() == scene)
        .and_then(|s| s.timeline().layers().iter().find(|l| l.id() == layer))
        .ok_or(LayerConvertError::NoSuchLayer)?;
    let kind = flare_commands::layers::set_raster(document, scene, layer, raster)?;
    if kind.is_empty() {
        return Ok(Vec::new());
    }
    if found.is_locked() {
        return Err(LayerConvertError::Locked);
    }
    let starts: Vec<u32> = found.spans().iter().map(|s| s.start()).collect();
    // Each keyframe is converted in the document as the ones before left it, so the library's
    // pictures are added and taken out once however many keyframes share them.
    let mut work = document.clone();
    let mut commands = Vec::new();
    for start in starts {
        let step = if raster {
            paint_keyframe(&work, scene, layer, start, name)?
        } else {
            trace_keyframe(&work, scene, layer, start).map_err(|error| {
                LayerConvertError::ToVector {
                    frame: start + 1,
                    error,
                }
            })?
        };
        if step.is_empty() {
            continue;
        }
        if let Some(next) =
            apply_all(&work, step.clone()).map_err(|e| LayerConvertError::Invalid(e.to_string()))?
        {
            work = next;
        }
        commands.extend(step);
    }
    commands.extend(kind);
    Ok(commands)
}

/// The commands that draw the keyframe's shapes, instances and pictures into one painting at
/// `PAINT_DENSITY` pixels to a unit (one, where that would be too large). Nothing to do for a
/// keyframe of pictures alone, which a raster layer already paints into, or one that draws
/// nothing.
fn paint_keyframe(
    document: &Document,
    scene: SceneId,
    layer: LayerId,
    start: u32,
    name: &str,
) -> Result<Vec<Command>, LayerConvertError> {
    let at = SpanRef {
        layer,
        start,
        element: 0,
    };
    let Some(span) = flare_tools::ops::find_span(document, scene, at) else {
        return Ok(Vec::new());
    };
    let drawable: Vec<SpanRef> = span
        .elements()
        .enumerate()
        .filter(|(_, c)| {
            matches!(
                c,
                Content::Shape(_) | Content::Instance(_) | Content::Bitmap(_)
            )
        })
        .map(|(element, _)| at.with_element(element))
        .collect();
    let only_pictures = span.elements().all(|c| matches!(c, Content::Bitmap(_)));
    if drawable.is_empty() || only_pictures {
        return Ok(Vec::new());
    }
    let failed = |error| LayerConvertError::ToRaster {
        frame: start + 1,
        error,
    };
    if span.tween() != Tween::None {
        return Err(failed(ToBitmapError::Tweened));
    }
    let edit = match convert_to_bitmap(document, scene, start, &drawable, PAINT_DENSITY, name) {
        Err(ToBitmapError::TooLarge(..)) => {
            convert_to_bitmap(document, scene, start, &drawable, 1, name)
        }
        other => other,
    };
    match edit {
        Ok(edit) => Ok(edit.commands),
        // Shapes that draw nothing (all clear) leave nothing to paint.
        Err(ToBitmapError::Empty) => Ok(Vec::new()),
        Err(error) => Err(failed(error)),
    }
}

#[cfg(test)]
#[path = "layer_convert_tests.rs"]
mod tests;
