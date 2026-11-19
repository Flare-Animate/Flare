//! The smooth line through a stroke's kept points, and its round ends, cut into straight pieces
//! fine enough to stay smooth however far the stage is zoomed in.
//!
//! The centre follows a centripetal Catmull-Rom spline. The width follows a monotone cubic
//! through the kept widths: blended in a straight ramp between points, as it once was, every
//! change of pressure bent the edges at a corner, and a cubic that is not monotone would bulge
//! past the widths it joins. Each span between two kept points gets as many pieces as it turns
//! and changes width, and each end as many as its radius needs, so a long sweep is round and a
//! short straight run costs little.

use glamx::DVec2;

use super::{Nib, Spine};

/// The most a piece of the centre line turns, in radians (about two degrees).
const MAX_TURN: f64 = 0.035;
/// The most the half width changes over one piece, in stage units.
const MAX_WIDTH_STEP: f64 = 0.25;
/// The shortest piece worth cutting, in stage units: a span shorter than a few of these (a hand
/// that shook between samples a pixel apart) is not cut finer than [`MIN_STEPS`], or the outline
/// would trace every tremor.
const MIN_PIECE: f64 = 0.25;
/// The fewest and the most pieces between two kept points.
pub(super) const MIN_STEPS: usize = 4;
const MAX_STEPS: usize = 64;
/// The most points the smoothed centre line may have; past it each span gets fewer pieces.
const MAX_LINE_POINTS: usize = 200_000;
/// How far, in stage units, a piece of a round end may stray inside the true circle.
const CAP_SAG: f64 = 0.02;
/// The fewest and the most pieces in a round end.
const MIN_CAP_STEPS: usize = 8;
const MAX_CAP_STEPS: usize = 96;

/// The angle between two directions, 0 when either is nothing.
fn turn(a: DVec2, b: DVec2) -> f64 {
    if a.length_squared() <= f64::EPSILON || b.length_squared() <= f64::EPSILON {
        return 0.0;
    }
    a.angle_to(b).abs()
}

/// The most a stroke's end is carried on turning past its last point, in radians.
const MAX_END_TURN: f64 = std::f64::consts::FRAC_PI_2;

/// The point past `end` the line would reach after a step like the one from `before` to `end`,
/// turned as the line turned from `earlier` to `before` to `end`: an end carried on straight
/// leaves the last span of a curve flat, so it keeps turning as it was.
fn carried_on(earlier: Option<DVec2>, before: DVec2, end: DVec2) -> DVec2 {
    let step = end - before;
    let Some(earlier) = earlier else {
        return end + step;
    };
    let bend = (before - earlier).angle_to(step);
    let bend = if bend.is_finite() {
        bend.clamp(-MAX_END_TURN, MAX_END_TURN)
    } else {
        0.0
    };
    end + DVec2::from_angle(bend).rotate(step)
}

/// The four points the span from `spine[i]` to `spine[i + 1]` is drawn with; past the ends,
/// the line carries on as it was turning.
fn controls(spine: &[Spine], i: usize) -> [DVec2; 4] {
    let last = spine.len() - 1;
    let (p1, p2) = (spine[i].at, spine[i + 1].at);
    let p0 = if i == 0 {
        carried_on(spine.get(2).map(|s| s.at), p2, p1)
    } else {
        spine[i - 1].at
    };
    let p3 = if i + 1 == last {
        carried_on(i.checked_sub(1).map(|j| spine[j].at), p1, p2)
    } else {
        spine[i + 2].at
    };
    [p0, p1, p2, p3]
}

/// How many points along a span are looked at to see how sharply it bends.
const PROBES: usize = 8;

/// The sharpest turn between neighbouring eighths of the span from `p1` to `p2`. The turns at
/// its ends cannot be judged from the points alone: past the ends of the stroke the line is
/// carried on straight, and the whole bend then falls inside the first and last spans.
fn sharpest_bend(p0: DVec2, p1: DVec2, p2: DVec2, p3: DVec2) -> f64 {
    let probe: Vec<DVec2> = (0..=PROBES)
        .map(|k| centripetal(p0, p1, p2, p3, k as f64 / PROBES as f64))
        .collect();
    probe
        .windows(3)
        .map(|w| turn(w[1] - w[0], w[2] - w[1]))
        .fold(0.0, f64::max)
}

/// How many pieces each span needs to stay within [`MAX_TURN`] and [`MAX_WIDTH_STEP`], within
/// the bounds and, all together, within [`MAX_LINE_POINTS`].
fn steps(spine: &[Spine]) -> Vec<usize> {
    let wanted: Vec<usize> = (0..spine.len() - 1)
        .map(|i| {
            let [p0, p1, p2, p3] = controls(spine, i);
            let bend = sharpest_bend(p0, p1, p2, p3);
            let by_turn = (bend * PROBES as f64 / MAX_TURN).ceil();
            let by_width = ((spine[i + 1].half - spine[i].half).abs() / MAX_WIDTH_STEP).ceil();
            let by_length = ((p2 - p1).length() / MIN_PIECE).ceil();
            let n = by_turn.max(by_width).min(by_length);
            if n.is_finite() {
                (n as usize).clamp(MIN_STEPS, MAX_STEPS)
            } else {
                MIN_STEPS
            }
        })
        .collect();
    let total: usize = wanted.iter().sum();
    if total <= MAX_LINE_POINTS {
        return wanted;
    }
    // A very long stroke: share the points out in proportion, at least one piece a span.
    wanted
        .into_iter()
        .map(|n| (n * MAX_LINE_POINTS / total).max(1))
        .collect()
}

/// The slope of the width at each kept point, in half-width per stage unit along the chord,
/// chosen so the cubic between two points never leaves the range of their widths
/// (Fritsch and Butland's monotone slopes, for unevenly spaced points).
fn width_slopes(spine: &[Spine], lengths: &[f64]) -> Vec<f64> {
    let n = spine.len();
    let delta: Vec<f64> = (0..n - 1)
        .map(|i| {
            if lengths[i] > f64::EPSILON {
                (spine[i + 1].half - spine[i].half) / lengths[i]
            } else {
                0.0
            }
        })
        .collect();
    (0..n)
        .map(|i| {
            if i == 0 {
                return delta[0];
            }
            if i == n - 1 {
                return delta[n - 2];
            }
            let (d0, d1) = (delta[i - 1], delta[i]);
            if d0 * d1 <= 0.0 {
                return 0.0;
            }
            let (h0, h1) = (lengths[i - 1], lengths[i]);
            3.0 * (h0 + h1) / ((2.0 * h1 + h0) / d0 + (h1 + 2.0 * h0) / d1)
        })
        .collect()
}

/// Points along the centripetal Catmull-Rom spline through `spine`, with the widths following a
/// monotone cubic through theirs.
///
/// The centripetal form spaces its knots by the square root of the distance between points.
/// Hand-drawn points are rarely evenly spaced (a stabilizer, for one, starts a stroke with tiny
/// steps), and the uniform form overshoots where a short step meets a long one, looping back
/// past the start of a line; the centripetal form never makes a cusp or a loop inside a segment.
pub(super) fn smooth(spine: &[Spine]) -> Vec<Spine> {
    if spine.len() < 2 {
        return spine.to_vec();
    }
    let lengths: Vec<f64> = spine
        .windows(2)
        .map(|w| (w[1].at - w[0].at).length())
        .collect();
    let slopes = width_slopes(spine, &lengths);
    let steps = steps(spine);
    let mut out = Vec::with_capacity(steps.iter().sum::<usize>() + 1);
    out.push(spine[0]);
    for (i, count) in steps.into_iter().enumerate() {
        let [p0, p1, p2, p3] = controls(spine, i);
        let (h1, h2) = (spine[i].half, spine[i + 1].half);
        let (m1, m2) = (slopes[i] * lengths[i], slopes[i + 1] * lengths[i]);
        for s in 1..=count {
            let t = s as f64 / count as f64;
            let at = centripetal(p0, p1, p2, p3, t);
            let (t2, t3) = (t * t, t * t * t);
            let half = (2.0 * t3 - 3.0 * t2 + 1.0) * h1
                + (t3 - 2.0 * t2 + t) * m1
                + (-2.0 * t3 + 3.0 * t2) * h2
                + (t3 - t2) * m2;
            // Monotone slopes keep it between h1 and h2; rounding must not take it below zero.
            out.push(Spine {
                at,
                half: half.max(0.0),
            });
        }
    }
    out
}

/// The point `t` (0 to 1) of the way from `p1` to `p2` on the centripetal Catmull-Rom spline
/// through the four points (Barry and Goldman's pyramid).
fn centripetal(p0: DVec2, p1: DVec2, p2: DVec2, p3: DVec2, t: f64) -> DVec2 {
    let knot = |a: DVec2, b: DVec2| (b - a).length().sqrt().max(1e-9);
    let t1 = knot(p0, p1);
    let t2 = t1 + knot(p1, p2);
    let t3 = t2 + knot(p2, p3);
    let u = t1 + (t2 - t1) * t;
    let lerp = |a: DVec2, b: DVec2, from: f64, to: f64| {
        a * ((to - u) / (to - from)) + b * ((u - from) / (to - from))
    };
    let a1 = lerp(p0, p1, 0.0, t1);
    let a2 = lerp(p1, p2, t1, t2);
    let a3 = lerp(p2, p3, t2, t3);
    let b1 = lerp(a1, a2, 0.0, t2);
    let b2 = lerp(a2, a3, t1, t3);
    lerp(b1, b2, t1, t2)
}

/// How many pieces a round end of `radius` needs to stay within [`CAP_SAG`] of a true half
/// circle.
pub(super) fn cap_steps(radius: f64) -> usize {
    if !(radius.is_finite() && radius > CAP_SAG) {
        return MIN_CAP_STEPS;
    }
    let piece = 2.0 * (1.0 - CAP_SAG / radius).acos();
    let n = (std::f64::consts::PI / piece).ceil();
    if n.is_finite() {
        (n as usize).clamp(MIN_CAP_STEPS, MAX_CAP_STEPS)
    } else {
        MAX_CAP_STEPS
    }
}

/// The half circle around `centre` from the `normal` side to the opposite side, turning the way
/// the stroke is heading (without the two end points, which the sides already have).
pub(super) fn cap(centre: DVec2, normal: DVec2, radius: f64, nib: Nib) -> Vec<DVec2> {
    let heading = DVec2::new(normal.y, -normal.x);
    let steps = cap_steps(radius);
    (1..steps)
        .map(|k| {
            let angle = std::f64::consts::PI * k as f64 / steps as f64;
            centre + nib.support(normal * angle.cos() + heading * angle.sin()) * radius
        })
        .collect()
}
