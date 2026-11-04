//! Tweening for 2D animation: interpolation families and easing, cubic-bezier keyframe handles,
//! extrapolation, a curve over keyframes, and grease-pencil-style in-betweens for stroke points.
//!
//! Clean-room. Each behaviour follows the documented behaviour of the feature it names (what an
//! interpolation or easing does, what a handle type means, how an extrapolation mode repeats a
//! range, how the in-betweens of stroke points are made) together with standard easing formulas.
//! No Blender source was read or copied; see docs/ARCHITECTURE.md, "Tweening".
#![forbid(unsafe_code)]

mod bezier;
mod curve;
mod easing;
mod sequence;
#[cfg(test)]
mod tests;

pub use curve::{Curve, Extrapolation, Handle, HandleKind, Key, MAX_KEYS};
pub use easing::{
    Easing, Interpolation, MAX_BACK_OVERSHOOT, MAX_ELASTIC_AMPLITUDE, MAX_ELASTIC_PERIOD,
    MIN_ELASTIC_PERIOD, Params, ease,
};
pub use sequence::{MAX_POINTS, SequenceOptions, StrokePoint, interpolate_sequence};

/// Why a value, curve or sequence was refused. A refused input changes nothing.
#[derive(Debug, Clone, Copy, PartialEq, thiserror::Error)]
pub enum TweenError {
    #[error("{what} is not a finite number")]
    NotFinite { what: &'static str },
    #[error("{what} must be between {min} and {max}, not {value}")]
    OutOfRange {
        what: &'static str,
        min: f64,
        max: f64,
        value: f64,
    },
    #[error("a curve needs between 1 and {max} keys, not {count}")]
    KeyCount { count: usize, max: usize },
    #[error("two keys share the time {time}")]
    DuplicateTime { time: f64 },
    #[error("a stroke needs between 1 and {max} points, not {count}")]
    PointCount { count: usize, max: usize },
    #[error("the interpolated value is not finite")]
    NonFiniteResult,
}
