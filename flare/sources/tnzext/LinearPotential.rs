//! Port of `tnzext/LinearPotential.cpp`: the arithmetic of `ToonzExt::LinearPotential`.
//! The stroke queries (`TStroke::getLength`) stay in the header's inline members, which hand
//! the measured lengths over as plain numbers; this file turns them into factors and values.

/// `setParameters_` factors: from `getLength()` and `getLength(par)`.
/// Writes `left`, `right` and `range`.
///
/// # Safety
/// The three outputs must be valid (null is refused).
#[no_mangle]
pub unsafe extern "C" fn flare_linear_potential_params(
    stroke_length: f64,
    length_at_param: f64,
    left: *mut f64,
    right: *mut f64,
    range: *mut f64,
) -> i32 {
    if left.is_null() || right.is_null() || range.is_null() {
        return 1;
    }
    *left = length_at_param;
    *right = stroke_length - length_at_param;
    *range = 2.8;
    0
}

/// `compute_shape`: `x_len` is `ref_->getLength(value2test)`.
#[no_mangle]
pub extern "C" fn flare_linear_potential_shape(x_len: f64, action_length: f64, length_at_param: f64, range: f64) -> f64 {
    let mut shape = action_length * 0.5;
    if -1e-8 < shape && shape < 1e-8 {
        shape = 1.0;
    }
    ((x_len - length_at_param) * range) / shape
}

/// `compute_value`: `x_len` is `ref_->getLength(value2test)`.
#[no_mangle]
pub extern "C" fn flare_linear_potential_value(
    x_len: f64,
    par: f64,
    action_length: f64,
    stroke_length: f64,
    length_at_param: f64,
    left: f64,
    right: f64,
) -> f64 {
    let mut x = x_len;
    let mut tmp_al = action_length * 0.5;
    if (-1e-8 < par && par < 1e-8) || (par - 1.0).abs() < 1e-8 {
        tmp_al = action_length;
    }
    if left == 0.0 {
        x = 1.0 - x / tmp_al;
    } else if right == 0.0 {
        x = (x - (stroke_length - tmp_al)) / tmp_al;
    } else if x <= length_at_param && (length_at_param - x) <= left {
        x = (x - (length_at_param - left)) / left;
    } else if x > length_at_param && (x - length_at_param) < right {
        x = (right - (x - length_at_param)) / right;
    } else {
        x = -1.0;
    }
    if x < 0.0 {
        return 0.0;
    }
    x
}
