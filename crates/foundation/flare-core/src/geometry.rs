//! Document-space geometry primitives.

use glamx::{DAffine2, DVec2};
use thiserror::Error;

/// Finite document-space point. Screen coordinates should use a different type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DocumentPoint(DVec2);

#[derive(Debug, Error, PartialEq, Eq)]
#[error("document coordinates must be finite")]
pub struct NonFiniteCoordinate;

impl DocumentPoint {
    pub fn new(x: f64, y: f64) -> Result<Self, NonFiniteCoordinate> {
        if !x.is_finite() || !y.is_finite() {
            return Err(NonFiniteCoordinate);
        }
        Ok(Self(DVec2::new(x, y)))
    }

    pub const fn as_vec(self) -> DVec2 {
        self.0
    }
}

/// Full 2D affine matrix with finite entries. Shear, nonuniform scale, reflection and
/// singular matrices are all allowed; decomposition is a separate, lossy view.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform2D(DAffine2);

impl Transform2D {
    pub const IDENTITY: Self = Self(DAffine2::IDENTITY);

    pub fn new(matrix: DAffine2) -> Result<Self, NonFiniteCoordinate> {
        if !matrix.is_finite() {
            return Err(NonFiniteCoordinate);
        }
        Ok(Self(matrix))
    }

    pub const fn as_affine(self) -> DAffine2 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn points_reject_nan_and_infinity() {
        assert!(DocumentPoint::new(f64::NAN, 0.0).is_err());
        assert!(DocumentPoint::new(0.0, f64::INFINITY).is_err());
        assert!(DocumentPoint::new(-42.5, 0.0).is_ok());
    }

    #[test]
    fn transforms_reject_nonfinite_but_allow_singular() {
        assert!(Transform2D::new(DAffine2::from_scale(DVec2::new(0.0, 1.0))).is_ok());
        assert!(Transform2D::new(DAffine2::from_translation(DVec2::new(f64::NAN, 0.0))).is_err());
    }
}
