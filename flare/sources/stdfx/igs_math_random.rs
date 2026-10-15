//! Port of stdfx/igs_math_random.cpp: portable linear congruential generator
//! (glibc-style constants 1103515245 / 12345) used by the hls/hsv noise fx.
//! `unsigned long` / `long` follow the C++ ABI of the target (c_ulong / c_long).

use std::os::raw::{c_long, c_ulong};

/// igs::math::random::random(): seed_(1)
pub const DEFAULT_SEED: c_ulong = 1;

/// igs::math::random::next(): advances `seed` and returns 0..=long::MAX
pub fn next(seed: &mut c_ulong) -> c_long {
    *seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
    /* 0x41C64E6D + 0x3039 */
    (*seed % ((c_long::MAX as c_ulong) + 1)) as c_long
}

/// igs::math::random::next_d(): 0 ... 1
pub fn next_d(seed: &mut c_ulong) -> f64 {
    next(seed) as f64 / c_long::MAX as f64
}

/// # Safety
/// `seed` must be valid for reads and writes.
#[no_mangle]
pub unsafe extern "C" fn flare_igs_random_next(seed: *mut c_ulong) -> c_long {
    next(&mut *seed)
}

/// # Safety
/// `seed` must be valid for reads and writes.
#[no_mangle]
pub unsafe extern "C" fn flare_igs_random_next_d(seed: *mut c_ulong) -> f64 {
    next_d(&mut *seed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_values_from_default_seed() {
        // seed 1 -> 1*1103515245 + 12345 = 1103527590
        let mut s = DEFAULT_SEED;
        assert_eq!(next(&mut s), 1103527590);
        assert_eq!(s, 1103527590);
        // second step: 1103527590*1103515245 + 12345 (mod 2^BITS) then % 2^(BITS-1)
        let expect = (1103527590 as c_ulong).wrapping_mul(1103515245).wrapping_add(12345);
        assert_eq!(next(&mut s), (expect % ((c_long::MAX as c_ulong) + 1)) as c_long);
    }

    #[test]
    fn stays_in_range_and_next_d_in_unit_interval() {
        let mut s: c_ulong = 42;
        for _ in 0..1000 {
            let v = next(&mut s);
            assert!(v >= 0);
            let d = next_d(&mut s);
            assert!((0.0..=1.0).contains(&d));
        }
    }

    #[test]
    fn seed_zero_gives_constant_then_progresses() {
        let mut s: c_ulong = 0;
        assert_eq!(next(&mut s), 12345);
    }

    #[test]
    fn ffi() {
        let mut s: c_ulong = 1;
        unsafe {
            assert_eq!(flare_igs_random_next(&mut s), 1103527590);
            let d = flare_igs_random_next_d(&mut s);
            assert!((0.0..=1.0).contains(&d));
        }
    }
}
