//! The Eraser tool and the Delete key: rubbing out the parts of shapes the eraser passes over,
//! and taking whole elements out of their spans.

use flare_commands::Command;
use flare_core::{LayerId, SceneId};
use flare_document::{Content, Document, Layer, Tween};
use flare_input::{InputSample, PressureCurve};
use glamx::DVec2;

use crate::brush::BrushStroke;
use crate::common::{SpanRef, by_span, find_span, remove_element, replace_span, scene_timeline};

mod cut;
mod frame;
pub use cut::{Cut, cut};

/// The widest eraser, in stage units.
pub const MAX_ERASER_SIZE: f64 = 400.0;
/// Points round the outline of an eraser that is pressed without moving.
const DOT_STEPS: usize = 32;

/// The area a round eraser `size` across covers along `samples`: the outline of the stroke it
/// rubs, or a disc where it was only pressed. Empty for a size that is not a positive number.
pub fn eraser_outline(samples: &[InputSample], size: f64) -> Vec<DVec2> {
    if !(size.is_finite() && size > 0.0) {
        return Vec::new();
    }
    let size = size.min(MAX_ERASER_SIZE);
    // An eraser rubs as hard wherever it goes: full width everywhere, whatever the pen says.
    let firm: Vec<InputSample> = samples
        .iter()
        .map(|s| InputSample {
            pressure: 1.0,
            ..*s
        })
        .collect();
    let stroke = BrushStroke::new(size, PressureCurve::new(1.0, 1.0));
    if let Some(sides) = stroke.sides(&firm, 0.0) {
        return sides
            .left
            .iter()
            .chain(&sides.end_cap)
            .chain(sides.right.iter().rev())
            .chain(&sides.start_cap)
            .copied()
            .collect();
    }
    let Some(at) = samples.first() else {
        return Vec::new();
    };
    (0..DOT_STEPS)
        .map(|k| {
            let angle = std::f64::consts::TAU * k as f64 / DOT_STEPS as f64;
            DVec2::new(at.x, at.y) + DVec2::new(angle.cos(), angle.sin()) * (size / 2.0)
        })
        .collect()
}

/// The commands that rub out the inside of the closed outlines `eraser` from every shape at
/// `frame` on the layers a drawing tool may change (shown and unlocked, not a folder or the
/// camera, in a keyframe that does not tween), as one undo step. With `only`, just that layer
/// (or the layers in that folder) is rubbed out, so what is drawn on the others is safe; with
/// none, every such layer. Symbols, bitmaps and text are left whole. Only `frame` changes: a
/// frame held from an earlier keyframe becomes a keyframe of its own first, and the frames around
/// it keep the drawing. Empty when the eraser touched nothing.
pub fn erase(
    document: &Document,
    scene: SceneId,
    frame: u32,
    eraser: &[Vec<DVec2>],
    only: Option<LayerId>,
) -> Vec<Command> {
    let Some(timeline) = scene_timeline(document, scene) else {
        return Vec::new();
    };
    let layers = timeline.layers();
    let mut out = Vec::new();
    for (index, layer) in layers.iter().enumerate() {
        if layer.is_locked()
            || layer.is_folder()
            || layer.is_camera()
            || hidden(layers, index)
            || !reaches(layers, index, only)
        {
            continue;
        }
        let Some(span) = layer.span_at(frame) else {
            continue;
        };
        if span.tween() != Tween::None {
            continue;
        }
        let mut changed = false;
        let mut kept = Vec::with_capacity(span.element_count());
        for content in span.elements() {
            match content {
                Content::Shape(shape) => match cut(shape, eraser) {
                    Cut::Untouched => kept.push(content.clone()),
                    Cut::Left(pieces) => {
                        changed = true;
                        kept.extend(pieces.into_iter().map(Content::Shape));
                    }
                },
                other => kept.push(other.clone()),
            }
        }
        if !changed {
            continue;
        }
        out.extend(frame::keep_on_frame(scene, layer.id(), span, frame, kept));
    }
    out
}

/// Whether an eraser kept to the layer `only` reaches `layers[index]`: with none, every layer;
/// else that layer and, when it is a folder, the layers in it however deep.
pub fn reaches(layers: &[Layer], index: usize, only: Option<LayerId>) -> bool {
    let Some(mut layer) = layers.get(index) else {
        return false;
    };
    let Some(only) = only else {
        return true;
    };
    // A layer's folders come before it; the walk is bounded in case a document nests in a loop.
    for _ in 0..=index {
        if layer.id() == only {
            return true;
        }
        let Some(folder) = layer
            .parent()
            .and_then(|id| layers[..index].iter().find(|l| l.id() == id))
        else {
            return false;
        };
        layer = folder;
    }
    false
}

/// Whether `layers[index]` is hidden, by itself or by a folder it is in.
fn hidden(layers: &[Layer], index: usize) -> bool {
    if !layers[index].is_visible() {
        return true;
    }
    let mut parent = layers[index].parent();
    while let Some(id) = parent {
        let Some(folder) = layers[..index].iter().find(|l| l.id() == id) else {
            return false;
        };
        if !folder.is_visible() {
            return true;
        }
        parent = folder.parent();
    }
    false
}

/// Takes the element `at` names out of its span: the span goes when it was the only element, and
/// stays with the others when not. Empty when the element is not there.
pub fn delete(document: &Document, scene: SceneId, at: SpanRef) -> Vec<Command> {
    remove_element(document, scene, at).unwrap_or_default()
}

/// [`delete`] as the Eraser's Faucet does it, on `frame` only: on a frame held from an earlier
/// keyframe the frame becomes a keyframe of its own without the element, and the frames around it
/// keep it. Empty when the element is not there.
pub fn delete_on_frame(
    document: &Document,
    scene: SceneId,
    at: SpanRef,
    frame: u32,
) -> Vec<Command> {
    let Some(span) = find_span(document, scene, at) else {
        return Vec::new();
    };
    if span.element(at.element).is_none() {
        return Vec::new();
    }
    let kept = span
        .elements()
        .enumerate()
        .filter(|(i, _)| *i != at.element)
        .map(|(_, content)| content.clone())
        .collect();
    frame::keep_on_frame(scene, at.layer, span, frame, kept)
}

/// [`delete`] for every element of `refs` as one list of commands: the elements of one span leave
/// together, and a span left with nothing goes.
pub fn delete_many(document: &Document, scene: SceneId, refs: &[SpanRef]) -> Vec<Command> {
    let mut out = Vec::new();
    for group in by_span(refs) {
        let Some(first) = group.first() else {
            continue;
        };
        let Some(span) = find_span(document, scene, *first) else {
            continue;
        };
        let kept: Vec<_> = span
            .elements()
            .enumerate()
            .filter(|(i, _)| !group.iter().any(|at| at.element == *i))
            .map(|(_, content)| content.clone())
            .collect();
        if kept.len() == span.element_count() {
            continue;
        }
        match span.with_elements(kept) {
            Ok(Some(rest)) => out.extend(replace_span(scene, *first, rest)),
            Ok(None) => out.push(Command::RemoveSpan {
                scene,
                layer: first.layer,
                start: first.start,
            }),
            Err(_) => {}
        }
    }
    out
}

#[cfg(test)]
mod cut_tests;
#[cfg(test)]
mod tests;
