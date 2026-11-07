//! Moving an instance's matrix by a motion object's values: the matrix is taken apart into
//! translation, rotation, shear and two signed scales, the changes are applied to those parts,
//! and the matrix is put back together. Written from the standard decomposition of a 2D affine
//! matrix; the parts are the same ones Flare's own motion tween interpolates.

use glamx::{DAffine2, DVec2};

/// `M = T(translation) * R(angle) * [[scale_x, shear], [0, scale_y]]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Parts {
    pub(super) translation: DVec2,
    pub(super) angle: f64,
    pub(super) scale_x: f64,
    pub(super) scale_y: f64,
    /// How far the second axis leans along the first, in the first axis's units.
    pub(super) shear: f64,
}

/// What a motion object adds to a matrix at one frame, relative to where it starts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Change {
    pub(super) dx: f64,
    pub(super) dy: f64,
    /// Radians added to the rotation.
    pub(super) angle: f64,
    /// Radians of horizontal skew added; it leans the second axis by `tan` of it.
    pub(super) skew_x: f64,
    /// Factors on the two scales.
    pub(super) scale_x: f64,
    pub(super) scale_y: f64,
}

impl Default for Change {
    fn default() -> Self {
        Self {
            dx: 0.0,
            dy: 0.0,
            angle: 0.0,
            skew_x: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
        }
    }
}

/// Splits a finite matrix. A matrix with a collapsed axis keeps the other one's direction.
pub(super) fn decompose(matrix: DAffine2) -> Option<Parts> {
    if !matrix.is_finite() {
        return None;
    }
    let (x, y) = (matrix.matrix2.x_axis, matrix.matrix2.y_axis);
    let length_x = x.length();
    let (angle, scale_x, shear, scale_y) = if length_x > 0.0 {
        let u = x / length_x;
        let v = DVec2::new(-u.y, u.x);
        (x.y.atan2(x.x), length_x, u.dot(y), v.dot(y))
    } else {
        let length_y = y.length();
        if length_y == 0.0 {
            (0.0, 0.0, 0.0, 0.0)
        } else {
            // The first axis has collapsed, so the rotation comes from the second.
            let v = y / length_y;
            (
                v.y.atan2(v.x) - std::f64::consts::FRAC_PI_2,
                0.0,
                0.0,
                length_y,
            )
        }
    };
    Some(Parts {
        translation: matrix.translation,
        angle,
        scale_x,
        scale_y,
        shear,
    })
}

impl Parts {
    pub(super) fn matrix(self) -> DAffine2 {
        let (s, c) = self.angle.sin_cos();
        let u = DVec2::new(c, s);
        let v = DVec2::new(-s, c);
        DAffine2::from_cols(
            u * self.scale_x,
            u * self.shear + v * self.scale_y,
            self.translation,
        )
    }
}

/// The matrix `base` changed by `change`, the rotation and scale turning about `pivot` (the
/// instance's transformation point, in its own coordinates). At the start of a motion object the
/// change is the identity, so the result is `base` again (up to rounding).
pub(super) fn apply(base: DAffine2, change: Change, pivot: DVec2) -> Option<DAffine2> {
    let parts = decompose(base)?;
    let moved = Parts {
        translation: DVec2::ZERO,
        angle: parts.angle + change.angle,
        scale_x: parts.scale_x * change.scale_x,
        scale_y: parts.scale_y * change.scale_y,
        // A horizontal skew leans the second axis by its own height times the tangent.
        shear: parts.shear + change.skew_x.tan() * parts.scale_y,
    }
    .matrix();
    // The pivot's place in the parent moves by the offset, and nothing else keeps its place.
    let translation = base.translation + base.matrix2 * pivot - moved.matrix2 * pivot
        + DVec2::new(change.dx, change.dy);
    let matrix = DAffine2 {
        translation,
        ..moved
    };
    matrix.is_finite().then_some(matrix)
}
