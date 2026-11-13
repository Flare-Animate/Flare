//! Frame rates and timestamps.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A positive, reduced rational number of frames per second.
///
/// 30000/1001 and 30/1 are intentionally different rates. Deserialization
/// goes through the same validation as normal construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "FrameRateWire")]
pub struct FrameRate {
    numerator: u32,
    denominator: u32,
}

#[derive(Deserialize)]
struct FrameRateWire {
    numerator: u32,
    denominator: u32,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RateError {
    #[error("frame-rate numerator and denominator must both be positive")]
    Zero,
    #[error("frame timestamp exceeds the supported integer range")]
    TimestampOverflow,
    #[error("a frame rate is from 0.01 to 240 frames per second")]
    OutOfRange,
}

impl FrameRate {
    /// The slowest rate a person can type: one frame every 100 seconds.
    pub const MIN_PER_SECOND: f64 = 0.01;
    /// The fastest, as Animate allows.
    pub const MAX_PER_SECOND: f64 = 240.0;

    /// The rate of `fps` frames a second, as a person types it (24, 29.97, 12.5), kept to a
    /// thousandth of a frame. Refused outside [`Self::MIN_PER_SECOND`] to
    /// [`Self::MAX_PER_SECOND`], or when it is not a number.
    pub fn from_per_second(fps: f64) -> Result<Self, RateError> {
        if !fps.is_finite() || !(Self::MIN_PER_SECOND..=Self::MAX_PER_SECOND).contains(&fps) {
            return Err(RateError::OutOfRange);
        }
        Self::new((fps * 1000.0).round() as u32, 1000)
    }
}

impl FrameRate {
    pub fn new(numerator: u32, denominator: u32) -> Result<Self, RateError> {
        if numerator == 0 || denominator == 0 {
            return Err(RateError::Zero);
        }
        let divisor = gcd(numerator, denominator);
        Ok(Self {
            numerator: numerator / divisor,
            denominator: denominator / divisor,
        })
    }

    pub const fn numerator(self) -> u32 {
        self.numerator
    }

    pub const fn denominator(self) -> u32 {
        self.denominator
    }

    /// Frames per second, as a number.
    pub fn per_second(self) -> f64 {
        f64::from(self.numerator) / f64::from(self.denominator)
    }

    /// Timestamp of a zero-based frame boundary, rounded down to nanoseconds.
    ///
    /// This is a boundary conversion, not the engine's canonical time type.
    /// Never accumulate these rounded values to advance an animation clock.
    pub fn frame_start_ns(self, frame: u64) -> Result<u64, RateError> {
        let scaled = u128::from(frame)
            .checked_mul(u128::from(self.denominator))
            .and_then(|n| n.checked_mul(1_000_000_000))
            .ok_or(RateError::TimestampOverflow)?;
        u64::try_from(scaled / u128::from(self.numerator)).map_err(|_| RateError::TimestampOverflow)
    }
}

impl TryFrom<FrameRateWire> for FrameRate {
    type Error = RateError;

    fn try_from(value: FrameRateWire) -> Result<Self, Self::Error> {
        Self::new(value.numerator, value.denominator)
    }
}

const fn gcd(mut left: u32, mut right: u32) -> u32 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rate_is_a_number_of_frames_per_second() -> Result<(), RateError> {
        assert_eq!(FrameRate::new(24, 1)?.per_second(), 24.0);
        assert_eq!(FrameRate::new(48, 2)?.per_second(), 24.0);
        assert!((FrameRate::new(30_000, 1_001)?.per_second() - 29.97).abs() < 0.001);
        Ok(())
    }

    #[test]
    fn rates_are_reduced_without_rounding() -> Result<(), RateError> {
        assert_eq!(FrameRate::new(60, 2)?, FrameRate::new(30, 1)?);
        assert_ne!(FrameRate::new(30_000, 1_001)?, FrameRate::new(30, 1)?);
        assert_eq!(
            FrameRate::new(30_000, 1_001)?.frame_start_ns(30_000)?,
            1_001_000_000_000
        );
        Ok(())
    }

    #[test]
    fn invalid_rates_and_overflow_fail() -> Result<(), RateError> {
        assert_eq!(FrameRate::new(0, 1), Err(RateError::Zero));
        assert_eq!(FrameRate::new(1, 0), Err(RateError::Zero));
        assert_eq!(
            FrameRate::new(1, 1)?.frame_start_ns(u64::MAX),
            Err(RateError::TimestampOverflow)
        );
        Ok(())
    }

    #[test]
    fn a_typed_rate_is_kept_to_a_thousandth_and_kept_in_range() -> Result<(), RateError> {
        assert_eq!(FrameRate::from_per_second(24.0)?, FrameRate::new(24, 1)?);
        assert_eq!(FrameRate::from_per_second(12.5)?, FrameRate::new(25, 2)?);
        assert_eq!(
            FrameRate::from_per_second(29.97)?,
            FrameRate::new(2997, 100)?
        );
        assert_eq!(FrameRate::from_per_second(0.01)?, FrameRate::new(1, 100)?);
        assert_eq!(FrameRate::from_per_second(240.0)?, FrameRate::new(240, 1)?);
        for bad in [0.0, -24.0, 0.009, 240.5, f64::NAN, f64::INFINITY] {
            assert_eq!(
                FrameRate::from_per_second(bad),
                Err(RateError::OutOfRange),
                "{bad}"
            );
        }
        Ok(())
    }

    #[test]
    fn deserialize_cannot_bypass_rate_validation() -> Result<(), serde_json::Error> {
        assert!(serde_json::from_str::<FrameRate>(r#"{"numerator":30,"denominator":0}"#).is_err());
        let value: FrameRate = serde_json::from_str(r#"{"numerator":60,"denominator":2}"#)?;
        assert_eq!(value.numerator(), 30);
        assert_eq!(value.denominator(), 1);
        Ok(())
    }
}
