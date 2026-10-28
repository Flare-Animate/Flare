//! Port of `tnzext/NotSymmetricBezierPotential.cpp`: the arithmetic of
//! `ToonzExt::NotSymmetricBezierPotential` (see `LinearPotential.rs` for the split with the
//! header). `myBlendFunc` evaluates the y of the quadratic Bezier (0,1) (0.5(1-t),1) (1,0)
//! exactly as `TQuadratic::getPoint` does: `p0*s*s + 2*t*s*p1 + t*t*p2`.

#[inline]
fn min(a: f64, b: f64) -> f64 {
    if b < a {
        b
    } else {
        a
    }
}

/// `myBlendFunc(t)(x)`.
fn blend(_t: f64, x: f64) -> f64 {
    let x = x.abs();
    if x >= 1.0 {
        return 0.0;
    }
    // curve: p0 = (0,1), p1 = (0.5*(1-t), 1), p2 = (1,0); only y is observed
    let s = 1.0 - x;
    (1.0 * s * s + 2.0 * x * s * 1.0) + x * x * 0.0
}

/// `setParameters_` factors. Writes `left` and `right`.
///
/// # Safety
/// The outputs must be valid (null is refused).
#[no_mangle]
pub unsafe extern "C" fn flare_nsbezier_potential_params(
    stroke_length: f64,
    length_at_param: f64,
    action_length: f64,
    left: *mut f64,
    right: *mut f64,
) -> i32 {
    if left.is_null() || right.is_null() {
        return 1;
    }
    let half = action_length * 0.5;
    *left = min(length_at_param, half);
    *right = if (stroke_length - length_at_param).abs() < 0.001 {
        0.0
    } else {
        min(stroke_length - length_at_param, half)
    };
    0
}

/// `compute_shape`: `x_len` is `ref_->getLength(w)`.
#[no_mangle]
pub extern "C" fn flare_nsbezier_potential_shape(x_len: f64, action_length: f64, length_at_param: f64) -> f64 {
    let mut shape = action_length * 0.5;
    if -1e-8 < shape && shape < 1e-8 {
        shape = 1.0;
    }
    (x_len - length_at_param) / shape
}

/// `compute_value`. `x_len` = `getLength(w)`, `len_at_1` = `getLength(1.0)`, `len_at_0` =
/// `getLength(0.0)` (only read on the matching branch).
#[no_mangle]
pub extern "C" fn flare_nsbezier_potential_value(
    x_len: f64,
    len_at_1: f64,
    len_at_0: f64,
    action_length: f64,
    stroke_length: f64,
    length_at_param: f64,
    left: f64,
    right: f64,
) -> f64 {
    let shape = |len: f64| flare_nsbezier_potential_shape(len, action_length, length_at_param);
    let tolerance = 0.0;
    let mut x = x_len;
    if length_at_param.max(0.0) < tolerance || (stroke_length - length_at_param).max(0.0) < tolerance {
        let tmp_al = action_length * 0.5;
        if left <= tolerance {
            x = 1.0 - x / tmp_al;
        } else {
            x = (x - (stroke_length - tmp_al)) / tmp_al;
        }
        if x < 0.0 {
            return 0.0;
        }
        // std::min(x, 1.0)
        if 1.0 < x {
            x = 1.0;
        }
        x * x
    } else {
        let len = x_len;
        let min_level = 0.01;
        if len >= length_at_param {
            let tmp_res = blend(0.0, shape(len_at_1));
            if tmp_res > min_level {
                x = if right != 0.0 { (len - length_at_param) / right } else { 0.0 };
                if -1e-8 < x && x < 1e-8 {
                    x = 0.0;
                }
                if (x - 1.0).abs() < 1e-8 {
                    x = 1.0;
                }
                let how_many = (stroke_length - length_at_param) / (action_length * 0.5);
                return blend(how_many, x);
            }
        } else {
            let tmp_res = blend(0.0, shape(len_at_0));
            if tmp_res > min_level {
                let xx = len / left;
                let diff = xx - 1.0;
                let how_many = length_at_param / (action_length * 0.5);
                return blend(how_many, diff);
            }
        }
        blend(0.0, shape(x_len))
    }
}
