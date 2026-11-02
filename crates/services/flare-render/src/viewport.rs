//! The part of the stage a render covers.

#[allow(unused_imports)]
use crate::RenderError;
#[allow(unused_imports)]
use crate::image::unpremultiply_in_place;
#[allow(unused_imports)]
use crate::paint::{bez_path, clip_path, color, linear_gradient, stroke_style};

/// A rectangle of the stage, in stage units, to draw into an image of any size.
///
/// Rendering only what is on screen, at the resolution it is shown, keeps the work and the
/// memory proportional to the window rather than to the stage, and keeps zoomed-in views sharp.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Viewport {
    /// The whole stage at one image pixel per stage unit.
    pub fn full(width: u32, height: u32) -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: f64::from(width),
            height: f64::from(height),
        }
    }

    pub(crate) fn is_valid(&self) -> bool {
        [self.x, self.y, self.width, self.height]
            .iter()
            .all(|v| v.is_finite())
            && self.width > 0.0
            && self.height > 0.0
    }
}
