//! The Pencil under a pen: a fine line that thins where the pen presses lightly.
//!
//! A mouse line, a pen line pressed fully all the way, or a line in a dashed or other drawn style, stays the plain stroked path the
//! Pencil has always made, so its width, cap and style can be changed later. A pressed line has a
//! different width at each point, which a stroke of one width cannot hold, so it is kept as the
//! filled outline the Brush makes, in the stroke color: round, with no texture, and starting
//! from a thinner touch than the Brush, as a pencil line is a finer mark.

use flare_input::{InputSample, PointerKind, PressureCurve};

use crate::brush::BrushStroke;

/// The lightest touch's share of the full width when nothing else is set.
pub const DEFAULT_PENCIL_LIGHT: f32 = 0.1;

/// Whether the Pencil follows pressure, and how thin its lightest touch is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PencilPressure {
    on: bool,
    light: f32,
}

impl Default for PencilPressure {
    fn default() -> Self {
        Self::new(true, DEFAULT_PENCIL_LIGHT)
    }
}

impl PencilPressure {
    /// `light` is the width at the lightest touch as a share of the full width, kept in 0 to 1.
    pub fn new(on: bool, light: f32) -> Self {
        let light = if light.is_finite() {
            light.clamp(0.0, 1.0)
        } else {
            DEFAULT_PENCIL_LIGHT
        };
        Self { on, light }
    }

    pub fn is_on(&self) -> bool {
        self.on
    }

    pub fn light(&self) -> f32 {
        self.light
    }

    /// Whether a line through `samples` follows pressure: it is on, and a pen (or a touch that
    /// reports force) drew it pressing less than fully somewhere. A mouse presses fully
    /// everywhere, and so does a pen held hard all the way, so their line is as wide throughout as
    /// the stroke width and stays a plain stroke that can be edited later.
    pub fn applies(&self, samples: &[InputSample]) -> bool {
        self.on
            && samples
                .iter()
                .any(|s| s.kind != PointerKind::Mouse && s.pressure < 1.0)
    }

    /// The shape of a pressed line `width` wide at full pressure.
    pub fn stroke(&self, width: f64) -> BrushStroke {
        // A little firmer than straight: the light end of the pen's range stays fine.
        BrushStroke::new(width, PressureCurve::new(self.light, 1.3))
    }
}
