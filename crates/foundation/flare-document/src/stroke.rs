//! A stroke: the outline drawn along a shape's path.

use flare_core::Rgba8;

use crate::shape::{MAX_STROKE_WIDTH, ShapeError};
use crate::stroke_style::StrokeStyle;

/// How an open line ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Cap {
    /// Stops exactly at the end point.
    Butt,
    #[default]
    Round,
    /// Stops half the width past the end point.
    Square,
}

/// How a stroke turns a corner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Join {
    #[default]
    Round,
    Bevel,
    Miter,
}

/// The kind of brush a stroke was drawn with. Stored on the stroke so the brush can be
/// swapped or edited after the fact (issue #25) instead of being baked into the path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BrushType {
    /// A uniform-width stroke (the default, matches classic Flash/Animate).
    #[default]
    Uniform,
    /// A stroke whose width follows pen pressure; the per-point pressure is stored
    /// alongside the path.
    Pressure,
    /// A stroke whose width is fixed but tapered at the ends (calligraphy-like).
    Tapered,
}

/// An outline drawn along a shape's path. Round caps and joins unless it says otherwise.
///
/// `opacity` is a multiplier in `[0, 1]` applied on top of `color`'s alpha, so a stroke
/// can be faded without touching its color. `brush` records which brush produced the
/// stroke so it can be swapped or edited after drawing (issue #25) the way Clip Studio
/// Paint's vector brushes do: the stroke keeps its identity and its width/opacity/brush
/// can be edited after the fact instead of being baked into the path.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stroke {
    width: f64,
    color: Rgba8,
    opacity: f64,
    brush: BrushType,
    cap: Cap,
    join: Join,
    style: StrokeStyle,
}

impl Stroke {
    pub fn new(width: f64, color: Rgba8) -> Result<Self, ShapeError> {
        Self::new_with_opacity(width, color, 1.0)
    }

    /// Like [`new`](Self::new), but with an explicit opacity in `[0, 1]`.
    pub fn new_with_opacity(width: f64, color: Rgba8, opacity: f64) -> Result<Self, ShapeError> {
        if !(width.is_finite() && width > 0.0 && width <= MAX_STROKE_WIDTH) {
            return Err(ShapeError::InvalidStrokeWidth);
        }
        if !(opacity.is_finite() && (0.0..=1.0).contains(&opacity)) {
            return Err(ShapeError::InvalidStrokeWidth);
        }
        Ok(Self {
            width,
            color,
            opacity,
            brush: BrushType::Uniform,
            cap: Cap::Round,
            join: Join::Round,
            style: StrokeStyle::Solid,
        })
    }

    /// The same stroke with line ends of another kind.
    #[must_use]
    pub fn with_cap(mut self, cap: Cap) -> Self {
        self.cap = cap;
        self
    }

    /// The same stroke with corners of another kind.
    #[must_use]
    pub fn with_join(mut self, join: Join) -> Self {
        self.join = join;
        self
    }

    /// The same stroke drawn in another style (see `StrokeStyle`).
    #[must_use]
    pub fn with_style(mut self, style: StrokeStyle) -> Self {
        self.style = style;
        self
    }

    pub fn style(&self) -> StrokeStyle {
        self.style
    }

    /// The same stroke with a different opacity in `[0, 1]`.
    #[must_use]
    pub fn with_opacity(mut self, opacity: f64) -> Self {
        if opacity.is_finite() && (0.0..=1.0).contains(&opacity) {
            self.opacity = opacity;
        }
        self
    }

    pub fn opacity(&self) -> f64 {
        self.opacity
    }

    /// The same stroke with a different brush type.
    #[must_use]
    pub fn with_brush(mut self, brush: BrushType) -> Self {
        self.brush = brush;
        self
    }

    pub fn brush(&self) -> BrushType {
        self.brush
    }

    pub fn cap(&self) -> Cap {
        self.cap
    }

    pub fn join(&self) -> Join {
        self.join
    }

    pub fn width(&self) -> f64 {
        self.width
    }

    pub fn color(&self) -> Rgba8 {
        self.color
    }
}
