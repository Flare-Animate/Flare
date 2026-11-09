//! Interpolation families and easing: how far a value has moved along its segment at a given
//! fraction of the segment's time.

use std::f64::consts::{FRAC_PI_2, TAU};

use crate::TweenError;

/// Limits on the parameters, so a bad value is refused instead of making an overshoot without end.
pub const MAX_BACK_OVERSHOOT: f64 = 10.0;
pub const MAX_ELASTIC_AMPLITUDE: f64 = 10.0;
pub const MAX_ELASTIC_PERIOD: f64 = 10.0;
/// Below this period the wobble is faster than a frame and the rounding of the sine dominates.
pub const MIN_ELASTIC_PERIOD: f64 = 0.01;

/// The shape of a segment between two keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Interpolation {
    /// Holds the first value until the next key.
    Constant,
    Linear,
    /// The shape comes from the handles of the key the segment starts at.
    Bezier,
    Sine,
    Quad,
    Cubic,
    Quart,
    Quint,
    Expo,
    Circ,
    Back,
    Bounce,
    Elastic,
}

impl Interpolation {
    pub const ALL: [Self; 13] = [
        Self::Constant,
        Self::Linear,
        Self::Bezier,
        Self::Sine,
        Self::Quad,
        Self::Cubic,
        Self::Quart,
        Self::Quint,
        Self::Expo,
        Self::Circ,
        Self::Back,
        Self::Bounce,
        Self::Elastic,
    ];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Constant => "constant",
            Self::Linear => "linear",
            Self::Bezier => "bezier",
            Self::Sine => "sine",
            Self::Quad => "quad",
            Self::Cubic => "cubic",
            Self::Quart => "quart",
            Self::Quint => "quint",
            Self::Expo => "expo",
            Self::Circ => "circ",
            Self::Back => "back",
            Self::Bounce => "bounce",
            Self::Elastic => "elastic",
        }
    }

    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.name() == name)
    }
}

/// Which end of a segment the easing bends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Easing {
    /// Starts slowly and speeds up.
    In,
    /// Starts fast and slows down.
    Out,
    InOut,
    /// Smooth at both ends, which is what InOut gives.
    Auto,
}

impl Easing {
    pub const ALL: [Self; 4] = [Self::In, Self::Out, Self::InOut, Self::Auto];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::In => "in",
            Self::Out => "out",
            Self::InOut => "in-out",
            Self::Auto => "auto",
        }
    }

    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|easing| easing.name() == name)
    }
}

/// The parameters of the Back and Elastic families. Each is checked by `validate`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Params {
    /// How far Back goes past its target before it comes back; 0 is no overshoot.
    pub back_overshoot: f64,
    /// Elastic's peak swing; at least 1.
    pub elastic_amplitude: f64,
    /// Elastic's period, as a fraction of the segment.
    pub elastic_period: f64,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            back_overshoot: 1.70158,
            elastic_amplitude: 1.0,
            elastic_period: 0.3,
        }
    }
}

impl Params {
    /// # Errors
    /// A parameter that is not finite or is outside its limits.
    pub fn validate(&self) -> Result<(), TweenError> {
        check_range(
            "back overshoot",
            self.back_overshoot,
            0.0,
            MAX_BACK_OVERSHOOT,
        )?;
        check_range(
            "elastic amplitude",
            self.elastic_amplitude,
            1.0,
            MAX_ELASTIC_AMPLITUDE,
        )?;
        check_range(
            "elastic period",
            self.elastic_period,
            MIN_ELASTIC_PERIOD,
            MAX_ELASTIC_PERIOD,
        )
    }
}

pub(crate) fn check_range(
    what: &'static str,
    value: f64,
    min: f64,
    max: f64,
) -> Result<(), TweenError> {
    if !value.is_finite() {
        return Err(TweenError::NotFinite { what });
    }
    if value < min || value > max {
        return Err(TweenError::OutOfRange {
            what,
            min,
            max,
            value,
        });
    }
    Ok(())
}

/// The eased fraction for `t` (clamped to 0..=1) of a segment.
///
/// `Constant` steps at the end, `Linear` and `Bezier` return `t` (a Bezier's shape is in its
/// key's handles, see `Curve`), and the other families take `easing`. Every family returns
/// exactly 0 at 0 and exactly 1 at 1.
///
/// # Errors
/// `t` is not finite, or `params` is outside its limits.
pub fn ease(
    interpolation: Interpolation,
    easing: Easing,
    t: f64,
    params: &Params,
) -> Result<f64, TweenError> {
    params.validate()?;
    if !t.is_finite() {
        return Err(TweenError::NotFinite { what: "time" });
    }
    Ok(ease_valid(interpolation, easing, t.clamp(0.0, 1.0), params))
}

/// The f32 form of `ease`: the same formulas, run once at f64 precision and rounded to f32.
/// Rounding once keeps the two in agreement to f32 precision, so a caller that stores f32 gets
/// every family without a second implementation to drift from this one.
///
/// # Errors
/// As `ease`: `t` is not finite, or `params` is outside its limits.
pub fn ease32(
    interpolation: Interpolation,
    easing: Easing,
    t: f32,
    params: &Params,
) -> Result<f32, TweenError> {
    ease(interpolation, easing, f64::from(t), params).map(|value| value as f32)
}

/// `ease` for input that is already known to be finite, in 0..=1, with valid parameters.
pub(crate) fn ease_valid(
    interpolation: Interpolation,
    easing: Easing,
    t: f64,
    params: &Params,
) -> f64 {
    match interpolation {
        Interpolation::Constant => {
            if t >= 1.0 {
                1.0
            } else {
                0.0
            }
        }
        Interpolation::Linear | Interpolation::Bezier => t,
        family => match easing {
            Easing::In => family_in(family, t, params),
            Easing::Out => 1.0 - family_in(family, 1.0 - t, params),
            Easing::InOut | Easing::Auto => {
                if t < 0.5 {
                    family_in(family, 2.0 * t, params) / 2.0
                } else {
                    1.0 - family_in(family, 2.0 - 2.0 * t, params) / 2.0
                }
            }
        },
    }
}

/// The "in" half of each family. The ends are pinned to exactly 0 and 1 because some formulas
/// (Expo, Elastic) are a little off there. The "out" and in-out forms are derived from this one by
/// reflection, so each family's endpoints hold for every easing.
fn family_in(kind: Interpolation, t: f64, params: &Params) -> f64 {
    if t <= 0.0 {
        return 0.0;
    }
    if t >= 1.0 {
        return 1.0;
    }
    match kind {
        Interpolation::Sine => 1.0 - (t * FRAC_PI_2).cos(),
        Interpolation::Quad => t * t,
        Interpolation::Cubic => t * t * t,
        Interpolation::Quart => t.powi(4),
        Interpolation::Quint => t.powi(5),
        Interpolation::Expo => 2f64.powf(10.0 * (t - 1.0)),
        Interpolation::Circ => 1.0 - (1.0 - t * t).max(0.0).sqrt(),
        Interpolation::Back => {
            let c = params.back_overshoot;
            (c + 1.0) * t * t * t - c * t * t
        }
        Interpolation::Bounce => 1.0 - bounce_out(1.0 - t),
        Interpolation::Elastic => elastic_in(t, params),
        Interpolation::Constant | Interpolation::Linear | Interpolation::Bezier => t,
    }
}

/// The standard bounce: four parabolas, each landing on 1 and rising less far.
fn bounce_out(t: f64) -> f64 {
    const N: f64 = 7.5625;
    const D: f64 = 2.75;
    if t < 1.0 / D {
        N * t * t
    } else if t < 2.0 / D {
        let t = t - 1.5 / D;
        N * t * t + 0.75
    } else if t < 2.5 / D {
        let t = t - 2.25 / D;
        N * t * t + 0.9375
    } else {
        let t = t - 2.625 / D;
        N * t * t + 0.984_375
    }
}

/// A decaying sine that starts at 0 and reaches 1 with amplitude `a`. The phase shift is chosen so
/// the first crossing lands exactly on 0.
fn elastic_in(t: f64, params: &Params) -> f64 {
    let amplitude = params.elastic_amplitude;
    let period = params.elastic_period;
    let shift = period / TAU * (1.0 / amplitude).asin();
    -amplitude * 2f64.powf(10.0 * (t - 1.0)) * ((t - 1.0 - shift) * TAU / period).sin()
}
