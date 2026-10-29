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
    fn deserialize_cannot_bypass_rate_validation() -> Result<(), serde_json::Error> {
        assert!(serde_json::from_str::<FrameRate>(r#"{"numerator":30,"denominator":0}"#).is_err());
        let value: FrameRate = serde_json::from_str(r#"{"numerator":60,"denominator":2}"#)?;
        assert_eq!(value.numerator(), 30);
        assert_eq!(value.denominator(), 1);
        Ok(())
    }
}
