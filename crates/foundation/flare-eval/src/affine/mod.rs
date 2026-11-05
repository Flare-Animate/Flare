//! Interpolating any 2D affine matrix, as Animate does for a motion tween: the matrix is split
//! into translation, rotation, a horizontal shear and two signed scales, and each part is
//! interpolated on its own.

use std::f64::consts::{PI, TAU};

use glamx::DAffine2;
use glamx::DVec2;

/// `M = T(translation) * R(angle) * [[scale_x, skew], [0, scale_y]]`. A reflection is a negative
/// `scale_y`, so the handedness of a matrix is the sign of one number.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Parts {
    pub translation: DVec2,
    pub angle: f64,
    pub scale_x: f64,
    pub scale_y: f64,
    /// How far the second axis leans along the first, in the first axis's units.
    pub skew: f64,
}

/// Splits `matrix`; `None` only when it holds a NaN or infinity.
pub(crate) fn decompose(matrix: DAffine2) -> Option<Parts> {
    if !matrix.is_finite() {
        return None;
    }
    let (x, y) = (matrix.matrix2.x_axis, matrix.matrix2.y_axis);
    let scale_x = x.length();
    let (angle, scale_x, skew, scale_y) = if scale_x > 0.0 {
        let u = x / scale_x;
        let v = DVec2::new(-u.y, u.x);
        (x.y.atan2(x.x), scale_x, u.dot(y), v.dot(y))
    } else if y.length() > 0.0 {
        // The first axis collapsed, so the direction comes from the second.
        let v = y / y.length();
        (v.y.atan2(v.x) - PI / 2.0, 0.0, 0.0, y.length())
    } else {
        (0.0, 0.0, 0.0, 0.0)
    };
    Some(Parts {
        translation: matrix.translation,
        angle,
        scale_x,
        scale_y,
        skew,
    })
}

impl Parts {
    pub(crate) fn matrix(self) -> DAffine2 {
        let (s, c) = self.angle.sin_cos();
        let u = DVec2::new(c, s);
        let v = DVec2::new(-s, c);
        DAffine2::from_cols(
            u * self.scale_x,
            u * self.skew + v * self.scale_y,
            self.translation,
        )
    }
}

/// The signed turn from `a` to `b` by the shorter way round.
pub(crate) fn shortest_turn(a: f64, b: f64) -> f64 {
    let d = (b - a) % TAU;
    if d > PI {
        d - TAU
    } else if d < -PI {
        d + TAU
    } else {
        d
    }
}

/// The matrix `t` of the way from `from` to `to`, with the registration point at `position`
/// when given. The ends are returned exactly, and so is any `t` between equal matrices.
pub(crate) fn interpolate(
    from: DAffine2,
    to: DAffine2,
    t: f64,
    position: Option<DVec2>,
) -> Option<DAffine2> {
    let (a, b) = (decompose(from)?, decompose(to)?);
    if !t.is_finite() {
        return None;
    }
    if position.is_none() && (from == to || t == 0.0) {
        return Some(from);
    }
    if position.is_none() && t == 1.0 {
        return Some(to);
    }
    let mix = |p: f64, q: f64| p + (q - p) * t;
    let parts = Parts {
        translation: position.unwrap_or_else(|| a.translation.lerp(b.translation, t)),
        angle: a.angle + shortest_turn(a.angle, b.angle) * t,
        scale_x: mix(a.scale_x, b.scale_x),
        scale_y: mix(a.scale_y, b.scale_y),
        skew: mix(a.skew, b.skew),
    };
    Some(parts.matrix())
}

#[cfg(test)]
mod tests;
