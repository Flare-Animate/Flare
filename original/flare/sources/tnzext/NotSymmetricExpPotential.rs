//! Port of `tnzext/NotSymmetricExpPotential.cpp`: the arithmetic of
//! `ToonzExt::NotSymmetricExpPotential` (see `LinearPotential.rs` for the split with the
//! header). Uses libm `exp`, so results are bit-identical to the C++ on the same libm.

#[inline]
fn min(a: f64, b: f64) -> f64 {
    if b < a {
        b
    } else {
        a
    }
}

fn me(x: f64) -> f64 {
    (-(x * x)).exp()
}

fn ms(x: f64) -> f64 {
    1.0 - x * x
}

fn blender(a: f64, b: f64, t: f64) -> f64 {
    a * (1.0 - t) + b * t
}

/// `setParameters_` factors. Writes `left`, `right` and `range`.
///
/// # Safety
/// The outputs must be valid (null is refused).
#[no_mangle]
pub unsafe extern "C" fn flare_nsexp_potential_params(
    stroke_length: f64,
    length_at_param: f64,
    action_length: f64,
    left: *mut f64,
    right: *mut f64,
    range: *mut f64,
) -> i32 {
    if left.is_null() || right.is_null() || range.is_null() {
        return 1;
    }
    let half = action_length * 0.5;
    *left = min(length_at_param, half);
    *right = if (stroke_length - length_at_param).abs() < 0.001 {
        0.0
    } else {
        min(stroke_length - length_at_param, half)
    };
    *range = 2.8;
    0
}

/// `compute_shape`: `x_len` is `ref_->getLength(w)`.
#[no_mangle]
pub extern "C" fn flare_nsexp_potential_shape(x_len: f64, action_length: f64, length_at_param: f64, range: f64) -> f64 {
    let mut shape = action_length * 0.5;
    if -1e-8 < shape && shape < 1e-8 {
        shape = 1.0;
    }
    ((x_len - length_at_param) * range) / shape
}

/// `compute_value`. `x_len` = `getLength(w)`, `len_at_1` = `getLength(1.0)`, `len_at_0` =
/// `getLength(0.0)`.
#[no_mangle]
pub extern "C" fn flare_nsexp_potential_value(
    x_len: f64,
    len_at_1: f64,
    len_at_0: f64,
    action_length: f64,
    stroke_length: f64,
    length_at_param: f64,
    left: f64,
    right: f64,
    range: f64,
) -> f64 {
    let shape = |len: f64| flare_nsexp_potential_shape(len, action_length, length_at_param, range);
    let tolerance = 2.0;
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
        x * x
    } else {
        let len = x_len;
        let min_level = 0.01;
        if len >= length_at_param {
            if me(shape(len_at_1)) > min_level {
                x = (len - length_at_param) / right;
                let exp_val = me(x * range);
                let how_many = (stroke_length - length_at_param) / (action_length * 0.5);
                return blender(ms(x), exp_val, how_many);
            }
        } else if me(shape(len_at_0)) > min_level {
            let xx = len / left;
            let diff = xx - 1.0;
            let exp_val = me(diff * range);
            let how_many = length_at_param / (action_length * 0.5);
            return blender(ms(diff), exp_val, how_many);
        }
        me(shape(x_len))
    }
}
