//! Port of stdfx/igs_gauss_distribution.cpp: normalised 1-D gaussian brush.
//!
//! ```text
//!   sequence[0 .. int_diameter-1]:  |<---------- int_diameter ---------->|
//!   the (2*int_radius+1)-wide window is centred; real_radius (<= int_radius)
//!   selects where the -1.0 position of the bell falls inside the last pixel.
//! ```

/// gauss_distribution_1d_(sequence, int_diameter, int_radius, real_radius, sigma)
///
/// # Safety
/// `sequence` must be valid for `int_diameter` doubles and the computed window
/// (`start_pos..end_pos`) must lie inside it, exactly as the C++ requires.
pub unsafe fn gauss_distribution_1d(
    sequence: *mut f64, int_diameter: i32, int_radius: i32, real_radius: f64, sigma: f64, /* = 0.25 */
) {
    /* clear memory */
    for ii in 0..int_diameter {
        *sequence.offset(ii as isize) = 0.0;
    }

    /* allow a zero radius, by force... */
    let int_radius2 = if 0 < int_radius { int_radius } else { 1 };
    let real_radius2 = if 0.0 < real_radius { real_radius } else { 0.000000000000001 };

    /* sub-pixel adjustment: the -1.0 position is placed at real_radius2 */
    let mut dd = -1.0 * int_radius2 as f64 / real_radius2;

    /* difference for 1 pixel */
    let delta = -dd / int_radius2 as f64;

    /* start position */
    let start_pos = (int_diameter - (int_radius2 * 2 + 1)) / 2;

    /* end position */
    let end_pos = int_diameter - start_pos;

    /* gaussian distribution */
    for ii in start_pos..end_pos {
        *sequence.offset(ii as isize) = (-(dd * dd) / (2.0 * sigma * sigma)).exp();
        dd += delta;
    }
    /* accumulate */
    dd = 0.0;
    for ii in start_pos..end_pos {
        dd += *sequence.offset(ii as isize);
    }
    /* normalise (divide so that the sum is 1) */
    for ii in start_pos..end_pos {
        *sequence.offset(ii as isize) /= dd;
    }
}

/// # Safety
/// See [`gauss_distribution_1d`].
#[no_mangle]
pub unsafe extern "C" fn flare_igs_gauss_distribution_1d(
    sequence: *mut f64, int_diameter: i32, int_radius: i32, real_radius: f64, sigma: f64,
) {
    gauss_distribution_1d(sequence, int_diameter, int_radius, real_radius, sigma)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(diam: i32, r: i32, rr: f64, sigma: f64) -> Vec<f64> {
        let mut v = vec![-1.0; diam as usize];
        unsafe { gauss_distribution_1d(v.as_mut_ptr(), diam, r, rr, sigma) };
        v
    }

    #[test]
    fn sums_to_one_and_is_symmetric() {
        let v = run(9, 4, 4.0, 0.25);
        let sum: f64 = v.iter().sum();
        assert!((sum - 1.0).abs() < 1e-12);
        for i in 0..9 {
            assert!((v[i] - v[8 - i]).abs() < 1e-15);
        }
        // bell: centre is the maximum
        assert!(v.iter().all(|&x| x <= v[4]));
    }

    #[test]
    fn cells_outside_window_are_zero() {
        // diameter 11, radius 2 -> window 5 wide starting at (11-5)/2 = 3
        let v = run(11, 2, 2.0, 0.25);
        for i in (0..3).chain(8..11) {
            assert_eq!(v[i], 0.0);
        }
        assert!((v[3..8].iter().sum::<f64>() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn zero_radius_is_forced_to_one() {
        // radius 0 -> int_radius2 = 1 (3 wide window), real radius -> 1e-15 (tiny): only centre survives
        let v = run(3, 0, 0.0, 0.25);
        assert!((v[1] - 1.0).abs() < 1e-12);
        assert_eq!(v[0], 0.0);
        assert_eq!(v[2], 0.0);
    }

    #[test]
    fn exact_three_point_values() {
        // int_radius 1, real_radius 1: dd = -1, delta = 1 -> exp(-16)*, exp(0), exp(-16) with sigma .25
        let v = run(3, 1, 1.0, 0.25);
        let e = (-(1.0f64) / (2.0 * 0.25 * 0.25)).exp();
        let total = 1.0 + 2.0 * e;
        assert!((v[0] - e / total).abs() < 1e-15);
        assert!((v[1] - 1.0 / total).abs() < 1e-15);
    }
}
