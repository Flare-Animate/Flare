//! Port of `tnzext/Potential.cpp`: the argument checks of `ToonzExt::Potential::setParameters`
//! and the clamp of `Potential::value`. The class (virtual `value_` / `setParameters_`, the
//! `isValid_` flag) stays in `ext/Potential.h`, which now defines both members inline and
//! throws the original `std::invalid_argument` / `std::range_error`.

const EPSILON: f64 = 1e-8; // TConsts::epsilon

/// Status codes of [`flare_potential_check_params`].
pub const POTENTIAL_OK: i32 = 0;
pub const POTENTIAL_NO_STROKE: i32 = 1; // "Not valid stroke!!!"
pub const POTENTIAL_BAD_PARAMETER: i32 = 2; // "Not valid parameter!!!"

/// Checks of `Potential::setParameters(ref, w, actionLength)`; writes the action length to
/// use (`TConsts::epsilon` when 0) to `*action_length_out`.
///
/// # Safety
/// `action_length_out` must be null or valid for one `double`.
#[no_mangle]
pub unsafe extern "C" fn flare_potential_check_params(
    has_stroke: i32,
    w: f64,
    action_length: f64,
    action_length_out: *mut f64,
) -> i32 {
    std::panic::catch_unwind(|| {
        if has_stroke == 0 {
            return POTENTIAL_NO_STROKE;
        }
        let al = if action_length == 0.0 { EPSILON } else { action_length };
        if !action_length_out.is_null() {
            *action_length_out = al;
        }
        if 0.0 > w || w > 1.0 {
            return POTENTIAL_BAD_PARAMETER;
        }
        POTENTIAL_OK
    })
    .unwrap_or(POTENTIAL_BAD_PARAMETER)
}

/// `if (0 > at || at > 1.0) at = std::min(std::max(at, 0.0), 1.0);` (NaN passes through).
#[no_mangle]
pub extern "C" fn flare_potential_clamp_value(at: f64) -> f64 {
    if 0.0 > at || at > 1.0 {
        // std::min(std::max(at, 0.0), 1.0)
        let m = if at < 0.0 { 0.0 } else { at };
        if 1.0 < m {
            1.0
        } else {
            m
        }
    } else {
        at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks() {
        let mut al = 0.0;
        unsafe {
            assert_eq!(flare_potential_check_params(0, 0.5, 3.0, &mut al), POTENTIAL_NO_STROKE);
            assert_eq!(flare_potential_check_params(1, 0.5, 0.0, &mut al), POTENTIAL_OK);
            assert_eq!(al, 1e-8);
            assert_eq!(flare_potential_check_params(1, 1.5, 2.0, &mut al), POTENTIAL_BAD_PARAMETER);
            assert_eq!(al, 2.0);
            assert_eq!(flare_potential_check_params(1, -0.1, 2.0, std::ptr::null_mut()), POTENTIAL_BAD_PARAMETER);
            assert_eq!(flare_potential_check_params(1, f64::NAN, 2.0, &mut al), POTENTIAL_OK);
        }
        assert_eq!(flare_potential_clamp_value(-3.0), 0.0);
        assert_eq!(flare_potential_clamp_value(3.0), 1.0);
        assert_eq!(flare_potential_clamp_value(0.25), 0.25);
        assert!(flare_potential_clamp_value(f64::NAN).is_nan());
    }

    /// Replays `tests/native/gen_potentials_cases.py`: the original `Potential` and the four
    /// potential classes (g++, `-ffp-contract=off`, stub stroke) against the Rust kernels
    /// wired the way the inline headers wire them.
    #[test]
    fn matches_recorded_cpp() {
        use crate::linearpotential as lin;
        use crate::nsbezierpotential as bez;
        use crate::nsexppotential as nsexp;
        use crate::squarepotential as sq;
        let golden = include_str!("potentials_cases.txt");
        let len = |l: f64, w: f64| l * (0.7 * w + 0.3 * w * w);
        let f = |h: &str| f64::from_bits(u64::from_str_radix(h, 16).unwrap());
        let (mut records, mut values, mut throws) = (0, 0, 0);
        for line in golden.lines() {
            let t: Vec<&str> = line.split(' ').collect();
            let kind = t[0];
            if !["lin", "sq", "bez", "exp"].contains(&kind) {
                match kind {
                    "uninit" | "nostroke" | "badw" | "badw2" | "clamp" => {
                        values += 1;
                        continue;
                    }
                    _ => panic!("{line}"),
                }
            }
            records += 1;
            let (l, par, al) = (f(t[1]), f(t[2]), f(t[3]));
            // Potential::setParameters
            let mut al2 = al;
            let rc = unsafe { flare_potential_check_params(1, par, al, &mut al2) };
            if t[4] == "throw" {
                assert_eq!(rc, POTENTIAL_BAD_PARAMETER, "{line}");
                assert_eq!(t[5..].join(" "), "Not valid parameter!!!");
                throws += 1;
                continue;
            }
            assert_eq!(rc, POTENTIAL_OK, "{line}");
            let al = al2;
            let stroke = len(l, 1.0);
            let lap = len(l, par);
            let (mut left, mut right, mut range) = (0.0, 0.0, 0.0);
            unsafe {
                match kind {
                    "lin" => {
                        lin::flare_linear_potential_params(stroke, lap, &mut left, &mut right, &mut range);
                    }
                    "sq" => {
                        sq::flare_square_potential_params(stroke, lap, al, &mut left, &mut right, &mut range);
                    }
                    "bez" => {
                        bez::flare_nsbezier_potential_params(stroke, lap, al, &mut left, &mut right);
                    }
                    _ => {
                        nsexp::flare_nsexp_potential_params(stroke, lap, al, &mut left, &mut right, &mut range);
                    }
                }
            }
            for pair in &t[4..] {
                let (wh, vh) = pair.split_once(':').unwrap();
                let w = flare_potential_clamp_value(f(wh));
                let x = len(l, w);
                let got = match kind {
                    "lin" => lin::flare_linear_potential_value(x, par, al, stroke, lap, left, right),
                    "sq" => sq::flare_square_potential_value(x, al, stroke, lap, left, right),
                    "bez" => bez::flare_nsbezier_potential_value(x, len(l, 1.0), len(l, 0.0), al, stroke, lap, left, right),
                    _ => nsexp::flare_nsexp_potential_value(x, len(l, 1.0), len(l, 0.0), al, stroke, lap, left, right, range),
                };
                assert_eq!(got.to_bits(), f(vh).to_bits(), "{line} w={w}");
                values += 1;
            }
        }
        assert_eq!(records, 600);
        assert_eq!((throws, values), (0, 4805));
    }
}
