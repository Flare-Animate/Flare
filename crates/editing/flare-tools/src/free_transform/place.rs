//! Where an instance is put: a point, a turn and a scale about its registration point, as
//! Animate's Transform panel sets them. The command line and the MCP server place instances
//! through this, so they accept and refuse the same.

use std::fmt;

use flare_core::Transform2D;
use glamx::{DAffine2, DVec2};

/// Why a placement cannot be made.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlacementError {
    /// A position, angle or scale that is not a finite number.
    NotFinite,
    /// A scale of zero, which would flatten the instance to nothing.
    ZeroScale,
}

impl fmt::Display for PlacementError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NotFinite => "a position, rotation or scale is not a finite number",
            Self::ZeroScale => "a scale of 0 would flatten the instance to nothing",
        })
    }
}

impl std::error::Error for PlacementError {}

/// The transform that puts an instance's registration point at (`x`, `y`), turned `rotate`
/// degrees clockwise and scaled by `scale_x` and `scale_y` (negative mirrors) about that point.
pub fn placement(
    x: f64,
    y: f64,
    rotate: f64,
    scale_x: f64,
    scale_y: f64,
) -> Result<Transform2D, PlacementError> {
    if ![x, y, rotate, scale_x, scale_y]
        .iter()
        .all(|v| v.is_finite())
    {
        return Err(PlacementError::NotFinite);
    }
    if scale_x == 0.0 || scale_y == 0.0 {
        return Err(PlacementError::ZeroScale);
    }
    // The stage's y runs down, so a positive angle turns clockwise on screen.
    let matrix = DAffine2::from_scale_angle_translation(
        DVec2::new(scale_x, scale_y),
        rotate.to_radians(),
        DVec2::new(x, y),
    );
    Transform2D::new(matrix).map_err(|_| PlacementError::NotFinite)
}
