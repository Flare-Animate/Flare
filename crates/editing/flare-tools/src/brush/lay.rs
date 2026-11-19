//! Laying finished brush strokes on a layer: what the Brush does when the pen lifts, for a
//! surface that hands over whole strokes at once (the command line, the MCP server) as much as
//! for the desktop's drag. Each stroke becomes a filled outline in the paint color; all of them
//! join the layer's keyframe in one batch, so they are one undo step.

use std::fmt;
use std::num::NonZeroU32;

use flare_core::{Rgba8, SceneId, ShapeId};
use flare_document::{Document, Shape, ShapeError};
use flare_input::{InputSample, Stabilizer};

use super::{BrushStroke, BuiltinBrushProfile};
use crate::drawing::{DrawPlan, DrawRefusal, Target, draw_shapes};

/// The most strokes laid in one go.
pub const MAX_STROKES: usize = 4096;
/// The most samples one stroke may have: far more than a long drag reports.
pub const MAX_STROKE_SAMPLES: usize = 16_384;
/// The most samples all the strokes of one go may have together.
pub const MAX_TOTAL_SAMPLES: usize = 1_000_000;

/// The filled outline `brush` leaves through `samples` in `color`, partly see-through as far as
/// `flow` (0 to 1) is below 1, its centre line simplified to within `tolerance`; `None` for a
/// click, which draws nothing.
pub fn stroke_shape(
    brush: &BrushStroke,
    samples: &[InputSample],
    color: Rgba8,
    flow: f32,
    tolerance: f64,
) -> Result<Option<Shape>, ShapeError> {
    let Some(path) = brush.outline(samples, tolerance) else {
        return Ok(None);
    };
    let flow = if flow.is_finite() {
        flow.clamp(0.0, 1.0)
    } else {
        1.0
    };
    // Flow is how opaque the paint is at the first touch.
    let alpha = (f32::from(color.a) * flow) as u8;
    let fill = Rgba8::new(color.r, color.g, color.b, alpha);
    Shape::new(ShapeId::new(), path, fill).map(Some)
}

/// `samples` as a hand steadied by a stabilizer at `level` draws them, ending where the pen
/// lifted. Level 0 or 1 hands them through unchanged.
pub fn steady(samples: &[InputSample], level: u8) -> Vec<InputSample> {
    if level <= 1 {
        return samples.to_vec();
    }
    // A stroke handed over whole has no screen behind it, so a stage unit counts as a pixel.
    let mut stabilizer = Stabilizer::new(level, true, 1.0);
    let mut out: Vec<InputSample> = samples.iter().map(|s| stabilizer.push(*s)).collect();
    out.extend(stabilizer.finish());
    out
}

/// What the strokes are painted with besides the brush's shape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Paint {
    pub color: Rgba8,
    /// How opaque the paint is, 0 to 1.
    pub flow: f32,
    /// Samples averaged to steady the hand, 0 (off) to [`flare_input::MAX_STABILIZER_LEVEL`].
    pub stabilizer: u8,
    /// How far, in stage units, the centre line may be simplified.
    pub tolerance: f64,
}

/// Why strokes were not laid.
#[derive(Debug, PartialEq)]
pub enum LayRefusal {
    NoStrokes,
    TooManyStrokes(usize),
    /// Stroke `index` has more samples than [`MAX_STROKE_SAMPLES`].
    TooManySamples {
        index: usize,
        count: usize,
    },
    TooManyTotalSamples(usize),
    /// Every stroke was a click: none moved, so nothing would be drawn.
    NothingDrawn,
    /// Stroke `index` made an outline that is not a valid shape (too long, most likely).
    Shape {
        index: usize,
        error: ShapeError,
    },
    Draw(DrawRefusal),
}

impl fmt::Display for LayRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoStrokes => f.write_str("no strokes were given"),
            Self::TooManyStrokes(n) => write!(f, "{n} strokes is more than {MAX_STROKES} at once"),
            Self::TooManySamples { index, count } => write!(
                f,
                "stroke {index} has {count} points, more than {MAX_STROKE_SAMPLES}"
            ),
            Self::TooManyTotalSamples(n) => write!(
                f,
                "{n} points in all is more than {MAX_TOTAL_SAMPLES} at once"
            ),
            Self::NothingDrawn => f.write_str("no stroke moves, so the brush draws nothing"),
            Self::Shape { index, error } => write!(f, "stroke {index} cannot be drawn: {error}"),
            Self::Draw(refusal) => write!(f, "cannot draw: {refusal}"),
        }
    }
}

impl std::error::Error for LayRefusal {}

/// Where the strokes went.
#[derive(Debug, Clone, PartialEq)]
pub struct LayPlan {
    pub draw: DrawPlan,
    /// The indices of the strokes that were clicks and drew nothing.
    pub skipped: Vec<usize>,
}

/// Where the strokes go: the scene, the layer, the frame and, for a new keyframe, its length.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    pub scene: SceneId,
    pub target: Target,
    pub frame: u32,
    pub duration: Option<NonZeroU32>,
}

/// The commands that lay `strokes` with `brush` and `paint` at `placement`, as one batch and so
/// one undo step, above what is there and in order. A layer gets "Layer 1" when there is none;
/// the layer rules are [`draw_shapes`]'s. Clicks are skipped and listed; strokes past the bounds,
/// an outline that is not a valid shape, or nothing drawn at all, are refused whole.
pub fn lay_strokes(
    document: &Document,
    placement: Placement,
    brush: &BrushStroke,
    paint: Paint,
    strokes: &[Vec<InputSample>],
) -> Result<LayPlan, LayRefusal> {
    if strokes.is_empty() {
        return Err(LayRefusal::NoStrokes);
    }
    if strokes.len() > MAX_STROKES {
        return Err(LayRefusal::TooManyStrokes(strokes.len()));
    }
    let mut total = 0usize;
    for (index, stroke) in strokes.iter().enumerate() {
        if stroke.len() > MAX_STROKE_SAMPLES {
            return Err(LayRefusal::TooManySamples {
                index,
                count: stroke.len(),
            });
        }
        total += stroke.len();
    }
    if total > MAX_TOTAL_SAMPLES {
        return Err(LayRefusal::TooManyTotalSamples(total));
    }
    let mut shapes = Vec::with_capacity(strokes.len());
    let mut skipped = Vec::new();
    for (index, stroke) in strokes.iter().enumerate() {
        let samples = steady(stroke, paint.stabilizer);
        match stroke_shape(brush, &samples, paint.color, paint.flow, paint.tolerance) {
            Ok(Some(shape)) => shapes.push(shape),
            Ok(None) => skipped.push(index),
            Err(error) => return Err(LayRefusal::Shape { index, error }),
        }
    }
    if shapes.is_empty() {
        return Err(LayRefusal::NothingDrawn);
    }
    let draw = draw_shapes(
        document,
        placement.scene,
        placement.target,
        placement.frame,
        placement.duration,
        shapes,
        "Layer 1",
    )
    .map_err(LayRefusal::Draw)?;
    Ok(LayPlan { draw, skipped })
}

impl BuiltinBrushProfile {
    /// The profile called `name`: its short key (`plain`, `ink`, `g-pen`, `dry-brush`...) or its
    /// full name, in any case, with spaces, hyphens and other punctuation ignored.
    pub fn find(name: &str) -> Option<Self> {
        let key = |text: &str| -> String {
            text.chars()
                .filter(char::is_ascii_alphanumeric)
                .map(|c| c.to_ascii_lowercase())
                .collect()
        };
        let wanted = key(name);
        if wanted.is_empty() {
            return None;
        }
        Self::ALL
            .into_iter()
            .find(|p| key(&format!("{p:?}")) == wanted || key(p.name()) == wanted)
    }

    /// The short keys [`BuiltinBrushProfile::find`] takes, for messages and help.
    pub fn keys() -> Vec<String> {
        Self::ALL
            .iter()
            .map(|p| {
                let mut key = String::new();
                for (i, c) in format!("{p:?}").chars().enumerate() {
                    if c.is_ascii_uppercase() && i > 0 {
                        key.push('-');
                    }
                    key.push(c.to_ascii_lowercase());
                }
                key
            })
            .collect()
    }
}
