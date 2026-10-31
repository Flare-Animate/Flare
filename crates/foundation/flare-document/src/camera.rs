//! The camera: which part of a scene the stage shows, and how it is turned and magnified.
//!
//! A camera layer holds keyframes of these, and a motion tween moves between them. With no
//! camera the stage shows the scene as drawn, which is the same as a camera in the middle of the
//! stage at zoom 1 and no rotation.

use flare_core::DocumentPoint;
use glamx::{DAffine2, DVec2};
use thiserror::Error;

use crate::Stage;

pub const MIN_ZOOM: f64 = 0.01;
pub const MAX_ZOOM: f64 = 100.0;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CameraError {
    #[error("a camera's zoom must be between {MIN_ZOOM} and {MAX_ZOOM}")]
    Zoom,
    #[error("a camera's rotation must be finite")]
    Rotation,
}

/// Where the camera looks: the point of the scene at the middle of the picture, how much it is
/// magnified (2 shows half as much, twice as large), and how far it is turned clockwise, in
/// radians. Turning the camera clockwise turns what it shows counterclockwise.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    center: DocumentPoint,
    zoom: f64,
    rotation: f64,
}

impl Camera {
    pub fn new(center: DocumentPoint, zoom: f64, rotation: f64) -> Result<Self, CameraError> {
        if !zoom.is_finite() || !(MIN_ZOOM..=MAX_ZOOM).contains(&zoom) {
            return Err(CameraError::Zoom);
        }
        if !rotation.is_finite() {
            return Err(CameraError::Rotation);
        }
        Ok(Self {
            center,
            zoom,
            rotation,
        })
    }

    /// The camera that shows the stage exactly as it is.
    pub fn centered(stage: &Stage) -> Self {
        Self {
            center: DocumentPoint::new(
                f64::from(stage.width()) / 2.0,
                f64::from(stage.height()) / 2.0,
            )
            .unwrap_or(DocumentPoint::ORIGIN),
            zoom: 1.0,
            rotation: 0.0,
        }
    }

    pub fn center(&self) -> DocumentPoint {
        self.center
    }

    pub fn zoom(&self) -> f64 {
        self.zoom
    }

    pub fn rotation(&self) -> f64 {
        self.rotation
    }

    /// What to apply to the scene so the picture is what this camera sees: the camera's point
    /// goes to the middle of the stage, then the scene is turned and magnified about it.
    pub fn view(&self, stage: &Stage) -> DAffine2 {
        let middle = DVec2::new(
            f64::from(stage.width()) / 2.0,
            f64::from(stage.height()) / 2.0,
        );
        DAffine2::from_translation(middle)
            * DAffine2::from_angle(-self.rotation)
            * DAffine2::from_scale(DVec2::splat(self.zoom))
            * DAffine2::from_translation(-self.center.as_vec())
    }

    /// `t` of the way from this camera to `other` (0 to 1): the point and the zoom move in a
    /// straight line and the rotation takes the shorter way round.
    pub fn lerp(&self, other: &Camera, t: f64) -> Camera {
        let t = t.clamp(0.0, 1.0);
        let tau = std::f64::consts::TAU;
        let mut turn = (other.rotation - self.rotation) % tau;
        if turn > std::f64::consts::PI {
            turn -= tau;
        } else if turn < -std::f64::consts::PI {
            turn += tau;
        }
        Camera {
            center: DocumentPoint::new(
                self.center.as_vec().x + (other.center.as_vec().x - self.center.as_vec().x) * t,
                self.center.as_vec().y + (other.center.as_vec().y - self.center.as_vec().y) * t,
            )
            .unwrap_or(self.center),
            zoom: (self.zoom + (other.zoom - self.zoom) * t).clamp(MIN_ZOOM, MAX_ZOOM),
            rotation: self.rotation + turn * t,
        }
    }
}

#[cfg(test)]
mod tests;
