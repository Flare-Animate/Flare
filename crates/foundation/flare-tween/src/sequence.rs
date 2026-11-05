//! In-betweens of stroke points, the way a grease pencil interpolates a drawing between two
//! keyframes: points are matched in order, and a stroke with a different number of points is first
//! resampled by arc length.

use crate::TweenError;
use crate::easing::{Easing, Interpolation, Params, ease};

/// Points per stroke. Resampling and matching are linear in this, so it bounds the work.
pub const MAX_POINTS: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StrokePoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SequenceOptions {
    /// How far from the first stroke to the second, 0 to 1.
    pub factor: f64,
    pub interpolation: Interpolation,
    pub easing: Easing,
    pub params: Params,
    /// Match the second stroke's points in reverse order, so it is drawn the other way round.
    pub flip: bool,
}

impl Default for SequenceOptions {
    fn default() -> Self {
        Self {
            factor: 0.5,
            interpolation: Interpolation::Linear,
            easing: Easing::Auto,
            params: Params::default(),
            flip: false,
        }
    }
}

/// The in-between stroke at `options.factor`.
///
/// With equal point counts each point moves straight to its partner, eased. Otherwise both strokes
/// are resampled to the larger count, evenly by arc length, and then matched. At factor 0 the
/// result is the first stroke (resampled, if the counts differ) and at factor 1 the second, flipped
/// if `flip` is set.
///
/// # Errors
/// A stroke with no points or more than `MAX_POINTS`, a coordinate or the factor that is not finite,
/// a factor outside 0..=1, invalid parameters, or an in-between that overflows.
pub fn interpolate_sequence(
    from: &[StrokePoint],
    to: &[StrokePoint],
    options: &SequenceOptions,
) -> Result<Vec<StrokePoint>, TweenError> {
    check_count(from.len())?;
    check_count(to.len())?;
    for point in from.iter().chain(to) {
        if !point.x.is_finite() || !point.y.is_finite() {
            return Err(TweenError::NotFinite {
                what: "stroke point",
            });
        }
    }
    let e = ease(
        options.interpolation,
        options.easing,
        options.factor,
        &options.params,
    )?;
    if !(0.0..=1.0).contains(&options.factor) {
        return Err(TweenError::OutOfRange {
            what: "factor",
            min: 0.0,
            max: 1.0,
            value: options.factor,
        });
    }
    let flipped: Vec<StrokePoint>;
    let target: &[StrokePoint] = if options.flip {
        flipped = to.iter().rev().copied().collect();
        &flipped
    } else {
        to
    };
    let n = from.len().max(target.len());
    let start = resample(from, n);
    let end = resample(target, n);
    let out: Vec<StrokePoint> = start
        .iter()
        .zip(&end)
        .map(|(a, b)| StrokePoint {
            x: a.x * (1.0 - e) + b.x * e,
            y: a.y * (1.0 - e) + b.y * e,
        })
        .collect();
    if out.iter().all(|p| p.x.is_finite() && p.y.is_finite()) {
        Ok(out)
    } else {
        Err(TweenError::NonFiniteResult)
    }
}

fn check_count(count: usize) -> Result<(), TweenError> {
    if count == 0 || count > MAX_POINTS {
        Err(TweenError::PointCount {
            count,
            max: MAX_POINTS,
        })
    } else {
        Ok(())
    }
}

/// `n` points spaced evenly by arc length along the polyline. A stroke with no length (every point
/// in one place) stays where it is.
fn resample(points: &[StrokePoint], n: usize) -> Vec<StrokePoint> {
    if points.len() == n {
        return points.to_vec();
    }
    let Some(&first) = points.first() else {
        return Vec::new();
    };
    if n == 1 || points.len() == 1 {
        return vec![first; n];
    }
    let mut cumulative = Vec::with_capacity(points.len());
    cumulative.push(0.0_f64);
    for pair in points.windows(2) {
        let last = cumulative.last().copied().unwrap_or(0.0);
        cumulative.push(last + (pair[1].x - pair[0].x).hypot(pair[1].y - pair[0].y));
    }
    let total = cumulative.last().copied().unwrap_or(0.0);
    if total <= 0.0 {
        return vec![first; n];
    }
    let mut out = Vec::with_capacity(n);
    let mut seg = 0_usize;
    for k in 0..n {
        let target = total * k as f64 / (n - 1) as f64;
        while seg + 2 < points.len() && cumulative[seg + 1] < target {
            seg += 1;
        }
        let (a, b) = (points[seg], points[seg + 1]);
        let span = cumulative[seg + 1] - cumulative[seg];
        let f = if span > 0.0 {
            ((target - cumulative[seg]) / span).clamp(0.0, 1.0)
        } else {
            0.0
        };
        out.push(StrokePoint {
            x: a.x + (b.x - a.x) * f,
            y: a.y + (b.y - a.y) * f,
        });
    }
    out
}
