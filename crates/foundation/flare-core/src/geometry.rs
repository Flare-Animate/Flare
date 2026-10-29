//! Document-space geometry primitives.

use glamx::DVec2;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn points_reject_nan_and_infinity() {
        assert!(DocumentPoint::new(f64::NAN, 0.0).is_err());
        assert!(DocumentPoint::new(0.0, f64::INFINITY).is_err());
        assert!(DocumentPoint::new(-42.5, 0.0).is_ok());
    }
}
