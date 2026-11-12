//! The Paint Bucket, Ink Bottle and Eyedropper, and the fill and stroke controls: repainting a
//! shape, and taking a shape's paint to paint with.

use flare_commands::Command;
use flare_core::{DocumentPoint, Rgba8, SceneId};
use flare_document::{Content, Document, Gradient, Shape, Stroke};

use crate::common::*;

pub mod area;
mod place;

pub use place::{area_shape, pour_above, pour_under};

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

/// The fill the Paint Bucket pours.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fill {
    Solid(Rgba8),
    /// A linear gradient across the whole shape, left to right or top to bottom, as the drawing
    /// tools lay one across what they draw.
    Linear {
        start: Rgba8,
        end: Rgba8,
        vertical: bool,
    },
}

/// The span's shape filled with `fill`, keeping its outline. `None` when the span holds no
/// shape or the fill would not change; a gradient over a shape with no width (or height, for a
/// vertical one) falls back to its start color, which is all that could show.
pub fn fill(document: &Document, scene: SceneId, at: SpanRef, fill: Fill) -> Option<Vec<Command>> {
    let (start, end, vertical) = match fill {
        Fill::Solid(color) => return restyle(document, scene, at, Some(color), None),
        Fill::Linear {
            start,
            end,
            vertical,
        } => (start, end, vertical),
    };
    let Some(Content::Shape(shape)) = find_element(document, scene, at) else {
        return None;
    };
    let Some(gradient) = gradient_across(shape, start, end, vertical) else {
        return restyle(document, scene, at, Some(start), None);
    };
    let painted = shape.clone().with_fill(start).with_gradient(gradient);
    if painted == *shape {
        return None;
    }
    replace_element(document, scene, at, Content::Shape(painted))
}

/// A gradient from one side of `shape`'s outline to the other, in its own coordinates.
pub(crate) fn gradient_across(
    shape: &Shape,
    start: Rgba8,
    end: Rgba8,
    vertical: bool,
) -> Option<Gradient> {
    // The bounds count half the stroke; the gradient spans the outline itself.
    let half = shape.stroke().map_or(0.0, |s| s.width() as f32 / 2.0);
    let bounds = shape_bounds(shape)?.shrink(half);
    let (x0, y0) = (f64::from(bounds.min.x), f64::from(bounds.min.y));
    let to = if vertical {
        (x0, f64::from(bounds.max.y))
    } else {
        (f64::from(bounds.max.x), y0)
    };
    let from = DocumentPoint::new(x0, y0).ok()?;
    let to = DocumentPoint::new(to.0, to.1).ok()?;
    Gradient::linear(from, to, start, end).ok()
}

/// What the Eyedropper takes from a shape.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Sample {
    /// The fill: its solid color, or the gradient it is painted with.
    Fill(Rgba8, Option<Gradient>),
    /// The whole stroke: color, width, caps, joins and style.
    Stroke(Stroke),
}

/// What the Eyedropper takes from `shape` where it was clicked: the stroke when the click is on
/// it (or anywhere on the shape with `stroke_only`), the fill when it is on a fill that shows.
/// `None` means the shape has nothing there to take, and what is under it should be asked.
pub fn sample(shape: &Shape, on_stroke: bool, on_fill: bool, stroke_only: bool) -> Option<Sample> {
    if let Some(stroke) = shape.stroke()
        && (on_stroke || stroke_only)
    {
        return Some(Sample::Stroke(stroke));
    }
    (on_fill && !stroke_only).then(|| Sample::Fill(shape.fill(), shape.gradient()))
}

/// Whether a gradient runs more down than across, as the fill controls' Vertical choice.
pub fn runs_vertically(gradient: &Gradient) -> bool {
    let (from, to) = (gradient.from().as_vec(), gradient.to().as_vec());
    (to.y - from.y).abs() > (to.x - from.x).abs()
}

#[cfg(test)]
mod area_tests;
#[cfg(test)]
mod place_tests;
#[cfg(test)]
mod tests;
